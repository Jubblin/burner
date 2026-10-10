# Testing Burner on a real drive

Nothing in Burner has been run against a real device yet. All automated tests use temp files. Do this once, carefully, before trusting a release.

## 1. Before you start
- [ ] Use a spare USB stick or SD card (8 to 32 GB) you do not mind erasing. Never your only copy of anything.
- [ ] Unplug every other external drive, backup disk and SD card, so only the test drive is attached.
- [ ] Use a small image (Raspberry Pi OS Lite or a tiny Linux ISO). Keep its published SHA-256.
- [ ] Build from main: `npm install`, then `npm run tauri dev` (or `src-tauri/target/debug/burner`).

## 2. Check the safety rails (nothing is written yet)
- [ ] Open **Write**. Your internal/system disk is NOT listed ("Your system disk is never listed.").
- [ ] With the stick plugged in, it is listed with the right name, size and path (macOS `/dev/diskN`, Linux `/dev/sdX`, Windows `\\.\PhysicalDriveN`). Cross-check in the OS: `diskutil list`, `lsblk`, or Disk Management.
- [ ] Choose the image and select the stick. The button stays disabled until you type the drive name (or tick "I understand every file on ... will be erased"). Only then is it red: "Erase *name* and write".

## 3. Write and verify
- [ ] Click the button. Expect an admin prompt (macOS authorization, Linux pkexec, Windows UAC). Note whether it appears once or repeatedly.
- [ ] Watch "Writing", then "Verifying" (read back). "Done. Verified." appears only after the read-back finishes.
- [ ] Eject and replug the stick, then boot or mount it to confirm the image really works.
- [ ] Optional: checksum the first N bytes (N = image size) of the raw device (`shasum -a 256` / `sha256sum`, read as root) and compare with the image.

## 4. Break it on purpose (each in a fresh run)
- [ ] Cancel mid-write: shows "Write cancelled", no stuck state; "Try again" returns to idle.
- [ ] Pull the stick mid-write: expect a clear "Write failed" message, not a hang or crash.
- [ ] Corrupt download: point the catalog at a copy with one character of an entry's `sha256` changed (`catalog_url.txt` in the app config dir, or `BURNER_CATALOG_URL`), then download it in **Download OS**. Expect "checksum mismatch: the download was corrupt and has been deleted", and the file gone.
- [ ] Deny the admin prompt: expect "authorization was denied or the helper failed" and nothing written.

## 5. Backup test
- [ ] In **Backup**, back up the freshly written stick twice: with "Compress with gzip" off and on.
- [ ] Plain file matches the original image for its length: `cmp -n LENGTH backup.img image.img`. The gzip file decompresses (`gunzip -c`) to the same bytes.

## 6. Download OS test
- [ ] Download one small image from the list. Confirm progress ("Downloading ..."), the checksum check, and that "Use this image" opens **Write** with the file preselected.

## 7. If anything looks wrong
Stop. Do not retry on a drive that matters. Note OS version, drive, exact steps and any message, and open an issue at github.com/Jubblin/burner.

## Pass criteria
- [ ] System disk never listed
- [ ] Right drive listed
- [ ] Write locked until confirmed
- [ ] Write + verify passes and the stick boots
- [ ] Cancel / pull / deny handled cleanly
- [ ] Backup matches
- [ ] Download checksum enforced
