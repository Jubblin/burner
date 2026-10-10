use crate::Jobs;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

#[derive(Clone, Copy)]
pub enum Job {
    Write,
    Backup,
    Download,
}

/// Notification text for a finished job; `None` for a user cancel (no notification).
/// `subject` is the drive name (write), file name (backup) or OS name (download).
pub fn message(job: Job, subject: &str, result: &Result<(), String>) -> Option<String> {
    let (ok, fail) = match job {
        Job::Write => ("Write finished and verified", "Write failed"),
        Job::Backup => ("Backup finished", "Backup failed"),
        Job::Download => ("Download finished", "Download failed"),
    };
    match result {
        Ok(()) => Some(format!("{ok}: {subject}")),
        Err(e) if e == "cancelled" => None,
        Err(e) if e.starts_with(crate::writer::DRIVE_REMOVED) => Some(format!(
            "{fail}: the drive was removed during the {}",
            fail.split(' ').next().unwrap_or("job").to_lowercase()
        )),
        Err(e) => Some(format!(
            "{fail}: {}",
            e.lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(120)
                .collect::<String>()
        )),
    }
}

/// Call when a job starts. Desktop grants silently; a denial or error is ignored.
pub fn on_start(app: &AppHandle) {
    if !app.state::<Jobs>().notify_off.load(Ordering::Relaxed) {
        let _ = app.notification().request_permission();
    }
}

/// Call once when a job ends. Only notifies when enabled and the window is not focused.
pub fn on_done(app: &AppHandle, job: Job, subject: &str, result: &Result<(), String>) {
    if app.state::<Jobs>().notify_off.load(Ordering::Relaxed) {
        return;
    }
    let focused = app
        .get_webview_window("main")
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false);
    if focused {
        return;
    }
    if let Some(body) = message(job, subject, result) {
        let _ = app
            .notification()
            .builder()
            .title("Burner")
            .body(body)
            .show();
    }
}

#[tauri::command]
pub fn set_notifications(jobs: tauri::State<'_, Jobs>, on: bool) {
    jobs.notify_off.store(!on, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts() {
        assert_eq!(
            message(Job::Write, "SanDisk", &Ok(())).unwrap(),
            "Write finished and verified: SanDisk"
        );
        assert_eq!(
            message(Job::Backup, "a.img", &Ok(())).unwrap(),
            "Backup finished: a.img"
        );
        assert_eq!(
            message(Job::Download, "Ubuntu", &Ok(())).unwrap(),
            "Download finished: Ubuntu"
        );
        assert_eq!(
            message(Job::Write, "x", &Err("disk full\nmore".into())).unwrap(),
            "Write failed: disk full"
        );
        assert_eq!(
            message(Job::Write, "x", &Err("drive_removed: EIO".into())).unwrap(),
            "Write failed: the drive was removed during the write"
        );
        assert!(message(Job::Backup, "x", &Err("cancelled".into())).is_none());
    }
}
