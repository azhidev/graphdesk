# Testing and compatibility evidence

## Automated checks

Run from the repository root with the README's toolchains/native prerequisites:

```sh
npm run check
npm run build
npm run test:editor
npm run test:schema
cargo test --locked --workspace
cargo test --locked --workspace --features tauri/custom-protocol
```

Frontend checks/build cover Svelte/TypeScript and production assets. Node tests
cover controller, shortcuts, dates, search and links. Python checks execute actual
migration SQL using Python SQLite, not the Rust migration runner or IPC. Temporary
fixtures use the OS temp directory; no project-specific absolute path is required.

Rust core tests cover migrations, rollback, constraints, Unicode, filesystem paths,
history and reopen. Adapter mock-runtime tests cover dispatch, errors, sessions and
non-main-window/remote-origin denial. Production-protocol tests use the generated
production ACL. Mock IPC is not a real WebView test.

### Readiness review validation — 2026-10-23

On Ubuntu 24.04, Node 24.21.0 and Python 3.12.3:

- Frontend check: zero errors/warnings; frontend build passed.
- Node tests: 20 passed; schema tests through the new launcher: 15 passed.
- Locked workspace tests: 31 core + 3 IPC passed in both default and
  production-protocol configurations, including the new spaced/Unicode path test.
- Unbundled Tauri release build passed.
- Local documentation links, Fedora shell-block syntax, schema-launcher JavaScript
  syntax and Linux smoke-fixture Python syntax passed.

Rust validation explicitly selected an installed `stable` toolchain after checking
that `rustc +stable --version` reported exactly the pinned **1.99.1**; this is not
permission to substitute a different compiler. Tests used `cargo +stable`, and
the Tauri build used `RUSTUP_TOOLCHAIN=stable`. Repository pins were unchanged.
The GUI smoke fixture was **not rerun** in this review; its temporary-directory
and interpreter changes have syntax checks only. The Windows launcher branch,
native Windows/macOS builds, installers and performance metrics were not tested.

## Compatibility record

### Milestone 3 reliability validation — 2026-10-24

On the existing Ubuntu VM/toolchains: 48 Rust core/recovery tests and 4 IPC/ACL
tests passed in both default and production-protocol configurations; 32 Node
controller/keyboard tests and 17 real SQLite schema tests passed. Svelte/TypeScript
checks reported zero errors/warnings. Production frontend and unbundled Tauri
release builds passed. Historical migrations were not edited; migration 4 adds
stable graph identity and monotonic text revisions.

New coverage includes composition-finalization and navigation event ordering,
composition during recovery/page-read IO, no persisted preedit, delayed/lost/never-
arriving save acknowledgements, newer drafts/ABA conflicts, explicit replacement,
capture timing/failure, private store reopen, exact conditional cleanup/stale discard,
14-day retention boundaries, rollback, busy/read-only/missing-parent errors, Unix
file/sidecar permissions, symlink/unknown-schema/corruption rejection and bounded
`SQLITE_FULL` injection. The full condition uses a disposable database-local page
cap with a 128-KiB payload, not VM disk exhaustion. Query-only/permissions/injected
errors are not hardware I/O or power-loss tests.

The executable production fixture has a dedicated recovery mode:

```sh
python3 tests/webview/smoke.py --reliability --artifacts test-artifacts/webview-recovery
```

It holds a real graph writer lock, checks the independent private snapshot, kills
only the disposable app with SIGKILL, restarts, advances persisted content separately,
and exercises startup discovery, side-by-side comparison, confirmed discard/cancel,
explicit recover, exact cleanup, stable UUIDs, restart and both databases' integrity.
SIGKILL is process-crash evidence, not power-loss coverage. Native IME candidate
services and assistive technology are not validated by clipboard Persian input,
synthetic controller composition tests or the nonfatal AT-SPI warning.

Both final production runs passed: `test-artifacts/webview-m3-recovery-final/`
and `test-artifacts/webview-m3-regression-final/` (ignored local artifacts).
The recovery run observed a committed snapshot 0.36 seconds after its paste while
the graph writer remained locked; this is a single smoke observation, not a latency
benchmark or durability deadline. Startup discovery, discard confirmation, comparison,
retained-error state and recovery focus screenshots were inspected. The regression
run also passed links/search/tasks/journals, error-retained drafts, blocked close,
explicit retry, navigation/close flushing, subtree delete/undo/redo and restart.
An earlier run's fixed Retry coordinate missed after a long status wrapped the
toolbar; compact recovery status now avoids that layout jump and the full run passed.
Read-only-file rejection and expiration-fixture expectations were corrected during
development before the final passing Rust checks.

Milestone 3 is ready to close for the tested local single-user Ubuntu scope.
Real IME candidate services, assistive technology, zoom/reflow, physical GPU/Wayland,
Windows/macOS and power-loss/storage-hardware failures remain unvalidated. The
composition/lost-acknowledgement reproductions use deterministic controller transports;
they are not a claim of native IME or an actual dropped WebView response test.

### Milestone 2 deletion acceptance — 2026-10-23

On the same Ubuntu toolchains, the approved native-textarea architecture was retained.
Current automated results: 36 Rust core + 3 IPC/ACL tests passed in default and
production-protocol modes, 22 Node controller/keyboard tests passed, 16 SQLite schema
tests passed, and Svelte/TypeScript checks reported zero errors/warnings. The unbundled
release executable and production frontend assets built successfully.

New fixtures cover subtree preview counts, stale tokens/content/topology/metadata,
autosave flush and confirmation locking/cancel, empty last/nested leaf removal,
exact content/UUID/hierarchy/position/timestamp/task/property/reference restoration,
undo/redo failure rollback, unexpected redo descendants, reopen integrity, and
oversized deletion rejection. A 1,101-level Rust subtree passed delete/undo/redo;
an independent 1,051-level SQL fixture passed leaf-first rollback/cleanup. These are
correctness fixtures, not performance or unlimited-depth claims. Migration files
were not changed. The WebView fixture now also exercises confirmation/cancel,
subtree undo/redo, empty-leaf deletion and restart after deletion.

The final production WebView run passed under authenticated, TCP-disabled Xvfb and
private D-Bus with sandbox settings unchanged. Existing links/search/tasks/journal,
retained-draft/error/close/navigation checks passed alongside three-level subtree
confirmation/cancel, exact SQL row and inline-binding restoration on undo, redo,
last-empty-leaf deletion without a dialog, and restart/integrity/foreign-key checks.
Confirmation, restored-subtree and reopened-app screenshots were inspected. Local
artifacts are in ignored `test-artifacts/webview-m2-acceptance/`.

Earlier runs exposed lost native focus after cancellation and history redo removing
the last block. Cancellation now explicitly restores selection/focus, and removal of
the active block without a replacement focuses the toolbar; the final keyboard-driven
run passed both paths. A fixture Tab count was also corrected for exhausted redo.
A nonfatal accessibility-bus warning remains; this is not real screen-reader, IME,
Wayland/hardware-GPU or native Windows/macOS validation.

Historical milestone validation on 2026-10-22 recorded Ubuntu Linux builds and
runtime checks: 30 core + 3 IPC tests in default/production-protocol modes, 20 Node
tests, 15 schema tests, frontend checks/build and unbundled Tauri release build.
These are historical results, not proof for newer changes.

The release WebView ran under authenticated, TCP-disabled Xvfb with private D-Bus.
Checks covered graph/page creation, text/structural edits, undo/redo, Persian/emoji,
links/backlinks/rename, tasks, journals, search, error-retained drafts, blocked close,
retry, navigation/close flush and restart with UUID/content/link preservation.
Integrity/foreign-key checks passed and screenshots were inspected. A 1,000-block
edit/split/undo smoke passed, not a CPU/memory/latency benchmark. Linux WebKit's
unidentified Shift+Tab event has a focused regression. Nonfatal graphics and
accessibility-bus warnings did not require security bypasses.

Native Windows/macOS build/runtime evidence does not exist. Other Linux distributions,
Wayland, hardware GPU, real IME services, assistive technology and installer formats
remain unverified. Intended portability is not official support.

## Linux WebView smoke

Requires Tauri native prerequisites, Xvfb, xauth, xdotool, D-Bus and Python with
PyGObject/GTK 3. Use the interpreter with those bindings; the child process reuses it.

```sh
npm run tauri -- build --no-bundle -- --locked
python3 tests/webview/smoke.py --artifacts test-artifacts/webview
```

Disposable OS-temp graph/app/auth directories and child processes are cleaned up.
Screenshots/logs remain in the selected artifact directory; `test-artifacts/` is
ignored by Git. Review artifacts before sharing. Fixed 1300×1000 geometry and
English/US date-picker layout make this Linux-only, not cross-platform automation
or an accessibility suite. Do not disable sandboxing.

## Native platform checklist

See [the reliability review and proposed validation scope](reliability.md) for
newly reproduced controller risks and the next targeted checks. That proposal is
not implemented recovery behavior or additional native-platform validation.

Record pass/fail/not tested, commit, OS/architecture and toolchain/WebView versions.
Use disposable graphs for failure injection.

- Build and launch without a dev server; create/reopen after normal exit/restart.
- Test Unicode/spaced paths; duplicate creation must never overwrite data.
- Exercise split, nesting, reorder, selection, paste, undo/redo and toolbar alternatives.
- Escape exits editing, Tab navigates controls, and focus remains visible.
- Check Persian/bidi, mixed scripts, emoji, multiline caret/selection and persistence.
- Use real IME preedit/candidates/commit/cancel, shortcuts and close while composing.
- Immediate switch/close must flush. A controlled SQLite writer lock must retain
  drafts and block close; retry must save after unlock. Forced quit has no draft guarantee.
- Check scaling, small windows, large fonts, monitors, suspend/resume, graphics,
  keyboard-only use, screen readers and contrast. The palette is fixed light,
  not native dark mode; no accessibility conformance is claimed.
- Confirm offline launch/edit/reopen, separate-copy backup restore, and (when
  packaging exists) upgrade/uninstall preserving notes.
- Measure startup, idle CPU/RSS and representative edit/search latency before
  claiming performance targets or adding optimization infrastructure.
