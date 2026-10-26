# GraphDesk architecture (Milestones 1–6)

## Boundaries

Modular monolith: Svelte → typed IPC wrappers → Tauri commands → `graph-core`.
The core has no Tauri dependency. `model.rs` contains public DTOs, `lib.rs` owns
transactional graph operations, `editor.rs` owns atomic editor actions and scoped
session history, `migrations.rs` owns schema history, and SQL files
own persistent constraints. These are small modules, not a generic repository framework.
A future CLI can depend directly on the core crate.

`rusqlite` is a mature synchronous SQLite wrapper with explicit transactions and
minimal runtime infrastructure. Bundled SQLite gives consistent FTS5/JSON/STRICT
support across platforms, rather than depending on each OS's SQLite version.
The desktop serializes access through a mutex. Milestone 2 editor commands use Tauri's
blocking-task facility so SQLite work/waiting does not occupy the GUI event loop.
Legacy Milestone 1 commands remain synchronous for compatibility; the editor UI uses
the session-guarded asynchronous commands. Long-running imports/search still need an
explicit workload design, not unbounded work on the UI event loop.

## Ownership and invariants

Each graph is one database; there is no shared graph registry or attached database.
Pages and blocks are separate entities. UUIDs are generated/validated in Rust and
serialized as canonical strings. Renaming changes only page title and its key;
relationships always target UUIDs. Titles are trimmed, NFC-normalized, and keyed
using Unicode lowercase plus NFC. This is not full Unicode case folding or Persian
Arabic-letter normalization; those semantics require an explicit future migration.

SQLite enforces:

- Nonempty titles, unique title keys, stable page/block identity.
- Page ownership, composite `(parent_id, page_id)` foreign keys, no cross-page parents.
- Recursive ancestor triggers prevent cycles on both insert and reparenting.
- Unique integer positions within each root/child sibling group.
- Typed task status, valid JSON, canonical valid journal dates, one journal per date.
- Reference endpoints and cascading removal of owned data and dangling references.

Rust enforces canonical UUID/date input, title-key computation, movement bounds and
explicit cross-page move errors. SQL clients must use the same title-key convention;
raw SQL is not a supported public mutation interface. Foreign keys are enabled on
every core connection, with recursive triggers and a five-second busy timeout.

Appending assigns the next integer position; deletions may leave harmless gaps.
Moving locks the writer with `BEGIN IMMEDIATE`, parks affected siblings at unique
negative positions, changes the parent, and writes compact nonnegative positions.
All changes commit together or roll back. Reordering is O(number of affected siblings),
an intentional simple baseline. No fractional ranks, CRDTs, or background compaction.
Cross-page subtree transfer is explicitly unsupported, not silently partially applied.
Legacy `Graph` subtree/page deletion uses FK cascades: SQLite's trigger-depth limit bounds extremely
deep deletes (typically ~1000 levels); a failure rolls back the statement, not partial data.

References record one edge per source block/target page, not text spans or occurrences.
Milestone 3 adds `inline_references` in migration 3, preserving legacy explicit edges.
Rust parses plain `[[title]]`, normalizes with the existing title-key convention and
synchronizes derived rows within each content/split/history transaction. Migration 3
backfills existing text in the migration transaction. Parsed labels are keyed per block;
resolved targets bind to UUIDs and survive rename without rewriting user text. Unresolved
labels resolve transactionally on page/journal creation or rename. Old labels are not
global aliases: a fresh occurrence uses the current title namespace. Backlinks union
both kinds of edges. Raw SQL content writes do not run the Rust parser and remain unsupported.

History retains inline bindings with row deltas; derived late-resolution changes are
excluded from conflict checks. Page operations resolve null bindings in retained history
as well, so undo/redo cannot retarget a renamed page to a new page reusing its old title.
Existing timestamp/content/explicit-metadata guards remain enforced. Capacity accounting
includes inline data. Title changes themselves are not editor-undo operations.
JSON properties have separate page/block tables to retain real foreign keys; there
is no speculative property-definition/type engine. Task status is an optional block
field, not a separate routine/task subsystem.

## Durability, dates, and portability

Linux, Windows and macOS are targets; native compatibility evidence is tracked in
[testing](testing.md), not implied by the use of Tauri. The desktop receives a
user-entered path as a string and the core uses Rust `AsRef<Path>` and `OpenOptions`
without Unix path concatenation or shell expansion. Bundled SQLite avoids reliance
on system SQLite feature versions. OS WebView, input, file-locking and packaging
behavior still requires native testing.

The adapter owns app-local language, calendar and search-mode preferences in
`preferences.json`, using Tauri's platform path resolver and private atomic writes.
Private recovery storage also stays outside graphs; the core remains independent
of desktop directories. The separate opt-in graph registry described below also
belongs to the adapter. No default graph directory exists. See
[data locations and recovery](data.md).

SQLite is authoritative; Markdown will be an interchange format, not a second database.
Disk graphs use WAL and `synchronous=FULL`. Timestamps are UTC Unix seconds;
journal days are ISO 8601 Gregorian `YYYY-MM-DD` civil dates, not UTC instants.
Gregorian/Jalali presentation converts at the UI boundary without changing the
stored identity of the day.

Never copy only a live `.sqlite3` file while WAL may contain writes. Close all graph
connections before copying the database, or use SQLite's backup API/tool. Automated
backup/export is not implemented. Do not put an actively open database on a network
filesystem or treat file synchronization as multi-writer synchronization.

## Migrations

An ordered embedded list applies SQL migrations inside one immediate transaction,
including creation of `schema_migrations`. History records version, name, SQL source,
and application time. Already applied versions are not executed again. Gaps, modified
historical SQL (except CRLF/LF changes), and future versions fail closed. Nonempty
databases without migration history are rejected. Unknown/future versions are checked
before changing journal mode. Applied migration files must never be edited; add the
next version. There are no destructive down migrations.

Version 1 creates the model. Version 2 adds search and rebuilds indexes for existing
data, demonstrating a data-preserving upgrade. Failure rolls back all pending versions.
The complete SQL source in the history table avoids a checksum dependency and enables
exact drift checks; this is consistency detection, not tamper-proof authentication.

FTS5 external-content tables index titles/content with `unicode61`; triggers update
indexes in the same transaction as source changes. Rust quotes literal phrase queries
rather than exposing FTS syntax. Original search preserves existing token behavior;
the optional Persian-friendly mode adds Yeh/Kaf/digit normalization and treats ZWNJ
as a boundary, as described below. Neither mode promises stemming or transliteration.
Search is a rebuildable index, never an independent source of truth.

## Security and validation boundary

Only the local main window receives core capabilities and the explicit `graph-access`
application permission for legacy and editor graph commands. Other windows and remote origins
are denied by the application ACL, verified using the generated context in IPC tests.
No general filesystem/shell/network access or remote pages are configured for the
frontend. The user supplies the graph path explicitly or opts into remembered paths;
there is no automatic filesystem discovery. Content is rendered as text, not HTML.
Databases are unencrypted local files and must only be opened from trusted sources;
hostile database-file hardening is outside this milestone. Graph contents cannot
invoke plugins.
Only the main window can request core window destruction, needed by Tauri's normal
close-event flow after the frontend has flushed outstanding drafts. This does not
grant filesystem, shell, remote-origin or other-window graph access.

## Milestone 2 editor boundary

Native textareas provide editing, selection, bidi and composition behavior. Components
in `src/lib/editor/` are replaceable views over a UI-independent controller. That
controller owns draft text, an ordered request queue, save acknowledgements, visible
traversal, collapsed IDs and focus/selection; it does not apply domain reparenting or
integer-position algorithms. DTOs are acknowledged SQLite state, not an alternative
persistent document format. There is no rich-text JSON, Markdown canonical store,
editor framework, generic command bus or new runtime dependency.

Core `Editor` owns `Graph` and page-scoped undo/redo stacks. Each action runs in one
immediate transaction; compound split and ordering changes cannot partially commit.
Existing `move_block` internals are shared without nested transactions. Split interprets
UTF-16 selections, rejects surrogate-interior/out-of-range offsets, and leaves children,
properties, task state and reference edges on the original UUID. Backspace removal
remains conservative: only empty, metadata-free root leaves with another block remaining.
The explicit Delete action additionally supports complete subtrees and the last block.
No automatic nonempty-block merge exists.

Deletion preparation reads an acknowledged page snapshot (including metadata) and
returns a single-use opaque token, count and confirmation requirement. Only one
pending preview is retained per graph. The controller flushes drafts and holds its
normal operation lock through a labelled HTML modal dialog. Empty metadata-free leaves
skip confirmation. Commit compares the prepared page snapshot under the immediate
transaction, rejecting stale content, structure, ordering or metadata. Tokens are
page/block/session scoped; no migration or new persistent document state is needed.
The whole page comparison deliberately rejects even unrelated same-page changes.
References owned by deleted blocks and properties cascade normally; other pages and
their incoming links are not deleted. History retains exact deleted records, including
explicit edges and inline UUID bindings. Undo restores parents before children; redo
and deletion remove leaves before parents and reject unexpected descendants. This
avoids recursive block cascade depth during editor deletion. Oversized deletions roll
back before commit rather than exceeding the existing undo budget. History is still
bounded and session-only, not durable trash or crash recovery.

History retains only differing rows and metadata/timestamp guards, not persistent page
snapshots. Text saves read only their touched row; structural actions/history replay may
scan the page. Undo/redo validates preconditions, parks affected positions, and restores
UUIDs/order atomically. Unexpected children cannot be cascaded away by history replay.
Failed transactions do not advance history. Typing groups coalesce consecutive saves.
The global 100-entry / approximately 8-MiB limit evicts old history safely; an oversized
entry saves the edit but clears that page's history. Mutable legacy graph access clears
history; creating an unrelated page through the editor does not. External database
commits detected through `data_version` invalidate history. Concurrent external writing
is unsupported, and expected-content checks reject stale text overwrites.

Each opened graph receives a fresh session UUID. Editor reads, mutations, page creation
and history requests check that token under the same lock used by graph switching.
Structured errors distinguish stale sessions, conflicts, invalid operations, missing
entities, busy/storage errors and worker failures. Old queued requests cannot write into
a newly opened graph. Every new command remains covered by the explicit application ACL.

The frontend coalesces text saves (300-ms debounce, one-second maximum wait), serializes
mutations, and never replaces newer drafts with older acknowledgements. Structural
commands flush text first and apply acknowledged topology only. Page/graph navigation
and normal close await the queue; errors retain drafts and prevent silent departure.
Retry never automatically repeats an uncertain structural mutation; explicit reload
discards drafts only after a successful read. Forced termination can lose unacknowledged
drafts. Selection/focus remain UI state and use UUIDs rather than array indexes.

Rust tests cover domain operations, migration history, upgrade/reopen and a deterministic
reorder model. Python fixtures independently exercise real SQL constraints, failure
rollback, upgrade indexing, persistence, and Persian search. They do not prove the Rust
adapter compiles or that Tauri IPC works. Adapter tests use Tauri's mock runtime
with the same state setup and command registration as production. They cover command
argument decoding, responses, error handling, and graph switching, but not WebView
transport, window rendering, or desktop lifecycle. Evidence is in [testing](testing.md).

## Native validation review

The core/adapter/frontend boundaries remain consistent; no new storage abstraction or
async framework is needed for this shell. Native validation found a missing explicit
application-command ACL; it is now enforced rather than relying on core permissions
to restrict custom commands. Migrations and their recorded SQL remain unchanged.

Remaining known risks are bounded, not new editor work: synchronous commands can block
on the five-second SQLite busy timeout; failed graph creation may leave a reserved file;
live-WAL copying is unsafe; very deep cascade deletes can hit SQLite's trigger limit.
These limitations remain documented above and in the README. The real release-app
smoke test under Xvfb verified rendering, WebView IPC under production CSP, graph
creation/reopen, error display, persistence after restart, and normal shutdown.
This is runtime evidence beyond mock IPC and successful linking, but not coverage of
all desktop compositors, GPU drivers, accessibility, or unsupported UI workflows.

### Milestone 1 storage limitation disposition

These are non-blocking for the supported scope: trusted, local, single-user graph files.
They are not guarantees for unsupported workloads.

| Limitation | Disposition and boundary |
| --- | --- |
| Five-second SQLite busy timeout in synchronous commands | Non-blocking for the shell; a worker boundary is required before long-running operations. |
| Failed creation can leave a reserved file | Non-blocking: existing files are never overwritten, failure is reported, and cleanup is deliberately not automatic. Inspect before removal. |
| Live WAL database copies can omit committed writes | Non-blocking with the documented close-before-copy rule; live backup/export must use a SQLite-aware backup mechanism. |
| Very deep cascade deletes can exceed SQLite trigger depth | Non-blocking for Milestone 1; failed deletion rolls back rather than partially deleting. Arbitrarily deep outlines are not guaranteed. |
| Network filesystems and file-sync multi-writer use | Unsupported, non-blocking for local-file use. Do not treat file synchronization as database concurrency control. |
| Unencrypted files and trusted-database requirement | Non-blocking within the stated threat model; encryption and hostile-database hardening are not implemented. |
| Limited Unicode title matching and FTS tokenization | Non-blocking with documented semantics; broader normalization requires an explicit future migration, not silent rewriting. |

No known Milestone 1 blocker remains within the trusted single-user scope. Two
authenticated temporary Xvfb/private D-Bus sessions ran the existing
release executable with TCP display listening disabled and sandbox settings unchanged.
Screenshots and xdotool verified graph/page/root-block workflows through the actual UI,
including saved content after restart and a duplicate-create error preserving state.
The runtime-created graph also passed SQLite integrity and foreign-key checks.
The sessions closed normally; temporary processes and test artifacts were cleaned up.

Milestone 1's historical scope is closed. Native desktop tests across target
platforms remain necessary for compositor/GPU, input and accessibility behavior.
Journals and nested editing are now available in the UI; see [workflows](workflows.md)
and [testing](testing.md) for current scope and evidence.

## Milestone 2 validation disposition

Stages 2A–2C passed core, IPC/security and UI-independent controller/keyboard tests before
desktop validation. Stage 2D exercised the release app under isolated Xvfb: text and
structure, undo/redo, real IPC, RTL/emoji, error-retained drafts, blocked close on save
failure, retry, immediate close/page-switch flush and restart persistence. A 1,000-block
fixture exercised rendering/edit/split/undo without introducing virtualization.
The only runtime defect found was unidentified Shift+Tab keyboard events on Linux
WebKit; the known physical Tab code is now recognized without reinterpreting unknown
IME/text keys. Real IME service integration, hardware/Wayland and assistive technology
are not validated by these checks. Existing storage limitations remain applicable.

The 2026-10-23 acceptance extension adds explicit confirmed subtree deletion, with
no confirmation for simple empty leaves. Core/controller/IPC/schema tests and the
refreshed production WebView run passed, including cancellation focus, whole-subtree
undo/redo, empty-page focus, restart and integrity checks. Editor leaf-first deletion
avoids the legacy recursive block-cascade limit; undo-budget/resource limits still
apply and oversized deletions fail atomically. Milestone 2 is ready to close within
the documented trusted local single-user scope, not as a cross-platform release or
an unlimited-depth/performance/accessibility guarantee. See [testing](testing.md).

## Milestone 3 workflow boundary

The original page/link/search/task milestone name below is historical. The approved
reliability extension adds independent private text recovery as described next.

### Reliability extension

Migration 4 adds a stable graph identity and text-revision tombstones. Triggers
increment revisions on content changes and deletion/restoration, preventing stale
text writes across same-content ABA and undo/redo. `save_text` checks expected text
and revision in the editor transaction; no-op saves perform no content update.
Outline content/revisions are read in one SQLite read transaction. History remains
session-only; recovery does not replay structural actions.

The frontend rejects navigation/structural locks synchronously during composition,
checks again after awaiting IO, and keeps the active composing textarea mounted.
Final composition input is accepted before composition state is cleared; preedit is
excluded from save/recovery. Failed text saves retain their sent content/revision.
Retry reads actual persisted state: only an unchanged base or matching one-step
committed result can reconcile automatically. Newer drafts are retained. Other
revisions require explicit comparison/replacement under fresh expected-state guards.

`graph-core::recovery` is a standalone SQLite store, opened only by the desktop
adapter at `app_local_data_dir()/recovery/<graph UUID>.sqlite3`. This approved
app-private store is independent of the graph and uses no new dependency. A separate
worker/mutex/connection avoids graph writer-lock interference. Snapshot requests are
serialized separately from graph commands and coalesced per finalized text draft,
targeting 700 ms. Acknowledged capture and canonical save are distinct states;
errors remain visible. Cleanup is exact-content conditional, so a late acknowledgement
cannot erase a newer different snapshot. Explicit discard also checks the compared
content/time. Other unresolved base versions survive until explicit resolution or
14-day expiry. Startup inventories only private recovery files, not graph paths.

Recovery UI displays both versions before explicit text replacement. It validates
the displayed block text/revision again before applying and clears private content
only after graph acknowledgement. Cross-database cleanup is intentionally not atomic:
a crash can leave an already-applied copy, which is safely compared/reconciled again.
No graph search, graph backup/export, telemetry, or log includes recovery text.
Privacy, copied-graph identity, expiry and loss-window limits are in [data](data.md).

The page/link/search/task commands use the graph blocking worker, mutex, session token and
explicit main-window ACL as the editor commands. Page rename and journal operations
flush frontend drafts before dispatch, retain page UUIDs and preserve valid block
history. Journal selection uses a local Gregorian civil day, never UTC conversion.
Task mutations have expected-state guards and run through `EditAction` and its
transaction/history path; failed writes never advance acknowledged frontend state.

Search generations invalidate older successes and failures on query/graph changes.
Block navigation flushes drafts, loads the owning page and reveals collapsed ancestors
by UUID. Link/backlink reads similarly reject stale page/graph responses. Frontend
suggestions are bounded presentation over the current page list, not a second resolver.
No remote content, HTML rendering, network capability, plugin, or dependency was added.

The executable WebView smoke fixture is `tests/webview/smoke.py`; its fixed geometry
is deliberately separate from UI-independent tests. It runs production assets/IPC
in an isolated authenticated Xvfb session, not a browser-only shell or mock runtime.

## Milestone 4 daily workspace

This section records the Milestone 4 boundary; Milestones 5–6 below supersede the
pre-submit recovery, calendar and graph-navigation limitations without changing
canonical identities.

No migration or dependency was added. `Page` now includes optional `journal_date`,
read by joining the existing journal table rather than parsing a title. Session-guarded
`editor_find_journal` performs a validated, read-only lookup. The editor controller's
`unload` flushes/guards composition before detaching a page for a date-only preview;
the preview has no fake UUID, persistent block model or separate autosave queue.

Opening a graph explicitly creates/opens today's journal. Date browsing only looks
up existing journals. An absent day's first-note form submits nonempty text before
creating the journal, then reuses append, revision-guarded text saves and recovery.
The pre-submit form is explicitly not autosaved/recovery-protected; it blocks normal
close and confirms nonempty cancellation. Failed submission retains form text; once
a block is acknowledged, retries reconcile that same block instead of appending it
again. Canonical journal creation, block append and text save remain separate guarded
operations, not an atomic multi-command transaction: failure may leave an empty journal
or block, but must not silently lose the submitted text. No structural action is
automatically replayed after an uncertain acknowledgement.

`dates.ts` defines ISO civil-day identity, calendar-independent day stepping and a
calendar presentation interface. Its current Gregorian formatter/picker are UI
choices, not stored page titles or alternative day keys. Future Jalali display and
picker conversion must map to the same ISO day/UUID without a database migration.
Jalali UI is deferred; regular Persian-language readiness is not claimed.

Search retains both existing FTS5 indexes and literal phrase semantics/ranking.
The controller debounces dispatch for 200 ms but invalidates prior generations
immediately; the shell pauses preedit queries and coalesces acknowledged-state
refreshes. Results show owning-page context and bounded previews. Draft/recovery
content is not indexed. No fuzzy search, new normalization or search cache was added.

`editor_tasks` uses the same blocking worker, session token, mutex and explicit ACL
as other editor reads. The core queries the existing partial `blocks_tasks` index,
joins page titles, validates typed filters and caps each request at 100 rows. Stable
ordering is page title key, page UUID, block UUID; offset pagination reflects current
acknowledged state, not a persistent snapshot under unsupported concurrent writers.
The Tasks UI is discovery-only: source navigation uses existing reveal/focus and task
changes still use `EditAction::Task` with its expected-state/history guarantees.

The shell retains the mounted editor while Tasks is visible, but all transitions
flush first. Application shortcuts avoid composition and modal dialogs. Graph
switching remains explicit path entry; no persistent settings/path registry, extra
filesystem capability, network listener or background service was introduced.

## Milestone 5 bilingual calendar and input boundary

`jalaali-js` 2.0.1 (MIT, no runtime dependencies) supplies conversion and validation.
The guaranteed Jalali window is Gregorian 1800-01-01 through 2256-12-31, inclusive;
the Node fixture exhaustively round-trips every day in that interval. The window is
developer documentation, not normal UI copy. Only an actual unsupported attempt
shows a localized message. Existing dates outside the window stay accessible and
retain Gregorian identity; calendar selection is never silently changed. Gregorian
input retains the existing canonical range. `Intl` is not a second conversion
authority. Labels/picker use the same calendar-fields functions; Today still takes
local civil fields while day arithmetic avoids DST/UTC-instant conversion.

English/Persian dictionaries are typed and centralized. Missing translations fall
back to English source keys; unexpected domain errors have a localized explanation
with optional LTR technical details. Language directs application chrome, not stored
content. Native textareas retain auto direction, native horizontal movement and
selection, with bidi isolation around user labels/identifiers. Shortcut physical-key
fallbacks apply only to non-Latin shortcut keys and never during composition/AltGr.

The first-note controller shares the existing private RecoveryStore and independent
worker. A pending note has graph identity, canonical date, stable draft identity,
intended block UUID and a versioned finalized snapshot. It has no fake page identity
and does not enter canonical search before explicit submission. Capture uses the
same 700-ms target, preedit exclusion, 14-day expiry and visible failure boundary.
Restoration is explicit into the form. Submission uses a single Rust journal/block
transaction and the same intended UUID on retries. Existing matching initial state
reconciles; changed revision/content, wrong ownership or a deletion tombstone fails
closed. An acknowledgement timeout does not cancel SQLite work or license a second
append. Recovery cleanup is conditional and occurs only after acknowledged success.

Minimal app-local preferences remember only language, calendar and search mode.
Defaults are English/Gregorian/original; choices are independent of graphs and one
another. The adapter owns private platform-resolved storage and serialized atomic
writes. The UI applies changes only after acknowledgement. No general configuration
framework, graph registry, broad filesystem plugin or remote service is introduced.

Graph migration 5 adds contentful normalized FTS5 indexes, preserving the two
original external-content indexes and their default query path. The deterministic,
innocuous Rust `persian_normalize` SQLite scalar is registered before migrations.
New triggers update normalized rows in the same canonical transaction, including
history/recovery writes. Raw-unchanged title/content updates and task-only changes
skip normalization. Each changed text is normalized once; no per-query index rebuild
or frontend corpus cache exists. The explicit rebuild method regenerates from
canonical text. Original text/title keys/reference bindings are never folded.

This deliberately adds derived text/index storage and write cost even while original
search mode is selected, so mode switching is immediate and does not change the
graph schema. Normalized matching can alter ranking and return both distinct
variant-title pages. Raw SQLite mutation clients must register the scalar function;
raw writes remain unsupported. Read-only inspection does not need it. Existing
backups need the usual pre-upgrade precautions; older binaries reject graph version
5 and recovery-store version 2 rather than discarding unknown data.

## Milestone 6 graph management and navigation

The desktop adapter owns native graph dialogs and the separate versioned private
graph registry. Remembering and startup reopening default to off. Registry entries
are bounded to ten, contain resolved absolute paths and expected graph UUIDs, and
are ordered only by successful activation. No note/page/search/history data is
stored. Files sharing a logical graph UUID remain separate path entries (copies
also share recovery identity, so existing compare-before-recover guidance applies).
Registry writes are serialized and atomic; failures are not graph-open failures.
Corrupt/future registry data disables retention until explicit settings reset.
There is no graph-directory scan, path relocation, file deletion or default graph
directory. Forget/disable affects only metadata, not recovery or graph files.

All frontend opening routes share `GraphSwitch`: flush and check source editor
state; prepare a candidate without replacing the active graph; recheck composition,
input generation and source session; detach native inputs; activate; synchronously
install the acknowledged frontend session. Backend activation compares the expected
source token and changes graph/recovery access under the established lock ordering.
Prepared graphs have bounded, one-use tickets. An exact current-session activation
receipt reconciles a lost acknowledgement without repeating activation. No graph,
registry and recovery multi-file atomicity or rollback of candidate migrations is
claimed. Opening failure retains the source; a subsequent Today-load failure shows
the new active graph with an explicit retry. History resets on activation, not when
opening the chooser. Automatic reopening requires a remembered expected UUID and
current supported graph format; remembered identity is inspected before writable
opening and checked again on the writable connection inside the migration
transaction before any upgrade.

Official Tauri dialog and single-instance integrations stay in Rust. Native dialogs
hold no graph/recovery lock. Only narrow custom commands receive main-window ACL
access; the frontend gains no general filesystem/shell permissions. Single-instance
registration precedes other plugins and a second launch only raises the current
window, ignoring arguments. On Linux it uses the user-session D-Bus service while
the app runs, not a system service or network listener. It is not an exclusive
database lock across independent applications or login sessions. GTK3 is the initial
Linux dialog backend; native Wayland/portal and packaging claims need separate
evidence. No global-shortcut, routing, store or window-state plugin is added.

`NavigationHistory` is a small UI-independent, memory-only controller, bounded to
100 destinations for one graph session: page UUID/optional block, ISO journal date,
or Tasks filters. It commits after route loading, preserves branches on failure,
replaces same-location anchors/filters, and truncates Forward only on a successful
new departure. Journal replay uses lookup, never creation. Missing block anchors
fall back to the heading; page-load failures leave the current position unchanged.
It is separate from transactional block undo/redo and never stores document text.

`PageSwitcher` filters the already-loaded titles/display labels, caps results at
100, and retains native composition and focus behavior. It has no content index,
fuzzy matcher or command execution. Application shortcuts use one small pure
classifier; modal/composition/AltGr/default-prevented/repeat guards are centralized.
History shortcuts are excluded from editable controls, including macOS Option+Arrow
text movement. The editor itself, source text and graph schema remain unchanged.
