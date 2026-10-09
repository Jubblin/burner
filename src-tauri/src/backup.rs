use crate::writer::{elevated, needs_elevation};
use crate::{Jobs, Progress};
use flate2::{write::GzEncoder, Compression};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, State};

/// Read `src` fully into `out` (gzip if `compress`), calling `progress(done,total)` per chunk.
pub fn copy_image(
    src: &str,
    out: &str,
    compress: bool,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let mut input = File::open(src).map_err(|e| format!("open {src}: {e}"))?;
    let total = input.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    input.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let file = File::create(out).map_err(|e| format!("create {out}: {e}"))?;
    let mut sink: Box<dyn Write> = if compress {
        Box::new(GzEncoder::new(file, Compression::default()))
    } else {
        Box::new(file)
    };
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    let res = (|| {
        while done < total {
            if cancel.load(Ordering::SeqCst) {
                return Err("cancelled".to_string());
            }
            let n = input.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            sink.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            done += n as u64;
            progress(done, total);
        }
        sink.flush().map_err(|e| e.to_string())
    })();
    drop(sink); // finishes the gzip trailer
    if res.is_err() {
        let _ = std::fs::remove_file(out);
    }
    res
}

/// Read `device_path` into `output_path`, optionally gzip-compressed. Emits "progress".
#[tauri::command]
pub async fn backup_drive(
    app: AppHandle,
    jobs: State<'_, Jobs>,
    device_path: String,
    output_path: String,
    compress: bool,
) -> Result<(), String> {
    if device_path.is_empty() || output_path.is_empty() {
        return Err("device and output path are required".into());
    }
    jobs.cancel.store(false, Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move || {
        let jobs = app.state::<Jobs>();
        let emit = |p: Progress| {
            let _ = app.emit("progress", p);
        };
        if needs_elevation(&device_path, false)? {
            // create the output as the user so the root helper (which truncates it) leaves ownership alone
            File::create(&output_path).map_err(|e| format!("create {output_path}: {e}"))?;
            let c = if compress { "1" } else { "0" };
            let r = elevated("--read-helper", &[&device_path, &output_path, c], &jobs.cancel, &emit);
            if r.is_err() {
                let _ = std::fs::remove_file(&output_path);
            }
            return r;
        }
        copy_image(&device_path, &output_path, compress, &jobs.cancel, |done, total| {
            let _ = app.emit("progress", Progress { phase: "backup".into(), done, total });
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;

    #[test]
    fn needs_elevation_checks_open() {
        let f = std::env::temp_dir().join(format!("burner-ne-{}", std::process::id()));
        std::fs::write(&f, b"x").unwrap();
        assert_eq!(needs_elevation(f.to_str().unwrap(), true), Ok(false));
        std::fs::remove_file(&f).unwrap();
        assert!(needs_elevation(f.to_str().unwrap(), false).is_err());
    }

    #[test]
    fn backup_matches_source() {
        let dir = std::env::temp_dir().join(format!("burner-bk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.img");
        let data: Vec<u8> = (0..3_000_000u32).map(|i| (i * 31 % 251) as u8).collect();
        std::fs::write(&src, &data).unwrap();
        let cancel = AtomicBool::new(false);
        let (raw, gz) = (dir.join("o.img"), dir.join("o.img.gz"));
        let mut last = (0, 0);
        copy_image(src.to_str().unwrap(), raw.to_str().unwrap(), false, &cancel, |d, t| last = (d, t)).unwrap();
        assert_eq!(std::fs::read(&raw).unwrap(), data);
        assert_eq!(last, (3_000_000, 3_000_000));
        copy_image(src.to_str().unwrap(), gz.to_str().unwrap(), true, &cancel, |_, _| {}).unwrap();
        let mut out = Vec::new();
        GzDecoder::new(File::open(&gz).unwrap()).read_to_end(&mut out).unwrap();
        assert_eq!(out, data);
        cancel.store(true, Ordering::SeqCst);
        assert!(copy_image(src.to_str().unwrap(), raw.to_str().unwrap(), false, &cancel, |_, _| {}).is_err());
        assert!(!raw.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
