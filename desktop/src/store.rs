use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use vao_core::{Layout, Profile, Scene};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ConnectionConfig {
    pub base: String,
    pub model: String,
    pub protocol: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub trial_enabled: bool,
    pub trial_consent_version: String,
    pub onboarding_complete: bool,
    pub shortcuts: crate::hotkeys::Shortcuts,
    pub asr: ConnectionConfig,
    pub llm: ConnectionConfig,
    pub backup: ConnectionConfig,
    pub language: String,
    pub translation_language: String,
    pub auto_input: bool,
    pub auto_stop: bool,
    pub silence_seconds: u8,
    pub complete_book_titles: bool,
    pub dictionary: Vec<String>,
    pub ui_language: String,
    pub theme: String,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            trial_enabled: true,
            trial_consent_version: String::new(),
            onboarding_complete: false,
            shortcuts: crate::hotkeys::Shortcuts::default(),
            asr: ConnectionConfig {
                protocol: "openai_audio".into(),
                ..Default::default()
            },
            llm: ConnectionConfig {
                protocol: "openai_chat".into(),
                ..Default::default()
            },
            backup: ConnectionConfig {
                protocol: "openai_chat".into(),
                ..Default::default()
            },
            language: "auto".into(),
            translation_language: "English".into(),
            auto_input: true,
            auto_stop: false,
            silence_seconds: 5,
            complete_book_titles: true,
            dictionary: vec![],
            ui_language: "zh".into(),
            theme: "system".into(),
            extra: Default::default(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefinementDiagnostic {
    pub policy_version: String,
    pub route: String,
    pub model: String,
    pub protocol: String,
    pub semantic_effective: bool,
    pub completion_requested: bool,
    pub completion_effective: bool,
    pub generation_attempts: usize,
    pub repair_attempted: bool,
    pub used_original: bool,
    pub first_issues: Vec<String>,
    pub final_issues: Vec<String>,
    #[serde(default)]
    pub completion_review: String,
    #[serde(default)]
    pub completion_applied: usize,
    #[serde(default)]
    pub completion_issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultRecord {
    pub id: i64,
    pub created: i64,
    pub scene: Scene,
    pub layout: Layout,
    pub revision: u64,
    pub original: String,
    pub text: String,
    pub risks: Vec<String>,
    pub app: String,
    pub delivery: String,
    pub feedback: bool,
    // Local history only: no credentials, endpoints or extra model text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refinement: Vec<RefinementDiagnostic>,
}
/// Retire old opt-ins on read/save without touching credentials or other settings.
fn retire_input_enhancements(value: &mut serde_json::Value) {
    if let Some(settings) = value.as_object_mut() {
        for key in ["live_preview", "input_context", "expanded_context"] { settings.remove(key); }
        if let Some(asr) = settings.get_mut("asr") {
            if asr["protocol"] == "openai_realtime" { asr["protocol"] = "openai_audio".into(); }
        }
    }
}
pub struct Store {
    conn: Connection,
}
impl Store {
    pub fn open_read_only(path: &Path) -> Result<Self, String> {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| "Could not read settings database")?;
        conn.execute_batch("PRAGMA query_only=ON").map_err(|e| e.to_string())?;
        Ok(Self { conn })
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS results (id INTEGER PRIMARY KEY AUTOINCREMENT,created INTEGER NOT NULL,data TEXT NOT NULL); DELETE FROM results WHERE created < unixepoch()-604800;").map_err(|e|e.to_string())?;
        Ok(Self { conn })
    }
    pub fn get<T: serde::de::DeserializeOwned + Default>(&self, key: &str) -> T {
        self.conn
            .query_row("SELECT value FROM kv WHERE key=?", [key], |r| {
                r.get::<_, String>(0)
            })
            .ok()
            .and_then(|v| {
                let mut value: serde_json::Value = serde_json::from_str(&v).ok()?;
                if key == "settings" { retire_input_enhancements(&mut value); }
                serde_json::from_value(value).ok()
            })
            .unwrap_or_default()
    }
    pub fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), String> {
        let v = serde_json::to_string(value).map_err(|e| e.to_string())?;
        self.conn.execute("INSERT INTO kv(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,v]).map_err(|e|e.to_string())?;
        Ok(())
    }
    pub fn save_settings_preserving(&self, settings: &Settings) -> Result<(), String> {
        let mut value: serde_json::Value = self.get("settings");
        let current = serde_json::to_value(settings).map_err(|e| e.to_string())?;
        if let Some(old) = value.as_object_mut() {
            old.extend(current.as_object().ok_or("Invalid settings")?.clone());
        } else {
            value = current;
        }
        retire_input_enhancements(&mut value);
        self.set("settings", &value)
    }
    pub fn add(&self, mut r: ResultRecord) -> Result<ResultRecord, String> {
        self.conn
            .execute(
                "INSERT INTO results(created,data) VALUES(?,?)",
                params![
                    r.created,
                    serde_json::to_string(&r).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        r.id = self.conn.last_insert_rowid();
        self.update(&r)?;
        self.conn.execute("DELETE FROM results WHERE id NOT IN (SELECT id FROM results ORDER BY id DESC LIMIT 200) OR created < unixepoch()-604800",[]).map_err(|e|e.to_string())?;
        Ok(r)
    }
    pub fn update(&self, r: &ResultRecord) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE results SET data=? WHERE id=?",
                params![serde_json::to_string(r).map_err(|e| e.to_string())?, r.id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn history(&self) -> Result<Vec<ResultRecord>, String> {
        self.conn
            .execute("DELETE FROM results WHERE created < unixepoch()-604800", [])
            .map_err(|e| e.to_string())?;
        let mut q = self
            .conn
            .prepare("SELECT data FROM results ORDER BY id DESC LIMIT 200")
            .map_err(|e| e.to_string())?;
        let rows = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let mut out = vec![];
        for row in rows {
            if let Ok(r) = serde_json::from_str(&row.map_err(|e| e.to_string())?) {
                out.push(r);
            }
        }
        Ok(out)
    }
    pub fn result(&self, id: i64) -> Result<ResultRecord, String> {
        let s = self
            .conn
            .query_row("SELECT data FROM results WHERE id=? AND created >= unixepoch()-604800", [id], |r| {
                r.get::<_, String>(0)
            })
            .map_err(|_| "结果已过期 / Result expired")?;
        serde_json::from_str(&s).map_err(|e| e.to_string())
    }
    pub fn feedback(&mut self, id: i64, layout: Layout, remember: bool) -> Result<bool, String> {
        let mut r = self.result(id)?;
        if r.risks.iter().any(|risk| risk == "unrefined") {
            return Err("整理未完成的结果不能用于学习 / Unrefined results cannot teach preferences".into());
        }
        if r.feedback {
            return Err("此结果已记录反馈 / Feedback already recorded".into());
        }
        let mut p: Profile = self.get("profile");
        let applied = p.feedback(id, r.scene, layout, remember)?;
        r.feedback = remember;
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO kv(key,value) VALUES('profile',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[serde_json::to_string(&p).unwrap()]).map_err(|e|e.to_string())?;
        tx.execute(
            "UPDATE results SET data=? WHERE id=?",
            params![serde_json::to_string(&r).unwrap(), id],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(applied)
    }
    pub fn clear_history(&self) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM results", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unfinished_and_expired_results_cannot_be_used_as_learning_evidence() {
        let mut db = Store::open(std::path::Path::new(":memory:")).unwrap();
        let record = db.add(ResultRecord {
            id: 0, created: crate::now(), scene: Scene::Tasks, layout: Layout::Paragraph,
            revision: 0, original: "synthetic".into(), text: "synthetic".into(),
            risks: vec!["unrefined".into()], app: "Synthetic".into(), delivery: "preview".into(), feedback: false, refinement: vec![],
        }).unwrap();
        assert!(db.feedback(record.id, Layout::Numbered, true).is_err());
        assert!(db.get::<Profile>("profile").votes.is_empty());
        db.conn.execute("UPDATE results SET created=unixepoch()-604801 WHERE id=?", [record.id]).unwrap();
        assert!(db.result(record.id).is_err());
        assert!(db.feedback(record.id, Layout::Numbered, true).is_err());
    }

    #[test]
    fn legacy_history_reads_without_diagnostics_and_new_diagnostics_round_trip() {
        let old = serde_json::json!({"id":0,"created":crate::now(),"scene":"chat","layout":"paragraph","revision":0,
            "original":"original","text":"result","risks":[],"app":"Synthetic","delivery":"preview","feedback":false});
        let mut record: ResultRecord = serde_json::from_value(old.clone()).unwrap();
        assert!(record.refinement.is_empty());
        assert_eq!(serde_json::to_value(&record).unwrap(), old);
        record.refinement.push(RefinementDiagnostic {policy_version:"test".into(),route:"custom_chat".into(),model:"synthetic".into(),protocol:"openai_chat".into(),semantic_effective:true,
            completion_requested:true,completion_effective:true,generation_attempts:2,repair_attempted:true,used_original:true,
            first_issues:vec!["invalid_annotation_output".into()],final_issues:vec!["invalid_annotation_output".into()],
            completion_review:"budget_exhausted".into(),completion_applied:0,completion_issues:vec!["completion_not_reviewed".into()]});
        let db = Store::open(Path::new(":memory:")).unwrap();
        let saved = db.add(record).unwrap();
        let loaded = db.result(saved.id).unwrap();
        assert!(loaded.refinement[0].used_original);
        assert_eq!(loaded.refinement[0].final_issues, ["invalid_annotation_output"]);
    }
}
