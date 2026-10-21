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
        $deadline = [DateTime]::UtcNow.AddSeconds(60)
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
        if (!$process.WaitForExit(5000)) { throw 'Owned probe process did not exit' }
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
foreach ($check in @('frame_free', 'no_activate', 'no_edge', 'bottom_retained', 'focus_preserved', 'content_height_changed', 'style_guarded', 'click_no_activate', 'click_preserves_position', 'drag_moves_without_frame', 'release_stops_drag')) {
    if ($smoke.overlay.$check -ne $true) { throw "Native overlay failed: $check" }
}
Write-Output 'Windows build identity, WebView IPC and shortcut registration passed; no audio/model request performed.'
$inputPath = Join-Path $output 'runner-input.json'
$inputProbe = Run-Probe -ProbeArgs @('--windows-input-probe', '--smoke-test', ('"' + $inputPath + '"')) -ReportPath $inputPath
if ($inputProbe.passed -ne $true -or $inputProbe.audio_captured -ne $false -or
    $inputProbe.models_contacted -ne $false -or $inputProbe.checks.Count -lt 20 -or
    @($inputProbe.checks | Where-Object { $_.passed -ne $true }).Count -ne 0) {
    throw ('Native WebView2 input receipt failed: ' + ($inputProbe | ConvertTo-Json -Depth 8 -Compress))
}
Write-Output 'Generic UI Automation WebView2 receipt, target/caret conflicts and non-sending checks passed.'
$unicodePath = Join-Path $output 'runner-unicode.json'
$unicodeProbe = Run-Probe -ProbeArgs @('--windows-input-probe', '--windows-unicode-preview', '--smoke-test', ('"' + $unicodePath + '"')) -ReportPath $unicodePath
$requiredUnicode = @('unicode_uses_verified_uia_snapshot', 'unicode_changed_content_rejected', 'unicode_changed_caret_rejected', 'unicode_changed_selection_rejected', 'unicode_trusted_input_updates_web_model', 'unicode_one_undo_restores_selection_text', 'unicode_redo_restores_inserted_text', 'unicode_multiline_does_not_guess_enter_semantics', 'unicode_returns_from_other_window', 'unicode_normalized_field_received_phrase', 'unicode_normalized_field_keeps_receipt_warning', 'unicode_normalized_field_token_cannot_replay', 'unicode_real_lf_at_end_is_not_empty_paragraph', 'unicode_empty_name_content_change_rejected', 'unicode_empty_name_change_rejected', 'unicode_nonempty_name_is_preserved', 'unicode_seven_field_shapes_observed', 'unicode_never_enter_or_submit')
if (!$unicodeProbe.passed -or $unicodeProbe.audio_captured -ne $false -or $unicodeProbe.models_contacted -ne $false -or
    $unicodeProbe.checks.Count -lt 62 -or @($unicodeProbe.checks | Where-Object { $_.passed -ne $true }).Count -ne 0) {
    throw ('Guarded Unicode input failed: ' + ($unicodeProbe | ConvertTo-Json -Depth 8 -Compress))
}
foreach ($name in $requiredUnicode) {
    if (!($unicodeProbe.checks | Where-Object { $_.name -eq $name -and $_.passed })) { throw "Missing guarded Unicode regression: $name" }
}
foreach ($shape in @('input_placeholder', 'textarea_placeholder', 'editable_empty', 'editable_paragraph_br', 'editable_paragraph_start', 'editable_generated_placeholder', 'editable_existing_text')) {
    $observation = @($unicodeProbe.observations | Where-Object { $_.shape -eq $shape })
    $expectedVerified = $shape -ne 'editable_generated_placeholder'
    if ($observation.Count -ne 1 -or !$observation[0].capture_supported -or !$observation[0].text_received_exactly -or
        $observation[0].receipt_verified -ne $expectedVerified) {
        throw "Missing or inconsistent field-shape observation: $shape"
    }
}
Write-Output 'Guarded Unicode snapshot, native receipt, input events, undo and conflict checks passed.'
$pastePath = Join-Path $output 'runner-paste.json'
$pasteProbe = Run-Probe -ProbeArgs @('--windows-input-probe', '--windows-paste-preview', '--smoke-test', ('"' + $pastePath + '"')) -ReportPath $pastePath
$requiredPaste = @('paste_selected_unicode_receipt', 'paste_single_trusted_input', 'paste_multiline_receipt', 'paste_one_undo', 'paste_changed_field_no_dispatch', 'paste_later_copy_not_overwritten', 'paste_marker_mismatch_not_overwritten', 'paste_delayed_consumer_exactly_once', 'paste_unconfirmed_keeps_result_clipboard', 'paste_unconfirmed_token_cannot_replay', 'paste_never_enter_or_submit', 'windows_context_changed_field_omitted')
if (!$pasteProbe.passed -or $pasteProbe.audio_captured -ne $false -or $pasteProbe.models_contacted -ne $false -or
    $pasteProbe.checks.Count -lt 27 -or @($pasteProbe.checks | Where-Object { $_.passed -ne $true }).Count -ne 0) {
    throw ('Native paste/clipboard regression failed: ' + ($pasteProbe | ConvertTo-Json -Depth 8 -Compress))
}
foreach ($name in $requiredPaste) {
    if (!($pasteProbe.checks | Where-Object { $_.name -eq $name -and $_.passed })) { throw "Missing native paste regression: $name" }
}
Write-Output 'Native paste receipt, clipboard ownership, delayed consumers and bounded context checks passed.'
