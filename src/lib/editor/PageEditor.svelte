<script lang="ts">
  import BlockRow from './BlockRow.svelte';
  import type { EditorController } from './controller';
  import type { Page, DeletePreview } from './types';
  import { tick } from 'svelte';
  import type { InlineReference } from '../references';
  import { preferences, tr } from '../preferences';
  import { statusText } from '../i18n';
  import ErrorNotice from '../ErrorNotice.svelte';
  let { editor, revision, rowsRevision, disabled = false, pages, references, onReference }: {
    editor: EditorController; revision: number; rowsRevision: number; disabled?: boolean; pages: Page[];
    references: InlineReference[]; onReference: (reference: InlineReference) => void;
  } = $props();
  const view = $derived.by(() => { revision; return {
    busy: editor.working || disabled, status: editor.status, error: editor.error,
    canUndo: editor.canUndo || editor.dirty, canRedo: editor.canRedo,
    protection: editor.protection, protectionError: editor.protectionError, conflicts: editor.conflicts,
    dirty: editor.dirty,
    composing: editor.composing,
  }; });
  const rowsView = $derived.by(() => { rowsRevision; return {
    rows: editor.rows(), active: editor.activeId, focus: editor.focus,
    busy: editor.working || disabled,
  }; });
  let previousActive: string | null = null;
  $effect(() => {
    if (view.busy) return;
    if (previousActive && !rowsView.active) document.getElementById('editor-toolbar')?.focus();
    previousActive = rowsView.active;
  });
  function run(operation: Promise<void>) { void operation.catch(() => {}); }
  let confirmation: { count: number; resolve: (accepted: boolean) => void } | null = $state(null);
  let dialog: HTMLDialogElement;
  let actionsOpen = $state(false);
  function confirmDeletion(preview: DeletePreview): Promise<boolean> {
    return new Promise((resolve, reject) => {
      confirmation = { count: preview.count, resolve };
      void tick().then(() => dialog.showModal()).catch(error => { confirmation = null; reject(error); });
    });
  }
  function answer(accepted: boolean) {
    const pending = confirmation;
    dialog.close(); confirmation = null; pending?.resolve(accepted);
  }
  async function remove() {
    const id = rowsView.active;
    if (!id) return;
    await editor.deleteSubtree(id, confirmDeletion);
    if (!editor.activeId) document.getElementById('editor-toolbar')?.focus();
  }
  function operate(kind: 'indent' | 'outdent' | 'reorder', direction = 0) {
    actionsOpen = false;
    if (rowsView.active) run(editor.action(kind === 'reorder' ? { kind, id: rowsView.active, direction } : { kind, id: rowsView.active }, editor.selection(rowsView.active)));
  }
</script>

<div id="editor-toolbar" tabindex="-1" role="group" aria-label={$tr('Block editor actions')}>
  <button disabled={view.busy || !view.canUndo} onclick={() => run(editor.action({ kind: 'undo' }))}>{$tr('Undo')}</button>
  <button disabled={view.busy || !view.canRedo} onclick={() => run(editor.action({ kind: 'redo' }))}>{$tr('Redo')}</button>
  <details class="more-actions" bind:open={actionsOpen}>
    <summary aria-label={$tr('More block actions')} title={$tr('More block actions')}>{$tr('More')} ···</summary>
    <div class="more-list">
      <button disabled={view.busy || !rowsView.active} onclick={() => operate('indent')}>{$tr('Indent')}</button>
      <button disabled={view.busy || !rowsView.active} onclick={() => operate('outdent')}>{$tr('Outdent')}</button>
      <button disabled={view.busy || !rowsView.active} onclick={() => operate('reorder', -1)}>{$tr('Move up')}</button>
      <button disabled={view.busy || !rowsView.active} onclick={() => operate('reorder', 1)}>{$tr('Move down')}</button>
      <button aria-label={$tr('Delete block and descendants')} disabled={view.busy || !rowsView.active} onclick={() => { actionsOpen = false; run(remove()); }}>{$tr('Delete…')}</button>
      <details class="editing-help"><summary>{$tr('Keyboard shortcuts')}</summary><p>{$tr('Enter: split · Shift+Enter: newline · Tab / Shift+Tab: nest · Alt+Shift+↑/↓: reorder · Escape: leave text editor. Changes autosave.')}</p></details>
    </div>
  </details>
  {#if view.dirty || view.error}<span role="status" class:problem={view.error}>{view.error ? $tr('Error — save failed') : statusText($preferences.language, view.status)}{view.dirty && !view.composing ? ` · ${$tr(view.protectionError ? 'Recovery failed' : view.protection.includes('stored locally') ? 'Recovery stored' : 'Recovery pending')}` : ''}</span>{/if}
</div>
{#if view.protectionError}
  <div role="alert"><ErrorNotice message={view.protectionError} />
    <button disabled={view.busy} onclick={() => run(editor.capture())}>{$tr('Retry recovery snapshot')}</button>
  </div>
{/if}
{#each view.conflicts as conflict (conflict.block.id)}
  <section aria-label={$tr('Text save conflict')}>
    <p>{$tr('Persisted text changed. Replacing it requires your explicit choice.')}</p>
    <label>{$tr('Persisted version')}<textarea readonly value={conflict.block.content} dir="auto"></textarea></label>
    <label>{$tr('Your unsaved version')}<textarea readonly value={editor.text(conflict.block.id)} dir="auto"></textarea></label>
    <button disabled={view.busy} onclick={() => run(editor.resolveConflict(conflict.block.id))}>{$tr('Replace compared persisted text with my draft')}</button>
  </section>
{/each}
<dialog bind:this={dialog} aria-labelledby="delete-heading" oncancel={event => { event.preventDefault(); answer(false); }}>
  <h3 id="delete-heading">{$tr('Delete {count} block(s)?', { count: confirmation?.count ?? 0 })}</h3>
  <p>{$tr('The selected block and all descendants will be deleted. You can undo this during this graph session.')}</p>
  <button onclick={() => answer(false)}>{$tr('Cancel')}</button>
  <button onclick={() => answer(true)}>{$tr('Delete {count} block(s)', { count: confirmation?.count ?? 0 })}</button>
</dialog>
{#if view.error}
  <div role="alert">
    <ErrorNotice message={view.error} />
    <button disabled={view.busy} onclick={() => run(editor.retry())}>{$tr('Retry saves / clear error')}</button>
    <button disabled={view.busy} onclick={() => run(editor.discardAndReload())}>{$tr('Discard unsaved drafts and reload')}</button>
  </div>
{/if}
{#if !rowsView.rows.length}
  <button disabled={view.busy} onclick={() => run(editor.action({ kind: 'append' }))}>{$tr('Add first block')}</button>
{:else}
  <div class="outline" role="group" aria-label={$tr('Page blocks')} aria-busy={view.busy}>
    {#each rowsView.rows as row (row.block.id)}
      <BlockRow {row} {editor} {pages} text={editor.text(row.block.id)} active={rowsView.active === row.block.id}
        busy={rowsView.busy} collapsed={editor.collapsed.has(row.block.id)} focus={rowsView.focus} />
      <div style:margin-inline-start={`${Math.min(row.depth, 20) * 25 + 26}px`}>
        {#each references.filter(r => r.source_block_id === row.block.id) as reference (reference.title_key)}
          <button disabled={view.busy || editor.dirty} onclick={() => onReference(reference)}
            title={$tr(reference.target_page_id ? 'Open referenced page' : 'Create referenced page')}><bdi>[[{reference.label}]]</bdi>{#if reference.target_title && reference.target_title !== reference.label} → <bdi>{reference.target_title}</bdi>{/if}{reference.target_page_id ? '' : ` ${$tr('(create)')}`}</button>
        {/each}
      </div>
    {/each}
  </div>
{/if}

<style>
  #editor-toolbar { display: flex; flex-wrap: wrap; gap: 5px; align-items: center; margin-block: 14px 20px; color: var(--muted); font-size: 12px; }
  #editor-toolbar > button { border: 0; background: transparent; }
  #editor-toolbar > button:hover:not(:disabled) { background: var(--hover); }
  button { font: inherit; padding: 5px 8px; cursor: pointer; }
  [role='status'] { margin-inline-start: auto; font-size: 12px; }
  .problem { color: var(--danger); }
  .more-actions { position: relative; }
  .more-actions > summary { list-style: none; padding: 5px 8px; cursor: pointer; border-radius: var(--radius-control); }
  .more-actions > summary::-webkit-details-marker { display: none; }
  .more-actions > summary:hover { background: var(--hover); }
  .more-list { position: absolute; z-index: 3; inset-inline-start: 0; top: 30px; min-width: 165px; display: grid; padding: 5px; border: 1px solid var(--line); border-radius: var(--radius-control); background: var(--surface); box-shadow: 0 6px 20px #14251b20; }
  .more-list button { border: 0; background: transparent; text-align: start; }
  .more-list button:hover:not(:disabled) { background: var(--hover); }
  .editing-help { border-block-start: 1px solid var(--line); margin-block-start: 3px; padding-block-start: 3px; }
  .editing-help summary { cursor: pointer; padding: 5px 8px; }
  .editing-help p { max-width: 240px; margin: 0; padding: 5px 8px; line-height: 1.5; }
  .outline { font-size: 16px; line-height: 1.68; }
  [role='alert'] { color: var(--danger); border: 1px solid; padding: .5rem; margin-block: .5rem; }
</style>
