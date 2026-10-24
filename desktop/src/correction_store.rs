//! Additive local persistence. The original result JSON and model transcript are immutable.
use crate::store::{ResultRecord, Store};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use vao_core::corrections::{Book, Pair};

pub fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS correction_edits(result_id INTEGER PRIMARY KEY,revision INTEGER NOT NULL,text TEXT NOT NULL,updated INTEGER NOT NULL);
        CREATE TRIGGER IF NOT EXISTS correction_history_cleanup AFTER DELETE ON results BEGIN DELETE FROM correction_edits WHERE result_id=OLD.id; END;
        DELETE FROM correction_edits WHERE result_id NOT IN (SELECT id FROM results);")
        .map_err(|e| e.to_string())
}
fn read_book(conn: &Connection) -> Result<Book, String> {
    let text: Option<String> = conn
        .query_row(
            "SELECT value FROM kv WHERE key='personal_corrections'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let book: Book = match text {
        None => Book::default(),
        Some(t) => serde_json::from_str(&t).map_err(|_| {
            "个人纠错数据无法读取，请先导出备份再检查 / Invalid local corrections data"
        })?,
    };
    book.validate()?;
    Ok(book)
}
fn write_book(conn: &Connection, book: &Book) -> Result<(), String> {
    conn.execute("INSERT INTO kv(key,value) VALUES('personal_corrections',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[serde_json::to_string(book).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
    Ok(())
}
fn stage_book(conn: &Connection, mut next: Book, expected: u64) -> Result<Book, String> {
    let old = read_book(conn)?;
    if old.revision != expected {
        return Err("词库已变化，请刷新后重试 / Correction library changed; refresh first".into());
    }
    next.validate()?;
    // Identical saves are idempotent and do not overwrite the previous undo point.
    if next.enabled == old.enabled && next.groups == old.groups {
        return Ok(old);
    }
    next.revision = old
        .revision
        .checked_add(1)
        .ok_or("Correction revision exhausted")?;
    conn.execute("INSERT INTO kv(key,value) VALUES('personal_corrections_undo',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[serde_json::to_string(&old).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
    write_book(conn, &next)?;
    Ok(next)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edit {
    pub revision: u64,
    pub text: String,
}
impl Store {
    pub fn corrections(&self) -> Result<Book, String> {
        read_book(&self.conn)
    }
    pub fn save_corrections(&mut self, book: Book, expected: u64) -> Result<Book, String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        let saved = stage_book(&tx, book, expected)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(saved)
    }
    pub fn undo_corrections(&mut self, expected: u64) -> Result<Book, String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        let old = read_book(&tx)?;
        if old.revision != expected {
            return Err("词库已变化 / Library changed".into());
        }
        let text: Option<String> = tx
            .query_row(
                "SELECT value FROM kv WHERE key='personal_corrections_undo'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let mut restored: Book =
            serde_json::from_str(&text.ok_or("没有可撤销的词库变更 / Nothing to undo")?)
                .map_err(|e| e.to_string())?;
        restored.validate()?;
        restored.revision = old.revision.checked_add(1).ok_or("Revision exhausted")?;
        write_book(&tx, &restored)?;
        tx.execute("DELETE FROM kv WHERE key='personal_corrections_undo'", [])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(restored)
    }
    pub fn correction_edit(&self, id: i64) -> Result<Option<Edit>, String> {
        let exists:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='correction_edits')",[],|r|r.get(0)).map_err(|e|e.to_string())?;
        if !exists || id <= 0 {
            return Ok(None);
        }
        self.conn
            .query_row(
                "SELECT revision,text FROM correction_edits WHERE result_id=?",
                [id],
                |r| {
                    Ok(Edit {
                        revision: r.get(0)?,
                        text: r.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(|e| e.to_string())
    }
    pub fn effective_text(&self, r: &ResultRecord) -> Result<String, String> {
        Ok(self
            .correction_edit(r.id)?
            .map(|e| e.text)
            .unwrap_or_else(|| r.text.clone()))
    }
    pub fn correction_view(&self, r: &ResultRecord) -> Result<Value, String> {
        let mut view = serde_json::to_value(r).map_err(|e| e.to_string())?;
        let edit = self.correction_edit(r.id)?;
        view["edit_revision"] = json!(edit.as_ref().map(|e| e.revision).unwrap_or(0));
        view["corrected_text"] = edit.as_ref().map(|e| json!(e.text)).unwrap_or(Value::Null);
        view["effective_text"] = edit.map(|e| json!(e.text)).unwrap_or_else(|| json!(r.text));
        Ok(view)
    }
    pub fn save_correction_edit(
        &mut self,
        r: &ResultRecord,
        expected_revision: u64,
        expected_text: &str,
        corrected: &str,
        pairs: &[Pair],
    ) -> Result<ResultRecord, String> {
        if corrected.trim().is_empty() || corrected.len() > 1024 * 1024 {
            return Err("纠正版不能为空或过长 / Empty or oversized corrected text".into());
        }
        if r.id > 0 {
            self.result(r.id)?;
        }
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        let current: Option<Edit> = if r.id > 0 {
            tx.query_row(
                "SELECT revision,text FROM correction_edits WHERE result_id=?",
                [r.id],
                |row| {
                    Ok(Edit {
                        revision: row.get(0)?,
                        text: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(|e| e.to_string())?
        } else {
            None
        };
        let revision = current.as_ref().map(|e| e.revision).unwrap_or(0);
        let actual = current.as_ref().map(|e| e.text.as_str()).unwrap_or(&r.text);
        if revision != expected_revision || actual != expected_text {
            return Err(
                "结果已变化，请重新打开纠正界面 / Result changed; reopen the editor".into(),
            );
        }
        let mut book = read_book(&tx)?;
        let previous_revision = book.revision;
        book.remember(pairs)?;
        if !pairs.is_empty() {
            stage_book(&tx, book, previous_revision)?;
        }
        let mut result = r.clone();
        if r.id == 0 {
            tx.execute(
                "INSERT INTO results(created,data) VALUES(?,?)",
                params![
                    r.created,
                    serde_json::to_string(r).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
            result.id = tx.last_insert_rowid();
            tx.execute(
                "UPDATE results SET data=? WHERE id=?",
                params![
                    serde_json::to_string(&result).map_err(|e| e.to_string())?,
                    result.id
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        // A retry with the same content does not create a second edit revision.
        if corrected != actual || current.is_none() {
            tx.execute("INSERT INTO correction_edits(result_id,revision,text,updated) VALUES(?,?,?,?) ON CONFLICT(result_id) DO UPDATE SET revision=excluded.revision,text=excluded.text,updated=excluded.updated",params![result.id,revision+1,corrected,crate::now()]).map_err(|e|e.to_string())?;
        }
        tx.execute("DELETE FROM results WHERE id NOT IN (SELECT id FROM results ORDER BY id DESC LIMIT 200) OR created < unixepoch()-604800",[]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn result() -> ResultRecord {
        serde_json::from_value(json!({"id":0,"created":crate::now(),"scene":"chat","layout":"paragraph","revision":0,"original":"原识别Tablets","text":"模型Tablets","risks":["unrefined"],"app":"Synthetic","delivery":"preview","feedback":false})).unwrap()
    }
    #[test]
    fn saved_edit_preserves_original_and_survives_library_history_separation() {
        let mut db = Store::open(std::path::Path::new(":memory:")).unwrap();
        let r = result();
        let saved = db
            .save_correction_edit(
                &r,
                0,
                &r.text,
                "模型typeless",
                &[Pair {
                    wrong: "Tablets".into(),
                    correct: "typeless".into(),
                }],
            )
            .unwrap();
        assert_eq!(db.result(saved.id).unwrap().text, r.text);
        assert_eq!(db.effective_text(&saved).unwrap(), "模型typeless");
        db.clear_history().unwrap();
        assert!(db.correction_edit(saved.id).unwrap().is_none());
        assert_eq!(db.corrections().unwrap().groups.len(), 1);
    }
    #[test]
    fn conflict_rolls_back_edit_and_temporary_history() {
        let mut db = Store::open(std::path::Path::new(":memory:")).unwrap();
        let mut b = Book::default();
        b.remember(&[Pair {
            wrong: "Tablets".into(),
            correct: "typeless".into(),
        }])
        .unwrap();
        db.save_corrections(b, 0).unwrap();
        let r = result();
        assert!(db
            .save_correction_edit(
                &r,
                0,
                &r.text,
                "other",
                &[Pair {
                    wrong: "TABLETS".into(),
                    correct: "other".into()
                }]
            )
            .is_err());
        assert!(db.history().unwrap().is_empty());
    }
    #[test]
    fn revision_and_undo_are_transactional() {
        let mut db = Store::open(std::path::Path::new(":memory:")).unwrap();
        let mut b = db.corrections().unwrap();
        b.remember(&[Pair {
            wrong: "华池追问".into(),
            correct: "划词追问".into(),
        }])
        .unwrap();
        let saved = db.save_corrections(b.clone(), 0).unwrap();
        assert_eq!(saved.revision, 1);
        assert!(db.save_corrections(b, 0).is_err());
        let undone = db.undo_corrections(1).unwrap();
        assert!(undone.groups.is_empty());
        assert_eq!(undone.revision, 2);
    }
}
