use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct Drive {
    pub name: String,
    pub size_bytes: u64,
    pub bus: String,
    pub path: String,
}

/// Stub: the real implementation must never return the system disk.
#[tauri::command]
pub fn list_drives() -> Result<Vec<Drive>, String> {
    let d = |name: &str, gb: f64, bus: &str, path: &str| Drive {
        name: name.into(),
        size_bytes: (gb * 1e9) as u64,
        bus: bus.into(),
        path: path.into(),
    };
    Ok(vec![
        d("Kingston DT 32 GB", 29.8, "USB", "/dev/disk4"),
        d("SanDisk Ultra 64 GB", 58.4, "USB", "/dev/disk5"),
        d("Generic SD Reader 16 GB", 14.9, "SD card", "/dev/disk6"),
    ])
}
