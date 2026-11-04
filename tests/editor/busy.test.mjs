import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'vite';
import { readFileSync } from 'node:fs';
import { parse } from 'svelte/compiler';
import { busyControl } from '../../src/lib/busyControl.ts';
import { NavigationHistory } from '../../src/lib/navigation.ts';

class Button extends EventTarget {
  attributes = new Map();
  disabled = false;
  setAttribute(key, value) { this.attributes.set(key, value); }
  removeAttribute(key) { this.attributes.delete(key); }
}

test('App wiring scopes ordinary run feedback locally and uses transient guards on stable shell navigation', () => {
  // Structural integration contract; actual client DOM/paint is verified separately.
  const ast = parse(readFileSync(new URL('../../src/App.svelte', import.meta.url), 'utf8'), { modern: true });
  function nodes(value, predicate, found = []) {
    if (!value || typeof value !== 'object') return found;
    if (predicate(value)) found.push(value);
    for (const child of Object.values(value)) nodes(child, predicate, found);
    return found;
  }
  const run = ast.instance.content.body.find(node => node.type === 'FunctionDeclaration' && node.id?.name === 'run');
  assert.equal(run.params[2].right.value, 'content');
  for (const name of ['open', 'recent', 'pick']) {
    const fn = ast.instance.content.body.find(node => node.type === 'FunctionDeclaration' && node.id?.name === name);
    const call = nodes(fn, node => node.type === 'CallExpression' && node.callee?.name === 'run')[0];
    assert.equal(call.arguments[2].value, 'shell', 'graph IO must retain explicit global feedback');
  }
  const className = node => node.attributes?.find(attr => attr.type === 'Attribute' && attr.name === 'class')?.value?.[0]?.data;
  const stable = nodes(ast.fragment, node => ['graph-switch', 'page-jump', 'history-nav', 'primary-nav', 'page-list shell-scroll', 'journal-nav'].includes(className(node)));
  assert.equal(stable.length, 6);
  for (const region of stable) {
    const buttons = region.name === 'button' ? [region] : nodes(region, node => node.type === 'RegularElement' && node.name === 'button');
    assert.ok(buttons.length);
    for (const button of buttons) {
      assert.ok(button.attributes.some(attr => attr.type === 'UseDirective' && attr.name === 'busyControl'));
      const disabled = button.attributes.find(attr => attr.type === 'Attribute' && attr.name === 'disabled');
      assert.equal(nodes(disabled, node => node.type === 'Identifier' && node.name === 'locked').length, 0);
    }
  }
});

test('transient busy lock blocks delegated mouse/keyboard clicks without changing native disabled availability', () => {
  const button = new Button();
  const action = busyControl(button, false);
  let activations = 0;
  button.addEventListener('click', () => activations++);
  const click = () => button.dispatchEvent(new Event('click', { cancelable: true, bubbles: true }));
  assert.equal(click(), true); assert.equal(activations, 1);
  action.update(true);
  assert.equal(button.attributes.get('aria-disabled'), 'true');
  assert.equal(button.disabled, false, 'busy must not invoke native disabled styling/focus removal');
  for (let i = 0; i < 10; i++) assert.equal(click(), false);
  assert.equal(activations, 1);
  action.update(false);
  assert.equal(button.attributes.has('aria-disabled'), false);
  click(); assert.equal(activations, 2);
  button.disabled = true; action.update(true); action.update(false);
  assert.equal(button.disabled, true, 'intrinsically unavailable controls must remain natively disabled');
  assert.equal(button.attributes.has('aria-disabled'), false, 'do not override intrinsic disabled semantics');
  action.destroy(); click(); assert.equal(activations, 3, 'destroy must remove the capture listener');
});

test('journal picker separates transient busy state from intrinsic disabled availability', async () => {
  const source = readFileSync(new URL('../../src/App.svelte', import.meta.url), 'utf8');
  assert.match(source, /<CalendarPicker\b[^>]*busy=\{locked\}/);
  assert.doesNotMatch(source, /<CalendarPicker\b[^>]*disabled=\{locked\}/);
  const vite = await createServer({ server: { middlewareMode: true, hmr: false, watch: null }, logLevel: 'error' });
  try {
    const { default: Picker } = await vite.ssrLoadModule('/src/lib/CalendarPicker.svelte');
    const { render } = await vite.ssrLoadModule('svelte/server');
    for (const disabled of [false, true]) for (const busy of [false, true]) {
      const { body } = render(Picker, { props: { day: '2026-11-04', calendar: 'gregorian', disabled, busy, onOpen() {} } });
      const unavailable = attributes => /(?:^|\s)disabled(?:\s|=|$)/.test(attributes);
      assert.equal(unavailable(body.match(/<button\b([^>]*)>/)[1]), disabled, 'busy alone must not dim the journal button');
      const inputs = [...body.matchAll(/<input\b([^>]*)>/g)];
      assert.equal(inputs.length, 3);
      for (const [, attributes] of inputs) assert.equal(unavailable(attributes), disabled || busy);
    }
  } finally { await vite.close(); }
});

test('history destinations stay visually stable while actual navigation remains serialized; failures retain selection', async () => {
  const history = new NavigationHistory(); history.reset('graph');
  await history.visit({ kind: 'page', id: 'a' }, async () => {});
  await history.visit({ kind: 'page', id: 'b' }, async () => {});
  await history.move(-1, async () => {});
  let release;
  const pending = history.move(1, () => new Promise(resolve => release = resolve));
  assert.equal(history.hasForward, true); assert.equal(history.canForward, false);
  assert.equal(history.current.id, 'a');
  await assert.rejects(history.visit({ kind: 'page', id: 'stale' }, async () => {}), /current operation/);
  release(); await pending;
  assert.equal(history.current.id, 'b'); assert.equal(history.hasBack, true);
  assert.equal(history.hasForward, false);
  let fail;
  const failed = history.move(-1, () => new Promise((_, reject) => fail = reject));
  assert.equal(history.hasBack, true); assert.equal(history.canBack, false);
  fail(new Error('page read failed')); await assert.rejects(failed, /read failed/);
  assert.equal(history.current.id, 'b'); assert.equal(history.canBack, true);
});

test('ordinary operation feedback is content-local with a stable empty slot; explicit graph operations retain shell feedback', async () => {
  const vite = await createServer({ server: { middlewareMode: true, hmr: false, watch: null }, logLevel: 'error' });
  try {
    const { default: Status } = await vite.ssrLoadModule('/src/lib/OperationStatus.svelte');
    const { render } = await vite.ssrLoadModule('svelte/server');
    for (const label of ['Loading…', 'Opening journal…', 'Preparing capture…']) {
      assert.doesNotMatch(render(Status, { props: { label, scope: 'content', region: 'shell' } }).body, /<p|role="status"|Loading/);
      const body = render(Status, { props: { label, scope: 'content', region: 'content' } }).body;
      assert.match(body, /class="content-operation/); assert.ok(body.includes(label));
    }
    for (const label of ['', 'Opening graph…', 'Choosing graph…']) {
      const content = render(Status, { props: { label, scope: 'shell', region: 'content' } }).body;
      assert.match(content, /class="content-operation/); assert.doesNotMatch(content, /Opening graph|Choosing graph/);
      const shell = render(Status, { props: { label, scope: 'shell', region: 'shell' } }).body;
      assert.equal(shell.includes('role="status"'), !!label);
    }
  } finally { await vite.close(); }
});
