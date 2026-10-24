//! Short-lived native dialogs and Windows file semantics. No input/DB locks.
use serde_json::{json, Value};
use std::{
    fs::File,
    os::windows::{ffi::OsStrExt, io::FromRawHandle},
    path::Path,
};
use windows_sys::Win32::{
    Foundation::{LocalFree, INVALID_HANDLE_VALUE},
    Security::{
        Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW, SECURITY_ATTRIBUTES,
    },
    Storage::FileSystem::*,
    UI::Controls::Dialogs::*,
};

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

pub fn private_audio(path: &Path) -> Result<File, String> {
    unsafe {
        let mut descriptor = std::ptr::null_mut();
        // Protected DACL: only the object owner. No inherited broad temp ACL.
        let sddl: Vec<_> = "D:P(A;;FA;;;OW)".encode_utf16().chain(Some(0)).collect();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err("无法保护录音文件 / Cannot protect audio storage".into());
        }
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let handle = CreateFileW(
            wide(path).as_ptr(),
            0x80000000 | 0x40000000 | 0x00010000,
            0,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_TEMPORARY | FILE_FLAG_DELETE_ON_CLOSE,
            std::ptr::null_mut(),
        );
        LocalFree(descriptor);
        if handle == INVALID_HANDLE_VALUE {
            return Err("无法创建安全录音缓冲 / Audio storage unavailable".into());
        }
        Ok(File::from_raw_handle(handle))
    }
}

pub fn replace(temporary: &Path, target: &Path) -> Result<(), &'static str> {
    let source = wide(temporary);
    let target_wide = wide(target);
    // Close the writer before replacing. ReplaceFile preserves the target ACL.
    // Never delete the destination. A concurrent create causes failure.
    let ok = unsafe {
        if target.exists() {
            ReplaceFileW(
                target_wide.as_ptr(),
                source.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null(),
            )
        } else {
            MoveFileExW(
                source.as_ptr(),
                target_wide.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        }
    };
    if ok == 0 {
        Err("无法替换词典，旧文件已保留 / Dictionary replacement failed")
    } else {
        Ok(())
    }
}

pub fn dictionary_dialog(save: bool) -> Result<Value, String> {
    file_dialog(save, false)
}
pub fn correction_dialog(save: bool) -> Result<Value, String> {
    file_dialog(save, true)
}
fn file_dialog(save: bool, corrections: bool) -> Result<Value, String> {
    static DIALOG: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _dialog = DIALOG.lock().map_err(|_| "文件窗口不可用")?;
    unsafe {
        let mut file = vec![0u16; 32768];
        if save {
            for (i, c) in (if corrections { "Vao-personal-corrections.json" } else { "Vao-dictionary.json" }).encode_utf16().enumerate() {
                file[i] = c;
            }
        }
        let filter: Vec<_> = (if corrections { "Personal corrections (*.json)\0*.json\0\0" } else { "Dictionary (*.json;*.txt)\0*.json;*.txt\0JSON (*.json)\0*.json\0Text (*.txt)\0*.txt\0\0" }).encode_utf16().collect();
        let extension: Vec<_> = "json\0".encode_utf16().collect();
        let mut dialog: OPENFILENAMEW = std::mem::zeroed();
        dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        dialog.lpstrFilter = filter.as_ptr();
        dialog.nFilterIndex = 1;
        dialog.lpstrFile = file.as_mut_ptr();
        dialog.nMaxFile = file.len() as u32;
        dialog.lpstrDefExt = extension.as_ptr();
        dialog.Flags = OFN_EXPLORER
            | OFN_NOCHANGEDIR
            | OFN_PATHMUSTEXIST
            | OFN_HIDEREADONLY
            | if save {
                OFN_OVERWRITEPROMPT
            } else {
                OFN_FILEMUSTEXIST
            };
        let accepted = if save {
            GetSaveFileNameW(&mut dialog)
        } else {
            GetOpenFileNameW(&mut dialog)
        };
        if accepted == 0 {
            if CommDlgExtendedError() == 0 {
                return Ok(json!({"cancelled":true}));
            }
            return Err("文件选择失败 / File dialog failed".into());
        }
        let length = file.iter().position(|v| *v == 0).ok_or("文件路径过长")?;
        Ok(json!({"path":String::from_utf16(&file[..length]).map_err(|_|"文件路径编码无效")?}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_exclusive_delete_on_close() {
        let path = std::env::temp_dir().join(format!("vao-private-test-{}", std::process::id()));
        let file = private_audio(&path).unwrap();
        assert!(File::open(&path).is_err());
        assert!(private_audio(&path).is_err());
        drop(file);
        assert!(!path.exists());
    }
    #[test]
    fn replacement_failure_retains_destination() {
        let target = std::env::temp_dir().join(format!("vao-replace-test-{}", std::process::id()));
        std::fs::write(&target, b"old dictionary").unwrap();
        assert!(replace(&target.with_extension("missing"), &target).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"old dictionary");
        std::fs::remove_file(target).unwrap();
    }
}
