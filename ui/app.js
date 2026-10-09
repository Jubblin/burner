const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const dialog = window.__TAURI__.dialog;
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];

const fmt = (b) => (b >= 1e9 ? (b / 1e9).toFixed(1) + ' GB' : Math.round(b / 1e6) + ' MB');
const clock = (s) => `${Math.floor(s / 60)}:${String(Math.round(s % 60)).padStart(2, '0')}`;
const el = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = text; return e; };
const base = (p) => p.split(/[\\/]/).pop();

const S = { drives: [], target: null, image: null, backupTarget: null, os: null, catalog: [], busy: false, lastPhase: null };

// ---- modes
function setMode(m) {
  $$('main').forEach((x) => (x.hidden = x.dataset.mode !== m));
  $$('.tab').forEach((t) => t.setAttribute('aria-selected', t.dataset.mode === m));
}
$$('.tab').forEach((t) => (t.onclick = () => !S.busy && setMode(t.dataset.mode)));

// ---- progress
let t0 = 0;
listen('progress', ({ payload: { phase, done, total } }) => {
  const box = $('.pbar:not([hidden])');
  if (!box) return;
  if (phase !== S.lastPhase) { S.lastPhase = phase; t0 = performance.now(); }
  const secs = (performance.now() - t0) / 1000 || 1;
  const rate = done / secs;
  const label = { write: 'Writing', verify: 'Verifying', backup: 'Reading drive', download: 'Downloading' }[phase] || phase;
  $('.phase', box).textContent = S.os && phase === 'download' ? `Downloading ${S.os.name}` : label;
  $('.pct', box).textContent = Math.round((done / total) * 100) + '%';
  $('.bar', box).style.width = (done / total) * 100 + '%';
  $('.bytes', box).textContent = `${fmt(done)} of ${fmt(total)}` + (phase === 'verify' ? ' read back' : '');
  $('.rate', box).textContent = done ? `${(rate / 1e6).toFixed(1)} MB/s · ${clock((total - done) / rate)} left` : '';
});
const run = async (box, fn) => {
  S.busy = true; S.lastPhase = null;
  box.hidden = false; $('.bar', box).style.width = '0%';
  try { return await fn(); } finally { S.busy = false; box.hidden = true; }
};

// ---- drives
function driveCard(d, cls, pressed, onclick) {
  const b = el('button', 'card ' + cls);
  b.setAttribute('aria-pressed', pressed);
  b.append(el('span', 'name', d.name), el('span', 'meta', `${fmt(d.size_bytes)} · ${d.bus}`), el('span', 'path mono', d.path));
  b.onclick = onclick;
  return b;
}
function renderDrives() {
  const list = $('#drives');
  list.replaceChildren();
  $('#noDrives').hidden = S.drives.length > 0;
  list.hidden = !S.drives.length;
  S.drives.forEach((d) => {
    const sel = S.target === d;
    const c = driveCard(d, sel ? 'danger' : '', sel, () => { S.target = d; $('#confirmText').value = ''; $('#confirmBox').checked = false; renderDrives(); updateWrite(); });
    if (sel) c.append(el('span', 'tag', 'Everything on this drive will be erased'));
    list.append(c);
  });
  if (S.drives.length) list.append(el('span', 'label', 'Your system disk is never listed.'));
  const bl = $('#backupDrives');
  bl.replaceChildren(...S.drives.map((d) => driveCard(d, 'pick', S.backupTarget === d, () => { S.backupTarget = d; renderDrives(); updateBackup(); })));
}
async function scan() {
  try { S.drives = await invoke('list_drives'); } catch (e) { S.drives = []; }
  if (!S.drives.includes(S.target)) S.target = S.drives[0] ?? null;
  if (!S.drives.includes(S.backupTarget)) S.backupTarget = S.drives[0] ?? null;
  renderDrives(); updateWrite(); updateBackup();
}
$('#scanAgain').onclick = scan;

// ---- write
function updateWrite() {
  const t = S.target, name = t?.name ?? '';
  $$('.dn').forEach((e) => (e.textContent = name));
  $('#confirmText').placeholder = name;
  $('#confirmText').disabled = $('#confirmBox').disabled = !t;
  // Write stays disabled until the exact drive name is typed or the box is ticked.
  const confirmed = !!t && ($('#confirmText').value.trim() === name || $('#confirmBox').checked);
  $('#writeBtn').disabled = !(confirmed && S.image && !S.busy);
  $('#writeBtn').textContent = t ? `Erase ${name} and write` : 'Erase and write';
}
$('#confirmText').oninput = $('#confirmBox').onchange = updateWrite;
function setImage(path, size) {
  S.image = { path, name: base(path) };
  $('#imageName').textContent = S.image.name;
  $('#imageSize').textContent = size ? fmt(size) : '';
  $('#imageFile').hidden = false;
  $('#chooseImage').textContent = 'Choose a different image';
  updateWrite();
}
$('#chooseImage').onclick = async () => {
  const p = await dialog.open({ filters: [{ name: 'Disk images', extensions: ['iso', 'img', 'dmg', 'zip', 'gz', 'xz'] }] });
  if (p) setImage(p);
};
const resetWrite = () => { $('#success').hidden = $('#error').hidden = true; $('#confirmBlock').hidden = false; $('#confirmText').value = ''; $('#confirmBox').checked = false; scan(); };
$('#again').onclick = $('#retry').onclick = resetWrite;
$('#writeBtn').onclick = async () => {
  const t = S.target, t1 = performance.now();
  $('#confirmBlock').hidden = true;
  try {
    await run($('#writeProgress'), () => invoke('write_image', { imagePath: S.image.path, devicePath: t.path, confirmName: t.name }));
    $('#success .in').textContent = S.image.name;
    $('#successMeta').textContent = `${t.path} · ${clock((performance.now() - t1) / 1000)}`;
    $('#success').hidden = false;
  } catch (e) {
    $('#errTitle').textContent = e === 'cancelled' ? 'Write cancelled' : 'Write failed';
    $('#errMsg').textContent = String(e);
    $('#errPath').textContent = t.path;
    $('#error').hidden = false;
  }
};
$('#cancelWrite').onclick = $('#cancelBackup').onclick = $('#cancelDownload').onclick = () => invoke('cancel_job');

// ---- backup
function updateBackup() {
  $('#startBackup').disabled = !(S.backupTarget && $('#backupOut').value.trim()) || S.busy;
}
$('#backupOut').oninput = updateBackup;
$('#browseOut').onclick = async () => {
  const p = await dialog.save({ defaultPath: S.backupTarget ? S.backupTarget.name.replace(/\s+/g, '-').toLowerCase() + '.img' : 'backup.img' });
  if (p) { $('#backupOut').value = p; updateBackup(); }
};
$('#startBackup').onclick = async () => {
  const m = $('#backupMsg'); m.hidden = true;
  $('#startBackup').disabled = true;
  try {
    await run($('#backupProgress'), () => invoke('backup_drive', { devicePath: S.backupTarget.path, outputPath: $('#backupOut').value.trim(), compress: $('#compress').checked }));
    m.textContent = 'Backup finished.';
  } catch (e) { m.textContent = e === 'cancelled' ? 'Backup cancelled.' : `Backup failed: ${e}`; }
  m.hidden = false; updateBackup();
};

// ---- download
async function loadCatalog() {
  try { S.catalog = await invoke('fetch_catalog'); } catch (e) { S.catalog = []; }
  $('#catalog').replaceChildren(...S.catalog.map((o, i) => {
    const l = el('label', 'os'), r = el('input'), body = el('span'), t = el('span', 't');
    r.type = 'radio'; r.name = 'os'; r.onchange = () => { S.os = o; $('#startDownload').disabled = false; $('#downloadDone').hidden = true; };
    body.style.flex = '1';
    t.append(el('span', '', o.name), el('span', 'mono', fmt(o.size_bytes)));
    body.append(t, el('span', 'muted', o.description));
    l.append(r, body);
    return l;
  }));
}
$('#startDownload').onclick = async () => {
  const m = $('#downloadMsg'); m.hidden = true; $('#downloadDone').hidden = true;
  $('#startDownload').disabled = true;
  try {
    S.downloaded = await run($('#downloadProgress'), () => invoke('download_image', { id: S.os.id }));
    $('#downloadDone').hidden = false;
  } catch (e) { m.textContent = e === 'cancelled' ? 'Download cancelled.' : `Download failed: ${e}`; m.hidden = false; }
  $('#startDownload').disabled = false;
};
$('#useImage').onclick = () => { setImage(S.downloaded, S.os.size_bytes); setMode('write'); };

scan();
loadCatalog();

// ---- silent auto-update: download in background, install only when idle, never restart
(async () => {
  try {
    const update = await window.__TAURI__.updater.check();
    if (!update) return;
    await update.download();
    while (S.busy) await new Promise((r) => setTimeout(r, 5000));
    await update.install(); // applies on next launch (Windows may relaunch itself via its installer)
    localStorage.setItem('updatedTo', update.version);
  } catch (e) { /* offline, unsigned dev build, or no release: stay silent */ }
})();
{
  const v = localStorage.getItem('updatedTo');
  if (v) {
    localStorage.removeItem('updatedTo');
    window.__TAURI__.app.getVersion().then((cur) => {
      if (cur === v) { const n = $('#downloadMsg'); n.textContent = `Updated to v${v}`; n.hidden = false; }
    });
  }
}
