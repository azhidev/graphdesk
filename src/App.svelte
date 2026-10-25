<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { isTauri } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { api, editorApi, recoveryApi, type Page } from './lib/api';
  import { EditorController, errorMessage } from './lib/editor/controller';
  import PageEditor from './lib/editor/PageEditor.svelte';
  import RecoveryPanel from './lib/editor/RecoveryPanel.svelte';
  import type { RecoveryDraft, Block } from './lib/editor/types';
  import { localDate, adjacentDay, gregorianPresentation } from './lib/dates';
  import TasksView from './lib/TasksView.svelte';
  import { SearchController, type SearchHit } from './lib/search';
  import type { InlineReference, Links } from './lib/references';

  let path = $state('');
  let pages = $state<Page[]>([]);
  let selected = $state<Page | null>(null);
  let session = $state<string | null>(null);
  let busy = $state(false);
  let operation = $state('');
  let activePath = $state('');
  let switching = $state(false);
  let view = $state<'notes' | 'tasks'>('notes');
  let previewDate = $state<string | null>(null);
  let firstNoteDialog: HTMLDialogElement;
  let firstNoteDate = $state<string | null>(null);
  let firstNote = $state('');
  let firstNoteBlock: string | null = null;
  let firstNoteComposing = $state(false);
  let confirmCancelNote = $state(false);
  let error = $state('');
  let title = $state('');
  let renameTitle = $state('');
  let journalDate = $state(localDate());
  let query = $state('');
  let searchComposing = $state(false);
  let resultIndex = $state(0);
  let searchRevision = $state(0);
  const search = new SearchController(api.search, () => { searchRevision += 1; });
  const results = $derived.by(() => { searchRevision; return { hits: search.hits, error: search.error, pending: search.pending }; });
  const acknowledgedKey = $derived.by(() => { revision; return JSON.stringify([pages, editor.blocks]); });
  $effect(() => {
    acknowledgedKey;
    const token = session, text = query, composing = searchComposing;
    untrack(() => {
      if (token && !composing) void search.query(token, text);
      else search.reset();
      resultIndex = 0;
    });
  });
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
  const editor = new EditorController(editorApi, () => { revision += 1; }, undefined, recoveryApi);
  let recoveryRecords = $state<RecoveryDraft[]>([]);
  let recoveryError = $state('');
  let inventory = $state(0);
  async function refreshRecovery() {
    if (!session) return;
    try { recoveryRecords = await recoveryApi.list(session); recoveryError = ''; }
    catch (e) { recoveryError = errorMessage(e); }
  }
  async function recover(draft: RecoveryDraft, block: Block, textRevision: number) {
    if (locked) throw new Error('Wait for the current operation');
    busy = true;
    try {
      await editor.flush();
      // Equality is already reconciled, requiring no duplicate graph write.
      await editorApi.apply(session!, draft.page_id, { kind: 'save_text', id: draft.block_id,
        expected: block.content, revision: textRevision, content: draft.content }, null);
      await recoveryApi.clear(session!, draft.block_id, draft.content);
      if (selected?.id === draft.page_id) await editor.load(draft.page_id);
    } finally { busy = false; }
  }
  const locked = $derived.by(() => { revision; return busy || editor.working; });

  async function run(action: () => Promise<void>, label = 'Loading…') {
    if (locked) return;
    busy = true; operation = label; error = '';
    try { await action(); } catch (e) { error = errorMessage(e); }
    finally { busy = false; operation = ''; }
  }
  async function open(create: boolean) {
    await run(async () => {
      await editor.flush();
      const result = await api.open(path, create);
      search.reset(); query = '';
      editor.reset(result.token); session = result.token; pages = result.pages; selected = null;
      activePath = path; switching = false; previewDate = null; view = 'notes';
      recoveryRecords = []; await refreshRecovery();
      await openJournal(localDate(), true);
    });
  }
  async function select(page: Page) {
    await run(async () => { await editor.load(page.id); rememberPage(page); await focusPage(); });
  }
  async function selectHit(hit: SearchHit) {
    const page = pages.find(p => p.id === hit.page_id);
    if (!page) { error = 'Matching page no longer exists; reopen the graph.'; return; }
    await run(async () => {
      await editor.load(page.id); rememberPage(page);
      if (hit.kind === 'block') editor.reveal(hit.id);
      else await focusPage();
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
      await editor.load(page.id); rememberPage(page); await focusPage();
    });
  }
  function rememberPage(page: Page) {
    pages = [...pages.filter(p => p.id !== page.id), page].sort((a, b) => a.title.localeCompare(b.title));
    selected = page; renameTitle = page.title;
    previewDate = null; view = 'notes';
    if (page.journal_date) journalDate = page.journal_date;
  }
  async function rename(event: SubmitEvent) {
    event.preventDefault();
    if (!session || !selected) return;
    await run(async () => { await editor.flush(); rememberPage(await api.renamePage(session!, selected!.id, renameTitle)); });
  }
  async function focusPage() { await tick(); document.getElementById('page-heading')?.focus(); }
  async function openJournal(date: string, create = false) {
    await editor.flush();
    let page: Page | null;
    try { page = create ? await api.journal(session!, date) : await api.findJournal(session!, date); }
    catch (e) {
      if (errorMessage(e).includes('UNIQUE constraint failed: pages.title_key')) {
        throw new Error(`Cannot create journal ${date}: an ordinary page already uses that title. Rename that page, then retry. No page was converted or overwritten.`);
      }
      throw e;
    }
    if (page) { await editor.load(page.id); rememberPage(page); }
    else { await editor.unload(); selected = null; previewDate = date; view = 'notes'; }
    journalDate = date; await focusPage();
  }
  async function journal(date: string) {
    if (!session) return;
    await run(() => openJournal(date), 'Opening journal…');
  }
  async function capture(date = localDate()) {
    await run(async () => {
      await openJournal(date, true);
      const empty = editor.blocks.find(b => !b.parent_id && !b.content && !b.task_status && !editor.blocks.some(child => child.parent_id === b.id));
      if (empty) editor.reveal(empty.id);
      else await editor.action({ kind: 'append' });
    }, 'Preparing capture…');
  }
  async function startFirstNote(date: string) {
    firstNoteDate = date; firstNote = ''; firstNoteBlock = null; confirmCancelNote = false; error = '';
    firstNoteDialog.showModal(); await tick(); document.getElementById('first-note')?.focus();
  }
  function cancelFirstNote() {
    if (firstNoteComposing || locked) return;
    if (firstNote && !confirmCancelNote) { confirmCancelNote = true; return; }
    firstNoteDialog.close(); firstNoteDate = null; firstNote = '';
  }
  async function saveFirstNote(event: SubmitEvent) {
    event.preventDefault();
    if (!firstNoteDate || !firstNote.trim() || firstNoteComposing) return;
    await run(async () => {
      if (firstNoteBlock) await editor.retry();
      else {
        await openJournal(firstNoteDate!, true);
        await editor.action({ kind: 'append' });
        firstNoteBlock = editor.activeId;
      }
      editor.setDraft(firstNoteBlock!, firstNote);
      await editor.flush();
      const id = firstNoteBlock!, end = firstNote.length;
      firstNoteDialog.close(); firstNoteDate = null; firstNote = '';
      await tick(); editor.activate(id, { start: end, end });
    }, 'Saving first note…');
  }
  async function showTasks() {
    await run(async () => { await editor.flush(); view = 'tasks'; await tick(); document.getElementById('tasks-heading')?.focus(); });
  }
  async function shortcut(event: KeyboardEvent) {
    if (document.querySelector('dialog[open]') || event.isComposing || event.keyCode === 229 || editor.composing || !session || locked || !(event.ctrlKey || event.metaKey)) return;
    if (event.key.toLowerCase() === 'k' && !event.shiftKey && !event.altKey) {
      event.preventDefault(); document.getElementById('search')?.focus();
    } else if (event.key.toLowerCase() === 'j' && event.shiftKey && !event.altKey) {
      event.preventDefault(); await capture();
    }
  }
  function searchKeys(event: KeyboardEvent) {
    if (event.isComposing || searchComposing || event.keyCode === 229) return;
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      resultIndex = Math.max(0, Math.min(results.hits.length - 1, resultIndex + (event.key === 'ArrowDown' ? 1 : -1)));
      document.getElementById(`search-hit-${resultIndex}`)?.scrollIntoView({ block: 'nearest' });
    } else if (event.key === 'Enter' && results.hits[resultIndex]) {
      event.preventDefault(); void selectHit(results.hits[resultIndex]);
    } else if (event.key === 'Escape') { query = ''; void focusPage(); }
  }
  async function addPage(event: SubmitEvent) {
    event.preventDefault();
    if (!session) return;
    await run(async () => {
      await editor.flush();
      const page = await api.createPage(session!, title);
      pages = [...pages, page]; title = '';
      await editor.load(page.id); rememberPage(page); await focusPage();
    });
  }
  onMount(() => {
    if (!isTauri()) return;
    void recoveryApi.inventory().then(count => inventory = count).catch(() => { recoveryError = 'Recovery inventory unavailable. Open the original graph and retry recovery inspection.'; });
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void getCurrentWindow().onCloseRequested(async event => {
      if (firstNoteDate) { event.preventDefault(); error = 'Finish or cancel the first-note form before closing. It is not autosaved before submission.'; return; }
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

<svelte:window onkeydown={shortcut} />

<dialog bind:this={firstNoteDialog} aria-labelledby="first-note-heading" oncancel={event => { event.preventDefault(); cancelFirstNote(); }}>
  <h2 id="first-note-heading">First note for {firstNoteDate ? gregorianPresentation.label(firstNoteDate) : ''}</h2>
  <p>The journal is created when you save this note. This form is not autosaved before submission.</p>
  <form onsubmit={saveFirstNote}>
    <label for="first-note">Note text</label>
    <textarea id="first-note" bind:value={firstNote} dir="auto" rows="5" readonly={locked}
      oncompositionstart={() => firstNoteComposing = true} oncompositionend={() => firstNoteComposing = false}></textarea>
    {#if error}<p role="alert">{error}</p>{/if}
    <button disabled={locked || firstNoteComposing || !firstNote.trim()}>Save first note</button>
    <button type="button" disabled={locked || firstNoteComposing} onclick={cancelFirstNote}>{confirmCancelNote ? 'Discard form text and cancel' : 'Cancel'}</button>
  </form>
</dialog>

<main>
  <header><h1>Graphdesk</h1>
    {#if session}<p class="graph-path">Graph: {activePath}</p><button disabled={locked} onclick={() => switching = !switching}>{switching ? 'Cancel graph switch' : 'Switch graph'}</button>
    {:else}<p>Open a local graph to begin your daily workspace.</p>{/if}
  </header>
  {#if !session || switching}
  <section aria-label="Graph selection">
    <label for="path">Database path (on this computer)</label>
    <input id="path" bind:value={path} placeholder="Full path to notes.sqlite3" disabled={locked} />
    <button disabled={locked || !path.trim()} onclick={() => open(false)}>Open graph</button>
    <button disabled={locked || !path.trim()} onclick={() => open(true)}>Create graph</button>
    <p class="hint">The parent directory must exist. Create never overwrites an existing file.</p>
  </section>
  {/if}
  {#if operation}<p role="status">{operation}</p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}
  {#if recoveryError}<p role="alert">{recoveryError}</p>{/if}
  {#if !session && inventory}<p role="status">Recovery drafts found for {inventory} graph(s). Open the original graph to compare and recover text.</p>{/if}
  {#if session}
    {#key session}<RecoveryPanel {session} records={recoveryRecords} disabled={locked} onChanged={refreshRecovery} onRecover={recover} />{/key}
  {/if}
  {#if session}
    <div class="workspace">
      <aside>
        <nav aria-label="Workspace">
          <button disabled={locked} onclick={() => journal(localDate())}>Today</button>
          <button disabled={locked} onclick={showTasks} aria-current={view === 'tasks' ? 'page' : undefined}>Tasks</button>
          <button disabled={locked} onclick={() => capture()} title="Ctrl/Cmd+Shift+J">Capture today</button>
        </nav>
        <label for="search">Search pages and blocks</label>
        <input id="search" type="search" bind:value={query} placeholder="Search · Ctrl/Cmd+K" onkeydown={searchKeys}
          oncompositionstart={() => searchComposing = true} oncompositionend={() => searchComposing = false}
          aria-describedby="search-help" />
        <p id="search-help" class="hint">Phrase search · ↑/↓ choose · Enter opens · Esc clears</p>
        {#if results.pending}<p role="status">Searching…</p>{/if}
        {#if results.error}<p role="alert">{results.error}</p>{/if}
        {#if query.trim() && !results.pending && !results.error && !results.hits.length}<p>No matches.</p>{/if}
        <ul aria-label="Search results" class="results">{#each results.hits as hit, index (`${hit.kind}:${hit.id}`)}
          <li><button id={`search-hit-${index}`} class:chosen={index === resultIndex} aria-current={index === resultIndex ? 'true' : undefined} disabled={locked} onclick={() => selectHit(hit)} dir="auto">
            <strong>{hit.kind === 'page' ? 'Page' : `Block · ${pages.find(p => p.id === hit.page_id)?.title ?? ''}`}</strong>
            <span>{hit.text.slice(0, 180)}</span>
          </button></li>
        {/each}</ul>
        {#if results.hits.filter(h => h.kind === 'page').length === 100 || results.hits.filter(h => h.kind === 'block').length === 100}<p class="hint">Up to 100 results per kind. Refine your phrase to find more.</p>{/if}
        <details><summary>Journal date</summary>
          <label for="journal-date">Gregorian date</label>
          <input id="journal-date" type="date" min="0001-01-01" max="9999-12-31" bind:value={journalDate} disabled={locked} />
          <button disabled={locked || !journalDate} onclick={() => journal(journalDate)}>Open journal</button>
        </details>
        <h2>Pages</h2>
        <details><summary>New page</summary>
        <form onsubmit={addPage}>
          <label for="title">New page title</label>
          <input id="title" bind:value={title} disabled={locked} />
          <button disabled={locked || !title.trim()}>Create page</button>
        </form>
        </details>
        <ul class="page-list">{#each pages as page (page.id)}<li><button disabled={locked} aria-current={view === 'notes' && selected?.id === page.id ? 'page' : undefined} onclick={() => select(page)} dir="auto">{page.title}</button></li>{/each}</ul>
      </aside>
      <div class="content">
      {#if view === 'tasks' && session}
        <TasksView {session} {pages} refreshKey={acknowledgedKey} disabled={locked}
          onOpen={hit => selectHit({ id: hit.id, page_id: hit.page_id, kind: 'block', text: hit.content })} />
      {/if}
      <div hidden={view !== 'notes'}>
      <section aria-label="Page content">
        {#if selected || previewDate}
          <h2 id="page-heading" tabindex="-1" dir="auto">{selected?.title ?? gregorianPresentation.label(previewDate!)}</h2>
          {#if selected?.journal_date || previewDate}
            {@const day = selected?.journal_date ?? previewDate!}
            <nav aria-label="Journal navigation">
              <button disabled={locked || day === '0000-01-01'} onclick={() => journal(adjacentDay(day, -1))}>Previous day</button>
              <button disabled={locked} onclick={() => journal(localDate())}>Today</button>
              <button disabled={locked || day === '9999-12-31'} onclick={() => journal(adjacentDay(day, 1))}>Next day</button>
              <span>{gregorianPresentation.label(day)}</span>
            </nav>
          {/if}
        {/if}
        {#if selected}
          <details><summary>Rename page</summary>
          <form onsubmit={rename}>
            <label for="rename-title">Page title</label>
            <input id="rename-title" bind:value={renameTitle} disabled={locked} />
            <button disabled={locked || !renameTitle.trim()}>Rename page</button>
          </form>
          </details>
          <PageEditor {editor} {revision} {pages} references={links.references} onReference={followReference} disabled={busy} />
          <section aria-label="Incoming backlinks">
            <h3>Backlinks</h3>
            {#if linksError}<p role="alert">{linksError}</p>{/if}
            {#if !links.backlinks.length}<p>No incoming links.</p>{/if}
            <ul>{#each links.backlinks as block (block.id)}
              <li><button disabled={locked} onclick={() => selectHit({ id: block.id, page_id: block.page_id, kind: 'block', text: block.content })}>{pages.find(p => p.id === block.page_id)?.title}: {block.content}</button></li>
            {/each}</ul>
          </section>
        {:else if previewDate}
          <p>No notes for this day yet. Browsing dates does not create a journal.</p>
          <button disabled={locked} onclick={() => startFirstNote(previewDate!)}>Add a note</button>
        {:else}<p>Select a page, open a journal, or capture a note for today.</p>{/if}
      </section>
      </div>
      </div>
    </div>
  {/if}
  <footer>Plain-text block editor · Undo and collapse state are session-local. Acknowledged saves survive restart.</footer>
  {#if session}<button id="review-recovery" disabled={locked} onclick={refreshRecovery}>Review local recovery drafts</button>{/if}
</main>

<style>
  :global(body) { margin: 0; background: #f6f7f4; color: #202b27; font: 16px/1.5 system-ui, sans-serif; }
  main { max-width: 1200px; margin: auto; padding: 1rem; }
  header { display: flex; align-items: center; flex-wrap: wrap; gap: .6rem; }
  header h1 { font-size: 1.4rem; }
  .graph-path { flex: 1; overflow-wrap: anywhere; min-width: 0; font-size: .85rem; }
  .content { min-width: 0; }
  h1, h2 { line-height: 1.2; }
  section, aside { min-width: 0; padding: 1rem; border: 1px solid #c6d1ca; border-radius: .5rem; background: white; }
  label { display: block; font-weight: 600; }
  input { box-sizing: border-box; width: 100%; padding: .6rem; margin: .3rem 0 .6rem; font: inherit; }
  button { padding: .5rem .8rem; margin: .2rem; cursor: pointer; font: inherit; }
  button[aria-current='page'] { font-weight: bold; }
  ul { padding: 0; list-style: none; }
  aside button { max-width: 100%; overflow-wrap: anywhere; text-align: start; }
  .page-list, .results { max-height: 34vh; overflow-y: auto; }
  .page-list button, .results button { width: calc(100% - .4rem); }
  .results span { display: block; font-size: .85rem; white-space: pre-wrap; }
  .chosen { border-color: #275d50; background: #eaf1ed; }
  details { margin-block: .7rem; }
  dialog { max-width: min(36rem, 85vw); border: 1px solid #71958b; border-radius: .5rem; }
  dialog textarea { box-sizing: border-box; width: 100%; font: inherit; }
  summary { cursor: pointer; }
  :global(:focus-visible) { outline: 2px solid #275d50; outline-offset: 2px; }
  .workspace { display: grid; grid-template-columns: 240px minmax(0, 1fr); gap: 1rem; margin-top: 1rem; }
  .hint, footer { font-size: .85rem; color: #4b5d52; }
  footer { margin-top: 2rem; }
  [role='alert'] { color: #a01818; white-space: pre-wrap; }
  @media (max-width: 700px) { .workspace { grid-template-columns: 1fr; } }
</style>
