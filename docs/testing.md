# Testing and compatibility evidence

## Automated checks

### Recovery recurrence investigation — disposable data only

`--reliability` now tests production IPC with three additional synthetic
multi-block private snapshots: after normal exit it records IDs/timestamps,
restarts, confirms all are still listed, confirms each discard in the UI,
checks the exact remaining IDs after each deletion, closes normally, restarts,
and checks that zero records remain and canonical block text is unchanged.
The original writer-lock, forced-interruption, comparison, explicit recovery,
and failed-save assertions remain. The extra rows are fixture data inserted
**only while the disposable app is stopped**. Rust Core tests exercise durable
multi-record removal/clear and reopen; editor tests exercise overlapping saves,
multiple blocks, rapid input and delayed acknowledgements.

No recurrence of *resolved* snapshot IDs has been reproduced in those isolated
checks. A dismissed banner is not a resolved snapshot: the sidebar's Review
entry remains available. `Saved` means canonical graph text was acknowledged,
not that every older private snapshot has been dismissed or deleted. A cleanup
failure retains the snapshot and reports an editor warning. Without a before/
after list of IDs and timestamps from the affected Fedora run, its cause is
not established; do not delete or edit that user's recovery store to investigate.

For an independent **native Fedora** reproduction, use only new disposable
paths (the commands below must not point to an existing graph or application
directory):

```sh
base=$(mktemp -d)  # private, disposable test directory
export XDG_CONFIG_HOME="$base/config" XDG_DATA_HOME="$base/data" XDG_CACHE_HOME="$base/cache"
mkdir -m 700 -p "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME"
./target/release/graphdesk-desktop  # create "$base/test.sqlite3" via Create graph
```

On several blocks, edit rapidly and wait for Saved; for a separate trial use
a SQLite writer lock on **only** `$base/test.sqlite3`, capture finalized drafts,
then release the lock and retry. Resolve each entry via Compare / Recover or
confirmed Discard (the notice's × only hides the notice). Record the UI count,
whether Review remains in the sidebar, and exact persisted text. Close the
app normally before each database inspection and before restarting it with the
**same** three XDG paths. Locate the one disposable store under `$base/data`
(do not search other application directories):

```sh
find "$base/data" -type f -path '*/recovery/*.sqlite3' -print
sqlite3 -readonly "$store" 'SELECT id,graph_id,block_id,revision,updated_at FROM recovery_drafts ORDER BY updated_at,id;'
sqlite3 -readonly "$store" 'PRAGMA integrity_check;'
```

Set `store` to the displayed disposable path, never a personal recovery file.
Collect the metadata-only query **before resolution**, **after all explicit
resolutions and normal exit**, and **after restart and normal exit**. Expected
after resolution and restart: zero rows and zero UI records. If IDs survive,
investigate failed deletion; if new IDs appear, investigate new captures; if
the store is empty but UI reports records, investigate enumeration/refresh.
Report failure messages and timings without note text. Unresolved versions
expire on list after 14 days; don't treat expiry as evidence of successful
resolution. This procedure is a handoff, **not** a Fedora result.

### Phase 4 Ubuntu regression status

The production `--daily` fixture now targets the active task selector through
keyboard focus for its first status change, and uses updated workspace,
dialog, reference and graph-chooser positions elsewhere. It still checks exact
SQLite content, task transitions, canceled/confirmed subtree deletion, retained
drafts under a writer lock, normal close/reopen and graph isolation. Run with
`python3 tests/webview/smoke.py --daily --artifacts test-artifacts/phase4-daily`.
The fixture is fixed-geometry Ubuntu/Xvfb automation, not a native Fedora or
assistive-technology check. The `--navigation` fixture waits for and focuses
the actual native Open/Create dialog instead of assuming it appeared after a
fixed delay. The `--persian` fixture closes Settings explicitly, enters the
Jalali picker fields on the RTL sidebar, and restores the pending note via its
notice before asserting exact text on restart. Run these suites separately;
neither Xvfb's window-manager-less focus behavior nor XKB layout input proves
native compositor activation or real IME candidate behavior.

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

### Mixed-direction outline structure — 2026-10-30 (Ubuntu/Xvfb; Fedora pending)

Outline rows now inherit the page's LTR/RTL layout rather than deriving their
flex direction from individual text. Markers, task selectors and depth-based
indentation share that edge; the controls precede the text in visual and tab
order. Native textareas and inactive text buttons still use `dir="auto"` for
character order, while their CSS **text alignment** follows the shared outline
edge. No text, input commands, persistence, or draft notification path changed.
The previous per-block structural direction described below is historical and
is superseded by this correction.

`python3 tests/webview/smoke.py --review --artifacts
test-artifacts/webview-outline-structure-review` captures production Light,
Dark, and Persian RTL layouts with Persian-parent/English-child,
English-parent/Persian-child, three-level mixed-language nesting, and long
multiline wrapped content. The fixture checks RTL rapid ASCII key input,
mixed-script text, selection/movement without mutation, native multiline edit,
undo/redo, and exact save/reopen via disposable SQLite data. `--shell --artifacts
test-artifacts/webview-outline-structure-shell` exercises source focus, task
selector placement, undo/redo, navigation and persisted RTL preferences.
`--reliability` remains the separate production recovery regression.
Unit tests cover composition and rapid drafts, while `tests/perf/editor_revision.mjs`
models scoped draft notifications. These Ubuntu/Xvfb results are **not** a real
Persian IME candidate/caret/selection or 5,000-row WebView latency check on
native Fedora.


### Recovery and mixed-direction outline — 2026-10-30 (Ubuntu/Xvfb; Fedora pending)

Recovery uses a compact dismissible notice above the workspace. Dismissing it
does not remove the private draft; the sidebar's Review recovery drafts entry
opens a focused modal with the existing read-only comparison, revision-guarded
recover, and separately confirmed discard. The header's current save status is
independent of the older local snapshot notice; failed saves still display the
prominent header/editor error. New or updated snapshots can resurface the
notice. Native dialogs support Escape, tab traversal and focus return.

The outline row's marker, content and task control follow the first strong
letter of **acknowledged** content, independently of the UI language; native
textareas and inactive block buttons retain `dir="auto"`. This is presentation
only: drafts, caret/IME, stored Unicode, commands, block order, and persistence
are unchanged. An inactive text button sizes to its content so it stays near
its marker even in the opposite-language UI. Indentation still follows each
row's logical start; opposite-direction parent/child rows can occupy opposite
edges of the page. Evaluate that visual hierarchy on Fedora before any further
outline changes; do not silently change editing semantics to force alignment.

From a locked unbundled release build, `python3 tests/webview/smoke.py
--reliability --artifacts test-artifacts/webview-recovery-bidi-reliability`
checks a real SQLite write lock, an exact private snapshot, restart with an
external graph edit, Light/Dark/RTL notice screenshots, dismissal without data
loss, review/Escape/reopen, comparison close, discard cancellation, explicit
recovery, draft cleanup, block identity and exact restart. `--review --artifacts
test-artifacts/webview-recovery-bidi-review` captures Light/Dark/RTL nested
English/Persian outlines and checks exact mixed-script edits in RTL, undo/redo,
and reopen. `--shell --artifacts test-artifacts/webview-recovery-bidi-shell`
covers search-to-source and navigation with the new row layout. Node editor
tests cover rapid drafts, native movement contracts, and simulated composition;
the two `tests/perf/editor_*.mjs` files model 5,000-block draft notifications
and lookups, **not** real WebView input latency. Ubuntu/Xvfb production IPC is
not native Fedora/Wayland, real Persian IME candidate handling, caret/selection
visual inspection, or assistive-technology testing.


### UI Foundation Phase 3B — 2026-10-30 (Ubuntu/Xvfb, Fedora pending)

Tasks now shares the Journal's 812px content width and logical padding, with
compact filter controls and quiet rows showing task content before status and
source. Search keeps the original controller, result order, keyboard selection
and source focus, but distinguishes page titles from block snippets, uses one
scrolling sidebar, and offers an explicit keyboard-accessible clear control.
Empty Tasks filters no longer suggest creating TODO tasks when viewing DONE.
The editor and scoped per-block draft path were not changed.

After an unbundled production build, run `python3 tests/webview/smoke.py
--review --artifacts test-artifacts/webview-phase3b-review` for Light/Dark and
Persian RTL Search/Tasks screenshots, filtered task/source/Back, empty filters,
Search no-results/clear, nested text undo/redo and exact reopen. Run `--shell
--artifacts test-artifacts/webview-phase3b-shell` for keyboard search-to-source,
draft persistence, task status/undo/redo, history and RTL overlay/reopen. These
use disposable graphs and native production WebKitGTK IPC under Ubuntu/Xvfb,
not native Fedora, real Persian IME or assistive technology.

**Deferred editor visual concern:** On Persian RTL pages, right-aligned block
markers can be far from auto-direction English text. Validate that spacing with
mixed-script native input and caret/selection on Fedora before a separate
focused corrective pass; do not infer a safe bidi fix from these Tasks/Search
captures. The old `--daily` fixed-coordinate fixture still misses the moved
task selector; it is not passing Phase 3B evidence. Real 5,000-block WebView
responsiveness likewise requires a native comparison, not the synthetic model.


### UI Foundation Phase 3A — 2026-10-29 (Ubuntu/Xvfb, Fedora pending)

The Journal heading now presents the selected calendar as a localized long-form
date, without changing the stored civil-day key or the compact sidebar date.
Renamed journals retain their actual titles. The duplicate date beside the day
controls was removed. Undo/Redo remain visible; indentation, movement, deletion
and editing-shortcut help are in the keyboard-reachable More menu. The editor
keeps native textareas and the same controller/row-revision contract; ordinary
inactive blocks no longer each display a task selector. Activate a block to
change its task status; assigned tasks keep a compact selector visible.

Build the production binary and run `python3 tests/webview/smoke.py --review
--artifacts test-artifacts/webview-phase3a-review-final` for English Light/Dark,
Persian RTL/Jalali and focused nested-row captures. The fixture also edits a
nested mixed-script block, checks undo/redo, and confirms exact text after a
normal close/reopen. `--shell --artifacts test-artifacts/webview-phase3a-shell`
checks search-source focus, task status/undo/redo, the contextual action menu,
history and saved preferences. Editor tests cover rapid drafts, delayed saves,
caret/selection, composition, nested structure, task edits and recovery. Run
`node tests/perf/editor_revision.mjs` and `node tests/perf/editor_input.mjs` for
the 5,000-row synthetic notification/lookup models; neither measures WebView
mount, rendering or typing latency.

These are Ubuntu/Xvfb production WebKitGTK tests, **not** native Fedora/Wayland,
screen-reader, real Persian IME candidate/preedit or 5,000-block interactive
responsiveness evidence. The pre-Phase-1 fixed-coordinate daily smoke still
requires rebasing at its old task-selector position; do not report it as passing.


### UI Foundation Phase 2 — 2026-10-29 (Ubuntu/Xvfb, not Fedora)

Settings is a native modal with four independently persisted preferences, a
keyboard-accessible close control, Escape and focus return. The narrow workspace
sidebar has a scrim, Escape/outside-click dismissal, contained keyboard focus
and a mounted editor; the content is inert only while the overlay is open.
Recovery and save failures remain in the visible notices/editor controls, while
routine footer copy has been removed. No schema, Core, block-input or routing
changes were made.

Run the production capture fixture with `python3 tests/webview/smoke.py --review
--artifacts test-artifacts/webview-phase2-review` after an unbundled release
build. Its nine screenshots include light/dark English Journal, Persian RTL
Journal, Settings, Search, and the narrow sidebar closed/open. The `--shell`
fixture additionally checks persisted preferences after restart, source focus,
Tasks/history and narrow search. These runs use an authenticated isolated Xvfb
display, a disposable graph, and native WebKitGTK IPC; they are **not** native
Fedora/Wayland, real Persian IME or assistive-technology evidence. Legacy
fixed-coordinate daily/reliability/Persian/navigation smoke suites still require
individual rebaselining for the Phase 1 shell before claiming coverage.

### UI Foundation Phase 1 — 2026-10-29 (Ubuntu guest, not native Fedora)

Svelte check (zero errors/warnings), frontend build, 62 Node editor/navigation
tests and a locked unbundled Tauri release build passed. The production WebView
shell fixture runs on authenticated Ubuntu/Xvfb via `python3 tests/webview/smoke.py
--shell --artifacts test-artifacts/webview-phase1-shell`; it exercises a disposable
graph, capture, search-source focus, Tasks/Back, saved theme and independent
Persian/Jalali preferences, narrow RTL overlay, close and reopen. Screenshots
were inspected for English LTR and Persian RTL/dark presentation. This is not a
real Persian IME, screen-reader, Fedora/Wayland, hardware graphics or 5,000-block
performance measurement. The legacy `--daily` fixed-coordinate WebView fixture
was attempted but its initial path click missed the newly centered chooser;
remaining old coordinate suites need rebaselining before they can provide fresh
coverage for this shell. Existing editor/controller tests still cover draft,
composition and navigation guards; Rust persistence/IPC tests for the theme
preference passed in the preceding theme commit. No native Fedora validation
has been performed for this phase.

### Milestone 6 graph management and navigation — 2026-10-26

On the isolated Ubuntu 24.04 guest, both locked Rust workspace configurations
passed **63 core + 21 desktop/ACL tests** each. The new cases cover private registry
validation and bounded writes, opt-out, Unicode paths, identity and migration
guards, exclusive creation, staged activation/cancellation/reconciliation, stale
sessions, native-dialog capability denial and preserved recovery access. The
SQLite schema migrations themselves were not changed. Frontend tests cover
candidate handoff, failed saves/composition, bounded navigation, shortcuts and
page filtering. Graph picker and single-instance plugins are registered only in
the desktop adapter; no general filesystem permission was added to the WebView.

**60 Node tests** and **19 actual SQLite schema fixtures** passed. Svelte/TypeScript
reported zero errors/warnings, and the locked unbundled Tauri release build passed.
All four WebView suites passed against that production binary:

```sh
python3 tests/webview/smoke.py --navigation --artifacts test-artifacts/webview-m6-navigation-final
python3 tests/webview/smoke.py --daily --artifacts test-artifacts/webview-m6-daily-final
python3 tests/webview/smoke.py --reliability --artifacts test-artifacts/webview-m6-recovery-final
python3 tests/webview/smoke.py --persian --artifacts test-artifacts/webview-m6-persian-final
```

The navigation run used two disposable graphs, real native open/save/cancel dialogs,
clipboard/keyboard input, SQLite inspection and private app configuration under
authenticated Xvfb. It verified opt-in/opt-out and automatic reopening, Unicode
file paths, page and task-source Back/Forward, read-only journal preview, graph
isolation, blocked switching on a real writer lock followed by an explicit Retry,
and integrity/foreign-key checks on both graphs. The daily, recovery and bilingual
suites repeated their existing regressions; the Persian run observed first-note
private capture after **0.80 s** under a writer lock (one observation, not a
durability deadline). Screenshots of native switching, retained failed-save text,
returned Tasks filters, and the opt-out startup were reviewed. Artifacts are
ignored disposable data. This is production WebKitGTK evidence, not an
accessibility or cross-platform test; coordinates are fixed for Linux/Xvfb.

Earlier navigation attempts missed displaced buttons after a failed save and
showed Xvfb native-dialog focus limitations; the fixture now focuses the native
dialog explicitly and clicks the visible Retry control. No app save/composition
guard was weakened to make the fixture pass.

Second-launch focus activation is **not validated** by the window-manager-less Xvfb
fixture: the second process exits without opening its graph argument, but the
existing window did not regain focus from a separate test window. Check focus with
an actual Fedora/Wayland compositor before claiming that behavior works there.
Real Persian IME candidates, scaling/reflow, screen readers, Windows/macOS builds
and power-loss are likewise separate unvalidated checks, not implied by these
synthetic and Xvfb runs. No sandbox/VM networking changes or host installs were
used.

### Milestone 5 bilingual input/calendar validation — 2026-10-25

On the existing isolated Ubuntu 24.04 guest, Node 24.21.0, Python 3.12.3, WebKitGTK
2.52.6 / GTK 3.24.41 and pinned Rust 1.99.1 (`stable` verified to match the pin):

- Locked workspace tests passed in **both default and production-protocol modes**:
  **60 core/recovery + 8 IPC/ACL + 2 preferences tests**, zero failures.
- **50 Node tests** passed, including all prior editor regressions, all-day Jalali
  round trips over the approved interval, Nowruz/Esfand/Gregorian leap boundaries,
  digit parsing, localization fallback/interpolation, Persian-layout shortcut guards,
  pending capture timing/failure, lost/never-arriving acknowledgements and composition
  during capture, canonical submission and post-submit page loading.
- **19 actual SQLite schema fixtures** passed, including normalized-index backfill,
  exact-source preservation, mixed digit/Yeh/Kaf/ZWNJ matching, joined-word negative
  cases, trigger updates/deletes, migration failure and rollback.
- Svelte/TypeScript: **zero errors/warnings**. Frontend production build and unbundled
  locked Tauri production build passed. Only `jalaali-js` 2.0.1 was added as a runtime
  dependency package; existing `serde_json` is now a direct adapter dependency and
  the existing rusqlite dependency enables scalar functions. No historical graph
  migration was changed.

All three production WebView suites passed against the final built assets:

```sh
python3 tests/webview/smoke.py --daily --artifacts test-artifacts/webview-m5-daily-final
python3 tests/webview/smoke.py --reliability --artifacts test-artifacts/webview-m5-recovery-final
python3 tests/webview/smoke.py --persian --artifacts test-artifacts/webview-m5-persian-final
```

The daily and existing-block recovery suites retain Milestone 4 coverage described
below, including ordinary journal/page/task/link/search workflows, subtree/history,
graph switching, busy-error retained drafts, normal-close flushing, restart and
SIGKILL recovery. The bilingual suite additionally verifies:

- English → Persian → English chrome, with calendar/search choices independent and
  persisted across process restarts; the final English UI still displays Jalali dates.
- Persian/Arabic-digit Jalali selection, localized unsupported-date feedback only
  after an actual attempt, and a valid Nowruz date preview. A persistent observer's
  `PRAGMA data_version` plus page/journal identities prove preferences/calendar
  changes and absent-date browsing perform no canonical commits.
- Private first-note capture while a real graph writer lock is held, no early page
  or block, SIGKILL/restart, explicit restoration without submission, atomic submit,
  conditional cleanup and subsequent reopen. The final run observed the durable
  capture after **0.81 s** from paste initiation; this is one smoke observation,
  not a latency benchmark or a zero-loss/power-failure guarantee.
- Exact native clipboard selection of mixed Persian/English, punctuation, URL,
  Persian/Arabic/ASCII digits, ZWNJ, multiline text and emoji. A real XKB Persian
  layout on the owned disposable X display plus physical XTest keycodes exercises
  Persian letter entry and the non-Latin Ctrl+K fallback. The display's US layout
  is restored in `finally`; no system or host keyboard configuration is changed.
- Original versus Persian-friendly result behavior, source-block focus and edits,
  raw Unicode persistence, graph/private integrity and graph foreign-key checks.

Reviewed screenshots include unsupported feedback, the restored Persian first-note
form, normalized results, English/Jalali independence, RTL chrome and mixed text.
Artifacts are ignored fixture data, not committed notes. Production testing found
and fixed lost focus after asynchronous first-note discard. Fixture corrections
also address native-select Escape reverting a choice, changed layout coordinates,
and waiting for private cleanup acknowledgement rather than merely observing the
canonical commit. No application safety guard was weakened to make a fixture pass.
Final review also removed an accidental new first-note size restriction: oversized
nondestructive first-note submissions now follow the established save-and-clear-
unretainable-history rule, with a dedicated core regression.

A separate **in-memory Python SQLite** fixture with 1,000 synthetic blocks measured
allocated pages at **1,179,648 → 1,773,568 bytes**, migration/backfill about **39.37 ms**
and 1,001 normalizer calls (one page + 1,000 blocks). Task-only/no-op content updates
made zero normalization calls; one changed block made one call. This is a small SQL
fixture measurement, not a Rust IPC/WebView throughput benchmark or a universal
storage multiplier. Derived indexes deliberately increase graph size/write cost.

Native XKB input is **not an IME candidate/preedit service**. Composition ordering
tests use deterministic transports/events; an actually dropped WebView response
was not injected. `ibus`/`fcitx5` executables were unavailable on PATH, and the private
session still reports a missing accessibility bus. Real IME services, screen readers,
physical hardware/Wayland, zoom/scaling, Windows/macOS and power-loss remain
unvalidated. Sandbox, ACL and VM networking were unchanged; no system packages or
services were installed. Milestone 5 is ready to close within the tested local,
single-user Ubuntu scope, not as a cross-platform or assistive-technology guarantee.

### Milestone 4 daily workspace validation — 2026-10-25

On the existing Ubuntu 24.04 guest, Node 24.21.0, Python 3.12.3 and pinned Rust
1.99.1 (installed `stable` verified to match the pin):

- 51 Rust core/recovery + 6 mock IPC/ACL tests passed in both default and
  production-protocol configurations, including read-only journal lookup,
  renamed journal metadata, task filters/pagination/index use, history retention,
  stale sessions and non-main-window/remote-origin denial.
- 38 Node tests passed, including debounce cancellation, task stale-response and
  failed-pagination handling, civil-date boundaries, and preview departure versus
  save failure/composition. The existing Milestone 3 reliability regressions remain.
- 17 actual SQLite schema fixtures passed. No migration or dependency was added.
- Svelte/TypeScript checks: zero errors/warnings. Production frontend and unbundled
  Tauri release builds passed with lockfiles and toolchain pins unchanged.

Both refreshed production WebView suites passed against the built executable:

```sh
python3 tests/webview/smoke.py --daily --artifacts test-artifacts/webview-m4-daily-final
python3 tests/webview/smoke.py --reliability --artifacts test-artifacts/webview-m4-recovery-final
```

The daily fixture covers journal-first opening, focused Unicode capture, date
preview without writes, first-note form close/cancel guards and creation only on
submission, status/page task filters, source focus, TODO/DOING/DONE and undo,
keyboard search/result selection, acknowledged-state refresh, a real writer-lock
error with retained drafts and blocked task navigation/normal close, explicit retry,
immediate close flushing, process restart, confirmed three-level subtree deletion,
cancel/undo/redo and exact restoration, empty-leaf deletion, explicit link creation,
rename-stable references/backlink navigation, tasks across journals/ordinary pages,
680-pixel-window source editing and independent graph switching. Integrity and
foreign-key checks pass; the second graph remains independent.

The recovery fixture again captures independently under a graph writer lock, kills
only the disposable app, restarts, compares an independently changed persisted
version, cancels confirmed discard, explicitly recovers, verifies exact cleanup and
stable identity, then reopens and checks both databases' integrity. SIGKILL remains
process-crash evidence, not power-loss evidence. Reviewed screenshots include the
first-note form, active tasks, renamed backlinks, narrow source editor, graph switch,
recovery comparison/confirmation and recovered state. Artifacts are ignored local
files, not committed user data.

Early fixture runs required updated click coordinates, dismissal of native select
popups and waiting for acknowledged structure before issuing another shortcut.
A Persian-search fixture also incorrectly expected a token match after joining
Latin/Persian text without a separator; it now respects unchanged FTS5 behavior.
These failures were not hidden by weakening application save/composition guards.

AT-SPI introspection libraries exist, but the private session lacks the accessibility
bus service; an attempted accessibility-driven fixture failed before interacting
with the app. Final suites use native keyboard/clicks instead. No service/system
installation, permission bypass or sandbox change was made. Real IME candidates,
screen readers, zoom/scaling, hardware/Wayland, Windows/macOS and power-loss behavior
remain unvalidated. Jalali display/picking remains deferred and required before
claiming readiness for regular Persian-language use. The pre-submit first-note form
is explicitly not autosaved/recovery-captured; this limitation is documented in the UI
and workflows, not covered by the editor's private-snapshot guarantee.

Milestone 4 is ready to close for the approved trusted local single-user Ubuntu
scope, not as a cross-platform release or a Persian-language readiness claim.

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

See [the reliability record](reliability.md) for implemented guarantees and the
historical proposal. Unavailable native-platform checks remain separate validation.

Record pass/fail/not tested, commit, OS/architecture and toolchain/WebView versions.
Use disposable graphs for failure injection.

- Build and launch without a dev server; create/reopen after normal exit/restart.
- Test Unicode/spaced paths; duplicate creation must never overwrite data.
- Exercise split, nesting, reorder, selection, paste, undo/redo and toolbar alternatives.
- Escape exits editing, Tab navigates controls, and focus remains visible.
- Check Persian/bidi, mixed scripts, emoji, multiline caret/selection and persistence.
- Use real IME preedit/candidates/commit/cancel, shortcuts and close while composing.
- Immediate switch/close must flush. A controlled SQLite writer lock must retain
  drafts and block close; retry must save after unlock. Forced quit can lose text
  not yet captured; explicitly compare any acknowledged private recovery snapshots.
- Check scaling, small windows, large fonts, monitors, suspend/resume, graphics,
  keyboard-only use, screen readers and contrast. The palette is fixed light,
  not native dark mode; no accessibility conformance is claimed.
- Confirm offline launch/edit/reopen, separate-copy backup restore, and (when
  packaging exists) upgrade/uninstall preserving notes.
- Measure startup, idle CPU/RSS and representative edit/search latency before
  claiming performance targets or adding optimization infrastructure.
