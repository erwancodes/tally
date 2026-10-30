//! Checks GitHub Releases for a newer signed build and installs it on request.

use parking_lot::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Version of the update found by the last check, if any.
#[derive(Default)]
pub struct Pending(pub Mutex<Option<String>>);

async fn check(app: &AppHandle) -> Option<String> {
    let update = app.updater().ok()?.check().await.ok()??;
    Some(update.version)
}

/// Checks shortly after launch, then every 6 hours. Emits `update-available`.
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        loop {
            if let Some(version) = tauri::async_runtime::block_on(check(&app)) {
                *app.state::<Pending>().0.lock() = Some(version.clone());
                let _ = app.emit("update-available", version);
            }
            std::thread::sleep(Duration::from_secs(6 * 3600));
        }
    });
}

#[tauri::command]
pub fn get_update(pending: tauri::State<'_, Pending>) -> Option<String> {
    pending.0.lock().clone()
}

/// Downloads, installs and restarts. The installer keeps `%APPDATA%\Tally`.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    let Some(update) = update else { return Ok(()) };
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    app.restart();
}
