//! A provider turns one agent's session files into `UsageEvent`s.
//! Adding an agent means adding a file here and one line in `registry()`.

mod claude_code;
mod codex;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub use claude_code::ClaudeCode;
pub use codex::Codex;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UsageEvent {
    /// Dedup key, unique per agent.
    pub id: String,
    pub agent: String,
    pub model: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub project: Option<String>,
}

pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn default_paths(&self) -> Vec<PathBuf>;
    /// Parses `path` from `from_offset`, returns the events and the offset to resume from.
    fn parse_file(&self, path: &Path, from_offset: u64) -> Result<(Vec<UsageEvent>, u64)>;
}

pub fn registry() -> Vec<Box<dyn Provider>> {
    vec![Box::new(ClaudeCode), Box::new(Codex)]
}

pub(crate) fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Reads complete lines from `from_offset`. A trailing line without `\n` is left
/// for the next pass, since the agent may still be writing it.
pub(crate) fn read_lines(path: &Path, from_offset: u64) -> Result<(Vec<String>, u64)> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(from_offset))?;
    let mut reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut offset = from_offset;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let n = reader.read_until(b'\n', &mut buf)?;
        if n == 0 || buf.last() != Some(&b'\n') {
            break;
        }
        offset += n as u64;
        lines.push(String::from_utf8_lossy(&buf).trim().to_string());
    }
    Ok((lines, offset))
}

pub(crate) fn parse_ts(v: &serde_json::Value) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(v.as_str()?).ok().map(|d| d.with_timezone(&Utc))
}

pub(crate) fn num(v: &serde_json::Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}

/// Last folder name of a working directory, used as the project label.
pub(crate) fn project_name(cwd: Option<&str>) -> Option<String> {
    let cwd = cwd?.trim_end_matches(['/', '\\']);
    cwd.rsplit(['/', '\\']).next().filter(|s| !s.is_empty()).map(str::to_string)
}

#[cfg(test)]
pub(crate) fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("fixtures").join(rel)
}
