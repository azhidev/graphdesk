/** Synthetic Node-only page-update costs. Not a WebView/DOM/typing benchmark.
 * Run: node tests/perf/editor_input.mjs (Node 24 with TS stripping).
 * Legacy functions stay here to compare the same data before/after an optimization.
 */
import { performance } from 'node:perf_hooks';
import { EditorController } from '../../src/lib/editor/controller.ts';

function measure(work, repetitions = 12) {
  for (let i = 0; i < 3; i++) work();
  const start = performance.now();
  for (let i = 0; i < repetitions; i++) work();
  return Math.round((performance.now() - start) / repetitions * 100) / 100;
}

for (const size of [200, 1_000, 5_000]) {
  const editor = new EditorController({});
  editor.blocks = Array.from({ length: size }, (_, position) => ({
    id: `block-${position}`, page_id: 'page', parent_id: null, position,
    content: `سلام‌دنیا English note ${position}; review 3 details.`, task_status: null,
  }));
  const rows = editor.rows();
  const legacy = () => { for (const row of rows) editor.blocks.find(b => b.id === row.block.id)?.content; };
  const current = () => { for (const row of rows) editor.text(row.block.id); };
  const legacyKey = () => JSON.stringify([[], editor.blocks]);
  // Model a draft on a middle block: the acknowledged block array does not change.
  const acknowledged = editor.blocks;
  const identityKey = () => editor.blocks === acknowledged;
  console.log(JSON.stringify({ blocks: size, legacy_text_pass_ms: measure(legacy),
    current_text_pass_ms: measure(current), legacy_key_ms: measure(legacyKey),
    identity_key_ms: measure(identityKey) }));
}
