# Graphdesk

A new local-first knowledge application, inspired by Logseq, not a fork.
Milestones 1–3 provide a Svelte 5/TypeScript plain-text nested block editor, connected
page/journal/search/reference/task workflows, a thin
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

References include explicit API edges and transactional `[[Page Name]]` inline links.
The UI is deliberately a plain-text outliner, not a rich-text editor. Properties
remain core APIs rather than a new UI workflow.

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

## Milestones 1–2 verification record

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

## Milestone 3 — Connected Knowledge Workflows

Page renaming and today's/date-selected journals are available. Journal identity
remains the local Gregorian civil date even when its page title changes. Navigation,
rename and journal creation flush drafts first; failures leave the current editor intact.

Search uses the existing Unicode FTS literal-phrase API (up to 100 page and 100 block
matches). Results reflect acknowledged SQLite text, not unsaved drafts. Query changes
and graph switches suppress stale results/errors. Choosing a block flushes drafts,
loads its owning page, expands ancestors and focuses the block by UUID.

Type `[[` for up to eight page suggestions; select a suggestion to complete the
literal source. Saved links appear as navigation buttons below their block.
Unresolved links offer explicit page creation (never implicit creation while typing).
Resolved links retain their target UUID across renames, showing the new title beside
the original label; text is not rewritten. New occurrences resolve by current title,
so reusing an old title does not retarget existing links. Removing the syntax removes
its binding. Undo/redo restores both source and binding. Incoming backlinks navigate
to source blocks. Grammar excludes nested brackets, multiline titles, aliases and
escaping; duplicate normalized labels within a block produce one edge.

Each block offers No task / TODO / DOING / DONE. Task changes flush text first,
are persisted transactionally and participate in the same page-scoped undo/redo.
Existing core CANCELLED values remain readable; the UI does not create them.

### Validation and reproduction

Automated validation: `npm run check`, `npm run build`, `npm run test:editor`,
`cargo test --locked --workspace`, `cargo test --locked --workspace --features tauri/custom-protocol`,
and `npm run test:schema`. No new runtime or npm test dependencies were added.

Real Linux WebView smoke fixture (requires the existing Tauri prerequisites plus
Xvfb, xauth, xdotool, D-Bus, and system Python PyGObject/GTK 3):

```sh
npm run tauri -- build --no-bundle -- --locked
/usr/bin/python3 tests/webview/smoke.py --artifacts /tmp/opencode/graphdesk-m3-webview
```

It launches the current release executable, uses a private D-Bus session and
authenticated Xvfb with TCP disabled, drives the actual WebView, asserts SQLite
results, captures screenshots, and closes normally. Disposable databases, app
directories, authentication files, and processes are cleaned up; screenshots/logs
remain in the chosen artifact directory, outside Git. The fixture uses fixed
1300×1000 geometry and the system's English/US date-picker layout. It is a smoke
test, not a cross-platform UI automation or accessibility suite.

Workspace toolchain note: the installed `stable` compiler reports exactly Rust
1.99.1, but downloading the named 1.99.1 rustup channel was unavailable. Validation
used `cargo +stable` and `RUSTUP_TOOLCHAIN=stable npm run tauri ...` after checking
that version. The repository remains pinned to 1.99.1; no user configuration changed.

Milestone 3 validation completed on 2026-10-22:

- Frontend check: zero errors/warnings; frontend and Tauri release builds passed.
- Frontend/controller helpers: 20 passed; SQLite schema fixtures: 15 passed.
- Locked workspace tests: 30 core + 3 IPC passed, in both default and
  production-protocol configurations. Includes migration backfill, binding-preserving
  rename/split/replay, task conflicts, injected rollback and security/session checks.
- Real release WebView fixture: passed. Screenshots inspected for actual rendering,
  suggestions, rename/backlinks, Persian search, date selection, rejected task change,
  retained drafts and restart. TODO/DOING/DONE, task undo/redo, blocked close, explicit
  retry, navigation/close flush and UUID/content/link persistence passed SQL assertions.
  Both app sessions closed normally; integrity and foreign-key checks passed.
- Final screenshots/logs are in `/tmp/opencode/graphdesk-m3-final` in this workspace.
  App/Xvfb/D-Bus processes and disposable graph/app/auth directories were cleaned up.
  Nonfatal GPU/DRI3 and accessibility-bus warnings did not require a sandbox or device
  permission override.

**GO for the approved local, trusted, single-user Milestone 3 scope.**

Real IME-service, Fedora/Wayland, hardware GPU, assistive technology and installer
validation remain follow-ups. Search is acknowledged-state, bounded phrase search;
undo/collapse remain memory-only. Renaming a page is not undoable through block history.

Markdown import/export,
plugins, routines, sync, CLI, AI, and calendar presentation remain future work.
