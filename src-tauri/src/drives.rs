use serde::Serialize;
use serde_json::Value;
use std::process::Command;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Drive {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub bus: String,
    pub path: String,
    pub removable: bool,
    pub mountpoints: Vec<String>,
}

/// Raw disk plus the facts the safety filter needs.
#[derive(Clone, Debug)]
struct Raw {
    drive: Drive,
    /// Holds the running OS / boot partition.
    system: bool,
}

/// Safety filter. System disks are NEVER returned; by default only removable/external disks.
fn filter(raws: Vec<Raw>, show_all: bool) -> Vec<Drive> {
    raws.into_iter()
        .filter(|r| !r.system && (show_all || r.drive.removable))
        .map(|r| r.drive)
        .collect()
}

#[tauri::command]
pub fn list_drives(show_all: Option<bool>) -> Result<Vec<Drive>, String> {
    Ok(filter(enumerate()?, show_all.unwrap_or(false)))
}

fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd).args(args).output().map_err(|e| format!("{cmd}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{cmd} failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

// ---------- Linux: lsblk --json ----------

#[allow(dead_code)]
fn parse_lsblk(json: &str) -> Result<Vec<Raw>, String> {
    fn mounts(v: &Value, out: &mut Vec<String>) {
        if let Some(a) = v["mountpoints"].as_array() {
            out.extend(a.iter().filter_map(|m| m.as_str().map(String::from)));
        }
        for c in v["children"].as_array().into_iter().flatten() {
            mounts(c, out);
        }
    }
    let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut res = vec![];
    for d in v["blockdevices"].as_array().into_iter().flatten() {
        if d["type"] != "disk" {
            continue;
        }
        let path = d["path"].as_str().unwrap_or_default().to_string();
        let mut mountpoints = vec![];
        mounts(d, &mut mountpoints);
        // ponytail: LVM/RAID/crypt stacks show up as children, so this covers them; swap files not considered.
        let system = mountpoints.iter().any(|m| m == "/" || m == "/boot" || m.starts_with("/boot/") || m == "/usr");
        let flag = |k: &str| d[k].as_bool().unwrap_or(false) || d[k] == "1";
        let tran = d["tran"].as_str().unwrap_or("").to_uppercase();
        res.push(Raw {
            system,
            drive: Drive {
                id: path.clone(),
                name: d["model"].as_str().map(str::trim).filter(|s| !s.is_empty()).unwrap_or(&path).to_string(),
                size_bytes: d["size"].as_u64().or_else(|| d["size"].as_str()?.parse().ok()).unwrap_or(0),
                bus: if tran.is_empty() { "Unknown".into() } else { tran.clone() },
                path,
                removable: flag("rm") || flag("hotplug") || tran == "USB",
                mountpoints,
            },
        });
    }
    Ok(res)
}

// ---------- macOS: diskutil (text; it has no JSON output) ----------

/// Value of "Key:   value" in `diskutil info` output.
#[allow(dead_code)]
fn field<'a>(info: &'a str, key: &str) -> Option<&'a str> {
    info.lines().find_map(|l| l.trim().strip_prefix(key)?.strip_prefix(':')).map(str::trim)
}

/// "disk3s1s1" -> "disk3"
#[allow(dead_code)]
fn whole(id: &str) -> String {
    let rest = id.trim_start_matches("disk");
    format!("disk{}", rest.split('s').next().unwrap_or(""))
}

#[allow(dead_code)]
fn parse_diskutil_info(info: &str, boot: &[String]) -> Option<Raw> {
    let id = field(info, "Device Identifier")?.to_string();
    if field(info, "Whole") != Some("Yes") || field(info, "Virtual") == Some("Yes") {
        return None;
    }
    let internal = field(info, "Device Location") == Some("Internal") || field(info, "Internal") == Some("Yes");
    let removable = !internal || field(info, "Removable Media").is_some_and(|v| v == "Removable" || v == "Yes");
    let size = field(info, "Disk Size")
        .and_then(|s| s.split('(').nth(1)?.split_whitespace().next()?.parse().ok())
        .unwrap_or(0);
    Some(Raw {
        system: boot.contains(&id) || (internal && !removable),
        drive: Drive {
            name: field(info, "Device / Media Name").unwrap_or(&id).to_string(),
            size_bytes: size,
            bus: field(info, "Protocol").unwrap_or("Unknown").to_string(),
            path: format!("/dev/{id}"),
            mountpoints: vec![],
            removable,
            id,
        },
    })
}

#[cfg(target_os = "macos")]
fn enumerate() -> Result<Vec<Raw>, String> {
    // Disks backing "/" (APFS container -> physical store, or plain whole disk).
    let root = run("diskutil", &["info", "/"])?;
    let mut boot = vec![];
    for k in ["Part of Whole", "APFS Physical Store"] {
        if let Some(v) = field(&root, k) {
            boot.push(whole(v));
        }
    }
    let list = run("diskutil", &["list"])?;
    let mut res = vec![];
    for l in list.lines().filter(|l| l.starts_with("/dev/disk") && !l.contains("synthesized")) {
        let dev = l.split_whitespace().next().unwrap_or("");
        if let Ok(info) = run("diskutil", &["info", dev]) {
            res.extend(parse_diskutil_info(&info, &boot));
        }
    }
    Ok(res)
}

// ---------- Windows: PowerShell Get-Disk ----------

#[allow(dead_code)]
fn parse_get_disk(json: &str) -> Result<Vec<Raw>, String> {
    let mut v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if !v.is_array() {
        v = Value::Array(vec![v]); // ConvertTo-Json emits a bare object for one disk
    }
    Ok(v.as_array().unwrap().iter().map(|d| {
        let n = d["Number"].as_u64().unwrap_or(0);
        let bus = d["BusType"].as_str().map(String::from).unwrap_or_else(|| d["BusType"].to_string());
        let letters = match &d["Letters"] {
            Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(|s| format!("{s}:\\"))).collect(),
            Value::String(s) => vec![format!("{s}:\\")],
            _ => vec![],
        };
        Raw {
            system: d["IsBoot"].as_bool().unwrap_or(false) || d["IsSystem"].as_bool().unwrap_or(false),
            drive: Drive {
                id: n.to_string(),
                name: d["FriendlyName"].as_str().unwrap_or("Disk").trim().to_string(),
                size_bytes: d["Size"].as_u64().unwrap_or(0),
                removable: matches!(bus.as_str(), "USB" | "SD" | "MMC" | "7"),
                bus,
                path: format!("\\\\.\\PhysicalDrive{n}"),
                mountpoints: letters,
            },
        }
    }).collect())
}

#[cfg(target_os = "windows")]
fn enumerate() -> Result<Vec<Raw>, String> {
    let ps = "Get-Disk | ForEach-Object { $n=$_.Number; [pscustomobject]@{Number=$n;FriendlyName=$_.FriendlyName;Size=$_.Size;BusType=[string]$_.BusType;IsBoot=$_.IsBoot;IsSystem=$_.IsSystem;Letters=@(Get-Partition -DiskNumber $n -ErrorAction SilentlyContinue | Where-Object DriveLetter | ForEach-Object {[string]$_.DriveLetter})} } | ConvertTo-Json -Compress";
    parse_get_disk(&run("powershell", &["-NoProfile", "-Command", ps])?)
}

#[cfg(target_os = "linux")]
fn enumerate() -> Result<Vec<Raw>, String> {
    parse_lsblk(&run("lsblk", &["--json", "-b", "-o", "NAME,PATH,SIZE,RM,HOTPLUG,TRAN,MODEL,TYPE,MOUNTPOINTS"])?)
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn enumerate() -> Result<Vec<Raw>, String> {
    Err("unsupported OS".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LSBLK: &str = r#"{"blockdevices":[
 {"name":"nvme0n1","path":"/dev/nvme0n1","size":512110190592,"rm":false,"hotplug":false,"tran":"nvme","model":"Samsung SSD","type":"disk","mountpoints":[null],
  "children":[{"name":"nvme0n1p1","type":"part","mountpoints":["/boot/efi"]},{"name":"nvme0n1p2","type":"part","mountpoints":["/"]}]},
 {"name":"sda","path":"/dev/sda","size":31914983424,"rm":true,"hotplug":true,"tran":"usb","model":"DT 100 G3 ","type":"disk","mountpoints":[null],
  "children":[{"name":"sda1","type":"part","mountpoints":["/media/u/KEY"]}]},
 {"name":"sdb","path":"/dev/sdb","size":1000204886016,"rm":false,"hotplug":false,"tran":"sata","model":"Data HDD","type":"disk","mountpoints":[null]},
 {"name":"loop0","path":"/dev/loop0","size":1,"type":"loop","mountpoints":["/snap/core"]}]}"#;

    #[test]
    fn linux_default_shows_only_usb() {
        let d = filter(parse_lsblk(LSBLK).unwrap(), false);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].path, "/dev/sda");
        assert_eq!(d[0].name, "DT 100 G3");
        assert_eq!(d[0].mountpoints, vec!["/media/u/KEY"]);
    }

    #[test]
    fn linux_show_all_still_hides_system_disk() {
        let paths: Vec<_> = filter(parse_lsblk(LSBLK).unwrap(), true).into_iter().map(|d| d.path).collect();
        assert_eq!(paths, ["/dev/sda", "/dev/sdb"]);
    }

    #[test]
    fn windows_excludes_boot_disk() {
        let j = r#"[{"Number":0,"FriendlyName":"NVMe","Size":512110190592,"BusType":"NVMe","IsBoot":true,"IsSystem":true,"Letters":["C"]},
                    {"Number":1,"FriendlyName":"USB Stick","Size":32000000000,"BusType":"USB","IsBoot":false,"IsSystem":false,"Letters":"E"}]"#;
        let d = filter(parse_get_disk(j).unwrap(), true);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].path, "\\\\.\\PhysicalDrive1");
        assert_eq!(d[0].mountpoints, vec!["E:\\"]);
    }

    #[test]
    fn macos_excludes_boot_and_internal() {
        let usb = "   Device Identifier:         disk4\n   Device / Media Name:       Kingston DT\n   Whole:                     Yes\n   Virtual:                   No\n   Device Location:           External\n   Removable Media:           Removable\n   Protocol:                  USB\n   Disk Size:                 32.0 GB (32015679488 Bytes) (exactly 62530624 512-Byte-Units)\n";
        let internal = usb.replace("disk4", "disk0").replace("External", "Internal").replace("Removable Media:           Removable", "Removable Media:           Fixed");
        let boot = vec!["disk0".to_string()];
        let u = parse_diskutil_info(usb, &boot).unwrap();
        assert_eq!(u.drive.size_bytes, 32015679488);
        assert!(!u.system && u.drive.removable);
        assert!(parse_diskutil_info(&internal, &boot).unwrap().system);
        assert_eq!(whole("disk3s1s1"), "disk3");
    }
}
