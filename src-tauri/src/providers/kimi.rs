use super::{num, read_lines, Provider, UsageEvent};
use anyhow::Result;
use chrono::DateTime;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Kimi Code CLI. Each session writes `sessions/<md5 of cwd>/<id>/wire.jsonl`
/// under its share dir; every API step logs a `StatusUpdate` whose
/// `token_usage` already splits uncached input from cache read and creation.
/// The wire carries neither the model nor the working directory.
pub struct Kimi;

impl Provider for Kimi {
    fn id(&self) -> &'static str {
        "kimi"
    }

    fn display_name(&self) -> &'static str {
        "Kimi Code"
    }

    fn default_paths(&self) -> Vec<PathBuf> {
        let share = std::env::var_os("KIMI_SHARE_DIR").map(PathBuf::from).unwrap_or_else(|| super::home().join(".kimi"));
        vec![share.join("sessions")]
    }

    fn parse_file(&self, path: &Path, from_offset: u64) -> Result<(Vec<UsageEvent>, u64)> {
        let (lines, offset) = read_lines(path, from_offset)?;
        let session = path.parent().and_then(Path::file_name).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let events = lines.iter().filter(|l| l.contains("\"StatusUpdate\"")).filter_map(|l| parse_line(l, &session)).collect();
        Ok((events, offset))
    }
}

fn parse_line(line: &str, session: &str) -> Option<UsageEvent> {
    let v: Value = serde_json::from_str(line).ok()?;
    let msg = v.get("message")?;
    if msg.get("type").and_then(Value::as_str) != Some("StatusUpdate") {
        return None;
    }
    let p = msg.get("payload")?;
    let usage = p.get("token_usage").filter(|u| u.is_object())?;
    let secs = v.get("timestamp").and_then(Value::as_f64)?;
    let id = match p.get("message_id").and_then(Value::as_str) {
        Some(id) => id.to_string(),
        None => format!("{session}:{secs}"),
    };
    Some(UsageEvent {
        id,
        agent: "kimi".into(),
        model: None,
        timestamp: DateTime::from_timestamp_millis((secs * 1000.0) as i64)?,
        input: num(usage, "input_other"),
        output: num(usage, "output"),
        cache_read: num(usage, "input_cache_read"),
        cache_write: num(usage, "input_cache_creation"),
        project: None,
        session: Some(session.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::fixture;

    #[test]
    fn reads_status_updates_only() {
        let (events, _) = Kimi.parse_file(&fixture("kimi/sessions/abc/s1/wire.jsonl"), 0).unwrap();
        assert_eq!(events.len(), 3);
        let first = &events[0];
        assert_eq!(first.id, "chatcmpl-1");
        assert_eq!(first.session.as_deref(), Some("s1"));
        assert_eq!((first.input, first.output, first.cache_read, first.cache_write), (1200, 340, 8000, 0));
        assert_eq!(first.timestamp.timestamp_millis(), 1793300000500);
        // No message_id: keyed by session and timestamp.
        assert_eq!(events[2].id, "s1:1793300100.25");
    }
}
