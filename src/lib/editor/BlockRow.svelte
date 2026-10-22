<script lang="ts">
  import { tick } from 'svelte';
  import type { Row } from './outline';
  import { keyIntent } from './keyboard';
  import type { EditorController, Focus } from './controller';
  import type { Page, TaskState } from './types';
  import { completion, suggestions, insertReference } from '../references';
  let { row, editor, text, active, busy, collapsed, focus, pages }: {
    row: Row; editor: EditorController; text: string; active: boolean;
    busy: boolean; collapsed: boolean; focus: Focus | null;
    pages: Page[];
  } = $props();
  let input: HTMLTextAreaElement | undefined = $state();
  let appliedFocus = 0;
  let caret = $state(0);
  const match = $derived(active && !editor.composing ? completion(text, caret) : null);
  const choices = $derived(match ? suggestions(pages, match.query) : []);
  function complete(title: string) {
    if (!input || editor.composing) return;
    const result = insertReference(input.value, caret, title);
    editor.setDraft(row.block.id, result.content);
    editor.activate(row.block.id, { start: result.caret, end: result.caret });
  }
  function resize() { if (input) { input.style.height = 'auto'; input.style.height = `${input.scrollHeight}px`; } }
  function remember() {
    if (input) caret = input.selectionStart;
    if (input) editor.remember(row.block.id, { start: input.selectionStart, end: input.selectionEnd, direction: input.selectionDirection });
  }
  function report(error: unknown) { /* Controller errors remain visible with retained drafts. */ console.warn('Editor operation failed', error); }
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
        input.focus(); input.setSelectionRange(request.start, request.end, request.direction);
        resize(); input.scrollIntoView({ block: 'nearest' });
      });
    }
  });
</script>

<div class="block-row" style:margin-inline-start={`${Math.min(row.depth, 20) * 1.3}rem`} data-block-id={row.block.id}>
  <select aria-label="Task status" value={row.block.task_status ?? ''} disabled={busy}
    onchange={event => void editor.action({ kind: 'task', id: row.block.id, expected: row.block.task_status,
      status: (event.currentTarget.value || null) as TaskState | null }).catch(report)}>
    <option value="">No task</option><option value="todo">TODO</option>
    <option value="doing">DOING</option><option value="done">DONE</option>
    {#if row.block.task_status === 'cancelled'}<option value="cancelled">CANCELLED</option>{/if}
  </select>
  {#if row.hasChildren}
    <button class="disclosure" aria-label={collapsed ? 'Expand children' : 'Collapse children'} aria-expanded={!collapsed}
      disabled={busy} onclick={() => void editor.toggle(row.block.id).catch(report)}>{collapsed ? '▸' : '▾'}</button>
  {:else}<span class="bullet" aria-hidden="true">•</span>{/if}
  {#if active}
    <textarea bind:this={input} value={text} dir="auto" aria-label="Block text" readonly={busy} rows="1"
      oninput={() => { if (input) { editor.setDraft(row.block.id, input.value); remember(); resize(); } }}
      onkeydown={keydown} onselect={remember} onclick={remember} onkeyup={remember}
      onblur={() => { remember(); editor.leave(row.block.id); }}
      oncompositionstart={() => editor.composition(true)}
      oncompositionend={() => { if (input) editor.setDraft(row.block.id, input.value); editor.composition(false); }}
      onbeforeinput={(event) => {
        if (!event.isComposing && !editor.composing && ['historyUndo', 'historyRedo'].includes(event.inputType)) {
          event.preventDefault(); void editor.action({ kind: event.inputType === 'historyUndo' ? 'undo' : 'redo' }).catch(report);
        }
      }}></textarea>
  {:else}
    <button class="block-content" dir="auto" disabled={busy} onclick={() => editor.activate(row.block.id)}>{text || 'Empty block — click to edit'}</button>
  {/if}
</div>
{#if match && choices.length}
  <div role="group" aria-label="Page suggestions">
    {#each choices as page (page.id)}
      <button disabled={busy} onmousedown={e => e.preventDefault()} onclick={() => complete(page.title)}>{page.title}</button>
    {/each}
  </div>
{/if}

<style>
  .block-row { display: flex; align-items: flex-start; gap: .35rem; padding-block: .2rem; }
  .bullet, .disclosure { flex: 0 0 1.5rem; text-align: center; margin: .2rem 0; padding: 0; }
  .disclosure { background: transparent; border: 1px solid transparent; cursor: pointer; font-size: 1rem; }
  textarea, .block-content { box-sizing: border-box; min-width: 0; flex: 1; font: inherit; line-height: 1.5; padding: .3rem .45rem; margin: 0; }
  textarea { resize: none; overflow: hidden; border: 1px solid #71958b; border-radius: .2rem; background: white; }
  .block-content { white-space: pre-wrap; overflow-wrap: anywhere; text-align: start; background: transparent; border: 1px solid transparent; cursor: text; }
  :focus-visible { outline: 2px solid #275d50; outline-offset: 2px; }
</style>
