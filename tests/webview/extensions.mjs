// Current compiled App + real shell events; disposable in-memory IPC, not Rust acceptance.
import { mount, unmount, tick } from 'svelte';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import App from '../../src/App.svelte';
import { preferences } from '../../src/lib/preferences';
import '../../src/foundation.css';

const graphs = new Map(['A', 'B'].map(id => [id, { enabled: false, revision: 0, value: null }]));
let current = 'A', picked = 'B', serial = 0, session, releaseWrite, failWrite = false;
let storedPreferences = { language: 'en', calendar: 'gregorian', search_mode: 'original', theme: 'light', habit_timezone: null };
const page = () => ({ id: `page-${current}`, title: `Page ${current}`, journal_date: '2026-11-05' });
const entry = () => ({ path: `/${current}.sqlite3`, graph_id: current });
const registry = () => ({ remember: true, reopen: true, entries: [entry()], warning: false });
mockWindows('main'); window.isTauri = true;
mockIPC((cmd, args) => {
  if (['extension_state', 'extension_configure', 'extension_request'].includes(cmd) && args.session !== session) throw new Error('stale session');
  switch (cmd) {
    case 'preferences_load': return { preferences: structuredClone(storedPreferences), warning: false };
    case 'preferences_save': {
      if (args.preferences.habit_timezone) new Intl.DateTimeFormat('en', { timeZone: args.preferences.habit_timezone });
      storedPreferences = structuredClone(args.preferences); return null;
    }
    case 'habits_timezone': return { system: 'America/New_York', resolved: storedPreferences.habit_timezone ?? 'America/New_York' };
    case 'graph_first_run': return { fresh: true, demo: entry(), registry: registry(), warning: false };
    case 'graph_pick': return `/${picked}.sqlite3`;
    case 'graph_prepare': return { path: args.path, graph_id: args.path.slice(1, 2), ticket: args.path.slice(1, 2) };
    case 'graph_activate': current = args.ticket; session = `session-${++serial}`; return { ...entry(), token: session, pages: [page()] };
    case 'graph_registry_record': return registry();
    case 'editor_journal': case 'editor_find_journal': return page();
    case 'editor_page': return { blocks: [], can_undo: false, can_redo: false };
    case 'editor_links': return { references: [], backlinks: [] };
    case 'editor_search': case 'recovery_notes': case 'recovery_list': return [];
    case 'recovery_inventory': return 0;
    case 'extension_state': return { graph_id: current, extensions: [{ id: 'graphdesk.tally', enabled: graphs.get(current).enabled }] };
    case 'extension_configure': graphs.get(current).enabled = args.enabled; return null;
    case 'extension_request': {
      const graph = graphs.get(current);
      if (!graph.enabled || args.apiVersion !== 1 || args.extensionId !== 'graphdesk.tally') throw new Error('denied');
      if (args.request.op === 'storage_read') return { revision: graph.revision, value: graph.value };
      return new Promise((resolve, reject) => releaseWrite = () => {
        if (failWrite) { reject(new Error('Fixture storage failure')); return; }
        if (args.request.expected_revision !== graph.revision) { reject(new Error('conflict')); return; }
        graph.revision++; graph.value = args.request.value;
        resolve({ revision: graph.revision, value: graph.value });
      });
    }
    default: if (cmd.startsWith('plugin:')) return null; throw new Error(`Unexpected IPC ${cmd}`);
  }
}, { shouldMockEvents: true });
const frame = () => new Promise(resolve => requestAnimationFrame(resolve));
async function settle() { await tick(); await frame(); await frame(); }
async function until(check) { for (let i = 0; i < 300; i++) { if (check()) return; await frame(); } throw new Error('fixture condition timed out'); }
function assert(value, message) { if (!value) throw new Error(message); }
const checkbox = () => document.querySelector('.extension-setting input');
const nav = () => document.querySelector('nav[aria-label="Extensions"] button, nav[aria-label="افزونه‌ها"] button');
const value = () => document.querySelector('.extension-document .value')?.textContent;
const action = () => document.querySelector('.extension-document .actions button');
const settings = () => [...document.querySelectorAll('.sidebar-bottom button')].find(button => ['Settings', 'تنظیمات'].includes(button.textContent.trim())).click();
const closeSettings = () => document.querySelector('.settings-header button').click();
async function setTimezone(value) {
  const input = document.getElementById('habit-timezone');
  input.value = value; input.dispatchEvent(new Event('input', { bubbles: true }));
  input.closest('form').requestSubmit(); await settle();
}
const switchGraph = async id => {
  await settle();
  picked = id;
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'o', code: 'KeyO', ctrlKey: true, bubbles: true, cancelable: true }));
  await until(() => current === id && document.querySelector('#page-heading')?.textContent === `Page ${id}` && checkbox() && !checkbox().disabled);
  await settle();
};
try {
  window.__stage = 'startup';
  let app = mount(App, { target: document.body });
  await until(() => checkbox() && !checkbox().disabled);
  assert(!nav() && !checkbox().checked, 'extension must default to disabled');
  window.__stage = 'timezone-settings';
  settings(); await until(() => document.querySelector('.habit-timezone-settings [role="status"]').textContent.includes('America/New_York'));
  assert(document.getElementById('habit-timezone').value === '', 'System must be default timezone preference');
  await setTimezone('Europe/Berlin');
  assert(storedPreferences.habit_timezone === 'Europe/Berlin', 'timezone override did not save');
  assert(document.querySelector('.habit-timezone-settings [role="status"]').textContent.includes('Europe/Berlin'), 'resolved override not displayed');
  await setTimezone('Invalid/Timezone');
  assert(storedPreferences.habit_timezone === 'Europe/Berlin', 'invalid override replaced acknowledged preference');
  await setTimezone('');
  assert(storedPreferences.habit_timezone === null && storedPreferences.calendar === 'gregorian', 'System reset altered calendar or did not save');
  closeSettings(); await settle();
  window.__stage = 'enable';
  settings(); await settle(); checkbox().click(); await until(nav); closeSettings();
  window.__stage = 'open-view';
  nav().click(); await until(() => value() === '0');
  window.__stage = 'write';
  releaseWrite = null; action().click(); await until(() => releaseWrite); action().click(); await settle();
  assert(value() === '0' && graphs.get('A').revision === 0, 'counter displayed or persisted optimistic/duplicate state');
  releaseWrite(); await until(() => value() === '1');
  settings(); await settle(); checkbox().click(); await until(() => !nav());
  assert(graphs.get('A').value.count === 1 && !checkbox().checked, 'disable erased data or failed');
  checkbox().click(); await until(nav); closeSettings(); nav().click(); await until(() => value() === '1');
  window.__stage = 'switch-B';
  await switchGraph('B'); assert(!nav() && !checkbox().checked, 'graph switch leaked enable state/contributions');
  settings(); await settle(); checkbox().click(); await until(nav); closeSettings(); nav().click(); await until(() => value() === '0');
  await switchGraph('A'); await until(nav); nav().click(); await until(() => value() === '1');
  preferences.update(p => ({ ...p, language: 'fa', theme: 'dark' })); await settle();
  assert(document.documentElement.dir === 'rtl' && document.querySelector('#extension-heading').textContent === 'شمارنده', 'extension localization/RTL failed');
  failWrite = true; releaseWrite = null; action().click(); await until(() => releaseWrite); releaseWrite();
  await until(() => document.querySelector('.extension-document').textContent.includes('Fixture storage failure'));
  assert(graphs.get('A').value.count === 1 && !nav(), 'failed extension corrupted storage or remained active');
  document.querySelector('.page-list button').click(); await until(() => document.querySelector('.notes-view')?.hidden === false);
  settings(); await settle(); document.querySelector('.extension-setting button').click(); await until(nav); closeSettings();
  nav().click(); await until(() => value() === '1');
  settings(); await settle(); await setTimezone('Asia/Tehran'); closeSettings(); await settle();
  assert(storedPreferences.language === 'fa' && storedPreferences.theme === 'dark', 'timezone preference disturbed language/theme');
  await unmount(app); await settle();
  app = mount(App, { target: document.body }); await until(nav); nav().click(); await until(() => value() === '1');
  assert(checkbox().checked, 'restart did not restore enabled state');
  settings(); await settle();
  assert(document.getElementById('habit-timezone').value === 'Asia/Tehran', 'timezone preference did not restore on remount');
  closeSettings(); await settle();
  const bounds = document.querySelector('.extension-document').getBoundingClientRect();
  assert(bounds.width > 0 && bounds.left >= 0 && bounds.right <= innerWidth, 'extension view overflows viewport');
  await unmount(app);
  window.__result = { passed: true, viewport: innerWidth, graphs: [...graphs].map(([id, graph]) => ({ id, enabled: graph.enabled, revision: graph.revision, value: graph.value })) };
} catch (error) { window.__result = { error: String(error), current, serial, stage: window.__stage, text: document.body.textContent.slice(-2500) }; }
