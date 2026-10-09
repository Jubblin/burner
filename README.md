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
