param([string]$Binary = 'target/x86_64-pc-windows-msvc/release/vao.exe')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$release = Get-Content (Join-Path $root 'release.windows.json') -Raw | ConvertFrom-Json
$binaryPath = (Resolve-Path (Join-Path $root $Binary)).Path
$output = Join-Path $root 'dist/windows'
New-Item -ItemType Directory -Force $output | Out-Null
$identityPath = Join-Path $output 'build-info.json'
$smokePath = Join-Path $output 'runner-startup.json'

function Run-Probe([string[]]$ProbeArgs, [string]$ReportPath) {
    if (Test-Path $ReportPath) { Remove-Item $ReportPath }
    $process = Start-Process -FilePath $binaryPath -ArgumentList $ProbeArgs -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(45)
        while (!(Test-Path $ReportPath) -and [DateTime]::UtcNow -lt $deadline) {
            if ($process.HasExited) { break }
            Start-Sleep -Milliseconds 250
        }
        if (!(Test-Path $ReportPath)) { throw "Probe produced no report: $ReportPath" }
        # Wait for the writer to finish flushing; no recorded content is involved.
        Start-Sleep -Milliseconds 250
        return Get-Content $ReportPath -Raw | ConvertFrom-Json
    } finally {
        if (!$process.HasExited) { Stop-Process -Id $process.Id -Force }
    }
}

$identity = Run-Probe -ProbeArgs @('--build-info', ('"' + $identityPath + '"')) -ReportPath $identityPath
if ($identity.version -ne $release.version -or $identity.build -ne $release.build -or
    $identity.bundle -ne $release.bundle -or $identity.platform -ne 'windows' -or $identity.architecture -ne 'x86_64') {
    throw 'Binary build identity mismatch'
}
$smoke = Run-Probe -ProbeArgs @('--windows-smoke', '--smoke-test', ('"' + $smokePath + '"')) -ReportPath $smokePath
if ($smoke.ipc -ne 'ready' -or $smoke.webview -ne 'main' -or $smoke.phase -ne 'idle' -or
    $smoke.bundle -ne $release.bundle -or $smoke.version -ne $release.version -or
    !$smoke.hotkeys -or $smoke.captured_audio -ne $false -or $smoke.trial.provisioned) {
    throw 'Native WebView/IPC startup probe failed'
}
foreach ($check in @('frame_free', 'no_activate', 'no_edge', 'bottom_retained', 'focus_preserved', 'content_height_changed')) {
    if ($smoke.overlay.$check -ne $true) { throw "Native overlay failed: $check" }
}
Write-Output 'Windows build identity, WebView IPC and shortcut registration passed; no audio/model request performed.'
