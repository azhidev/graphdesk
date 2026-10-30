<script lang="ts">
  import { untrack } from 'svelte';
  import { api, type Block, type Page } from './api';
  import { TaskController, type TaskFilter, type TaskHit } from './tasks';
  import { preferences, tr } from './preferences';
  import { displayPageTitle } from './dates';
  import { statusText } from './i18n';
  import ErrorNotice from './ErrorNotice.svelte';
  let { session, pages, refreshKey, disabled, onOpen, status = $bindable('active'), page = $bindable(''), onFilter }: {
    session: string; pages: Page[]; refreshKey: Block[]; disabled: boolean; onOpen: (hit: TaskHit) => void;
    status: TaskFilter; page: string; onFilter: () => void;
  } = $props();
  let revision = $state(0);
  const tasks = new TaskController(api.tasks, () => revision++);
  const view = $derived.by(() => { revision; return { hits: tasks.hits, pending: tasks.pending, error: tasks.error, more: tasks.more }; });
  $effect(() => {
    refreshKey; pages;
    const token = session, filter = status, source = page || null;
    untrack(() => { void tasks.load(token, filter, source); });
  });
</script>

<section class="tasks-workspace" aria-label={$tr('Tasks')}>
  <h2 tabindex="-1" id="tasks-heading">{$tr('Tasks')}</h2>
  <p class="description">{$tr('Tasks from this graph. Open a task to edit it on its original page.')}</p>
  <div class="task-filters">
  <label>{$tr('Status')} <select bind:value={status} disabled={disabled} onchange={e => { status = e.currentTarget.value as TaskFilter; onFilter(); }}>
    <option value="active">{$tr('Active (TODO + DOING)')}</option><option value="todo">{$tr('TODO')}</option>
    <option value="doing">{$tr('DOING')}</option><option value="done">{$tr('DONE')}</option>
  </select></label>
  <label>{$tr('Page')} <select bind:value={page} disabled={disabled} onchange={e => { page = e.currentTarget.value; onFilter(); }}>
    <option value="">{$tr('All pages and journals')}</option>
    {#each pages as item (item.id)}<option value={item.id}>{displayPageTitle(item, $preferences.calendar, $preferences.language)}</option>{/each}
  </select></label>
  <button class="refresh" disabled={disabled || view.pending} onclick={() => tasks.load(session, status, page || null)}>{$tr('Refresh tasks')}</button>
  </div>
  {#if view.pending}<p role="status">{$tr('Loading tasks…')}</p>{/if}
  {#if view.error}<ErrorNotice message={view.error} /><p>{$tr('Use Refresh tasks to retry.')}</p>{/if}
  {#if !view.pending && !view.error && !view.hits.length}<p class="empty-state">{$tr('No tasks match these filters. Set a block to TODO or DOING in a page or journal.')}</p>{/if}
  <ul class="task-list">{#each view.hits as hit (hit.id)}
    <li><button class="task-result" disabled={disabled} onclick={() => onOpen(hit)}>
      <span class="task-marker" aria-hidden="true"></span><span class="task-copy">
        <span class="task-content" dir="auto">{hit.content.slice(0, 220) || $tr('Empty task')}</span>
        <span class="task-source"><span class="task-status">{statusText($preferences.language, hit.task_status.toUpperCase())}</span><bdi dir="auto">{displayPageTitle(pages.find(p => p.id === hit.page_id) ?? { title: hit.page_title }, $preferences.calendar, $preferences.language)}</bdi></span>
      </span>
    </button></li>
  {/each}</ul>
  {#if view.more}<button class="load-more" disabled={disabled || view.pending} onclick={() => tasks.next()}>{$tr('Load more tasks')}</button>{/if}
</section>

<style>
  .tasks-workspace { max-width: 812px; margin-inline: auto; padding: 52px 52px 88px; }
  h2 { font-size: 28px; line-height: 1.25; letter-spacing: -.025em; font-weight: 650; margin: 0 0 12px; }
  .description { color: var(--muted); font-size: 13px; margin: 0; }
  .task-filters { display: flex; flex-wrap: wrap; align-items: end; gap: 12px; padding-block: 28px 22px; border-block-end: 1px solid var(--line); }
  label { display: grid; gap: 6px; min-width: 0; color: var(--muted); font-size: 12px; }
  select { width: 100%; min-width: 145px; max-width: 235px; min-height: 36px; padding: 6px 10px; color: var(--ink); font-size: 13px; }
  .refresh { min-height: 36px; padding: 6px 10px; font-size: 12px; color: var(--muted); }
  .task-list { list-style: none; margin: 16px 0 0; padding: 0; }
  .task-list li + li { border-block-start: 1px solid var(--line); }
  .task-result { display: flex; align-items: flex-start; gap: 13px; width: 100%; min-width: 0; padding: 13px 11px; text-align: start; border: 0; background: transparent; overflow-wrap: anywhere; }
  .task-result:hover:not(:disabled) { background: var(--hover); }
  .task-marker { width: 6px; height: 6px; flex: none; border-radius: 50%; background: var(--accent); margin: 8px 5px 0; }
  .task-copy { display: grid; flex: 1; min-width: 0; gap: 4px; }
  .task-content { color: var(--ink); font-size: 14px; font-weight: 540; line-height: 1.5; white-space: pre-wrap; }
  .task-source { display: flex; flex-wrap: wrap; gap: 5px 10px; color: var(--muted); font-size: 12px; }
  .task-source bdi { overflow-wrap: anywhere; }
  .task-status { color: var(--accent-ink); font-weight: 650; }
  .empty-state { color: var(--muted); font-size: 13px; padding: 24px 11px; }
  .load-more { margin-block-start: 16px; padding: 7px 12px; color: var(--muted); }
  @media (max-width: 760px) { .tasks-workspace { padding: 40px 32px 70px; } }
  @media (max-width: 540px) { .tasks-workspace { padding: 32px 20px 60px; } }
</style>
