use crate::{run_fake_job, Jobs};
use tauri::{AppHandle, State};

/// Stub: read `device_path` into `output_path`, optionally gzip-compressed.
#[tauri::command]
pub async fn backup_drive(
    app: AppHandle,
    jobs: State<'_, Jobs>,
    device_path: String,
    output_path: String,
    compress: bool,
) -> Result<(), String> {
    let _ = compress;
    if device_path.is_empty() || output_path.is_empty() {
        return Err("device and output path are required".into());
    }
    run_fake_job(app, &jobs, &["backup"], 29_800_000_000).await
}
