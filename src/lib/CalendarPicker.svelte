<script lang="ts">
  import { calendarFields, calendarDate, dateNumber, CalendarError, type Calendar } from './dates';
  import { tr } from './preferences';
  import { untrack } from 'svelte';
  let { day, calendar, disabled, onOpen }: { day: string; calendar: Calendar; disabled: boolean; onOpen: (day: string) => void } = $props();
  let year = $state(''), month = $state(''), date = $state(''), issue = $state('');
  $effect(() => {
    const value = day, system = calendar;
    untrack(() => {
      try { const fields = calendarFields(value, system); year = String(fields.year); month = String(fields.month); date = String(fields.day); issue = ''; }
      catch (e) { year = ''; month = ''; date = ''; issue = e instanceof CalendarError && e.name === 'unsupported' ? 'unsupported' : 'invalid'; }
    });
  });
  function open(event: SubmitEvent) {
    event.preventDefault();
    try { const value = calendarDate({ year: dateNumber(year), month: dateNumber(month), day: dateNumber(date) }, calendar); issue = ''; onOpen(value); }
    catch (e) { issue = e instanceof CalendarError && e.name === 'unsupported' ? 'unsupported' : 'invalid'; }
  }
</script>
<form onsubmit={open} aria-label={$tr('Journal date')}>
  <div class="fields" dir="ltr">
    <label>{$tr('Year')}<input aria-label={$tr('Year')} inputmode="numeric" bind:value={year} disabled={disabled} /></label>
    <label>{$tr('Month')}<input aria-label={$tr('Month')} inputmode="numeric" bind:value={month} disabled={disabled} /></label>
    <label>{$tr('Day')}<input aria-label={$tr('Day')} inputmode="numeric" bind:value={date} disabled={disabled} /></label>
  </div>
  {#if issue}<p role="alert">{$tr(issue === 'unsupported' ? 'This date cannot be used in Jalali. Its Gregorian identity is unchanged.' : 'Enter a valid date.')}</p>{/if}
  <button disabled={disabled}>{$tr('Open journal')}</button>
</form>
<style>
  .fields { display: grid; grid-template-columns: 2fr 1fr 1fr; gap: .3rem; }
  label { min-width: 0; font-size: .85rem; } input { width: 100%; box-sizing: border-box; font: inherit; padding: .4rem; }
  button { font: inherit; padding: .5rem; margin-block: .4rem; } [role='alert'] { color: var(--danger); }
</style>
