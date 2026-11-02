<script lang="ts">
  import type { GraphRegistry, RecentGraph } from './graphs';
  import { tr } from './preferences';
  let { current, registry, disabled, ready, onPick, onRecent }: {
    current: string; registry: GraphRegistry; disabled: boolean; ready: boolean;
    onPick: (create: boolean) => void; onRecent: (graph: RecentGraph) => void;
  } = $props();
  const name = (path: string) => path.split(/[\\/]/).at(-1) || path;
  const recent = $derived(registry.remember ? registry.entries.filter(graph => graph.path !== current) : []);
  function keys(event: KeyboardEvent) {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    const buttons = [...(event.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (index < 0 || !buttons.length) return;
    event.preventDefault(); buttons[(index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length].focus();
  }
</script>

<dialog id="graph-switch-popover" open aria-label={$tr('Switch graph')} onkeydown={keys}>
  <p class="eyebrow">{$tr('Graph:')}</p>
  <p class="current" dir="auto"><bdi>{name(current)}</bdi></p>
  <div class="actions">
    <button disabled={disabled || !ready} onclick={() => onPick(false)}>{$tr('Open graph…')}</button>
    <button disabled={disabled || !ready} onclick={() => onPick(true)}>{$tr('Create graph…')}</button>
  </div>
  {#if registry.remember && recent.length}
    <p class="eyebrow">{$tr('Recent graphs')}</p>
    <ul>{#each recent as graph (graph.path)}
      <li><button dir="auto" disabled={disabled} onclick={() => onRecent(graph)}><bdi>{name(graph.path)}</bdi></button></li>
    {/each}</ul>
  {/if}
</dialog>

<style>
  dialog { position: absolute; z-index: 12; inset-block-start: calc(100% + 6px); inset-inline-start: 0; margin: 0;
    width: min(300px, calc(100vw - 24px)); max-height: min(430px, calc(100vh - 82px)); overflow-y: auto;
    padding: 12px; border: 1px solid var(--line); border-radius: var(--radius-panel);
    background: var(--surface); box-shadow: 0 18px 45px #101a1438; }
  .eyebrow { margin: 5px 8px 6px; color: var(--soft); font-size: 11px; font-weight: 700; }
  .current { margin: 0 8px 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 14px; font-weight: 650; }
  .actions { display: grid; gap: 2px; padding-block: 5px; border-block: 1px solid var(--line); }
  button { display: block; width: 100%; border: 0; border-radius: var(--radius-control); padding: 9px 8px;
    text-align: start; background: transparent; color: var(--ink); font: inherit; font-size: 13px; cursor: pointer; }
  button:hover:not(:disabled), button:focus-visible { background: var(--hover); }
  ul { margin: 0; padding: 0; list-style: none; }
  ul button { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
