<script lang="ts">
  import { untrack } from 'svelte';
  import type { Habit, EventKind, HabitsRequest } from '../extensions/habits-contracts';
  import { tr } from '../preferences';
  import { eventInput } from './forms';
  import { instantInput } from './time';
  import TimestampField from './TimestampField.svelte';
  let { habit, disabled, onSave }: { habit: Habit; disabled: boolean; onSave: (request: HabitsRequest) => Promise<void> } = $props();
  const id = crypto.randomUUID();
  let kind = $state<EventKind>(untrack(() => habit.definition.kind === 'positive' ? 'completion' : 'lapse'));
  let time = $state(instantInput()), start = $state(''), end = $state(''), issue = $state('');
  const zone = $derived(habit.definition.schedule?.timezone ?? 'UTC');
  async function save(e: SubmitEvent) {
    e.preventDefault(); if (disabled) return;
    try { const input = eventInput(habit, id, kind, time, start, end); issue = ''; await onSave({ op: 'record', input }); }
    catch (e) { issue = e instanceof Error ? e.message : String(e); }
  }
</script>
<form onsubmit={save}>
  <p><bdi dir="auto">{habit.definition.name}</bdi></p>
  <p class="hint">{$tr('Enter Gregorian timestamps with Z (UTC) or an explicit offset. The preview uses your display calendar and the schedule zone.')}</p>
  <p class="hint">{$tr('Timezone')}: <bdi>{zone}</bdi></p>
  {#if habit.definition.kind === 'negative'}
    <label>{$tr('Event kind')}<select bind:value={kind} {disabled}><option value="lapse">{$tr('lapse')}</option><option value="confirmation">{$tr('confirmation')}</option></select></label>
    <p class="hint">{$tr('No schedule or reminders. Missing reports mean unknown, not success.')}</p>
  {/if}
  <TimestampField label="Event time" bind:value={time} {zone} {disabled} />
  {#if kind === 'confirmation'}
    <TimestampField label="Period start (inclusive)" bind:value={start} {zone} {disabled} />
    <TimestampField label="Period end (exclusive)" bind:value={end} {zone} {disabled} />
    <p class="hint">{$tr('Confirmation is an explicit report for this period, interpreted in UTC; it does not infer success for missing reports.')}</p>
  {/if}
  {#if issue}<p role="alert">{$tr(issue as Parameters<typeof $tr>[0])}</p>{/if}
  <button disabled={disabled}>{$tr('Save event')}</button>
</form>
<style>
  form, label { display: grid; gap: 8px; } form { gap: 16px; } p { margin: 0; } .hint { font-size: 12px; color: var(--muted); } select { padding: 8px; } button { padding: 9px; } [role='alert'] { color: var(--danger); }
</style>
