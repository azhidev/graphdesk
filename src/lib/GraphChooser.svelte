<script lang="ts">
  import type { GraphRegistry, RecentGraph } from './graphs';
  import { tr } from './preferences';
  let { path = $bindable(''), composing = $bindable(false), registry, ready, disabled, onManual, onPick, onRecent, onSettings, onForget }: {
    path: string; composing: boolean; registry: GraphRegistry; ready: boolean; disabled: boolean;
    onManual: (create: boolean) => void; onPick: (create: boolean) => void;
    onRecent: (graph: RecentGraph) => void; onSettings: (remember: boolean, reopen: boolean) => void | Promise<void>;
    onForget: (path: string | null) => void;
  } = $props();
  function filename(value: string) { return value.split(/[\\/]/).at(-1) || value; }
</script>

<section aria-label={$tr('Graph selection')}>
  <label for="path">{$tr('Database path (on this computer)')}</label>
  <input id="path" bind:value={path} dir="ltr" placeholder={$tr('Full path to notes.sqlite3')} disabled={disabled || !ready}
    oncompositionstart={() => composing = true} oncompositionend={e => { path = e.currentTarget.value; composing = false; }} />
  <button disabled={disabled || !ready || composing || !path.trim()} onclick={() => onManual(false)}>{$tr('Open graph')}</button>
  <button disabled={disabled || !ready || composing || !path.trim()} onclick={() => onManual(true)}>{$tr('Create graph')}</button>
  <p class="hint">{$tr('The parent directory must exist. Create never overwrites an existing file.')}</p>
  <div>
    <button disabled={disabled || !ready || composing} onclick={() => onPick(false)}>{$tr('Open graph…')}</button>
    <button disabled={disabled || !ready || composing} onclick={() => onPick(true)}>{$tr('Create graph…')}</button>
  </div>
  {#if !ready}<p role="status">{$tr('Loading…')}</p>{/if}
  <label class="check"><input type="checkbox" checked={registry.remember} disabled={disabled || !ready} onchange={async e => { const input = e.currentTarget; try { await onSettings(input.checked, false); } finally { input.checked = registry.remember; } }} />{$tr('Remember recent graphs')}</label>
  <label class="check"><input type="checkbox" checked={registry.reopen} disabled={disabled || !ready || !registry.remember} onchange={async e => { const input = e.currentTarget; try { await onSettings(true, input.checked); } finally { input.checked = registry.reopen; } }} />{$tr('Reopen last graph on startup')}</label>
  <p class="hint">{$tr('Remembering stores file paths privately on this computer, not notes. Turning it off clears remembered paths.')}</p>
  {#if registry.remember}
    <h2>{$tr('Recent graphs')}</h2>
    {#if registry.entries.length}
      <button disabled={disabled} onclick={() => onRecent(registry.entries[0])}>{$tr('Reopen last graph')}</button>
      <ul>{#each registry.entries as graph (graph.path)}
        <li><button class="recent" disabled={disabled} onclick={() => onRecent(graph)}><strong><bdi dir="auto">{filename(graph.path)}</bdi></strong><span><bdi dir="ltr">{graph.path}</bdi></span></button>
          <button disabled={disabled} onclick={() => onForget(graph.path)} aria-label={`${$tr('Forget graph')} · ${graph.path}`}>{$tr('Forget graph')}</button></li>
      {/each}</ul>
      <button disabled={disabled} onclick={() => onForget(null)}>{$tr('Clear remembered graphs')}</button>
    {:else}<p>{$tr('No remembered graphs. Open a graph to add it here.')}</p>{/if}
  {/if}
</section>

<style>
  section { padding: 1rem; border: 1px solid var(--line); border-radius: var(--radius-panel); background: var(--surface); }
  label { display: block; font-weight: 600; }
  input:not([type='checkbox']) { box-sizing: border-box; width: 100%; padding: .6rem; margin: .3rem 0 .6rem; font: inherit; }
  .check { font-weight: normal; margin-block: .5rem; }
  .check input { margin-inline-end: .5rem; }
  button { padding: .5rem .8rem; margin: .2rem; cursor: pointer; font: inherit; max-width: 100%; }
  .hint { font-size: .85rem; color: var(--muted); }
  ul { list-style: none; padding: 0; max-height: 36vh; overflow: auto; }
  li { display: flex; align-items: center; flex-wrap: wrap; }
  .recent { flex: 1; min-width: 12rem; text-align: start; }
  span { display: block; font-size: .85rem; overflow-wrap: anywhere; }
</style>
