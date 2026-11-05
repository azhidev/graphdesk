<script lang="ts">
  import { tick } from 'svelte';
  import { busyControl } from '../busyControl';
  import { tr } from '../preferences';
  import type { TaskState } from './types';

  let { status, busy, onChange }: {
    status: TaskState | null; busy: boolean; onChange: (status: TaskState | null) => void;
  } = $props();
  const id = $props.id();
  const editableOptions = [
    { value: null, label: 'No task' }, { value: 'todo', label: 'TODO' },
    { value: 'doing', label: 'DOING' }, { value: 'done', label: 'DONE' },
  ] as const;
  // Retain the existing policy: legacy CANCELLED is readable, not a new action.
  const options = $derived(status === 'cancelled'
    ? [...editableOptions, { value: 'cancelled', label: 'CANCELLED' } as const] : editableOptions);
  const label = $derived(options.find(option => option.value === status)!.label);
  let trigger: HTMLButtonElement;
  let menu: HTMLDivElement;
  let opened = $state(false);
  let left = $state(0), top = $state(0);
  let returnFocus: HTMLElement | null = null;
  function close(restore = false) {
    menu.hidePopover();
    if (restore && returnFocus?.isConnected) returnFocus.focus({ preventScroll: true });
  }
  async function show(last = false) {
    if (busy) return;
    if (opened) { close(true); return; }
    returnFocus = document.activeElement instanceof HTMLElement && document.activeElement !== document.body ? document.activeElement : trigger;
    const rect = trigger.getBoundingClientRect();
    menu.showPopover();
    opened = true;
    await tick();
    if (busy || !menu.matches(':popover-open')) return;
    // Top-layer positioning avoids clipping by the editor's scrolling viewport.
    const width = menu.offsetWidth, height = menu.offsetHeight;
    const rtl = getComputedStyle(trigger).direction === 'rtl';
    left = Math.max(8, Math.min(rtl ? rect.right - width : rect.left, window.innerWidth - width - 8));
    top = Math.max(8, rect.bottom + height + 6 <= window.innerHeight - 8 ? rect.bottom + 6 : rect.top - height - 6);
    await tick();
    if (busy || !menu.matches(':popover-open')) return;
    const buttons = menu.querySelectorAll<HTMLButtonElement>('button');
    buttons[last ? buttons.length - 1 : options.findIndex(option => option.value === status)]?.focus({ preventScroll: true });
  }
  function choose(value: TaskState | null) {
    if (busy) return;
    close(true);
    // The trigger always displays acknowledged status, never an optimistic value.
    if (value !== status) onChange(value);
  }
  function keys(event: KeyboardEvent) {
    if (event.isComposing || event.keyCode === 229) return;
    if (event.key === 'Escape' || event.key === 'Tab') {
      if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); }
      close(true); return;
    }
    const buttons = [...menu.querySelectorAll<HTMLButtonElement>('button')];
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1
      : event.key === 'ArrowDown' ? (index + 1) % buttons.length
      : event.key === 'ArrowUp' ? (index + buttons.length - 1) % buttons.length : -1;
    if (next >= 0) { event.preventDefault(); buttons[next]?.focus({ preventScroll: true }); }
  }
  $effect(() => {
    if (!opened) return;
    if (busy) { close(true); return; }
    const dismiss = (event: Event) => { if (!(event.target instanceof Node) || !menu.contains(event.target)) close(); };
    window.addEventListener('resize', dismiss);
    window.addEventListener('scroll', dismiss, true);
    return () => {
      window.removeEventListener('resize', dismiss);
      window.removeEventListener('scroll', dismiss, true);
    };
  });
</script>

{#snippet icon(value: TaskState | null)}
  <svg class="status-icon" data-status={value ?? 'none'} viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
    {#if value === null}<path d="M4 8h8" />
    {:else}
      <circle cx="8" cy="8" r="5.75" stroke-dasharray={value === 'todo' ? '2 2' : undefined} />
      {#if value === 'doing'}<path d="M8 2.25a5.75 5.75 0 0 1 0 11.5Z" fill="currentColor" stroke="none" />
      {:else if value === 'done'}<path d="m5 8 2 2 4-4" />
      {:else if value === 'cancelled'}<path d="m4 12 8-8" />{/if}
    {/if}
  </svg>
{/snippet}

<button bind:this={trigger} class="task-status" type="button" use:busyControl={busy}
  aria-label={`${$tr('Task status')}: ${$tr(label)}`} title={`${$tr('Task status')}: ${$tr(label)}`}
  aria-haspopup="menu" aria-expanded={opened} aria-controls={id}
  onpointerdown={event => { if (event.button === 0) event.preventDefault(); }}
  onclick={() => void show()}
  onkeydown={event => {
    if (!event.isComposing && event.keyCode !== 229 && ['ArrowDown', 'ArrowUp'].includes(event.key)) {
      event.preventDefault(); void show(event.key === 'ArrowUp');
    }
  }}>
  {@render icon(status)}<span class="status-label">{$tr(label)}</span><span class="chevron" aria-hidden="true">⌄</span>
</button>
<div bind:this={menu} {id} class="status-menu" popover="auto" role="menu" tabindex="-1" aria-label={$tr('Task status')}
  style:left={`${left}px`} style:top={`${top}px`} onkeydown={keys}
  ontoggle={event => opened = event.newState === 'open'}>
  {#if opened}
    {#each options as option (option.value)}
      <button type="button" role="menuitemradio" aria-checked={status === option.value} tabindex="-1"
        use:busyControl={busy} onclick={() => choose(option.value)}>
        {@render icon(option.value)}<span>{$tr(option.label)}</span><span class="check" aria-hidden="true">{status === option.value ? '✓' : ''}</span>
      </button>
    {/each}
  {/if}
</div>

<style>
  .task-status { display: inline-flex; align-items: center; gap: 5px; flex: none; inline-size: 100px; min-block-size: 27px;
    padding: 3px 5px; border: 1px solid transparent; background: transparent; color: var(--muted); font: inherit; font-size: 11px; line-height: 19px; }
  .task-status:hover, .task-status[aria-expanded='true'] { border-color: var(--line); background: var(--hover); }
  .status-label { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .chevron { margin-inline-start: auto; color: var(--soft); }
  .status-icon { inline-size: 15px; block-size: 15px; flex: none; }
  .status-icon[data-status='doing'] { color: var(--warning); }
  .status-icon[data-status='done'] { color: var(--success); }
  .status-icon[data-status='cancelled'] { color: var(--soft); }
  .status-menu { position: fixed; inset: auto; margin: 0; inline-size: min(190px, calc(100vw - 16px)); max-block-size: calc(100vh - 16px);
    overflow-y: auto; padding: 5px; border: 1px solid var(--line); border-radius: var(--radius-panel); background: var(--surface); color: var(--ink); box-shadow: 0 12px 32px #101a1430; }
  .status-menu button { display: flex; align-items: center; gap: 9px; inline-size: 100%; padding: 7px 8px; border: 0; background: transparent; font: inherit; font-size: 12px; text-align: start; }
  .status-menu button:hover, .status-menu button:focus-visible { background: var(--hover); }
  .status-menu button[aria-checked='true'] { color: var(--accent-ink); background: var(--selected); }
  .check { margin-inline-start: auto; inline-size: 12px; }
  @media (max-width: 600px) {
    .task-status { inline-size: 36px; gap: 2px; }
    .status-label { display: none; }
  }
</style>
