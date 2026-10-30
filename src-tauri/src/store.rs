//! SQLite store: raw events (deduped on id) and per-file read offsets.
//! Only aggregated counters live here, never session content.

use crate::providers::UsageEvent;
use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
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
    /// Estimated API cost in dollars; None for models without a known price.
    pub cost: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionTotals {
    pub agent: String,
    pub session: String,
    pub project: Option<String>,
    /// Models used, most tokens first.
    pub models: Vec<String>,
    pub first_ms: i64,
    pub last_ms: i64,
    pub totals: Totals,
    pub cost: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectTotals {
    pub project: Option<String>,
    pub agents: Vec<String>,
    pub sessions: usize,
    pub last_ms: i64,
    pub totals: Totals,
    pub cost: Option<f64>,
}

fn add_cost(acc: &mut Option<f64>, c: Option<f64>) {
    if let Some(c) = c {
        *acc = Some(acc.unwrap_or(0.0) + c);
    }
}

/// Local midnight `days - 1` days ago, so 1 means today.
pub fn days_ago_ms(days: u32) -> i64 {
    local_ms(Local::now().date_naive() - Duration::days(days.clamp(1, 3650) as i64 - 1))
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
               cache_read INTEGER NOT NULL, cache_write INTEGER NOT NULL, project TEXT, session TEXT,
               PRIMARY KEY(agent, id));
             CREATE INDEX IF NOT EXISTS events_ts ON events(ts);
             CREATE TABLE IF NOT EXISTS file_state(
               path TEXT PRIMARY KEY, size INTEGER NOT NULL, mtime INTEGER NOT NULL, offset INTEGER NOT NULL);",
        )?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 1 {
            // v1 adds the session column; older rows lack it, so everything is read again.
            let _ = conn.execute("ALTER TABLE events ADD COLUMN session TEXT", []);
            conn.execute_batch("DELETE FROM events; DELETE FROM file_state; PRAGMA user_version=1;")?;
        }
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
                "INSERT OR IGNORE INTO events(id, agent, model, ts, input, output, cache_read, cache_write, project, session)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;
            for e in events {
                added += stmt.execute(params![
                    e.id, e.agent, e.model, e.timestamp.timestamp_millis(),
                    e.input as i64, e.output as i64, e.cache_read as i64, e.cache_write as i64, e.project, e.session
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
                let model: String = r.get(1)?;
                let totals = row_totals(r, 2)?;
                Ok(ModelTotals {
                    agent: r.get(0)?,
                    cost: crate::pricing::cost(&model, &totals),
                    model,
                    totals,
                    last_seen_ms: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Per-session totals since `from_ms`, most recent first.
    pub fn sessions(&self, from_ms: i64, agents: &[String]) -> Result<Vec<SessionTotals>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT agent, COALESCE(session, ''), COALESCE(model, ''), MAX(project), MIN(ts), MAX(ts), {TOTALS_SQL}
             FROM events WHERE ts >= ?1 GROUP BY agent, session, model"
        ))?;
        let rows = stmt
            .query_map([from_ms], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                    row_totals(r, 6)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        // One row per model: fold them into their session, keeping model sizes for ordering.
        let mut by_key: HashMap<(String, String), (SessionTotals, Vec<(String, u64)>)> = HashMap::new();
        for (agent, session, model, project, first, last, totals) in rows {
            if !agents.contains(&agent) {
                continue;
            }
            let (s, models) = by_key.entry((agent.clone(), session.clone())).or_insert_with(|| {
                let s = SessionTotals {
                    agent,
                    session,
                    project: None,
                    models: vec![],
                    first_ms: first,
                    last_ms: last,
                    totals: Totals::default(),
                    cost: None,
                };
                (s, vec![])
            });
            s.project = s.project.take().or(project);
            s.first_ms = s.first_ms.min(first);
            s.last_ms = s.last_ms.max(last);
            add_cost(&mut s.cost, crate::pricing::cost(&model, &totals));
            s.totals.add(&totals);
            if !model.is_empty() {
                models.push((model, totals.total(true)));
            }
        }
        let mut out: Vec<SessionTotals> = by_key
            .into_values()
            .map(|(mut s, mut models)| {
                models.sort_by(|a, b| b.1.cmp(&a.1));
                s.models = models.into_iter().map(|m| m.0).collect();
                s
            })
            .collect();
        out.sort_by(|a, b| b.last_ms.cmp(&a.last_ms));
        Ok(out)
    }

    /// Per-project totals since `from_ms`, most tokens first.
    pub fn projects(&self, from_ms: i64, agents: &[String]) -> Result<Vec<ProjectTotals>> {
        let mut by_project: HashMap<Option<String>, ProjectTotals> = HashMap::new();
        for s in self.sessions(from_ms, agents)? {
            let p = by_project.entry(s.project.clone()).or_insert_with(|| ProjectTotals {
                project: s.project.clone(),
                agents: vec![],
                sessions: 0,
                last_ms: 0,
                totals: Totals::default(),
                cost: None,
            });
            if !p.agents.contains(&s.agent) {
                p.agents.push(s.agent.clone());
            }
            p.sessions += 1;
            p.last_ms = p.last_ms.max(s.last_ms);
            p.totals.add(&s.totals);
            add_cost(&mut p.cost, s.cost);
        }
        let mut out: Vec<ProjectTotals> = by_project.into_values().collect();
        out.sort_by(|a, b| b.totals.total(true).cmp(&a.totals.total(true)));
        Ok(out)
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
            session: Some(format!("s-{agent}")),
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

    #[test]
    fn sessions_and_projects_fold_models() {
        let mut s = Store::in_memory().unwrap();
        let st = FileState { size: 0, mtime: 0, offset: 0 };
        let mut a = ev("a", "claude-code", 1, 1_000_000);
        a.model = Some("claude-opus-5-5".into());
        a.project = Some("tally".into());
        let mut b = ev("b", "claude-code", 0, 500_000);
        b.model = Some("claude-haiku-5-5".into());
        s.ingest("f", st, &[a, b, ev("c", "codex", 0, 5)]).unwrap();
        let sessions = s.sessions(0, &agents()).unwrap();
        assert_eq!(sessions.len(), 2);
        let claude = sessions.iter().find(|x| x.agent == "claude-code").unwrap();
        assert_eq!(claude.project.as_deref(), Some("tally"));
        assert_eq!(claude.totals.input, 1_500_000);
        assert_eq!(claude.models[0], "claude-opus-5-5");
        assert!(claude.cost.unwrap() > 4.0);
        assert!(sessions.iter().find(|x| x.agent == "codex").unwrap().cost.is_none());
        let projects = s.projects(0, &agents()).unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].project.as_deref(), Some("tally"));
        assert_eq!(projects[0].sessions, 1);
    }

    #[test]
    fn migration_adds_session_and_forces_rescan() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE events(id TEXT NOT NULL, agent TEXT NOT NULL, model TEXT, ts INTEGER NOT NULL,
               input INTEGER NOT NULL, output INTEGER NOT NULL, cache_read INTEGER NOT NULL,
               cache_write INTEGER NOT NULL, project TEXT, PRIMARY KEY(agent, id));
             CREATE TABLE file_state(path TEXT PRIMARY KEY, size INTEGER NOT NULL, mtime INTEGER NOT NULL, offset INTEGER NOT NULL);
             INSERT INTO file_state VALUES ('f', 1, 1, 1);",
        )
        .unwrap();
        let mut s = Store::init(conn).unwrap();
        assert_eq!(s.file_state("f").unwrap(), None);
        let st = FileState { size: 0, mtime: 0, offset: 0 };
        assert_eq!(s.ingest("f", st, &[ev("x", "codex", 0, 1)]).unwrap(), 1);
    }
}
