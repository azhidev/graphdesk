<script lang="ts">
  import type { ExtensionHost, ExtensionState } from './host';
  import { preferences, tr } from '../preferences';
  import ErrorNotice from '../ErrorNotice.svelte';
  let { host, states, loading, error, disabled, onRetry }: {
    host: ExtensionHost; states: ExtensionState[]; loading: boolean; error: string; disabled: boolean; onRetry: () => void;
  } = $props();
</script>

<section aria-label={$tr('Extensions')}>
  <h3>{$tr('Extensions')}</h3>
  <p>{$tr('Bundled extensions are enabled separately for each graph. Disabling keeps their saved data.')}</p>
  {#if error}<ErrorNotice message={error} /><button disabled={disabled || loading} onclick={onRetry}>{$tr('Retry extensions')}</button>{/if}
  {#each states as state (state.manifest.id)}
    <div class="extension-setting">
      <label><input type="checkbox" checked={state.enabled} disabled={disabled || loading || state.pending || !!error}
        onchange={async event => {
          const input = event.currentTarget;
          await host.configure(state.manifest.id, input.checked);
          input.checked = host.states().find(s => s.manifest.id === state.manifest.id)?.enabled ?? false;
        }} />
        <span><strong>{state.manifest.name[$preferences.language]}</strong><small>{state.manifest.description[$preferences.language]}</small></span>
      </label>
      {#if state.error}
        <ErrorNotice message={state.error} />
        {#if state.enabled}<button disabled={disabled || state.pending || loading} onclick={() => host.configure(state.manifest.id, true)}>{$tr('Retry extensions')}</button>{/if}
      {/if}
      {#if state.status === 'starting'}<span role="status">{$tr('Loading…')}</span>{/if}
    </div>
  {/each}
</section>

<style>
  section { border-block-start: 1px solid var(--line); margin-block-start: 16px; padding-block-start: 8px; }
  h3 { font-size: 13px; margin-block: 6px; } p, small { color: var(--muted); font-size: 12px; }
  label { display: flex; align-items: flex-start; gap: 9px; padding-block: 8px; }
  small { display: block; margin-block-start: 3px; } input { flex: none; margin-block-start: 3px; }
  button { padding: 5px 8px; } .extension-setting { margin-block: 6px; }
</style>
