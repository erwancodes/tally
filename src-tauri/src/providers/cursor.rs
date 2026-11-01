use super::{parse_ts, Provider, UsageEvent};
use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Cursor. Conversations live in `state.vscdb`, table `cursorDiskKV`: one
/// `bubbleId:<composer>:<bubble>` row per message, one `composerData:<composer>`
/// row per conversation with the chosen model. Cursor fills `tokenCount` on only
/// some messages, so totals are a lower bound.
pub struct Cursor;

impl Provider for Cursor {
    fn id(&self) -> &'static str {
        "cursor"
    }

    fn display_name(&self) -> &'static str {
        "Cursor"
    }

    fn default_paths(&self) -> Vec<PathBuf> {
        let config = dirs::config_dir().unwrap_or_else(|| super::home().join(".config"));
        vec![config.join("Cursor").join("User").join("globalStorage")]
    }

    /// The database; its WAL maps back to it.
    fn source(&self, path: &Path) -> Option<PathBuf> {
        let name = path.file_name()?.to_str()?;
        (name == "state.vscdb" || name == "state.vscdb-wal").then(|| path.with_file_name("state.vscdb"))
    }

    /// Read whole each time; dedup on the bubble id keeps totals right.
    fn parse_file(&self, path: &Path, _from_offset: u64) -> Result<(Vec<UsageEvent>, u64)> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        let mut models = HashMap::new();
        let mut stmt = conn.prepare(
            "SELECT substr(key, 14), json_extract(value, '$.modelConfig.modelName') FROM cursorDiskKV
             WHERE key LIKE 'composerData:%' AND json_valid(value)",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))?;
        for (composer, model) in rows.filter_map(Result::ok) {
            // "default" is Cursor's auto mode: the real model isn't recorded.
            if let Some(model) = model.filter(|m| m != "default") {
                models.insert(composer, model);
            }
        }
        // Pull only the fields we need, so large message blobs never reach Rust.
        let mut stmt = conn.prepare(
            "SELECT key, json_object('tokenCount', json_extract(value, '$.tokenCount'), 'createdAt', json_extract(value, '$.createdAt'))
             FROM cursorDiskKV WHERE key LIKE 'bubbleId:%' AND json_valid(value)
             AND json_extract(value, '$.tokenCount.inputTokens') + json_extract(value, '$.tokenCount.outputTokens') > 0",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let events = rows
            .filter_map(Result::ok)
            .filter_map(|(key, row)| parse_bubble(&key, &serde_json::from_str(&row).ok()?, &models))
            .collect();
        Ok((events, 0))
    }
}

fn parse_bubble(key: &str, v: &Value, models: &HashMap<String, String>) -> Option<UsageEvent> {
    let mut parts = key.split(':').skip(1);
    let composer = parts.next()?;
    parts.next()?;
    let tokens = v.get("tokenCount")?;
    Some(UsageEvent {
        id: key.to_string(),
        agent: "cursor".into(),
        model: models.get(composer).cloned(),
        timestamp: parse_ts(v.get("createdAt")?)?,
        input: super::num(tokens, "inputTokens"),
        output: super::num(tokens, "outputTokens"),
        cache_read: 0,
        cache_write: 0,
        project: None,
        session: Some(composer.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bubbles_with_tokens() {
        let dir = std::env::temp_dir().join(format!("tally-cursor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("state.vscdb");
        let _ = std::fs::remove_file(&db);
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE cursorDiskKV (key TEXT UNIQUE, value BLOB)").unwrap();
        let bubble = |input: u64| format!(r#"{{"createdAt":"2026-01-10T09:30:10.115Z","tokenCount":{{"inputTokens":{input},"outputTokens":20}}}}"#);
        let rows = [
            ("composerData:c1", r#"{"modelConfig":{"modelName":"grok-4.6"}}"#.to_string()),
            ("composerData:c2", r#"{"modelConfig":{"modelName":"default"}}"#.to_string()),
            ("bubbleId:c1:b1", bubble(100)),
            ("bubbleId:c2:b2", bubble(5)),
            ("bubbleId:c1:b3", r#"{"createdAt":"2026-01-10T09:30:10.115Z","tokenCount":{"inputTokens":0,"outputTokens":0}}"#.to_string()),
        ];
        for (k, v) in &rows {
            conn.execute("INSERT INTO cursorDiskKV VALUES (?1, ?2)", [k, &v.as_str()]).unwrap();
        }
        conn.execute("INSERT INTO cursorDiskKV VALUES ('bubbleId:c1:b4', NULL)", []).unwrap();
        drop(conn);

        let (mut events, _) = Cursor.parse_file(&db, 0).unwrap();
        events.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id, "bubbleId:c1:b1");
        assert_eq!((events[0].input, events[0].output), (100, 20));
        assert_eq!(events[0].model.as_deref(), Some("grok-4.6"));
        assert_eq!(events[0].session.as_deref(), Some("c1"));
        assert_eq!(events[1].model, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn maps_wal_to_db() {
        let wal = Path::new("C:/x/state.vscdb-wal");
        assert_eq!(Cursor.source(wal), Some(PathBuf::from("C:/x/state.vscdb")));
        assert_eq!(Cursor.source(Path::new("C:/x/state.vscdb.backup")), None);
    }
}
