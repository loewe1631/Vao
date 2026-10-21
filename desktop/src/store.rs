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
    pub live_preview: bool,
    pub input_context: bool,
    pub expanded_context: bool,
    pub dictionary: Vec<String>,
    pub ui_language: String,
    pub theme: String,
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
            live_preview: false,
            input_context: false,
            expanded_context: false,
            dictionary: vec![],
            ui_language: "zh".into(),
            theme: "system".into(),
        }
    }
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
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default()
    }
    pub fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), String> {
        let v = serde_json::to_string(value).map_err(|e| e.to_string())?;
        self.conn.execute("INSERT INTO kv(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,v]).map_err(|e|e.to_string())?;
        Ok(())
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
            risks: vec!["unrefined".into()], app: "Synthetic".into(), delivery: "preview".into(), feedback: false,
        }).unwrap();
        assert!(db.feedback(record.id, Layout::Numbered, true).is_err());
        assert!(db.get::<Profile>("profile").votes.is_empty());
        db.conn.execute("UPDATE results SET created=unixepoch()-604801 WHERE id=?", [record.id]).unwrap();
        assert!(db.result(record.id).is_err());
        assert!(db.feedback(record.id, Layout::Numbered, true).is_err());
    }
}
