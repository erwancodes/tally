mod commands;
mod format;
mod providers;
mod settings;
mod store;
mod tray;
mod watcher;

use parking_lot::Mutex;
use providers::Provider;
use settings::Settings;
use std::path::PathBuf;
use std::sync::Arc;
use store::Store;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_window_state::StateFlags;

pub struct AppState {
    pub store: Mutex<Store>,
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    pub providers: Vec<Box<dyn Provider>>,
    pub scanning: Mutex<bool>,
    /// Bumped when watched folders change; the watcher thread rebuilds itself.
    pub watch_generation: Mutex<u64>,
}

fn data_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("Tally")
}

/// Refreshes the tray tooltip and tells the front to reload.
pub fn notify_updated(app: &AppHandle, state: &AppState) {
    let agents: Vec<String> = {
        let s = state.settings.lock();
        state.providers.iter().filter(|p| !s.provider(p.id()).disabled).map(|p| p.id().to_string()).collect()
    };
    let include_cache = state.settings.lock().include_cache;
    if let Ok(total) = state.store.lock().today_total(&agents, include_cache) {
        if let Some(tray) = app.tray_by_id(tray::TRAY_ID) {
            let _ = tray.set_tooltip(Some(format!("Aujourd'hui : {} tokens", format::compact(total))));
        }
    }
    let _ = app.emit("usage-updated", ());
}

pub fn full_rescan(app: AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        if let Err(e) = state.store.lock().reset() {
            eprintln!("tally: reset failed: {e}");
        }
        watcher::scan_all(&app, &state);
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let dir = data_dir();
    let store = Store::open(&dir.join("tally.db")).expect("open database");
    let settings_path = dir.join("settings.json");
    let state = Arc::new(AppState {
        store: Mutex::new(store),
        settings: Mutex::new(Settings::load(&settings_path)),
        settings_path,
        providers: providers::registry(),
        scanning: Mutex::new(false),
        watch_generation: Mutex::new(0),
    });
    let started_hidden = std::env::args().any(|a| a == "--autostart");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_main(app)))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(
            // Size and position only: visibility is decided by `--autostart`.
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all() - StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_summary,
            commands::get_daily,
            commands::get_overview,
            commands::get_models,
            commands::get_agents,
            commands::refresh,
            commands::get_settings,
            commands::set_settings,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            tray::build(&handle, state.clone())?;
            if let Some(w) = app.get_webview_window("main") {
                #[cfg(target_os = "windows")]
                let _ = window_vibrancy::apply_mica(&w, Some(true));
                if !started_hidden {
                    let _ = w.show();
                }
            }
            watcher::spawn(handle, state.clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing hides to the tray; "Quitter" in the menu really exits.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tally");
}
