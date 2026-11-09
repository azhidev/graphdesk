import test from 'node:test';
import assert from 'node:assert/strict';
import { HabitsController } from '../../src/lib/habits/controller.ts';
import { definitionInput, eventInput } from '../../src/lib/habits/forms.ts';
import { parseInstant, zonedLabel } from '../../src/lib/habits/time.ts';

const draft = { name: 'Practice', kind: 'positive', anchor: '2026-01-01T09:00:00+03:30', every: 1, unit: 'days', mode: 'fixed', zone: 'Asia/Tehran' };
const positive = { id: 'p', revision: 1, definition: definitionInput(draft, null) };
const negative = { id: 'n', revision: 1, definition: definitionInput({ ...draft, kind: 'negative', zone: '', anchor: '' }, null) };
const record = { op: 'record', input: { id: 'stable-event', habit_id: 'n', kind: 'lapse', occurred_at: 123, period_start: null, period_end: null } };
const event = { ...record.input, revision: 1, voided: false, recorded_at: 124 };
const gate = () => { let resolve, reject; const promise = new Promise((a,b) => { resolve = a; reject = b; }); return { promise, resolve, reject }; };

test('both creation paths, immutable kind and snapshot/explicit zone editing', () => {
  assert.equal(positive.definition.schedule.mode, 'fixed');
  assert.equal(positive.definition.schedule.timezone, 'Asia/Tehran');
  assert.equal(negative.definition.schedule, null);
  const edited = definitionInput({ ...draft, kind: 'negative', zone: 'America/New_York' }, positive);
  assert.equal(edited.kind, 'positive'); assert.equal(edited.schedule.anchor, positive.definition.schedule.anchor);
  assert.equal(edited.schedule.timezone, 'America/New_York');
  for (const bad of [{ name: '' }, { name: 'ع'.repeat(257) }, { every: 0 }, { every: 1.5 }, { every: 10001 }, { zone: '' }, { zone: 'bad/zone' }, { zone: '+03:30' }]) assert.throws(() => definitionInput({ ...draft, ...bad }, null));
});

test('offset-only adapter is independent of machine timezone, preserves both fold sides and explicit gap instants', () => {
  for (const zone of ['UTC', 'Pacific/Auckland', 'America/Los_Angeles']) {
    const previous = process.env.TZ; process.env.TZ = zone;
    assert.equal(parseInstant('2026-01-01T09:00:00+03:30'), Date.UTC(2026, 0, 1, 5, 30) / 1000);
    process.env.TZ = previous;
  }
  assert.equal(parseInstant('2024-11-03T01:30:00-05:00') - parseInstant('2024-11-03T01:30:00-04:00'), 3600);
  assert.match(zonedLabel(parseInstant('2024-03-10T02:30:00-05:00'), 'America/New_York', 'gregorian', 'en'), /03:30:00/);
  assert.match(zonedLabel(parseInstant('2026-03-21T00:00:00Z'), 'UTC', 'jalali', 'en'), /1405\/01\/01/);
  for (const text of ['2026-02-30T10:00Z', '2026-01-01T10:00', '2026-01-01T24:00Z', '2026-01-01T10:00:60Z', '2026-01-01T10:00+25:00']) assert.throws(() => parseInstant(text));
});

test('events reject future/invalid kinds/anchor violations; confirmations require explicit completed periods', () => {
  const now = parseInstant('2026-06-01T00:00Z');
  assert.equal(eventInput(positive, 'id', 'completion', '2026-05-01T00:00Z', '', '', now).kind, 'completion');
  assert.equal(eventInput(negative, 'id', 'lapse', '2026-05-01T00:00Z', '', '', now).period_start, null);
  const confirmed = eventInput(negative, 'id', 'confirmation', '2026-05-01T00:00Z', '2026-04-01T00:00Z', '2026-05-01T00:00Z', now);
  assert.equal(confirmed.period_end, confirmed.occurred_at);
  for (const args of [
    [positive, 'id', 'lapse', '2026-05-01T00:00Z', '', ''], [negative, 'id', 'completion', '2026-05-01T00:00Z', '', ''],
    [positive, 'id', 'completion', '2025-01-01T00:00Z', '', ''], [negative, 'id', 'lapse', '2027-01-01T00:00Z', '', ''],
    [negative, 'id', 'confirmation', '2026-05-01T00:00Z', '', ''],
    [negative, 'id', 'confirmation', '2026-05-01T00:00Z', '2026-05-02T00:00Z', '2026-05-03T00:00Z'],
    [{ ...negative, definition: { ...negative.definition, archived: true } }, 'id', 'lapse', '2026-05-01T00:00Z', '', ''],
  ]) assert.throws(() => eventInput(...args, now));
});

test('duplicate/uncertain submissions retain exact payload; acknowledgement only applies state', async () => {
  const deferred = gate(), calls = []; let fail = true;
  const model = new HabitsController({ request: async r => { calls.push(r); if (fail) return deferred.promise; return { type: 'event', data: event }; } });
  model.selected = negative;
  const saving = model.mutate(record);
  assert.equal(await model.mutate(record), false); assert.equal(calls.length, 1); assert.equal(model.events.length, 0);
  deferred.reject(new Error('lost response')); assert.equal(await saving, false);
  assert.deepEqual(model.pending, record); assert.match(model.error, /lost response/);
  record.input.occurred_at = 999; // caller cannot alter retained retry payload
  fail = false; assert.equal(await model.mutate(), true);
  assert.equal(calls[1].input.occurred_at, 123); assert.equal(calls[1].input.id, 'stable-event');
  assert.equal(model.events.length, 1); assert.equal(model.pending, null);
});

test('conflicts are surfaced, never overwritten or automatically replayed; explicit reload reads fresh revisions', async () => {
  const calls = [];
  const model = new HabitsController({ request: async r => { calls.push(r); if (r.op === 'update') throw new Error('revision conflict'); if (r.op === 'get') return { type: 'habit', data: { ...negative, revision: 4 } }; return { type: r.op === 'events' ? 'events' : 'habits', data: [] }; } });
  model.selected = negative;
  await model.mutate({ op: 'update', id: 'n', expected_revision: 1, definition: { ...negative.definition, archived: true } });
  assert.equal(model.selected.definition.archived, false); assert.equal(model.selected.revision, 1); assert.match(model.error, /conflict/);
  assert.equal(calls.length, 1); await model.reconcile(); assert.equal(model.pending, null); assert.equal(model.selected.revision, 4);
  assert.equal(calls.filter(r => r.op === 'update').length, 1);
});

test('void/restore and archive preserve payloads and use expected revisions', async () => {
  const calls = [];
  const model = new HabitsController({ request: async r => { calls.push(r); return r.op === 'correct' ? { type: 'event', data: { ...event, revision: r.expected_revision + 1, voided: r.voided } } : { type: 'habit', data: { ...negative, revision: 2, definition: r.definition } }; } });
  model.selected = negative;
  await model.mutate({ op: 'correct', event_id: event.id, expected_revision: 1, voided: true });
  assert.equal(model.events[0].occurred_at, event.occurred_at); assert.equal(model.events[0].voided, true);
  await model.mutate({ op: 'correct', event_id: event.id, expected_revision: 2, voided: false });
  assert.equal(model.events[0].voided, false);
  await model.mutate({ op: 'update', id: 'n', expected_revision: 1, definition: { ...negative.definition, archived: true } });
  assert.equal(model.selected.definition.archived, true); assert.equal(model.events.length, 1); assert.equal(calls[1].expected_revision, 2);
});

test('graph switch/disable disposal rejects late successes and late failures without exposing old data', async () => {
  for (const reject of [false, true]) {
    const deferred = gate(); const old = new HabitsController({ request: () => deferred.promise });
    const loading = old.load(); old.dispose();
    const fresh = new HabitsController({ request: async () => ({ type: 'habits', data: [negative] }) }); await fresh.load();
    if (reject) deferred.reject(new Error('old failure')); else deferred.resolve({ type: 'habits', data: [positive] });
    await loading; assert.deepEqual(old.habits, []); assert.equal(old.error, ''); assert.deepEqual(fresh.habits, [negative]);
  }
});

test('bounded definition/history pages use existing keyset cursors; negative habits never request due status', async () => {
  const calls = []; const events = Array.from({ length: 20 }, (_, i) => ({ ...event, id: `e${i}`, occurred_at: 200 - i }));
  const model = new HabitsController({ request: async r => { calls.push(r); if (r.op === 'events') return { type: 'events', data: r.before ? [] : events }; return { type: 'habits', data: Array.from({ length: 30 }, (_, i) => ({ ...negative, id: `n${i}` })) }; } });
  await model.load(); assert.equal(model.more, true); await model.load(true);
  assert.equal(calls[1].after, 'n29'); assert.equal(model.habits.length, 30);
  await model.details(negative); assert.equal(model.moreEvents, true); await model.details(negative, true);
  assert.deepEqual(calls.at(-1).before, { occurred_at: 181, id: 'e19' }); assert.equal(model.events.length, 0);
  assert.equal(calls.some(r => r.op === 'status'), false);
});

test('loading/error/empty controller transitions retain acknowledged state on read failure', async () => {
  const deferred = gate(); const model = new HabitsController({ request: () => deferred.promise });
  const loading = model.load(); assert.equal(model.busy, true); assert.equal(model.loaded, false);
  deferred.resolve({ type: 'habits', data: [] }); await loading; assert.equal(model.loaded, true); assert.equal(model.busy, false);
  const failed = new HabitsController({ request: async () => { throw new Error('read failed'); } });
  failed.habits = [negative]; await failed.load(); assert.match(failed.error, /read failed/); assert.deepEqual(failed.habits, [negative]);
});

test('due refresh is page-bounded, uses explicit now, skips negative/archive and never reloads definitions or replays pending writes', async () => {
  const calls = [];
  const model = new HabitsController({ request: async r => { calls.push(r); return { type: 'status', data: { habit_id: 'p', positive: { due: true, occurrence: 1, next_occurrence: 2 }, negative: null } }; } });
  model.habits = [positive, negative, { ...positive, id: 'archived', definition: { ...positive.definition, archived: true } }];
  await model.refresh(); assert.equal(calls.length, 1); assert.equal(calls[0].op, 'status'); assert.ok(Number.isSafeInteger(calls[0].now));
  assert.equal(model.statuses.get('p').positive.due, true);
  model.pending = structuredClone(record); await model.refresh(); assert.equal(calls.length, 1);
});

test('never-arriving acknowledgement times out without losing identity; late reply cannot apply state', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const deferred = gate(); const model = new HabitsController({ request: () => deferred.promise }); model.selected = negative;
  const saving = model.mutate(record); t.mock.timers.tick(10001); await saving;
  assert.match(model.error, /timed out/); assert.equal(model.busy, false); assert.equal(model.pending.input.id, 'stable-event');
  deferred.resolve({ type: 'event', data: event }); await Promise.resolve(); await Promise.resolve();
  assert.equal(model.events.length, 0); assert.ok(model.pending);
});
