import test from 'node:test';
import assert from 'node:assert/strict';
import { graphDisplayName, graphLabels, mergeGraphs, cycleCursor } from '../../src/lib/graphList.ts';
import { graphKey } from '../../src/lib/graphKeys.ts';
import { GraphSwitch } from '../../src/lib/graphSwitch.ts';
import { EditorController } from '../../src/lib/editor/controller.ts';

const graph = (path, graph_id = 'shared-copy-identity') => ({ path, graph_id });
test('created/opened session graphs stay visible without persisting paths; canonical paths deduplicate, not UUIDs', () => {
  const a = graph('/graphs/A.sqlite3'), b = graph('/graphs/B.db');
  assert.deepEqual(mergeGraphs([b, a], []), [b, a]);
  assert.deepEqual(mergeGraphs([a, b], [b, a]), [a, b]);
  const replaced = graph(a.path, 'replacement');
  assert.deepEqual(mergeGraphs([replaced], [a, b]), [replaced, b]);
  assert.equal(mergeGraphs(Array.from({ length: 12 }, (_, n) => graph(`/graphs/${n}.db`)), []).length, 10);
});

test('graph names strip database extensions and disambiguate copies with identical names and UUIDs', () => {
  assert.equal(graphDisplayName('/graphs/GraphDesk Demo.sqlite3'), 'GraphDesk Demo');
  assert.equal(graphDisplayName('C:\\گراف\\فارسی English.DB'), 'فارسی English');
  assert.equal(graphDisplayName('/graphs/no-extension'), 'no-extension');
  const copies = ['/one/work/Notes.db', '/two/work/Notes.db', '/three/other/Notes.db'].map(path => graph(path));
  const labels = graphLabels(copies);
  assert.equal(new Set(labels.values()).size, 3);
  assert.equal(labels.get(copies[2].path), 'Notes — other');
  assert.equal(graphLabels([graph('/graphs/A.db')]).get('/graphs/A.db'), 'A');
});

const key = (key, extra = {}) => ({ key, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, isComposing: false, ...extra });
const context = { session: true, open: false, blocked: false, composing: false };
test('Ctrl+Tab/reverse WebKit variants cycle only; Enter activates and Escape cancels only an open switcher', () => {
  let open = false, cursor = -1, activated = null;
  function dispatch(event) {
    const intent = graphKey(event, { ...context, open });
    if (intent === 'next' || intent === 'previous') {
      open = true; cursor = cycleCursor(cursor, intent === 'next' ? 1 : -1, 3);
    } else if (intent === 'activate') { activated = cursor; open = false; cursor = -1; }
    else if (intent === 'cancel') { open = false; cursor = -1; }
    return intent;
  }
  assert.equal(dispatch(key('Enter')), null);
  dispatch(key('Tab', { ctrlKey: true })); assert.equal(cursor, 0); assert.equal(activated, null);
  dispatch(key('Tab', { ctrlKey: true, repeat: true })); assert.equal(cursor, 1);
  dispatch(key('Tab', { ctrlKey: true, shiftKey: true })); assert.equal(cursor, 0);
  dispatch(key('ISO_Left_Tab', { ctrlKey: true })); assert.equal(cursor, 2);
  dispatch(key('Unidentified', { code: 'Tab', ctrlKey: true })); assert.equal(cursor, 0);
  dispatch(key('Enter')); assert.equal(activated, 0); assert.equal(open, false);
  dispatch(key('Unidentified', { keyCode: 9, ctrlKey: true, shiftKey: true })); assert.equal(cursor, 2);
  dispatch(key('Escape')); assert.equal(open, false); assert.equal(activated, 0);
  assert.equal(cycleCursor(-1, 1, 0), -1);
  assert.equal(cycleCursor(0, -1, 1), 0);
});

test('graph keys leave IME, AltGr, modal dialogs and busy operations untouched', () => {
  for (const extra of [{ isComposing: true }, { keyCode: 229 }, { altKey: true }, { metaKey: true },
    { defaultPrevented: true }, { getModifierState: name => name === 'AltGraph' }]) {
    assert.equal(graphKey(key('Tab', { ctrlKey: true, ...extra }), context), null);
  }
  for (const extra of [{ composing: true }, { session: false }, { blocked: true }]) {
    for (const event of [key('Tab', { ctrlKey: true }), key('Enter'), key('Escape')])
      assert.equal(graphKey(event, { ...context, open: true, ...extra }), null);
  }
  assert.equal(graphKey(key('Tab'), context), null);
  assert.equal(graphKey(key('Unidentified', { ctrlKey: true }), context), null);
});

test('A → B → A flushes real editor drafts before installing a new active identity', async () => {
  const stored = new Map(['A', 'B'].map(name => [name, {
    id: `${name}-block`, page_id: 'page', content: `${name} original`, parent_id: null, position: 0, task_status: null,
  }]));
  let active = 'A';
  const editor = new EditorController({
    async page(session) { return { blocks: [structuredClone(stored.get(session))], revisions: {}, can_undo: false, can_redo: false }; },
    async apply(session, page, action) {
      const block = stored.get(session);
      assert.equal(block.content, action.expected);
      block.content = action.content;
      return { changed: [structuredClone(block)], removed: [], focus_id: block.id, can_undo: true, can_redo: false };
    },
  });
  editor.reset(active); await editor.load('page');
  const switcher = new GraphSwitch({
    async prepare(request) { return { ticket: request }; },
    async activate(ticket) { return ticket; },
    async reconcile() { return null; }, async cancel() {},
  });
  const sessionGraphs = [graph('/A.db', 'A')];
  for (const target of ['B', 'A']) {
    const source = active;
    editor.setDraft(`${source}-block`, `${source} edited فارسی`);
    await switcher.open(target, source, {
      flush: () => editor.flush(), check() { assert.equal(active, source); }, async detach() {},
      install(session) { active = session; editor.reset(session); sessionGraphs.unshift(graph(`/${session}.db`, session)); },
    });
    await editor.load('page');
    assert.equal(editor.session, target);
    assert.equal(editor.text(`${target}-block`), stored.get(target).content);
  }
  assert.equal(stored.get('A').content, 'A edited فارسی');
  assert.equal(stored.get('B').content, 'B edited فارسی');
  assert.equal(active, 'A');
  assert.equal(mergeGraphs(sessionGraphs, []).length, 2);
  await editor.flush();
});
