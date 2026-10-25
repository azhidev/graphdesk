# GraphDesk

A lightweight, local-first desktop knowledge app, inspired by Logseq—not a fork.
Organize notes in a plain-text outliner with linked pages, journals, search and
tasks. Each graph is an independent SQLite file. No account, cloud service or
runtime network access is required.

**Early development (0.1.0).** Linux, Windows and macOS are target platforms, not
yet an officially supported release matrix. Existing desktop validation covers
Ubuntu with GTK/WebKit under Xvfb. Other Linux distributions, native Wayland,
Windows and macOS still need native validation. No installers or signed releases
are published. Use disposable graphs or backed-up copies for evaluation.

## Features

- Nested blocks with stable UUIDs, split, indent/outdent and sibling reordering.
- Debounced autosave, visible save/error state, and bounded session undo/redo.
- Private local text-draft snapshots with explicit compare/recover/discard.
- Confirmed subtree deletion with affected-block counts and session undo/redo.
- Pages, renaming, local-date journals, `[[page links]]` and backlinks.
- Journal-first graph opening, previous/next day previews and focused daily capture.
- Debounced Unicode phrase search with page context and keyboard navigation.
- Cross-page active-task discovery with status/page filters; edit tasks at their source.
- Transactional migrations, SQLite constraints and atomic structural edits.
- Native text inputs for Unicode/bidi text; Markdown stays literal source.

The goal is a modern, minimal, consistent UI with low CPU/memory overhead and a
maintainable architecture. These are design goals, not benchmark or accessibility
claims. Real IME services, assistive technology and hardware graphics need further
testing. See [workflows and shortcuts](docs/workflows.md).

## Architecture

Svelte 5 / TypeScript → typed Tauri 2 IPC → independent Rust core → bundled SQLite.
Tauri uses each platform's system WebView rather than shipping a browser engine.
The core has no UI dependency; there is no background service, sync engine or
plugin runtime. SQLite is the source of truth, not a cache of Markdown files.

| Location | Responsibility |
| --- | --- |
| `src/` | App shell, UI-independent editor controller and Svelte views |
| `src-tauri/` | Desktop lifecycle, session-guarded commands and explicit IPC ACL |
| `crates/graph-core/` | Domain operations, migrations, storage and undo history |
| `tests/` | Controller tests, SQL fixtures and Linux WebView smoke fixture |
| `docs/` | Architecture, usage, data safety and platform guides |

See [architecture](docs/architecture.md) for security, durability, performance
tradeoffs and known constraints. No filesystem/shell/network plugins or remote
pages are configured. Only the local main window receives graph access.

## Build from source

Requirements:

- Node.js **24.x** and npm (`.nvmrc` / `package.json`).
- Rust **1.99.1**, pinned in `rust-toolchain.toml`, with Cargo on PATH.
- A native C/C++ toolchain for bundled SQLite and the
  [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/).
- Python 3 with SQLite **3.37+**, FTS5 and JSON support for schema tests only.
  On Windows, install the `py` launcher as well.

Install native dependencies for your platform first:
[Linux](docs/platforms/linux.md) · [Windows](docs/platforms/windows.md) ·
[macOS](docs/platforms/macos.md). Build on the target OS/architecture; these
instructions are not a cross-compilation setup. If the pinned Rust channel is
unavailable, report the build as blocked rather than silently changing versions.

From the repository root:

```sh
npm ci --ignore-scripts
npm run check
npm run test:editor
npm run test:schema
cargo test --locked --workspace
npm run tauri -- dev
```

For a production-protocol executable without an installer:

```sh
npm run tauri -- build --no-bundle -- --locked
```

Both lockfiles are tracked; review deliberate dependency updates.
`npm run dev` is browser-only: graph operations require desktop IPC. Development
servers bind to loopback; production assets are local. `cargo test --locked -p
graph-core` needs no GTK/WebKit. See [testing](docs/testing.md) for scope and evidence.

## Data and limitations

Enter a native full database path in an existing writable directory. Create never
overwrites an existing file. No graph picker, default graph directory, recent-file
registry or persistent application settings are implemented. An opened graph lands
on today's journal; **Switch graph** remains accessible in the header. Browsing an
absent day is read-only. Its first-note form creates the journal on submission,
not while browsing or typing. The form is explicitly not autosaved before submit;
normal close is blocked and cancelling nonempty text requires confirmation.

**Saved** means SQLite acknowledged the write. Normal navigation/close flushes
drafts; forced termination can lose unacknowledged text. Graphs are unencrypted
and intended for trusted, local, single-user use. Do not open a graph in multiple
processes or edit it on a network/file-synchronized filesystem.

**Back up before upgrades. Never copy only a live database while WAL may contain
writes.** See [data, backup and recovery](docs/data.md).

Undo/collapse state is memory-only; page renaming is not block-undoable. Search
uses bounded acknowledged-state phrase matches. Rich text, Markdown import/export,
sync, plugins, encryption, automatic backups and structural crash recovery are not
implemented.

Jalali display/date selection remains deferred and is required before claiming
readiness for regular Persian-language use. Civil-day identity is independent of
presentation. Search retains SQLite FTS5 phrase semantics: no Persian stemming,
ZWNJ normalization or Arabic/Persian yeh/kaf equivalence is promised.

Finalized text drafts target a recovery snapshot after about 700 ms, independently
of graph writes. Recovery lives in app-private local storage, not the graph/export;
unresolved versions expire after 14 days. Startup detects snapshots; open the graph
to compare and explicitly recover them. **Saved** and **recovery snapshot stored**
are different acknowledgements. Preedit, pre-capture input, failed recovery writes,
power failure and forced termination can still lose text. See
[recovery guarantees and privacy](docs/data.md#text-draft-recovery).

Nesting has no fixed model depth cap, not an unlimited-depth guarantee. Visual
indentation is capped; SQLite/resource limits and large-outline costs still apply.
Oversized deletions are rejected if they cannot retain undo. See
[workflows and nesting limits](docs/workflows.md).

## Contributing and distribution

See [CONTRIBUTING.md](CONTRIBUTING.md) for focused changes, tests and safe bug
reports. Native platform testing is particularly useful. Packaging is deliberately
disabled for now; [distribution recommendations](docs/distribution.md) describe
the next steps without implying shipped or verified packages.

## License

[MIT](LICENSE), as already declared in the Rust and npm manifests. Dependency
licenses and platform redistribution obligations must be reviewed before release.
