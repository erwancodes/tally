//! Commands exposed to the front.

use crate::settings::Settings;
use crate::store::{DayStat, ModelTotals, ProjectTotals, SessionTotals, Summary};
use crate::AppState;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, State};

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Serialize)]
pub struct AgentInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub enabled: bool,
    /// Folders actually read, and whether each exists.
    pub roots: Vec<(String, bool)>,
    pub default_roots: Vec<String>,
    pub custom_path: Option<String>,
}

#[derive(Serialize)]
pub struct Overview {
    pub summary: Summary,
    pub daily: Vec<DayStat>,
    pub include_cache: bool,
    pub scanning: bool,
}

fn enabled_agents(state: &AppState) -> Vec<String> {
    let s = state.settings.lock();
    state.providers.iter().filter(|p| !s.provider(p.id()).disabled).map(|p| p.id().to_string()).collect()
}

#[tauri::command]
pub fn get_summary(state: State<Arc<AppState>>) -> Res<Summary> {
    state.store.lock().summary(&enabled_agents(&state)).map_err(err)
}

#[tauri::command]
pub fn get_daily(days: u32, state: State<Arc<AppState>>) -> Res<Vec<DayStat>> {
    state.store.lock().daily(days.clamp(1, 366), &enabled_agents(&state)).map_err(err)
}

/// Everything the overview needs in one round trip.
#[tauri::command]
pub fn get_overview(state: State<Arc<AppState>>) -> Res<Overview> {
    let agents = enabled_agents(&state);
    let store = state.store.lock();
    Ok(Overview {
        summary: store.summary(&agents).map_err(err)?,
        daily: store.daily(30, &agents).map_err(err)?,
        include_cache: state.settings.lock().include_cache,
        scanning: *state.scanning.lock(),
    })
}

#[tauri::command]
pub fn get_models(state: State<Arc<AppState>>) -> Res<Vec<ModelTotals>> {
    state.store.lock().models().map_err(err)
}

/// Sessions active in the last `days` days, most recent first, at most `limit`.
#[tauri::command]
pub fn get_sessions(days: u32, limit: usize, state: State<Arc<AppState>>) -> Res<Vec<SessionTotals>> {
    let mut rows = state.store.lock().sessions(crate::store::days_ago_ms(days), &enabled_agents(&state)).map_err(err)?;
    rows.truncate(limit.clamp(1, 1000));
    Ok(rows)
}

#[tauri::command]
pub fn get_projects(days: u32, state: State<Arc<AppState>>) -> Res<Vec<ProjectTotals>> {
    state.store.lock().projects(crate::store::days_ago_ms(days), &enabled_agents(&state)).map_err(err)
}

#[tauri::command]
pub fn get_agents(state: State<Arc<AppState>>) -> Vec<AgentInfo> {
    let settings = state.settings.lock().clone();
    state
        .providers
        .iter()
        .map(|p| {
            let ps = settings.provider(p.id());
            AgentInfo {
                id: p.id(),
                name: p.display_name(),
                enabled: !ps.disabled,
                roots: settings
                    .roots(p.id(), p.default_paths())
                    .into_iter()
                    .map(|r| (r.to_string_lossy().into_owned(), r.is_dir()))
                    .collect(),
                default_roots: p.default_paths().iter().map(|r| r.to_string_lossy().into_owned()).collect(),
                custom_path: ps.path,
            }
        })
        .collect()
}

/// Forgets every offset and reads all files again.
#[tauri::command]
pub fn refresh(app: AppHandle, state: State<Arc<AppState>>) -> Res<()> {
    crate::full_rescan(app, state.inner().clone());
    Ok(())
}

#[tauri::command]
pub fn get_settings(state: State<Arc<AppState>>) -> Settings {
    state.settings.lock().clone()
}

#[tauri::command]
pub fn set_settings(settings: Settings, app: AppHandle, state: State<Arc<AppState>>) -> Res<()> {
    let roots_changed = {
        let mut cur = state.settings.lock();
        let changed = cur.providers != settings.providers;
        *cur = settings;
        cur.save(&state.settings_path).map_err(err)?;
        changed
    };
    if roots_changed {
        // The watcher thread rebuilds its watches and rescans.
        *state.watch_generation.lock() += 1;
    }
    crate::notify_updated(&app, &state);
    Ok(())
}
