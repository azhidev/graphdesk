import test from 'node:test';
import assert from 'node:assert/strict';
import { EditorController } from '../../src/lib/editor/controller.ts';
import { visibleRows } from '../../src/lib/editor/outline.ts';
import { keyIntent } from '../../src/lib/editor/keyboard.ts';

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const block = (id, content = '', parent_id = null, position = 0) => ({ id, content, parent_id, position, page_id: 'page', task_status: null });
const result = (changed = [], focus_id = null, removed = []) => ({ changed, removed, focus_id, can_undo: true, can_redo: false });
function fixture(blocks = [block('a', 'old')], timing = { debounce: 10_000, maxWait: 20_000, groupGap: 1000 }) {
  const calls = [];
  let stored = structuredClone(blocks);
  const transport = {
    async page(session, pageId) { calls.push(['page', session, pageId]); return { blocks: structuredClone(stored), can_undo: false, can_redo: false }; },
    async apply(session, pageId, action, group) {
      calls.push(['apply', session, pageId, structuredClone(action), group]);
      assert.equal(action.kind, 'save', 'test transport only implements text saves');
      const target = stored.find(b => b.id === action.id);
      assert.equal(target.content, action.expected);
      target.content = action.content;
      return result([structuredClone(target)], target.id);
    },
    async history(session, pageId, redo) { calls.push(['history', session, pageId, redo]); return result(); },
  };
  const editor = new EditorController(transport, () => {}, timing);
  editor.reset('session');
  return { editor, calls, transport, stored };
}

test('presentation traversal uses sibling positions, hides descendants, handles a deep outline iteratively', () => {
  const blocks = [block('b', '', null, 1), block('child', '', 'a'), block('a'), block('grandchild', '', 'child')];
  assert.deepEqual(visibleRows(blocks, new Set()).map(r => [r.block.id, r.depth]), [['a',0],['child',1],['grandchild',2],['b',0]]);
  assert.deepEqual(visibleRows(blocks, new Set(['a'])).map(r => r.block.id), ['a','b']);
  const deep = Array.from({ length: 1000 }, (_, i) => block(String(i), 'سلام😀', i ? String(i - 1) : null));
  const start = performance.now();
  assert.equal(visibleRows(deep, new Set()).length, 1000);
  console.log(`1000-block presentation traversal: ${(performance.now() - start).toFixed(2)} ms`);
});

const key = (key, overrides = {}) => keyIntent({ key, shiftKey:false,ctrlKey:false,metaKey:false,altKey:false,isComposing:false,start:0,end:0,length:5,nested:false,...overrides });
test('approved keyboard contract preserves native selection, multiline and bidi movement', () => {
  assert.equal(key('Enter'), 'split'); assert.equal(key('Enter',{shiftKey:true}), null);
  assert.equal(key('Tab'), 'indent'); assert.equal(key('Tab',{shiftKey:true}), 'outdent');
  assert.equal(key('Unidentified',{code:'Tab',keyCode:9,shiftKey:true}), 'outdent');
  assert.equal(key('Unidentified',{code:'Tab',keyCode:9,shiftKey:true,isComposing:true}), null);
  assert.equal(key('Unidentified',{shiftKey:true}), null);
  assert.equal(key('ArrowUp'), 'up'); assert.equal(key('ArrowUp',{start:2,end:2}), null);
  assert.equal(key('ArrowDown',{start:5,end:5}), 'down');
  assert.equal(key('ArrowDown',{start:0,end:3}), null);
  assert.equal(key('ArrowLeft'), null); assert.equal(key('ArrowRight'), null);
  assert.equal(key('Backspace'), 'up');
  assert.equal(key('Backspace',{length:0}), 'remove_empty');
  assert.equal(key('Backspace',{length:0,nested:true}), 'outdent');
  assert.equal(key('Backspace',{start:1,end:1}), null);
  assert.equal(key('ArrowUp',{altKey:true,shiftKey:true}), 'reorder_up');
  assert.equal(key('ArrowDown',{altKey:true,shiftKey:true}), 'reorder_down');
  assert.equal(key('z',{ctrlKey:true}), 'undo'); assert.equal(key('z',{metaKey:true,shiftKey:true}), 'redo');
  assert.equal(key('y',{ctrlKey:true}), 'redo'); assert.equal(key('Escape'), 'escape');
});

test('composition and legacy keyCode 229 never invoke structural/history commands', () => {
  for (const name of ['Enter','Tab','Backspace','ArrowUp','z']) {
    assert.equal(key(name,{isComposing:true,ctrlKey:name==='z'}),null);
    assert.equal(key(name,{keyCode:229}),null);
  }
});

test('debounced typing persists exact Persian, emoji, combining marks and Markdown source', async () => {
  const { editor, calls } = fixture(undefined,{debounce:15,maxWait:60,groupGap:1000});
  await editor.load('page');
  editor.setDraft('a','first'); editor.setDraft('a','سلام😀 e\u0301 **literal**\nline');
  assert.equal(editor.status,'Unsaved');
  await sleep(40); await editor.flush();
  assert.equal(calls.filter(c => c[0]==='apply').length,1);
  assert.equal(editor.text('a'),'سلام😀 e\u0301 **literal**\nline'); assert.equal(editor.status,'Saved');
});

test('continuous typing has a maximum save wait, not an indefinitely sliding debounce', async () => {
  const { editor, calls } = fixture(undefined,{debounce:100,maxWait:35,groupGap:1000});
  await editor.load('page');
  for (let i=0;i<7;i++) { editor.setDraft('a',String(i)); await sleep(10); }
  assert.ok(calls.some(c => c[0]==='apply'));
  await editor.flush(); assert.equal(editor.text('a'),'6');
});

test('late save acknowledgement cannot replace newer input; serial saves use acknowledged expected content', async () => {
  const { editor, calls, transport } = fixture(); await editor.load('page');
  let release; const original = transport.apply;
  transport.apply = async (...args) => { await new Promise(resolve => { release = resolve; }); return original(...args); };
  editor.setDraft('a','first'); const pending = editor.flush();
  await sleep(0); editor.setDraft('a','newer');
  transport.apply = original; release(); await pending;
  assert.equal(editor.text('a'),'newer');
  const saves = calls.filter(c => c[0]==='apply');
  assert.deepEqual(saves.map(c => [c[3].expected,c[3].content]),[['old','first'],['first','newer']]);
  assert.equal(editor.dirty,false);
});

test('storage errors retain drafts and block page/graph changes until explicit recovery', async () => {
  const { editor, transport } = fixture(); await editor.load('page'); const original = transport.apply;
  transport.apply = async () => { throw { code:'busy', message:'database locked' }; };
  editor.setDraft('a','must survive'); await assert.rejects(editor.flush());
  assert.equal(editor.text('a'),'must survive'); assert.equal(editor.dirty,true);
  assert.match(editor.status,/Error/); await assert.rejects(editor.load('other'));
  assert.equal(editor.pageId,'page'); assert.throws(() => editor.reset('new'));
  transport.apply = original; await editor.retry(); assert.equal(editor.status,'Saved');
});

test('structural operations flush before dispatch and focus the acknowledged new UUID', async () => {
  const { editor, transport, calls } = fixture(); await editor.load('page');
  const original = transport.apply;
  transport.apply = async (...args) => {
    if (args[2].kind==='save') return original(...args);
    calls.push(['split',args[2]]); return result([block('a','new'),block('new-id','text',null,1)],'new-id');
  };
  editor.setDraft('a','newtext');
  await editor.action({kind:'split',id:'a',start:3,end:3});
  assert.deepEqual(calls.filter(c => c[0]==='apply'||c[0]==='split').map(c => c[0]),['apply','split']);
  assert.equal(editor.activeId,'new-id'); assert.equal(editor.focus.start,0);
  assert.equal(editor.blocks.length,2); assert.equal(editor.working,false);
});

test('collapse moves hidden focus to parent, navigation skips collapsed descendants', async () => {
  const { editor } = fixture([block('a'),block('child','','a'),block('b','last',null,1)]);
  await editor.load('page'); editor.activate('child'); await editor.toggle('a');
  assert.equal(editor.activeId,'a'); editor.navigate('a',1); assert.equal(editor.activeId,'b');
  assert.equal(editor.focus.start,0); editor.navigate('b',-1); assert.equal(editor.activeId,'a');
  await editor.flush();
});

test('history uses the same queue, saving the newest draft before undo; selection stays frontend-only', async () => {
  const { editor, calls, transport } = fixture(); await editor.load('page');
  transport.history = async (session,page,redo) => {
    calls.push(['history',session,page,redo]); return {...result([block('a',redo?'new':'old')],'a'),can_redo:!redo};
  };
  editor.remember('a',{start:1,end:2,direction:'backward'}); editor.setDraft('a','new');
  await editor.action({kind:'undo'});
  assert.deepEqual(calls.filter(c => c[0]!=='page').map(c => c[0]),['apply','history']);
  assert.equal(editor.text('a'),'old'); assert.equal(editor.focus.direction,'backward'); assert.equal(editor.canRedo,true);
  await editor.action({kind:'redo'}); assert.equal(editor.text('a'),'new');
});

test('composition keeps drafts local and prevents close/navigation flush until committed', async () => {
  const { editor, calls } = fixture(undefined,{debounce:10,maxWait:20,groupGap:1000});
  await editor.load('page'); editor.composition(true); editor.setDraft('a','فارسی');
  await sleep(25); assert.equal(calls.filter(c=>c[0]==='apply').length,0);
  await assert.rejects(editor.flush(),/composition/);
  editor.composition(false); await editor.flush(); assert.equal(editor.text('a'),'فارسی');
});

test('explicit discard only removes drafts after a successful read; failed loads retain current view', async () => {
  const { editor, transport } = fixture(); await editor.load('page'); editor.setDraft('a','draft');
  const original = transport.page; transport.page = async () => { throw new Error('read failed'); };
  await assert.rejects(editor.discardAndReload()); assert.equal(editor.text('a'),'draft');
  assert.equal(editor.working,false); transport.page = original;
  await editor.discardAndReload(); assert.equal(editor.text('a'),'old'); assert.equal(editor.dirty,false);
  transport.page = async () => { throw new Error('missing page'); };
  await assert.rejects(editor.load('missing')); assert.equal(editor.pageId,'page'); assert.equal(editor.blocks.length,1);
});

test('repeated structural key presses cannot dispatch concurrent topology operations', async () => {
  const { editor, transport } = fixture(); await editor.load('page');
  let release; let count = 0;
  transport.apply = async () => { count++; await new Promise(resolve=>{release=resolve;}); return result(); };
  const first = editor.action({kind:'indent',id:'a'}); await sleep(0);
  await editor.action({kind:'outdent',id:'a'}); assert.equal(count,1);
  release(); await first; assert.equal(editor.working,false);
});

test('typing group ends on leaving a block, saves are scoped to current graph and page', async () => {
  const { editor, calls } = fixture(); await editor.load('page');
  editor.setDraft('a','one'); await editor.flush();
  editor.setDraft('a','two'); await editor.flush();
  editor.leave('a'); await editor.flush();
  editor.setDraft('a','three'); await editor.flush();
  const saves = calls.filter(c=>c[0]==='apply');
  assert.equal(saves[0][4],saves[1][4]); assert.notEqual(saves[1][4],saves[2][4]);
  assert.ok(saves.every(c=>c[1]==='session'&&c[2]==='page'));
});

test('unchanged drafts need no IPC, queued failure is not silently retried', async () => {
  const { editor, calls, transport } = fixture(); await editor.load('page');
  editor.setDraft('a','old'); await editor.flush(); assert.equal(calls.filter(c=>c[0]==='apply').length,0);
  let count = 0; transport.apply = async () => {count++;throw new Error('disconnected');};
  editor.setDraft('a','draft'); await assert.rejects(editor.flush());
  editor.setDraft('a','more typing'); await assert.rejects(editor.flush()); assert.equal(count,1);
  // Explicit discard is safe and cancels all draft timers.
  await editor.discardAndReload();
});
