use crate::{Jobs, Progress};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use tauri::{AppHandle, Emitter, Manager, State};

const EXTRA_OS: &str = include_str!("extra_os.json");
const DEFAULT_URL: &str = "https://downloads.raspberrypi.org/os_list_imagingutility_v4.json";

#[derive(Serialize, Clone)]
pub struct OsEntry {
    /// The download URL (unique per image); passed back to `download_image`.
    pub id: String,
    pub name: String,
    pub description: String,
    pub size_bytes: u64,
    pub url: String,
    pub sha256: String,
}

#[derive(Deserialize)]
struct RawList {
    os_list: Vec<RawItem>,
}

#[derive(Deserialize)]
struct RawItem {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    url: Option<String>,
    image_download_size: Option<u64>,
    extract_size: Option<u64>,
    image_download_sha256: Option<String>,
    extract_sha256: Option<String>,
    #[serde(default)]
    subitems: Vec<RawItem>,
}

fn flatten(items: Vec<RawItem>, out: &mut Vec<OsEntry>) {
    for i in items {
        if let Some(url) = i.url {
            out.push(OsEntry {
                id: url.clone(),
                name: i.name,
                description: i.description,
                size_bytes: i.image_download_size.or(i.extract_size).unwrap_or(0),
                url,
                // the file we save is the download, so only its hash can be checked
                sha256: i.image_download_sha256.or(i.extract_sha256).unwrap_or_default(),
            });
        }
        // ponytail: `subitems_url` (lazily loaded sub-lists) is ignored
        flatten(i.subitems, out);
    }
}

pub fn parse_catalog(json: &str) -> Result<Vec<OsEntry>, String> {
    let list: RawList = serde_json::from_str(json).map_err(|e| format!("bad catalog: {e}"))?;
    let mut out = Vec::new();
    flatten(list.os_list, &mut out);
    Ok(out)
}

/// Catalog URL: first line of `<config dir>/catalog_url.txt`, else $BURNER_CATALOG_URL, else the Raspberry Pi list.
fn catalog_url(app: &AppHandle) -> String {
    app.path()
        .app_config_dir()
        .ok()
        .and_then(|d| std::fs::read_to_string(d.join("catalog_url.txt")).ok())
        .and_then(|s| s.lines().next().map(|l| l.trim().to_string()))
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("BURNER_CATALOG_URL").ok())
        .unwrap_or_else(|| DEFAULT_URL.into())
}

#[tauri::command]
pub async fn fetch_catalog(app: AppHandle) -> Result<Vec<OsEntry>, String> {
    let body = reqwest::get(catalog_url(&app))
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("could not load the OS list: {e}"))?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let mut out = parse_catalog(&body)?;
    // built-in extras (checksums copied from each project's official page, see extra_os.json)
    for e in parse_catalog(EXTRA_OS)? {
        if !out.iter().any(|o| o.id == e.id) {
            out.push(e);
        }
    }
    Ok(out)
}

fn sha256_file(path: &std::path::Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let (mut h, mut buf) = (Sha256::new(), vec![0u8; 1 << 20]);
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Ok if `expected` is empty or matches the file's SHA-256; otherwise deletes the file and errors.
fn verify_or_delete(path: &std::path::Path, expected: &str) -> Result<(), String> {
    if expected.is_empty() || sha256_file(path).map_err(|e| e.to_string())?.eq_ignore_ascii_case(expected) {
        return Ok(());
    }
    let _ = std::fs::remove_file(path);
    Err("checksum mismatch: the download was corrupt and has been deleted".into())
}

/// Downloads into the cache dir (resuming a `.part` file), verifies SHA-256, returns the local path.
#[tauri::command]
pub async fn download_image(app: AppHandle, jobs: State<'_, Jobs>, id: String) -> Result<String, String> {
    jobs.cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    crate::notify::on_start(&app);
    let r = download(&app, &jobs, &id).await;
    let (name, res) = match r {
        Ok((n, p)) => (n, Ok(p)),
        Err(e) => (String::new(), Err(e)),
    };
    crate::notify::on_done(&app, crate::notify::Job::Download, &name, &res.clone().map(|_| ()));
    res
}

/// Returns (OS name, local path).
async fn download(app: &AppHandle, jobs: &Jobs, id: &str) -> Result<(String, String), String> {
    let entry = fetch_catalog(app.clone())
        .await?
        .into_iter()
        .find(|e| e.id == id)
        .ok_or("that image is no longer in the OS list")?;

    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("images");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // name = hash of url + file name, so different images never collide
    let tag: String = Sha256::digest(entry.url.as_bytes()).iter().take(4).map(|b| format!("{b:02x}")).collect();
    let base = entry.url.rsplit('/').next().unwrap_or("image").split('?').next().unwrap_or("image");
    let (dest, part) = (dir.join(format!("{tag}-{base}")), dir.join(format!("{tag}-{base}.part")));

    if dest.exists() && verify_or_delete(&dest, &entry.sha256).is_ok() {
        return Ok((entry.name.clone(), dest.to_string_lossy().into()));
    }

    let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    let mut req = reqwest::Client::new().get(&entry.url);
    if have > 0 {
        req = req.header("Range", format!("bytes={have}-"));
    }
    let mut resp = req.send().await.and_then(|r| r.error_for_status()).map_err(|e| format!("download failed: {e}"))?;
    let resumed = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    let mut done = if resumed { have } else { 0 };
    let total = done + resp.content_length().unwrap_or(0).max(if resp.content_length().is_some() { 0 } else { entry.size_bytes.saturating_sub(done) });
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(resumed)
        .truncate(!resumed)
        .open(&part)
        .map_err(|e| e.to_string())?;

    let mut last = std::time::Instant::now();
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("download failed: {e}"))? {
        if jobs.cancel.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("cancelled".into()); // keep .part so the next try resumes
        }
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        done += chunk.len() as u64;
        if last.elapsed().as_millis() > 100 {
            last = std::time::Instant::now();
            let _ = app.emit("progress", Progress { phase: "download".into(), done, total });
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    let _ = app.emit("progress", Progress { phase: "download".into(), done, total: done });

    verify_or_delete(&part, &entry.sha256)?;
    std::fs::rename(&part, &dest).map_err(|e| e.to_string())?;
    Ok((entry.name.clone(), dest.to_string_lossy().into()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn checksum_mismatch_deletes_match_keeps() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("x");
        std::fs::write(&f, "abc").unwrap();
        let good = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(super::verify_or_delete(&f, good).is_ok() && f.exists());
        assert!(super::verify_or_delete(&f, "00").is_err());
        assert!(!f.exists());
    }

    #[test]
    fn extras_have_checksums() {
        let l = super::parse_catalog(super::EXTRA_OS).unwrap();
        assert_eq!(l.len(), 5);
        assert!(l.iter().all(|e| e.sha256.len() == 64 && e.size_bytes > 0));
    }

    #[test]
    fn parses_nested_fixture() {
        let l = super::parse_catalog(include_str!("../tests/os_list.json")).unwrap();
        let names: Vec<_> = l.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Raspberry Pi OS (64-bit)", "Ubuntu", "Deep"]);
        assert_eq!((l[0].size_bytes, l[0].sha256.as_str()), (1200, "bb"));
        assert_eq!(l[2].sha256, "dd");
    }
}
