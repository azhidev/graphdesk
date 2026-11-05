<script lang="ts">
  import { tick, untrack } from 'svelte';
  import type { Row } from './outline';
  import { keyIntent } from './keyboard';
  import { needsReveal } from './scroll';
  import type { EditorController, Focus } from './controller';
  import type { Page, TaskState } from './types';
  import { completion, suggestions, insertReference } from '../references';
  import { tr } from '../preferences';
  import TaskStatus from './TaskStatus.svelte';
  let { row, editor, text, active, busy, composing, collapsed, focus, pages }: {
    row: Row; editor: EditorController; text: string; active: boolean;
    busy: boolean; composing: boolean; collapsed: boolean; focus: Focus | null;
    pages: Page[];
  } = $props();
  let input: HTMLTextAreaElement | undefined = $state();
  let appliedFocus = 0;
  let caret = $state(0);
  let localText = $state(untrack(() => text));
  // A draft is local to its native input; acknowledged changes and activation
  // still reconcile against the controller's latest draft (including late saves).
  $effect(() => { text; active; localText = editor.text(row.block.id); });
  const match = $derived(active && !editor.composing ? completion(localText, caret) : null);
  const choices = $derived(match ? suggestions(pages, match.query) : []);
  function complete(title: string) {
    if (!input || editor.composing) return;
    const result = insertReference(input.value, caret, title);
    editor.setDraft(row.block.id, result.content);
    localText = result.content;
    editor.activate(row.block.id, { start: result.caret, end: result.caret });
  }
  function resize() { if (input) { input.style.height = 'auto'; input.style.height = `${input.scrollHeight}px`; } }
  function remember() {
    if (input) caret = input.selectionStart;
    if (input) editor.remember(row.block.id, { start: input.selectionStart, end: input.selectionEnd, direction: input.selectionDirection });
  }
  function report(error: unknown) { /* Controller errors remain visible with retained drafts. */ console.warn('Editor operation failed', error); }
  async function setTask(status: TaskState | null) {
    if (busy || editor.composing) return;
    try { await editor.action({ kind: 'task', id: row.block.id, expected: row.block.task_status, status }, editor.selection(row.block.id)); }
    catch (error) { report(error); }
  }
  function keydown(event: KeyboardEvent) {
    if (!input) return;
    remember();
    const intent = keyIntent({ key: event.key, shiftKey: event.shiftKey, ctrlKey: event.ctrlKey,
      metaKey: event.metaKey, altKey: event.altKey, isComposing: event.isComposing || editor.composing,
      keyCode: event.keyCode, code: event.code, start: input.selectionStart, end: input.selectionEnd,
      length: input.value.length, nested: row.block.parent_id !== null });
    if (!intent) return;
    event.preventDefault();
    if (intent === 'escape') { document.getElementById('editor-toolbar')?.focus(); return; }
    if (intent === 'up' || intent === 'down') { editor.navigate(row.block.id, intent === 'up' ? -1 : 1); return; }
    if (intent === 'remove_empty' && editor.blocks.length === 1) return;
    const selection = editor.selection(row.block.id);
    if (intent === 'split') void editor.action({ kind: 'split', id: row.block.id, start: selection.start, end: selection.end }).catch(report);
    else if (intent === 'undo' || intent === 'redo') void editor.action({ kind: intent }).catch(report);
    else if (intent === 'reorder_up' || intent === 'reorder_down') void editor.action({ kind: 'reorder', id: row.block.id, direction: intent === 'reorder_up' ? -1 : 1 }, selection).catch(report);
    else void editor.action({ kind: intent, id: row.block.id }, selection).catch(report);
  }
  $effect(() => { text; active; if (input) resize(); });
  $effect(() => {
    if (active && input && focus?.id === row.block.id && focus.serial !== appliedFocus) {
      const request = focus; appliedFocus = request.serial;
      void tick().then(() => {
        if (!input || editor.focus?.serial !== request.serial) return;
        input.focus({ preventScroll: true }); input.setSelectionRange(request.start, request.end, request.direction);
        resize();
        const viewport = input.closest<HTMLElement>('.content');
        if (viewport && needsReveal(input.getBoundingClientRect(), viewport.getBoundingClientRect())) {
          input.scrollIntoView({ block: 'nearest' });
        }
      });
    }
  });
</script>

<div class="block-row" class:editing={active} style:margin-inline-start={`${Math.min(row.depth, 20) * 25}px`} data-block-id={row.block.id}>
  {#if row.hasChildren}
    <button class="disclosure" aria-label={$tr(collapsed ? 'Expand children' : 'Collapse children')} aria-expanded={!collapsed}
      disabled={busy} onclick={() => void editor.toggle(row.block.id).catch(report)}>{collapsed ? '▸' : '▾'}</button>
  {:else}<span class="bullet" aria-hidden="true">•</span>{/if}
  {#if active || row.block.task_status}
    <TaskStatus status={row.block.task_status} busy={busy || composing} onChange={setTask} />
  {/if}
  {#if active}
    <textarea bind:this={input} value={localText} dir="auto" aria-label={$tr('Block text')} readonly={busy} rows="1"
      oninput={() => { if (input) { editor.setDraft(row.block.id, input.value); localText = input.value; remember(); resize(); } }}
      onkeydown={keydown} onselect={remember} onclick={remember} onkeyup={remember}
      onblur={() => { remember(); editor.leave(row.block.id); }}
      oncompositionstart={() => editor.composition(true)}
      oncompositionend={() => { if (input) { editor.setDraft(row.block.id, input.value); localText = input.value; } editor.composition(false); }}
      onbeforeinput={(event) => {
        if (!event.isComposing && !editor.composing && ['historyUndo', 'historyRedo'].includes(event.inputType)) {
          event.preventDefault(); void editor.action({ kind: event.inputType === 'historyUndo' ? 'undo' : 'redo' }).catch(report);
        }
      }}></textarea>
  {:else}
    <!-- This button presents document text, not an unavailable mutation. Keep it
      readable/focusable during IPC; aria-disabled plus the click guard blocks
      activation without applying native disabled styling to the whole outline. -->
    <button class="block-content" dir="auto" aria-disabled={busy} onclick={() => { if (!busy) editor.activate(row.block.id); }}>{localText || $tr('Empty block — click to edit')}</button>
  {/if}
</div>
{#if match && choices.length}
  <div role="group" aria-label={$tr('Page suggestions')}>
    {#each choices as page (page.id)}
      <button disabled={busy} onmousedown={e => e.preventDefault()} onclick={() => complete(page.title)}><bdi>{page.title}</bdi></button>
    {/each}
  </div>
{/if}

<style>
  .block-row { display: flex; align-items: flex-start; gap: 12px; min-width: 0; margin-block: 4px; padding: 3px 8px 3px 2px; border-radius: var(--radius-control); }
  .block-row:hover:not(.editing) { background: var(--hover); }
  .block-row.editing { background: var(--active-row); border-inline-start: 2px solid var(--accent); padding-inline-start: 0; }
  .block-row.editing:focus-within { outline: 2px solid var(--focus); outline-offset: 2px; }
  .bullet, .disclosure { flex: 0 0 12px; width: 12px; text-align: center; line-height: 1.68; color: var(--soft); padding: 0; margin: 0; user-select: none; }
  .editing .bullet { color: var(--accent); }
  .disclosure { background: transparent; border: 0; cursor: pointer; font-size: 13px; }
  :global(html[dir='rtl']) .disclosure[aria-expanded='false'] { transform: scaleX(-1); }
  textarea, .block-content { min-width: 0; flex: 1; font: inherit; line-height: inherit; padding: 0; margin: 0; }
  /* Text direction remains native auto; alignment follows the shared outline edge. */
  textarea, .block-content { text-align: left; }
  :global(html[dir='rtl']) textarea, :global(html[dir='rtl']) .block-content { text-align: right; }
  textarea { resize: none; overflow: hidden; border: 0; outline: 0; background: transparent; color: var(--ink); }
  textarea:focus-visible { outline: none; }
  .block-content { flex: 0 1 auto; width: max-content; max-width: calc(100% - 24px); white-space: pre-wrap; overflow-wrap: anywhere; background: transparent; border: 0; cursor: text; color: var(--ink); }
  :focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
</style>
