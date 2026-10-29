<script lang="ts">
  import { tick } from 'svelte';
  import type { Page } from './editor/types';
  import { filterPages } from './navigation';
  import { tr } from './preferences';
  let { pages, label, disabled, onOpen }: { pages: Page[]; label: (page: Page) => string; disabled: boolean; onOpen: (page: Page) => Promise<void> } = $props();
  let dialog: HTMLDialogElement;
  let input: HTMLInputElement;
  let query = $state(''), index = $state(0), composing = $state(false);
  let returnFocus: HTMLElement | null = null;
  const hits = $derived(filterPages(pages, query, label));
  export async function show() {
    if (disabled || dialog.open) return;
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    query = ''; index = 0; dialog.showModal(); await tick(); input.focus();
  }
  function close() {
    if (composing) return;
    dialog.close();
    if (returnFocus?.isConnected) returnFocus.focus();
  }
  async function open(page: Page) {
    if (composing || disabled) return;
    // Close before loading so the new page can receive focus. Failure restores
    // the source through the existing navigation guard, not a second editor.
    close(); await onOpen(page);
  }
  function keys(event: KeyboardEvent) {
    if (composing || event.isComposing || event.keyCode === 229) return;
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault(); index = Math.max(0, Math.min(hits.length - 1, index + (event.key === 'ArrowDown' ? 1 : -1)));
      document.getElementById(`quick-page-${index}`)?.scrollIntoView({ block: 'nearest' });
    } else if (event.key === 'Enter') { event.preventDefault(); if (hits[index]) void open(hits[index]); }
  }
</script>

<dialog bind:this={dialog} aria-labelledby="go-page-heading" oncancel={e => { e.preventDefault(); close(); }}>
  <h2 id="go-page-heading">{$tr('Go to page')}</h2>
  <label for="quick-page-query">{$tr('Filter page titles')}</label>
  <input bind:this={input} id="quick-page-query" dir="auto" bind:value={query} oninput={() => index = 0} onkeydown={keys}
    oncompositionstart={() => composing = true} oncompositionend={() => composing = false} aria-describedby="quick-page-help" />
  <p id="quick-page-help">{$tr('↑/↓ choose · Enter opens · Esc cancels · Up to 100 pages')}</p>
  {#if !hits.length}<p role="status">{$tr('No matching pages.')}</p>{/if}
  <ul>{#each hits as page, i (page.id)}<li><button id={`quick-page-${i}`} class:chosen={i === index} aria-current={i === index ? 'true' : undefined} disabled={disabled || composing} onclick={() => open(page)}><bdi>{label(page)}</bdi></button></li>{/each}</ul>
  <button disabled={composing} onclick={close}>{$tr('Cancel')}</button>
</dialog>

<style>
  dialog { width: min(32rem, 85vw); max-height: 85vh; border: 1px solid var(--line); border-radius: var(--radius-panel); }
  input { width: 100%; box-sizing: border-box; padding: .6rem; font: inherit; }
  label { display: block; font-weight: 600; }
  p { font-size: .85rem; }
  ul { list-style: none; padding: 0; max-height: 45vh; overflow: auto; }
  button { padding: .5rem; font: inherit; cursor: pointer; }
  li button { width: 100%; text-align: start; overflow-wrap: anywhere; margin-block: .1rem; }
  .chosen { border-color: var(--focus); background: var(--selected); }
</style>
