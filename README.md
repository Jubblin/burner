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

One-time setup:
1. `npm run tauri signer generate -- -w ~/.tauri/burner.key`
2. Paste the printed public key into `plugins.updater.pubkey` in `src-tauri/tauri.conf.json` and set `endpoints` to your repo's `releases/latest/download/latest.json`.
3. Add the secrets below under GitHub → Settings → Secrets → Actions. Never commit the key.

| Secret | Purpose |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Updater signature (all platforms; Linux AppImage needs only this) |
| `APPLE_CERTIFICATE` (base64 .p12 of Developer ID Application), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID` | macOS signing + notarization |
| `WINDOWS_CERTIFICATE` (base64 .pfx Authenticode cert), `WINDOWS_CERTIFICATE_PASSWORD` | Windows signing: workflow imports the pfx and passes its thumbprint to Tauri (`certificateThumbprint`, SHA-256, DigiCert timestamp) |
