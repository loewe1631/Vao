use serde_json::{json, Value};
#[cfg(target_os = "macos")]
use std::io::{BufRead, BufReader, Write};
#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};
#[cfg(target_os = "windows")]
#[path = "windows_platform.rs"]
pub mod windows;
#[cfg(target_os = "macos")]
struct Bridge {
    child: std::process::Child,
    input: std::process::ChildStdin,
    output: BufReader<std::process::ChildStdout>,
}
#[cfg(target_os = "macos")]
static BRIDGE: std::sync::OnceLock<std::sync::Mutex<Option<Bridge>>> = std::sync::OnceLock::new();
#[cfg(target_os = "macos")]
fn helper_path() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let helper = exe.parent().unwrap().join("../Resources/vao-macos-bridge");
    #[cfg(debug_assertions)]
    let helper = if helper.exists() {
        helper
    } else {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../.build/vao-macos-bridge")
    };
    if !helper.exists() {
        return Err("Bundled macOS helper is missing. Reinstall Vao.".into());
    }
    Ok(helper)
}
#[cfg(target_os = "macos")]
fn dialog_reply(mut command: Command, request: &Value) -> Result<Value, String> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "文件选择器不可用 / File dialog unavailable")?;
    let written = {
        let mut input = child.stdin.take().unwrap();
        writeln!(input, "{request}")
    }; // Closing stdin lets the one-shot helper read and finish its request.
    if written.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return Err("文件选择请求失败 / File dialog request failed".into());
    }
    let output = child.wait_with_output().map_err(|_| "文件选择器异常退出")?;
    if !output.status.success() {
        return Err("文件选择器异常退出 / File dialog exited unexpectedly".into());
    }
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "文件选择器响应无效 / Invalid file dialog response")?;
    if let Some(error) = value["error"].as_str() {
        return Err(error.to_owned());
    }
    if value["cancelled"] != true && value["path"].as_str().filter(|p| !p.is_empty()).is_none() {
        return Err("文件选择器响应无效 / Invalid file dialog response".into());
    }
    Ok(value)
}
pub fn call(request: Value) -> Result<Value, String> {
    #[cfg(target_os = "macos")]
    {
        if matches!(request["action"].as_str(), Some("dictionary_import" | "dictionary_export")) {
            static DIALOG: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let _dialog = DIALOG.lock().unwrap();
            // Modal AppKit windows must not outlive their event loop in the daemon.
            // A separate process also leaves the captured input target available.
            return dialog_reply(Command::new(helper_path()?), &request);
        }
        let mut state = BRIDGE
            .get_or_init(|| std::sync::Mutex::new(None))
            .lock()
            .unwrap();
        if state
            .as_mut()
            .map(|b| b.child.try_wait().ok().flatten().is_some())
            .unwrap_or(false)
        {
            *state = None;
        }
        if state.is_none() {
            let mut command = Command::new(helper_path()?);
            command.arg("--daemon");
            if std::env::args().any(|a| a == "--verify-ui") {
                command.arg("--verify-selection");
            }
            let mut child = command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| "系统适配器不可用 / Platform adapter unavailable")?;
            let input = child.stdin.take().unwrap();
            let output = BufReader::new(child.stdout.take().unwrap());
            *state = Some(Bridge {
                child,
                input,
                output,
            });
        }
        let bridge = state.as_mut().unwrap();
        let result = (|| {
            writeln!(bridge.input, "{}", request).map_err(|_| "系统适配器请求失败")?;
            bridge.input.flush().map_err(|_| "系统适配器请求失败")?;
            let mut line = String::new();
            bridge
                .output
                .read_line(&mut line)
                .map_err(|_| "系统适配器异常退出")?;
            serde_json::from_str::<Value>(&line).map_err(|_| "系统适配器响应无效")
        })();
        if result.is_err() {
            if let Some(mut b) = state.take() {
                let _ = b.child.kill();
                let _ = b.child.wait();
            }
        }
        let v: Value = result?;
        if let Some(e) = v["error"].as_str() {
            return Err(e.to_owned());
        }
        Ok(v)
    }
    #[cfg(target_os = "windows")]
    {
        windows::call(request)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = request;
        Err("This release implements the macOS adapter only".into())
    }
}
pub fn target(selection: bool) -> Result<Value, String> {
    call(json!({"action":"target","selection":selection}))
}
/// No credential payload is sent to the WebView. Windows has no Keychain prompt.
pub fn has_secret(slot: &str) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn vao_has_credential(slot: *const std::ffi::c_char) -> i32;
        }
        let slot = std::ffi::CString::new(slot).map_err(|_| "Invalid credential slot")?;
        match unsafe { vao_has_credential(slot.as_ptr()) } {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("无法检查钥匙串状态 / Keychain status unavailable".into()),
        }
    }
    #[cfg(target_os = "windows")]
    {
        match keyring::Entry::new(env!("VAO_BUNDLE_ID"), slot)
            .map_err(|_| "凭据服务初始化失败")?
            .get_password()
        {
            Ok(_) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(_) => Err("无法检查 Windows 凭据管理器 / Credential Manager unavailable".into()),
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = slot;
        Ok(false)
    }
}
pub fn secret(slot: &str) -> Result<String, String> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        match keyring::Entry::new(env!("VAO_BUNDLE_ID"), slot)
            .map_err(|_| "凭据服务初始化失败")?
            .get_password()
        {
            Ok(s) => Ok(s),
            Err(keyring::Error::NoEntry) => Ok(String::new()),
            Err(_) => Err("无法读取Vao的系统凭据 / System credential unavailable".into()),
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = slot;
        Err("Credential adapter not implemented".into())
    }
}
pub fn save_secret(slot: &str, value: &str) -> Result<(), String> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let e =
            keyring::Entry::new(env!("VAO_BUNDLE_ID"), slot).map_err(|_| "凭据服务初始化失败")?;
        if value.is_empty() {
            match e.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(_) => Err("凭据删除失败".into()),
            }
        } else {
            e.set_password(value).map_err(|_| "凭据保存失败".into())
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (slot, value);
        Err("Credential adapter not implemented".into())
    }
}

pub fn connection_secret(slot: &str, base: &str) -> Result<String, String> {
    decode_secret(&secret(slot)?, base)
}
fn decode_secret(raw: &str, base: &str) -> Result<String, String> {
    if raw.is_empty() {
        return Ok(String::new());
    }
    let value: Value = serde_json::from_str(raw).map_err(|_| "凭据格式已变化，请重新填写")?;
    if value["base"].as_str() != Some(base.trim().trim_end_matches('/')) {
        return Err(
            "服务地址已变化，请重新填写此连接的API Key / Re-enter the key for the changed URL"
                .into(),
        );
    }
    Ok(value["key"].as_str().ok_or("凭据无效")?.to_owned())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    fn dialog_fixture(script: &str) -> Result<Value, String> {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        dialog_reply(command, &json!({"action":"dictionary_import"}))
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn dialog_closes_stdin_and_waits_for_exit() {
        let value = dialog_fixture(
            "IFS= read -r request; [ \"$request\" = '{\"action\":\"dictionary_import\"}' ] || exit 9; if IFS= read -r extra; then exit 10; fi; printf '%s\\n' '{\"cancelled\":true}'",
        ).unwrap();
        assert_eq!(value, json!({"cancelled":true}));
        assert_eq!(dialog_fixture("cat >/dev/null; printf '%s\\n' '{\"path\":\"/tmp/dictionary.json\"}'").unwrap()["path"], "/tmp/dictionary.json");
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn dialog_errors_do_not_masquerade_as_cancellation() {
        assert!(dialog_fixture("cat >/dev/null; printf '%s\\n' '{\"cancelled\":true}'; exit 7").is_err());
        assert!(dialog_fixture("cat >/dev/null; printf 'invalid'").is_err());
        assert!(dialog_fixture("cat >/dev/null; printf '{}'").is_err());
        assert!(dialog_fixture("cat >/dev/null; printf '%s\\n' '{\"path\":\"\"}'").is_err());
        assert_eq!(dialog_fixture("cat >/dev/null; printf '%s\\n' '{\"error\":\"fixture error\"}'").unwrap_err(), "fixture error");
    }
    #[test]
    fn credential_bound_to_service() {
        let raw = r#"{"base":"https://one.example/v1","key":"synthetic"}"#;
        assert_eq!(
            decode_secret(raw, "https://one.example/v1/").unwrap(),
            "synthetic"
        );
        assert!(decode_secret(raw, "https://other.example/v1").is_err());
        assert!(decode_secret("", "https://other.example")
            .unwrap()
            .is_empty());
    }
}
