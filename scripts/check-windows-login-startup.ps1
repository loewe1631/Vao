param([string]$Binary = 'target/x86_64-pc-windows-msvc/release/vao.exe')
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') { throw 'Background startup fixture is restricted to GitHub Windows runners' }
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class VaoLoginWindowProbe {
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder value, int size);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out Rect r);
    struct Rect { public int left, top, right, bottom; }
    public static string Describe(IntPtr h) {
        var name = new StringBuilder(256); GetClassName(h, name, name.Capacity);
        Rect r; GetWindowRect(h, out r);
        return name + " " + (r.right-r.left) + "x" + (r.bottom-r.top);
    }
}
'@
$root = Split-Path $PSScriptRoot -Parent
$binaryPath = (Resolve-Path (Join-Path $root $Binary)).Path
$output = Join-Path $root 'dist/windows/runner-login-startup.json'
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('vao-login-probe-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$initialPath = Join-Path $fixture 'initial.json'
$process = $null
$checks = [ordered]@{}
try {
    $process = Start-Process -FilePath $binaryPath -ArgumentList @('--autostart', '--windows-smoke', '--smoke-test', ('"' + $initialPath + '"')) -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(60)
    while (!(Test-Path $initialPath) -and [DateTime]::UtcNow -lt $deadline -and !$process.HasExited) { Start-Sleep -Milliseconds 250 }
    if (!(Test-Path $initialPath)) { throw 'Background startup produced no IPC report' }
    Start-Sleep -Milliseconds 250
    $initial = Get-Content $initialPath -Raw | ConvertFrom-Json
    # Process.MainWindowHandle can select a WebView/framework helper. Check the
    # actual Tauri main window, and first verify it belongs to this owned process.
    $mainHandle = [IntPtr]([long]$initial.main_native_handle)
    [uint32]$windowPid = 0
    [VaoLoginWindowProbe]::GetWindowThreadProcessId($mainHandle, [ref]$windowPid) | Out-Null
    if (![VaoLoginWindowProbe]::IsWindow($mainHandle) -or $windowPid -ne $process.Id) { throw 'Missing owned native main window' }
    $checks.background_main_hidden = $initial.main_visible -eq $false -and $initial.login_background -eq $true
    $checks.background_tray_and_shortcuts_ready = $initial.tray_ready -eq $true -and $initial.hotkeys -eq $true
    $checks.background_idle_without_capture = $initial.phase -eq 'idle' -and $initial.captured_audio -eq $false -and !$initial.trial.provisioned
    $process.Refresh()
    $backgroundProcessWindow = [VaoLoginWindowProbe]::Describe($process.MainWindowHandle)
    $checks.background_has_no_visible_main = ![VaoLoginWindowProbe]::IsWindowVisible($mainHandle)
    $second = Start-Process -FilePath $binaryPath -ArgumentList @('--autostart') -PassThru
    try {
        if (!$second.WaitForExit(10000)) { throw 'Second background instance did not exit' }
    } finally { if (!$second.HasExited) { Stop-Process -Id $second.Id -Force } }
    Start-Sleep -Milliseconds 500
    $process.Refresh()
    $checks.second_background_keeps_main_hidden = !$process.HasExited -and ![VaoLoginWindowProbe]::IsWindowVisible($mainHandle)
    $manual = Start-Process -FilePath $binaryPath -PassThru
    try {
        if (!$manual.WaitForExit(10000)) { throw 'Second manual instance did not exit' }
    } finally { if (!$manual.HasExited) { Stop-Process -Id $manual.Id -Force } }
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do { $process.Refresh(); if ([VaoLoginWindowProbe]::IsWindowVisible($mainHandle)) { break }; Start-Sleep -Milliseconds 100 } while ([DateTime]::UtcNow -lt $deadline)
    $checks.manual_launch_opens_existing_main = !$process.HasExited -and [VaoLoginWindowProbe]::IsWindowVisible($mainHandle)
    $report = [ordered]@{
        passed = !($checks.Values -contains $false)
        version = $initial.version
        bundle = $initial.bundle
        audio_captured = $false
        models_contacted = $false
        framework_process_window = $backgroundProcessWindow
        checks = @($checks.GetEnumerator() | ForEach-Object { @{name=$_.Key;passed=$_.Value} })
    }
    [IO.File]::WriteAllText($output, ($report | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
    if (!$report.passed) { throw 'Login startup native checks failed' }
    Write-Output 'Login startup remains hidden and idle; manual launch opens the existing instance.'
} finally {
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force; $process.WaitForExit(5000) | Out-Null }
    Remove-Item -LiteralPath $fixture -Recurse -Force
}
