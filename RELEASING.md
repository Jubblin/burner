# Releasing Burner

Unsigned builds and PR checks work without any of this. Signing and auto-update need the items below.

## 1. One-time setup, all platforms (needed for auto-update)
- [ ] Generate the update key: `npm run tauri signer generate -- -w ~/.tauri/burner.key`. Choose a password; store the key safely; never commit it.
- [ ] Paste the printed public key into `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`), replacing `REPLACE_WITH_CONTENTS_OF_UPDATER_PUBLIC_KEY_FILE`.
- [x] Updater endpoint now points at `Jubblin/burner` (done).
- [ ] Create the GitHub repo and push the branch (the release workflow runs there).
- [ ] Add GitHub secrets (Settings, Secrets and variables, Actions): `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Linux needs only these.

## 2. macOS (signing and notarization)
- [ ] Paid Apple Developer Program account (about 99 USD a year).
- [ ] A "Developer ID Application" certificate exported as .p12 and base64-encoded.
- [ ] Secrets: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD` (app-specific password, not the login password), `APPLE_TEAM_ID`.

## 3. Windows (signing)
- [ ] Code signing certificate as .pfx, base64-encoded (slowest item: a purchase from a certificate authority plus identity checks).
- [ ] Secrets: `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`.
- Without it, Windows users see a SmartScreen warning on install.

## 4. Cutting a release
- [ ] Bump `version` in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`, push a tag like `v0.1.0`.
- Tags containing a hyphen (e.g. `v0.1.0-rc1`, `v0.0.1-test`) publish as GitHub pre-releases and are never "latest".
- The release workflow builds all three platforms, attaches them to a GitHub Release, and publishes `latest.json` when signing secrets exist.

## 5. Test before trusting it
- [ ] Do a dry run with a test tag.
- [ ] Install an older build and confirm it updates itself silently and runs the new version on next launch.
- [ ] Windows: confirm the installer does not relaunch the app or interrupt a write/verify/backup.
- Not yet run end to end on real GitHub runners.

## 6. CI security checks
Workflows use least-privilege `permissions`, actions pinned to commit SHAs (Renovate bumps them), `--locked` Cargo builds and `npm ci --ignore-scripts`.

`security.yml` (PRs, pushes to main, weekly, manual):
| Job | What it does | Repo private? |
|---|---|---|
| `cargo-deny` | RustSec advisories, licence allow-list, duplicate crates (warn), crates.io-only sources (`src-tauri/deny.toml`) | runs |
| `npm-audit` | `npm audit --omit=dev` (only devDependencies today, so currently a no-op guard) | runs |
| `gitleaks` | secret scan of full git history | runs |
| `workflow-lint` | actionlint + zizmor on `.github/workflows` | runs |
| `codeql` | CodeQL for javascript-typescript and rust | skipped until public |
| `dependency-review` | blocks PRs adding vulnerable dependencies | skipped until public |

`release.yml` also publishes a CycloneDX SBOM (Rust and npm) and `SHA256SUMS` on every release (works on private repos), and a build-provenance attestation, which is skipped until the repo is public.

## 7. Repo settings the owner should enable (not changed by code)
- [ ] Branch protection on `main`: require PRs, and require the `rust` (CI) and `cargo-deny`, `npm-audit`, `gitleaks`, `workflow-lint` (Security) checks.
- [ ] Secret scanning and push protection (Settings, Code security).
- [ ] Dependabot security alerts and private vulnerability reporting.
- [ ] Optional: require signed commits.
- [ ] If the repo goes public, `codeql`, `dependency-review` and attestations switch on automatically; add `codeql` to required checks then.
