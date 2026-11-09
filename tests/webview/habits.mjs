// Compiled full-App synthetic IPC. Not native persistence, physical keys or IME acceptance.
import { mount, unmount, tick } from 'svelte';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';
import App from '../../src/App.svelte';
import { preferences } from '../../src/lib/preferences';
import '../../src/foundation.css';

const graphs = new Map(['A', 'B'].map(id => [id, { enabled: false, habits: [], events: [] }]));
let current = 'A', session, serial = 0, picked = 'B', hold = false, release, failRead = false, destroyed = false;
let stored = { language: 'en', calendar: 'gregorian', search_mode: 'original', theme: 'light', habit_timezone: null };
const page = () => ({ id: `page-${current}`, title: `Page ${current}`, journal_date: '2026-11-05' });
const entry = () => ({ path: `/${current}.sqlite3`, graph_id: current });
const registry = () => ({ remember: true, reopen: true, entries: [entry()], warning: false });
const calls = [];
mockWindows('main'); window.isTauri = true;
mockIPC((cmd, args) => {
  if (['extension_state', 'extension_configure', 'habits_request'].includes(cmd) && args.session !== session) throw new Error('stale session');
  switch (cmd) {
    case 'plugin:window|destroy': destroyed = true; return null;
    case 'preferences_load': return { preferences: stored, warning: false };
    case 'preferences_save': stored = structuredClone(args.preferences); return null;
    case 'habits_timezone': return { system: 'Asia/Tehran', resolved: 'Asia/Tehran' };
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
    case 'extension_state': return { graph_id: current, extensions: [{ id: 'graphdesk.habits', enabled: graphs.get(current).enabled }] };
    case 'extension_configure': graphs.get(current).enabled = args.enabled; return null;
    case 'habits_request': {
      const g = graphs.get(current), r = args.request; calls.push(structuredClone(r));
      if (!g.enabled || args.extensionId !== 'graphdesk.habits' || args.apiVersion !== 1) throw new Error('denied');
      if (r.op === 'list') { if (failRead && r.limit === 30) throw new Error('fixture read failure'); return { type: 'habits', data: structuredClone(g.habits) }; }
      if (r.op === 'get') return { type: 'habit', data: structuredClone(g.habits.find(h => h.id === r.id)) };
      if (r.op === 'status') return { type: 'status', data: { habit_id: r.id, positive: { due: !g.events.some(e => e.habit_id === r.id && !e.voided), occurrence: 1700000000, next_occurrence: 1900000000 }, negative: null } };
      if (r.op === 'events') return { type: 'events', data: structuredClone(g.events.filter(e => e.habit_id === r.habit_id)) };
      const execute = () => {
        if (r.op === 'create') {
          let h = g.habits.find(h => h.id === r.input.id);
          if (!h) { h = { ...structuredClone(r.input), revision: 1 }; g.habits.push(h); }
          return { type: 'habit', data: structuredClone(h) };
        }
        if (r.op === 'record') {
          let event = g.events.find(e => e.id === r.input.id);
          if (!event) { event = { ...r.input, revision: 1, voided: false, recorded_at: r.input.occurred_at }; g.events.unshift(event); }
          return { type: 'event', data: structuredClone(event) };
        }
        if (r.op === 'correct') {
          const e = g.events.find(e => e.id === r.event_id);
          if (e.revision !== r.expected_revision) throw new Error('revision conflict');
          e.voided = r.voided; e.revision++; return { type: 'event', data: structuredClone(e) };
        }
        if (r.op === 'update') {
          const h = g.habits.find(h => h.id === r.id);
          if (h.revision !== r.expected_revision) throw new Error('revision conflict');
          h.definition = structuredClone(r.definition); h.revision++; return { type: 'habit', data: structuredClone(h) };
        }
        throw new Error('unexpected Habits operation');
      };
      if (hold) return new Promise(resolve => release = () => resolve(execute()));
      return execute();
    }
    default: if (cmd.startsWith('plugin:')) return null; throw new Error(`Unexpected IPC ${cmd}`);
  }
}, { shouldMockEvents: true });
const frame = () => new Promise(resolve => requestAnimationFrame(resolve));
async function settle() { await tick(); await frame(); await frame(); }
async function until(check) { for (let i = 0; i < 250; i++) { if (check()) { await settle(); return; } await frame(); } throw new Error(`condition timed out: ${window.__stage}`); }
function assert(v, m) { if (!v) throw new Error(m); }
function button(text, root = document) { const b = [...root.querySelectorAll('button')].find(b => b.textContent.trim() === text); if (!b) throw new Error(`missing button ${text}`); return b; }
const click = async (text, root) => { button(text, root).click(); await settle(); };
const dialog = () => document.querySelector('.habits + dialog');
const habitCheckbox = () => [...document.querySelectorAll('.extension-setting')].find(el => el.textContent.includes('Habits'))?.querySelector('input');
const nav = () => document.querySelector('.extension-nav button');
const field = label => [...dialog().querySelectorAll('label')].find(el => el.firstChild?.textContent?.trim() === label)?.querySelector('input, select');
async function input(label, value) { const el = field(label); assert(el, `missing field ${label}`); el.value = value; el.dispatchEvent(new Event(el.tagName === 'SELECT' ? 'change' : 'input', { bubbles: true })); await settle(); }
async function switchTo(id) {
  picked = id; await settle();
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'o', code: 'KeyO', ctrlKey: true, bubbles: true, cancelable: true }));
  await until(() => current === id && document.querySelector('#page-heading')?.textContent === `Page ${id}`);
}

try {
  window.__stage = 'startup-enable';
  const app = mount(App, { target: document.body });
  await until(() => habitCheckbox() && !habitCheckbox().disabled);
  assert(!nav(), 'disabled navigation');
  await click('Settings'); habitCheckbox().click(); await until(nav); document.querySelector('.settings-header button').click(); await settle();
  failRead = true; nav().click(); await until(() => document.querySelector('.habits [role="alert"]'));
  assert(document.querySelector('.habits').textContent.includes('fixture read failure'), 'read error missing');
  failRead = false; await click('Retry'); await until(() => document.querySelector('.empty'));
  window.__stage = 'positive-form';
  const newButton = button('New habit'); newButton.focus(); newButton.click(); await until(() => dialog().open && field('Name'));
  assert(document.activeElement === field('Name'), 'initial focus');
  await until(() => field('Timezone')?.value === 'Asia/Tehran');
  assert(field('Mode').value === 'fixed', 'Fixed default');
  await input('Name', 'Morning practice'); await input('First occurrence', '2026-01-01T09:00:00+03:30'); await input('Unit', 'months');
  assert(dialog().textContent.includes('not Jalali recurrence'), 'Gregorian disclosure');
  dialog().querySelector('form').dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true })); await settle();
  dialog().dispatchEvent(new Event('cancel', { cancelable: true })); await settle();
  assert(!dialog().textContent.includes('Discard this form?'), 'composition cancellation');
  dialog().querySelector('form').dispatchEvent(new CompositionEvent('compositionend', { bubbles: true })); await settle();
  hold = true; dialog().querySelector('form').requestSubmit(); await until(() => release);
  dialog().querySelector('form').requestSubmit(); await settle();
  assert(calls.filter(r => r.op === 'create').length === 1 && graphs.get('A').habits.length === 0, 'duplicate or optimistic create');
  hold = false; release(); release = null; await until(() => !dialog().open);
  assert(document.activeElement === newButton, 'focus return');
  window.__stage = 'positive-event';
  await click('Complete'); await until(() => dialog().open);
  await input('Event time', '2099-01-01T00:00:00Z'); dialog().querySelector('form').requestSubmit(); await settle();
  assert(dialog().textContent.includes('Future events are not allowed.') && field('Event time').value === '2099-01-01T00:00:00Z', 'invalid event lost input');
  await input('Event time', '2026-05-01T00:00:00Z'); dialog().querySelector('form').requestSubmit(); await until(() => !dialog().open);
  assert(graphs.get('A').events[0].kind === 'completion', 'positive completion kind');
  assert(!document.querySelector('.habits').textContent.includes('Morning practice'), 'completed habit still due');
  window.__stage = 'negative-form';
  await click('New habit'); await input('Name', 'No smoking'); await input('Kind', 'negative');
  assert(!field('Timezone') && !field('First occurrence'), 'negative schedule fields');
  dialog().querySelector('form').requestSubmit(); await until(() => !dialog().open);
  assert(graphs.get('A').habits[1].definition.schedule === null, 'negative payload schedule');
  window.__stage = 'events-history-correction';
  await click('Record lapse'); await until(() => dialog().open);
  assert(field('Event kind').value === 'lapse', 'default lapse');
  await input('Event time', '2026-05-01T00:00:00Z'); dialog().querySelector('form').requestSubmit(); await until(() => !dialog().open);
  await click('No smoking'); await until(() => document.querySelector('.detail'));
  assert(document.activeElement.id === 'habit-detail-heading', 'detail focus');
  const originalTime = graphs.get('A').events[0].occurred_at;
  await click('Void'); await until(() => graphs.get('A').events[0].voided); await click('Restore');
  assert(graphs.get('A').events[0].occurred_at === originalTime && !graphs.get('A').events[0].voided, 'payload rewritten');
  await click('Record lapse'); await input('Event kind', 'confirmation'); await input('Event time', '2026-05-02T00:00:00Z'); await input('Period start (inclusive)', '2026-05-01T00:00:00Z'); await input('Period end (exclusive)', '2026-05-02T00:00:00Z');
  assert(dialog().textContent.includes('interpreted in UTC'), 'confirmation zone');
  dialog().querySelector('form').requestSubmit(); await until(() => !dialog().open);
  await click('Archive'); assert(button('Record lapse').disabled, 'archived event guard'); await click('Unarchive');
  await click('Edit habit'); assert(field('Kind').disabled, 'immutable kind');
  window.__stage = 'dirty-navigation-escape';
  // Programmatic click intentionally bypasses modal hit testing: shell guard must still hold.
  document.querySelector('.primary-nav button').click(); await settle(); assert(dialog().open && document.querySelector('.habits'), 'dirty navigation lost form');
  await emit('tauri://close-requested'); await settle(); assert(!destroyed && dialog().open, 'dirty mocked close was not blocked');
  dialog().dispatchEvent(new Event('cancel', { cancelable: true })); await settle(); assert(dialog().textContent.includes('Discard this form?'), 'Escape/cancel guard');
  await click('Keep editing', dialog()); assert(dialog().open, 'keep editing');
  await click('Cancel', dialog()); await click('Discard form', dialog());
  window.__stage = 'isolation-disable-reenable';
  await switchTo('B'); assert(!nav(), 'B inherited enablement'); await switchTo('A'); await until(nav); nav().click(); await until(() => document.querySelector('.habits'));
  assert(document.querySelector('.habits').textContent.includes('No smoking'), 'A data lost');
  await click('Settings'); habitCheckbox().click(); await until(() => !nav()); assert(graphs.get('A').habits.length === 2, 'disable deleted data');
  habitCheckbox().click(); await until(nav); document.querySelector('.settings-header button').click(); await settle();
  window.__stage = 'rtl-narrow-theme';
  preferences.set({ ...stored, language: 'fa', calendar: 'jalali', theme: 'dark' }); await settle();
  assert(document.documentElement.dir === 'rtl' && document.querySelector('.habits').textContent.includes('عادت‌ها'), 'RTL localization');
  assert(document.documentElement.scrollWidth <= innerWidth + 1, 'horizontal overflow');
  await unmount(app);
  window.__result = { ok: true, suite: 'habits', width: innerWidth, checks: ['loading/error/empty', 'both forms', 'snapshot zone and summary', 'ack/duplicate suppression', 'event history/confirmation/void/restore/archive', 'composition/cancel/focus', 'dirty departure', 'A-B-A', 'disable/enable', 'RTL/dark/narrow'] };
} catch (error) { window.__result = { error: String(error), stack: error.stack, stage: window.__stage }; }
