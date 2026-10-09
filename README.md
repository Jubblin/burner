# Burner

Calm, precise disk image tool (Tauri 2). Design system: `DESIGN.md`; mockup: `design/mockup.html`.

## Run

```
npm install
npm run tauri dev
npm run tauri build -- --debug --bundles app   # --bundles app skips the DMG step on macOS
```

Layout: `ui/` is vanilla HTML/CSS/JS (no bundler, fonts bundled in `ui/fonts`); `src-tauri/src/` has one module per area. All backend commands are currently stubs that return sample data and fake progress.

## Commands (`invoke(name, args)`; Rust snake_case args become camelCase in JS)

| Command | Args | Returns |
|---|---|---|
| `list_drives` | none | `Drive[]` `{name, size_bytes, bus, path}` (never the system disk) |
| `write_image` | `image_path, device_path, confirm_name` | `()`; emits phases `write` then `verify` |
| `backup_drive` | `device_path, output_path, compress` | `()`; emits phase `backup` |
| `fetch_catalog` | none | `OsEntry[]` `{id, name, description, size_bytes, url, sha256}` |
| `download_image` | `id` | local path `String`; emits phase `download` |
| `cancel_job` | none | `()`; running job returns `Err("cancelled")` |

Errors are `Err(String)`. Long commands emit the Tauri event `progress` with `{phase: string, done: u64, total: u64}`.

The UI keeps Write disabled until the typed name equals the target drive name (or the "I understand" box is ticked); `confirm_name` carries that name so the backend can re-check it.

## Building

CI (`.github/workflows/ci.yml`) runs `cargo test` and `cargo clippy` on pull requests. Pushing a `v*` tag runs `release.yml`, which builds Tauri bundles for Windows, macOS (universal) and Linux and uploads them as per-OS workflow artifacts.

## Releasing

Push a `v*` tag (bump `version` in `src-tauri/tauri.conf.json`, `Cargo.toml` first). `release.yml` builds all three platforms, attaches them to a GitHub Release and, when signing secrets exist, a signed `latest.json`. Without secrets (forks, PRs) it still builds, just unsigned and with no updater artifacts.

### Auto-update
On launch the app checks `latest.json`, downloads in the background, and installs only when no write/verify/backup is running. It never prompts or restarts; the new version runs on the next launch (the Windows installer may relaunch the app itself). A small "Updated to vX" note shows once afterwards.

Setup, secrets and the release checklist: see [RELEASING.md](RELEASING.md).

## Write engine (`writer.rs`)
`write_image` unmounts the target, re-checks it is not the system disk, writes in 1 MiB blocks (.img/.iso/.gz/.xz/.zip), fsyncs, then re-reads and compares SHA-256. If the device can't be opened it re-launches the app binary as `burner --write-helper …` elevated (macOS osascript, Linux pkexec, Windows PowerShell `RunAs`/UAC); progress/cancel pass through temp files. Automated tests (`cargo test`) cover temp-file targets only. **Real-device write: not yet manually tested** — needs a spare USB stick (never run against an internal disk).
