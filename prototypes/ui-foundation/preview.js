// Visual-only state selector: no graph, Tauri, or application preference access.
const root = document.documentElement;
const view = root.dataset.view;
const layout = document.querySelector('.layout');
const toggle = document.querySelector('.sidebar-toggle');
const editor = document.querySelector('#example-block');
const settingsDialog = document.querySelector('.settings-dialog');
const settingsTrigger = document.querySelector('.settings-trigger');
const themeSelect = document.querySelector('select[aria-label="Theme"]');
const languageSelect = document.querySelector('select[aria-label="Language"]');
const calendarSelect = document.querySelector('select[aria-label="Calendar"]');
if (view === 'search') toggle.setAttribute('aria-controls', 'search-sidebar');

themeSelect.value = view === 'dark' ? 'dark' : 'system';
languageSelect.value = view === 'rtl' ? 'fa' : 'en';
calendarSelect.value = view === 'rtl' ? 'jalali' : 'gregorian';

function display() {
  const language = languageSelect.value;
  root.lang = language;
  root.dir = language === 'fa' ? 'rtl' : 'ltr';
  for (const element of document.querySelectorAll('[data-en][data-fa]')) {
    element.textContent = element.getAttribute(`data-${language}`);
  }
  const fa = language === 'fa';
  for (const [selector, en, persian] of [
    ['.sidebar-toggle', 'Toggle navigation', 'باز و بسته کردن پیمایش'],
    ['.graph-switch', 'Switch graph', 'تغییر گراف'],
    ['.history-back', 'Back', 'بازگشت'],
    ['.history-forward', 'Forward', 'جلو'],
    ['.add-button', 'New page', 'صفحهٔ جدید'],
    ['.settings-close', 'Close settings', 'بستن تنظیمات'],
    ['.search-field input', 'Search pages and blocks', 'جست‌وجوی صفحه‌ها و بلوک‌ها'],
    ['.block-actions', 'Block editor actions', 'کنش‌های ویرایش بلوک'],
  ]) document.querySelector(selector).setAttribute('aria-label', fa ? persian : en);
  document.querySelector('.history-back').title = `${fa ? 'بازگشت' : 'Back'} · Alt+Left`;
  document.querySelector('.history-forward').title = `${fa ? 'جلو' : 'Forward'} · Alt+Right`;
  document.querySelector('.search-field input').placeholder = fa ? 'جست‌وجوی صفحه‌ها و بلوک‌ها' : 'Search pages and blocks';
  const dates = {
    en: { gregorian: 'October 29, 2026', jalali: '7 Aban 1405' },
    fa: { gregorian: '۲۹ اکتبر ۲۰۲۶', jalali: '۷ آبان ۱۴۰۵' },
  };
const label = view === 'empty'
    ? ({ en: { gregorian: 'October 27, 2026', jalali: '5 Aban 1405' },
        fa: { gregorian: '۲۷ اکتبر ۲۰۲۶', jalali: '۵ آبان ۱۴۰۵' } })[language][calendarSelect.value]
    : dates[language][calendarSelect.value];
  document.querySelector('.document h1').textContent = label;
  document.querySelector('.date-label').textContent = label;
  if (view === 'empty') document.querySelector('.eyebrow').textContent = language === 'fa' ? 'روزانه / سه‌شنبه' : 'JOURNAL / TUESDAY';
  editor.value = language === 'fa'
    ? 'یادداشت فارسی با واژهٔ GraphDesk و پیوند [[Project Atlas]]'
    : 'A small, deliberate next step.';
}
function theme() {
  root.dataset.theme = themeSelect.value === 'system'
    ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
    : themeSelect.value;
}
display();
theme();
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => { if (themeSelect.value === 'system') theme(); });
languageSelect.addEventListener('change', display);
calendarSelect.addEventListener('change', display);
themeSelect.addEventListener('change', theme);
settingsTrigger.addEventListener('click', () => { settingsDialog.showModal(); document.querySelector('#settings-heading').focus(); });
document.querySelector('.settings-close').addEventListener('click', () => settingsDialog.close());
document.querySelector('.settings-done').addEventListener('click', () => settingsDialog.close());
settingsDialog.addEventListener('close', () => { if (settingsTrigger.isConnected) settingsTrigger.focus(); });

for (const link of document.querySelectorAll('.review [data-link]')) {
  if (link.dataset.link === view) link.setAttribute('aria-current', 'page');
}
if (view === 'tasks') {
  document.querySelector('.journal-entry').removeAttribute('aria-current');
  document.querySelector('.journal-entry').classList.remove('selected');
  document.querySelector('.tasks-entry').setAttribute('aria-current', 'page');
}
toggle.addEventListener('click', () => {
  const collapsed = layout.classList.toggle('nav-collapsed');
  layout.classList.toggle('nav-open', !collapsed);
  toggle.setAttribute('aria-expanded', String(!collapsed));
  if (!collapsed && (view === 'narrow' || matchMedia('(max-width: 760px)').matches)) {
    (view === 'search' ? document.querySelector('.back-link') : document.querySelector('.journal-entry')).focus();
  }
});
document.addEventListener('keydown', event => {
  if (event.key === 'Escape' && layout.classList.contains('nav-open')) {
    layout.classList.remove('nav-open');
    layout.classList.add('nav-collapsed');
    toggle.setAttribute('aria-expanded', 'false');
    toggle.focus();
  }
});
if (view === 'narrow' || window.matchMedia('(max-width: 760px)').matches) {
  const open = new URLSearchParams(location.search).get('nav') === 'open';
  layout.classList.toggle('nav-collapsed', !open);
  layout.classList.toggle('nav-open', open);
  toggle.setAttribute('aria-expanded', String(open));
}
const params = new URLSearchParams(location.search);
if (view === 'settings' || params.get('settings') === 'open') settingsTrigger.click();
if (params.get('actions') === 'open') document.querySelector('.more-actions').open = true;
if (params.get('focus') === '1') editor.focus();
// The remaining controls are illustrative, not wired to controllers or storage.
