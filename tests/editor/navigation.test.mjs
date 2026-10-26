import test from 'node:test';
import assert from 'node:assert/strict';
import { NavigationHistory, filterPages } from '../../src/lib/navigation.ts';
import { GraphSwitch } from '../../src/lib/graphSwitch.ts';
import { appShortcut } from '../../src/lib/appShortcuts.ts';
import { EditorController } from '../../src/lib/editor/controller.ts';

const page = (id, block) => ({ kind: 'page', id, block });
const noop = async () => {};
test('history commits successful destinations, branches, suppresses duplicates and bounds memory', async () => {
  const h = new NavigationHistory(); h.reset('one');
  await h.visit(page('a'), noop); await h.visit(page('b'), noop);
  await h.visit(page('b', 'anchor'), noop);
  assert.equal(h.length, 2); assert.equal(h.current.block, 'anchor');
  await h.move(-1, noop); assert.equal(h.current.id, 'a'); assert.equal(h.canForward, true);
  await h.visit(page('c'), noop); assert.equal(h.canForward, false); assert.equal(h.length, 2);
  for (let i = 0; i < 150; i++) await h.visit(page(String(i)), noop);
  assert.equal(h.length, 100); assert.equal(h.current.id, '149');
  for (let i = 0; i < 99; i++) await h.move(-1, noop);
  assert.equal(h.current.id, '50'); assert.equal(h.canBack, false);
  h.reset('two'); assert.equal(h.length, 0); assert.equal(h.canForward, false);
});

test('failed page/history loads preserve both branches; task filters and ISO previews are locations only', async () => {
  const h = new NavigationHistory(); h.reset('one');
  const journal = { kind: 'journal', date: '2024-03-20' };
  await h.visit(journal, noop);
  await h.visit({ kind: 'tasks', status: 'active', page: '' }, noop);
  h.replace({ kind: 'tasks', status: 'doing', page: 'a' });
  await h.visit(page('a', 'b'), noop);
  const fails = async () => { throw new Error('storage'); };
  await assert.rejects(h.move(-1, fails)); assert.equal(h.current.kind, 'page');
  await h.move(-1, noop); assert.deepEqual(h.current, { kind: 'tasks', status: 'doing', page: 'a' });
  await assert.rejects(h.visit(page('missing'), fails)); assert.equal(h.canForward, true);
  await h.move(-1, async target => assert.deepEqual(target, journal));
  assert.equal(h.current.date, '2024-03-20');
});

test('session reset rejects late history commits and parallel navigation', async () => {
  const h = new NavigationHistory(); h.reset('one');
  let release;
  const pending = h.visit(page('old'), () => new Promise(resolve => release = resolve));
  await assert.rejects(h.visit(page('parallel'), noop));
  h.reset('two'); await h.visit(page('new'), noop); release(); await pending;
  assert.equal(h.current.id, 'new'); assert.equal(h.length, 1);
});

test('quick pages match raw/display labels, Persian, mixed scripts and NFC, without altering titles', () => {
  const pages = [{ title: 'English فارسی' }, { title: 'Café' }, { title: '2024-03-20' }];
  const label = p => p.title === '2024-03-20' ? '۱۴۰۳-۰۱-۰۱' : p.title;
  assert.equal(filterPages(pages, 'ENGLISH فا', label)[0], pages[0]);
  assert.equal(filterPages(pages, 'Cafe\u0301', label)[0], pages[1]);
  assert.equal(filterPages(pages, '۱۴۰۳', label)[0], pages[2]);
  assert.equal(filterPages(pages, '2024', label)[0], pages[2]);
  assert.equal(filterPages(Array.from({ length: 1000 }, () => ({ title: 'a' })), '', label).length, 100);
  assert.deepEqual(filterPages(pages, 'missing', label), []);
});

const key = (key, extra = {}) => ({ key, code: '', shiftKey: false, ctrlKey: true, metaKey: false, altKey: false, isComposing: false, ...extra });
const context = { modal: false, composing: false, busy: false, editable: false, session: true };
test('app shortcuts preserve composition, AltGr, modal controls and native editable-field movement', () => {
  for (const [letter, action] of [['k', 'search'], ['p', 'pages'], ['o', 'open']]) assert.equal(appShortcut(key(letter), context), action);
  assert.equal(appShortcut(key('j', { shiftKey: true }), context), 'capture');
  assert.equal(appShortcut(key('ح', { code: 'KeyP' }), context), 'pages');
  assert.equal(appShortcut(key('q', { code: 'KeyP' }), context), null);
  for (const extra of [{ altKey: true }, { isComposing: true }, { keyCode: 229 }, { repeat: true }, { defaultPrevented: true }, { getModifierState: modifier => modifier === 'AltGraph' }]) assert.equal(appShortcut(key('p', extra), context), null);
  for (const extra of [{ modal: true }, { composing: true }, { busy: true }]) assert.equal(appShortcut(key('p'), { ...context, ...extra }), null);
  const back = key('ArrowLeft', { ctrlKey: false, altKey: true });
  assert.equal(appShortcut(back, context), 'back');
  assert.equal(appShortcut(back, { ...context, editable: true }), null);
  assert.equal(appShortcut(back, { ...context, session: false }), null);
  assert.equal(appShortcut(key('o'), { ...context, session: false }), 'open');
});

function switchFixture() {
  const calls = []; let fail = ''; let release;
  const transport = {
    async prepare() { calls.push('prepare'); if (fail === 'prepare') throw new Error('invalid'); return { ticket: 'candidate' }; },
    async activate() { calls.push('activate'); return 'new'; },
    async reconcile() { calls.push('reconcile'); return null; },
    async cancel() { calls.push('cancel'); },
  };
  const hooks = {
    async flush() { calls.push('flush'); if (fail === 'flush') throw new Error('save failed'); },
    check() { calls.push('check'); if (fail === 'check') throw new Error('composition'); },
    async detach() { calls.push('detach'); if (fail === 'detach') await new Promise(resolve => release = resolve); },
    install(session) { calls.push(`install:${session}`); },
  };
  return { calls, transport, hooks, switcher: new GraphSwitch(transport), fail(value) { fail = value; }, release() { release(); } };
}
test('graph switch flushes, stages, checks and detaches before activation; failures retain source', async () => {
  const f = switchFixture(); await f.switcher.open({}, 'old', f.hooks);
  assert.deepEqual(f.calls, ['flush', 'check', 'prepare', 'check', 'detach', 'check', 'activate', 'install:new']);
  const failed = switchFixture(); failed.fail('flush'); await assert.rejects(failed.switcher.open({}, 'old', failed.hooks));
  assert.deepEqual(failed.calls, ['flush']);
  const invalid = switchFixture(); invalid.fail('prepare'); await assert.rejects(invalid.switcher.open({}, 'old', invalid.hooks));
  assert.deepEqual(invalid.calls, ['flush', 'check', 'prepare']);
  const composition = switchFixture();
  composition.hooks.detach = async () => composition.fail('check');
  await assert.rejects(composition.switcher.open({}, 'old', composition.hooks));
  assert.equal(composition.calls.includes('activate'), false); assert.equal(composition.calls.at(-1), 'cancel');
});
test('stale prepared graph operations are canceled, never activated; concurrent switches are rejected', async () => {
  const f = switchFixture(); f.fail('detach');
  const pending = f.switcher.open({}, 'old', f.hooks);
  await new Promise(resolve => setImmediate(resolve));
  await assert.rejects(f.switcher.open({}, 'old', f.hooks));
  f.switcher.invalidate(); f.release(); await assert.rejects(pending, /changed/);
  assert.equal(f.calls.includes('activate'), false); assert.equal(f.calls.at(-1), 'cancel');
});
test('lost activation acknowledgement reconciles the exact receipt without repeating activation', async () => {
  const f = switchFixture();
  f.transport.activate = async () => { f.calls.push('activate'); throw new Error('response lost'); };
  f.transport.reconcile = async () => { f.calls.push('reconcile'); return 'new'; };
  await f.switcher.open({}, 'old', f.hooks);
  assert.equal(f.calls.filter(c => c === 'activate').length, 1);
  assert.deepEqual(f.calls.slice(-3), ['activate', 'reconcile', 'install:new']);
  const stale = switchFixture(); stale.transport.activate = f.transport.activate;
  await assert.rejects(stale.switcher.open({}, 'old', stale.hooks), /response lost/);
  assert.equal(stale.calls.includes('install:new'), false);
});
test('graph departure stamps catch a full composition cycle and finalized input during candidate IO', async () => {
  const e = new EditorController({ async page() { return { blocks: [{ id: 'b', content: 'old' }], revisions: {}, can_undo: false, can_redo: false }; }, async apply() { throw new Error('locked'); } });
  e.reset('old'); await e.load('p');
  let stamp = e.departureStamp(); e.assertDeparture(stamp);
  e.composition(true); e.composition(false); assert.throws(() => e.assertDeparture(stamp));
  stamp = e.departureStamp(); e.setDraft('b', 'final'); assert.throws(() => e.assertDeparture(stamp));
  await assert.rejects(e.flush()); assert.equal(e.text('b'), 'final'); assert.equal(e.session, 'old');
});

test('real editor failed saves and composition prevent history departure without losing drafts', async () => {
  const block = { id: 'b', page_id: 'a', parent_id: null, position: 0, content: 'saved', task_status: null };
  let fail = false;
  const e = new EditorController({
    async page() { return { blocks: [{ ...block }], revisions: { b: 0 }, can_undo: false, can_redo: false }; },
    async apply(session, pageId, action) {
      if (fail) throw new Error('locked');
      block.content = action.content;
      return { changed: [{ ...block }], removed: [], focus_id: 'b', revisions: { b: 1 }, can_undo: true, can_redo: false };
    },
  }, () => {}, { debounce: 10000, maxWait: 20000, groupGap: 1000 });
  const h = new NavigationHistory(); h.reset('session'); e.reset('session');
  await h.visit(page('a'), d => e.load(d.id));
  e.setDraft('b', 'سلام retained'); fail = true;
  await assert.rejects(h.visit(page('other'), d => e.load(d.id)), /locked/);
  assert.equal(h.current.id, 'a'); assert.equal(e.pageId, 'a'); assert.equal(e.text('b'), 'سلام retained');
  fail = false; await e.retry(); e.composition(true);
  await assert.rejects(h.visit({ kind: 'journal', date: '2024-03-20' }, () => e.unload()), /composition/);
  e.composition(false); await e.flush();
  assert.equal(h.current.id, 'a'); assert.equal(h.length, 1); assert.equal(block.content, 'سلام retained');
});
