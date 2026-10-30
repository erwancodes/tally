use super::{num, parse_ts, project_name, read_lines, Provider, UsageEvent};
use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Claude Code writes one JSONL line per content block, so one API message can
/// appear several times with the same usage. Dedup on `message.id` + `requestId`.
pub struct ClaudeCode;

impl Provider for ClaudeCode {
    fn id(&self) -> &'static str {
        "claude-code"
    }

    fn display_name(&self) -> &'static str {
        "Claude Code"
    }

    fn default_paths(&self) -> Vec<PathBuf> {
        vec![super::home().join(".claude").join("projects")]
    }

    fn parse_file(&self, path: &Path, from_offset: u64) -> Result<(Vec<UsageEvent>, u64)> {
        let (lines, offset) = read_lines(path, from_offset)?;
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
        let events = lines.iter().filter_map(|l| parse_line(l, stem.as_deref())).collect();
        Ok((events, offset))
    }
}

fn parse_line(line: &str, file_session: Option<&str>) -> Option<UsageEvent> {
    // Cheap pre-filter: most lines are tool output without usage.
    if !line.contains("\"usage\"") {
        return None;
    }
    let v: Value = serde_json::from_str(line).ok()?;
    let msg = v.get("message")?;
    let usage = msg.get("usage")?;
    let msg_id = msg.get("id").and_then(Value::as_str)?;
    let request_id = v.get("requestId").and_then(Value::as_str).unwrap_or("");
    let model = msg.get("model").and_then(Value::as_str).filter(|m| *m != "<synthetic>");
    Some(UsageEvent {
        id: format!("{msg_id}:{request_id}"),
        agent: "claude-code".into(),
        model: model.map(str::to_string),
        timestamp: parse_ts(v.get("timestamp")?)?,
        input: num(usage, "input_tokens"),
        output: num(usage, "output_tokens"),
        cache_read: num(usage, "cache_read_input_tokens"),
        cache_write: num(usage, "cache_creation_input_tokens"),
        project: project_name(v.get("cwd").and_then(Value::as_str)),
        session: v.get("sessionId").and_then(Value::as_str).or(file_session).map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::fixture;
    use std::collections::HashSet;

    #[test]
    fn parses_fixture_and_skips_bad_lines() {
        let path = fixture("claude-code/session.jsonl");
        let (events, offset) = ClaudeCode.parse_file(&path, 0).unwrap();
        assert_eq!(offset, std::fs::metadata(&path).unwrap().len());
        assert_eq!(events.len(), 11);
        let first = &events[0];
        assert_eq!(first.agent, "claude-code");
        assert_eq!(first.model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(first.project.as_deref(), Some("demo"));
        assert!(first.session.is_some());
        assert_eq!((first.input, first.output), (2, 516));
        assert_eq!((first.cache_read, first.cache_write), (50328, 37109));
    }

    #[test]
    fn duplicate_lines_share_an_id() {
        let (events, _) = ClaudeCode.parse_file(&fixture("claude-code/session.jsonl"), 0).unwrap();
        let ids: HashSet<_> = events.iter().map(|e| e.id.clone()).collect();
        assert_eq!(ids.len(), 6);
    }

    #[test]
    fn resumes_from_offset() {
        let path = fixture("claude-code/session.jsonl");
        let (_, end) = ClaudeCode.parse_file(&path, 0).unwrap();
        let (again, end2) = ClaudeCode.parse_file(&path, end).unwrap();
        assert!(again.is_empty());
        assert_eq!(end, end2);
    }
}
