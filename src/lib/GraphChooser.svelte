<script lang="ts">
  import type { GraphRegistry, RecentGraph } from './graphs';
  import { tr } from './preferences';
  let { path = $bindable(''), composing = $bindable(false), registry, ready, disabled, inventory, onManual, onPick, onRecent, onForget }: {
    path: string; composing: boolean; registry: GraphRegistry; ready: boolean; disabled: boolean; inventory: number;
    onManual: (create: boolean) => void; onPick: (create: boolean) => void;
    onRecent: (graph: RecentGraph) => void; onForget: (path: string | null) => void;
  } = $props();
  let advanced = $state(false);
  function filename(value: string) { return value.split(/[\\/]/).at(-1) || value; }
</script>

<section class="chooser" aria-label={$tr('Graph selection')}>
  <div class="identity"><span class="mark" aria-hidden="true"><i></i><i></i><i></i><i></i></span>
    <h1>GraphDesk</h1><p>{$tr('Open a local graph to begin your daily workspace.')}</p></div>
  {#if inventory}<p class="recovery" role="status">{$tr('Recovery drafts found for {count} graph(s). Open the original graph to compare and recover text.', { count: inventory })}</p>{/if}
  {#if !ready}<p role="status">{$tr('Loading…')}</p>{/if}
  {#if !advanced}<div class="actions">
    <button class="primary" disabled={disabled || !ready || composing} onclick={() => onPick(false)}>{$tr('Open graph…')}</button>
    <button disabled={disabled || !ready || composing} onclick={() => onPick(true)}>{$tr('Create graph…')}</button>
  </div>{/if}
  {#if registry.remember && registry.entries.length}
    <div class="recent"><h2>{$tr('Recent graphs')}</h2>
      <ul>{#each registry.entries as graph (graph.path)}
        <li><button class="recent-graph" dir="auto" disabled={disabled || !ready} onclick={() => onRecent(graph)}><bdi>{filename(graph.path)}</bdi></button>
          <button class="forget" disabled={disabled || !ready} onclick={() => onForget(graph.path)} aria-label={`${$tr('Forget graph')} · ${filename(graph.path)}`} title={$tr('Forget graph')}>×</button></li>
      {/each}</ul>
    </div>
  {/if}
  <details class="advanced" bind:open={advanced}>
    <summary>{$tr('Database path (on this computer)')}</summary>
    <div class="advanced-fields">
      <label for="path">{$tr('Database path (on this computer)')}</label>
      <input id="path" bind:value={path} dir="ltr" placeholder={$tr('Full path to notes.sqlite3')} disabled={disabled || !ready}
        oncompositionstart={() => composing = true} oncompositionend={e => { path = e.currentTarget.value; composing = false; }} />
      <div class="manual-actions">
        <button disabled={disabled || !ready || composing || !path.trim()} onclick={() => onManual(false)}>{$tr('Open graph')}</button>
        <button disabled={disabled || !ready || composing || !path.trim()} onclick={() => onManual(true)}>{$tr('Create graph')}</button>
      </div>
      <p class="hint">{$tr('The parent directory must exist. Create never overwrites an existing file.')}</p>
    </div>
  </details>
</section>

<style>
  .chooser { width: min(480px, 100%); margin-inline: auto; padding: 32px; border: 1px solid var(--line);
    border-radius: var(--radius-panel); background: var(--surface); box-shadow: 0 14px 50px #101a1410; }
  .identity { text-align: center; }.identity h1 { margin: 15px 0 5px; font-size: 26px; letter-spacing: -.035em; }
  .identity p { margin: 0; color: var(--muted); font-size: 13px; line-height: 1.5; }
  .mark { display: grid; grid-template-columns: repeat(2, 9px); gap: 3px; width: max-content; margin-inline: auto; transform: rotate(-9deg); }
  .mark i { width: 9px; height: 9px; background: var(--accent); border-radius: 2px; }.mark i:nth-child(2) { opacity: .47; }.mark i:nth-child(3) { opacity: .7; }.mark i:nth-child(4) { opacity: .32; }
  .actions { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; margin-block-start: 30px; }
  button { cursor: pointer; font: inherit; border: 1px solid var(--line); background: var(--surface); border-radius: var(--radius-control); color: var(--ink); padding: 11px 12px; }
  button:hover:not(:disabled) { background: var(--hover); }.actions .primary { background: var(--accent); color: var(--surface); border-color: var(--accent); }
  .actions .primary:hover:not(:disabled) { filter: brightness(.94); }
  .recovery { margin: 22px 0 0; padding: 12px; border: 1px solid var(--warning); border-radius: var(--radius-control); font-size: 12px; line-height: 1.5; }
  .recent { margin-block-start: 26px; }.recent h2 { margin: 0 0 8px; color: var(--soft); font-size: 11px; font-weight: 700; }
  ul { list-style: none; margin: 0; padding: 0; max-height: 180px; overflow-y: auto; }
  li { display: flex; align-items: center; gap: 4px; border-radius: var(--radius-control); }
  li:hover { background: var(--hover); }.recent-graph { flex: 1; min-width: 0; text-align: start; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; border: 0; background: transparent; padding: 8px; }
  .forget { flex: none; border: 0; background: transparent; padding: 6px 10px; font-size: 18px; color: var(--muted); }
  .advanced { margin-block-start: 22px; border-block-start: 1px solid var(--line); padding-block-start: 15px; font-size: 12px; color: var(--muted); }
  summary { cursor: pointer; width: fit-content; }.advanced-fields { margin-block-start: 14px; }
  label { display: block; font-weight: 600; }.advanced input { box-sizing: border-box; width: 100%; margin-block: 6px 10px; padding: 9px; border: 1px solid var(--line); border-radius: var(--radius-control); background: var(--surface); color: var(--ink); font: inherit; }
  .manual-actions { display: flex; flex-wrap: wrap; gap: 8px; }.manual-actions button { padding: 8px 12px; font-size: 12px; }
  .hint { font-size: 11px; line-height: 1.5; }
  @media (max-width: 440px) { .chooser { padding: 20px; }.actions { grid-template-columns: 1fr; } }
</style>
