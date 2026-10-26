import test from 'node:test';
import assert from 'node:assert/strict';
import { calendarDate, calendarFields, adjacentDay, dateLabel, dateNumber, asciiDigits, localDate, displayPageTitle } from '../../src/lib/dates.ts';
import { messages, translate, errorText } from '../../src/lib/i18n.ts';
import { keyIntent, shortcutLetter } from '../../src/lib/editor/keyboard.ts';
import { FirstNoteController } from '../../src/lib/firstNote.ts';

test('Jalali Nowruz, Esfand leap days, month ends and digit input share one conversion authority', () => {
  for (const [iso, fields] of [
    ['2024-03-20', {year:1403,month:1,day:1}], ['2025-03-20', {year:1403,month:12,day:30}],
    ['2025-03-21', {year:1404,month:1,day:1}], ['2021-03-20', {year:1399,month:12,day:30}],
    ['2024-09-21', {year:1403,month:6,day:31}], ['2024-09-22', {year:1403,month:7,day:1}],
  ]) { assert.deepEqual(calendarFields(iso, 'jalali'), fields); assert.equal(calendarDate(fields, 'jalali'), iso); }
  assert.equal(dateNumber('۱۴۰۳'), 1403); assert.equal(dateNumber('١٢'), 12);
  assert.equal(asciiDigits('۰۱۲۳۴۵۶۷۸۹ ٠١٢٣٤٥٦٧٨٩'), '0123456789 0123456789');
  assert.equal(dateLabel('2024-03-20', 'jalali', 'fa'), '۱۴۰۳/۰۱/۰۱');
  assert.equal(dateLabel('2024-03-20', 'gregorian', 'en'), '2024-03-20');
  assert.throws(() => calendarDate({year:1402,month:12,day:30}, 'jalali'));
  assert.throws(() => calendarDate({year:1403,month:7,day:31}, 'jalali'));
  assert.throws(() => calendarDate({year:2023,month:2,day:29}, 'gregorian'));
  assert.equal(calendarDate({year:2024,month:2,day:29}, 'gregorian'), '2024-02-29');
  assert.throws(() => calendarDate({year:1900,month:2,day:29}, 'gregorian'));
  assert.equal(calendarDate({year:2000,month:2,day:29}, 'gregorian'), '2000-02-29');
  assert.throws(() => calendarDate({year:5000,month:1,day:1}, 'jalali'), { name: 'unsupported' });
  for (const value of ['1.5', '-1', '1e2', '۱۴۰۳/۱', '']) assert.throws(() => dateNumber(value));
});

test('calendar range boundaries and date-only round trips never change the stored identity', () => {
  for (const iso of ['1800-01-01', '1800-12-31', '2256-01-01', '2256-12-31']) assert.equal(calendarDate(calendarFields(iso, 'jalali'), 'jalali'), iso);
  for (const iso of ['1799-12-31', '2257-01-01']) {
    assert.throws(() => calendarFields(iso, 'jalali'), { name: 'unsupported' });
    assert.equal(calendarDate(calendarFields(iso, 'gregorian'), 'gregorian'), iso);
  }
  let iso = '1800-01-01';
  for (; iso <= '2256-12-31'; iso = adjacentDay(iso, 1)) {
    assert.equal(calendarDate(calendarFields(iso, 'jalali'), 'jalali'), iso);
  }
  const date = new Date(2024, 2, 20, 0, 1); assert.equal(localDate(date), '2024-03-20');
  const page={id:'stable-id',title:'2024-03-20',journal_date:'2024-03-20'};
  assert.equal(displayPageTitle(page,'jalali','en'),'1403/01/01');
  assert.equal(displayPageTitle({...page,title:'Renamed فارسی'},'jalali','fa'),'Renamed فارسی');
  assert.equal(displayPageTitle(page,'gregorian','en'),'2024-03-20');
  assert.deepEqual(page,{id:'stable-id',title:'2024-03-20',journal_date:'2024-03-20'});
  assert.equal(displayPageTitle({title:'1700-01-01',journal_date:'1700-01-01'},'jalali','fa'),'1700-01-01');
});

test('translations cover every declared key, preserve content interpolation and have a safe fallback', () => {
  for (const [key, text] of Object.entries(messages)) {
    assert.ok(text.trim()); assert.equal(translate('en', key), key);
    assert.deepEqual([...key.matchAll(/\{(\w+)\}/g)].map(m => m[1]).sort(), [...text.matchAll(/\{(\w+)\}/g)].map(m => m[1]).sort());
  }
  assert.equal(translate('en', 'unknown future key'), 'unknown future key');
  assert.equal(translate('fa', 'unknown future key'), 'unknown future key');
  assert.equal(translate('fa', '__proto__'), '__proto__');
  assert.equal(translate('fa', 'toString'), 'toString');
  assert.match(translate('fa', 'Delete {count} block(s)?', {count:12}), /۱۲/);
  const content = 'فارسی GraphDesk ۱۲۳ [x]';
  assert.match(translate('fa', 'First note for {date}', {date:content}), new RegExp(content.replace('[x]', '\\[x\\]')));
  assert.notEqual(errorText('fa', 'unknown storage error'), 'unknown storage error');
  assert.equal(errorText('fa', 'edit conflict: remembered path now contains a different graph'), translate('fa', 'remembered path now contains a different graph'));
  assert.equal(errorText('fa', 'migration: remembered graph requires an explicit upgrade'), translate('fa', 'This graph needs an explicit open. Automatic reopening never upgrades old or unsupported graph formats.'));
});

test('Persian layout shortcut fallback does not reinterpret native movement or IME preedit', () => {
  assert.equal(shortcutLetter('ن', 'KeyK'), 'k'); assert.equal(shortcutLetter('a', 'KeyQ'), 'a');
  const key = {key:'ظ',code:'KeyZ',ctrlKey:true,metaKey:false,altKey:false,shiftKey:false,isComposing:false,start:0,end:0,length:4,nested:false};
  assert.equal(keyIntent(key), 'undo'); assert.equal(keyIntent({...key,isComposing:true}), null);
  assert.equal(keyIntent({...key,keyCode:229}), null); assert.equal(keyIntent({...key,altKey:true}), null);
});

function fixture(options = {}) {
  let stored = null, submitted = null, version = 0, fail = false, lose = false;
  const calls = [];
  const transport = {
    async put(session,id,block_id,journal_date,content) {
      calls.push(['put',content]); if (fail) throw new Error('storage');
      return stored = {id,block_id,journal_date,content,version:++version,updated_at:1,graph_id:'g'};
    },
    async remove(session,id,v) { calls.push(['remove',v]); assert.equal(stored.version,v); stored = null; },
    async submit(session,date,id,content) {
      calls.push(['submit',id,content]);
      if (submitted && submitted.block.content !== content) throw new Error('conflict');
      submitted ??= {page:{id:'p',title:date,journal_date:date},block:{id,page_id:'p',parent_id:null,position:0,content,task_status:null},revision:0};
      if (lose) { lose=false; throw new Error('lost acknowledgement'); }
      return submitted;
    },
  };
  const note = new FirstNoteController('session','2024-03-20',transport,()=>{},undefined,options.delay ?? 10,options.timeout ?? 30);
  return {note,transport,calls,get stored(){return stored},get submitted(){return submitted},set fail(value){fail=value},set lose(value){lose=value}};
}
const sleep = ms => new Promise(resolve => setTimeout(resolve,ms));

test('first-note capture excludes preedit, is bounded during typing and creates no canonical data', async () => {
  const f=fixture(); f.note.composition(true); f.note.input('پیش'); await sleep(20);
  assert.equal(f.stored,null); f.note.input('سلام English ۱۲۳ 😀'); f.note.composition(false);
  await sleep(20); assert.equal(f.stored.content,'سلام English ۱۲۳ 😀'); assert.equal(f.submitted,null); assert.equal(f.note.protected,true);
  f.note.input('new'); await f.note.flush(); assert.equal(f.stored.content,'new'); await f.note.discard(); assert.equal(f.stored,null);
});

test('first-note lost acknowledgement retries the same intended UUID without discarding private text', async () => {
  const f=fixture(); f.note.input('محفوظ'); f.lose=true;
  await assert.rejects(f.note.submit(),/lost/); assert.equal(f.stored.content,'محفوظ');
  const result=await f.note.submit(); assert.equal(result.block.id,f.note.blockId); assert.equal(f.stored,null);
  assert.deepEqual(f.calls.filter(c=>c[0]==='submit').map(c=>c[1]),[f.note.blockId,f.note.blockId]);
});

test('first-note capture failure is visible, submit can still save, and changed commits are never overwritten', async () => {
  const f=fixture(); f.fail=true; f.note.input('first'); await assert.rejects(f.note.flush()); assert.equal(f.note.protected,false);
  f.lose=true; await assert.rejects(f.note.submit()); f.fail=false; f.note.input('newer'); await f.note.flush();
  await assert.rejects(f.note.submit(),/conflict/); assert.equal(f.submitted.block.content,'first'); assert.equal(f.stored.content,'newer');
  await f.note.discard();
});

test('first-note composition during capture prevents submission and preserves finalized text', async () => {
  const f=fixture({delay:10000}); let release;
  f.transport.put=async (...args)=>{ await new Promise(resolve=>release=resolve); return {version:1}; };
  f.note.input('before'); const saving=f.note.submit(); await new Promise(resolve=>setImmediate(resolve));
  f.note.composition(true); f.note.input('نهایی'); f.note.composition(false); release();
  await assert.rejects(saving,/composition/); assert.equal(f.submitted,null); assert.equal(f.note.content,'نهایی');
  f.transport.put=async()=>({version:2}); f.transport.remove=async()=>{}; await f.note.discard();
});

test('continuous first-note input does not indefinitely postpone private capture', async () => {
  const f=fixture({delay:20});
  for (let i=0;i<10;i++) { f.note.input(String(i)); await sleep(5); }
  assert.ok(f.calls.some(c=>c[0]==='put')); assert.equal(f.submitted,null);
  await f.note.discard();
});

test('a never-arriving first-note acknowledgement can be retried without a new identity', async () => {
  const f=fixture({timeout:15}); const submit=f.transport.submit;
  f.transport.submit=async(...args)=>{ await submit(...args); return new Promise(()=>{}); };
  f.note.input('ثبت شده'); await assert.rejects(f.note.submit(),/timed out/);
  assert.equal(f.stored.content,'ثبت شده');
  const id=f.submitted.block.id; f.transport.submit=submit;
  await f.note.submit(); assert.equal(f.submitted.block.id,id); assert.equal(f.stored,null);
});

test('a complete composition cycle during post-submit page loading cannot silently close the form', async () => {
  const f=fixture(); f.note.input('saved'); await f.note.submit(); const stamp=f.note.stamp;
  f.note.composition(true); f.note.input('متن جدید'); f.note.composition(false);
  assert.throws(()=>f.note.assertStable(stamp),/composition/);
  await f.note.flush(); assert.equal(f.stored.content,'متن جدید'); assert.equal(f.submitted.block.content,'saved');
  await f.note.discard();
});

test('finalized composition during canonical submission survives its late acknowledgement and cleanup', async () => {
  const f=fixture({delay:5}); const submit=f.transport.submit; let release;
  f.transport.submit=async(...args)=>{ const result=await submit(...args); await new Promise(resolve=>release=resolve); return result; };
  f.note.input('sent'); const saving=f.note.submit(); await new Promise(resolve=>setImmediate(resolve));
  f.note.composition(true); f.note.input('نهایی تازه'); f.note.composition(false); await f.note.flush();
  release(); await assert.rejects(saving,/composition/);
  assert.equal(f.submitted.block.content,'sent'); assert.equal(f.stored.content,'نهایی تازه');
  assert.equal(f.calls.filter(c=>c[0]==='remove').length,0);
  await f.note.discard();
});
