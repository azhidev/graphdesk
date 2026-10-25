import test from 'node:test';
import assert from 'node:assert/strict';
import { adjacentDay, gregorianPresentation } from '../../src/lib/dates.ts';
import { SearchController } from '../../src/lib/search.ts';
import { TaskController } from '../../src/lib/tasks.ts';
import { EditorController } from '../../src/lib/editor/controller.ts';

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
test('civil date navigation handles leap years, DST days and year boundaries without changing identity', () => {
  for (const [day, delta, expected] of [
    ['2024-02-28', 1, '2024-02-29'], ['2024-03-01', -1, '2024-02-29'],
    ['2025-02-28', 1, '2025-03-01'], ['2026-12-31', 1, '2027-01-01'],
    ['2026-01-01', -1, '2025-12-31'], ['2026-03-08', 1, '2026-03-09'],
  ]) assert.equal(adjacentDay(day, delta), expected);
  assert.equal(gregorianPresentation.label('2024-02-29'), '2024-02-29');
  assert.equal(adjacentDay('0001-01-01', -1), '0000-12-31');
  for (const day of ['2023-02-29', '2024-2-29', 'invalid']) assert.throws(() => adjacentDay(day, 1));
  assert.throws(() => adjacentDay('9999-12-31', 1));
  assert.throws(() => adjacentDay('0000-01-01', -1));
});

test('search debounces dispatch and clearing or graph changes cancel pending work', async () => {
  const calls = [];
  const search = new SearchController(async (session, text) => { calls.push([session, text]); return []; }, () => {}, 20);
  const first = search.query('a', 'س');
  const second = search.query('a', 'سلام');
  assert.equal(search.pending, true);
  await Promise.all([first, second]);
  assert.deepEqual(calls, [['a', 'سلام']]);
  const pending = search.query('a', 'old'); search.reset(); await pending;
  await sleep(25); assert.equal(calls.length, 1);
  await search.query('b', 'new'); assert.deepEqual(calls.at(-1), ['b', 'new']);
  await search.query('b', ' '); assert.deepEqual(search.hits, []); assert.equal(search.pending, false);
});

test('tasks reject stale successes and failures across graph/filter changes; paging stays bounded', async () => {
  const requests = [];
  const tasks = new TaskController((...args) => new Promise((resolve, reject) => requests.push({ args, resolve, reject })));
  const old = tasks.load('one', 'active', null);
  const current = tasks.load('two', 'doing', 'page');
  requests[1].resolve(Array.from({ length: 100 }, (_, i) => ({ id: String(i) }))); await current;
  requests[0].reject(new Error('old failure')); await old;
  assert.equal(tasks.hits.length, 100); assert.equal(tasks.error, ''); assert.equal(tasks.more, true);
  const next = tasks.next();
  assert.deepEqual(requests[2].args, ['two', 'doing', 'page', 100]);
  requests[2].resolve([{ id: '100' }]); await next;
  assert.equal(tasks.hits.length, 101); assert.equal(tasks.more, false);
  const stale = tasks.load('two', 'done', null);
  const latest = tasks.load('two', 'todo', null);
  requests[4].resolve([]); await latest;
  requests[3].resolve([{ id: 'stale' }]); await stale;
  assert.deepEqual(tasks.hits, []); assert.equal(tasks.pending, false);
});

test('date-only preview unload flushes drafts and refuses failures or composition without losing the page', async () => {
  let fail = false;
  const stored = { id: 'block', page_id: 'page', parent_id: null, position: 0, content: 'old', task_status: null };
  const editor = new EditorController({
    async page() { return { blocks: [{ ...stored }], revisions: { block: 0 }, can_undo: false, can_redo: false }; },
    async apply(session, page, action) {
      if (fail) throw new Error('locked');
      stored.content = action.content;
      return { changed: [{ ...stored }], removed: [], focus_id: 'block', revisions: { block: 1 }, can_undo: true, can_redo: false };
    },
  }, () => {}, { debounce: 10000, maxWait: 20000, groupGap: 1000 });
  editor.reset('session'); await editor.load('page');
  editor.setDraft('block', 'سلام'); fail = true;
  await assert.rejects(editor.unload(), /locked/);
  assert.equal(editor.pageId, 'page'); assert.equal(editor.text('block'), 'سلام');
  fail = false; await editor.retry();
  editor.composition(true); await assert.rejects(editor.unload(), /composition/);
  editor.composition(false); await editor.unload();
  assert.equal(stored.content, 'سلام'); assert.equal(editor.pageId, null); assert.deepEqual(editor.blocks, []);
  assert.equal(editor.session, 'session');
});

test('a composition cycle during preview departure retains finalized text on its original page', async () => {
  let release;
  let entered;
  const capturing = new Promise(resolve => entered = resolve);
  const block = { id: 'b', page_id: 'p', parent_id: null, position: 0, content: 'old', task_status: null };
  const editor = new EditorController({
    async page() { return { blocks: [{ ...block }], revisions: { b: 0 }, can_undo: false, can_redo: false }; },
    async apply(session, page, action) {
      block.content = action.content;
      return { changed: [{ ...block }], removed: [], focus_id: 'b', revisions: { b: 1 }, can_undo: true, can_redo: false };
    },
  }, () => {}, { debounce: 10000, maxWait: 20000, groupGap: 1000 }, {
    async put() { entered(); await new Promise(resolve => release = resolve); }, async clear() {},
  });
  editor.reset('session'); await editor.load('p'); editor.activate('b');
  editor.setDraft('b', 'before');
  const leaving = editor.unload(); await capturing;
  editor.composition(true); editor.setDraft('b', 'سلام'); editor.composition(false);
  release(); await assert.rejects(leaving, /composition/);
  assert.equal(editor.pageId, 'p'); assert.equal(editor.text('b'), 'سلام'); assert.equal(block.content, 'old');
  // Dispose the fixture through its normal guarded path, releasing any new capture.
  const flush = editor.flush(); await new Promise(resolve => setImmediate(resolve)); release(); await flush;
  assert.equal(block.content, 'سلام');
});

test('task pagination failures keep acknowledged rows and retry the same offset', async () => {
  const offsets = [];
  let fail = false;
  const tasks = new TaskController(async (session, status, page, offset) => {
    offsets.push(offset);
    if (fail) throw new Error('busy');
    return offset ? [{ id: 'last' }] : Array.from({ length: 100 }, (_, i) => ({ id: String(i) }));
  });
  await tasks.load('s', 'active', null); fail = true; await tasks.next();
  assert.equal(tasks.hits.length, 100); assert.equal(tasks.error, 'busy'); assert.equal(tasks.more, true);
  fail = false; await tasks.next();
  assert.deepEqual(offsets, [0, 100, 100]); assert.equal(tasks.hits.length, 101); assert.equal(tasks.error, '');
});
