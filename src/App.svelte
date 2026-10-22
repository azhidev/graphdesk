<script lang="ts">
  import { onMount } from 'svelte';
  import { isTauri } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { api, editorApi, type Page } from './lib/api';
  import { EditorController, errorMessage } from './lib/editor/controller';
  import PageEditor from './lib/editor/PageEditor.svelte';

  let path = $state('');
  let pages = $state<Page[]>([]);
  let selected = $state<Page | null>(null);
  let session = $state<string | null>(null);
  let busy = $state(false);
  let error = $state('');
  let title = $state('');
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
      editor.reset(result.token); session = result.token; pages = result.pages; selected = null;
    });
  }
  async function select(page: Page) {
    await run(async () => { await editor.load(page.id); selected = page; });
  }
  async function addPage(event: SubmitEvent) {
    event.preventDefault();
    if (!session) return;
    await run(async () => {
      await editor.flush();
      const page = await api.createPage(session!, title);
      pages = [...pages, page]; title = '';
      await editor.load(page.id); selected = page;
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
        <h2>Pages</h2>
        <form onsubmit={addPage}>
          <label for="title">New page title</label>
          <input id="title" bind:value={title} disabled={locked} />
          <button disabled={locked || !title.trim()}>Create page</button>
        </form>
        <ul>{#each pages as page (page.id)}<li><button disabled={locked} aria-current={selected?.id === page.id ? 'page' : undefined} onclick={() => select(page)}>{page.title}</button></li>{/each}</ul>
      </aside>
      <section aria-label="Page content">
        {#if selected}<h2>{selected.title}</h2><PageEditor {editor} {revision} disabled={busy} />
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
