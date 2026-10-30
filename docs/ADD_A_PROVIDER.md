# Add an agent to Tally

An agent is a **provider**: a Rust file that reads the agent's session logs and turns them into `UsageEvent`s. The rest of the app (SQLite store, watcher, totals, UI) doesn't change.

Count about an hour for an agent that writes JSONL.

## 1. Find the files

Find where the agent writes its sessions on Windows and what a line holds. At minimum you need:

- a timestamp (RFC 3339);
- the token counters (input, output, and cache if it exists);
- a stable id per response, to dedup if the same line shows up again.

The model and the working directory are a bonus; the app shows them when they are there.

## 2. Add fixtures

Copy a real session into `fixtures/<agent-id>/`, then **scrub it**: replace prompts, answers and personal paths with neutral text. Keep the counters as they are.

Add a few hard cases on purpose: a duplicated line, a broken JSON line, a line without usage.

## 3. Write the provider

Create `src-tauri/src/providers/<agent_id>.rs`:

```rust
use super::{home, num, parse_ts, project_name, read_lines, Provider, UsageEvent};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct MyAgent;

impl Provider for MyAgent {
    fn id(&self) -> &'static str { "my-agent" }
    fn display_name(&self) -> &'static str { "My Agent" }

    fn default_paths(&self) -> Vec<PathBuf> {
        vec![home().join(".my-agent").join("sessions")]
    }

    fn parse_file(&self, path: &Path, from_offset: u64) -> Result<(Vec<UsageEvent>, u64)> {
        let (lines, offset) = read_lines(path, from_offset)?;
        let events = lines.iter().filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            let usage = v.get("usage")?;
            Some(UsageEvent {
                id: v.get("id")?.as_str()?.to_string(),
                agent: self.id().to_string(),
                model: v.get("model").and_then(|m| m.as_str()).map(str::to_string),
                timestamp: parse_ts(v.get("timestamp")?)?,
                input: num(usage, "input_tokens"),
                output: num(usage, "output_tokens"),
                cache_read: num(usage, "cache_read_tokens"),
                cache_write: 0,
                project: project_name(v.get("cwd").and_then(|c| c.as_str())),
            })
        }).collect();
        Ok((events, offset))
    }
}
```

The `providers` module helpers do the heavy lifting:

| Helper | Role |
| --- | --- |
| `read_lines(path, offset)` | Reads complete lines from `offset` and leaves out a last line that isn't finished yet. |
| `parse_ts(value)` | Turns an RFC 3339 timestamp into a `DateTime<Utc>`. |
| `num(value, key)` | Reads a counter, 0 when missing. |
| `project_name(cwd)` | Keeps the last folder name as the project. |

If the agent doesn't write JSONL, override `source()` to pick the files to read (see `opencode.rs`, which reads a SQLite database).

Rules:

- **Read only.** Never change the agent's files.
- **Never crash on a line.** An unreadable line is skipped.
- **`id` unique per agent.** The store dedups on it, so rereading a file never counts anything twice.
- **Input without cache.** If the agent includes cache in input, subtract it.

## 4. Register it

In `src-tauri/src/providers/mod.rs`:

```rust
mod my_agent;
pub use my_agent::MyAgent;

pub fn registry() -> Vec<Box<dyn Provider>> {
    vec![Box::new(ClaudeCode), Box::new(Codex), Box::new(MyAgent)]
}
```

On the front end, add its colour and name in `src/lib/agents.ts`.

## 5. Test

Add a `tests` module to your file, modelled on `claude_code.rs`:

- the first line's counters are right;
- duplicates share the same `id`;
- resuming from an offset rereads nothing.

```bash
cd src-tauri
cargo test
```

Then run the app with `npm run tauri dev` and check that the agent shows up in the By agent view.

## 6. Open the PR

Say in the description where the agent writes its files and which version you tested with.
