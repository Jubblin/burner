use crate::{Jobs, Progress};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};


const BLOCK: usize = 1 << 20;

/// `confirm_name` must equal the target drive name (UI-checked; backend only requires it present).
/// Unmounts, writes `image_path` to `device_path` in 1 MiB blocks, then re-reads and compares SHA-256.
/// Emits `progress` for phases `write` then `verify`. Re-escalates via a helper process if the device
/// can't be opened (see `helper_main`).
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
    jobs.cancel.store(false, Ordering::SeqCst);
    // ponytail: blocks one async worker for the job's duration; cancel_job is a sync command so it still runs.
    run(&image_path, &device_path, &jobs.cancel, &|p| {
        let _ = app.emit("progress", p);
    })
}

fn run(image: &str, device: &str, cancel: &AtomicBool, emit: &dyn Fn(Progress)) -> Result<(), String> {
    ensure_not_system_disk(device)?;
    unmount(device)?;
    match OpenOptions::new().read(true).write(true).open(device) {
        Ok(_) => write_to(Path::new(image), Path::new(device), cancel, emit),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => elevated(image, device, cancel, emit),
        Err(e) => Err(format!("cannot open {device}: {e}")),
    }
}

/// Core engine: stream (decompressed) image to `target`, flush, then verify by SHA-256 read-back.
pub fn write_to(image: &Path, target: &Path, cancel: &AtomicBool, emit: &dyn Fn(Progress)) -> Result<(), String> {
    let e = |e: std::io::Error| e.to_string();
    let pos = Arc::new(AtomicU64::new(0));
    let (mut src, total) = open_source(image, pos.clone())?;
    let mut dst = OpenOptions::new().write(true).create(true).open(target).map_err(e)?;
    let pad = !dst.metadata().map_err(e)?.is_file();
    let (mut hasher, mut buf, mut n) = (Sha256::new(), vec![0u8; BLOCK], 0u64);
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled".into());
        }
        let k = read_full(&mut src, &mut buf).map_err(e)?;
        if k == 0 {
            break;
        }
        hasher.update(&buf[..k]);
        let mut w = k;
        if pad && k % 512 != 0 {
            w = k.div_ceil(512) * 512;
            buf[k..w].fill(0); // raw devices need sector-aligned writes
        }
        dst.write_all(&buf[..w]).map_err(e)?;
        n += k as u64;
        emit(Progress { phase: "write".into(), done: pos.load(Ordering::Relaxed).min(total), total });
    }
    dst.flush().map_err(e)?;
    dst.sync_all().map_err(e)?;
    drop(dst);
    let want = hasher.finalize();

    let mut rd = File::open(target).map_err(e)?;
    rd.seek(SeekFrom::Start(0)).map_err(e)?;
    let (mut h, mut left) = (Sha256::new(), n);
    while left > 0 {
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled".into());
        }
        let want_n = left.min(BLOCK as u64) as usize;
        rd.read_exact(&mut buf[..want_n]).map_err(e)?;
        h.update(&buf[..want_n]);
        left -= want_n as u64;
        emit(Progress { phase: "verify".into(), done: n - left, total: n });
    }
    if h.finalize() != want {
        return Err("verification failed: data read back does not match the image".into());
    }
    Ok(())
}

fn read_full(r: &mut dyn Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut k = 0;
    while k < buf.len() {
        match r.read(&mut buf[k..])? {
            0 => break,
            m => k += m,
        }
    }
    Ok(k)
}

struct Counting<R>(R, Arc<AtomicU64>);
impl<R: Read> Read for Counting<R> {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        let n = self.0.read(b)?;
        self.1.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

/// Returns a reader of the raw image bytes plus the total that `pos` counts against
/// (compressed file size for .gz/.xz, image size otherwise).
fn open_source(p: &Path, pos: Arc<AtomicU64>) -> Result<(Box<dyn Read>, u64), String> {
    let e = |e: std::io::Error| format!("cannot read image: {e}");
    let f = File::open(p).map_err(e)?;
    let size = f.metadata().map_err(e)?.len();
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    Ok(match ext.as_str() {
        "gz" => (Box::new(flate2::read::MultiGzDecoder::new(Counting(f, pos))), size),
        "xz" => (Box::new(xz2::read::XzDecoder::new(Counting(f, pos))), size),
        "zip" => {
            let mut z = zip::ZipArchive::new(f).map_err(|x| x.to_string())?;
            let idx = (0..z.len())
                .find(|&i| z.by_index(i).map(|f| f.is_file()).unwrap_or(false))
                .ok_or("zip contains no file")?;
            let (len, mut data) = {
                let mut ent = z.by_index(idx).map_err(|x| x.to_string())?;
                let mut v = Vec::with_capacity(ent.size() as usize);
                // ponytail: zip entry is buffered in RAM (zip reader borrows the archive); stream via a thread if images get huge.
                ent.read_to_end(&mut v).map_err(|x| x.to_string())?;
                (v.len() as u64, v)
            };
            data.shrink_to_fit();
            pos.store(len, Ordering::Relaxed);
            (Box::new(std::io::Cursor::new(data)), len)
        }
        _ => (Box::new(Counting(f, pos)), size),
    })
}

fn out(cmd: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(cmd).args(args).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Whole-disk name of a device path: /dev/rdisk4s1 -> disk4, /dev/sda1 -> sda, /dev/nvme0n1p2 -> nvme0n1.
fn base_disk(dev: &str) -> String {
    let n = dev.rsplit('/').next().unwrap_or(dev);
    let n = n.strip_prefix('r').filter(|r| r.starts_with("disk")).unwrap_or(n);
    let cut = |n: &str, sep: &str| n.rfind(sep).filter(|&i| n[i + sep.len()..].chars().all(|c| c.is_ascii_digit()) && i > 0).map(|i| n[..i].to_string());
    if n.starts_with("disk") {
        return cut(n, "s").filter(|d| d.len() > 4).unwrap_or(n.into());
    }
    if n.starts_with("nvme") || n.starts_with("mmcblk") {
        return cut(n, "p").unwrap_or(n.into());
    }
    n.trim_end_matches(|c: char| c.is_ascii_digit()).into()
}

/// Called again immediately before writing; refuses the disk that holds the running system.
fn ensure_not_system_disk(device: &str) -> Result<(), String> {
    if cfg!(windows) {
        // ponytail: Windows only blocks PhysicalDrive0 (usual boot disk); query Win32_DiskDrive boot flag if needed.
        return if device.to_lowercase().ends_with("physicaldrive0") {
            Err("refusing to write to the system disk".into())
        } else {
            Ok(())
        };
    }
    let dev = base_disk(device);
    let sys = if cfg!(target_os = "macos") {
        out("diskutil", &["info", "/"]).and_then(|s| {
            s.lines().find(|l| l.contains("Part of Whole:")).map(|l| l.rsplit(':').next().unwrap().trim().to_string())
        })
    } else {
        out("findmnt", &["-no", "SOURCE", "/"])
            .and_then(|src| out("lsblk", &["-no", "pkname", src.trim()]))
            .map(|s| s.trim().to_string())
    };
    match sys {
        Some(s) if s == dev || s.is_empty() => Err("refusing to write to the system disk".into()),
        Some(_) => Ok(()),
        None => Err("cannot determine the system disk; refusing to write".into()),
    }
}

fn unmount(device: &str) -> Result<(), String> {
    let ok = if cfg!(target_os = "macos") {
        Command::new("diskutil").args(["unmountDisk", &format!("/dev/{}", base_disk(device))]).status().map(|s| s.success())
    } else if cfg!(target_os = "linux") {
        // best effort: unmount every mounted partition of this disk
        let disk = base_disk(device);
        for l in std::fs::read_to_string("/proc/mounts").unwrap_or_default().lines() {
            let src = l.split(' ').next().unwrap_or("");
            if src.starts_with("/dev/") && base_disk(src) == disk {
                let _ = Command::new("umount").arg(src).status();
            }
        }
        Ok(true)
    } else {
        Ok(true) // ponytail: Windows volumes are dismounted by the OS when \\.\PhysicalDriveN is opened exclusively
    };
    match ok {
        Ok(true) => Ok(()),
        _ => Err("could not unmount the target drive (is something using it?)".into()),
    }
}

// ---- privilege elevation: one path, the app re-launches itself as `burner --write-helper ...` ----

fn temp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("burner-{}-{name}", std::process::id()))
}

fn elevated(image: &str, device: &str, cancel: &AtomicBool, emit: &dyn Fn(Progress)) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?.to_string_lossy().into_owned();
    let (prog, cancel_f) = (temp("progress"), temp("cancel"));
    let _ = std::fs::remove_file(&cancel_f);
    std::fs::write(&prog, "").map_err(|e| e.to_string())?;
    let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let args = [q(&exe), "--write-helper".into(), q(image), q(device), q(&prog.to_string_lossy()), q(&cancel_f.to_string_lossy())];
    let mut cmd = if cfg!(target_os = "macos") {
        let sh = args.join(" ").replace('\\', "\\\\").replace('"', "\\\"");
        let mut c = Command::new("osascript");
        c.args(["-e", &format!("do shell script \"{sh}\" with administrator privileges")]);
        c
    } else if cfg!(windows) {
        let al = args[1..].iter().map(|a| a.replace('\'', "''")).collect::<Vec<_>>().join(" ");
        let mut c = Command::new("powershell");
        c.args(["-Command", &format!("Start-Process -Verb RunAs -Wait '{exe}' -ArgumentList \"{al}\"")]);
        c
    } else {
        let mut c = Command::new("pkexec");
        c.arg(&exe).args(args[1..].iter().map(|a| a.trim_matches('\'').to_string()));
        c
    };
    let mut child = cmd.spawn().map_err(|e| format!("cannot start privileged helper: {e}"))?;
    let mut seen = 0;
    let result = loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = std::fs::write(&cancel_f, "");
        }
        let text = std::fs::read_to_string(&prog).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        for l in lines.iter().skip(seen) {
            let f: Vec<&str> = l.split_whitespace().collect();
            match f.as_slice() {
                [ph, d, t] => emit(Progress { phase: (*ph).into(), done: d.parse().unwrap_or(0), total: t.parse().unwrap_or(0) }),
                _ => {}
            }
        }
        seen = lines.len();
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            let last = std::fs::read_to_string(&prog).unwrap_or_default().lines().last().unwrap_or("").to_string();
            break match (st.success(), last.strip_prefix("ERR ")) {
                (_, Some(m)) => Err(m.to_string()),
                (true, None) => Ok(()),
                _ => Err("authorization was denied or the helper failed".into()),
            };
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    let _ = std::fs::remove_file(&prog);
    let _ = std::fs::remove_file(&cancel_f);
    result
}

/// Entry for the elevated child. Returns true if this process was a helper (caller should exit).
pub fn helper_main() -> bool {
    let a: Vec<String> = std::env::args().collect();
    if a.get(1).map(String::as_str) != Some("--write-helper") || a.len() < 6 {
        return false;
    }
    let (prog, cancel_f) = (a[4].clone(), a[5].clone());
    let cancel = AtomicBool::new(false);
    let log = |s: String| {
        if let Ok(mut f) = OpenOptions::new().append(true).open(&prog) {
            let _ = writeln!(f, "{s}");
        }
    };
    let emit = |p: Progress| {
        if Path::new(&cancel_f).exists() {
            cancel.store(true, Ordering::SeqCst);
        }
        log(format!("{} {} {}", p.phase, p.done, p.total));
    };
    // re-check in the privileged process too, right before writing
    let r = ensure_not_system_disk(&a[3]).and_then(|_| write_to(Path::new(&a[2]), Path::new(&a[3]), &cancel, &emit));
    if let Err(m) = &r {
        log(format!("ERR {m}"));
    }
    std::process::exit(if r.is_ok() { 0 } else { 1 });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 31 % 251) as u8).collect()
    }
    fn go(src: &Path, dst: &Path) -> Result<(), String> {
        write_to(src, dst, &AtomicBool::new(false), &|_| {})
    }

    #[test]
    fn write_and_verify_all_formats() {
        let d = tempfile::tempdir().unwrap();
        let data = img(3 * BLOCK + 777);
        let raw = d.path().join("a.img");
        std::fs::write(&raw, &data).unwrap();
        let gz = d.path().join("a.img.gz");
        let mut g = flate2::write::GzEncoder::new(File::create(&gz).unwrap(), flate2::Compression::fast());
        g.write_all(&data).unwrap();
        g.finish().unwrap();
        let xz = d.path().join("a.img.xz");
        let mut x = xz2::write::XzEncoder::new(File::create(&xz).unwrap(), 1);
        x.write_all(&data).unwrap();
        x.finish().unwrap();
        let zp = d.path().join("a.zip");
        let mut z = zip::ZipWriter::new(File::create(&zp).unwrap());
        z.start_file("a.iso", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(&data).unwrap();
        z.finish().unwrap();
        for s in [raw, gz, xz, zp] {
            let t = d.path().join("target");
            go(&s, &t).unwrap();
            assert_eq!(std::fs::read(&t).unwrap(), data, "{s:?}");
        }
    }

    #[test]
    fn verify_detects_corruption() {
        let d = tempfile::tempdir().unwrap();
        let (src, t) = (d.path().join("a.img"), d.path().join("t"));
        std::fs::write(&src, img(2 * BLOCK)).unwrap();
        let corrupt = |p: crate::Progress| {
            if p.phase == "write" && p.done == BLOCK as u64 {
                // flip a byte in the already-written first block before verify reads it
                let mut f = OpenOptions::new().write(true).open(d.path().join("t")).unwrap();
                f.seek(SeekFrom::Start(5)).unwrap();
                f.write_all(&[0xFF, 0xFE]).unwrap();
            }
        };
        let r = write_to(&src, &t, &AtomicBool::new(false), &corrupt);
        assert!(r.unwrap_err().contains("verification failed"));
    }

    #[test]
    fn cancel_stops() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("a.img");
        std::fs::write(&src, img(BLOCK)).unwrap();
        let r = write_to(&src, &d.path().join("t"), &AtomicBool::new(true), &|_| {});
        assert_eq!(r.unwrap_err(), "cancelled");
    }

    #[test]
    fn disk_names() {
        assert_eq!(base_disk("/dev/rdisk4"), "disk4");
        assert_eq!(base_disk("/dev/disk4s1"), "disk4");
        assert_eq!(base_disk("/dev/sda1"), "sda");
        assert_eq!(base_disk("/dev/nvme0n1p2"), "nvme0n1");
        assert_eq!(base_disk("/dev/disk4"), "disk4");
    }
}
