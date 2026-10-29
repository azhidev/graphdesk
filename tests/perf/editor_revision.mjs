/** Synthetic Node model of draft notification + row-prop evaluation. No Svelte DOM.
 * node tests/perf/editor_revision.mjs (Node 24)
 */
import { performance } from 'node:perf_hooks';
import { EditorController } from '../../src/lib/editor/controller.ts';

for (const size of [200, 1_000, 5_000]) {
  const blocks = Array.from({ length: size }, (_, position) => ({
    id: `block-${position}`, page_id: 'page', parent_id: null, position,
    content: `سلام‌دنیا English note ${position}`, task_status: null,
  }));
  function run(pageWide) {
    let rowPasses = 0;
    const editor = new EditorController({}, scope => {
      if (!pageWide && scope === 'draft') return;
      for (const block of editor.blocks) editor.text(block.id);
      rowPasses++;
    });
    editor.blocks = blocks;
    editor.composition(true); // avoid timer/IPC work in this synchronous model
    const samples = [];
    for (let round = 0; round < 14; round++) {
      rowPasses = 0;
      const start = performance.now();
      for (let i = 0; i < 40; i++) editor.setDraft('block-0', `سلام‌دنیا ${round}-${i}`);
      if (round >= 4) samples.push(performance.now() - start);
    }
    samples.sort((a, b) => a - b);
    return { median_40_drafts_ms: Math.round(samples[5] * 100) / 100, rowPasses_per_40_drafts: rowPasses };
  }
  console.log(JSON.stringify({ blocks: size, legacy_page_wide: run(true), scoped_drafts: run(false) }));
}
