<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { isTauri } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { api, editorApi, recoveryApi, noteApi, preferencesApi, type Page } from './lib/api';
  import { EditorController, errorMessage } from './lib/editor/controller';
  import PageEditor from './lib/editor/PageEditor.svelte';
  import RecoveryPanel from './lib/editor/RecoveryPanel.svelte';
  import type { RecoveryDraft, Block } from './lib/editor/types';
  import { localDate, adjacentDay, dateLabel, dateHeading, calendarFields, displayPageTitle } from './lib/dates';
  import CalendarPicker from './lib/CalendarPicker.svelte';
  import { preferences, tr } from './lib/preferences';
  import { get } from 'svelte/store';
  import ErrorNotice from './lib/ErrorNotice.svelte';
  import OperationStatus from './lib/OperationStatus.svelte';
  import { busyControl } from './lib/busyControl';
  import { FirstNoteController, type PendingNote, type NoteState } from './lib/firstNote';
  import { appShortcut } from './lib/appShortcuts';
  import { NavigationHistory, type Destination } from './lib/navigation';
  import { graphApi, type GraphRegistry, type RecentGraph } from './lib/graphs';
  import { graphDisplayName, graphLabels, mergeGraphs, cycleCursor } from './lib/graphList';
  import { GraphSwitch } from './lib/graphSwitch';
  import { graphKey } from './lib/graphKeys';
  import GraphChooser from './lib/GraphChooser.svelte';
  import GraphSwitchPopover from './lib/GraphSwitchPopover.svelte';
  import PageSwitcher from './lib/PageSwitcher.svelte';
  import type { TaskFilter } from './lib/tasks';
  import type { MessageKey } from './lib/i18n';
  import TasksView from './lib/TasksView.svelte';
  import { SearchController, type SearchHit } from './lib/search';
  import type { InlineReference, Links } from './lib/references';
  import { statusText } from './lib/i18n';

  let path = $state('');
  let pages = $state<Page[]>([]);
  let selected = $state<Page | null>(null);
  let session = $state<string | null>(null);
  let busy = $state(false);
  let operation = $state<MessageKey | ''>('');
  let operationScope = $state<'shell' | 'content'>('content');
  let activePath = $state('');
  let switching = $state(false);
  let handoff = $state(false), landingError = $state(false);
  let registry = $state<GraphRegistry>({ remember: false, reopen: false, entries: [], warning: false });
  let registryReady = $state(false), registryError = $state(''), graphComposing = $state(false);
  let startupError = $state('');
  // Successfully activated graphs are always visible for this session, even when
  // persistence is disabled; they are only written to disk when remembering is on.
  let sessionGraphs = $state<RecentGraph[]>([]);
  let graphCursor = $state(-1);
  let firstRun: { fresh: boolean; demo: RecentGraph | null; warning: boolean } | null = null;
  let quickPages: { show: () => Promise<void> };
  const graphSwitch = new GraphSwitch(graphApi);
  const navigation = new NavigationHistory();
  let navRevision = $state(0);
  const history = $derived.by(() => { navRevision; return { back: navigation.hasBack, forward: navigation.hasForward }; });
  let taskStatus = $state<TaskFilter>('active'), taskPage = $state('');
  let view = $state<'notes' | 'tasks'>('notes');
  let searchOpen = $state(false), sidebarCollapsed = $state(false), sidebarOverlay = $state(false), narrow = $state(false);
  let settingsOpen = $state(false);
  let graphSwitchRoot = $state<HTMLElement>();
  let graphSwitchButton = $state<HTMLButtonElement>();
  let graphSwitchReturn: HTMLElement | null = null;
  let settingsDialog: HTMLDialogElement;
  let settingsReturn: HTMLElement | null = null;
  let sidebarToggle = $state<HTMLButtonElement>();
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
  let searchInput = $state<HTMLInputElement>();
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
  $effect(() => {
    document.documentElement.lang = $preferences.language;
    document.documentElement.dir = $preferences.language === 'fa' ? 'rtl' : 'ltr';
    document.documentElement.dataset.theme = $preferences.theme;
  });
  function presentDate(day: string) {
    try { return dateLabel(day, $preferences.calendar, $preferences.language); } catch { return day; }
  }
  function headingDate(day: string) {
    try { return dateHeading(day, $preferences.calendar, $preferences.language); } catch { return presentDate(day); }
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
  let linksPending = $state(false);
  let linksContext: { token: string | null; page: string | undefined } | undefined;
  $effect(() => {
    acknowledgedBlocks; pages;
    const token = session, page = selected?.id;
    let cancelled = false;
    // A same-document refresh must not collapse every inline reference region.
    // Never retain another page/session's links, and keep stale links inert until
    // the latest request succeeds (including after a refresh failure).
    if (linksContext?.token !== token || linksContext?.page !== page) {
      links = { references: [], backlinks: [] };
      linksContext = { token, page };
    }
    linksError = ''; linksPending = !!(token && page);
    if (token && page) void api.links(token, page).then(value => {
      if (!cancelled) links = value;
    }).catch(e => { if (!cancelled) linksError = errorMessage(e); })
      .finally(() => { if (!cancelled) linksPending = false; });
    return () => { cancelled = true; };
  });
  let revision = $state(0);
  let rowsRevision = $state(0);
  const editor = new EditorController(editorApi, scope => {
    revision += 1; // Toolbar, recovery and navigation state update for every draft.
    if (scope !== 'draft') rowsRevision += 1; // Row props update only for acknowledged/structural changes.
  }, undefined, recoveryApi);
  let recoveryRecords = $state<RecoveryDraft[]>([]);
  let recoveryPanel = $state<{ show: () => Promise<void> }>();
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
  const saveView = $derived.by(() => { revision; return { status: editor.status, error: !!editor.error, dirty: editor.dirty }; });
  const knownGraphs = $derived.by(() => mergeGraphs(sessionGraphs, registry.remember ? registry.entries : []));
  const graphLabelMap = $derived(graphLabels(knownGraphs));
  const graphName = $derived(graphLabelMap.get(activePath) ?? graphDisplayName(activePath));
  const switchCandidates = $derived(knownGraphs.filter(graph => graph.path !== activePath));
  function rememberSessionGraph(graph: RecentGraph) {
    sessionGraphs = [graph, ...sessionGraphs.filter(entry => entry.path !== graph.path)].slice(0, 10);
  }
  async function closeOverlay(restore = true) {
    if (!sidebarOverlay || searchComposing || editor.composing) return;
    sidebarOverlay = false;
    if (restore) { await tick(); sidebarToggle?.focus(); }
  }
  async function toggleSidebar() {
    if (searchComposing || editor.composing) return;
    if (narrow) {
      if (sidebarOverlay) await closeOverlay();
      else { sidebarOverlay = true; await tick();
        (document.querySelector<HTMLElement>('#workspace-sidebar button:not(:disabled)'))?.focus(); }
    } else sidebarCollapsed = !sidebarCollapsed;
  }
  async function openSearch() {
    if (!session || locked) return;
    switching = false;
    searchOpen = true; sidebarCollapsed = false;
    if (narrow) sidebarOverlay = true;
    await tick(); document.getElementById('search')?.focus();
  }
  async function closeSearch() {
    if (searchComposing) return;
    searchOpen = false; await tick(); document.getElementById('search-entry')?.focus();
  }
  async function revealSettings() {
    if (settingsDialog.open || editor.composing || searchComposing) return;
    settingsReturn = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    settingsDialog.showModal(); settingsOpen = true;
    await tick(); document.getElementById('settings-theme')?.focus();
  }
  function closeSettings() {
    settingsDialog.close(); settingsOpen = false;
    if (settingsReturn?.isConnected) settingsReturn.focus();
    else sidebarToggle?.focus();
  }
  async function goPages() {
    if (locked) return;
    switching = false;
    try { departureAllowed(); await quickPages.show(); } catch (e) { error = errorMessage(e); }
  }

  async function run(action: () => Promise<void>, label: MessageKey = 'Loading…', scope: 'shell' | 'content' = 'content') {
    if (locked) return;
    busy = true; operation = label; operationScope = scope; error = '';
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
          rememberSessionGraph({ path: result.path, graph_id: result.graph_id });
          graphCursor = -1;
          searchOpen = false; sidebarOverlay = false;
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
  async function open(create: boolean) { await run(() => performSwitch(path, create), 'Opening graph…', 'shell'); }
  async function recent(graph: RecentGraph, automatic = false) {
    const before = session;
    await run(() => performSwitch(graph.path, false, graph.graph_id, automatic), 'Opening graph…', 'shell');
    // A failed switch keeps the source graph; surface the reason from the chooser
    // and return focus to the header control instead of leaving a dead popover.
    if (session === before && switching) dismissGraphSwitch();
  }
  async function openStartupGraph(graph: RecentGraph, automatic: boolean) {
    await recent(graph, automatic);
    if (!session) error = `${$tr(automatic ? 'The last active graph could not be reopened.' : 'The demo graph could not be opened.')} ${error}`;
  }
  async function pick(create: boolean) {
    const origin = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    await run(async () => {
      departureAllowed();
      const source = session;
      const candidate = await graphApi.pick(create, $preferences.language);
      if (source !== session) throw new Error('The graph operation changed. Try again.');
      if (candidate) await performSwitch(candidate, create);
    }, 'Choosing graph…', 'shell');
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
    await run(async () => { try {
      registry = await graphApi.forget(graphPath); registryError = '';
      sessionGraphs = sessionGraphs.filter(graph => graph.path === activePath || (graphPath !== null && graph.path !== graphPath));
    } catch { registryError = 'Graph registry changes could not be saved.'; } });
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
    switching = false;
    departureAllowed(); rememberLocation();
    const wasOverlay = narrow && sidebarOverlay;
    if (wasOverlay) { sidebarOverlay = false; await tick(); }
    try { await navigation.visit(target, loadDestination); navRevision++; }
    catch (e) { if (wasOverlay) sidebarOverlay = true; throw e; }
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
      if (narrow && sidebarOverlay) { sidebarOverlay = false; await tick(); }
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
    if (action === 'search') await openSearch();
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
    } else if (event.key === 'Escape') { event.preventDefault(); query = ''; searchOpen = false; if (narrow) sidebarOverlay = false; void focusPage(); }
  }
  function clearSearch() {
    if (searchComposing) return;
    query = ''; resultIndex = 0;
    searchInput?.focus();
  }
  function overlayKeys(event: KeyboardEvent) {
    const intent = graphKey(event, { session: !!session, open: switching,
      blocked: locked || !!document.querySelector('dialog[open]:not(#graph-switch-popover)'),
      composing: graphComposing || searchComposing || editor.composing });
    if (intent === 'next' || intent === 'previous') {
      // Capture phase plus preventDefault stops both WebKit focus traversal and
      // any dialog default, and composition/preedit keys are left untouched.
      event.preventDefault(); event.stopPropagation();
      void quickSwitch(intent === 'previous' ? -1 : 1);
      return;
    }
    if (intent === 'cancel' || (intent === 'activate' && graphCursor >= 0)) {
      event.preventDefault(); event.stopPropagation();
      if (intent === 'cancel') dismissGraphSwitch();
      else if (switchCandidates[graphCursor]) void recent(switchCandidates[graphCursor]);
      return;
    }
    if (event.key === 'Tab' && sidebarOverlay && !document.querySelector('dialog[open]')) {
      const controls = [...document.querySelectorAll<HTMLElement>('#workspace-sidebar button:not(:disabled), #workspace-sidebar input:not(:disabled), #workspace-sidebar select:not(:disabled), #workspace-sidebar summary')].filter(el => el.getClientRects().length);
      if (controls.length) {
        const first = controls[0], last = controls[controls.length - 1];
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
      }
    }
    if (event.key === 'Escape' && sidebarOverlay && !document.querySelector('dialog[open]') && !event.isComposing && event.keyCode !== 229 && !searchComposing && !editor.composing) {
      event.preventDefault(); event.stopPropagation(); void closeOverlay();
    }
  }
  function firstPopoverControl() { return graphSwitchRoot?.querySelector<HTMLButtonElement>('#graph-switch-popover button:not(:disabled)'); }
  function captureGraphFocus() { graphSwitchReturn = document.activeElement instanceof HTMLElement ? document.activeElement : null; }
  function dismissGraphSwitch() {
    switching = false; graphCursor = -1;
    if (graphSwitchReturn?.isConnected) graphSwitchReturn.focus({ preventScroll: true });
    else graphSwitchButton?.focus();
  }
  async function toggleGraphSwitch() {
    if (locked) return;
    if (switching) { dismissGraphSwitch(); return; }
    captureGraphFocus();
    switching = !switching;
    graphCursor = -1;
    if (switching) { await tick(); firstPopoverControl()?.focus(); }
  }
  function focusCursor() {
    if (graphCursor < 0) firstPopoverControl()?.focus();
    else graphSwitchRoot?.querySelector<HTMLButtonElement>(`#graph-switch-candidate-${graphCursor}`)?.focus();
  }
  // Ctrl+Tab opens/re-targets the existing anchored switcher; repeated presses
  // cycle forward, Ctrl+Shift+Tab cycles backward. Nothing is activated here.
  async function quickSwitch(delta: number) {
    const length = switchCandidates.length;
    if (!switching) { captureGraphFocus(); switching = true; graphCursor = length ? cycleCursor(-1, delta, length) : -1; await tick(); focusCursor(); return; }
    if (length) { graphCursor = cycleCursor(graphCursor, delta, length); await tick(); focusCursor(); }
  }
  function outsideGraphSwitch(event: MouseEvent) {
    if (switching && graphSwitchRoot && !event.composedPath().includes(graphSwitchRoot)) { switching = false; graphCursor = -1; }
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
    const media = window.matchMedia('(max-width: 760px)');
    const updateWidth = () => { narrow = media.matches; sidebarOverlay = false; };
    updateWidth(); media.addEventListener('change', updateWidth);
    if (!isTauri()) return () => media.removeEventListener('change', updateWidth);
    const prefs = preferencesApi.load().then(result => { preferences.set(result.preferences); if (result.warning) prefsError = 'Preferences could not be loaded. Safe defaults are in use.'; })
      .catch(() => { prefsError = 'Preferences could not be loaded. Safe defaults are in use.'; }).finally(() => prefsReady = true);
    const graphs = graphApi.firstRun(localDate()).then(result => {
      registry = result.registry;
      firstRun = { fresh: result.fresh, demo: result.demo, warning: result.warning };
      if (result.registry.warning) registryError = 'Remembered graphs could not be loaded. Paths will not be remembered until you explicitly reset these settings.';
      else if (result.warning) startupError = 'Startup could not be completed. Existing graphs and recovery drafts were not replaced. Open a graph manually or check local storage permissions.';
    }).catch(() => { registryError = 'Remembered graphs could not be loaded. Paths will not be remembered until you explicitly reset these settings.'; }).finally(() => registryReady = true);
    void Promise.all([prefs, graphs]).then(async () => {
      if (disposed) return;
      // A genuinely fresh installation opens its seeded demo; otherwise reopen
      // the last successfully activated graph when the user opted in.
      if (firstRun?.fresh && firstRun.demo) { await openStartupGraph(firstRun.demo, false); return; }
      if (registry.remember && registry.reopen && !registry.warning && registry.entries[0]) await openStartupGraph(registry.entries[0], true);
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
    return () => { disposed = true; graphSwitch.invalidate(); unlisten?.(); media.removeEventListener('change', updateWidth); };
  });
</script>

<svelte:window onkeydown={shortcut} onkeydowncapture={overlayKeys} onclick={outsideGraphSwitch} />
<svg class="icon-defs" aria-hidden="true" focusable="false" xmlns="http://www.w3.org/2000/svg">
  <symbol id="shell-panel" viewBox="0 0 20 20"><rect x="2.5" y="3" width="15" height="14" rx="2"/><path d="M7.5 3v14"/></symbol>
  <symbol id="shell-journal" viewBox="0 0 20 20"><rect x="3.25" y="3.25" width="13.5" height="13.5" rx="2"/><path d="M3.5 7.5h13M7 2v3M13 2v3M7 11h2.5"/></symbol>
  <symbol id="shell-tasks" viewBox="0 0 20 20"><rect x="3" y="3" width="14" height="14" rx="2"/><path d="m6.5 10 2.3 2.2 4.7-4.7"/></symbol>
  <symbol id="shell-search" viewBox="0 0 20 20"><circle cx="8.7" cy="8.7" r="5.2"/><path d="m12.6 12.6 4 4"/></symbol>
  <symbol id="shell-page" viewBox="0 0 20 20"><path d="M5 2.75h7l3 3V17H5zM12 2.75V6h3M7.5 9h5M7.5 12h5"/></symbol>
  <symbol id="shell-plus" viewBox="0 0 20 20"><path d="M10 4v12M4 10h12"/></symbol>
  <symbol id="shell-settings" viewBox="0 0 20 20"><circle cx="10" cy="10" r="2.6"/><path d="M8.8 2.5h2.4l.45 1.7 1.55.65 1.55-.9 1.7 1.7-.9 1.55.65 1.55 1.7.45v2.4l-1.7.45-.65 1.55.9 1.55-1.7 1.7-1.55-.9-1.55.65-.45 1.7H8.8l-.45-1.7-1.55-.65-1.55.9-1.7-1.7.9-1.55-.65-1.55-1.7-.45V8.8l1.7-.45.65-1.55-.9-1.55 1.7-1.7 1.55.9 1.55-.65-.45-1.7z"/></symbol>
  <symbol id="shell-left" viewBox="0 0 20 20"><path d="m11.8 4.8-5.2 5.2 5.2 5.2M7 10h10"/></symbol>
  <symbol id="shell-right" viewBox="0 0 20 20"><path d="m8.2 4.8 5.2 5.2-5.2 5.2M13 10H3"/></symbol>
  <symbol id="shell-chevron" viewBox="0 0 20 20"><path d="m5.5 7.5 4.5 4.5 4.5-4.5"/></symbol>
</svg>
<PageSwitcher bind:this={quickPages} {pages} label={pageLabel} disabled={locked || !session || composing} onOpen={select} />

<dialog bind:this={settingsDialog} class="settings-dialog" aria-labelledby="settings-heading" oncancel={e => { e.preventDefault(); closeSettings(); }}>
  <div class="settings-header"><h2 id="settings-heading">{$tr('Settings')}</h2><button class="icon-button" aria-label={$tr('Close settings')} onclick={closeSettings}>×</button></div>
  <div class="settings-fields">
    <label>{$tr('Theme')}<select id="settings-theme" value={$preferences.theme} disabled={locked || prefsSaving || !prefsReady || !!firstNoteDate} onchange={e => changePreference('theme', e.currentTarget)}><option value="system">{$tr('System')}</option><option value="light">{$tr('Light')}</option><option value="dark">{$tr('Dark')}</option></select></label>
    <label>{$tr('Language')}<select value={$preferences.language} disabled={locked || prefsSaving || !prefsReady || !!firstNoteDate} onchange={e => changePreference('language', e.currentTarget)}><option value="en">English</option><option value="fa">فارسی</option></select></label>
    <label>{$tr('Calendar')}<select value={$preferences.calendar} disabled={locked || prefsSaving || !prefsReady || !!firstNoteDate} onchange={e => changePreference('calendar', e.currentTarget)}><option value="gregorian">{$tr('Gregorian')}</option><option value="jalali">{$tr('Jalali')}</option></select></label>
    <label>{$tr('Search mode')}<select value={$preferences.search_mode} disabled={locked || prefsSaving || !prefsReady || !!firstNoteDate} onchange={e => changePreference('search_mode', e.currentTarget)}><option value="original">{$tr('Original phrase search')}</option><option value="persian">{$tr('Persian-friendly')}</option></select></label>
  </div>
  <details class="graph-options"><summary>{$tr('Recent graphs')}</summary>
    <label><input type="checkbox" checked={registry.remember} disabled={locked || !registryReady} onchange={async e => { const input = e.currentTarget; await registrySettings(input.checked, false); input.checked = registry.remember; }} />{$tr('Remember recent graphs')}</label>
    <label><input type="checkbox" checked={registry.reopen} disabled={locked || !registryReady || !registry.remember} onchange={async e => { const input = e.currentTarget; await registrySettings(true, input.checked); input.checked = registry.reopen; }} />{$tr('Reopen last graph on startup')}</label>
    {#if registry.remember && registry.entries.length}<button disabled={locked} onclick={() => forget(null)}>{$tr('Clear remembered graphs')}</button>{/if}
    <p>{$tr('Remembering stores file paths privately on this computer, not notes. Turning it off clears remembered paths.')}</p>
  </details>
  {#if prefsError}<ErrorNotice message={prefsError} />{/if}
  <div class="settings-footer"><details><summary>{$tr('Keyboard shortcuts')}</summary><p>{$tr('Ctrl/Cmd+K: search · Ctrl/Cmd+P: pages · Ctrl/Cmd+O: open graph · Ctrl/Cmd+Shift+J: capture · Ctrl+Tab: switch graph · Alt+←/→: history outside text fields')}</p></details><button class="settings-done" onclick={closeSettings}>{$tr('Done')}</button></div>
</dialog>

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

<main class="app-shell">
  <header class="app-header">
    <div class="header-start">
      {#if session}<button bind:this={sidebarToggle} class="icon-button sidebar-toggle" aria-label={$tr('Workspace')} aria-expanded={narrow ? sidebarOverlay : !sidebarCollapsed} aria-controls="workspace-sidebar" onclick={toggleSidebar}><svg class="icon" aria-hidden="true"><use href="#shell-panel" /></svg></button>{/if}
      <span class="brand-mark" aria-hidden="true"><i></i><i></i><i></i><i></i></span><span class="brand">GraphDesk</span>
      {#if session}<span class="header-divider" aria-hidden="true"></span>
        <div class="graph-switch-anchor" bind:this={graphSwitchRoot}>
          <button bind:this={graphSwitchButton} class="graph-switch" use:busyControl={locked} aria-expanded={switching} aria-controls="graph-switch-popover" aria-haspopup="dialog" onclick={toggleGraphSwitch} title={$tr('Switch graph (Ctrl+Tab)')}><span class="graph-avatar" aria-hidden="true">{graphName.charAt(0).toUpperCase()}</span><bdi dir="auto" class="graph-name">{graphName}</bdi><svg class="icon chevron" aria-hidden="true"><use href="#shell-chevron" /></svg><span class="sr-only">{$tr('Switch graph')}</span></button>
          {#if switching}<GraphSwitchPopover currentLabel={graphName} candidates={switchCandidates} labels={graphLabelMap} cursor={graphCursor} ready={registryReady} disabled={locked} onPick={pick} onSelect={recent} onCursor={index => { graphCursor = index; }} />{/if}
        </div>
      {/if}
    </div>
    {#if session}<div class="header-end">
      {#if selected}<span class:problem={saveView.error} class:pending={saveView.dirty} class="header-status" role="status"><span class="status-dot" aria-hidden="true"></span>{saveView.error ? $tr('Error — save failed') : statusText($preferences.language, saveView.status)}</span>{/if}
      <span class="header-divider" aria-hidden="true"></span>
      <nav class="history-nav" aria-label={$tr('Navigation history')}>
        <button class="icon-button" aria-label={$tr('Back')} disabled={!history.back} use:busyControl={locked} onclick={() => moveHistory(-1)} title="Alt+←"><svg class="icon direction-icon" aria-hidden="true"><use href="#shell-left" /></svg></button>
        <button class="icon-button" aria-label={$tr('Forward')} disabled={!history.forward} use:busyControl={locked} onclick={() => moveHistory(1)} title="Alt+→"><svg class="icon direction-icon" aria-hidden="true"><use href="#shell-right" /></svg></button>
      </nav>
      <button class="page-jump" use:busyControl={locked || composing} onclick={goPages} title="Ctrl/Cmd+P">{$tr('Go to page')} <kbd dir="ltr">Ctrl/Cmd P</kbd></button>
    </div>{:else}<button class="icon-button" aria-label={$tr('Settings')} onclick={revealSettings}><svg class="icon" aria-hidden="true"><use href="#shell-settings" /></svg></button>{/if}
  </header>
  {#if !session}
  <div class="graph-chooser-surface"><GraphChooser bind:path bind:composing={graphComposing} {registry} known={knownGraphs} labels={graphLabelMap} {inventory} ready={prefsReady && registryReady} disabled={locked}
    onManual={open} onPick={pick} onRecent={recent} onForget={forget} />
  </div>
  {/if}
  <div class="shell-notices">
  <OperationStatus label={operation} scope={operationScope} region="shell" />
  {#if error}<ErrorNotice message={error} />{/if}
  {#if startupError}<ErrorNotice message={startupError} />{/if}
  {#if recoveryError}<ErrorNotice message={recoveryError} />{/if}
  {#if prefsError}<ErrorNotice message={prefsError} />{/if}
  {#if registryError}<section aria-label={$tr('Graph registry')}><ErrorNotice message={registryError} />
    <button disabled={locked} onclick={() => registrySettings(false, false)}>{$tr('Reset remembered paths (turn off)')}</button>
  </section>{/if}
  {#if landingError}<p role="alert">{$tr('Graph opened, but today could not be loaded. Retry Today or switch graphs.')} <button disabled={locked} onclick={() => run(landToday, 'Opening journal…')}>{$tr('Retry Today')}</button></p>{/if}
  {#if session}
    {#key session}<RecoveryPanel bind:this={recoveryPanel} {session} records={recoveryRecords} disabled={locked} onChanged={refreshRecovery} onRecover={recover} />{/key}
  {/if}
  {#if session && pendingNotes.length}
    <section aria-label={$tr('Pending journal notes')}>
      <h2>{$tr('Pending journal notes')}</h2><p>{$tr('Restore never submits automatically. Compare persisted content before submitting again.')}</p>
      {#each pendingNotes as note (note.id)}
        <p><bdi>{presentDate(note.journal_date)}</bdi> <button disabled={locked} onclick={() => startFirstNote(note.journal_date, note)}>{$tr('Restore first-note form')}</button></p>
      {/each}
    </section>
  {/if}
  </div>
  {#if session}
    <div class="workspace" class:sidebar-collapsed={sidebarCollapsed} class:sidebar-overlay={sidebarOverlay}>
      {#if narrow && sidebarOverlay}<button class="sidebar-scrim" tabindex="-1" aria-label={$tr('Close workspace navigation')} onclick={() => closeOverlay()}></button>{/if}
      <aside class="app-sidebar" id="workspace-sidebar" aria-label={$tr('Workspace')}>
        {#if searchOpen}
        <div class="search-sidebar shell-scroll">
          <button class="back-to-workspace" onclick={closeSearch} disabled={searchComposing}><svg class="icon direction-icon" aria-hidden="true"><use href="#shell-left" /></svg>{$tr('Workspace')}</button>
          <h2>{$tr('Search')}</h2>
          <label for="search" class="sr-only">{$tr('Search pages and blocks')}</label>
          <div class="search-field"><input bind:this={searchInput} id="search" type="search" dir="auto" bind:value={query} placeholder={$tr('Search · Ctrl/Cmd+K')} onkeydown={searchKeys}
          oncompositionstart={() => searchComposing = true} oncompositionend={() => searchComposing = false}
          aria-describedby="search-help" />
          {#if query}<button class="clear-search" aria-label={$tr('Clear search')} title={$tr('Clear search')} disabled={searchComposing} onclick={clearSearch}>×</button>{/if}</div>
          <p id="search-help" class="hint">{$tr('Phrase search · ↑/↓ choose · Enter opens · Esc clears')}</p>
          {#if results.pending}<p role="status">{$tr('Searching…')}</p>{/if}
          {#if results.error}<ErrorNotice message={results.error} />{/if}
          {#if !query.trim()}<p class="search-empty">{$tr('Search pages and blocks')}</p>
          {:else if !results.pending && !results.error && !results.hits.length}<p class="search-empty">{$tr('No matches.')}</p>{/if}
          <ul aria-label={$tr('Search results')} class="results">{#each results.hits as hit, index (`${hit.kind}:${hit.id}`)}
            <li><button id={`search-hit-${index}`} class:chosen={index === resultIndex} aria-current={index === resultIndex ? 'true' : undefined} use:busyControl={locked} onclick={() => selectHit(hit)}>
              <span class="result-context"><span class="result-type">{$tr(hit.kind === 'page' ? 'Page' : 'Block')}</span><bdi dir="auto" class:page-title={hit.kind === 'page'}>{contextLabel(hit.page_id)}</bdi></span>
              {#if hit.kind === 'block'}<span class="result-snippet" dir="auto">{hit.text.slice(0, 180)}</span>{/if}
            </button></li>
          {/each}</ul>
          {#if results.hits.filter(h => h.kind === 'page').length === 100 || results.hits.filter(h => h.kind === 'block').length === 100}<p class="hint">{$tr('Up to 100 results per kind. Refine your phrase to find more.')}</p>{/if}
        </div>
        {:else}
        <div class="sidebar-top shell-scroll">
          <p class="sidebar-heading">{$tr('Workspace')}</p>
          <nav class="primary-nav" aria-label={$tr('Workspace')}>
            <button use:busyControl={locked} aria-current={view === 'notes' && !!(selected?.journal_date || previewDate) ? 'page' : undefined} onclick={() => journal(localDate())}><svg class="icon" aria-hidden="true"><use href="#shell-journal" /></svg>{$tr('Journal')}<span class="nav-hint">{$tr('Today')}</span></button>
            <button use:busyControl={locked} onclick={() => capture()} title="Ctrl/Cmd+Shift+J"><svg class="icon" aria-hidden="true"><use href="#shell-plus" /></svg>{$tr('Capture today')}</button>
            <button use:busyControl={locked} onclick={showTasks} aria-current={view === 'tasks' ? 'page' : undefined}><svg class="icon" aria-hidden="true"><use href="#shell-tasks" /></svg>{$tr('Tasks')}</button>
            <button id="search-entry" use:busyControl={locked} onclick={openSearch}><svg class="icon" aria-hidden="true"><use href="#shell-search" /></svg>{$tr('Search')}<bdi dir="ltr" class="nav-hint">Ctrl/Cmd K</bdi></button>
          </nav>
          <div class="sidebar-divider"></div>
          <p class="sidebar-heading">{$tr('Journal date')}</p>
          <details class="date-disclosure"><summary><svg class="icon" aria-hidden="true"><use href="#shell-journal" /></svg><bdi>{presentDate(journalDate)}</bdi></summary>
            <CalendarPicker day={journalDate} calendar={$preferences.calendar} disabled={locked} onOpen={journal} />
          </details>
        </div>
        <div class="pages-area"><div class="pages-heading"><h2>{$tr('Pages')}</h2>
          <details class="create-page"><summary aria-label={$tr('New page')} title={$tr('New page')}><svg class="icon" aria-hidden="true"><use href="#shell-plus" /></svg></summary>
            <form onsubmit={addPage}>
              <label for="title">{$tr('New page title')}</label>
              <input id="title" bind:value={title} dir="auto" disabled={locked} />
              <button disabled={locked || !title.trim()}>{$tr('Create page')}</button>
            </form>
          </details></div>
          <ul class="page-list shell-scroll">{#each pages as page (page.id)}<li><button use:busyControl={locked} aria-current={view === 'notes' && selected?.id === page.id ? 'page' : undefined} onclick={() => select(page)} dir="auto"><svg class="icon" aria-hidden="true"><use href="#shell-page" /></svg><bdi>{pageLabel(page)}</bdi></button></li>{/each}</ul>
        </div>
        {/if}
        <div class="sidebar-bottom">
          {#if recoveryRecords.length || recoveryError}<button id="review-recovery" use:busyControl={locked} onclick={() => { if (!locked) void (recoveryRecords.length ? recoveryPanel?.show() : refreshRecovery()); }}>{$tr('Review recovery drafts')} {#if recoveryRecords.length}<bdi>({recoveryRecords.length})</bdi>{/if}</button>{/if}
          <button onclick={revealSettings}><svg class="icon" aria-hidden="true"><use href="#shell-settings" /></svg>{$tr('Settings')}</button>
        </div>
      </aside>
      <div class="content shell-scroll" inert={narrow && sidebarOverlay}>
      <OperationStatus label={operation} scope={operationScope} region="content" />
      {#if view === 'tasks' && session}
        <TasksView {session} {pages} refreshKey={acknowledgedBlocks} disabled={locked} bind:status={taskStatus} bind:page={taskPage} onFilter={rememberLocation}
          onOpen={hit => selectHit({ id: hit.id, page_id: hit.page_id, kind: 'block', text: hit.content })} />
      {/if}
      <div hidden={view !== 'notes'} class="notes-view" aria-busy={locked}>
      <section class="page-document" aria-label={$tr('Page content')}>
        {#if selected || previewDate}
          <h2 id="page-heading" tabindex="-1" dir="auto">{selected?.journal_date && selected.title === selected.journal_date ? headingDate(selected.journal_date) : selected ? pageLabel(selected) : headingDate(previewDate!)}</h2>
          {#if selected?.journal_date || previewDate}
            {@const day = selected?.journal_date ?? previewDate!}
            <nav class="journal-nav" aria-label={$tr('Journal navigation')}>
              <button disabled={day === '0000-01-01'} use:busyControl={locked} onclick={() => journal(adjacentDay(day, -1))}>{$tr('Previous day')}</button>
              <button use:busyControl={locked} onclick={() => journal(localDate())}>{$tr('Today')}</button>
              <button disabled={day === '9999-12-31'} use:busyControl={locked} onclick={() => journal(adjacentDay(day, 1))}>{$tr('Next day')}</button>
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
          {#if !handoff}<PageEditor {editor} {revision} {rowsRevision} {pages} references={links.references} referencesBusy={linksPending || !!linksError} onReference={followReference} disabled={busy} />{/if}
          <section class="backlinks" aria-label={$tr('Incoming backlinks')}>
            <h3>{$tr('Backlinks')}</h3>
            {#if linksError}<ErrorNotice message={linksError} />{/if}
            {#if !links.backlinks.length}<p>{$tr('No incoming links.')}</p>{/if}
            <ul>{#each links.backlinks as block (block.id)}
              <li><button use:busyControl={locked || linksPending || !!linksError} onclick={() => selectHit({ id: block.id, page_id: block.page_id, kind: 'block', text: block.content })}><bdi>{contextLabel(block.page_id)}</bdi>: <bdi>{block.content}</bdi></button></li>
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
</main>

<style>
  .icon-defs { position: absolute; width: 0; height: 0; overflow: hidden; }
  .icon { width: 16px; height: 16px; flex: none; fill: none; stroke: currentColor; stroke-width: 1.5; stroke-linecap: round; stroke-linejoin: round; }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; border: 0; }
  .app-shell { height: 100vh; min-height: 480px; display: flex; flex-direction: column; overflow: hidden; background: var(--surface); }
  .app-header { display: flex; flex: 0 0 54px; align-items: center; justify-content: space-between; gap: var(--space-4); padding-inline: var(--space-4) var(--space-6); border-block-end: 1px solid var(--line); min-width: 0; }
  .header-start, .header-end, .history-nav { display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
  .header-end { margin-inline-start: auto; }
  .brand-mark { width: 16px; height: 16px; flex: none; display: grid; grid-template-columns: 1fr 1fr; gap: 3px; transform: rotate(-9deg); }
  .brand-mark i { background: var(--accent); border-radius: 2px; }.brand-mark i:nth-child(2) { opacity: .47; }.brand-mark i:nth-child(3) { opacity: .7; }.brand-mark i:nth-child(4) { opacity: .32; }
  .brand { font-size: 13px; font-weight: 700; white-space: nowrap; }
  .header-divider { width: 1px; height: 18px; background: var(--line); margin-inline: var(--space-2); flex: none; }
  .icon-button, .graph-switch, .page-jump { border: 0; background: transparent; border-radius: var(--radius-control); }
  .icon-button:hover:not(:disabled), .graph-switch:hover:not(:disabled), .page-jump:hover:not(:disabled) { background: var(--hover); }
  .icon-button { width: 30px; height: 30px; display: grid; place-items: center; padding: 0; }
  .graph-switch { display: flex; align-items: center; gap: var(--space-2); min-width: 0; padding: 5px 8px; font-size: 13px; font-weight: 610; }
  .graph-switch-anchor { position: relative; min-width: 0; }
  .graph-avatar { width: 20px; height: 20px; flex: none; display: grid; place-items: center; border-radius: 5px; background: var(--selected); color: var(--accent-ink); font-size: 11px; }
  .graph-name { max-width: 170px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .chevron { width: 14px; height: 14px; color: var(--soft); }
  .header-status { display: flex; align-items: center; gap: 7px; white-space: nowrap; font-size: 12px; color: var(--muted); }
  .status-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--success); }
  .header-status.pending .status-dot { background: var(--warning); }.header-status.problem { color: var(--danger); }.header-status.problem .status-dot { background: var(--danger); }
  .page-jump { white-space: nowrap; padding: 6px 9px; color: var(--muted); font-size: 12px; }
  kbd { font: 10px system-ui, sans-serif; color: var(--soft); border: 1px solid var(--line); border-radius: 4px; padding: 1px 4px; margin-inline-start: 8px; }
  :global([dir='rtl']) .direction-icon { transform: scaleX(-1); }
  .graph-chooser-surface { width: min(36rem, calc(100% - 32px)); max-height: calc(100vh - 110px); overflow-y: auto; margin: auto; padding-block: 16px; flex: none; }
  .shell-notices { max-height: 28vh; overflow-y: auto; padding-inline: var(--space-4); }
  .shell-notices > :global(*) { margin-block: var(--space-2); }
  .workspace { position: relative; display: grid; grid-template-columns: 254px minmax(0, 1fr); flex: 1; min-height: 0; }
  .workspace.sidebar-collapsed { grid-template-columns: minmax(0, 1fr); }
  .workspace.sidebar-collapsed .app-sidebar { display: none; }
  .app-sidebar { display: flex; flex-direction: column; min-height: 0; min-width: 0; border-inline-end: 1px solid var(--line); background: var(--sidebar); }
  .sidebar-top { flex: none; max-height: 65%; overflow-y: auto; padding: var(--space-6) var(--space-3) 0; }
  .sidebar-heading { margin: 0 var(--space-3) var(--space-3); color: var(--soft); font-size: 10px; font-weight: 750; letter-spacing: .1em; text-transform: uppercase; }
  .primary-nav { display: grid; gap: 2px; }
  .primary-nav button, .page-list button, .sidebar-bottom button { display: flex; align-items: center; width: 100%; min-height: 34px; gap: 10px; text-align: start; padding: 6px 11px; background: transparent; border: 0; border-radius: var(--radius-control); color: var(--muted); font-size: 13px; }
  .primary-nav button:hover:not(:disabled), .page-list button:hover:not(:disabled), .sidebar-bottom button:hover { background: var(--hover); }
  .primary-nav button[aria-current='page'], .page-list button[aria-current='page'] { background: var(--selected); color: var(--accent-ink); font-weight: 630; }
  .nav-hint { margin-inline-start: auto; color: var(--soft); font-size: 11px; font-weight: 450; white-space: nowrap; }
  .sidebar-divider { height: 1px; background: var(--line); margin: 19px 11px 22px; }
  .date-disclosure { margin: 0; }.date-disclosure summary { display: flex; align-items: center; gap: 10px; min-height: 34px; padding: 6px 11px; border-radius: var(--radius-control); cursor: pointer; font-size: 13px; list-style: none; }
  .date-disclosure summary::-webkit-details-marker, .create-page summary::-webkit-details-marker { display: none; }
  .date-disclosure summary:hover { background: var(--hover); }
  .pages-area { display: flex; flex: 1; flex-direction: column; min-height: 0; padding: 27px var(--space-3) 0; }
  .pages-heading { display: flex; align-items: center; justify-content: space-between; padding-inline: 11px; }
  .pages-heading h2 { margin: 0; color: var(--soft); font-size: 10px; font-weight: 750; letter-spacing: .1em; text-transform: uppercase; }
  .create-page { position: relative; }.create-page summary { display: grid; place-items: center; width: 24px; height: 24px; border-radius: var(--radius-control); color: var(--muted); cursor: pointer; list-style: none; }
  .create-page summary:hover { background: var(--hover); }
  .create-page form { position: absolute; z-index: 2; inset-inline-end: 0; top: 28px; width: 225px; padding: var(--space-3); border: 1px solid var(--line); border-radius: var(--radius-control); background: var(--surface); box-shadow: 0 6px 20px #14251b20; }
  .create-page input { width: 100%; padding: 6px; margin-block: 6px; }.create-page form button { padding: 6px 8px; }
  .page-list { flex: 1; min-height: 0; overflow-y: auto; padding: 0 0 var(--space-4); margin: var(--space-2) 0 0; list-style: none; }
  .page-list button { overflow: hidden; }.page-list bdi { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sidebar-bottom { flex: none; border-block-start: 1px solid var(--line); padding: var(--space-2) var(--space-3); }
  .search-sidebar { min-height: 0; flex: 1; overflow-y: auto; padding: 21px 15px; }
  .search-sidebar h2 { font-size: 19px; letter-spacing: -.025em; margin: 24px 5px 15px; }.back-to-workspace { display: flex; align-items: center; gap: 9px; padding: 4px 5px; border: 0; border-radius: var(--radius-control); background: transparent; color: var(--muted); font-size: 12px; }
  .back-to-workspace:hover:not(:disabled) { background: var(--hover); }
  .search-field { display: flex; align-items: center; min-height: 36px; border: 1px solid var(--line); border-radius: var(--radius-control); background: var(--surface); }
  .search-field:focus-within { outline: 2px solid var(--focus); outline-offset: 2px; }
  .search-field input { width: 100%; min-width: 0; padding: 7px 8px; border: 0 !important; outline: 0; background: transparent !important; }
  .search-field input:focus-visible { outline: 0; }
  .search-field input::-webkit-search-cancel-button { display: none; }
  .clear-search { flex: none; width: 28px; height: 28px; padding: 0; border: 0; background: transparent; color: var(--muted); font-size: 18px; }
  .clear-search:hover:not(:disabled) { background: var(--hover); }
  .hint { color: var(--muted); font-size: 11px; }.search-sidebar .hint { margin-block: var(--space-2) var(--space-4); }
  .results { margin: 0; padding: 0; list-style: none; }
  .results button { width: 100%; text-align: start; padding: 9px 8px; margin-block: 2px; border: 0; border-radius: var(--radius-control); background: transparent; overflow-wrap: anywhere; }
  .results button:hover:not(:disabled) { background: var(--hover); }
  .results button.chosen { background: var(--selected); color: var(--accent-ink); }
  .result-context { display: flex; flex-wrap: wrap; gap: 2px 7px; align-items: baseline; color: var(--muted); font-size: 11px; }
  .result-context bdi { min-width: 0; overflow-wrap: anywhere; }
  .result-context .page-title { color: var(--ink); font-size: 13px; font-weight: 630; }
  .result-type { color: var(--soft); font-weight: 650; }
  .result-snippet { display: block; margin-block-start: 4px; font-size: 12px; line-height: 1.5; white-space: pre-wrap; color: var(--ink); }
  .search-empty { padding: 12px 8px; color: var(--muted); font-size: 12px; }
  .content { min-width: 0; min-height: 0; overflow-y: auto; }
  .notes-view { min-width: 0; }.page-document { max-width: 812px; margin-inline: auto; padding: 52px 52px 88px; }
  #page-heading { font-size: 28px; line-height: 1.25; letter-spacing: -.025em; font-weight: 650; margin: 0 0 var(--space-4); }
  .journal-nav { display: flex; align-items: center; gap: 5px; flex-wrap: wrap; padding-block-end: 27px; margin-block-end: var(--space-6); border-block-end: 1px solid var(--line); }
  .journal-nav button { border: 0; background: transparent; padding: 5px 8px; color: var(--muted); font-size: 12px; }.journal-nav button:hover:not(:disabled) { background: var(--hover); }
  .page-document > details { margin-block: var(--space-4); color: var(--muted); font-size: 12px; }.page-document > details input { width: min(24rem, 100%); padding: 8px; margin: var(--space-2); }
  .backlinks { border-block-start: 1px solid var(--line); margin-block-start: var(--space-8); padding-block-start: var(--space-3); color: var(--muted); font-size: 13px; }
  .backlinks h3 { font-size: 13px; }.backlinks ul { padding: 0; list-style: none; }.backlinks button { text-align: start; max-width: 100%; overflow-wrap: anywhere; }
  .settings-dialog { width: min(440px, calc(100vw - 32px)); max-height: calc(100vh - 48px); overflow-y: auto; padding: 24px; box-shadow: 0 18px 55px #101a1438; }
  .settings-header { display: flex; justify-content: space-between; align-items: center; gap: 12px; margin-block-end: 24px; }
  .settings-header h2 { margin: 0; font-size: 21px; line-height: 1.25; }
  .settings-header button { font-size: 23px; color: var(--muted); }
  .settings-fields { display: grid; gap: 20px; }
  .settings-fields label { display: grid; gap: 7px; font-size: 13px; font-weight: 620; }
  .settings-fields select { min-height: 36px; width: 100%; padding: 6px 10px; font-weight: 400; }
  .graph-options { margin-block-start: var(--space-4); color: var(--muted); font-size: 13px; }
  .graph-options summary { cursor: pointer; }.graph-options label { display: block; margin-block: 10px; }
  .graph-options input { margin-inline-end: 8px; }
  .settings-footer { display: flex; justify-content: space-between; align-items: start; gap: 14px; border-block-start: 1px solid var(--line); margin-block-start: 28px; padding-block-start: 16px; }
  .settings-footer details { font-size: 12px; color: var(--muted); }.settings-footer summary { cursor: pointer; }
  .settings-done { padding: 7px 14px; background: var(--selected); color: var(--accent-ink); font-weight: 650; }
  [role='alert'] { color: var(--danger); white-space: pre-wrap; }
  @media (max-width: 760px) {
    .workspace, .workspace.sidebar-collapsed { grid-template-columns: minmax(0, 1fr); }
    .workspace .app-sidebar { display: none; }
    .workspace .sidebar-scrim { position: absolute; z-index: 3; inset: 0; width: 100%; height: 100%; border: 0; border-radius: 0; background: #0c161391; }
    .workspace.sidebar-overlay .app-sidebar { display: flex; position: absolute; z-index: 4; inset-block: 0; inset-inline-start: 0; width: min(254px, 80vw); box-shadow: 0 8px 28px #14251b45; }
    .page-document { padding: 38px 32px 60px; }.page-jump { display: none; }
  }
  @media (max-width: 540px) {
    .app-header { gap: 4px; padding-inline: 10px; }.header-start, .header-end { gap: 2px; }
    .brand, .header-divider { display: none; }.graph-name { max-width: 106px; }
    .page-document { padding: 32px 20px 60px; } #page-heading { font-size: 24px; }
  }
</style>
