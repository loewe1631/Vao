# CI-only install/update/uninstall fixture. Never run against an existing installation.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$release = Get-Content (Join-Path $root 'release.windows.json') -Raw | ConvertFrom-Json
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') { throw 'Installer lifecycle fixture is restricted to GitHub Windows runners' }
$uninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\' + $release.product
if ((Test-Path ('HKCU:\' + $uninstallKey)) -or (Test-Path ('HKLM:\' + $uninstallKey))) { throw 'Refusing to replace an existing installation' }
$installer = @(Get-ChildItem (Join-Path $root 'target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe'))
if ($installer.Count -ne 1) { throw 'Expected one candidate installer' }
$name = [string]$release.bundle
$run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$approval = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run'
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('vao-install-probe-' + [Guid]::NewGuid().ToString('N'))
$installed = Join-Path $fixture 'installed'
$binary = Join-Path $installed 'vao.exe'
$uninstaller = Join-Path $installed 'uninstall.exe'
$expected = '"' + $binary + '" --autostart'
$other = '"' + (Join-Path $fixture 'newer/vao.exe') + '" --autostart'
$checks = [ordered]@{}
function Read-Startup {
    if (!(Test-Path $run)) { return $null }
    return Get-ItemPropertyValue -Path $run -Name $name -ErrorAction SilentlyContinue
}
function Run-Installer([string[]]$Arguments) {
    $p = Start-Process -FilePath $installer[0].FullName -ArgumentList $Arguments -PassThru
    try { if (!$p.WaitForExit(60000)) { throw 'Fixture installation timed out' }; if ($p.ExitCode -ne 0) { throw 'Fixture installation failed' } }
    finally { if (!$p.HasExited) { Stop-Process -Id $p.Id -Force } }
}
function Run-Uninstaller {
    # _?= runs the owned uninstaller in place, so waiting covers actual deletion.
    $p = Start-Process -FilePath $uninstaller -ArgumentList @('/S', ('_?=' + $installed)) -PassThru
    try { if (!$p.WaitForExit(60000)) { throw 'Fixture uninstall timed out' }; if ($p.ExitCode -ne 0) { throw 'Fixture uninstall failed' } }
    finally { if (!$p.HasExited) { Stop-Process -Id $p.Id -Force } }
}
foreach ($keyPath in @($run, $approval)) {
    if ((Test-Path $keyPath) -and (Get-Item $keyPath).GetValueNames() -contains $name) { throw 'Refusing to touch an existing bundle startup value' }
}
try {
    Run-Installer @('/S', ('/D=' + $installed))
    $checks.installer_has_standalone_application = Test-Path $binary
    $checks.install_does_not_enable_startup = !(Read-Startup)
    New-Item -Path $run -Force | Out-Null
    New-ItemProperty -Path $run -Name $name -PropertyType String -Value $expected -Force | Out-Null
    New-Item -Path $approval -Force | Out-Null
    $disabled = [byte[]]@(3,0,0,0,1,2,3,4,5,6,7,8)
    New-ItemProperty -Path $approval -Name $name -PropertyType Binary -Value $disabled -Force | Out-Null
    Run-Installer @('/S', '/UPDATE', ('/D=' + $installed))
    $checks.in_place_update_keeps_opt_in = (Read-Startup) -ceq $expected
    $actual = Get-ItemPropertyValue -Path $approval -Name $name
    $checks.update_preserves_system_disable = [Convert]::ToBase64String($actual) -ceq [Convert]::ToBase64String($disabled)
    New-ItemProperty -Path $run -Name $name -PropertyType String -Value $other -Force | Out-Null
    Run-Uninstaller
    $checks.old_uninstaller_preserves_newer_path = (Read-Startup) -ceq $other
    Remove-ItemProperty -Path $run -Name $name
    Remove-ItemProperty -Path $approval -Name $name
    Run-Installer @('/S', ('/D=' + $installed))
    New-ItemProperty -Path $run -Name $name -PropertyType String -Value $expected -Force | Out-Null
    New-ItemProperty -Path $approval -Name $name -PropertyType Binary -Value $disabled -Force | Out-Null
    Run-Uninstaller
    $checks.uninstall_removes_owned_startup = !(Read-Startup)
    $checks.uninstall_removes_owned_approval = !(Get-ItemPropertyValue -Path $approval -Name $name -ErrorAction SilentlyContinue)
    $report = [ordered]@{passed=!($checks.Values -contains $false);checks=@($checks.GetEnumerator() | ForEach-Object { @{name=$_.Key;passed=$_.Value} });scope='owned CI installer fixture, no login or reboot'}
    [IO.File]::WriteAllText((Join-Path $root 'dist/windows/runner-installer-startup.json'), ($report | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
    if (!$report.passed) { throw 'Installer startup lifecycle checks failed' }
    Write-Output 'Installer startup opt-in, update, system-disable and owned uninstall checks passed.'
} finally {
    if ((Read-Startup) -in @($expected, $other)) { Remove-ItemProperty -Path $run -Name $name -ErrorAction SilentlyContinue }
    Remove-ItemProperty -Path $approval -Name $name -ErrorAction SilentlyContinue
    if (Test-Path $uninstaller) { Run-Uninstaller }
    if (Test-Path $fixture) { Remove-Item -LiteralPath $fixture -Recurse -Force }
}
