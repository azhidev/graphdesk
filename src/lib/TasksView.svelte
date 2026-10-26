<script lang="ts">
  import { untrack } from 'svelte';
  import { api, type Page } from './api';
  import { TaskController, type TaskFilter, type TaskHit } from './tasks';
  import { preferences, tr } from './preferences';
  import { displayPageTitle } from './dates';
  import { statusText } from './i18n';
  import ErrorNotice from './ErrorNotice.svelte';
  let { session, pages, refreshKey, disabled, onOpen, status = $bindable('active'), page = $bindable(''), onFilter }: {
    session: string; pages: Page[]; refreshKey: string; disabled: boolean; onOpen: (hit: TaskHit) => void;
    status: TaskFilter; page: string; onFilter: () => void;
  } = $props();
  let revision = $state(0);
  const tasks = new TaskController(api.tasks, () => revision++);
  const view = $derived.by(() => { revision; return { hits: tasks.hits, pending: tasks.pending, error: tasks.error, more: tasks.more }; });
  $effect(() => {
    refreshKey;
    const token = session, filter = status, source = page || null;
    untrack(() => { void tasks.load(token, filter, source); });
  });
</script>

<section aria-label={$tr('Tasks')}>
  <h2 tabindex="-1" id="tasks-heading">{$tr('Tasks')}</h2>
  <p>{$tr('Tasks from this graph. Open a task to edit it on its original page.')}</p>
  <label>{$tr('Status')} <select bind:value={status} disabled={disabled} onchange={e => { status = e.currentTarget.value as TaskFilter; onFilter(); }}>
    <option value="active">{$tr('Active (TODO + DOING)')}</option><option value="todo">{$tr('TODO')}</option>
    <option value="doing">{$tr('DOING')}</option><option value="done">{$tr('DONE')}</option>
  </select></label>
  <label>{$tr('Page')} <select bind:value={page} disabled={disabled} onchange={e => { page = e.currentTarget.value; onFilter(); }}>
    <option value="">{$tr('All pages and journals')}</option>
    {#each pages as item (item.id)}<option value={item.id}>{displayPageTitle(item, $preferences.calendar, $preferences.language)}</option>{/each}
  </select></label>
  <button disabled={disabled || view.pending} onclick={() => tasks.load(session, status, page || null)}>{$tr('Refresh tasks')}</button>
  {#if view.pending}<p role="status">{$tr('Loading tasks…')}</p>{/if}
  {#if view.error}<ErrorNotice message={view.error} /><p>{$tr('Use Refresh tasks to retry.')}</p>{/if}
  {#if !view.pending && !view.error && !view.hits.length}<p>{$tr('No tasks match these filters. Set a block to TODO or DOING in a page or journal.')}</p>{/if}
  <ul>{#each view.hits as hit (hit.id)}
    <li><button disabled={disabled} onclick={() => onOpen(hit)} dir="auto">
      <strong>{statusText($preferences.language, hit.task_status.toUpperCase())} · <bdi>{displayPageTitle(pages.find(p => p.id === hit.page_id) ?? { title: hit.page_title }, $preferences.calendar, $preferences.language)}</bdi></strong><span dir="auto">{hit.content.slice(0, 220) || $tr('Empty task')}</span>
    </button></li>
  {/each}</ul>
  {#if view.more}<button disabled={disabled || view.pending} onclick={() => tasks.next()}>{$tr('Load more tasks')}</button>{/if}
</section>

<style>
  label { display: inline-block; margin: .25rem .5rem .5rem 0; max-width: 100%; }
  select, button { font: inherit; padding: .4rem; max-width: 100%; }
  ul { list-style: none; padding: 0; }
  li button { display: block; width: 100%; text-align: start; margin-block: .4rem; overflow-wrap: anywhere; }
  span { display: block; white-space: pre-wrap; }
</style>
