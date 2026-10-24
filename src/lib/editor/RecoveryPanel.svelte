<script lang="ts">
  import type { Block, RecoveryDraft } from './types';
  import { editorApi, recoveryApi } from '../api';
  import { errorMessage } from './controller';
  import { tick } from 'svelte';
  let { session, records, disabled, onChanged, onRecover }: {
    session: string; records: RecoveryDraft[]; disabled: boolean;
    onChanged: () => Promise<void>;
    onRecover: (draft: RecoveryDraft, block: Block, revision: number) => Promise<void>;
  } = $props();
  let comparison: { draft: RecoveryDraft; block: Block | null; revision: number } | null = $state(null);
  let busy = $state(false), error = $state('');
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
    try { await onRecover(comparison.draft, comparison.block, comparison.revision); comparison = null; await onChanged(); await tick(); document.getElementById('review-recovery')?.focus(); }
    catch (e) { error = errorMessage(e); }
    finally { busy = false; }
  }
  async function remove() {
    const draft = discarding;
    discard.close(); discarding = null;
    if (!draft) return;
    busy = true; error = '';
    try { await recoveryApi.remove(session, draft.id, draft.content, draft.updated_at); comparison = null; await onChanged(); await tick(); document.getElementById('review-recovery')?.focus(); }
    catch (e) { error = errorMessage(e); }
    finally { busy = false; }
  }
</script>

{#if records.length}
  <section aria-label="Recoverable text drafts">
    <h2>Recoverable text drafts ({records.length})</h2>
    <p>Stored only on this computer, outside your graph. Unresolved snapshots expire after 14 days. Compare before recovering; no structure is replayed.</p>
    {#each records as draft (draft.id)}
      <div>
        <span>Block {draft.block_id.slice(0, 8)} · {new Date(draft.updated_at * 1000).toLocaleString()}</span>
        <button id={`recovery-compare-${draft.id}`} disabled={disabled || busy} onclick={() => compare(draft)}>Compare / Recover</button>
        <button disabled={disabled || busy} onclick={() => { discarding = draft; discard.showModal(); }}>Discard recovery draft…</button>
      </div>
    {/each}
    {#if comparison}
      <section aria-label="Compare recovery draft">
        <label>Persisted version<textarea readonly dir="auto" value={comparison.block?.content ?? 'Original block unavailable'}></textarea></label>
        <label>Recovery version — select to copy<textarea readonly dir="auto" value={comparison.draft.content}></textarea></label>
        <p>Recover replaces only the displayed block text, if its revision has not changed. Both versions remain available until you resolve this choice.</p>
        <button disabled={disabled || busy || !comparison.block} onclick={recover}>Recover this text</button>
        <button disabled={busy} onclick={closeComparison}>Close comparison</button>
      </section>
    {/if}
    {#if error}<p role="alert">{error}</p>{/if}
  </section>
{/if}
<dialog bind:this={discard} aria-labelledby="discard-recovery-heading" oncancel={() => discarding = null}>
  <h3 id="discard-recovery-heading">Discard this recovery draft?</h3>
  <p>This permanently removes the selected recovery copy, not the persisted block. It cannot be undone.</p>
  <button onclick={() => { discard.close(); discarding = null; }}>Cancel</button>
  <button onclick={remove}>Discard recovery draft</button>
</dialog>

<style>
  section { border: 1px solid #8d9f96; padding: .7rem; margin-block: .7rem; }
  label { display: block; font-weight: bold; }
  textarea { display: block; box-sizing: border-box; width: 100%; min-height: 5rem; font: inherit; }
  button { font: inherit; margin: .25rem; padding: .3rem; }
</style>
