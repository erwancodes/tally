use super::{project_name, Provider, UsageEvent};
use anyhow::Result;
use chrono::DateTime;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// OpenCode. Current versions keep every message in `opencode.db` (table
/// `message`, one JSON blob per row); older ones wrote one
/// `storage/message/<session>/msg_*.json` file per message. Both carry the same
/// shape, with `tokens.input` already excluding cache. A message is rewritten
/// while it streams, so only completed ones are counted: the store keeps the
/// first version of an id.
pub struct OpenCode;

impl Provider for OpenCode {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn display_name(&self) -> &'static str {
        "OpenCode"
    }

    fn default_paths(&self) -> Vec<PathBuf> {
        let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| super::home().join(".local").join("share"));
        vec![data.join("opencode")]
    }

    /// The database (its WAL maps back to it) and legacy message files.
    fn source(&self, path: &Path) -> Option<PathBuf> {
        let name = path.file_name()?.to_str()?;
        if name == "opencode.db" || name == "opencode.db-wal" {
            return Some(path.with_file_name("opencode.db"));
        }
        let legacy = name.starts_with("msg_") && name.ends_with(".json") && path.components().any(|c| c.as_os_str() == "message");
        legacy.then(|| path.to_path_buf())
    }

    /// Skips `bin/` and `snapshot/`, which hold thousands of unrelated files.
    fn files(&self, root: &Path) -> Vec<PathBuf> {
        let db = root.join("opencode.db");
        let legacy = WalkDir::new(root.join("storage").join("message"))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| self.source(e.path()));
        db.exists().then_some(db).into_iter().chain(legacy).collect()
    }

    /// Both formats are read whole each time; dedup on the message id keeps totals right.
    fn parse_file(&self, path: &Path, _from_offset: u64) -> Result<(Vec<UsageEvent>, u64)> {
        if path.extension().is_some_and(|x| x == "db") {
            return Ok((read_db(path)?, 0));
        }
        let v: Value = serde_json::from_slice(&std::fs::read(path)?)?;
        let mut event = parse_message(&v);
        // Legacy files sit in `message/<session>/`.
        if let Some(e) = event.as_mut().filter(|e| e.session.is_none()) {
            e.session = path.parent().and_then(Path::file_name).map(|s| s.to_string_lossy().into_owned());
        }
        Ok((event.into_iter().collect(), 0))
    }
}

fn read_db(path: &Path) -> Result<Vec<UsageEvent>> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    // Pull only the fields we need, so large message blobs never reach Rust.
    let mut stmt = conn.prepare(
        "SELECT json_object('id', id, 'role', json_extract(data, '$.role'), 'tokens', json_extract(data, '$.tokens'),
                'modelID', json_extract(data, '$.modelID'), 'time', json_extract(data, '$.time'), 'path', json_extract(data, '$.path'),
                'session', session_id)
         FROM message WHERE json_extract(data, '$.role') = 'assistant' AND json_extract(data, '$.time.completed') IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.filter_map(|r| serde_json::from_str::<Value>(&r.ok()?).ok()).filter_map(|v| parse_message(&v)).collect())
}

fn parse_message(v: &Value) -> Option<UsageEvent> {
    if v.get("role")?.as_str()? != "assistant" {
        return None;
    }
    let time = v.get("time")?;
    time.get("completed")?.as_i64()?;
    let t = v.get("tokens")?;
    let n = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    let cache = t.get("cache").cloned().unwrap_or(Value::Null);
    let event = UsageEvent {
        id: v.get("id")?.as_str()?.to_string(),
        agent: "opencode".into(),
        model: v.get("modelID").and_then(Value::as_str).map(str::to_string),
        timestamp: DateTime::from_timestamp_millis(time.get("created")?.as_i64()?)?,
        input: n(t, "input"),
        output: n(t, "output") + n(t, "reasoning"),
        cache_read: n(&cache, "read"),
        cache_write: n(&cache, "write"),
        project: project_name(v.get("path").and_then(|p| p.get("cwd")).and_then(Value::as_str)),
        session: v.get("session").or_else(|| v.get("sessionID")).and_then(Value::as_str).map(str::to_string),
    };
    (event.input + event.output + event.cache_read + event.cache_write > 0).then_some(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::fixture;

    fn msg(id: &str, role: &str, completed: bool, input: u64) -> String {
        let done = if completed { r#","completed":1793300001000"# } else { "" };
        format!(
            r#"{{"role":"{role}","modelID":"claude-sonnet-5-5","path":{{"cwd":"C:\\dev\\demo"}},"time":{{"created":1793300000000{done}}},"tokens":{{"input":{input},"output":20,"reasoning":5,"cache":{{"read":300,"write":40}}}},"id_hint":"{id}"}}"#
        )
    }

    #[test]
    fn reads_completed_assistant_messages_from_db() {
        let dir = std::env::temp_dir().join(format!("tally-opencode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("opencode.db");
        let _ = std::fs::remove_file(&db);
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT)").unwrap();
        for (id, role, done, input) in [("m1", "assistant", true, 100), ("m2", "user", true, 0), ("m3", "assistant", false, 50)] {
            conn.execute("INSERT INTO message VALUES (?1, 's', 0, 0, ?2)", [id, &msg(id, role, done, input)]).unwrap();
        }
        drop(conn);

        let (events, _) = OpenCode.parse_file(&db, 0).unwrap();
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e.id, "m1");
        assert_eq!((e.input, e.output, e.cache_read, e.cache_write), (100, 25, 300, 40));
        assert_eq!(e.model.as_deref(), Some("claude-sonnet-5-5"));
        assert_eq!(e.project.as_deref(), Some("demo"));
        assert_eq!(e.session.as_deref(), Some("s"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_legacy_message_file() {
        let path = fixture("opencode/storage/message/ses_1/msg_1.json");
        assert_eq!(OpenCode.source(&path), Some(path.clone()));
        let (events, _) = OpenCode.parse_file(&path, 0).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "msg_1");
        assert_eq!(events[0].session.as_deref(), Some("ses_1"));
        assert_eq!((events[0].input, events[0].output, events[0].cache_read), (1046, 164, 25452));
    }

    #[test]
    fn maps_wal_to_db() {
        let wal = Path::new("C:/x/opencode/opencode.db-wal");
        assert_eq!(OpenCode.source(wal), Some(PathBuf::from("C:/x/opencode/opencode.db")));
        assert_eq!(OpenCode.source(Path::new("C:/x/opencode/bin/node.exe")), None);
    }
}
