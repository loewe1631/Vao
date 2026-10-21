use crate::{platform, store::Settings, AppState};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    io::{Read, Write},
    path::Path,
};
use tauri::Manager;

pub fn parse(text: &str) -> Result<Vec<String>, String> {
    let text = text.trim_start_matches('\u{feff}').trim();
    let words: Vec<String> = if text.starts_with('[') || text.starts_with('{') {
        let value: Value = serde_json::from_str(text)
            .map_err(|_| "词典 JSON 格式无效 / Invalid dictionary JSON")?;
        let list = value
            .as_array()
            .or_else(|| value.get("words").and_then(Value::as_array))
            .or_else(|| value.get("dictionary").and_then(Value::as_array))
            .ok_or("词典须为字符串数组或包含 words 的对象 / Expected a word list")?;
        list.iter()
            .map(|v| {
                v.as_str().map(str::to_owned).ok_or_else(|| {
                    "词典包含非文字条目 / Dictionary contains non-text entries".to_string()
                })
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        text.lines().map(str::to_owned).collect()
    };
    merge(&[], words)
}
fn merge(existing: &[String], incoming: Vec<String>) -> Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    let mut out = vec![];
    for word in existing.iter().cloned().chain(incoming) {
        let word = word.trim();
        if word.is_empty() {
            continue;
        }
        if word.len() > 160 || word.contains(['\n', '\r', '\0']) {
            return Err("词条过长或包含换行 / Invalid dictionary entry".into());
        }
        if seen.insert(word.to_owned()) {
            out.push(word.to_owned());
        }
    }
    if out.len() > 100 {
        return Err("合并后超过 100 个词条，请精简后导入；已有词典未更改 / More than 100 merged entries; dictionary unchanged".into());
    }
    Ok(out)
}
#[tauri::command]
pub async fn import_dictionary(app: tauri::AppHandle) -> Result<Value, String> {
    let selection = tauri::async_runtime::spawn_blocking(|| {
        platform::call(json!({"action":"dictionary_import"}))
    })
    .await
    .map_err(|_| "文件选择失败")??;
    let Some(path) = selection["path"].as_str() else {
        return Ok(json!({"cancelled":true}));
    };
    let path = Path::new(path);
    if std::fs::metadata(path)
        .map_err(|_| "无法读取词典文件")?
        .len()
        > 1024 * 1024
    {
        return Err("词典文件超过 1 MiB / Dictionary file too large".into());
    }
    let mut bytes = vec![];
    std::fs::File::open(path)
        .map_err(|_| "无法读取词典文件")?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取词典文件")?;
    if bytes.len() > 1024 * 1024 {
        return Err("词典文件过大".into());
    }
    let incoming = parse(
        std::str::from_utf8(&bytes)
            .map_err(|_| "请使用 UTF-8 文本或 JSON 词典 / Use a UTF-8 dictionary")?,
    )?;
    let state = app.state::<AppState>();
    let db = state.store.lock().unwrap();
    let mut settings = db.get::<Settings>("settings");
    let before = settings.dictionary.len();
    let dictionary = merge(&settings.dictionary, incoming)?;
    let added = dictionary.len().saturating_sub(before);
    settings.dictionary = dictionary;
    db.set("settings", &settings)?;
    Ok(json!({"added":added,"total":settings.dictionary.len()}))
}
#[tauri::command]
pub async fn export_dictionary(app: tauri::AppHandle) -> Result<Value, String> {
    let words = app
        .state::<AppState>()
        .store
        .lock()
        .unwrap()
        .get::<Settings>("settings")
        .dictionary;
    let selection = tauri::async_runtime::spawn_blocking(|| {
        platform::call(json!({"action":"dictionary_export"}))
    })
    .await
    .map_err(|_| "文件选择失败")??;
    let Some(path) = selection["path"].as_str() else {
        return Ok(json!({"cancelled":true}));
    };
    let text = if Path::new(path)
        .extension()
        .and_then(|v| v.to_str())
        .is_some_and(|v| v.eq_ignore_ascii_case("txt"))
    {
        words.join("\n")
    } else {
        serde_json::to_string_pretty(&json!({"format":"vao-dictionary","version":1,"words":words}))
            .map_err(|_| "词典编码失败")?
    };
    atomic_write(Path::new(path), text.as_bytes())?;
    Ok(json!({"count":words.len()}))
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| "无法准备词典文件")?;
    let suffix = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let temporary = path.with_file_name(format!(".vao-dictionary-{suffix}.tmp"));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(|_| "无法创建词典文件")?;
        file.write_all(bytes).map_err(|_| "无法写入词典")?;
        file.sync_all().map_err(|_| "无法保存词典")?;
        drop(file);
        #[cfg(windows)]
        { crate::windows_files::replace(&temporary, path) }
        #[cfg(not(windows))]
        { std::fs::rename(&temporary, path).map_err(|_| "无法保存词典 / Could not save dictionary") }
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result.map_err(str::to_owned)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exported_json_and_text_roundtrip() {
        let words = vec!["Vao".to_owned(), "千问".to_owned()];
        let dir = std::env::temp_dir().join(format!("vao-dictionary-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("terms.json");
        atomic_write(&path, b"old").unwrap();
        atomic_write(
            &path,
            serde_json::to_string(&json!({"format":"vao-dictionary","version":1,"words":words}))
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(
            parse(&std::fs::read_to_string(&path).unwrap()).unwrap(),
            words
        );
        assert_eq!(parse(&words.join("\n")).unwrap(), words);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn formats_merge_and_invalid_import_are_atomic() {
        assert_eq!(
            parse("\u{feff}Vao\n千问\nVao\n\n").unwrap(),
            vec!["Vao", "千问"]
        );
        assert_eq!(
            parse(r#"{"format":"vao-dictionary","version":1,"words":["Vao","千问"]}"#).unwrap(),
            vec!["Vao", "千问"]
        );
        assert_eq!(parse(r#"["Vao"]"#).unwrap(), vec!["Vao"]);
        let old = vec!["Vao".to_string()];
        assert_eq!(
            merge(&old, vec!["Vao".into(), "Qwen".into()]).unwrap(),
            vec!["Vao", "Qwen"]
        );
        assert!(parse(r#"["valid",1]"#).is_err());
        assert!(parse(&"a".repeat(161)).is_err());
        assert!(merge(&old, (0..100).map(|i| i.to_string()).collect()).is_err());
        assert_eq!(old, vec!["Vao"]);
    }
}
