import test from 'node:test';
import assert from 'node:assert/strict';
import { ExtensionHost, ExtensionRegistry } from '../../src/lib/extensions/host.ts';
import { tally } from '../../src/lib/extensions/tally.ts';
import { NavigationHistory } from '../../src/lib/navigation.ts';

const deferred = () => { let resolve, reject; const promise = new Promise((a, b) => { resolve = a; reject = b; }); return { promise, resolve, reject }; };
class Transport {
  graphs = new Map();
  sessions = new Map([['a', 'A'], ['a-reopened', 'A'], ['b', 'B']]);
  calls = [];
  row(session, id = tally.manifest.id) {
    const graph = this.sessions.get(session);
    if (!graph) throw new Error('stale session');
    const key = `${graph}/${id}`;
    if (!this.graphs.has(key)) this.graphs.set(key, { enabled: false, revision: 0, value: null });
    return this.graphs.get(key);
  }
  async state(session) { return { graph_id: this.sessions.get(session), extensions: [{ id: tally.manifest.id, enabled: this.row(session).enabled }] }; }
  async configure(session, id, enabled) { this.row(session, id).enabled = enabled; }
  async request(session, id, version, request) {
    this.calls.push({ session, id, version, request });
    const row = this.row(session, id);
    if (version !== 1 || !row.enabled) throw new Error('denied');
    if (request.op === 'storage_write') {
      if (request.expected_revision !== row.revision) throw new Error('conflict');
      row.revision++; row.value = structuredClone(request.value);
    }
    return structuredClone({ revision: row.revision, value: row.value });
  }
}
function setup(definitions = [tally], transport = new Transport()) {
  const registry = new ExtensionRegistry(); definitions.forEach(d => registry.register(d));
  return { host: new ExtensionHost(registry, transport), registry, transport };
}

test('registration is inert, validates IDs/API/capabilities and rejects duplicates', () => {
  let activations = 0;
  const definition = { ...tally, activate: async api => { activations++; return tally.activate(api); } };
  const { registry } = setup([definition]);
  assert.equal(activations, 0);
  assert.throws(() => registry.register(definition));
  for (const manifest of [{ ...tally.manifest, id: '../bad' }, { ...tally.manifest, apiVersion: 2 }, { ...tally.manifest, capabilities: ['filesystem'] }]) {
    assert.throws(() => new ExtensionRegistry().register({ ...definition, manifest }));
  }
});

test('bundled Tally exercises enable, storage, serialized actions, disable/restore and graph/restart isolation', async () => {
  const { host, transport } = setup();
  await host.attach('a', 'A');
  assert.equal(host.states()[0].status, 'disabled'); assert.equal(host.navigation().length, 0);
  assert.equal(transport.calls.length, 0);
  await host.configure(tally.manifest.id, true);
  assert.equal(host.navigation()[0].viewId, 'counter');
  assert.equal(host.view(tally.manifest.id, 'counter').value, '0');
  await Promise.all([host.invoke(tally.manifest.id, 'increment'), host.invoke(tally.manifest.id, 'increment')]);
  assert.equal(host.view(tally.manifest.id, 'counter').value, '1', 'rapid pending action must be ignored, not duplicated');
  await host.configure(tally.manifest.id, false);
  assert.equal(host.navigation().length, 0); assert.equal(host.view(tally.manifest.id, 'counter'), undefined);
  await host.invoke(tally.manifest.id, 'increment');
  assert.equal(transport.row('a').value.count, 1);
  await host.configure(tally.manifest.id, true);
  assert.equal(host.view(tally.manifest.id, 'counter').value, '1');
  await host.attach('b', 'B');
  assert.equal(host.navigation().length, 0, 'enabled preference must not cross graphs');
  await host.configure(tally.manifest.id, true);
  assert.equal(host.view(tally.manifest.id, 'counter').value, '0');
  await host.invoke(tally.manifest.id, 'increment'); await host.invoke(tally.manifest.id, 'increment');
  assert.equal(transport.row('b').value.count, 2); assert.equal(transport.row('a').value.count, 1);
  host.detach();
  const restarted = setup([tally], transport).host;
  await restarted.attach('a-reopened', 'A');
  assert.equal(restarted.view(tally.manifest.id, 'counter').value, '1');
  assert.equal(restarted.states()[0].enabled, true);
});

test('late activation is revoked and cleaned on graph switching; captured APIs cannot cross graphs', async () => {
  const gate = deferred(); let api, disposed = 0;
  const definition = { ...tally, activate: async hostApi => { api = hostApi; await gate.promise; return { invoke: async () => {}, dispose: () => disposed++ }; } };
  const { host, transport } = setup([definition]); transport.row('a').enabled = true;
  const pending = host.attach('a', 'A'); await new Promise(resolve => setImmediate(resolve));
  const captured = api;
  await host.attach('b', 'B'); gate.resolve(); await pending;
  assert.equal(disposed, 1); assert.equal(host.navigation().length, 0);
  await assert.rejects(captured.storage.read(), /revoked/);
  assert.throws(() => captured.ui.setNavigation([]), /revoked/);
  assert.equal(transport.calls.length, 0);
});

test('late state reads and late command acknowledgements cannot restore old contributions', async () => {
  const { host, transport } = setup();
  const stateGate = deferred(); const originalState = transport.state.bind(transport);
  transport.state = session => session === 'a' ? stateGate.promise : originalState(session);
  const loading = host.attach('a', 'A'); await host.attach('b', 'B');
  stateGate.resolve({ graph_id: 'A', extensions: [{ id: tally.manifest.id, enabled: true }] }); await loading;
  assert.equal(host.states()[0].enabled, false);
  await host.configure(tally.manifest.id, true);
  const writeGate = deferred(), originalRequest = transport.request.bind(transport);
  transport.request = (session, id, version, request) => request.op === 'storage_write' ? writeGate.promise : originalRequest(session, id, version, request);
  const writing = host.invoke(tally.manifest.id, 'increment');
  await host.attach('a-reopened', 'A');
  writeGate.resolve({ revision: 1, value: { schema: 1, count: 99 } }); await writing;
  assert.equal(host.states()[0].enabled, false); assert.equal(host.navigation().length, 0);
});

test('activation, command, storage, configure and disposal errors stay extension-local', async () => {
  const { host, transport } = setup(); await host.attach('a', 'A');
  transport.row('a').value = { schema: 999, count: 4 };
  await host.configure(tally.manifest.id, true);
  assert.equal(host.states()[0].status, 'error'); assert.match(host.states()[0].error, /Unsupported/);
  assert.deepEqual(transport.row('a').value, { schema: 999, count: 4 });
  transport.row('a').value = null;
  await host.configure(tally.manifest.id, true);
  transport.request = async () => { throw new Error('fixture writer failure'); };
  await host.invoke(tally.manifest.id, 'increment');
  assert.equal(host.states()[0].status, 'error'); assert.equal(host.navigation().length, 0);
  assert.equal(transport.row('a').value, null);
  transport.configure = async () => { throw new Error('fixture preference failure'); };
  await host.configure(tally.manifest.id, false);
  assert.equal(host.states()[0].enabled, true, 'failed disable must not pretend preference was saved');
  assert.match(host.states()[0].error, /preference failure/);
  const disposal = setup([{ ...tally, activate: async () => ({ invoke: async () => {}, dispose() { throw new Error('cleanup'); } }) }]);
  await disposal.host.attach('a', 'A'); await disposal.host.configure(tally.manifest.id, true);
  await disposal.host.configure(tally.manifest.id, false);
  assert.equal(disposal.host.navigation().length, 0); assert.match(disposal.host.states()[0].error, /cleanup/);
});

test('host denies undeclared capabilities, rejects graph identity mismatch and scopes view snapshots', async () => {
  const { host } = setup([{ ...tally, manifest: { ...tally.manifest, capabilities: [] } }]);
  await host.attach('a', 'A'); await host.configure(tally.manifest.id, true);
  assert.equal(host.states()[0].status, 'error'); assert.match(host.states()[0].error, /capability denied/);
  await host.attach('b', 'WRONG'); assert.match(host.error, /identity/); assert.equal(host.navigation().length, 0);
  const normal = setup(); await normal.host.attach('a', 'A'); await normal.host.configure(tally.manifest.id, true);
  normal.host.states()[0].views[0].value = 'tampered';
  assert.equal(normal.host.view(tally.manifest.id, 'counter').value, '0');
});

test('late enable acknowledgement and malformed contributions cannot affect a new graph or another extension', async () => {
  const { host, transport } = setup();
  await host.attach('a', 'A');
  const gate = deferred(); transport.configure = () => gate.promise;
  const enabling = host.configure(tally.manifest.id, true);
  await host.attach('b', 'B'); gate.resolve(); await enabling;
  assert.equal(host.states()[0].enabled, false); assert.equal(host.navigation().length, 0);
  const healthy = { ...tally, manifest: { ...tally.manifest, id: 'org.healthy', capabilities: [] },
    activate: async () => ({ invoke: async () => {}, dispose() {} }) };
  const broken = { ...tally, activate: async api => {
    api.ui.setView({ id: 'bad', title: undefined, description: tally.manifest.description, actions: [] });
    throw new Error('unreachable');
  } };
  const isolated = setup([broken, healthy]); await isolated.host.attach('a', 'A');
  await isolated.host.configure('org.healthy', true); await isolated.host.configure(tally.manifest.id, true);
  assert.equal(isolated.host.states().find(s => s.manifest.id === 'org.healthy').status, 'active');
  assert.equal(isolated.host.states().find(s => s.manifest.id === tally.manifest.id).status, 'error');
  assert.equal(isolated.host.error, '');
});

test('extension history uses normal bounded navigation without conflating separate views', async () => {
  const history = new NavigationHistory(); history.reset('graph');
  const a = { kind: 'extension', extensionId: 'graphdesk.tally', viewId: 'counter' };
  await history.visit(a, async () => {});
  await history.visit({ ...a, viewId: 'other' }, async () => {});
  assert.equal(history.length, 2);
  await assert.rejects(history.move(-1, async () => { throw new Error('disabled extension'); }));
  assert.equal(history.current.viewId, 'other');
  await history.move(-1, async () => {}); assert.equal(history.current.viewId, 'counter');
});
