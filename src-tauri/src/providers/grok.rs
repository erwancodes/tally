use super::{home, num, parse_ts, project_name, Provider, UsageEvent};
use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Grok CLI. Each session folder holds a `usage.json`, rewritten after every
/// turn, with per-model counters for each turn. `inputTokens` includes cached
/// reads and `outputTokens` includes reasoning. The working directory is in the
/// sibling `summary.json`.
pub struct Grok;

impl Provider for Grok {
    fn id(&self) -> &'static str {
        "grok"
    }

    fn display_name(&self) -> &'static str {
        "Grok CLI"
    }

    fn default_paths(&self) -> Vec<PathBuf> {
        vec![home().join(".grok").join("sessions")]
    }

    fn source(&self, path: &Path) -> Option<PathBuf> {
        (path.file_name()? == "usage.json").then(|| path.to_path_buf())
    }

    /// Read whole each time; dedup on session, turn and model keeps totals right.
    fn parse_file(&self, path: &Path, _from_offset: u64) -> Result<(Vec<UsageEvent>, u64)> {
        let v: Value = serde_json::from_slice(&std::fs::read(path)?)?;
        let summary = path.parent().and_then(|d| std::fs::read(d.join("summary.json")).ok());
        let summary: Option<Value> = summary.and_then(|s| serde_json::from_slice(&s).ok());
        let cwd = summary.as_ref().and_then(|s| s.get("info")?.get("cwd")?.as_str());
        let project = project_name(cwd);
        let Some(session) = v.get("sessionId").and_then(Value::as_str) else {
            return Ok((Vec::new(), 0));
        };
        let turns = v.get("turns").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
        let events = turns.iter().flat_map(|turn| parse_turn(turn, session, &project)).collect();
        Ok((events, 0))
    }
}

fn parse_turn(turn: &Value, session: &str, project: &Option<String>) -> Vec<UsageEvent> {
    let (Some(number), Some(timestamp)) = (turn.get("turnNumber").and_then(Value::as_u64), turn.get("endedAt").and_then(parse_ts)) else {
        return Vec::new();
    };
    let Some(models) = turn.get("modelUsage").and_then(Value::as_object) else {
        return Vec::new();
    };
    models
        .iter()
        .filter_map(|(model, u)| {
            let cache_read = num(u, "cachedReadTokens");
            let event = UsageEvent {
                id: format!("{session}:{number}:{model}"),
                agent: "grok".into(),
                model: Some(model.clone()),
                timestamp,
                input: num(u, "inputTokens").saturating_sub(cache_read),
                output: num(u, "outputTokens"),
                cache_read,
                cache_write: num(u, "cacheCreationTokens"),
                project: project.clone(),
                session: Some(session.to_string()),
            };
            (event.input + event.output + event.cache_read + event.cache_write > 0).then_some(event)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_each_turn_and_model() {
        let dir = std::env::temp_dir().join(format!("tally-grok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("usage.json");
        std::fs::write(&path, r#"{"sessionId":"s1","turns":[
            {"turnNumber":1,"endedAt":"2026-10-15T13:56:55Z","modelUsage":{
                "grok-4.7":{"inputTokens":1000,"cachedReadTokens":800,"outputTokens":50,"reasoningTokens":20},
                "grok-build":{"inputTokens":0,"outputTokens":0}}},
            {"turnNumber":2,"modelUsage":{"grok-4.7":{"inputTokens":5}}}
        ]}"#).unwrap();
        std::fs::write(dir.join("summary.json"), r#"{"info":{"cwd":"C:\\dev\\demo"}}"#).unwrap();

        let (events, _) = Grok.parse_file(&path, 0).unwrap();
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e.id, "s1:1:grok-4.7");
        assert_eq!((e.input, e.output, e.cache_read), (200, 50, 800));
        assert_eq!(e.project.as_deref(), Some("demo"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_watches_usage_files() {
        let usage = Path::new("C:/x/sessions/a/b/usage.json");
        assert_eq!(Grok.source(usage), Some(usage.to_path_buf()));
        assert_eq!(Grok.source(Path::new("C:/x/sessions/a/b/updates.jsonl")), None);
    }
}
