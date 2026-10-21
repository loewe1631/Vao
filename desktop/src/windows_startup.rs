//! Current-user login startup. Reading status never registers or repairs an entry.
use serde::Serialize;
use std::{io, path::Path};
use winreg::{enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_BINARY, REG_SZ}, RegKey};

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVAL: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
pub const BACKGROUND_ARG: &str = "--autostart";

#[derive(Serialize)]
pub struct Status {
    pub available: bool,
    /// Whether this executable owns the registered command, independent of OS approval.
    pub registered: bool,
    pub system_disabled: bool,
}

struct Startup<'a> {
    run: &'a str,
    approval: &'a str,
    name: &'a str,
}

pub fn background_launch(args: &[String]) -> bool {
    args.iter().any(|arg| arg == BACKGROUND_ARG)
}

fn command(executable: &Path) -> Result<String, String> {
    let path = executable.to_str().ok_or("程序路径无法用于登录启动 / Unsupported startup path")?;
    if !executable.is_absolute() || path.contains(['\0', '"']) || !path.to_ascii_lowercase().ends_with(".exe") {
        return Err("程序路径无法用于登录启动 / Unsupported startup path".into());
    }
    let value = format!("\"{path}\" {BACKGROUND_ARG}");
    // Windows documents a 260-character limit for a Run value's command line.
    if value.encode_utf16().count() > 260 {
        return Err("程序路径过长，无法设置登录启动 / Startup path is too long".into());
    }
    Ok(value)
}

fn missing<T>(result: io::Result<T>) -> io::Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

impl Startup<'_> {
    fn read(&self, expected: &str) -> io::Result<Status> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let registered = if let Some(key) = missing(hkcu.open_subkey_with_flags(self.run, KEY_READ))? {
            match missing(key.get_raw_value(self.name))? {
                Some(raw) if raw.vtype == REG_SZ => {
                    let stored: String = key.get_value(self.name)?;
                    stored.eq_ignore_ascii_case(expected)
                }
                _ => false,
            }
        } else { false };
        let approval = if let Some(key) = missing(hkcu.open_subkey_with_flags(self.approval, KEY_READ))? {
            missing(key.get_raw_value(self.name))?
        } else { None };
        // Observe Task Manager's disabled states; never rewrite its approval record.
        let system_disabled = registered && approval.is_some_and(|raw| {
            raw.vtype == REG_BINARY && raw.bytes.len() >= 4
                && matches!(u32::from_le_bytes(raw.bytes[..4].try_into().unwrap()), 3 | 7)
        });
        Ok(Status { available: true, registered, system_disabled })
    }

    fn set(&self, expected: &str, enabled: bool) -> io::Result<Status> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if enabled {
            let (key, _) = hkcu.create_subkey(self.run)?;
            key.set_value(self.name, &expected)?;
        } else if let Some(key) = missing(hkcu.open_subkey_with_flags(self.run, KEY_READ | KEY_SET_VALUE))? {
            // An older copy must not disable a newer copy registered at another path.
            if self.read(expected)?.registered { missing(key.delete_value(self.name))?; }
        }
        self.read(expected)
    }
}

fn current() -> Startup<'static> {
    Startup { run: RUN, approval: APPROVAL, name: env!("VAO_BUNDLE_ID") }
}

pub fn status() -> Result<Status, String> {
    let executable = std::env::current_exe().map_err(|_| "无法读取程序路径 / Could not read executable path")?;
    current().read(&command(&executable)?).map_err(|_| "无法读取登录启动设置 / Could not read login startup settings".into())
}

pub fn set_enabled(enabled: bool) -> Result<Status, String> {
    let executable = std::env::current_exe().map_err(|_| "无法读取程序路径 / Could not read executable path")?;
    current().set(&command(&executable)?, enabled).map_err(|_| "无法更改登录启动设置 / Could not change login startup settings".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use winreg::RegValue;

    struct Fixture(String);
    impl Fixture {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            Self(format!(r"Software\Vao\StartupTests\{}-{nonce}", std::process::id()))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) { let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.0); }
    }

    #[test]
    fn startup_quotes_unicode_paths_and_rejects_invalid_run_commands() {
        assert_eq!(command(Path::new(r"C:\Program Files\语音\vao.exe")).unwrap(), "\"C:\\Program Files\\语音\\vao.exe\" --autostart");
        for path in ["relative.exe", "C:\\vao.exe\" -bad", "C:\\vao.cmd", "C:\\bad\0.exe"] {
            assert!(command(Path::new(path)).is_err());
        }
        assert!(command(Path::new(&format!("C:\\{}.exe", "语".repeat(260)))).is_err());
        assert!(background_launch(&["vao.exe".into(), "--autostart".into()]));
        assert!(!background_launch(&["vao.exe".into(), "--not-autostart".into()]));
    }

    #[test]
    fn isolated_startup_registration_preserves_other_paths_and_system_disable() {
        let fixture = Fixture::new();
        let run = format!(r"{}\Run", fixture.0);
        let approval = format!(r"{}\Approval", fixture.0);
        let startup = Startup { run: &run, approval: &approval, name: "test-preview" };
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let expected = command(Path::new(r"C:\Synthetic App\vao.exe")).unwrap();
        assert!(!startup.read(&expected).unwrap().registered);
        assert!(hkcu.open_subkey(&fixture.0).is_err(), "status must not create registry keys");
        assert!(startup.set(&expected, true).unwrap().registered);
        let key = hkcu.open_subkey_with_flags(&run, KEY_READ | KEY_SET_VALUE).unwrap();
        key.set_value("other-app", &"untouched").unwrap();
        let (approved, _) = hkcu.create_subkey(&approval).unwrap();
        let disabled = RegValue { vtype: REG_BINARY, bytes: vec![3,0,0,0,1,2,3,4,5,6,7,8].into() };
        approved.set_raw_value(startup.name, &disabled).unwrap();
        let status = startup.set(&expected, true).unwrap();
        assert!(status.registered && status.system_disabled);
        assert_eq!(approved.get_raw_value(startup.name).unwrap(), disabled);
        let newer = command(Path::new(r"C:\Newer App\vao.exe")).unwrap();
        key.set_value(startup.name, &newer).unwrap();
        assert!(!startup.set(&expected, false).unwrap().registered);
        assert_eq!(key.get_value::<String, _>(startup.name).unwrap(), newer);
        assert!(!startup.set(&newer, false).unwrap().registered);
        assert!(key.get_raw_value(startup.name).is_err());
        assert_eq!(key.get_value::<String, _>("other-app").unwrap(), "untouched");
        assert_eq!(approved.get_raw_value(startup.name).unwrap(), disabled);
        key.set_raw_value(startup.name, &RegValue { vtype: REG_BINARY, bytes: vec![0,0].into() }).unwrap();
        assert!(!startup.read(&expected).unwrap().registered);
    }
}
