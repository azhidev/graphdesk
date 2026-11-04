import { mount, tick } from 'svelte';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import App from '../../src/App.svelte';
import '../../src/foundation.css';

const size = Number(new URL(location.href).searchParams.get('size') ?? 3);
const page = { id: 'page', title: 'Fixture', journal_date: '2026-11-04' };
const other = { id: 'other', title: 'Other', journal_date: null };
const registry = { remember: true, reopen: true, entries: [{ path: '/fixture.sqlite3', graph_id: 'graph' }], warning: false };
let blocks = Array.from({ length: size }, (_, position) => ({ id: `b${position}`, page_id: page.id,
  parent_id: null, position, content: `English فارسی ${position}${position < 3 ? ' [[Target]]' : ''}`, task_status: null }));
let releaseEdit, releaseLinks, rejectLinks, holdLinks = false, calls = [];
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
    case 'recovery_list': case 'recovery_notes': case 'editor_search': return [];
    case 'editor_journal': case 'editor_find_journal': return page;
    case 'editor_page': return { blocks: args.pageId === page.id ? blocks : [{ ...blocks[0], id: 'other-block', page_id: other.id }], can_undo: false, can_redo: false };
    case 'editor_links': {
      const value = args.pageId === page.id ? links() : { references: [], backlinks: [] };
      return holdLinks ? new Promise((resolve, reject) => { releaseLinks = () => resolve(value); rejectLinks = reject; }) : value;
    }
    case 'editor_apply': return new Promise(resolve => releaseEdit = () => {
      const { action } = args;
      let changed, focus_id = action.id;
      if (action.kind === 'split') {
        const old = blocks.find(b => b.id === action.id);
        changed = [{ ...old, content: old.content.slice(0, action.start) },
          { ...old, id: 'new', position: old.position + .5, content: old.content.slice(action.end) }];
        focus_id = 'new';
      } else changed = [{ ...blocks.find(b => b.id === action.id), parent_id: action.kind === 'indent' ? 'b0' : null }];
      for (const b of changed) { const i = blocks.findIndex(v => v.id === b.id); if (i < 0) blocks.push(b); else blocks[i] = b; }
      resolve({ changed, removed: [], focus_id, can_undo: true, can_redo: false });
    });
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
  mount(App, { target: document.body });
  await until(() => reference('b2'));
  row('b1').querySelector('.block-content').click(); await settle();
  let active = 'b1';
  for (const kind of ['split', 'indent', 'outdent']) {
    window.__progress = kind;
    const input = row(active).querySelector('textarea');
    input.focus(); input.setSelectionRange(2, 4, 'backward');
    const neighbor = row('b2'), text = neighbor.querySelector('.block-content'), link = reference('b2');
    const top = neighbor.getBoundingClientRect().top, outline = document.querySelector('.outline');
    const height = input.getBoundingClientRect().height;
    const styles = node => { const s = getComputedStyle(node); return [s.opacity, s.visibility, s.backgroundColor]; };
    holdLinks = true; releaseLinks = null; releaseEdit = null;
    input.dispatchEvent(new KeyboardEvent('keydown', { key: kind === 'split' ? 'Enter' : 'Tab', shiftKey: kind === 'outdent', bubbles: true, cancelable: true }));
    await until(() => releaseEdit); await settle();
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
  window.__result = { size, metrics };
} catch (error) { window.__result = { error: String(error), metrics, calls }; }
