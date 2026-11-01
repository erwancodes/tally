//! Threshold notifications. Remembers what already fired so an alert shows once
//! per period and limit, even across restarts.

use crate::store::Summary;
use crate::{format, AppState};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Period key and limit of the last alert.
type Last = Option<(String, u64)>;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Fired {
    day: Last,
    month: Last,
}

impl Fired {
    pub fn load(path: &Path) -> Fired {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    fn save(&self, path: &Path) {
        if let Ok(json) = serde_json::to_string(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

/// Records the alert and returns the limit when `total` newly passes it.
/// Changing the limit re-arms the alert.
fn cross(last: &mut Last, period: &str, limit: Option<u64>, total: u64) -> Option<u64> {
    let limit = limit.filter(|l| *l > 0 && total >= *l)?;
    if last.as_ref().is_some_and(|(p, l)| p == period && *l == limit) {
        return None;
    }
    *last = Some((period.to_string(), limit));
    Some(limit)
}

pub fn check(app: &AppHandle, state: &AppState, summary: &Summary) {
    let (daily, monthly, include_cache) = {
        let s = state.settings.lock();
        (s.alert_daily, s.alert_monthly, s.include_cache)
    };
    let now = Local::now();
    let today = summary.today.totals.total(include_cache);
    let month = summary.month.totals.total(include_cache);

    let mut fired = state.alerts.lock();
    let day_hit = cross(&mut fired.day, &now.format("%Y-%m-%d").to_string(), daily, today);
    let month_hit = cross(&mut fired.month, &now.format("%Y-%m").to_string(), monthly, month);
    if day_hit.is_some() || month_hit.is_some() {
        fired.save(&state.alerts_path);
    }
    drop(fired);

    if let Some(limit) = day_hit {
        show(app, &format!("You passed {} tokens today", format::compact(limit)), today);
    }
    if let Some(limit) = month_hit {
        show(app, &format!("You passed {} tokens this month", format::compact(limit)), month);
    }
}

fn show(app: &AppHandle, title: &str, total: u64) {
    let body = format!("{} tokens so far.", format::compact(total));
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        eprintln!("tally: notification failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_once_per_period_and_limit() {
        let mut last = None;
        assert_eq!(cross(&mut last, "2026-11-01", Some(100), 99), None);
        assert_eq!(cross(&mut last, "2026-11-01", Some(100), 100), Some(100));
        assert_eq!(cross(&mut last, "2026-11-01", Some(100), 500), None);
        assert_eq!(cross(&mut last, "2026-11-02", Some(100), 500), Some(100));
    }

    #[test]
    fn new_limit_rearms_and_none_or_zero_never_fires() {
        let mut last = None;
        assert_eq!(cross(&mut last, "d", None, 500), None);
        assert_eq!(cross(&mut last, "d", Some(0), 500), None);
        assert_eq!(cross(&mut last, "d", Some(100), 500), Some(100));
        assert_eq!(cross(&mut last, "d", Some(200), 500), Some(200));
    }
}
