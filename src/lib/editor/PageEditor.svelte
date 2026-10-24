<script lang="ts">
  import BlockRow from './BlockRow.svelte';
  import type { EditorController } from './controller';
  import type { Page, DeletePreview } from './types';
  import { tick } from 'svelte';
  import type { InlineReference } from '../references';
  let { editor, revision, disabled = false, pages, references, onReference }: {
    editor: EditorController; revision: number; disabled?: boolean; pages: Page[];
    references: InlineReference[]; onReference: (reference: InlineReference) => void;
  } = $props();
  const view = $derived.by(() => { revision; return {
    rows: editor.rows(), active: editor.activeId, focus: editor.focus,
    busy: editor.working || disabled, status: editor.status, error: editor.error,
    canUndo: editor.canUndo || editor.dirty, canRedo: editor.canRedo,
    protection: editor.protection, protectionError: editor.protectionError, conflicts: editor.conflicts,
    dirty: editor.dirty,
    composing: editor.composing,
  }; });
  let previousActive: string | null = null;
  $effect(() => {
    if (view.busy) return;
    if (previousActive && !view.active) document.getElementById('editor-toolbar')?.focus();
    previousActive = view.active;
  });
  function run(operation: Promise<void>) { void operation.catch(() => {}); }
  let confirmation: { count: number; resolve: (accepted: boolean) => void } | null = $state(null);
  let dialog: HTMLDialogElement;
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
    const id = view.active;
    if (!id) return;
    await editor.deleteSubtree(id, confirmDeletion);
    if (!editor.activeId) document.getElementById('editor-toolbar')?.focus();
  }
  function operate(kind: 'indent' | 'outdent' | 'reorder', direction = 0) {
    if (view.active) run(editor.action(kind === 'reorder' ? { kind, id: view.active, direction } : { kind, id: view.active }, editor.selection(view.active)));
  }
</script>

<div id="editor-toolbar" tabindex="-1" role="group" aria-label="Block editor actions">
  <button disabled={view.busy || !view.canUndo} onclick={() => run(editor.action({ kind: 'undo' }))}>Undo</button>
  <button disabled={view.busy || !view.canRedo} onclick={() => run(editor.action({ kind: 'redo' }))}>Redo</button>
  <button disabled={view.busy || !view.active} onclick={() => operate('indent')}>Indent</button>
  <button disabled={view.busy || !view.active} onclick={() => operate('outdent')}>Outdent</button>
  <button disabled={view.busy || !view.active} onclick={() => operate('reorder', -1)}>Move up</button>
  <button disabled={view.busy || !view.active} onclick={() => operate('reorder', 1)}>Move down</button>
  <button aria-label="Delete block and descendants" title="Delete block and descendants" disabled={view.busy || !view.active} onclick={() => run(remove())}>Delete…</button>
  <span role="status" title={view.protection}>{view.error ? 'Error — save failed' : view.status}{view.dirty && !view.composing ? view.protectionError ? ' · recovery failed' : view.protection.includes('stored locally') ? ' · recovery stored' : ' · recovery pending' : ''}</span>
</div>
{#if view.protectionError}
  <div role="alert">{view.protectionError}
    <button disabled={view.busy} onclick={() => run(editor.capture())}>Retry recovery snapshot</button>
  </div>
{/if}
{#each view.conflicts as conflict (conflict.block.id)}
  <section aria-label="Text save conflict">
    <p>Persisted text changed. Replacing it requires your explicit choice.</p>
    <label>Persisted version<textarea readonly value={conflict.block.content} dir="auto"></textarea></label>
    <label>Your unsaved version<textarea readonly value={editor.text(conflict.block.id)} dir="auto"></textarea></label>
    <button disabled={view.busy} onclick={() => run(editor.resolveConflict(conflict.block.id))}>Replace compared persisted text with my draft</button>
  </section>
{/each}
<dialog bind:this={dialog} aria-labelledby="delete-heading" oncancel={event => { event.preventDefault(); answer(false); }}>
  <h3 id="delete-heading">Delete {confirmation?.count ?? 0} block(s)?</h3>
  <p>The selected block and all descendants will be deleted. You can undo this during this graph session.</p>
  <button onclick={() => answer(false)}>Cancel</button>
  <button onclick={() => answer(true)}>Delete {confirmation?.count ?? 0} block(s)</button>
</dialog>
{#if view.error}
  <div role="alert">
    <p>{view.error}</p>
    <button disabled={view.busy} onclick={() => run(editor.retry())}>Retry saves / clear error</button>
    <button disabled={view.busy} onclick={() => run(editor.discardAndReload())}>Discard unsaved drafts and reload</button>
  </div>
{/if}
<p class="help">Enter: split · Shift+Enter: newline · Tab / Shift+Tab: nest · Alt+Shift+↑/↓: reorder · Escape: leave text editor. Changes autosave.</p>
{#if !view.rows.length}
  <button disabled={view.busy} onclick={() => run(editor.action({ kind: 'append' }))}>Add first block</button>
{:else}
  <div role="group" aria-label="Page blocks">
    {#each view.rows as row (row.block.id)}
      <BlockRow {row} {editor} {pages} text={editor.text(row.block.id)} active={view.active === row.block.id}
        busy={view.busy} collapsed={editor.collapsed.has(row.block.id)} focus={view.focus} />
      <div style:margin-inline-start={`${Math.min(row.depth, 20) * 1.3 + 2}rem`}>
        {#each references.filter(r => r.source_block_id === row.block.id) as reference (reference.title_key)}
          <button disabled={view.busy || editor.dirty} onclick={() => onReference(reference)}
            title={reference.target_page_id ? 'Open referenced page' : 'Create referenced page'}>[[{reference.label}]]{reference.target_title && reference.target_title !== reference.label ? ` → ${reference.target_title}` : ''}{reference.target_page_id ? '' : ' (create)'}</button>
        {/each}
      </div>
    {/each}
  </div>
{/if}

<style>
  #editor-toolbar { display: flex; flex-wrap: wrap; gap: .25rem; align-items: center; }
  button { font: inherit; padding: .3rem .5rem; cursor: pointer; }
  [role='status'] { margin-inline-start: auto; font-size: .85rem; }
  .help { color: #4b5d52; font-size: .8rem; }
  [role='alert'] { color: #a01818; border: 1px solid; padding: .5rem; margin-block: .5rem; }
</style>
