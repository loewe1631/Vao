use crate::{store::ResultRecord, AppState, Runtime};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{Emitter, Manager};
use vao_core::{
    corrections::{self, Matcher, Pair, Suggestion},
    Phase,
};

#[derive(Debug, Clone, Serialize)]
pub struct PromptItem {
    pub id: usize,
    pub found: String,
    pub correct: String,
    pub before: String,
    pub after: String,
    pub overlaps: Vec<usize>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Pending {
    pub session_id: u64,
    pub result_id: i64,
    pub book_revision: u64,
    pub result_revision: u64,
    pub auto_input: bool,
    pub items: Vec<PromptItem>,
    #[serde(skip)]
    pub base_text: String,
    #[serde(skip)]
    pub matches: Vec<Suggestion>,
}
pub fn prepare(
    runtime: &mut Runtime,
    record: &ResultRecord,
    eligible: bool,
) -> Result<bool, String> {
    let matches = if eligible {
        Matcher::new(&runtime.correction_book)?.suggest(&record.text, &record.original)?
    } else {
        vec![]
    };
    if matches.is_empty() {
        runtime.session.advance(runtime.session.id, Phase::Ready)?;
        return Ok(false);
    }
    let items = matches
        .iter()
        .map(|s| PromptItem {
            id: s.id,
            found: s.found.clone(),
            correct: s.correct.clone(),
            overlaps: matches
                .iter()
                .filter(|other| other.id != s.id && other.start < s.end && other.end > s.start)
                .map(|other| other.id)
                .collect(),
            before: record.text[..s.start]
                .chars()
                .rev()
                .take(24)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect(),
            after: record.text[s.end..].chars().take(24).collect(),
        })
        .collect();
    runtime.pending_correction = Some(Pending {
        session_id: runtime.session.id,
        result_id: record.id,
        book_revision: runtime.correction_book.revision,
        result_revision: 0,
        auto_input: runtime.settings.auto_input
            && (cfg!(target_os = "windows") || runtime.target["pid"].is_number()),
        items,
        base_text: record.text.clone(),
        matches,
    });
    runtime
        .session
        .advance(runtime.session.id, Phase::AwaitingCorrection)?;
    runtime.detail = "请确认文字 / Review correction".into();
    Ok(true)
}

// Commit before delivery. A repeated confirmation cannot create another delivery claim.
fn settle(
    runtime: &mut Runtime,
    db: &mut crate::store::Store,
    session_id: u64,
    selected: &[usize],
) -> Result<Option<(ResultRecord, Value, bool)>, String> {
    if runtime.correction_resolved == Some(session_id) {
        return Ok(None);
    }
    if runtime.session.id != session_id || runtime.session.phase != Phase::AwaitingCorrection {
        return Err("纠错提示已过期 / Correction prompt expired".into());
    }
    let pending = runtime
        .pending_correction
        .clone()
        .ok_or("无待确认纠错 / No correction pending")?;
    let mut record = runtime
        .result
        .clone()
        .filter(|r| r.id == pending.result_id && r.text == pending.base_text)
        .ok_or("结果已变化 / Result changed")?;
    let corrected = corrections::apply(&pending.base_text, &pending.matches, selected)?;
    let revision = db
        .correction_edit(record.id)?
        .map(|e| e.revision)
        .unwrap_or(0);
    if revision != pending.result_revision {
        return Err(
            "结果已编辑，请取消提示并查看历史 / Result was edited; cancel and review History"
                .into(),
        );
    }
    if corrected != pending.base_text {
        record = db.save_correction_edit(
            &record,
            pending.result_revision,
            &pending.base_text,
            &corrected,
            &[],
        )?;
        runtime.result = Some(record.clone());
    }
    runtime.session.advance(session_id, Phase::Ready)?;
    runtime.pending_correction = None;
    runtime.correction_resolved = Some(session_id);
    runtime.detail = "已确认 / Confirmed".into();
    Ok(Some((
        record,
        runtime.target.clone(),
        runtime.settings.auto_input,
    )))
}
#[tauri::command]
pub fn resolve_correction_prompt(
    app: tauri::AppHandle,
    session_id: u64,
    selected: Vec<usize>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let resolved = {
        let mut runtime = state.runtime.lock().unwrap();
        let mut db = state.store.lock().unwrap();
        settle(&mut runtime, &mut db, session_id, &selected)?
    };
    let Some((record, target, auto_input)) = resolved else {
        return Ok(());
    };
    crate::show_bar(&app, false);
    crate::escape_key(&app, false);
    crate::route_result(&app, session_id, &target, &record, auto_input);
    let _ = app.emit("result-ready", record.id);
    Ok(())
}

#[tauri::command]
pub fn save_result_correction(
    app: tauri::AppHandle,
    id: i64,
    session_id: u64,
    revision: u64,
    expected_text: String,
    text: String,
    pairs: Vec<Pair>,
) -> Result<Value, String> {
    let state = app.state::<AppState>();
    let mut runtime = state.runtime.lock().unwrap();
    let mut db = state.store.lock().unwrap();
    let record = if id > 0 {
        db.result(id)?
    } else {
        if runtime.session.id != session_id {
            return Err("临时结果已过期 / Temporary result expired".into());
        }
        runtime
            .result
            .clone()
            .filter(|r| r.id == 0 && r.text == expected_text)
            .ok_or("临时结果已变化 / Temporary result changed")?
    };
    for pair in &pairs {
        if !expected_text.contains(&pair.wrong) || !text.contains(&pair.correct) {
            return Err(
                "请只记住本次文字中确认的纠正 / Remember only confirmed changes in this result"
                    .into(),
            );
        }
    }
    let saved = db.save_correction_edit(&record, revision, &expected_text, &text, &pairs)?;
    let view = db.correction_view(&saved)?;
    let same = runtime.result.as_ref().is_some_and(|r| r.id == id)
        && (id > 0 || runtime.session.id == session_id);
    let cancel_pending = same && runtime.session.phase == Phase::AwaitingCorrection;
    if same {
        runtime.result = Some(saved);
    }
    if cancel_pending {
        runtime.session.cancel();
        runtime.pending_correction = None;
    }
    drop(db);
    drop(runtime);
    if cancel_pending {
        crate::show_bar(&app, false);
        crate::escape_key(&app, false);
    }
    let _ = app.emit("result-ready", json!({"edited":id}));
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    fn fixture() -> (Runtime, Store, ResultRecord) {
        let db = Store::open(std::path::Path::new(":memory:")).unwrap();
        let record:ResultRecord=serde_json::from_value(json!({"id":0,"created":crate::now(),"scene":"chat","layout":"paragraph","revision":0,"original":"Tablets 和 Tablets","text":"Tablets 和 Tablets","risks":[],"app":"Synthetic","delivery":"preview","feedback":false})).unwrap();
        let record = db.add(record).unwrap();
        let mut r = Runtime::default();
        r.correction_book
            .remember(&[Pair {
                wrong: "Tablets".into(),
                correct: "typeless".into(),
            }])
            .unwrap();
        let id = r.session.begin().unwrap();
        r.session.advance(id, Phase::Listening).unwrap();
        r.session.advance(id, Phase::Processing).unwrap();
        r.result = Some(record.clone());
        // A synthetic PID is only data in these unit tests; no platform call occurs.
        r.target = json!({"pid":123,"synthetic":true});
        (r, db, record)
    }
    #[test]
    fn waiting_is_static_frozen_and_confirmation_commits_only_selected_occurrences() {
        let (mut r, mut db, record) = fixture();
        assert!(prepare(&mut r, &record, true).unwrap());
        let id = r.session.id;
        assert_eq!(r.session.phase, Phase::AwaitingCorrection);
        assert!(r.session.begin().is_err());
        assert_eq!(db.effective_text(&record).unwrap(), record.text);
        r.correction_book.groups.clear(); // The in-flight candidate uses its frozen snapshot.
        let (_, target, _) = settle(&mut r, &mut db, id, &[1]).unwrap().unwrap();
        assert_eq!(target["synthetic"], true);
        assert_eq!(db.effective_text(&record).unwrap(), "Tablets 和 typeless");
        assert_eq!(db.result(record.id).unwrap().text, "Tablets 和 Tablets");
        assert!(settle(&mut r, &mut db, id, &[1]).unwrap().is_none());
        assert_eq!(r.session.phase, Phase::Ready);
    }
    #[test]
    fn stale_ids_edits_and_cancel_do_not_reach_ready() {
        let (mut r, mut db, record) = fixture();
        prepare(&mut r, &record, true).unwrap();
        let id = r.session.id;
        assert!(settle(&mut r, &mut db, id + 1, &[0]).is_err());
        assert!(settle(&mut r, &mut db, id, &[999]).is_err());
        assert_eq!(r.session.phase, Phase::AwaitingCorrection);
        db.save_correction_edit(&record, 0, &record.text, "a manual edit", &[])
            .unwrap();
        assert!(settle(&mut r, &mut db, id, &[0]).is_err());
        r.session.cancel();
        r.pending_correction = None;
        assert!(settle(&mut r, &mut db, id, &[]).is_err());
        assert_eq!(r.session.phase, Phase::Cancelled);
    }
    #[test]
    fn keep_and_ineligible_paths_preserve_the_original() {
        let (mut r, mut db, record) = fixture();
        prepare(&mut r, &record, true).unwrap();
        let id = r.session.id;
        assert!(settle(&mut r, &mut db, id, &[]).unwrap().is_some());
        assert_eq!(db.effective_text(&record).unwrap(), record.text);
        assert!(db.correction_edit(record.id).unwrap().is_none());
        let (mut r, _, record) = fixture();
        assert!(!prepare(&mut r, &record, false).unwrap());
        assert_eq!(r.session.phase, Phase::Ready);
        assert!(r.pending_correction.is_none());
    }
    #[test]
    fn failed_transaction_keeps_the_prompt_and_original_for_retry() {
        let (mut r, mut db, record) = fixture();
        prepare(&mut r, &record, true).unwrap();
        let id = r.session.id;
        db.conn.execute_batch("CREATE TRIGGER fail_edit BEFORE INSERT ON correction_edits BEGIN SELECT RAISE(ABORT,'synthetic disk failure'); END;").unwrap();
        assert!(settle(&mut r, &mut db, id, &[0]).is_err());
        assert_eq!(r.session.phase, Phase::AwaitingCorrection);
        assert!(r.pending_correction.is_some());
        assert_eq!(db.effective_text(&record).unwrap(), record.text);
        db.conn.execute_batch("DROP TRIGGER fail_edit").unwrap();
        assert!(settle(&mut r, &mut db, id, &[0]).unwrap().is_some());
    }
}
