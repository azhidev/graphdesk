# GraphDesk architecture (Milestones 1–3)

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
Subtree/page deletion uses FK cascades: SQLite's trigger-depth limit bounds extremely
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

SQLite is authoritative; Markdown will be an interchange format, not a second database.
Disk graphs use WAL and `synchronous=FULL`. Timestamps are UTC Unix seconds;
journal days are ISO 8601 Gregorian `YYYY-MM-DD` civil dates, not UTC instants.
Future Gregorian/Jalali presentation must convert at the UI boundary without changing
the stored identity of the day.

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
rather than exposing FTS syntax. Persian UTF-8 token search works, but stemming,
ZWNJ handling, Arabic/Persian yeh/kaf equivalence and transliteration are not promised.
Search is a rebuildable index, never an independent source of truth.

## Security and validation boundary

Only the local main window receives core capabilities and the explicit `graph-access`
application permission for legacy and editor graph commands. Other windows and remote origins
are denied by the application ACL, verified using the generated context in IPC tests.
No filesystem/shell/network
plugins or remote pages are configured. The user supplies the graph path explicitly;
there is no automatic filesystem discovery. Content is rendered as text, not HTML.
Databases are unencrypted local files and must only be opened from trusted sources;
hostile database-file hardening is outside this milestone. No plugin execution exists.
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
properties, task state and reference edges on the original UUID. Safe removal is narrower
than public subtree deletion: only empty, metadata-free root leaves with another block
remaining can be removed. No automatic nonempty-block merge exists.

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
transport, window rendering, or desktop lifecycle. Current results are in the README.

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

No known Milestone 1 blocker remains. Following administrator installation of Xvfb
and xdotool, two authenticated temporary Xvfb/private D-Bus sessions ran the existing
release executable with TCP display listening disabled and sandbox settings unchanged.
Screenshots and xdotool verified graph/page/root-block workflows through the actual UI,
including saved content after restart and a duplicate-create error preserving state.
The runtime-created graph also passed SQLite integrity and foreign-key checks.
The sessions closed normally; temporary processes and test artifacts were cleaned up.

Milestone 1 is closed for this scope. Fedora desktop smoke testing remains a recommended,
non-blocking platform check for compositor/GPU, Wayland, and accessibility behavior.
It must use a separately authorized checkout or installed build; this VM session does
not gain access to the Fedora host. Journal creation remains outside the UI; nested
editing is now validated separately by the Milestone 2 desktop checks in the README.

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
