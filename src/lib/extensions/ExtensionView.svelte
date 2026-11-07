<script lang="ts">
  import type { ExtensionHost, ExtensionState } from './host';
  import type { ViewModel } from './contracts';
  import { preferences, tr } from '../preferences';
  import { busyControl } from '../busyControl';
  import ErrorNotice from '../ErrorNotice.svelte';
  let { host, extensionId, model, state, disabled }: {
    host: ExtensionHost; extensionId: string; model: ViewModel | undefined; state: ExtensionState | undefined; disabled: boolean;
  } = $props();
</script>

<section class="extension-document" aria-busy={state?.pending ?? false}>
  {#if model}
    <h2 id="extension-heading" tabindex="-1">{model.title[$preferences.language]}</h2>
    <p class="description">{model.description[$preferences.language]}</p>
    {#if model.value !== undefined}<p class="value"><bdi>{model.value}</bdi></p>{/if}
    <div class="actions">
      {#each model.actions as action (action.id)}
        <button use:busyControl={disabled || !!state?.pending} onclick={() => host.invoke(extensionId, action.id)}>{action.label[$preferences.language]}</button>
      {/each}
    </div>
    {#if state?.pending}<span role="status">{$tr('Saving…')}</span>{/if}
  {:else}
    <h2 id="extension-heading" tabindex="-1">{$tr('Extension unavailable')}</h2>
    <p>{$tr('Enable or retry this extension in Settings. Saved data has not been discarded.')}</p>
  {/if}
  {#if state?.error}<ErrorNotice message={state.error} />{/if}
</section>

<style>
  .extension-document { max-width: 812px; margin-inline: auto; padding: 32px clamp(16px, 5vw, 48px); overflow-wrap: anywhere; }
  h2 { margin-block: 0 8px; font-size: 24px; } .description { color: var(--muted); }
  .value { font-size: clamp(40px, 9vw, 64px); line-height: 1.2; margin-block: 24px; font-variant-numeric: tabular-nums; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; margin-block: 16px; }
  button { padding: 7px 12px; } [role='status'] { color: var(--muted); font-size: 12px; }
</style>
