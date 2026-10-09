#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backup;
mod catalog;
mod drives;
mod writer;

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
pub struct Jobs {
    cancel: AtomicBool,
}

#[derive(Serialize, Clone)]
pub struct Progress {
    pub phase: String,
    pub done: u64,
    pub total: u64,
}

/// Stub job runner: emits "progress" {phase, done, total} for each phase, honouring cancel_job.
pub async fn run_fake_job(app: AppHandle, jobs: &Jobs, phases: &[&str], total: u64) -> Result<(), String> {
    jobs.cancel.store(false, Ordering::SeqCst);
    for phase in phases {
        for i in 0..=20u64 {
            if jobs.cancel.load(Ordering::SeqCst) {
                return Err("cancelled".into());
            }
            let p = Progress { phase: (*phase).into(), done: total / 20 * i, total };
            app.emit("progress", p).map_err(|e| e.to_string())?;
            tauri::async_runtime::spawn_blocking(|| std::thread::sleep(Duration::from_millis(150)))
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn cancel_job(jobs: State<'_, Jobs>) {
    jobs.cancel.store(true, Ordering::SeqCst);
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Jobs::default())
        .invoke_handler(tauri::generate_handler![
            drives::list_drives,
            writer::write_image,
            backup::backup_drive,
            catalog::fetch_catalog,
            catalog::download_image,
            cancel_job,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Burner");
}
