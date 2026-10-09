use crate::{run_fake_job, Jobs};
use serde::Serialize;
use tauri::{AppHandle, State};

#[derive(Serialize, Clone)]
pub struct OsEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub size_bytes: u64,
    pub url: String,
    pub sha256: String,
}

#[tauri::command]
pub fn fetch_catalog() -> Result<Vec<OsEntry>, String> {
    let e = |id: &str, name: &str, desc: &str, mb: u64| OsEntry {
        id: id.into(),
        name: name.into(),
        description: desc.into(),
        size_bytes: mb * 1_000_000,
        url: format!("https://example.invalid/{id}.iso"),
        sha256: "0".repeat(64),
    };
    Ok(vec![
        e("ubuntu-24.04.1", "Ubuntu 24.04.1 Desktop", "Friendly Linux desktop with 5 years of updates.", 5900),
        e("raspios-64", "Raspberry Pi OS (64-bit)", "Official Debian-based system for Raspberry Pi 3, 4 and 5.", 1200),
        e("debian-12-netinst", "Debian 12 Netinstall", "Small installer that fetches packages as it goes.", 650),
        e("fedora-41", "Fedora Workstation 41", "Current GNOME desktop with recent software.", 2300),
        e("tails-6.8", "Tails 6.8", "Privacy-focused system that leaves no trace on the computer.", 1400),
    ])
}

/// Stub: returns the local path of the downloaded image.
#[tauri::command]
pub async fn download_image(app: AppHandle, jobs: State<'_, Jobs>, id: String) -> Result<String, String> {
    run_fake_job(app, &jobs, &["download"], 5_900_000_000).await?;
    Ok(format!("~/Downloads/{id}.iso"))
}
