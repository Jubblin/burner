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
    crate::notify::on_start(&app);
    // ponytail: blocks one async worker for the job's duration; cancel_job is a sync command so it still runs.
    let r = run(&image_path, &device_path, &jobs.cancel, &|p| {
        let _ = app.emit("progress", p);
    });
    crate::notify::on_done(&app, crate::notify::Job::Write, &confirm_name, &r);
    r
}

fn run(image: &str, device: &str, cancel: &AtomicBool, emit: &dyn Fn(Progress)) -> Result<(), String> {
    ensure_not_system_disk(device)?;
    unmount(device)?;
    if needs_elevation(device, true)? {
        // macOS: authopen asks for the password and hands this process the open device, so the app itself writes
        #[cfg(target_os = "macos")]
        return write_file(Path::new(image), mac_open_device(device)?, Path::new(device), cancel, emit);
        #[allow(unreachable_code)]
        elevated("--write-helper", &[image, device], cancel, emit)
    } else {
        write_to(Path::new(image), Path::new(device), cancel, emit)
    }
}

/// Can `path` be opened directly (read, plus write if `write`)? `Ok(true)` = permission denied, so the
/// caller must re-run the work through `elevated`. Shared by the write and backup paths.
pub fn needs_elevation(path: &str, write: bool) -> Result<bool, String> {
    match OpenOptions::new().read(true).write(write).open(path) {
        Ok(_) => Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Ok(true),
        Err(e) => Err(format!("cannot open {path}: {e}")),
    }
}

/// Stable error code the UI keys on; the job error string is `drive_removed: <OS detail>`.
pub const DRIVE_REMOVED: &str = "drive_removed";

/// Turn an I/O failure on `device` into the job error string. If the OS error means the device went
/// away it becomes `drive_removed: ..`; ambiguous errors only do so when re-enumerating drives no
/// longer lists `device`. Shared by the write, verify and backup paths.
pub fn classify_io(e: &std::io::Error, device: &Path) -> String {
    classify_with(e, device, || crate::drives::list_drives(Some(true)).ok().map(|v| v.into_iter().map(|d| d.path).collect()))
}

fn classify_with(e: &std::io::Error, device: &Path, listed: impl FnOnce() -> Option<Vec<String>>) -> String {
    // ENXIO/ENODEV; ERROR_NO_SUCH_DEVICE/ERROR_DEVICE_NOT_CONNECTED: the device is gone by definition
    let gone = cfg!(unix) && matches!(e.raw_os_error(), Some(6 | 19)) || cfg!(windows) && matches!(e.raw_os_error(), Some(433 | 1167));
    // EIO; NotFound after open; ERROR_FILE_NOT_FOUND/ERROR_INVALID_HANDLE: only if the drive list agrees
    let maybe = e.kind() == std::io::ErrorKind::NotFound
        || cfg!(unix) && e.raw_os_error() == Some(5)
        || cfg!(windows) && matches!(e.raw_os_error(), Some(2 | 6));
    let removed = gone
        || maybe && {
            // macOS raw nodes (/dev/rdiskN) are listed as /dev/diskN
            let want = device.to_string_lossy().replace("/dev/rdisk", "/dev/disk");
            listed().is_some_and(|l| !l.contains(&want))
        };
    if removed { format!("{DRIVE_REMOVED}: {e}") } else { e.to_string() }
}

/// Core engine: stream (decompressed) image to `target`, flush, then verify by SHA-256 read-back.
pub fn write_to(image: &Path, target: &Path, cancel: &AtomicBool, emit: &dyn Fn(Progress)) -> Result<(), String> {
    let e = |err: std::io::Error| classify_io(&err, target);
    // never create(true) on a device path: a vanished node must not become a regular file under /dev
    let dev = target.starts_with("/dev/") || target.to_string_lossy().starts_with(r"\\.\");
    let dst = OpenOptions::new().read(true).write(true).create(!dev).truncate(false).open(target).map_err(e)?;
    write_file(image, dst, target, cancel, emit)
}

/// Write + verify on an already-open target (`target` is only used to classify errors). Verification
/// reads back through the same handle.
fn write_file(image: &Path, mut dst: File, target: &Path, cancel: &AtomicBool, emit: &dyn Fn(Progress)) -> Result<(), String> {
    let e = |err: std::io::Error| classify_io(&err, target);
    let pos = Arc::new(AtomicU64::new(0));
    let (mut src, total) = open_source(image, pos.clone())?;
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
    let want = hasher.finalize();

    let mut rd = dst;
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

/// Ask macOS (authopen, system password prompt) to open the raw device read/write and pass the fd back
/// over a socketpair, so no root process is needed.
#[cfg(target_os = "macos")]
fn mac_open_device(device: &str) -> Result<File, String> {
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;
    let raw = device.replacen("/dev/disk", "/dev/rdisk", 1);
    let (ours, theirs) = UnixStream::pair().map_err(|e| e.to_string())?;
    let mut child = Command::new("/usr/libexec/authopen")
        .args(["-stdoutpipe", "-o", "2", &raw]) // 2 = O_RDWR
        .stdout(Stdio::from(std::os::fd::OwnedFd::from(theirs)))
        .spawn()
        .map_err(|e| format!("cannot start authopen: {e}"))?;
    let fd = recv_fd(&ours);
    let st = child.wait().map_err(|e| e.to_string())?;
    match fd {
        Some(f) if st.success() => Ok(f),
        _ => Err("authorization was denied or the drive could not be opened".into()),
    }
}

/// Receive one file descriptor sent with SCM_RIGHTS.
#[cfg(target_os = "macos")]
fn recv_fd(sock: &std::os::unix::net::UnixStream) -> Option<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    // SAFETY: plain recvmsg into zeroed, correctly sized buffers; the received fd is owned by the returned File.
    unsafe {
        let mut byte = [0u8; 1];
        let mut iov = libc::iovec { iov_base: byte.as_mut_ptr().cast(), iov_len: 1 };
        let mut ctl = [0u64; 8]; // 64 bytes, 8-aligned, >= CMSG_SPACE(sizeof(int))
        let mut msg: libc::msghdr = std::mem::zeroed();
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = ctl.as_mut_ptr().cast();
        msg.msg_controllen = std::mem::size_of_val(&ctl) as _;
        if libc::recvmsg(sock.as_raw_fd(), &mut msg, 0) < 0 {
            return None;
        }
        let c = libc::CMSG_FIRSTHDR(&msg);
        if c.is_null() || (*c).cmsg_level != libc::SOL_SOCKET || (*c).cmsg_type != libc::SCM_RIGHTS {
            return None;
        }
        Some(File::from_raw_fd(std::ptr::read_unaligned(libc::CMSG_DATA(c).cast::<i32>())))
    }
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

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn sh_line(exe: &str, args: &[&str]) -> String {
    std::iter::once(exe).chain(args.iter().copied()).map(sh_quote).collect::<Vec<_>>().join(" ")
}

/// PowerShell single-quoted literal: only `'` is special, and it is doubled.
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn ps_script(exe: &str, args: &[&str]) -> String {
    let list = args.iter().map(|a| ps_quote(a)).collect::<Vec<_>>().join(",");
    format!("Start-Process -Verb RunAs -Wait -FilePath {} -ArgumentList {list}", ps_quote(exe))
}

/// Re-launch this exe as root with `flag extra... progress cancel`; relays the helper's progress lines.
pub fn elevated(flag: &str, extra: &[&str], cancel: &AtomicBool, emit: &dyn Fn(Progress)) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?.to_string_lossy().into_owned();
    let (prog, cancel_f) = (temp("progress"), temp("cancel"));
    let _ = std::fs::remove_file(&cancel_f);
    std::fs::write(&prog, "").map_err(|e| e.to_string())?;
    let (prog_s, cancel_s) = (prog.to_string_lossy().into_owned(), cancel_f.to_string_lossy().into_owned());
    let mut raw: Vec<&str> = vec![flag];
    raw.extend(extra);
    raw.extend([prog_s.as_str(), cancel_s.as_str()]);
    let mut cmd = if cfg!(target_os = "macos") {
        let sh = sh_line(&exe, &raw).replace('\\', "\\\\").replace('"', "\\\"");
        let mut c = Command::new("osascript");
        c.args(["-e", &format!("do shell script \"{sh}\" with administrator privileges")]);
        c
    } else if cfg!(windows) {
        let mut c = Command::new("powershell");
        c.args(["-Command", &ps_script(&exe, &raw)]);
        c
    } else {
        let mut c = Command::new("pkexec");
        c.arg(&exe).args(&raw); // separate argv entries, no shell
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
            if let [ph, d, t] = f.as_slice() {
                emit(Progress { phase: (*ph).into(), done: d.parse().unwrap_or(0), total: t.parse().unwrap_or(0) })
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
    let flag = a.get(1).map(String::as_str).unwrap_or("");
    if !matches!(flag, "--write-helper" | "--read-helper") || a.len() < 6 {
        return false;
    }
    let (prog, cancel_f) = (a[a.len() - 2].clone(), a[a.len() - 1].clone());
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
    let r = if flag == "--write-helper" {
        ensure_not_system_disk(&a[3]).and_then(|_| write_to(Path::new(&a[2]), Path::new(&a[3]), &cancel, &emit))
    } else {
        crate::backup::copy_image(&a[2], &a[3], a[4] == "1", &cancel, |done, total| {
            emit(Progress { phase: "backup".into(), done, total })
        })
    };
    if let Err(m) = &r {
        log(format!("ERR {m}"));
    }
    std::process::exit(if r.is_ok() { 0 } else { 1 });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn recv_fd_gets_a_descriptor_sent_with_scm_rights() {
        use std::os::fd::AsRawFd;
        let mut f = tempfile::tempfile().unwrap();
        f.write_all(b"hi").unwrap();
        let (a, b) = std::os::unix::net::UnixStream::pair().unwrap();
        unsafe {
            let mut byte = [0u8; 1];
            let mut iov = libc::iovec { iov_base: byte.as_mut_ptr().cast(), iov_len: 1 };
            let mut ctl = [0u64; 8];
            let mut msg: libc::msghdr = std::mem::zeroed();
            msg.msg_iov = &mut iov;
            msg.msg_iovlen = 1;
            msg.msg_control = ctl.as_mut_ptr().cast();
            msg.msg_controllen = libc::CMSG_SPACE(4) as _;
            let c = libc::CMSG_FIRSTHDR(&msg);
            (*c).cmsg_level = libc::SOL_SOCKET;
            (*c).cmsg_type = libc::SCM_RIGHTS;
            (*c).cmsg_len = libc::CMSG_LEN(4) as _;
            std::ptr::write_unaligned(libc::CMSG_DATA(c).cast::<i32>(), f.as_raw_fd());
            assert!(libc::sendmsg(a.as_raw_fd(), &msg, 0) >= 0);
        }
        let mut got = recv_fd(&b).unwrap();
        got.seek(SeekFrom::Start(0)).unwrap();
        let mut s = String::new();
        got.read_to_string(&mut s).unwrap();
        assert_eq!(s, "hi");
    }

    #[test]
    fn quoting_handles_spaces_and_single_quotes() {
        assert_eq!(sh_line("/a b/it's", &["--x", "/d/o'k"]), r#"'/a b/it'\''s' '--x' '/d/o'\''k'"#);
        assert_eq!(
            ps_script(r"C:\Pro g\b'r.exe", &["--w", r"\\.\PhysicalDrive'1", "C:\\t m\\p"]),
            r"Start-Process -Verb RunAs -Wait -FilePath 'C:\Pro g\b''r.exe' -ArgumentList '--w','\\.\PhysicalDrive''1','C:\t m\p'"
        );
    }

    #[test]
    fn missing_device_path_errors_and_creates_nothing() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("i.img");
        std::fs::write(&src, vec![1u8; 4096]).unwrap();
        let dev = Path::new("/dev/burner-no-such-device");
        assert!(go(&src, dev).is_err());
        assert!(!dev.exists());
    }

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
    fn classifier_maps_gone_devices_and_leaves_others() {
        let none = || -> Option<Vec<String>> { panic!("no enumeration for definite or unrelated errors") };
        let dev = Path::new("/dev/sdx");
        #[cfg(unix)]
        for code in [6, 19] {
            let m = classify_with(&std::io::Error::from_raw_os_error(code), dev, none);
            assert!(m.starts_with("drive_removed: "), "{m}");
        }
        #[cfg(windows)]
        for code in [433, 1167] {
            let m = classify_with(&std::io::Error::from_raw_os_error(code), dev, none);
            assert!(m.starts_with("drive_removed: "), "{m}");
        }
        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert_eq!(classify_with(&denied, dev, none), denied.to_string());
        let full = std::io::Error::other("no space left");
        assert_eq!(classify_with(&full, dev, none), "no space left");
    }

    #[test]
    fn classifier_uses_injected_drive_list_for_ambiguous_errors() {
        let nf = std::io::Error::from(std::io::ErrorKind::NotFound);
        let (dev, gone) = (Path::new("/dev/rdisk4"), || Some(vec!["/dev/disk5".to_string()]));
        assert!(classify_with(&nf, dev, gone).starts_with("drive_removed: "));
        let still = || Some(vec!["/dev/disk4".to_string()]);
        assert_eq!(classify_with(&nf, dev, still), nf.to_string());
        assert_eq!(classify_with(&nf, dev, || None), nf.to_string()); // enumeration failed: keep error
        #[cfg(unix)]
        {
            let eio = std::io::Error::from_raw_os_error(5);
            assert!(classify_with(&eio, dev, gone).starts_with("drive_removed: "));
            assert_eq!(classify_with(&eio, dev, still), eio.to_string());
        }
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
