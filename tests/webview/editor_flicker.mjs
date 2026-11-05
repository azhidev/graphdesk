import { mount, tick, unmount } from 'svelte';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import App from '../../src/App.svelte';
import { preferences } from '../../src/lib/preferences';
import CalendarPicker from '../../src/lib/CalendarPicker.svelte';
import TaskStatus from '../../src/lib/editor/TaskStatus.svelte';
import '../../src/foundation.css';

const size = Number(new URL(location.href).searchParams.get('size') ?? 3);
const controlsOnly = new URL(location.href).searchParams.get('controlsOnly') === '1';
const page = { id: 'page', title: 'Fixture', journal_date: '2026-11-04' };
const other = { id: 'other', title: 'Other', journal_date: null };
const registry = { remember: true, reopen: true, entries: [{ path: '/fixture.sqlite3', graph_id: 'graph' }], warning: false };
let blocks = Array.from({ length: size }, (_, position) => ({ id: `b${position}`, page_id: page.id,
  parent_id: null, position, content: `English فارسی ${position}${position < 3 ? ' [[Target]]' : ''}`, task_status: null }));
let releaseEdit, rejectEdit, releaseLinks, rejectLinks, holdLinks = false, calls = [];
let taskUndo = [], taskRedo = [];
const links = () => ({ references: blocks.filter(b => b.content.includes('[[Target]]')).map(b => ({ source_block_id: b.id, title_key: 'target', label: 'Target', target_page_id: 'target', target_title: 'Target' })), backlinks: [] });
mockWindows('main');
window.isTauri = true;
mockIPC((cmd, args) => {
  calls.push(cmd);
  switch (cmd) {
    case 'preferences_load': return { preferences: { language: 'en', calendar: 'gregorian', search_mode: 'original', theme: 'light' }, warning: false };
    case 'graph_first_run': return { fresh: true, demo: registry.entries[0], registry, warning: false };
    case 'graph_prepare': return { ...registry.entries[0], ticket: 'ticket' };
    case 'graph_activate': return { ...registry.entries[0], token: 'session', pages: [page, other] };
    case 'graph_registry_record': return registry;
    case 'recovery_inventory': return 0;
    case 'recovery_clear': return null;
    case 'recovery_list': case 'recovery_notes': case 'editor_search': return [];
    case 'editor_journal': case 'editor_find_journal': return page;
    case 'editor_page': return { blocks: args.pageId === page.id ? blocks : [{ ...blocks[0], id: 'other-block', page_id: other.id }], can_undo: true, can_redo: true };
    case 'editor_links': {
      const value = args.pageId === page.id ? links() : { references: [], backlinks: [] };
      return holdLinks ? new Promise((resolve, reject) => { releaseLinks = () => resolve(value); rejectLinks = reject; }) : value;
    }
    case 'editor_history': {
      const from = args.redo ? taskRedo : taskUndo, to = args.redo ? taskUndo : taskRedo;
      const change = from.pop();
      assert(change, 'history fixture exhausted');
      const current = blocks.find(b => b.id === change.id);
      to.push({ ...current }); blocks = blocks.map(b => b.id === change.id ? change : b);
      return { changed: [change], removed: [], focus_id: change.id, can_undo: taskUndo.length > 0, can_redo: taskRedo.length > 0 };
    }
    case 'editor_apply': return new Promise((resolve, reject) => { rejectEdit = reject; releaseEdit = () => {
      const { action } = args;
      let changed, focus_id = action.id;
      if (action.kind === 'split') {
        const old = blocks.find(b => b.id === action.id);
        changed = [{ ...old, content: old.content.slice(0, action.start) },
          { ...old, id: 'new', position: old.position + .5, content: old.content.slice(action.end) }];
        focus_id = 'new';
      } else if (action.kind === 'task') {
        const old = blocks.find(b => b.id === action.id);
        assert(old.task_status === action.expected, 'task mutation used stale expected status');
        taskUndo.push({ ...old }); taskRedo = [];
        changed = [{ ...old, task_status: action.status }];
      } else changed = [{ ...blocks.find(b => b.id === action.id), parent_id: action.kind === 'indent' ? 'b0' : null }];
      for (const b of changed) { const i = blocks.findIndex(v => v.id === b.id); if (i < 0) blocks.push(b); else blocks[i] = b; }
      resolve({ changed, removed: [], focus_id, can_undo: true, can_redo: false });
    }; });
    default: if (cmd.startsWith('plugin:')) return null;
      throw new Error(`Unexpected IPC: ${cmd}`);
  }
}, { shouldMockEvents: true });

const frame = () => new Promise(resolve => requestAnimationFrame(resolve));
async function settle() { await tick(); await frame(); await frame(); }
async function until(check) { for (let i = 0; i < 200; i++) { if (check()) return; await frame(); } throw new Error('Condition timed out'); }
const row = id => document.querySelector(`[data-block-id="${id}"]`);
const reference = id => row(id)?.nextElementSibling.querySelector('button');
function assert(value, message) { if (!value) throw new Error(message); }
const metrics = [];
try {
  if (!controlsOnly) {
  mount(App, { target: document.body });
  await until(() => reference('b2'));
  document.querySelector('.date-disclosure').open = true;
  await settle();
  row('b1').querySelector('.block-content').click(); await settle();
  const composingInput = row('b1').querySelector('textarea');
  composingInput.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true })); await settle();
  const compositionCalls = calls.length;
  row('b1').querySelector('.task-status').click();
  document.querySelectorAll('#editor-toolbar > button').forEach(button => button.click()); await settle();
  assert(calls.length === compositionCalls && row('b1').querySelector('.task-status').getAttribute('aria-expanded') === 'false', 'editor controls interrupted composition');
  assert(!composingInput.readOnly, 'task control composition lock made native input readonly');
  composingInput.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true })); await settle();
  let active = 'b1';
  for (const kind of ['split', 'indent', 'outdent']) {
    window.__progress = kind;
    preferences.update(p => ({ ...p, theme: kind === 'split' ? 'light' : 'dark', language: kind === 'outdent' ? 'fa' : 'en' }));
    await settle();
    const journalForm = document.querySelector('form .fields').closest('form');
    const journalButton = journalForm.querySelector('button');
    const journalAppearance = () => {
      const style = getComputedStyle(journalButton);
      return [style.opacity, style.visibility, style.backgroundColor, style.color];
    };
    const idleJournal = journalAppearance();
    const input = row(active).querySelector('textarea');
    input.focus(); input.setSelectionRange(2, 4, 'backward');
    const neighbor = row('b2'), text = neighbor.querySelector('.block-content'), link = reference('b2');
    const top = neighbor.getBoundingClientRect().top, outline = document.querySelector('.outline');
    const height = input.getBoundingClientRect().height;
    const styles = node => { const s = getComputedStyle(node); return [s.opacity, s.visibility, s.backgroundColor]; };
    const historyButtons = [...document.querySelectorAll('#editor-toolbar > button')];
    const historyStyles = historyButtons.map(styles);
    const task = row(active).querySelector('.task-status'), taskStyle = styles(task);
    if (kind === 'indent') { task.click(); await settle(); assert(task.getAttribute('aria-expanded') === 'true', 'task popover did not open before pending operation'); }
    holdLinks = true; releaseLinks = null; releaseEdit = null;
    input.dispatchEvent(new KeyboardEvent('keydown', { key: kind === 'split' ? 'Enter' : 'Tab', shiftKey: kind === 'outdent', bubbles: true, cancelable: true }));
    await until(() => releaseEdit); await settle();
    assert(historyButtons.every((button, i) => JSON.stringify(styles(button)) === JSON.stringify(historyStyles[i])), 'history controls changed appearance while pending');
    assert(JSON.stringify(styles(task)) === JSON.stringify(taskStyle) && !task.disabled, 'task status changed appearance while pending');
    assert(task.getAttribute('aria-disabled') === 'true', 'task trigger missing busy semantics');
    const pendingCalls = calls.length;
    historyButtons.forEach(button => button.click()); task.click(); await settle();
    assert(calls.length === pendingCalls && task.getAttribute('aria-expanded') === 'false', 'pending editor controls dispatched an action');
    metrics.push({ kind, journalOpacity: getComputedStyle(journalButton).opacity, journalDisabled: journalButton.disabled });
    assert(JSON.stringify(journalAppearance()) === JSON.stringify(idleJournal), 'Open journal changed appearance during mutation');
    assert(!journalButton.disabled && journalButton.getAttribute('aria-disabled') === 'true', 'journal busy lock must be guarded, not native disabled');
    assert(journalForm.querySelector('input').disabled, 'date fields must remain locked');
    const journalReads = calls.filter(c => c === 'editor_find_journal').length;
    journalButton.click();
    journalForm.requestSubmit(); // Also covers submission that bypasses the button click guard.
    await settle();
    assert(calls.filter(c => c === 'editor_find_journal').length === journalReads, 'pending journal submission navigated');
    metrics.push({ kind, phase: 'pending-edit', text: styles(text), link: styles(link), sameRow: row('b2') === neighbor, sameInput: row(active).querySelector('textarea') === input });
    assert(styles(text)[0] === '1' && styles(link)[0] === '1', 'unrelated content dimmed');
    assert(input.getBoundingClientRect().height === height, 'busy state resized input');
    // Busy activation must not navigate, even when derived data is retained.
    const reads = calls.filter(c => c === 'editor_page').length;
    link.click(); await settle();
    assert(calls.filter(c => c === 'editor_page').length === reads, 'busy reference navigated');
    releaseEdit(); await until(() => releaseLinks); await settle();
    if (kind === 'split') active = 'new';
    const nextInput = row(active).querySelector('textarea');
    metrics.push({ kind, phase: 'pending-links', linkConnected: link.isConnected, neighborTopDelta: neighbor.getBoundingClientRect().top - top,
      inputHeight: nextInput.getBoundingClientRect().height, sameOutline: document.querySelector('.outline') === outline,
      sameRow: row('b2') === neighbor, sameText: neighbor.querySelector('.block-content') === text,
      focused: document.activeElement === nextInput, selection: [nextInput.selectionStart, nextInput.selectionEnd, nextInput.selectionDirection] });
    assert(link.isConnected && reference('b2') === link, 'refresh removed unrelated reference');
    link.click(); await settle();
    assert(calls.filter(c => c === 'editor_page').length === reads, 'stale reference navigated');
    if (kind !== 'split') assert(neighbor.getBoundingClientRect().top === top, 'refresh moved unrelated row');
    releaseLinks(); holdLinks = false; await settle();
    assert(journalForm.querySelector('button') === journalButton, 'journal button replaced');
    assert(JSON.stringify(journalAppearance()) === JSON.stringify(idleJournal), 'journal appearance changed after mutation');
    assert(!journalButton.hasAttribute('aria-disabled'), 'journal remained blocked after mutation');
    assert(reference('b2') === link, 'equal refreshed reference replaced DOM');
    assert(document.querySelectorAll('[data-block-id]').length === size + 1, 'split count');
    assert(row('b2') === neighbor && neighbor.querySelector('.block-content') === text, 'unrelated text replaced');
    assert(document.activeElement === nextInput, 'focus lost');
    if (kind !== 'split') {
      assert(nextInput === input, 'move replaced textarea');
      assert(nextInput.selectionStart === 2 && nextInput.selectionEnd === 4 && nextInput.selectionDirection === 'backward', 'selection lost');
      assert(blocks.find(b => b.id === active).parent_id === (kind === 'indent' ? 'b0' : null), 'incorrect hierarchy');
    }
  }
  // A failed refresh retains readable but inert links. A later request supersedes
  // it; changing page clears the snapshot and ignores the cancelled old reply.
  holdLinks = true; releaseLinks = null; releaseEdit = null;
  window.__progress = 'failure';
  row(active).querySelector('textarea').dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }));
  await until(() => releaseEdit); releaseEdit(); await until(() => releaseLinks);
  const link = reference('b2');
  rejectLinks(new Error('Fixture links unavailable')); await settle();
  window.__progress = 'failed-links';
  assert(reference('b2') === link && link.getAttribute('aria-disabled') === 'true', 'failed refresh must retain inert snapshot');
  const reads = calls.filter(c => c === 'editor_page').length;
  link.click(); await settle();
  assert(calls.filter(c => c === 'editor_page').length === reads, 'failed-refresh link navigated');
  releaseLinks = null; releaseEdit = null;
  row(active).querySelector('textarea').dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true, cancelable: true }));
  await until(() => releaseEdit); releaseEdit(); await until(() => releaseLinks); await settle();
  window.__progress = 'retry-links';
  const staleReply = releaseLinks;
  releaseLinks = null;
  [...document.querySelectorAll('.page-list button')].find(button => button.textContent.includes('Other')).click();
  window.__progress = 'navigation';
  await until(() => row('other-block') && releaseLinks); await settle();
  assert(!link.isConnected && !document.querySelector('.outline bdi'), 'page change retained old links');
  staleReply(); await settle();
  assert(!document.querySelector('.outline bdi'), 'cancelled response installed old links');
  releaseLinks(); await settle();
  assert(row('other-block') && !document.querySelector('.outline bdi'), 'destination changed after latest response');
  preferences.update(p => ({ ...p, language: 'fa', theme: 'light' })); await settle();
  const journalForm = document.querySelector('form .fields').closest('form');
  const readsBeforeJournal = calls.filter(c => c === 'editor_find_journal').length;
  journalForm.querySelector('button').focus();
  assert(document.activeElement === journalForm.querySelector('button'), 'journal button not focusable');
  holdLinks = false;
  journalForm.querySelector('button').click();
  await until(() => calls.filter(c => c === 'editor_find_journal').length === readsBeforeJournal + 1 && row('b0'));
  await settle();
  assert(!journalForm.querySelector('button').hasAttribute('aria-disabled'), 'journal click did not unlock');
  }
  // Standalone availability contract: retain real disabled states and guard
  // implicit/programmatic form submission independently of the button click.
  for (const disabled of [false, true]) for (const busy of [false, true]) {
    const target = document.body.appendChild(document.createElement('div'));
    let opened = 0;
    const picker = mount(CalendarPicker, { target, props: { day: '2026-11-04', calendar: 'gregorian', disabled, busy, onOpen() { opened++; } } });
    await settle();
    const form = target.querySelector('form'), button = target.querySelector('button');
    assert(button.disabled === disabled, 'intrinsic journal availability changed');
    assert(getComputedStyle(button).opacity === (disabled ? '0.55' : '1'), 'journal availability styling incorrect');
    form.requestSubmit();
    assert(opened === (disabled || busy ? 0 : 1), 'journal form submit guard incorrect');
    button.click();
    assert(opened === (disabled || busy ? 0 : 2), 'journal click guard incorrect');
    await unmount(picker); target.remove();
  }
  // Exercise the compact native popover separately from persistence: statuses
  // remain acknowledged props; selection emits one existing task value only.
  for (const theme of ['light', 'dark']) for (const language of ['en', 'fa']) {
    preferences.update(p => ({ ...p, theme, language })); await settle();
    document.documentElement.dir = language === 'fa' ? 'rtl' : 'ltr';
    document.documentElement.dataset.theme = theme;
    const target = document.body.appendChild(document.createElement('div'));
    target.style.cssText = 'position:fixed;bottom:8px;right:8px';
    const changes = [];
    const picker = mount(TaskStatus, { target, props: { status: 'cancelled', busy: false, onChange: value => changes.push(value) } });
    await settle();
    const trigger = target.querySelector('.task-status');
    if (innerWidth <= 600) assert(getComputedStyle(trigger.querySelector('.status-label')).display === 'none', 'narrow status label not compact');
    trigger.focus();
    trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true })); await settle();
    const menu = target.querySelector('[role="menu"]');
    const items = [...menu.querySelectorAll('button')];
    assert(menu.matches(':popover-open') && items.length === 5, 'status menu did not open');
    assert(items[4].getAttribute('aria-checked') === 'true' && document.activeElement === items[4], 'selected status not announced/focused');
    const bounds = menu.getBoundingClientRect();
    assert(bounds.left >= 0 && bounds.right <= innerWidth && bounds.top >= 0 && bounds.bottom <= innerHeight, 'status menu clipped at viewport edge');
    const key = value => document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: value, bubbles: true, cancelable: true }));
    key('Home'); assert(document.activeElement === items[0], 'menu Home');
    key('ArrowUp'); assert(document.activeElement === items[4], 'menu ArrowUp wrap');
    key('ArrowDown'); assert(document.activeElement === items[0], 'menu ArrowDown wrap');
    key('End'); assert(document.activeElement === items[4], 'menu End');
    key('Escape'); await settle();
    assert(!menu.matches(':popover-open') && document.activeElement === trigger && changes.length === 0, 'Escape did not cancel/restore focus');
    trigger.click(); await settle();
    menu.querySelectorAll('button')[1].click(); await settle();
    assert(changes.length === 1 && changes[0] === 'todo', 'status selection dispatched wrong value');
    assert(trigger.querySelector('svg').dataset.status === 'cancelled', 'status displayed optimistic/unacknowledged value');
    assert(document.activeElement === trigger, 'status choice lost focus');
    await unmount(picker); target.remove();
  }
  // Real row callback/controller path with acknowledged and failed task writes.
  if (!controlsOnly) {
  row('b1').querySelector('.block-content').click(); await settle();
  const taskInput = row('b1').querySelector('textarea');
  taskInput.setSelectionRange(0, 1, 'backward');
  taskInput.dispatchEvent(new Event('select', { bubbles: true }));
  const taskTrigger = row('b1').querySelector('.task-status');
  const taskMenu = () => document.getElementById(taskTrigger.getAttribute('aria-controls'));
  releaseEdit = null;
  taskTrigger.click(); await settle();
  taskMenu().querySelectorAll('button')[2].click();
  await until(() => releaseEdit); await settle();
  assert(taskTrigger.querySelector('svg').dataset.status === 'none', 'task changed before acknowledgement');
  releaseEdit(); await settle();
  assert(taskTrigger.querySelector('svg').dataset.status === 'doing', 'acknowledged task not displayed');
  assert(document.activeElement === taskInput && taskInput.selectionStart === 0 && taskInput.selectionEnd === 1, 'task edit lost editor selection');
  document.querySelectorAll('#editor-toolbar > button')[0].click(); await settle();
  assert(taskTrigger.querySelector('svg').dataset.status === 'none', 'task undo failed');
  document.querySelectorAll('#editor-toolbar > button')[1].click(); await settle();
  assert(taskTrigger.querySelector('svg').dataset.status === 'doing', 'task redo failed');
  releaseEdit = null;
  taskTrigger.click(); await settle();
  taskMenu().querySelectorAll('button')[3].click();
  await until(() => releaseEdit);
  rejectEdit(new Error('Fixture task save failure')); await settle();
  assert(taskTrigger.querySelector('svg').dataset.status === 'doing', 'failed task write displayed unacknowledged status');
  assert(document.activeElement === taskInput, 'failed task write lost editor focus');
  }
  window.__result = { size, controlsOnly, viewport: innerWidth, metrics };
} catch (error) { window.__result = { error: String(error), metrics, calls }; }
