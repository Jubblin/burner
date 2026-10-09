use crate::{run_fake_job, Jobs};
use tauri::{AppHandle, State};

/// Stub: `confirm_name` must equal the target drive name (typed or ticked in the UI).
#[tauri::command]
pub async fn write_image(
    app: AppHandle,
    jobs: State<'_, Jobs>,
    image_path: String,
    device_path: String,
    confirm_name: String,
) -> Result<(), String> {
    if image_path.is_empty() || device_path.is_empty() || confirm_name.is_empty() {
        return Err("image, device and confirmation are required".into());
    }
    run_fake_job(app, &jobs, &["write", "verify"], 5_900_000_000).await
}
