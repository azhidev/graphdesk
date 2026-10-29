<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { isTauri } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { api, editorApi, recoveryApi, noteApi, preferencesApi, type Page } from './lib/api';
  import { EditorController, errorMessage } from './lib/editor/controller';
  import PageEditor from './lib/editor/PageEditor.svelte';
  import RecoveryPanel from './lib/editor/RecoveryPanel.svelte';
  import type { RecoveryDraft, Block } from './lib/editor/types';
  import { localDate, adjacentDay, dateLabel, calendarFields, displayPageTitle } from './lib/dates';
  import CalendarPicker from './lib/CalendarPicker.svelte';
  import { preferences, tr } from './lib/preferences';
  import { get } from 'svelte/store';
  import ErrorNotice from './lib/ErrorNotice.svelte';
  import { FirstNoteController, type PendingNote, type NoteState } from './lib/firstNote';
  import { appShortcut } from './lib/appShortcuts';
  import { NavigationHistory, type Destination } from './lib/navigation';
  import { graphApi, type GraphRegistry, type RecentGraph } from './lib/graphs';
  import { GraphSwitch } from './lib/graphSwitch';
  import GraphChooser from './lib/GraphChooser.svelte';
  import PageSwitcher from './lib/PageSwitcher.svelte';
  import type { TaskFilter } from './lib/tasks';
  import type { MessageKey } from './lib/i18n';
  import TasksView from './lib/TasksView.svelte';
  import { SearchController, type SearchHit } from './lib/search';
  import type { InlineReference, Links } from './lib/references';

  let path = $state('');
  let pages = $state<Page[]>([]);
  let selected = $state<Page | null>(null);
  let session = $state<string | null>(null);
  let busy = $state(false);
  let operation = $state<MessageKey | ''>('');
  let activePath = $state('');
  let switching = $state(false);
  let handoff = $state(false), landingError = $state(false);
  let registry = $state<GraphRegistry>({ remember: false, reopen: false, entries: [], warning: false });
  let registryReady = $state(false), registryError = $state(''), graphComposing = $state(false);
  let quickPages: { show: () => Promise<void> };
  const graphSwitch = new GraphSwitch(graphApi);
  const navigation = new NavigationHistory();
  let navRevision = $state(0);
  const history = $derived.by(() => { navRevision; return { back: navigation.canBack, forward: navigation.canForward }; });
  let taskStatus = $state<TaskFilter>('active'), taskPage = $state('');
  let view = $state<'notes' | 'tasks'>('notes');
  let previewDate = $state<string | null>(null);
  let firstNoteDialog: HTMLDialogElement;
  let firstNoteReturn: HTMLElement | null = null;
  let firstNoteDate = $state<string | null>(null);
  let firstNote: FirstNoteController | null = null;
  let noteRevision = $state(0);
  const noteView = $derived.by(() => { noteRevision; return { content: firstNote?.content ?? '', composing: firstNote?.composing ?? false, protected: firstNote?.protected ?? false, error: firstNote?.error ?? '' }; });
  let pendingNotes = $state<PendingNote[]>([]);
  let noteComparison = $state<NoteState | null>(null);
  let prefsReady = $state(false), prefsSaving = $state(false), prefsError = $state('');
  let confirmCancelNote = $state(false);
  let error = $state('');
  let title = $state('');
  let renameTitle = $state('');
  let journalDate = $state(localDate());
  let query = $state('');
  let searchComposing = $state(false);
  let resultIndex = $state(0);
  let searchRevision = $state(0);
  const search = new SearchController((session, text) => api.search(session, text, get(preferences).search_mode === 'persian'), () => { searchRevision += 1; });
  const results = $derived.by(() => { searchRevision; return { hits: search.hits, error: search.error, pending: search.pending }; });
  // The controller replaces this array on acknowledged changes, never for a draft.
  // Keep search, tasks and links fresh without serializing every block per keystroke.
  const acknowledgedBlocks = $derived.by(() => { revision; return editor.blocks; });
  $effect(() => {
    acknowledgedBlocks; pages;
    $preferences.search_mode;
    const token = session, text = query, composing = searchComposing;
    untrack(() => {
      if (token && !composing) void search.query(token, text);
      else search.reset();
      resultIndex = 0;
    });
  });
  $effect(() => { document.documentElement.lang = $preferences.language; document.documentElement.dir = $preferences.language === 'fa' ? 'rtl' : 'ltr'; });
  function presentDate(day: string) {
    try { return dateLabel(day, $preferences.calendar, $preferences.language); } catch { return day; }
  }
  function unsupportedDate(day: string) { try { calendarFields(day, $preferences.calendar); return false; } catch { return true; } }
  function pageLabel(page: Page) { return displayPageTitle(page, $preferences.calendar, $preferences.language); }
  function contextLabel(id: string) { const page = pages.find(p => p.id === id); return page ? pageLabel(page) : ''; }
  async function changePreference(key: keyof typeof $preferences, select: HTMLSelectElement) {
    if (locked || prefsSaving || editor.composing || searchComposing) { select.value = get(preferences)[key]; return; }
    const next = { ...$preferences, [key]: select.value };
    const restoreFocus = document.activeElement === select;
    prefsSaving = true;
    try { await preferencesApi.save(next); preferences.set(next); prefsError = ''; }
    catch { prefsError = 'Preferences could not be saved. Your previous settings remain active.'; }
    finally {
      prefsSaving = false; await tick(); select.value = get(preferences)[key];
      if (restoreFocus && (document.activeElement === document.body || document.activeElement === select)) select.focus();
    }
  }
  let links = $state<Links>({ references: [], backlinks: [] });
  let linksError = $state('');
  $effect(() => {
    acknowledgedBlocks; pages;
    const token = session, page = selected?.id;
    let cancelled = false;
    links = { references: [], backlinks: [] }; linksError = '';
    if (token && page) void api.links(token, page).then(value => {
      if (!cancelled) links = value;
    }).catch(e => { if (!cancelled) linksError = errorMessage(e); });
    return () => { cancelled = true; };
  });
  let revision = $state(0);
  let rowsRevision = $state(0);
  const editor = new EditorController(editorApi, scope => {
    revision += 1; // Toolbar, recovery and navigation state update for every draft.
    if (scope !== 'draft') rowsRevision += 1; // Row props update only for acknowledged/structural changes.
  }, undefined, recoveryApi);
  let recoveryRecords = $state<RecoveryDraft[]>([]);
  let recoveryError = $state('');
  let inventory = $state(0);
  async function refreshRecovery() {
    if (!session) return;
    const token = session;
    try { const [blocks, notes] = await Promise.all([recoveryApi.list(token), noteApi.list(token)]); if (token !== session) return; recoveryRecords = blocks; pendingNotes = notes; recoveryError = ''; }
    catch (e) { if (token === session) recoveryError = errorMessage(e); }
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
  const composing = $derived.by(() => { revision; return editor.composing; });
  async function goPages() {
    if (locked) return;
    try { departureAllowed(); await quickPages.show(); } catch (e) { error = errorMessage(e); }
  }

  async function run(action: () => Promise<void>, label: MessageKey = 'Loading…') {
    if (locked) return;
    busy = true; operation = label; error = '';
    try { await action(); } catch (e) { error = errorMessage(e); }
    finally { busy = false; operation = ''; navRevision++; }
  }
  function departureAllowed() {
    if (firstNoteDate) throw new Error('Finish or cancel the first-note form before switching graphs.');
    if (editor.composing || searchComposing || graphComposing) throw new Error('Finish text composition before leaving the editor');
  }
  async function performSwitch(candidate: string, create: boolean, expectedGraph: string | null = null, automatic = false) {
    departureAllowed();
    const source = session, stamp = editor.departureStamp();
    let installed = false;
    try {
      await graphSwitch.open({ path: candidate, create, sourceSession: source, expectedGraph, automatic }, source, {
        flush: () => editor.flush(),
        check: () => { departureAllowed(); if (session !== source) throw new Error('The graph operation changed. Try again.'); editor.assertDeparture(stamp); },
        // Remove native inputs before the final activation IPC. Late finalized
        // input during preparation is checked above, never erased by reset.
        detach: async () => { handoff = true; await tick(); },
        install: result => {
          editor.reset(result.token); session = result.token; pages = result.pages; selected = null;
          activePath = result.path; path = result.path; switching = false; previewDate = null; view = 'notes';
          journalDate = localDate();
          taskStatus = 'active'; taskPage = ''; title = ''; renameTitle = '';
          search.reset(); query = ''; searchComposing = false;
          recoveryRecords = []; pendingNotes = []; recoveryError = ''; landingError = false;
          navigation.reset(result.token); navRevision++; installed = true;
        },
      });
    } finally { handoff = false; }
    if (!installed) return;
    try { registry = await graphApi.record(session!); registryError = registry.warning ? 'Graph opened, but could not be remembered.' : ''; }
    catch { registryError = 'Graph opened, but could not be remembered.'; }
    await refreshRecovery();
    await landToday();
  }
  async function landToday() {
    try { await navigation.visit({ kind: 'journal', date: localDate() }, d => openJournal(d.kind === 'journal' ? d.date : localDate(), true)); landingError = false; }
    catch (e) { landingError = true; throw e; }
    finally { navRevision++; }
  }
  async function open(create: boolean) { await run(() => performSwitch(path, create), 'Opening graph…'); }
  async function recent(graph: RecentGraph, automatic = false) { await run(() => performSwitch(graph.path, false, graph.graph_id, automatic), 'Opening graph…'); }
  async function pick(create: boolean) {
    const origin = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    await run(async () => {
      departureAllowed();
      const source = session;
      const candidate = await graphApi.pick(create, $preferences.language);
      if (source !== session) throw new Error('The graph operation changed. Try again.');
      if (candidate) await performSwitch(candidate, create);
    }, 'Choosing graph…');
    await tick(); if (origin?.isConnected) origin.focus();
  }
  async function registrySettings(remember: boolean, reopen: boolean) {
    const origin = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    await run(async () => {
      try {
        registry = await graphApi.settings(remember, reopen); registryError = '';
        if (remember && session) registry = await graphApi.record(session);
      } catch { registryError = 'Graph registry changes could not be saved.'; }
    });
    await tick(); if (origin?.isConnected) origin.focus();
  }
  async function forget(graphPath: string | null) {
    await run(async () => { try { registry = await graphApi.forget(graphPath); registryError = ''; } catch { registryError = 'Graph registry changes could not be saved.'; } });
  }
  function destination(page: Page, block?: string): Destination { return page.journal_date ? { kind: 'journal', date: page.journal_date, block } : { kind: 'page', id: page.id, block }; }
  function rememberLocation() {
    if (view === 'tasks') navigation.replace({ kind: 'tasks', status: taskStatus, page: taskPage });
    else if (selected) navigation.replace(destination(selected, editor.activeId ?? undefined));
    navRevision++;
  }
  async function loadDestination(target: Destination) {
    if (target.kind === 'tasks') {
      const stamp = editor.departureStamp(); await editor.flush(); editor.assertDeparture(stamp);
      taskStatus = target.status; taskPage = pages.some(p => p.id === target.page) ? target.page : '';
      view = 'tasks'; await tick(); document.getElementById('tasks-heading')?.focus(); return;
    }
    if (target.kind === 'journal') await openJournal(target.date);
    else {
      const page = pages.find(p => p.id === target.id);
      if (!page) throw new Error('This destination is no longer available. Choose another page.');
      await editor.load(page.id); rememberPage(page);
    }
    if (target.block && editor.blocks.some(b => b.id === target.block)) editor.reveal(target.block);
    else await focusPage();
  }
  async function navigate(target: Destination) {
    departureAllowed(); rememberLocation(); await navigation.visit(target, loadDestination); navRevision++;
  }
  async function moveHistory(delta: -1 | 1) {
    await run(async () => { departureAllowed(); rememberLocation(); await navigation.move(delta, loadDestination); });
  }
  async function select(page: Page) {
    await run(() => navigate(destination(page)));
  }
  async function selectHit(hit: SearchHit) {
    const page = pages.find(p => p.id === hit.page_id);
    if (!page) { error = 'Matching page no longer exists; reopen the graph.'; return; }
    await run(async () => {
      await navigate(destination(page, hit.kind === 'block' ? hit.id : undefined));
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
      await navigate(destination(page));
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
        throw new Error('A page already uses this date title. Rename that page before creating this journal.');
      }
      throw e;
    }
    if (page) { await editor.load(page.id); rememberPage(page); }
    else { await editor.unload(); selected = null; previewDate = date; view = 'notes'; }
    journalDate = date; await focusPage();
  }
  async function journal(date: string) {
    if (!session) return;
    await run(() => navigate({ kind: 'journal', date }), 'Opening journal…');
  }
  async function capture(date = localDate()) {
    await run(async () => {
      departureAllowed(); rememberLocation();
      await navigation.visit({ kind: 'journal', date }, () => openJournal(date, true));
      const empty = editor.blocks.find(b => !b.parent_id && !b.content && !b.task_status && !editor.blocks.some(child => child.parent_id === b.id));
      if (empty) editor.reveal(empty.id);
      else await editor.action({ kind: 'append' });
    }, 'Preparing capture…');
  }
  async function startFirstNote(date: string, record?: PendingNote) {
    if (locked || editor.composing) return;
    firstNoteReturn = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    firstNote = new FirstNoteController(session!, date, noteApi, () => noteRevision++, record);
    firstNoteDate = date; noteRevision++; confirmCancelNote = false; error = ''; noteComparison = null;
    firstNoteDialog.showModal(); await tick(); document.getElementById('first-note')?.focus();
    if (record) {
      const instance = firstNote, token = session!;
      try { const comparison = await noteApi.state(token, record.block_id); if (firstNote === instance && token === session) noteComparison = comparison; }
      catch (e) { if (firstNote === instance && token === session) error = errorMessage(e); }
    }
  }
  async function cancelFirstNote() {
    if (noteView.composing || locked) return;
    if (noteView.content && !confirmCancelNote) { confirmCancelNote = true; return; }
    await run(async () => {
      await firstNote!.discard();
      firstNoteDialog.close(); firstNoteDate = null; firstNote = null; noteRevision++;
      await refreshRecovery();
    });
    if (!firstNoteDate) { await tick(); if (firstNoteReturn?.isConnected) firstNoteReturn.focus(); else await focusPage(); }
  }
  async function saveFirstNote(event: SubmitEvent) {
    event.preventDefault();
    if (!firstNoteDate || !noteView.content.trim() || noteView.composing) return;
    await run(async () => {
      await editor.flush();
      try {
        const result = await firstNote!.submit();
        const stamp = firstNote!.stamp;
        const end = noteView.content.length;
        await editor.load(result.page.id); firstNote!.assertStable(stamp); rememberPage(result.page);
        await navigation.visit(destination(result.page, result.block.id), async () => {}); navRevision++;
        firstNoteDialog.close(); firstNoteDate = null; firstNote = null; noteRevision++;
        await tick(); editor.activate(result.block.id, { start: end, end }); await refreshRecovery();
      } catch (e) { noteComparison = await noteApi.state(session!, firstNote!.blockId).catch(() => null); throw e; }
    }, 'Saving first note…');
  }
  async function showTasks() {
    await run(() => navigate({ kind: 'tasks', status: taskStatus, page: taskPage }));
  }
  async function shortcut(event: KeyboardEvent) {
    const target = event.target instanceof HTMLElement ? event.target : null;
    const action = appShortcut(event, { modal: !!document.querySelector('dialog[open]'), composing: searchComposing || graphComposing || editor.composing,
      busy: locked || !prefsReady || !registryReady, editable: !!target && (target.isContentEditable || !!target.closest('input, textarea, select')), session: !!session });
    if (!action) return;
    event.preventDefault();
    if (action === 'search') document.getElementById('search')?.focus();
    else if (action === 'capture') await capture();
    else if (action === 'pages') await goPages();
    else if (action === 'open') await pick(false);
    else await moveHistory(action === 'back' ? -1 : 1);
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
      await navigate(destination(page));
    });
  }
  onMount(() => {
    if (!isTauri()) return;
    const prefs = preferencesApi.load().then(result => { preferences.set(result.preferences); if (result.warning) prefsError = 'Preferences could not be loaded. Safe defaults are in use.'; })
      .catch(() => { prefsError = 'Preferences could not be loaded. Safe defaults are in use.'; }).finally(() => prefsReady = true);
    const graphs = graphApi.load().then(result => { registry = result; if (result.warning) registryError = 'Remembered graphs could not be loaded. Paths will not be remembered until you explicitly reset these settings.'; })
      .catch(() => { registryError = 'Remembered graphs could not be loaded. Paths will not be remembered until you explicitly reset these settings.'; }).finally(() => registryReady = true);
    void Promise.all([prefs, graphs]).then(async () => {
      if (!disposed && registry.remember && registry.reopen && !registry.warning && registry.entries[0]) await recent(registry.entries[0], true);
    });
    void recoveryApi.inventory().then(count => inventory = count).catch(() => { recoveryError = 'Recovery inventory unavailable. Open the original graph and retry recovery inspection.'; });
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void getCurrentWindow().onCloseRequested(async event => {
      if (firstNoteDate) { event.preventDefault(); try { await firstNote!.flush(); } catch (e) { error = errorMessage(e); return; } error = 'Finish or cancel the first-note form before closing.'; return; }
      if (locked) { event.preventDefault(); error = 'Wait for the current operation before closing.'; return; }
      busy = true;
      try { await editor.flush(); }
      catch (e) { event.preventDefault(); error = `Not closed: ${errorMessage(e)}`; }
      finally { busy = false; }
    }).then(stop => { if (disposed) stop(); else unlisten = stop; })
      .catch(e => { error = `Close protection unavailable: ${errorMessage(e)}`; });
    return () => { disposed = true; graphSwitch.invalidate(); unlisten?.(); };
  });
</script>

<svelte:window onkeydown={shortcut} />
<PageSwitcher bind:this={quickPages} {pages} label={pageLabel} disabled={locked || !session || composing} onOpen={select} />

<dialog bind:this={firstNoteDialog} aria-labelledby="first-note-heading" oncancel={event => { event.preventDefault(); cancelFirstNote(); }}>
  <h2 id="first-note-heading">{$tr('First note for {date}', { date: firstNoteDate ? presentDate(firstNoteDate) : '' })}</h2>
  <p>{$tr('Finalized text is captured privately before submission. The journal is created only when you submit.')}</p>
  <form onsubmit={saveFirstNote}>
    <label for="first-note">{$tr('Note text')}</label>
    <textarea id="first-note" value={noteView.content} dir="auto" rows="5" readonly={locked}
      oninput={e => firstNote?.input(e.currentTarget.value)}
      oncompositionstart={() => firstNote?.composition(true)} oncompositionend={e => { firstNote?.input(e.currentTarget.value); firstNote?.composition(false); }}></textarea>
    <p role="status">{$tr(noteView.composing ? 'Composing — finish input before leaving' : noteView.protected ? 'Recovery stored' : 'Recovery pending')}</p>
    {#if noteView.error}<ErrorNotice message={noteView.error} /><button type="button" disabled={locked || noteView.composing} onclick={() => firstNote?.capture().catch(() => {})}>{$tr('Retry recovery snapshot')}</button>{/if}
    {#if noteComparison}
      {#if noteComparison.block}<label>{$tr('Persisted version')}<textarea readonly dir="auto" value={noteComparison.block.content}></textarea></label>
        {#if noteComparison.revision !== 0 || noteComparison.block.content !== noteView.content}<p>{$tr('The persisted note differs. Copy the retained text if needed; it will not be overwritten.')}</p>{/if}
      {:else}<p>{$tr(noteComparison.deleted ? 'This note was deleted after submission. It will not be recreated.' : 'No committed note exists.')}</p>{/if}
    {/if}
    {#if error}<ErrorNotice message={error} />{/if}
    <button disabled={locked || noteView.composing || !noteView.content.trim() || noteComparison?.deleted}>{$tr('Save first note')}</button>
    <button type="button" disabled={locked || noteView.composing} onclick={cancelFirstNote}>{$tr(confirmCancelNote ? 'Discard form text and cancel' : 'Cancel')}</button>
  </form>
</dialog>

<main>
  <header><h1>Graphdesk</h1>
    {#if session}<p class="graph-path" title={activePath}>{$tr('Graph:')} <bdi dir="ltr">{activePath}</bdi></p>
      <nav aria-label={$tr('Navigation history')}>
        <button disabled={locked || !history.back} onclick={() => moveHistory(-1)} title="Alt+←">{$tr('Back')}</button>
        <button disabled={locked || !history.forward} onclick={() => moveHistory(1)} title="Alt+→">{$tr('Forward')}</button>
      </nav>
      <button disabled={locked || composing} onclick={goPages} title="Ctrl/Cmd+P">{$tr('Go to page')}</button>
      <button disabled={locked} onclick={() => switching = !switching}>{$tr(switching ? 'Cancel graph switch' : 'Switch graph')}</button>
    {:else}<p>{$tr('Open a local graph to begin your daily workspace.')}</p>{/if}
  </header>
  {#if !session || switching}
  <GraphChooser bind:path bind:composing={graphComposing} {registry} ready={prefsReady && registryReady} disabled={locked}
    onManual={open} onPick={pick} onRecent={recent} onSettings={registrySettings} onForget={forget} />
  {/if}
  {#if operation}<p role="status">{$tr(operation)}</p>{/if}
  {#if error}<ErrorNotice message={error} />{/if}
  {#if recoveryError}<ErrorNotice message={recoveryError} />{/if}
  {#if prefsError}<ErrorNotice message={prefsError} />{/if}
  {#if registryError}<section aria-label={$tr('Graph registry')}><ErrorNotice message={registryError} />
    <button disabled={locked} onclick={() => registrySettings(false, false)}>{$tr('Reset remembered paths (turn off)')}</button>
  </section>{/if}
  {#if landingError}<p role="alert">{$tr('Graph opened, but today could not be loaded. Retry Today or switch graphs.')} <button disabled={locked} onclick={() => run(landToday, 'Opening journal…')}>{$tr('Retry Today')}</button></p>{/if}
  {#if !session && inventory}<p role="status">{$tr('Recovery drafts found for {count} graph(s). Open the original graph to compare and recover text.', { count: inventory })}</p>{/if}
  {#if session}
    {#key session}<RecoveryPanel {session} records={recoveryRecords} disabled={locked} onChanged={refreshRecovery} onRecover={recover} />{/key}
  {/if}
  {#if session && pendingNotes.length}
    <section aria-label={$tr('Pending journal notes')}>
      <h2>{$tr('Pending journal notes')}</h2><p>{$tr('Restore never submits automatically. Compare persisted content before submitting again.')}</p>
      {#each pendingNotes as note (note.id)}
        <p><bdi>{presentDate(note.journal_date)}</bdi> <button disabled={locked} onclick={() => startFirstNote(note.journal_date, note)}>{$tr('Restore first-note form')}</button></p>
      {/each}
    </section>
  {/if}
  {#if session}
    <div class="workspace">
      <aside>
        <nav aria-label={$tr('Workspace')}>
          <button disabled={locked} onclick={() => journal(localDate())}>{$tr('Today')}</button>
          <button disabled={locked} onclick={showTasks} aria-current={view === 'tasks' ? 'page' : undefined}>{$tr('Tasks')}</button>
          <button disabled={locked} onclick={() => capture()} title="Ctrl/Cmd+Shift+J">{$tr('Capture today')}</button>
        </nav>
        <label for="search">{$tr('Search pages and blocks')}</label>
        <input id="search" type="search" dir="auto" bind:value={query} placeholder={$tr('Search · Ctrl/Cmd+K')} onkeydown={searchKeys}
          oncompositionstart={() => searchComposing = true} oncompositionend={() => searchComposing = false}
          aria-describedby="search-help" />
        <p id="search-help" class="hint">{$tr('Phrase search · ↑/↓ choose · Enter opens · Esc clears')}</p>
        {#if results.pending}<p role="status">{$tr('Searching…')}</p>{/if}
        {#if results.error}<ErrorNotice message={results.error} />{/if}
        {#if query.trim() && !results.pending && !results.error && !results.hits.length}<p>{$tr('No matches.')}</p>{/if}
        <ul aria-label={$tr('Search results')} class="results">{#each results.hits as hit, index (`${hit.kind}:${hit.id}`)}
          <li><button id={`search-hit-${index}`} class:chosen={index === resultIndex} aria-current={index === resultIndex ? 'true' : undefined} disabled={locked} onclick={() => selectHit(hit)} dir="auto">
            <strong>{$tr(hit.kind === 'page' ? 'Page' : 'Block')} · <bdi>{contextLabel(hit.page_id)}</bdi></strong>
            <span dir="auto">{hit.text.slice(0, 180)}</span>
          </button></li>
        {/each}</ul>
        {#if results.hits.filter(h => h.kind === 'page').length === 100 || results.hits.filter(h => h.kind === 'block').length === 100}<p class="hint">{$tr('Up to 100 results per kind. Refine your phrase to find more.')}</p>{/if}
        <details><summary>{$tr('Journal date')}</summary>
          <CalendarPicker day={journalDate} calendar={$preferences.calendar} disabled={locked} onOpen={journal} />
        </details>
        <h2>{$tr('Pages')}</h2>
        <details><summary>{$tr('New page')}</summary>
        <form onsubmit={addPage}>
          <label for="title">{$tr('New page title')}</label>
          <input id="title" bind:value={title} dir="auto" disabled={locked} />
          <button disabled={locked || !title.trim()}>{$tr('Create page')}</button>
        </form>
        </details>
        <ul class="page-list">{#each pages as page (page.id)}<li><button disabled={locked} aria-current={view === 'notes' && selected?.id === page.id ? 'page' : undefined} onclick={() => select(page)} dir="auto"><bdi>{pageLabel(page)}</bdi></button></li>{/each}</ul>
      </aside>
      <div class="content">
      {#if view === 'tasks' && session}
        <TasksView {session} {pages} refreshKey={acknowledgedBlocks} disabled={locked} bind:status={taskStatus} bind:page={taskPage} onFilter={rememberLocation}
          onOpen={hit => selectHit({ id: hit.id, page_id: hit.page_id, kind: 'block', text: hit.content })} />
      {/if}
      <div hidden={view !== 'notes'}>
      <section aria-label={$tr('Page content')}>
        {#if selected || previewDate}
          <h2 id="page-heading" tabindex="-1" dir="auto">{selected ? pageLabel(selected) : presentDate(previewDate!)}</h2>
          {#if selected?.journal_date || previewDate}
            {@const day = selected?.journal_date ?? previewDate!}
            <nav aria-label={$tr('Journal navigation')}>
              <button disabled={locked || day === '0000-01-01'} onclick={() => journal(adjacentDay(day, -1))}>{$tr('Previous day')}</button>
              <button disabled={locked} onclick={() => journal(localDate())}>{$tr('Today')}</button>
              <button disabled={locked || day === '9999-12-31'} onclick={() => journal(adjacentDay(day, 1))}>{$tr('Next day')}</button>
              <bdi>{presentDate(day)}</bdi>
            </nav>
            {#if unsupportedDate(day)}<p role="alert">{$tr('This date cannot be used in Jalali. Its Gregorian identity is unchanged.')}</p>{/if}
          {/if}
        {/if}
        {#if selected}
          <details><summary>{$tr('Rename page')}</summary>
          <form onsubmit={rename}>
            <label for="rename-title">{$tr('Page title')}</label>
            <input id="rename-title" bind:value={renameTitle} dir="auto" disabled={locked} />
            <button disabled={locked || !renameTitle.trim()}>{$tr('Rename page')}</button>
          </form>
          </details>
          {#if !handoff}<PageEditor {editor} {revision} {rowsRevision} {pages} references={links.references} onReference={followReference} disabled={busy} />{/if}
          <section aria-label={$tr('Incoming backlinks')}>
            <h3>{$tr('Backlinks')}</h3>
            {#if linksError}<ErrorNotice message={linksError} />{/if}
            {#if !links.backlinks.length}<p>{$tr('No incoming links.')}</p>{/if}
            <ul>{#each links.backlinks as block (block.id)}
              <li><button disabled={locked} onclick={() => selectHit({ id: block.id, page_id: block.page_id, kind: 'block', text: block.content })}><bdi>{contextLabel(block.page_id)}</bdi>: <bdi>{block.content}</bdi></button></li>
            {/each}</ul>
          </section>
        {:else if previewDate}
          <p>{$tr('No notes for this day yet. Browsing dates does not create a journal.')}</p>
          <button disabled={locked} onclick={() => startFirstNote(previewDate!)}>{$tr('Add a note')}</button>
        {:else}<p>{$tr('Select a page, open a journal, or capture a note for today.')}</p>{/if}
      </section>
      </div>
      </div>
    </div>
  {/if}
  <footer>{$tr('Plain-text block editor · Undo and collapse state are session-local. Acknowledged saves survive restart.')}</footer>
  {#if session}<button id="review-recovery" disabled={locked} onclick={refreshRecovery}>{$tr('Review local recovery drafts')}</button>{/if}
  <details id="settings"><summary>{$tr('Settings')}</summary>
    <label>{$tr('Language')}<select value={$preferences.language} disabled={locked || prefsSaving || !prefsReady || !!firstNoteDate} onchange={e => changePreference('language', e.currentTarget)}><option value="en">English</option><option value="fa">فارسی</option></select></label>
    <label>{$tr('Calendar')}<select value={$preferences.calendar} disabled={locked || prefsSaving || !prefsReady || !!firstNoteDate} onchange={e => changePreference('calendar', e.currentTarget)}><option value="gregorian">{$tr('Gregorian')}</option><option value="jalali">{$tr('Jalali')}</option></select></label>
    <label>{$tr('Search mode')}<select value={$preferences.search_mode} disabled={locked || prefsSaving || !prefsReady || !!firstNoteDate} onchange={e => changePreference('search_mode', e.currentTarget)}><option value="original">{$tr('Original phrase search')}</option><option value="persian">{$tr('Persian-friendly')}</option></select></label>
  </details>
  <details><summary>{$tr('Keyboard shortcuts')}</summary><p>{$tr('Ctrl/Cmd+K: search · Ctrl/Cmd+P: pages · Ctrl/Cmd+O: open graph · Ctrl/Cmd+Shift+J: capture · Alt+←/→: history outside text fields')}</p></details>
</main>

<style>
  :global(body) { margin: 0; background: #f6f7f4; color: #202b27; font: 16px/1.5 system-ui, sans-serif; }
  main { max-width: 1200px; margin: auto; padding: 1rem; }
  header { display: flex; align-items: center; flex-wrap: wrap; gap: .6rem; }
  header h1 { font-size: 1.4rem; }
  .graph-path { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 3rem; font-size: .85rem; }
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
