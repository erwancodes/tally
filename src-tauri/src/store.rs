//! SQLite store: raw events (deduped on id) and per-file read offsets.
//! Only aggregated counters live here, never session content.

use crate::providers::UsageEvent;
use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;

pub struct Store {
    conn: Connection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileState {
    pub size: u64,
    pub mtime: i64,
    pub offset: u64,
}

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
pub struct Totals {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Totals {
    pub fn total(&self, include_cache: bool) -> u64 {
        let base = self.input + self.output;
        if include_cache {
            base + self.cache_read + self.cache_write
        } else {
            base
        }
    }

    fn add(&mut self, o: &Totals) {
        self.input += o.input;
        self.output += o.output;
        self.cache_read += o.cache_read;
        self.cache_write += o.cache_write;
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentTotals {
    pub agent: String,
    pub totals: Totals,
}

#[derive(Debug, Clone, Serialize)]
pub struct Period {
    pub totals: Totals,
    pub previous: Totals,
    pub by_agent: Vec<AgentTotals>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub today: Period,
    pub week: Period,
    pub month: Period,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayStat {
    /// Local date, YYYY-MM-DD.
    pub day: String,
    pub by_agent: Vec<AgentTotals>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelTotals {
    pub agent: String,
    pub model: String,
    pub totals: Totals,
    pub last_seen_ms: i64,
}

const TOTALS_SQL: &str =
    "COALESCE(SUM(input),0), COALESCE(SUM(output),0), COALESCE(SUM(cache_read),0), COALESCE(SUM(cache_write),0)";

fn row_totals(row: &rusqlite::Row, start: usize) -> rusqlite::Result<Totals> {
    Ok(Totals {
        input: row.get::<_, i64>(start)? as u64,
        output: row.get::<_, i64>(start + 1)? as u64,
        cache_read: row.get::<_, i64>(start + 2)? as u64,
        cache_write: row.get::<_, i64>(start + 3)? as u64,
    })
}

fn local_ms(date: NaiveDate) -> i64 {
    Local
        .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .map(|d| d.timestamp_millis())
        .unwrap_or(0)
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS events(
               id TEXT NOT NULL, agent TEXT NOT NULL, model TEXT, ts INTEGER NOT NULL,
               input INTEGER NOT NULL, output INTEGER NOT NULL,
               cache_read INTEGER NOT NULL, cache_write INTEGER NOT NULL, project TEXT,
               PRIMARY KEY(agent, id));
             CREATE INDEX IF NOT EXISTS events_ts ON events(ts);
             CREATE TABLE IF NOT EXISTS file_state(
               path TEXT PRIMARY KEY, size INTEGER NOT NULL, mtime INTEGER NOT NULL, offset INTEGER NOT NULL);",
        )?;
        Ok(Store { conn })
    }

    pub fn file_state(&self, path: &str) -> Result<Option<FileState>> {
        Ok(self
            .conn
            .query_row("SELECT size, mtime, offset FROM file_state WHERE path=?1", [path], |r| {
                Ok(FileState { size: r.get::<_, i64>(0)? as u64, mtime: r.get(1)?, offset: r.get::<_, i64>(2)? as u64 })
            })
            .optional()?)
    }

    /// Inserts events and records the new offset in one transaction, so a crash
    /// never leaves an offset ahead of its events.
    pub fn ingest(&mut self, path: &str, state: FileState, events: &[UsageEvent]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut added = 0;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR IGNORE INTO events(id, agent, model, ts, input, output, cache_read, cache_write, project)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for e in events {
                added += stmt.execute(params![
                    e.id, e.agent, e.model, e.timestamp.timestamp_millis(),
                    e.input as i64, e.output as i64, e.cache_read as i64, e.cache_write as i64, e.project
                ])?;
            }
            tx.execute(
                "INSERT INTO file_state(path, size, mtime, offset) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(path) DO UPDATE SET size=?2, mtime=?3, offset=?4",
                params![path, state.size as i64, state.mtime, state.offset as i64],
            )?;
        }
        tx.commit()?;
        Ok(added)
    }

    pub fn reset(&mut self) -> Result<()> {
        self.conn.execute_batch("DELETE FROM events; DELETE FROM file_state;")?;
        Ok(())
    }

    fn by_agent(&self, from: i64, to: i64, agents: &[String]) -> Result<Vec<AgentTotals>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT agent, {TOTALS_SQL} FROM events WHERE ts >= ?1 AND ts < ?2 GROUP BY agent"
        ))?;
        let rows = stmt
            .query_map(params![from, to], |r| Ok(AgentTotals { agent: r.get(0)?, totals: row_totals(r, 1)? }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows.into_iter().filter(|a| agents.contains(&a.agent)).collect())
    }

    fn period(&self, from: i64, to: i64, prev_from: i64, prev_to: i64, agents: &[String]) -> Result<Period> {
        let by_agent = self.by_agent(from, to, agents)?;
        let mut totals = Totals::default();
        by_agent.iter().for_each(|a| totals.add(&a.totals));
        let mut previous = Totals::default();
        self.by_agent(prev_from, prev_to, agents)?.iter().for_each(|a| previous.add(&a.totals));
        Ok(Period { totals, previous, by_agent })
    }

    /// Today vs yesterday at the same time, last 7 days vs the 7 before,
    /// month to date vs the same span of last month. Local time.
    pub fn summary(&self, agents: &[String]) -> Result<Summary> {
        let now = Local::now();
        let now_ms = now.timestamp_millis();
        let today = now.date_naive();
        let day = Duration::days(1).num_milliseconds();
        let today_start = local_ms(today);

        let week_start = local_ms(today - Duration::days(6));
        let prev_week_start = local_ms(today - Duration::days(13));

        let month_start_date = today.with_day(1).unwrap();
        let month_start = local_ms(month_start_date);
        let prev_month_date = (month_start_date - Duration::days(1)).with_day(1).unwrap();
        let prev_month_start = local_ms(prev_month_date);
        let prev_month_end = (prev_month_start + (now_ms - month_start)).min(month_start);

        Ok(Summary {
            today: self.period(today_start, now_ms + 1, today_start - day, now_ms - day, agents)?,
            week: self.period(week_start, now_ms + 1, prev_week_start, week_start, agents)?,
            month: self.period(month_start, now_ms + 1, prev_month_start, prev_month_end, agents)?,
        })
    }

    pub fn today_total(&self, agents: &[String], include_cache: bool) -> Result<u64> {
        let now = Local::now();
        let start = local_ms(now.date_naive());
        Ok(self
            .by_agent(start, now.timestamp_millis() + 1, agents)?
            .iter()
            .map(|a| a.totals.total(include_cache))
            .sum())
    }

    /// One entry per local day for the last `days` days, oldest first, empty days included.
    pub fn daily(&self, days: u32, agents: &[String]) -> Result<Vec<DayStat>> {
        let today = Local::now().date_naive();
        let first = today - Duration::days(days.saturating_sub(1) as i64);
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT date(ts/1000, 'unixepoch', 'localtime') AS d, agent, {TOTALS_SQL}
             FROM events WHERE ts >= ?1 GROUP BY d, agent ORDER BY d"
        ))?;
        let rows = stmt
            .query_map([local_ms(first)], |r| {
                Ok((r.get::<_, String>(0)?, AgentTotals { agent: r.get(1)?, totals: row_totals(r, 2)? }))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut out: Vec<DayStat> = (0..days as i64)
            .map(|i| DayStat { day: (first + Duration::days(i)).format("%Y-%m-%d").to_string(), by_agent: vec![] })
            .collect();
        for (d, a) in rows {
            if !agents.contains(&a.agent) {
                continue;
            }
            if let Some(slot) = out.iter_mut().find(|s| s.day == d) {
                slot.by_agent.push(a);
            }
        }
        Ok(out)
    }

    /// All-time totals per agent and model.
    pub fn models(&self) -> Result<Vec<ModelTotals>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT agent, COALESCE(model, ''), {TOTALS_SQL}, MAX(ts) FROM events GROUP BY agent, model"
        ))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(ModelTotals {
                    agent: r.get(0)?,
                    model: r.get(1)?,
                    totals: row_totals(r, 2)?,
                    last_seen_ms: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn ev(id: &str, agent: &str, hours_ago: i64, input: u64) -> UsageEvent {
        UsageEvent {
            id: id.into(),
            agent: agent.into(),
            model: Some("m".into()),
            timestamp: Utc::now() - Duration::hours(hours_ago),
            input,
            output: 10,
            cache_read: 100,
            cache_write: 1,
            project: None,
        }
    }

    fn agents() -> Vec<String> {
        vec!["claude-code".into(), "codex".into()]
    }

    #[test]
    fn dedups_on_id_and_tracks_offsets() {
        let mut s = Store::in_memory().unwrap();
        let st = FileState { size: 10, mtime: 1, offset: 10 };
        let a = ev("x", "claude-code", 0, 5);
        assert_eq!(s.ingest("f", st, &[a.clone(), a.clone()]).unwrap(), 1);
        assert_eq!(s.ingest("f", st, &[a]).unwrap(), 0);
        assert_eq!(s.file_state("f").unwrap(), Some(st));
    }

    #[test]
    fn same_id_in_two_agents_is_kept() {
        let mut s = Store::in_memory().unwrap();
        let st = FileState { size: 0, mtime: 0, offset: 0 };
        let n = s.ingest("f", st, &[ev("x", "claude-code", 0, 1), ev("x", "codex", 0, 1)]).unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn summary_and_daily() {
        let mut s = Store::in_memory().unwrap();
        let st = FileState { size: 0, mtime: 0, offset: 0 };
        s.ingest("f", st, &[ev("a", "claude-code", 0, 5), ev("b", "codex", 0, 7), ev("c", "codex", 24 * 40, 9)])
            .unwrap();
        let sum = s.summary(&agents()).unwrap();
        assert_eq!(sum.today.totals.input, 12);
        assert_eq!(sum.today.by_agent.len(), 2);
        assert_eq!(sum.today.totals.total(true), 12 + 20 + 200 + 2);
        assert_eq!(sum.today.totals.total(false), 32);
        let only_claude = s.summary(&["claude-code".into()]).unwrap();
        assert_eq!(only_claude.today.totals.input, 5);
        let days = s.daily(30, &agents()).unwrap();
        assert_eq!(days.len(), 30);
        assert_eq!(days.last().unwrap().by_agent.len(), 2);
        assert_eq!(s.models().unwrap().len(), 2);
    }
}
