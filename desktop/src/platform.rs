use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
#[cfg(target_os = "macos")]
struct Bridge {
    child: std::process::Child,
    input: std::process::ChildStdin,
    output: BufReader<std::process::ChildStdout>,
}
#[cfg(target_os = "macos")]
static BRIDGE: std::sync::OnceLock<std::sync::Mutex<Option<Bridge>>> = std::sync::OnceLock::new();
pub fn call(request: Value) -> Result<Value, String> {
    #[cfg(target_os = "macos")]
    {
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
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let helper = exe.parent().unwrap().join("../Resources/vao-macos-bridge");
            let helper = if helper.exists() {
                helper
            } else {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../.build/vao-macos-bridge")
            };
            let mut command = Command::new(helper);
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
    #[cfg(not(target_os = "macos"))]
    {
        let _ = request;
        Err("This release implements the macOS adapter only".into())
    }
}
pub fn target(selection: bool) -> Result<Value, String> {
    call(json!({"action":"target","selection":selection}))
}
/// Presence lookup never reads password data or shows Keychain consent on page load.
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
    #[cfg(not(target_os = "macos"))]
    {
        let _ = slot;
        Ok(false)
    }
}
pub fn secret(slot: &str) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        match keyring::Entry::new("app.vaovao.desktop", slot)
            .map_err(|_| "凭据服务初始化失败")?
            .get_password()
        {
            Ok(s) => Ok(s),
            Err(keyring::Error::NoEntry) => Ok(String::new()),
            Err(_) => Err("无法读取Vao的凭据；请检查钥匙串授权".into()),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = slot;
        Err("Credential adapter not implemented".into())
    }
}
pub fn save_secret(slot: &str, value: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let e =
            keyring::Entry::new("app.vaovao.desktop", slot).map_err(|_| "凭据服务初始化失败")?;
        if value.is_empty() {
            match e.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(_) => Err("凭据删除失败".into()),
            }
        } else {
            e.set_password(value).map_err(|_| "凭据保存失败".into())
        }
    }
    #[cfg(not(target_os = "macos"))]
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
