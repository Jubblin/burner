// Theme: "auto" follows the OS (prefers-color-scheme); "light"/"dark" is a manual override kept in localStorage.
const mq = matchMedia('(prefers-color-scheme: dark)');
const modes = ['auto', 'light', 'dark'];
const label = { auto: 'Theme: Auto', light: 'Theme: Light', dark: 'Theme: Dark' };
const get = () => (modes.includes(localStorage.getItem('theme')) ? localStorage.getItem('theme') : 'auto');
function apply() {
  const m = get();
  document.documentElement.dataset.theme = m === 'auto' ? (mq.matches ? 'dark' : 'light') : m;
  const b = document.getElementById('theme');
  if (b) b.textContent = label[m];
}
mq.addEventListener('change', apply);
apply();
window.addEventListener('DOMContentLoaded', () => {
  apply();
  document.getElementById('theme').onclick = () => {
    localStorage.setItem('theme', modes[(modes.indexOf(get()) + 1) % 3]);
    apply();
  };
});
