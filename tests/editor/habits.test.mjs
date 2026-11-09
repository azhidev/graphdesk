import test from 'node:test';
import assert from 'node:assert/strict';
import { ExtensionHost, ExtensionRegistry } from '../../src/lib/extensions/host.ts';
import { habits } from '../../src/lib/extensions/habits.ts';
import { tally } from '../../src/lib/extensions/tally.ts';

function setup(definition = habits) {
  const registry = new ExtensionRegistry(); registry.register(definition);
  const enabled = new Map(); const requests = [];
  const transport = {
    async state(session) { return { graph_id: session, extensions: [{ id: definition.manifest.id, enabled: enabled.get(session) ?? false }] }; },
    async configure(session, _id, value) { enabled.set(session, value); },
    async request() { throw new Error('Habits must not store history in the JSON document'); },
    async habits(session, extensionId, version, request) {
      requests.push({ session, extensionId, version, request });
      if (!enabled.get(session) || extensionId !== 'graphdesk.habits' || version !== 1) throw new Error('denied');
      return { type: 'habits', data: [] };
    },
  };
  return { host: new ExtensionHost(registry, transport), transport, requests };
}

test('Habits registration/lifecycle contributes bounded navigation, no events or storage document', async () => {
  const { host, requests } = setup();
  assert.equal(habits.manifest.id, 'graphdesk.habits');
  assert.deepEqual(habits.manifest.capabilities, ['habits.core', 'ui.views', 'ui.navigation']);
  await host.attach('A', 'A'); assert.equal(requests.length, 0);
  await host.configure('graphdesk.habits', true);
  assert.equal(host.states()[0].status, 'active');
  assert.deepEqual(requests, [{ session: 'A', extensionId: 'graphdesk.habits', version: 1, request: { op: 'list', after: null, limit: 1 } }]);
  assert.equal(host.navigation()[0].viewId, 'habits'); assert.equal(host.states()[0].views[0].actions.length, 0);
  const client = host.habitsClient(); assert.ok(client);
  await host.configure('graphdesk.habits', false);
  assert.equal(host.habitsClient(), undefined);
  await assert.rejects(client.request({ op: 'list', after: null, limit: 1 }), /revoked/);
  await host.attach('B', 'B'); assert.equal(host.states()[0].enabled, false);
  await host.attach('A', 'A'); assert.equal(host.states()[0].enabled, false);
  await host.configure('graphdesk.habits', true); host.detach();
  assert.equal(requests.length, 2);
});

test('habits host calls bind session/identity and revoke captured APIs and pending replies', async () => {
  let api;
  const definition = { ...habits, async activate(value) {
    api = value;
    return { async invoke() {}, dispose() {} };
  } };
  const { host, transport, requests } = setup(definition);
  await host.attach('A', 'A'); await host.configure('graphdesk.habits', true);
  const captured = api;
  const result = await captured.habits.request({ op: 'status', id: 'habit', now: 123 });
  assert.equal(result.type, 'habits');
  assert.equal(requests[0].session, 'A');
  let release;
  transport.habits = () => new Promise(resolve => release = resolve);
  const pending = captured.habits.request({ op: 'list', after: null, limit: 1 });
  await host.attach('B', 'B'); release({ type: 'habits', data: [] });
  await assert.rejects(pending, /revoked/);
  await assert.rejects(captured.habits.request({ op: 'list', after: null, limit: 1 }), /revoked/);
});

test('habits capability is explicit and cannot be acquired under another bundled identity', async () => {
  for (const manifest of [
    { ...habits.manifest, capabilities: [] },
    { ...tally.manifest, capabilities: ['habits.core'] },
  ]) {
    const definition = { ...habits, manifest };
    const { host, requests } = setup(definition);
    await host.attach('A', 'A'); await host.configure(manifest.id, true);
    assert.equal(host.states()[0].status, 'error'); assert.equal(requests.length, 0);
    assert.match(host.states()[0].error, /denied/);
  }
});
