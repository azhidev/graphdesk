<script lang="ts">
  import { tick } from 'svelte';
  import type { RecentGraph } from './graphs';
  import { cycleCursor } from './graphList';
  import { tr } from './preferences';
  let { currentLabel, candidates, labels, cursor, disabled, ready, onPick, onSelect, onCursor }: {
    currentLabel: string; candidates: RecentGraph[]; labels: Map<string, string>;
    cursor: number; disabled: boolean; ready: boolean;
    onPick: (create: boolean) => void; onSelect: (graph: RecentGraph) => void; onCursor: (index: number) => void;
  } = $props();
  const label = (graph: RecentGraph) => labels.get(graph.path) ?? graph.path;
  async function keys(event: KeyboardEvent) {
    if (event.isComposing || event.keyCode === 229) return;
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    if (!candidates.length) return;
    event.preventDefault();
    const index = cycleCursor(cursor, event.key === 'ArrowDown' ? 1 : -1, candidates.length);
    onCursor(index);
    await tick(); document.getElementById(`graph-switch-candidate-${index}`)?.focus();
  }
</script>

<dialog id="graph-switch-popover" open aria-label={$tr('Switch graph')} onkeydown={keys}>
  <p class="eyebrow">{$tr('Graph:')}</p>
  <p class="current" dir="auto"><bdi>{currentLabel}</bdi><span class="badge">{$tr('current')}</span></p>
  <div class="actions">
    <button disabled={disabled || !ready} onfocus={() => onCursor(-1)} onclick={() => onPick(false)}>{$tr('Open graph…')}</button>
    <button disabled={disabled || !ready} onfocus={() => onCursor(-1)} onclick={() => onPick(true)}>{$tr('Create graph…')}</button>
  </div>
  {#if candidates.length}
    <p class="eyebrow" id="graph-switch-list-label">{$tr('Switch to')} <kbd dir="ltr">Ctrl+Tab</kbd></p>
    <ul aria-labelledby="graph-switch-list-label">
      {#each candidates as graph, index (graph.path)}
        <li><button id={`graph-switch-candidate-${index}`} dir="auto" class:chosen={index === cursor}
          title={graph.path} disabled={disabled} onfocus={() => onCursor(index)}
          onclick={() => onSelect(graph)} onmouseenter={() => onCursor(index)}><bdi>{label(graph)}</bdi></button></li>
      {/each}
    </ul>
  {:else}
    <p class="empty">{$tr('No other graphs yet. Open or create one above.')}</p>
  {/if}
</dialog>

<style>
  dialog { position: absolute; z-index: 12; inset-block-start: calc(100% + 6px); inset-inline-start: 0; margin: 0;
    width: min(300px, calc(100vw - 24px)); max-height: min(430px, calc(100vh - 82px)); overflow-y: auto;
    padding: 12px; border: 1px solid var(--line); border-radius: var(--radius-panel);
    background: var(--surface); box-shadow: 0 18px 45px #101a1438; }
  .eyebrow { display: flex; align-items: center; gap: 6px; margin: 5px 8px 6px; color: var(--soft); font-size: 11px; font-weight: 700; }
  .current { display: flex; align-items: center; gap: 6px; margin: 0 8px 12px; overflow: hidden; font-size: 14px; font-weight: 650; }
  .current bdi { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .badge { flex: none; border-radius: 999px; background: var(--selected); color: var(--accent-ink); padding: 1px 7px; font-size: 10px; font-weight: 650; }
  .actions { display: grid; gap: 2px; padding-block: 5px; border-block: 1px solid var(--line); }
  button { display: block; width: 100%; border: 0; border-radius: var(--radius-control); padding: 9px 8px;
    text-align: start; background: transparent; color: var(--ink); font: inherit; font-size: 13px; cursor: pointer; }
  button:hover:not(:disabled), button:focus-visible { background: var(--hover); }
  ul { margin: 0; padding: 0; list-style: none; }
  ul button { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  ul button.chosen { background: var(--selected); color: var(--accent-ink); }
  .empty { margin: 6px 8px 4px; color: var(--muted); font-size: 12px; }
  kbd { font: 10px system-ui, sans-serif; color: var(--soft); border: 1px solid var(--line); border-radius: 4px; padding: 0 4px; }
</style>
