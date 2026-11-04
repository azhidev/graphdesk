import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'vite';
import { EditorController } from '../../src/lib/editor/controller.ts';

// Use the existing Svelte/Vite compiler, without a server listener or DOM library.
// This checks rendered markup/CSS applicability, not WebKit paint or native input.
test('pending editor markup retains readable text and correct availability at 5,000 blocks', async () => {
  const vite = await createServer({ server: { middlewareMode: true, hmr: false, watch: null }, logLevel: 'error' });
  try {
    const { default: PageEditor } = await vite.ssrLoadModule('/src/lib/editor/PageEditor.svelte');
    const { render } = await vite.ssrLoadModule('svelte/server');
    const editor = new EditorController({});
    editor.blocks = Array.from({ length: 5000 }, (_, position) => ({
      id: `b${position}`, page_id: 'page', parent_id: null, position, content: `English فارسی ${position}`, task_status: null,
    }));
    editor.activeId = 'b1';
    const props = { editor, revision: 0, rowsRevision: 0, pages: [], references: [], onReference() {} };
    for (const busy of [false, true, false]) {
      editor.working = busy;
      const { body } = render(PageEditor, { props });
      assert.match(body, new RegExp(`aria-busy="${busy}"`));
      const buttons = [...body.matchAll(/<button\b([^>]*class="block-content[^>]*)>(.*?)<\/button>/gs)];
      assert.equal(buttons.length, 4999);
      for (const [, attributes, text] of buttons) {
        assert.match(attributes, new RegExp(`aria-disabled="${busy}"`));
        assert.doesNotMatch(attributes, /(?:^|\s)disabled(?:\s|=|$)/, 'document text must not acquire button:disabled dimming');
        assert.match(text, /English فارسی/);
      }
      assert.match(body, /data-block-id="b4999"/);
      const textarea = body.match(/<textarea\b([^>]*)>/)[1];
      assert.equal(/(?:^|\s)readonly(?:\s|=|$)/.test(textarea), busy, 'active input must remain read-only during structural IO');
      assert.equal((body.match(/data-block-id=/g) ?? []).length, 5000);
    }
  } finally { await vite.close(); }
});
