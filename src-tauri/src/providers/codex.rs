use super::{num, parse_ts, project_name, read_lines, Provider, UsageEvent};
use anyhow::Result;
use serde_json::Value;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Codex CLI rollouts. Recent versions write a `token_usage_record` per API
/// response (keyed by `response_id`); older ones only emit `token_count` events.
/// A file uses records when it has any, so the two are never counted together.
/// OpenAI's `input_tokens` includes cached tokens, so cached is subtracted.
pub struct Codex;

const HEAD_BYTES: u64 = 256 * 1024;

impl Provider for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn display_name(&self) -> &'static str {
        "Codex"
    }

    fn default_paths(&self) -> Vec<PathBuf> {
        vec![super::home().join(".codex").join("sessions")]
    }

    fn parse_file(&self, path: &Path, from_offset: u64) -> Result<(Vec<UsageEvent>, u64)> {
        let mut state = FileState::from_head(path)?;
        let (lines, offset) = read_lines(path, from_offset)?;
        let mut events = Vec::new();
        for line in &lines {
            if let Some(e) = state.parse_line(line) {
                events.push(e);
            }
        }
        Ok((events, offset))
    }
}

struct FileState {
    file_key: String,
    uses_records: bool,
    model: Option<String>,
    project: Option<String>,
}

impl FileState {
    /// Session id, cwd, model and record mode come from the start of the file,
    /// which an incremental pass does not re-read.
    fn from_head(path: &Path) -> Result<Self> {
        let mut head = String::new();
        File::open(path)?.take(HEAD_BYTES).read_to_string(&mut head).ok();
        let mut state = FileState {
            file_key: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            uses_records: head.contains("\"token_usage_record\""),
            model: None,
            project: None,
        };
        for line in head.lines() {
            if line.contains("\"session_meta\"") || line.contains("\"turn_context\"") {
                state.absorb_context(line);
            }
        }
        Ok(state)
    }

    fn absorb_context(&mut self, line: &str) {
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        let Some(p) = v.get("payload") else { return };
        match v.get("type").and_then(Value::as_str) {
            Some("session_meta") => {
                if let Some(id) = p.get("id").and_then(Value::as_str) {
                    self.file_key = id.to_string();
                }
                self.project = project_name(p.get("cwd").and_then(Value::as_str)).or(self.project.take());
            }
            Some("turn_context") => {
                if let Some(m) = p.get("model").and_then(Value::as_str) {
                    self.model = Some(m.to_string());
                }
                self.project = project_name(p.get("cwd").and_then(Value::as_str)).or(self.project.take());
            }
            _ => {}
        }
    }

    fn parse_line(&mut self, line: &str) -> Option<UsageEvent> {
        if line.contains("\"turn_context\"") || line.contains("\"session_meta\"") {
            self.absorb_context(line);
            return None;
        }
        if line.contains("\"token_usage_record\"") {
            self.uses_records = true;
            let v: Value = serde_json::from_str(line).ok()?;
            let p = v.get("payload")?;
            let id = p.get("response_id").and_then(Value::as_str)?;
            return self.event(id.to_string(), p.get("usage")?, v.get("timestamp")?);
        }
        if !self.uses_records && line.contains("\"token_count\"") {
            let v: Value = serde_json::from_str(line).ok()?;
            let info = v.get("payload")?.get("info")?;
            // Codex repeats the same count; the running total makes a stable key.
            let total = num(info.get("total_token_usage")?, "total_tokens");
            let id = format!("{}:{}", self.file_key, total);
            return self.event(id, info.get("last_token_usage")?, v.get("timestamp")?);
        }
        None
    }

    fn event(&self, id: String, usage: &Value, ts: &Value) -> Option<UsageEvent> {
        let cached = num(usage, "cached_input_tokens");
        Some(UsageEvent {
            id,
            agent: "codex".into(),
            model: self.model.clone(),
            timestamp: parse_ts(ts)?,
            input: num(usage, "input_tokens").saturating_sub(cached),
            output: num(usage, "output_tokens"),
            cache_read: cached,
            cache_write: num(usage, "cache_write_input_tokens"),
            project: self.project.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::fixture;
    use std::collections::HashSet;

    #[test]
    fn records_win_over_token_count() {
        let (events, _) = Codex.parse_file(&fixture("codex/rollout-with-records.jsonl"), 0).unwrap();
        assert_eq!(events.len(), 4);
        assert!(events.iter().all(|e| e.id.starts_with("resp_")));
        let second = &events[1];
        assert_eq!(second.model.as_deref(), Some("gpt-6-sol"));
        assert_eq!(second.project.as_deref(), Some("demo"));
        assert_eq!((second.input, second.cache_read, second.output), (49471 - 36864, 36864, 365));
    }

    #[test]
    fn legacy_token_count_is_deduped_by_total() {
        let (events, _) = Codex.parse_file(&fixture("codex/rollout-legacy.jsonl"), 0).unwrap();
        assert_eq!(events.len(), 5);
        let ids: HashSet<_> = events.iter().map(|e| e.id.clone()).collect();
        assert_eq!(ids.len(), 4);
    }

    #[test]
    fn incremental_pass_keeps_model() {
        let path = fixture("codex/rollout-with-records.jsonl");
        let text = std::fs::read_to_string(&path).unwrap();
        let mid = text.find("\"token_usage_record\"").unwrap();
        let line_start = text[..mid].rfind('\n').map(|i| i + 1).unwrap_or(0) as u64;
        let (events, _) = Codex.parse_file(&path, line_start).unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].model.as_deref(), Some("gpt-6-sol"));
    }
}
