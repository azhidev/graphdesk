<script lang="ts">
  import type { Block, RecoveryDraft } from './types';
  import { editorApi, recoveryApi } from '../api';
  import { errorMessage } from './controller';
  import { tick } from 'svelte';
  import { tr } from '../preferences';
  import ErrorNotice from '../ErrorNotice.svelte';
  let { session, records, disabled, onChanged, onRecover }: {
    session: string; records: RecoveryDraft[]; disabled: boolean;
    onChanged: () => Promise<void>;
    onRecover: (draft: RecoveryDraft, block: Block, revision: number) => Promise<void>;
  } = $props();
  let comparison: { draft: RecoveryDraft; block: Block | null; revision: number } | null = $state(null);
  let busy = $state(false), error = $state('');
  let review: HTMLDialogElement;
  let returnFocus: HTMLElement | null = null;
  let dismissed = $state(false);
  let previousRecords: RecoveryDraft[] = [];
  $effect(() => {
    if (records.some(record => !previousRecords.some(previous => previous.id === record.id && previous.updated_at === record.updated_at && previous.content === record.content))) dismissed = false;
    previousRecords = records;
  });
  export async function show() {
    if (!records.length || review.open) return;
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    review.showModal(); await tick(); document.getElementById('recovery-heading')?.focus();
  }
  function closeReview() {
    if (discard.open || busy) return;
    review.close(); comparison = null; error = '';
    if (returnFocus?.isConnected) returnFocus.focus();
    else (document.getElementById('review-recovery') ?? document.getElementById('page-heading'))?.focus();
  }
  let discard: HTMLDialogElement;
  let discarding: RecoveryDraft | null = $state(null);
  async function compare(draft: RecoveryDraft) {
    busy = true; error = '';
    try {
      const page = await editorApi.page(session, draft.page_id);
      comparison = { draft, block: page.blocks.find(b => b.id === draft.block_id) ?? null, revision: page.revisions[draft.block_id] ?? 0 };
    } catch { comparison = { draft, block: null, revision: 0 }; error = 'Cannot read the original block. Its draft remains available to select and copy; no structure will be recreated.'; }
    finally { busy = false; await tick(); document.getElementById(`recovery-compare-${draft.id}`)?.focus(); }
  }
  async function closeComparison() {
    const id = comparison?.draft.id; comparison = null;
    await tick(); document.getElementById(`recovery-compare-${id}`)?.focus();
  }
  async function recover() {
    if (!comparison?.block) return;
    busy = true; error = '';
    try { await onRecover(comparison.draft, comparison.block, comparison.revision); comparison = null; await onChanged(); await tick(); if (!records.length) { busy = false; closeReview(); } else document.querySelector<HTMLElement>('.recovery-list button')?.focus(); }
    catch (e) { error = errorMessage(e); }
    finally { busy = false; }
  }
  async function remove() {
    const draft = discarding;
    discard.close(); discarding = null;
    if (!draft) return;
    busy = true; error = '';
    try { await recoveryApi.remove(session, draft.id, draft.content, draft.updated_at); comparison = null; await onChanged(); await tick(); if (!records.length) { busy = false; closeReview(); } else document.querySelector<HTMLElement>('.recovery-list button')?.focus(); }
    catch (e) { error = errorMessage(e); }
    finally { busy = false; }
  }
</script>

{#if records.length}
  {#if !dismissed}<div class="recovery-notice" role="status"><span>{$tr('Recoverable text drafts')} (<bdi>{records.length}</bdi>) · {$tr('Older local snapshots — separate from current save status')}</span>
    <button onclick={show}>{$tr('Review drafts')}</button><button aria-label={$tr('Dismiss recovery notice')} onclick={() => dismissed = true}>×</button>
  </div>{/if}
{/if}
<dialog bind:this={review} class="recovery-review" aria-labelledby="recovery-heading" oncancel={event => { event.preventDefault(); closeReview(); }}>
  <div class="review-heading"><h2 id="recovery-heading" tabindex="-1">{$tr('Recoverable text drafts')} (<bdi>{records.length}</bdi>)</h2><button aria-label={$tr('Close recovery review')} disabled={busy} onclick={closeReview}>×</button></div>
  {#if records.length}
    <p>{$tr('Stored only on this computer, outside your graph. Unresolved snapshots expire after 14 days. Compare before recovering; no structure is replayed.')}</p>
    <div class="recovery-list">{#each records as draft (draft.id)}
      <div class="draft-row">
        <span>{$tr('Block')} <bdi dir="ltr">{draft.block_id.slice(0, 8)}</bdi> · <bdi>{new Date(draft.updated_at * 1000).toLocaleString()}</bdi></span>
        <button id={`recovery-compare-${draft.id}`} disabled={disabled || busy} onclick={() => compare(draft)}>{$tr('Compare / Recover')}</button>
        <button disabled={disabled || busy} onclick={() => { discarding = draft; discard.showModal(); }}>{$tr('Discard recovery draft…')}</button>
      </div>
    {/each}</div>
    {#if comparison}
      <section class="comparison" aria-label={$tr('Compare recovery draft')}>
        <label>{$tr('Persisted version')}<textarea readonly dir="auto" value={comparison.block?.content ?? $tr('Original block unavailable')}></textarea></label>
        <label>{$tr('Recovery version — select to copy')}<textarea readonly dir="auto" value={comparison.draft.content}></textarea></label>
        <p>{$tr('Recover replaces only the displayed block text, if its revision has not changed. Both versions remain available until you resolve this choice.')}</p>
        <button disabled={disabled || busy || !comparison.block} onclick={recover}>{$tr('Recover this text')}</button>
        <button disabled={busy} onclick={closeComparison}>{$tr('Close comparison')}</button>
      </section>
    {/if}
  {/if}
  {#if error}<ErrorNotice message={error} />{/if}
</dialog>
<dialog bind:this={discard} aria-labelledby="discard-recovery-heading" oncancel={() => discarding = null}>
  <h3 id="discard-recovery-heading">{$tr('Discard this recovery draft?')}</h3>
  <p>{$tr('This permanently removes the selected recovery copy, not the persisted block. It cannot be undone.')}</p>
  <button onclick={() => { discard.close(); discarding = null; }}>{$tr('Cancel')}</button>
  <button onclick={remove}>{$tr('Discard recovery draft')}</button>
</dialog>

<style>
  .recovery-notice { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; max-width: 812px; margin-inline: auto; padding: 7px 12px; border: 1px solid var(--line); border-inline-start: 3px solid var(--warning); border-radius: var(--radius-control); background: var(--warning-bg); color: var(--ink); font-size: 12px; }
  .recovery-notice span { flex: 1; min-width: 180px; }
  button { font: inherit; padding: 5px 9px; }
  .recovery-notice button:last-child { border: 0; background: transparent; font-size: 18px; line-height: 1; }
  .recovery-review { width: min(680px, calc(100vw - 32px)); max-height: calc(100vh - 48px); overflow-y: auto; padding: 24px; }
  .review-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .review-heading h2 { margin: 0; font-size: 20px; }
  .review-heading h2:focus { outline: none; }
  .review-heading button { border: 0; background: transparent; font-size: 22px; }
  .recovery-review > p, .comparison p { color: var(--muted); font-size: 12px; }
  .recovery-list { max-height: 26vh; overflow-y: auto; }
  .draft-row { display: flex; gap: 5px 12px; flex-wrap: wrap; align-items: center; padding-block: 10px; border-block-end: 1px solid var(--line); }
  .draft-row span { flex: 1; min-width: 180px; color: var(--muted); font-size: 12px; }
  .comparison { border-block-start: 1px solid var(--line); margin-block-start: 16px; padding-block-start: 14px; }
  label { display: grid; gap: 5px; margin-block: 10px; font-size: 12px; font-weight: 630; }
  textarea { display: block; box-sizing: border-box; width: 100%; min-height: 5rem; padding: 8px; font: inherit; }
</style>
