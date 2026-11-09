<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type { Habit, HabitsRequest } from '../extensions/habits-contracts';
  import { preferencesApi } from '../api';
  import { tr } from '../preferences';
  import { definitionInput, type DefinitionDraft } from './forms';
  import { instantInput } from './time';
  import TimestampField from './TimestampField.svelte';
  let { habit, disabled, onSave }: { habit: Habit | null; disabled: boolean; onSave: (request: HabitsRequest) => Promise<void> } = $props();
  const original = untrack(() => habit);
  const id = original?.id ?? crypto.randomUUID();
  const schedule = original?.definition.schedule;
  let draft = $state<DefinitionDraft>({ name: original?.definition.name ?? '', kind: original?.definition.kind ?? 'positive', anchor: instantInput(schedule?.anchor), every: schedule?.every ?? 1, unit: schedule?.unit ?? 'days', mode: schedule?.mode ?? 'fixed', zone: schedule?.timezone ?? (original ? 'UTC' : '') });
  let issue = $state(''), zoneLoading = $state(!original);
  onMount(() => {
    let live = true;
    if (!original) void preferencesApi.habitTimezone().then(r => { if (live && !draft.zone) draft.zone = r.resolved ?? ''; }).catch(() => { if (live) issue = 'System timezone unavailable. Choose an IANA timezone.'; }).finally(() => { if (live) zoneLoading = false; });
    return () => { live = false; };
  });
  async function save(e: SubmitEvent) {
    e.preventDefault(); if (disabled || zoneLoading) return;
    try {
      const definition = definitionInput(draft, original); issue = '';
      await onSave(original ? { op: 'update', id, expected_revision: original.revision, definition } : { op: 'create', input: { id, definition } });
    } catch (e) { issue = e instanceof Error ? e.message : String(e); }
  }
</script>
<form onsubmit={save}>
  <label>{$tr('Name')}<input dir="auto" bind:value={draft.name} maxlength="512" required {disabled} /></label>
  <label>{$tr('Kind')}<select bind:value={draft.kind} disabled={disabled || !!original}><option value="positive">{$tr('Positive')}</option><option value="negative">{$tr('Negative')}</option></select></label>
  {#if draft.kind === 'positive'}
    <p class="hint">{$tr('Enter Gregorian timestamps with Z (UTC) or an explicit offset. The preview uses your display calendar and the schedule zone.')}</p>
    <TimestampField label="First occurrence" bind:value={draft.anchor} zone={draft.zone || 'UTC'} {disabled} />
    <div class="pair"><label>{$tr('Interval')}<input type="number" min="1" max="10000" step="1" bind:value={draft.every} required {disabled} /></label>
    <label>{$tr('Unit')}<select bind:value={draft.unit} {disabled}>{#each ['hours', 'days', 'weeks', 'months', 'years'] as unit}<option value={unit}>{$tr(unit as 'hours' | 'days' | 'weeks' | 'months' | 'years')}</option>{/each}</select></label></div>
    <label>{$tr('Mode')}<select bind:value={draft.mode} {disabled}><option value="fixed">{$tr('Fixed')}</option><option value="after_completion">{$tr('After Completion')}</option></select></label>
    <label>{$tr('Timezone')}<input dir="ltr" bind:value={draft.zone} placeholder="Asia/Tehran" required disabled={disabled || zoneLoading} /></label>
    <p class="summary">{$tr('Every')} <bdi>{draft.every}</bdi> {$tr(draft.unit)} · {$tr(draft.mode === 'fixed' ? 'Fixed' : 'After Completion')}<br />
      {$tr(draft.mode === 'fixed' ? 'Only the current occurrence is due; missed occurrences do not accumulate.' : 'The next deadline follows the latest completion.')}</p>
    {#if draft.unit === 'months' || draft.unit === 'years'}<p class="hint">{$tr('Monthly and yearly recurrence uses Gregorian boundaries, not Jalali recurrence.')}</p>{/if}
    {#if original}<p class="hint">{$tr('Changing timezone preserves the anchor instant. Edit the timestamp too to change its local clock. Review the preview before saving.')}</p>{/if}
  {:else}<p class="hint">{$tr('No schedule or reminders. Missing reports mean unknown, not success.')}</p>{/if}
  {#if issue}<p role="alert">{$tr(issue as Parameters<typeof $tr>[0])}</p>{/if}
  <button class="primary" disabled={disabled || zoneLoading}>{$tr('Save habit')}</button>
</form>
<style>
  form, label { display: grid; gap: 7px; } form { gap: 15px; } input, select { width: 100%; padding: 8px; } .pair { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; } p { margin: 0; } .hint { color: var(--muted); font-size: 12px; } .summary { padding: 12px; background: var(--hover); border-radius: var(--radius-control); } button { padding: 9px 14px; } [role='alert'] { color: var(--danger); }
</style>
