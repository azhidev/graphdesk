# Graphdesk

A new local-first knowledge application, inspired by Logseq, not a fork.
Milestones 1–2 provide a Svelte 5/TypeScript plain-text nested block editor, a thin
Tauri 2 adapter, and an independent Rust/SQLite core. No cloud services or runtime
network access required.

## Layout

- `src/`: graph/page shell and block editor; `src/lib/editor/` separates input/view
  components from the UI-independent draft/IO controller and presentation helpers.
- `src-tauri/`: desktop lifecycle, command adapters, application state accessible only
  to the local main window.
- `crates/graph-core/`: domain operations, persistence, migrations, transactional
  editor operations and bounded session history, Rust tests.
- `tests/`: additional executable SQLite schema fixtures.
- [Architecture](docs/architecture.md): storage decisions and boundaries.

## Development

Install Rust 1.99.1 (pinned in `rust-toolchain.toml`) and the [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/)
on your development computer. Linux also needs a C compiler for bundled SQLite.
Node.js 24 (`nvm use`, using `.nvmrc`) and npm are required. No global npm packages needed.
Keep rustup's `~/.cargo/bin` on PATH. Both dependency lockfiles remain tracked;
Node 24 runs the TypeScript controller tests directly without a test transpiler.

```sh
npm ci --ignore-scripts
npm run check
npm run build
npm run test:editor
cargo test --locked --workspace
npm run test:schema
npm run tauri -- dev
```

`npm run dev` is browser-only: the shell renders, but graph operations require
Tauri IPC. Development servers bind only to loopback. Production assets are local.
`cargo test -p graph-core` requires no GTK/WebKit; the desktop build does.
`Cargo.lock` is included for reproducible Rust builds. Use `--locked` for validation;
regenerate and review the lockfile deliberately when changing dependencies.

Enter a database path whose parent directory exists. Each graph is a separate
SQLite file; opening another graph switches the active connection. Create refuses
existing files. A failed creation can leave an empty reserved file; inspect it
before deciding whether to remove it.

## Implemented core

- UUID pages and nested blocks, rename/update/delete, ordered children.
- Atomic within-page moves and reordering, cycle/cross-page relationship rejection.
- Journal pages with canonical ISO dates, task statuses, JSON properties.
- Explicit block-to-page reference edges, backlinks, and Unicode FTS5 search.
- Automatic transactional migrations, history validation and future-version rejection.

References are explicit edges; parsing `[[links]]` from text is not implemented.
The UI is deliberately a plain-text outliner, not a rich-text editor. Task status,
properties, journals and references remain core APIs rather than new UI workflows.

## Block editor

Create/select a page, then choose **Add first block**. Click any block to edit it.
Markdown remains literal source; multiline paste stays inside one block.

| Key | Behavior |
| --- | --- |
| Enter | Split at the caret/selection into a following sibling; children and metadata stay on the original UUID. |
| Shift+Enter | Newline within the block. |
| Tab / Shift+Tab | Indent under the preceding sibling / outdent immediately after the parent; move the whole subtree. |
| Alt+Shift+Up / Down | Reorder among siblings; equivalent toolbar buttons are available. |
| Up / Down | Native text movement, crossing to the previous/next visible block only at absolute text start/end without a selection. |
| Backspace at nonempty text start | Focus the previous visible block; never automatically merge blocks. |
| Backspace on empty block | Outdent a nested block; remove only a metadata-free root leaf when another block exists. Children/tasks/properties/references prevent removal. |
| Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z, Ctrl+Y | Undo/redo text and structural edits within this graph session. |
| Escape | Leave the textarea for the toolbar; Tab then navigates controls normally. |

Disclosure buttons collapse/expand without modifying graph data. Collapse state is
session-local. Undo history is page-scoped and memory-only, bounded globally to 100
entries / approximately 8 MiB. Typing bursts coalesce; oversized edits still save but
clear that page's history. Reopening a graph/restarting clears history. Legacy non-editor
block mutations invalidate history; external writes also invalidate it. Concurrent
external editing is not supported.

Typing autosaves after 300 ms idle, with a one-second maximum wait. **Saved** means
acknowledged by SQLite; **Unsaved/Saving/Error** does not promise crash recovery.
Page/graph navigation and normal window close flush outstanding drafts. Storage errors
retain drafts and prevent silent navigation/close; resolve the cause and choose **Retry
saves / clear error**, or explicitly **Discard unsaved drafts and reload**. Forced
termination can lose unacknowledged text. During IME composition, structural shortcuts
and leaving with unfinished composition are blocked rather than truncating input.

## Verification in this workspace

- `npm run check`: passed, zero errors/warnings.
- `npm run build`: passed.
- `python3 -m unittest discover -s tests -v`: 14 schema tests passed using SQLite 3.45.1.
- Rust core: 25 tests passed with Rust 1.99.1, including migrations, editor operations,
  rollback injection, UUID/metadata preservation, Unicode offsets, history bounds and reopen.
- `npm run test:editor`: 15 UI-independent controller/keyboard tests passed using
  Node 24's built-in test runner; no additional npm dependency installed.
- `Cargo.lock`: generated; locked workspace metadata accepted. Registry dependencies
  use crates.io and have checksums. Locked workspace tests pass (25 core + 3 IPC).
- Tauri CLI debug build (`--no-bundle -- --locked`): passed.
- Production-protocol workspace tests (`--features tauri/custom-protocol`):
  passed, 25 core + 3 IPC, including non-main-window/remote-origin denial for every
  editor command and stale-session/structured-error checks.
- Tauri CLI release build (`npm run tauri -- build --no-bundle -- --locked`):
  passed; executable at `target/release/graphdesk-desktop`. Installer bundling is
  disabled by project configuration and was not validated.
- Desktop GUI smoke test: **passed** on 2026-10-22 using the existing release binary
  under authenticated temporary Xvfb (`-nolisten tcp`) and a private D-Bus session.
  Screenshots verified actual GTK/WebKit UI rendering. xdotool exercised graph creation,
  page creation, root-block append, and graph reopen/page selection after a clean restart;
  the saved page and block rendered again. Real WebView-to-Rust IPC passed.
- Duplicate graph creation visibly reported `File exists` without replacing the active
  graph/content. The runtime-created database returned `integrity_check=ok` and no
  foreign-key violations. Both app sessions closed normally via `WM_DELETE_WINDOW`.
- Nonfatal Xvfb warnings: unavailable GPU/DRI3 access, missing accessibility bus, and
  keyboard symbols. No sandbox or device-permission bypass was used. Fedora hardware,
  Wayland, accessibility, and installer behavior were not validated.

Python checks execute the actual migration SQL, not the Rust migration runner or
Tauri IPC. Mock-runtime IPC tests exercise command dispatch without a real WebView;
they do not establish desktop runtime behavior. Native prerequisites are installed
inside agent-lab; rustup uses the existing user's home without shell-profile changes.
All listed checks, including the real desktop smoke test, pass for the tested scope.
The GUI checks used the production WebView and UI, not mock IPC or a browser-only shell.
Temporary display sessions and test data/screenshots were cleaned up after inspection.
Milestone 1 remains closed. Milestone 2 stages 2A–2D are validated for the approved MVP.

### Milestone 2 real desktop validation (2026-10-22)

The current release executable ran under authenticated temporary Xvfb with TCP disabled,
private D-Bus and disposable app directories. Screenshots were inspected; xdotool drove
the actual WebView, with SQL assertions against disposable runtime-created graphs:

- Graph/page creation, inline editing, split, indent/outdent, sibling reorder, and
  text/structural undo/redo preserving UUIDs and descendants.
- Persian/RTL, emoji, literal Markdown, Shift+Enter multiline text, collapse/expand,
  arrow navigation, conservative Backspace, and Escape keyboard exit.
- Immediate page-switch and normal-close flush; restart restored identical UUIDs,
  parent links, order and Unicode/multiline contents. History correctly reset.
- A held SQLite writer lock caused a visible save error without losing the draft;
  close was prevented. Explicit retry after releasing the lock persisted the draft.
- A 1,000-block fixture rendered and accepted text save, split and undo. This is a
  representative smoke check, not a comprehensive latency/large-document benchmark.
- Database integrity and foreign-key checks passed. App/display sessions exited normally.

The desktop run exposed and fixed Linux WebKit's Shift+Tab event (`key=Unidentified`,
`code=Tab`, keyCode 9); a focused keyboard regression covers that event. No debug UI,
sandbox override or security-configuration change remains in the application.

**Remaining validation gaps:** no real IME service was available; composition guards
are tested synthetically, not a claim of end-to-end IME validation. Fedora/Wayland,
hardware GPU, assistive technology and installer behavior remain platform follow-ups.
No rich text, merging nonempty blocks, drag/drop, cross-page moves or persistent
undo/collapse state was added.

Next work requires separate authorization. Markdown import/export,
plugins, routines, sync, CLI, AI, and calendar presentation remain future work.
