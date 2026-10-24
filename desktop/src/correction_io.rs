use crate::{platform, AppState};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};
use tauri::{Manager, State};
use vao_core::corrections::{self, Book, Exchange, Group};

#[derive(Clone)]
struct PendingImport {
    token: u64,
    revision: u64,
    groups: Vec<Group>,
}
#[derive(Default)]
pub struct Transfers {
    pending: Mutex<Option<PendingImport>>,
    serial: AtomicU64,
}

#[tauri::command]
pub fn save_correction_book(
    state: State<AppState>,
    mut book: Book,
    revision: u64,
) -> Result<Book, String> {
    book.canonicalize()?;
    state.store.lock().unwrap().save_corrections(book, revision)
}
#[tauri::command]
pub fn undo_correction_book(state: State<AppState>, revision: u64) -> Result<Book, String> {
    state.store.lock().unwrap().undo_corrections(revision)
}

#[tauri::command]
pub async fn preview_correction_import(app: tauri::AppHandle) -> Result<Value, String> {
    let selected = tauri::async_runtime::spawn_blocking(|| {
        platform::call(json!({"action":"corrections_import"}))
    })
    .await
    .map_err(|_| "文件选择中断 / File selection interrupted")??;
    let Some(path) = selected["path"].as_str() else {
        return Ok(json!({"cancelled":true}));
    };
    let mut bytes = vec![];
    std::fs::File::open(path)
        .map_err(|_| "无法读取纠错文件 / Could not read correction file")?
        .take(corrections::MAX_FILE as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取纠错文件")?;
    let incoming = corrections::parse_exchange(
        std::str::from_utf8(&bytes).map_err(|_| "请使用UTF-8 JSON / Use UTF-8 JSON")?,
    )?;
    let state = app.state::<AppState>();
    let book = state.store.lock().unwrap().corrections()?;
    let report = book.merge(&incoming.groups, &HashMap::new())?;
    let transfers = app.state::<Transfers>();
    let token = transfers.serial.fetch_add(1, Ordering::Relaxed) + 1;
    *transfers.pending.lock().unwrap() = Some(PendingImport {
        token,
        revision: book.revision,
        groups: incoming.groups,
    });
    Ok(
        json!({"token":token,"revision":book.revision,"added_groups":report.added_groups,"added_aliases":report.added_aliases,"duplicates":report.duplicates,"skipped":report.skipped,"conflicts":report.conflicts}),
    )
}
#[tauri::command]
pub fn commit_correction_import(
    state: State<AppState>,
    transfers: State<Transfers>,
    token: u64,
    choices: HashMap<String, String>,
) -> Result<Value, String> {
    let mut pending = transfers.pending.lock().unwrap();
    let selected = pending
        .as_ref()
        .filter(|p| p.token == token)
        .ok_or("导入预览已过期 / Import preview expired")?;
    let mut db = state.store.lock().unwrap();
    let book = db.corrections()?;
    if book.revision != selected.revision {
        return Err(
            "词库已变化，请重新选择文件预览 / Library changed; preview the file again".into(),
        );
    }
    let report = book.merge(&selected.groups, &choices)?;
    for conflict in &report.conflicts {
        let choice = choices
            .get(&conflict.key)
            .ok_or("请确认每项导入冲突 / Resolve each import conflict")?;
        let allowed = choice.is_empty()
            || report
                .conflicts
                .iter()
                .any(|c| c.key == conflict.key && (c.incoming == *choice || c.existing == *choice));
        if !allowed {
            return Err("导入冲突选择无效 / Invalid conflict choice".into());
        }
    }
    let saved = db.save_corrections(report.book, selected.revision)?;
    *pending = None;
    Ok(
        json!({"revision":saved.revision,"added_groups":report.added_groups,"added_aliases":report.added_aliases}),
    )
}
#[tauri::command]
pub fn cancel_correction_import(transfers: State<Transfers>, token: u64) {
    let mut p = transfers.pending.lock().unwrap();
    if p.as_ref().is_some_and(|p| p.token == token) {
        *p = None;
    }
}

#[tauri::command]
pub async fn export_corrections(
    app: tauri::AppHandle,
    selected: Option<Vec<String>>,
) -> Result<Value, String> {
    let mut book = app
        .state::<AppState>()
        .store
        .lock()
        .unwrap()
        .corrections()?;
    if let Some(selected) = selected {
        let wanted: HashSet<String> = selected.into_iter().collect();
        book.groups.retain(|g| wanted.contains(&g.correct));
    }
    let count = book.groups.len();
    let exchange = Exchange {
        format: corrections::FORMAT.into(),
        schema_version: 1,
        exported_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|e| e.to_string())?,
        groups: book.groups,
    };
    let bytes = serde_json::to_vec_pretty(&exchange).map_err(|e| e.to_string())?;
    if bytes.len() > corrections::MAX_FILE {
        return Err("导出文件超出大小上限 / Export too large".into());
    }
    let selected = tauri::async_runtime::spawn_blocking(|| {
        platform::call(json!({"action":"corrections_export"}))
    })
    .await
    .map_err(|_| "文件选择中断")??;
    let Some(path) = selected["path"].as_str() else {
        return Ok(json!({"cancelled":true}));
    };
    let path = std::path::Path::new(path);
    let parent = path.parent().ok_or("无效导出目录")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| "无法写入导出目录 / Could not write export directory")?;
    temp.write_all(&bytes)
        .map_err(|_| "导出写入失败 / Export failed")?;
    temp.as_file().sync_all().map_err(|_| "导出保存失败")?;
    temp.persist(path).map_err(|_| {
        "无法完成导出，原文件保持 / Could not finish export; original file preserved"
    })?;
    Ok(json!({"groups":count}))
}
