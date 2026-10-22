<script lang="ts">
  import { onMount } from 'svelte';
  import { isTauri } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { api, editorApi, type Page } from './lib/api';
  import { EditorController, errorMessage } from './lib/editor/controller';
  import PageEditor from './lib/editor/PageEditor.svelte';
  import { localDate } from './lib/dates';
  import { SearchController, type SearchHit } from './lib/search';
  import type { InlineReference, Links } from './lib/references';

  let path = $state('');
  let pages = $state<Page[]>([]);
  let selected = $state<Page | null>(null);
  let session = $state<string | null>(null);
  let busy = $state(false);
  let error = $state('');
  let title = $state('');
  let renameTitle = $state('');
  let journalDate = $state(localDate());
  let query = $state('');
  let searchRevision = $state(0);
  const search = new SearchController(api.search, () => { searchRevision += 1; });
  const results = $derived.by(() => { searchRevision; return { hits: search.hits, error: search.error, pending: search.pending }; });
  let links = $state<Links>({ references: [], backlinks: [] });
  let linksError = $state('');
  const linksKey = $derived.by(() => { revision; return JSON.stringify([session, selected?.id, pages, editor.blocks]); });
  $effect(() => {
    const key = linksKey;
    const token = session, page = selected?.id;
    let cancelled = false;
    links = { references: [], backlinks: [] }; linksError = '';
    if (token && page) void api.links(token, page).then(value => {
      if (!cancelled && key === linksKey) links = value;
    }).catch(e => { if (!cancelled && key === linksKey) linksError = errorMessage(e); });
    return () => { cancelled = true; };
  });
  let revision = $state(0);
  const editor = new EditorController(editorApi, () => { revision += 1; });
  const locked = $derived.by(() => { revision; return busy || editor.working; });

  async function run(action: () => Promise<void>) {
    if (locked) return;
    busy = true; error = '';
    try { await action(); } catch (e) { error = errorMessage(e); }
    finally { busy = false; }
  }
  async function open(create: boolean) {
    await run(async () => {
      await editor.flush();
      const result = await api.open(path, create);
      search.reset(); query = '';
      editor.reset(result.token); session = result.token; pages = result.pages; selected = null;
    });
  }
  async function select(page: Page) {
    await run(async () => { await editor.load(page.id); selected = page; renameTitle = page.title; });
  }
  async function selectHit(hit: SearchHit) {
    const page = pages.find(p => p.id === hit.page_id);
    if (!page) { error = 'Matching page no longer exists; reopen the graph.'; return; }
    await run(async () => {
      await editor.load(page.id); selected = page; renameTitle = page.title;
      if (hit.kind === 'block') editor.reveal(hit.id);
    });
  }
  async function followReference(reference: InlineReference) {
    const page = pages.find(p => p.id === reference.target_page_id);
    if (page) { await select(page); return; }
    if (reference.target_page_id) { error = 'Referenced page no longer exists; reopen the graph.'; return; }
    await run(async () => {
      await editor.flush();
      const page = await api.createPage(session!, reference.label);
      pages = [...pages.filter(p => p.id !== page.id), page];
      await editor.load(page.id); rememberPage(page);
    });
  }
  function rememberPage(page: Page) {
    pages = [...pages.filter(p => p.id !== page.id), page].sort((a, b) => a.title.localeCompare(b.title));
    selected = page; renameTitle = page.title;
  }
  async function rename(event: SubmitEvent) {
    event.preventDefault();
    if (!session || !selected) return;
    await run(async () => { await editor.flush(); rememberPage(await api.renamePage(session!, selected!.id, renameTitle)); });
  }
  async function journal(date: string) {
    if (!session) return;
    await run(async () => {
      await editor.flush(); const page = await api.journal(session!, date);
      await editor.load(page.id); rememberPage(page);
    });
  }
  async function addPage(event: SubmitEvent) {
    event.preventDefault();
    if (!session) return;
    await run(async () => {
      await editor.flush();
      const page = await api.createPage(session!, title);
      pages = [...pages, page]; title = '';
      await editor.load(page.id); rememberPage(page);
    });
  }
  onMount(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void getCurrentWindow().onCloseRequested(async event => {
      if (locked) { event.preventDefault(); error = 'Wait for the current operation before closing.'; return; }
      busy = true;
      try { await editor.flush(); }
      catch (e) { event.preventDefault(); error = `Not closed: ${errorMessage(e)}`; }
      finally { busy = false; }
    }).then(stop => { if (disposed) stop(); else unlisten = stop; })
      .catch(e => { error = `Close protection unavailable: ${errorMessage(e)}`; });
    return () => { disposed = true; unlisten?.(); };
  });
</script>

<main>
  <header><h1>Graphdesk</h1><p>Local knowledge, one independent database per graph.</p></header>
  <section aria-label="Graph selection">
    <label for="path">Database path (on this computer)</label>
    <input id="path" bind:value={path} placeholder="/path/to/notes.sqlite3" disabled={locked} />
    <button disabled={locked || !path.trim()} onclick={() => open(false)}>Open graph</button>
    <button disabled={locked || !path.trim()} onclick={() => open(true)}>Create graph</button>
    <p class="hint">The parent directory must exist. Create never overwrites an existing file.</p>
  </section>
  {#if error}<p role="alert">{error}</p>{/if}
  {#if session}
    <div class="workspace">
      <aside>
        <label for="search">Search pages and blocks</label>
        <input id="search" bind:value={query} oninput={() => { if (session) void search.query(session, query); }} />
        {#if results.pending}<p role="status">Searching…</p>{/if}
        {#if results.error}<p role="alert">{results.error}</p>{/if}
        {#if query.trim() && !results.pending && !results.error && !results.hits.length}<p>No matches.</p>{/if}
        <ul aria-label="Search results">{#each results.hits as hit (`${hit.kind}:${hit.id}`)}
          <li><button disabled={locked} onclick={() => selectHit(hit)}>{hit.kind}: {hit.text}</button></li>
        {/each}</ul>
        <h2>Pages</h2>
        <button disabled={locked} onclick={() => journal(localDate())}>Today's journal</button>
        <label for="journal-date">Journal date</label>
        <input id="journal-date" type="date" bind:value={journalDate} disabled={locked} />
        <button disabled={locked || !journalDate} onclick={() => journal(journalDate)}>Open journal</button>
        <form onsubmit={addPage}>
          <label for="title">New page title</label>
          <input id="title" bind:value={title} disabled={locked} />
          <button disabled={locked || !title.trim()}>Create page</button>
        </form>
        <ul>{#each pages as page (page.id)}<li><button disabled={locked} aria-current={selected?.id === page.id ? 'page' : undefined} onclick={() => select(page)}>{page.title}</button></li>{/each}</ul>
      </aside>
      <section aria-label="Page content">
        {#if selected}<h2>{selected.title}</h2>
          <form onsubmit={rename}>
            <label for="rename-title">Page title</label>
            <input id="rename-title" bind:value={renameTitle} disabled={locked} />
            <button disabled={locked || !renameTitle.trim()}>Rename page</button>
          </form>
          <PageEditor {editor} {revision} {pages} references={links.references} onReference={followReference} disabled={busy} />
          <section aria-label="Incoming backlinks">
            <h3>Backlinks</h3>
            {#if linksError}<p role="alert">{linksError}</p>{/if}
            {#if !links.backlinks.length}<p>No incoming links.</p>{/if}
            <ul>{#each links.backlinks as block (block.id)}
              <li><button disabled={locked} onclick={() => selectHit({ id: block.id, page_id: block.page_id, kind: 'block', text: block.content })}>{pages.find(p => p.id === block.page_id)?.title}: {block.content}</button></li>
            {/each}</ul>
          </section>
        {:else}<p>Select or create a page.</p>{/if}
      </section>
    </div>
  {/if}
  <footer>Plain-text block editor · Undo and collapse state are session-local. Acknowledged saves survive restart.</footer>
</main>

<style>
  :global(body) { margin: 0; background: #f6f7f4; color: #202b27; font: 16px/1.5 system-ui, sans-serif; }
  main { max-width: 1200px; margin: auto; padding: 1.5rem; }
  h1, h2 { line-height: 1.2; }
  section, aside { min-width: 0; padding: 1rem; border: 1px solid #c6d1ca; border-radius: .5rem; background: white; }
  label { display: block; font-weight: 600; }
  input { box-sizing: border-box; width: 100%; padding: .6rem; margin: .3rem 0 .6rem; font: inherit; }
  button { padding: .5rem .8rem; margin: .2rem; cursor: pointer; font: inherit; }
  button[aria-current='page'] { font-weight: bold; }
  .workspace { display: grid; grid-template-columns: 240px minmax(0, 1fr); gap: 1rem; margin-top: 1rem; }
  .hint, footer { font-size: .85rem; color: #4b5d52; }
  footer { margin-top: 2rem; }
  [role='alert'] { color: #a01818; white-space: pre-wrap; }
  @media (max-width: 700px) { .workspace { grid-template-columns: 1fr; } }
</style>
