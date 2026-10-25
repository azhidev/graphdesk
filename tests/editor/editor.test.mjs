import test from 'node:test';
import assert from 'node:assert/strict';
import { EditorController } from '../../src/lib/editor/controller.ts';
import { visibleRows } from '../../src/lib/editor/outline.ts';
import { keyIntent } from '../../src/lib/editor/keyboard.ts';
import { localDate } from '../../src/lib/dates.ts';
import { SearchController } from '../../src/lib/search.ts';
import { completion, suggestions, insertReference } from '../../src/lib/references.ts';

test('reference completion preserves Unicode source and uses bounded literal title suggestions', () => {
  const text = '😀 [[سل]] tail';
  const caret = text.indexOf(']]');
  assert.deepEqual(completion(text, caret), { start: 3, query: 'سل' });
  assert.deepEqual(insertReference(text, caret, 'سلام'), { content: '😀 [[سلام]] tail', caret: 11 });
  assert.deepEqual(insertReference('[[Old title]] tail', 5, 'New'), { content: '[[New]] tail', caret: 7 });
  assert.equal(completion('[[bad\ntext', 10), null);
  assert.equal(completion('[[Closed]]', 10), null);
  assert.deepEqual(suggestions([{id:'1',title:'سلام'}, {id:'2',title:'Other'}], 'سل').map(p => p.id), ['1']);
  assert.equal(suggestions(Array.from({length:20}, (_,i) => ({id:String(i),title:`Page ${i}`})), '').length, 8);
});

test('search rejects stale successes/errors and graph-switch responses', async () => {
  const requests = [];
  const search = new SearchController((session, text) => new Promise((resolve, reject) => requests.push({ session, text, resolve, reject })), () => {}, 0);
  const old = search.query('one', 'old');
  const latest = search.query('one', 'سلام');
  requests[1].resolve([{ id: 'b', page_id: 'p', kind: 'block', text: 'سلام' }]); await latest;
  requests[0].reject(new Error('stale error')); await old;
  assert.equal(search.hits[0].text, 'سلام'); assert.equal(search.error, '');
  const switched = search.query('one', 'pending'); search.reset();
  requests[2].resolve([{ id: 'stale' }]); await switched;
  assert.deepEqual(search.hits, []); assert.equal(search.pending, false);
  await search.query('two', '  '); assert.equal(requests.length, 3);
});

test('matching blocks reveal collapsed ancestors and keep UUID focus', async () => {
  const { editor } = fixture([block('a'), block('b', 'سلام', 'a')]);
  await editor.load('page'); editor.collapsed.add('a');
  editor.reveal('b'); assert.equal(editor.activeId, 'b'); assert.equal(editor.collapsed.has('a'), false);
  assert.throws(() => editor.reveal('missing'), /no longer exists/);
});

test('journal date uses local civil fields rather than UTC serialization', () => {
  assert.equal(localDate(new Date(2024, 1, 29, 23, 59)), '2024-02-29');
});

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
      assert.equal(action.kind, 'save_text', 'test transport only implements guarded text saves');
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
    if (args[2].kind==='save_text') return original(...args);
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

test('task status changes flush drafts and share history/acknowledgement queue', async () => {
  const { editor, transport, calls } = fixture();
  await editor.load('page');
  editor.setDraft('a', 'saved before task');
  const save = transport.apply;
  transport.apply = async (session, page, action, group) => {
    if (action.kind !== 'task') return save(session, page, action, group);
    calls.push(['task', action]);
    assert.equal(editor.text('a'), 'saved before task'); assert.equal(editor.dirty, false);
    return result([{...block('a', 'saved before task'), task_status: action.status}], 'a');
  };
  await editor.action({ kind:'task', id:'a', expected:null, status:'doing' });
  assert.equal(editor.blocks[0].task_status, 'doing');
  assert.deepEqual(calls.map(c => c[0]), ['page', 'apply', 'task']);
  await editor.action({kind:'undo'}); assert.equal(calls.at(-1)[0], 'history');
  transport.apply = async () => { throw new Error('database is locked'); };
  await assert.rejects(editor.action({kind:'task',id:'a',expected:'doing',status:'done'}), /locked/);
  assert.equal(editor.blocks[0].task_status, 'doing');
  assert.match(editor.error, /locked/);
});

test('deletion flushes newest text before preview, locks editing through confirmation and focuses acknowledgement', async () => {
  const { editor, transport, calls } = fixture([block('a'), block('child', 'nested', 'a'), block('b', 'next', null, 1)]);
  await editor.load('page'); editor.activate('a'); editor.setDraft('a', 'newest');
  transport.prepareDelete = async (session, page, id) => {
    calls.push(['preview', session, page, id]);
    assert.equal(editor.dirty, false); assert.equal(editor.text(id), 'newest');
    return { token: 'preview-token', count: 2, requires_confirmation: true };
  };
  const save = transport.apply;
  transport.apply = async (...args) => {
    if (args[2].kind !== 'delete') return save(...args);
    calls.push(['delete', args[2]]);
    return result([], 'b', ['a', 'child']);
  };
  let answer;
  const deleting = editor.deleteSubtree('a', async preview => {
    assert.equal(preview.count, 2);
    return new Promise(resolve => { answer = resolve; });
  });
  await sleep(0);
  editor.setDraft('a', 'must not edit while confirming');
  await editor.action({ kind: 'indent', id: 'b' });
  await assert.rejects(editor.load('other')); assert.throws(() => editor.reset('other'));
  assert.equal(editor.text('a'), 'newest'); assert.equal(editor.working, true);
  answer(true); await deleting;
  assert.deepEqual(calls.map(c => c[0]), ['page', 'apply', 'preview', 'delete']);
  assert.deepEqual(calls.at(-1)[1], { kind: 'delete', id: 'a', token: 'preview-token' });
  assert.equal(editor.activeId, 'b'); assert.deepEqual(editor.blocks.map(b => b.id), ['b']);
});

test('deletion cancellation, empty leaf, failed flush and uncertain writes never discard or replay silently', async () => {
  const { editor, transport } = fixture(); await editor.load('page'); editor.activate('a');
  const focusSerial = editor.focus.serial;
  editor.remember('a', { start: 1, end: 2, direction: 'backward' });
  let confirmations = 0, writes = 0, previews = 0;
  transport.prepareDelete = async () => { previews++; return {token:'token',count:1,requires_confirmation:true}; };
  transport.apply = async () => { writes++; throw new Error('uncertain deletion'); };
  await editor.deleteSubtree('a', async () => { confirmations++; return false; });
  assert.equal(writes, 0); assert.equal(editor.blocks.length, 1); assert.equal(editor.working, false);
  assert.ok(editor.focus.serial > focusSerial, 'cancel explicitly restores native text focus');
  assert.equal(editor.focus.start, 1); assert.equal(editor.focus.end, 2); assert.equal(editor.focus.direction, 'backward');
  transport.prepareDelete = async () => { previews++; return {token:'token',count:1,requires_confirmation:false}; };
  await assert.rejects(editor.deleteSubtree('a', async () => { confirmations++; return true; }), /uncertain/);
  assert.equal(confirmations, 1); assert.equal(writes, 1); assert.equal(editor.blocks.length, 1);
  await editor.retry(); assert.equal(writes, 1);
  editor.setDraft('a','retained');
  await assert.rejects(editor.deleteSubtree('a', async () => true));
  assert.equal(editor.text('a'), 'retained'); assert.equal(previews, 2);
  await editor.discardAndReload();
});

test('composition-finalization race rejects navigation synchronously before locking out final Persian input', async () => {
  const {editor, calls} = fixture(); await editor.load('page'); editor.activate('a');
  editor.composition(true); editor.setDraft('a', 'س');
  const leaving = editor.load('other');
  assert.equal(editor.working, false);
  editor.setDraft('a', 'سلام\u200cدنیا 😀'); editor.composition(false);
  await assert.rejects(leaving, /composition/);
  await editor.flush();
  assert.equal(editor.pageId, 'page'); assert.equal(editor.text('a'), 'سلام\u200cدنیا 😀');
  assert.deepEqual(calls.filter(c=>c[0]==='apply').map(c=>c[3].content), ['سلام\u200cدنیا 😀']);
});

test('composition beginning during an awaited navigation blocks departure and preserves final input', async () => {
  const {editor, calls} = fixture(); await editor.load('page');
  const leaving = editor.load('other');
  editor.composition(true); editor.setDraft('a', 'پیش');
  await assert.rejects(leaving, /composition/);
  editor.setDraft('a', 'پایان'); editor.composition(false); await editor.flush();
  assert.equal(editor.pageId, 'page');
  assert.deepEqual(calls.filter(c=>c[0]==='apply').map(c=>c[3].content), ['پایان']);
});

test('composition blocks structural/discard operations and active-input replacement without persisting preedit', async () => {
  const {editor,calls} = fixture([block('a'),block('b','next',null,1)]); await editor.load('page'); editor.activate('a');
  editor.composition(true); editor.setDraft('a','پیش'); editor.activate('b');
  assert.equal(editor.activeId, 'a');
  for (const operation of [() => editor.action({kind:'split',id:'a',start:0,end:0}), () => editor.toggle('a'),
    () => editor.deleteSubtree('a', async()=>true), () => editor.discardAndReload(), () => editor.flush()]) {
    await assert.rejects(operation(), /composition/); assert.equal(editor.working,false);
  }
  await editor.capture(); assert.equal(calls.filter(c=>c[0]==='apply').length,0);
  editor.setDraft('a','نهایی'); editor.composition(false); await editor.flush();
});

function reliableFixture() {
  let stored = block('a', 'old'), revision = 0, lose = false;
  const writes = [];
  const transport = {
    async page() { return {blocks:[structuredClone(stored)],revisions:{a:revision},can_undo:false,can_redo:false}; },
    async apply(_s,_p,action) {
      assert.equal(action.kind,'save_text');
      if (action.revision !== revision || action.expected !== stored.content) throw new Error('conflict');
      if (action.content !== stored.content) { stored.content=action.content; revision++; writes.push(action.content); }
      if (lose) {lose=false;throw new Error('response lost after commit');}
      return {...result([structuredClone(stored)],'a'),revisions:{a:revision}};
    },
  };
  const snapshots = new Map();
  const recovery = {
    async put(_s,_p,id,base,rev,content) { snapshots.set(`${id}:${rev}`,{base,content}); return {}; },
    async clear(_s,id,content) { for (const [key,value] of snapshots) if(key.startsWith(`${id}:`) && value.content===content) snapshots.delete(key); },
  };
  const editor = new EditorController(transport,()=>{}, {debounce:10000,maxWait:20000,groupGap:1000}, recovery);
  editor.reset('session');
  return {editor,transport,recovery,writes,snapshots,lose:()=>lose=true,external:content=>{stored.content=content;revision++;}};
}

test('lost save acknowledgement reconciles revision and keeps newer draft without a duplicate write', async () => {
  const f=reliableFixture(); await f.editor.load('page'); f.lose(); f.editor.setDraft('a','committed');
  await assert.rejects(f.editor.flush(), /response lost/);
  f.editor.setDraft('a','newer'); await f.editor.retry();
  assert.deepEqual(f.writes,['committed','newer']); assert.equal(f.editor.text('a'),'newer');
  assert.equal(f.editor.revisions.a,2); assert.equal(f.editor.dirty,false); assert.equal(f.snapshots.size,0);
});

test('ambiguous response plus concurrent/ABA changes requires explicit comparison and guarded replacement', async () => {
  const f=reliableFixture(); await f.editor.load('page'); f.lose(); f.editor.setDraft('a','sent');
  await assert.rejects(f.editor.flush()); f.editor.setDraft('a','mine');
  f.external('other'); f.external('sent');
  await assert.rejects(f.editor.retry(), /Compare/);
  assert.equal(f.editor.conflicts[0].block.content,'sent'); assert.equal(f.editor.text('a'),'mine');
  assert.deepEqual(f.writes,['sent']);
  await f.editor.resolveConflict('a');
  assert.deepEqual(f.writes,['sent','mine']); assert.equal(f.editor.dirty,false);
});

test('recovery captures finalized Unicode only, preserves newer snapshots on late cleanup and reports failure', async () => {
  const f=reliableFixture(); await f.editor.load('page'); f.editor.composition(true); f.editor.setDraft('a','پیش');
  await f.editor.capture(); assert.equal(f.snapshots.size,0);
  f.editor.setDraft('a','نهایی\n😀'); f.editor.composition(false); await f.editor.capture();
  assert.equal([...f.snapshots.values()][0].content,'نهایی\n😀'); assert.match(f.editor.protection,/stored locally/);
  const original=f.recovery.put; f.recovery.put=async()=>{throw new Error('injected full disk');};
  f.editor.setDraft('a','newer'); await assert.rejects(f.editor.capture(),/not protected/);
  assert.match(f.editor.protectionError,/not protected/); assert.equal(f.editor.text('a'),'newer');
  f.recovery.put=original; await f.editor.capture(); assert.equal(f.editor.protectionError,'');
  await f.editor.flush(); assert.equal(f.snapshots.size,0);
});

test('normal recovery capture has a bounded wait during continuous unsaved typing', async () => {
  const f=reliableFixture(); await f.editor.load('page');
  for(let i=0;i<9;i++){f.editor.setDraft('a',`text ${i}`);await sleep(100);}
  assert.ok(f.snapshots.size>0,'capture must not slide forever');
  await f.editor.flush(); assert.equal(f.editor.dirty,false);
});

test('a response that never arrives times out and reconciles the committed revision without replaying it', async () => {
  const f=reliableFixture();
  const editor=new EditorController(f.transport,()=>{}, {debounce:10000,maxWait:20000,groupGap:1000,ackTimeout:15}, f.recovery);
  editor.reset('session'); await editor.load('page');
  const original=f.transport.apply;
  f.transport.apply=async(...args)=>{await original(...args);return new Promise(()=>{});};
  editor.setDraft('a','committed'); await assert.rejects(editor.flush(),/timed out/);
  editor.setDraft('a','newer'); f.transport.apply=original;
  await editor.retry(); assert.deepEqual(f.writes,['committed','newer']); assert.equal(editor.dirty,false);
});

test('composition arriving during recovery IO aborts navigation without looping or saving preedit', async () => {
  const f=reliableFixture(); await f.editor.load('page'); f.editor.setDraft('a','final before preedit');
  const original=f.recovery.put; let release;
  f.recovery.put=async(...args)=>{await new Promise(resolve=>release=resolve); return original(...args);};
  const leaving=f.editor.load('other'); await sleep(0);
  f.editor.composition(true); f.editor.setDraft('a','پیش'); release();
  await assert.rejects(leaving,/composition/); assert.equal(f.writes.length,0);
  f.recovery.put=original; f.editor.setDraft('a','متن نهایی'); f.editor.composition(false);
  await f.editor.retry(); assert.deepEqual(f.writes,['متن نهایی']);
});

test('a complete composition cycle during page-read IO cannot hide the finalized draft on another page', async () => {
  const f=reliableFixture(); await f.editor.load('page'); let release;
  const original=f.transport.page;
  f.transport.page=async(...args)=>{await new Promise(resolve=>release=resolve);return original(...args);};
  const leaving=f.editor.load('other'); await sleep(0);
  f.editor.composition(true); f.editor.setDraft('a','پیش');
  f.editor.setDraft('a','نهایی'); f.editor.composition(false); release();
  await assert.rejects(leaving,/composition/);
  assert.equal(f.editor.pageId,'page'); assert.equal(f.editor.text('a'),'نهایی');
  f.transport.page=original; await f.editor.flush(); assert.deepEqual(f.writes,['نهایی']);
});
