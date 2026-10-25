<script lang="ts">
  import { untrack } from 'svelte';
  import { api, type Page } from './api';
  import { TaskController, type TaskFilter, type TaskHit } from './tasks';
  let { session, pages, refreshKey, disabled, onOpen }: {
    session: string; pages: Page[]; refreshKey: string; disabled: boolean; onOpen: (hit: TaskHit) => void;
  } = $props();
  let status = $state<TaskFilter>('active');
  let page = $state('');
  let revision = $state(0);
  const tasks = new TaskController(api.tasks, () => revision++);
  const view = $derived.by(() => { revision; return { hits: tasks.hits, pending: tasks.pending, error: tasks.error, more: tasks.more }; });
  $effect(() => {
    refreshKey;
    const token = session, filter = status, source = page || null;
    untrack(() => { void tasks.load(token, filter, source); });
  });
</script>

<section aria-label="Tasks">
  <h2 tabindex="-1" id="tasks-heading">Tasks</h2>
  <p>Tasks from this graph. Open a task to edit it on its original page.</p>
  <label>Status <select bind:value={status} disabled={disabled}>
    <option value="active">Active (TODO + DOING)</option><option value="todo">TODO</option>
    <option value="doing">DOING</option><option value="done">DONE</option>
  </select></label>
  <label>Page <select bind:value={page} disabled={disabled}>
    <option value="">All pages and journals</option>
    {#each pages as item (item.id)}<option value={item.id}>{item.title}</option>{/each}
  </select></label>
  <button disabled={disabled || view.pending} onclick={() => tasks.load(session, status, page || null)}>Refresh tasks</button>
  {#if view.pending}<p role="status">Loading tasks…</p>{/if}
  {#if view.error}<p role="alert">{view.error} Use Refresh tasks to retry.</p>{/if}
  {#if !view.pending && !view.error && !view.hits.length}<p>No tasks match these filters. Set a block to TODO or DOING in a page or journal.</p>{/if}
  <ul>{#each view.hits as hit (hit.id)}
    <li><button disabled={disabled} onclick={() => onOpen(hit)} dir="auto">
      <strong>{hit.task_status.toUpperCase()} · {hit.page_title}</strong><span>{hit.content.slice(0, 220) || 'Empty task'}</span>
    </button></li>
  {/each}</ul>
  {#if view.more}<button disabled={disabled || view.pending} onclick={() => tasks.next()}>Load more tasks</button>{/if}
</section>

<style>
  label { display: inline-block; margin: .25rem .5rem .5rem 0; max-width: 100%; }
  select, button { font: inherit; padding: .4rem; max-width: 100%; }
  ul { list-style: none; padding: 0; }
  li button { display: block; width: 100%; text-align: start; margin-block: .4rem; overflow-wrap: anywhere; }
  span { display: block; white-space: pre-wrap; }
  [role='alert'] { color: #a01818; }
</style>
