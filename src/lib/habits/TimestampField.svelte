<script lang="ts">
  import { preferences, tr } from '../preferences';
  import { parseInstant, zonedLabel } from './time';
  import type { MessageKey } from '../i18n';
  let { label, value = $bindable(''), zone = 'UTC', disabled = false }: { label: MessageKey; value: string; zone?: string; disabled?: boolean } = $props();
  const preview = $derived.by(() => {
    try { return zonedLabel(parseInstant(value), zone, $preferences.calendar, $preferences.language); }
    catch { return ''; }
  });
</script>
<label><span>{$tr(label)}</span><input dir="ltr" bind:value {disabled} required placeholder="2026-11-09T09:00:00+03:30" autocomplete="off" spellcheck="false" />
  {#if preview}<small><bdi>{preview}</bdi></small>{/if}
</label>
<style>
  label { display: grid; gap: 5px; min-width: 0; } input { width: 100%; padding: 8px; font-variant-numeric: tabular-nums; } small { color: var(--muted); overflow-wrap: anywhere; }
</style>
