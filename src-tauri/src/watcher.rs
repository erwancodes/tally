//! Scans provider folders incrementally and watches them for changes.

use crate::providers::Provider;
use crate::store::{FileState, Store};
use crate::AppState;
use anyhow::Result;
use notify::RecursiveMode;
use parking_lot::Mutex;
use notify_debouncer_mini::new_debouncer;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

#[derive(Clone, serde::Serialize)]
pub struct ScanProgress {
    pub done: usize,
    pub total: usize,
}

fn stat_one(path: &Path) -> Option<(u64, i64)> {
    let m = std::fs::metadata(path).ok()?;
    let mtime = m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_millis() as i64;
    Some((m.len(), mtime))
}

/// Size and mtime, counting a SQLite WAL next to the file: writes land there first.
fn stat(path: &Path) -> Option<(u64, i64)> {
    let (size, mtime) = stat_one(path)?;
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    Some(match stat_one(Path::new(&wal)) {
        Some((s, m)) => (size + s, mtime.max(m)),
        None => (size, mtime),
    })
}

/// Reads what is new in one file. A file that shrank, or whose mtime went back,
/// was rewritten: read it again from the start (dedup keeps totals right).
/// Parsing happens outside the lock so the UI is never blocked by a large file.
pub fn scan_file(store: &Mutex<Store>, provider: &dyn Provider, path: &Path) -> Result<usize> {
    let Some((size, mtime)) = stat(path) else { return Ok(0) };
    let key = path.to_string_lossy().to_string();
    let prev = store.lock().file_state(&key)?;
    let from = match prev {
        Some(p) if p.size == size && p.mtime == mtime => return Ok(0),
        Some(p) if size < p.offset || mtime < p.mtime => 0,
        Some(p) => p.offset,
        None => 0,
    };
    let (events, offset) = provider.parse_file(path, from)?;
    store.lock().ingest(&key, FileState { size, mtime, offset }, &events)
}

pub fn enabled_roots(state: &AppState) -> Vec<(usize, PathBuf)> {
    let settings = state.settings.lock().clone();
    state
        .providers
        .iter()
        .enumerate()
        .filter(|(_, p)| !settings.provider(p.id()).disabled)
        .flat_map(|(i, p)| settings.roots(p.id(), p.default_paths()).into_iter().map(move |r| (i, r)))
        .collect()
}

/// Full pass over every enabled folder. Emits progress, then `usage-updated`.
pub fn scan_all(app: &AppHandle, state: &AppState) {
    let mut files = Vec::new();
    for (i, root) in enabled_roots(state) {
        files.extend(state.providers[i].files(&root).into_iter().map(|f| (i, f)));
    }
    let total = files.len();
    *state.scanning.lock() = true;
    let _ = app.emit("scan-progress", ScanProgress { done: 0, total });
    for (n, (i, file)) in files.iter().enumerate() {
        let provider = state.providers[*i].as_ref();
        if let Err(e) = scan_file(&state.store, provider, file) {
            eprintln!("tally: {}: {e}", file.display());
        }
        if n % 50 == 0 {
            let _ = app.emit("scan-progress", ScanProgress { done: n, total });
        }
    }
    *state.scanning.lock() = false;
    let _ = app.emit("scan-progress", ScanProgress { done: total, total });
    crate::notify_updated(app, state);
}

/// Watches every enabled folder with a 2 s debounce and rescans only the files that changed.
pub fn spawn(app: AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        scan_all(&app, &state);
        loop {
            let generation = *state.watch_generation.lock();
            let (tx, rx) = std::sync::mpsc::channel();
            let Ok(mut debouncer) = new_debouncer(Duration::from_secs(2), tx) else { return };
            let roots = enabled_roots(&state);
            for (_, root) in &roots {
                if root.exists() {
                    let _ = debouncer.watcher().watch(root, RecursiveMode::Recursive);
                }
            }
            // Wake up regularly to notice a settings change (new folders to watch).
            loop {
                if *state.watch_generation.lock() != generation {
                    break;
                }
                let Ok(res) = rx.recv_timeout(Duration::from_secs(1)) else { continue };
                let Ok(events) = res else { continue };
                let mut changed = false;
                for ev in events {
                    if let Some((i, _)) = roots.iter().find(|(_, r)| ev.path.starts_with(r)) {
                        let provider = state.providers[*i].as_ref();
                        let Some(path) = provider.source(&ev.path) else { continue };
                        if let Ok(n) = scan_file(&state.store, provider, &path) {
                            changed |= n > 0;
                        }
                    }
                }
                if changed {
                    crate::notify_updated(&app, &state);
                }
            }
            scan_all(&app, &state);
        }
    });
}
