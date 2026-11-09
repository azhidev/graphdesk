<script lang="ts">
  import { onMount, onDestroy, tick, untrack } from 'svelte';
  import type { ExtensionAPI } from '../extensions/contracts';
  import type { Habit, HabitsRequest } from '../extensions/habits-contracts';
  import { preferences, tr } from '../preferences';
  import { HabitsController } from './controller';
  import HabitForm from './HabitForm.svelte';
  import EventForm from './EventForm.svelte';
  import { zonedLabel } from './time';
  import ErrorNotice from '../ErrorNotice.svelte';
  let { client, disabled, onGuard }: { client: ExtensionAPI['habits']; disabled: boolean; onGuard: (blocked: boolean) => void } = $props();
  let revision = $state(0);
  const model = new HabitsController(untrack(() => client), () => { revision++; });
  const snapshot = $derived.by(() => { revision; return { habits: model.habits, events: model.events, selected: model.selected, statuses: model.statuses, busy: model.busy, pending: model.pending, error: model.error, more: model.more, moreEvents: model.moreEvents, loaded: model.loaded }; });
  let filter = $state<'due' | 'all' | 'archived'>('due');
  let dialog: HTMLDialogElement;
  let form = $state<'habit' | 'event' | null>(null), editing = $state<Habit | null>(null);
  let discard = $state(false), composing = $state(false);
  let returnFocus: HTMLElement | null = null;
  const blocked = $derived(disabled || snapshot.busy || !!snapshot.pending);
  const rows = $derived(snapshot.habits.filter(h => filter === 'archived' ? h.definition.archived : !h.definition.archived && (filter === 'all' || h.definition.kind === 'negative' || snapshot.statuses.get(h.id)?.positive?.due)));
  $effect(() => { onGuard(!!form || snapshot.busy || !!snapshot.pending || composing); });
  onMount(() => {
    void model.load();
    // One timer for the bounded displayed page, paused while hidden or editing.
    const timer = setInterval(() => { if (!document.hidden && !form && !disabled) void model.refresh(); }, 60000);
    return () => clearInterval(timer);
  });
  onDestroy(() => { model.dispose(); onGuard(false); });
  function date(time: number, habit: Habit) {
    try { return zonedLabel(time, habit.definition.schedule?.timezone ?? 'UTC', $preferences.calendar, $preferences.language); } catch { return String(time) + ' UTC'; }
  }
  async function openForm(kind: 'habit' | 'event', habit: Habit | null = null) {
    if (blocked) return;
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    editing = habit; form = kind; discard = false;
    await tick(); dialog.showModal(); dialog.querySelector<HTMLInputElement>('input')?.focus();
  }
  async function closeForm() {
    dialog.close(); form = null; discard = false; composing = false;
    await tick(); if (returnFocus?.isConnected) returnFocus.focus(); else document.getElementById('extension-heading')?.focus();
  }
  async function cancel() {
    if (snapshot.busy || snapshot.pending || composing) return;
    discard = true; await tick(); dialog.querySelector<HTMLButtonElement>('.discard-guard button')?.focus();
  }
  async function keepEditing() { discard = false; await tick(); dialog.querySelector<HTMLInputElement>('input')?.focus(); }
  async function save(request?: HabitsRequest) {
    if (disabled || composing) return;
    const ok = await model.mutate(request);
    if (ok && form) await closeForm();
  }
  async function abandon() {
    // This is an explicit acknowledgement that the original operation may have
    // committed; reload never replays it and fresh edits use fresh revisions.
    await model.reconcile();
    if (form) await closeForm();
  }
  async function openDetails(habit: Habit) {
    if (blocked) return;
    await model.details(habit); await tick();
    if (model.selected?.id === habit.id) document.getElementById('habit-detail-heading')?.focus();
  }
  async function backToHabits() {
    const id = model.selected?.id;
    model.selected = null; model.events = []; revision++;
    await tick(); (document.getElementById(`habit-${id}`) ?? document.getElementById('extension-heading'))?.focus();
  }
</script>

{#snippet feedback()}
  {#if snapshot.pending && !snapshot.busy}
    <div class="uncertain" role="alert">
      <strong>{$tr('Mutation not acknowledged. It may already be saved, or its revision may conflict.')}</strong>
      <p>{$tr('Retry keeps the original ID and payload. Revision conflicts are never overwritten. Reload to review saved state before editing again.')}</p>
      <div class="actions"><button disabled={snapshot.busy || disabled} onclick={() => save()}>{$tr('Retry same mutation')}</button><button disabled={snapshot.busy || disabled} onclick={abandon}>{$tr('Stop retrying and reload (discard form)')}</button></div>
    </div>
  {/if}
  {#if snapshot.error}<ErrorNotice message={snapshot.error} />{#if !snapshot.pending}<button disabled={snapshot.busy || disabled} onclick={() => snapshot.selected ? model.details(snapshot.selected) : model.load()}>{$tr('Retry')}</button>{/if}{/if}
{/snippet}

<section class="habits" aria-busy={snapshot.busy}>
  <header><div><h2 id="extension-heading" tabindex="-1">{$tr('Habits')}</h2><p>{$tr('A small practice, one occurrence at a time.')}</p></div><button class="primary" disabled={blocked} onclick={() => openForm('habit')}>{$tr('New habit')}</button></header>
  {#if !form}{@render feedback()}{/if}
  {#if snapshot.busy}<p role="status">{$tr('Loading…')}</p>{/if}
  {#if snapshot.selected}
    {@const habit = snapshot.selected}
    <div class="detail">
      <button disabled={blocked} onclick={backToHabits}>{$tr('Back to habits')}</button>
      <h3 id="habit-detail-heading" tabindex="-1" dir="auto">{habit.definition.name}</h3>
      <div class="actions">
        <button disabled={blocked} onclick={() => openForm('habit', habit)}>{$tr('Edit habit')}</button>
        <button disabled={blocked} onclick={() => save({ op: 'update', id: habit.id, expected_revision: habit.revision, definition: { ...habit.definition, archived: !habit.definition.archived } })}>{$tr(habit.definition.archived ? 'Unarchive' : 'Archive')}</button>
        <button disabled={blocked || habit.definition.archived} onclick={() => openForm('event', habit)}>{$tr(habit.definition.kind === 'positive' ? 'Complete' : 'Record lapse')}</button>
      </div>
      {#if habit.definition.archived}<p>{$tr('Archived habits retain history and cannot accept new events.')}</p>{/if}
      {#if habit.definition.schedule}<p><bdi>{date(habit.definition.schedule.anchor, habit)}</bdi> · {$tr('Every')} <bdi>{habit.definition.schedule.every}</bdi> {$tr(habit.definition.schedule.unit)} · {$tr(habit.definition.schedule.mode === 'after_completion' ? 'After Completion' : 'Fixed')}</p>{:else}<p>{$tr('No schedule or reminders. Missing reports mean unknown, not success.')}</p>{/if}
      <h3>{$tr('History')}</h3>
      <p class="hint">{$tr('To correct time or kind, void the original, then record a replacement. Original payloads are retained.')}</p>
      <p class="hint">{$tr('Timezone')}: <bdi>{habit.definition.schedule?.timezone ?? 'UTC'}</bdi></p>
      {#if !snapshot.events.length && !snapshot.busy}<p>{$tr('No events yet.')}</p>{/if}
      <ul>{#each snapshot.events as event (event.id)}<li class:voided={event.voided}>
        <div><strong>{$tr(event.kind)}</strong>{#if event.voided} · {$tr('Voided')}{/if}<br /><bdi>{date(event.occurred_at, habit)}</bdi>
          {#if event.period_start !== null && event.period_end !== null}<p>{$tr('Period start (inclusive)')}: <bdi>{date(event.period_start, habit)}</bdi><br />{$tr('Period end (exclusive)')}: <bdi>{date(event.period_end, habit)}</bdi></p>{/if}
        </div><button disabled={blocked} onclick={() => save({ op: 'correct', event_id: event.id, expected_revision: event.revision, voided: !event.voided })}>{$tr(event.voided ? 'Restore' : 'Void')}</button>
      </li>{/each}</ul>
      <div class="actions"><button disabled={blocked} onclick={() => model.details(habit)}>{$tr('Latest events')}</button>{#if snapshot.moreEvents}<button disabled={blocked} onclick={() => model.details(habit, true)}>{$tr('Older events')}</button>{/if}</div>
    </div>
  {:else}
    <nav aria-label={$tr('Habit filters')} class="filters">{#each ['due', 'all', 'archived'] as tab}<button aria-pressed={filter === tab} onclick={() => filter = tab as typeof filter}>{$tr(tab === 'due' ? 'Due and negative' : tab === 'all' ? 'All active' : 'Archived')}</button>{/each}</nav>
    {#if snapshot.loaded && !rows.length && !snapshot.busy}<div class="empty"><h3>{$tr('No habits here yet.')}</h3><p>{$tr('Create a habit, choose another filter, or browse the next page.')}</p><button disabled={blocked} onclick={() => openForm('habit')}>{$tr('New habit')}</button></div>{/if}
    <ul>{#each rows as habit (habit.id)}<li>
      <div class="row-content"><button id={`habit-${habit.id}`} class="name" disabled={blocked} onclick={() => openDetails(habit)}><bdi dir="auto">{habit.definition.name}</bdi></button>
        <p class="hint">{#if habit.definition.kind === 'negative'}{$tr('Negative')} · {$tr('Unreported periods are unknown')}{:else if snapshot.statuses.get(habit.id)?.positive}{@const due = snapshot.statuses.get(habit.id)!.positive!}{$tr(due.due ? 'Due' : 'Next occurrence')}: <bdi>{date(due.due ? due.occurrence : due.next_occurrence, habit)}</bdi>{:else}{$tr('Positive')}{/if}</p>
      </div>
      {#if !habit.definition.archived}<button disabled={blocked} onclick={() => openForm('event', habit)}>{$tr(habit.definition.kind === 'positive' ? 'Complete' : 'Record lapse')}</button>{/if}
    </li>{/each}</ul>
    <footer><p class="hint">{$tr('Up to 30 habits per page. Filters apply to this page.')}</p><div class="actions"><button disabled={blocked} onclick={() => model.load()}>{$tr('First page / Refresh')}</button>{#if snapshot.more}<button disabled={blocked} onclick={() => model.load(true)}>{$tr('Next page')}</button>{/if}</div></footer>
  {/if}
</section>

<dialog bind:this={dialog} aria-labelledby="habit-form-heading" oncancel={e => { e.preventDefault(); cancel(); }} oncompositionstart={() => composing = true} oncompositionend={() => composing = false}>
  {#if form}
    <div class="dialog-header"><h2 id="habit-form-heading">{$tr(form === 'event' ? 'Record event' : editing ? 'Edit habit' : 'New habit')}</h2><button disabled={snapshot.busy || !!snapshot.pending || composing} onclick={cancel}>{$tr('Cancel')}</button></div>
    {#if snapshot.busy}<p role="status">{$tr('Saving…')}</p>{/if}
    {@render feedback()}
    {#if discard}<div class="uncertain discard-guard" role="alert"><p>{$tr('Discard this form? Unsaved input will be lost.')}</p><div class="actions"><button onclick={keepEditing}>{$tr('Keep editing')}</button><button onclick={closeForm}>{$tr('Discard form')}</button></div></div>{/if}
    {#if form === 'habit'}<HabitForm habit={editing} disabled={blocked || discard} onSave={save} />{:else if editing}<EventForm habit={editing} disabled={blocked || discard} onSave={save} />{/if}
  {/if}
</dialog>

<style>
  .habits { max-width: 860px; margin-inline: auto; padding: 24px clamp(12px, 3vw, 32px); } header, .dialog-header { display: flex; align-items: center; justify-content: space-between; gap: 16px; } h2 { font-size: 23px; margin: 0; letter-spacing: -.02em; } header p, .hint, footer { color: var(--muted); } header p { margin-block: 4px 20px; } h3 { font-size: 16px; } button { padding: 7px 12px; } .primary { color: var(--accent-ink); font-weight: 600; } ul { list-style: none; padding: 0; margin-block: 16px; } li { display: flex; align-items: center; justify-content: space-between; gap: 12px; border-block-end: 1px solid var(--line); padding-block: 16px; } .row-content { min-width: 0; } .name { border: 0; padding: 0; background: transparent; font-weight: 600; text-align: start; overflow-wrap: anywhere; } .hint { font-size: 12px; margin-block: 5px; } bdi { overflow-wrap: anywhere; } .actions, .filters { display: flex; flex-wrap: wrap; gap: 8px; } [aria-pressed='true'] { background: var(--selected); color: var(--accent-ink); } .empty { padding: 30px 16px; text-align: center; background: var(--sidebar); border-radius: var(--radius-panel); margin-block: 20px; } .empty p { color: var(--muted); } .uncertain { background: var(--warning-bg); border: 1px solid var(--warning); border-radius: var(--radius-control); padding: 12px; margin-block: 12px; } .voided { color: var(--muted); } dialog { width: 560px; padding: 24px; max-height: calc(100dvh - 32px); overflow-y: auto; } .dialog-header { margin-block-end: 20px; } .dialog-header h2 { font-size: 20px; } .detail { margin-block-start: 16px; } @media(max-width: 480px) { header { flex-wrap: wrap; } li { flex-wrap: wrap; } dialog { padding: 16px; } }
</style>
