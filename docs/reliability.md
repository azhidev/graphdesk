# Reliability hardening — Milestone 3

## Approved implementation (2026-10-24)

The user approved reliability hardening and selected **app-private storage separate
from the graph database**, superseding the graph-local-table recommendation in the
historical proposal below. Implemented boundaries and guarantees are in
[architecture](architecture.md#reliability-extension) and
[data/recovery](data.md#text-draft-recovery); actual validation is in [testing](testing.md).

- Composition is guarded synchronously and across awaited IO, including a complete
  start/end cycle during navigation. The finalized input remains on the original
  page, and preedit is never submitted as final text or recovery content.
- Text saves use expected content plus monotonic revisions. Lost/rejected or timed-out
  acknowledgements retain newer drafts. Retry compares actual persisted state and
  reconciles only an unchanged base or the matching committed revision; other changes
  require explicit side-by-side comparison and revision-guarded replacement.
- Separate per-graph SQLite recovery stores live under Tauri's app-local data
  directory, with independent IO so graph locks do not prevent capture. Finalized
  drafts target 700-ms capture; statuses distinguish capture from canonical save.
- Startup discovery, compare/recover, confirmed discard, conditional cleanup and
  14-day expiry are implemented. No structural replay, network sync, export inclusion,
  telemetry or logging of recovery text is introduced. This is not zero-loss storage.

The tests distinguish controller/transport failure injection, mock IPC, actual SQL
fixtures and production WebView/SIGKILL evidence. Real IME services, screen readers,
Wayland/hardware, Windows/macOS and power-loss behavior remain separate requirements.

## Historical review/proposal (superseded where noted)

Review date: 2026-10-24. Baseline: `4988a53` (Milestone 2 closure).
This is a proposed reliability scope, **not implemented crash recovery or new
platform validation**. Existing page/link/search/task workflows are already present;
the next milestone must not recreate them. Keep the current controller, native
textareas, typed IPC, Rust transactions and SQLite authority.

## Evidence and priorities

Milestone 2 acceptance results remain those in [testing](testing.md): 36 core + 3
IPC tests in both protocol modes, 22 Node tests, 16 schema tests, frontend checks,
release build and production WebView smoke. Those expensive checks were not rerun
for this review. Diff/staged-content review and whitespace checks passed. Builds,
databases and WebView artifacts remain ignored, not committed.

Two disposable, in-memory Node transport experiments were run against the actual
controller; neither used a database, real IPC or native IME:

| Priority | Finding | Evidence / boundary |
| --- | --- | --- |
| P0 | Composition/navigation race | Start composition, draft `س`, call `load('other')`, deliver final `سلام` via `setDraft` while `working`, then end composition before the awaited flush resumes. The final draft is ignored, preedit `س` is saved, and navigation completes with no dirty draft. Confirmed synthetic controller defect; actual native event ordering is unverified. |
| P1 | Ambiguous save acknowledgement | A transport commits text, then throws instead of acknowledging. After more typing, Retry sends the old expected content and receives a conflict. The newer draft remains in memory and is not overwritten; there is no non-discarding reconciliation UI. Confirmed injected-transport recovery gap, not observed real IPC loss. |
| P1 | Forced-termination draft loss | Drafts exist only in the controller's Map. Debounce, pending writes, composition and failed saves leave text outside acknowledged SQLite state. No durable recovery copy exists. Confirmed implementation limitation; this review did not perform a process-kill or power-failure test. |
| P1 | Storage failure coverage | Busy-writer failure, retained drafts, blocked close, retry and transactional rollback passed acceptance. Disk-full/I/O-error and interrupted-commit behavior are not established by a busy-lock test. These are validation gaps, not evidence of corruption. |
| P2 | Keyboard/focus and accessibility | Deletion cancellation and empty-page history focus defects were fixed and passed WebView acceptance. Controls have labels, status/alert regions and toolbar alternatives, but flat outline markup does not expose level/parent relationships. Real accessibility-tree/screen-reader behavior, zoom/reflow and full error-path focus remain unvalidated. |
| P2 | Native input/platforms | Persian/emoji source persistence and synthetic composition guards passed, not real IME preedit/candidate/cancel behavior. Linux WebKit Shift+Tab has a tested workaround. No newly confirmed Windows/macOS/Wayland defect; those environments remain unvalidated. |

The synthetic composition case follows `load` setting `working` before awaiting
`flush`, `setDraft` ignoring input while working, and `flush` checking composition
only after awaiting the queue. It merits a permanent regression and a small safety
fix before major features, regardless of whether a native IME reproduces it. Also
exercise close, graph switching, toolbar actions and clicking another block around
composition end; do not assume the same sequence on all WebViews.

## Proposed next milestone: reliability hardening

This uses "Milestone 3" for the next agreed scope; historical architecture sections
already use that name for the shipped page/link/search/task work.

1. **Input and recovery safety first.** Add deterministic regressions for composition
   finalization versus navigation/close/structural locks, plus dropped/delayed save
   acknowledgements. Preserve final input, prevent departure during unresolved
   composition, and provide explicit conflict reconciliation without silently
   discarding the newest draft. Do not blindly retry structural actions.
2. **A small durable text-draft recovery slice, subject to approval.** Recommend
   graph-local SQLite recovery records keyed by page/block UUID, containing base
   acknowledged text and recoverable draft text. Keep them separate from canonical
   block content/search/history; restore via explicit review, never silent replay.
   Validate graph identity, missing blocks and base-content conflicts. Recovery rows
   should be cleared only after an acknowledged save or explicit discard. A durable
   capture acknowledgement must be distinguishable from a canonical save. This is
   an architectural proposal requiring review, not a claim that SQLite can capture
   every keystroke: busy/full/unwritable storage and pre-capture process death still
   leave a loss window. No structural replay, durable undo, sync or generic journal.
3. **Focused input/accessibility checks.** Keyboard-only completion of create/edit/
   nest/move/delete/recover flows, dialog name/count/cancel and focus return,
   readable save/error/recovery state, Persian/mixed bidi/ZWNJ/emoji/multiline
   selection, and zoom/small-window usability. Fix confirmed problems only; avoid
   speculative editor rewrites or adding a complex ARIA tree keyboard model.

Defer new major product features, broad packaging work, persistent history, rich
text and performance infrastructure until the input-loss race and recovery behavior
are understood. Measure before optimizing large-outline rendering.

## Targeted validation plan (not yet executed)

| Environment | Checks / acceptance evidence |
| --- | --- |
| Node controller + Rust core | Deterministic composition event-order permutations; text-save commit-before-lost-ack plus newer typing; stale session/base text; recovery-record lifecycle and failed transactions. Assert newest text remains recoverable and canonical data is never overwritten silently. |
| Ubuntu production WebView under isolated Xvfb | Reuse authenticated, TCP-disabled Xvfb/private D-Bus with sandbox unchanged. Add focused scenarios rather than repeat every old workflow. Inspect visible status, focus and exact persisted Unicode. Use only disposable graphs and child processes owned by the fixture. |
| Ubuntu abrupt-process exit | Terminate only the fixture app at controlled pre-save, post-commit/pre-ack and recovery-capture checkpoints. Restart and compare UUID/content/hierarchy/properties/references and integrity/FK checks. Distinguish acknowledged saves, captured recovery drafts and explicitly uncaptured input. SIGKILL is not power-loss/OS-crash coverage. |
| Ubuntu storage failure | Hold a disposable graph writer lock, then unlock and recover. Inject transaction failures and test SQLite full/error conditions on bounded disposable fixtures, e.g. database-local page limits where applicable; never fill the VM disk or alter mounts/permissions globally. Verify rollback, retained latest drafts, no blind structural retry, and recovery after restart where a capture was durable. |
| Ubuntu native input / accessibility | Check installed, usable input and accessibility services first. `ibus`, `fcitx5` and `orca` were not found on PATH in this review; Xvfb/xdotool were. This is not proof no libraries/services exist. Test Persian layout input separately from composition-enabled IME input. Synthetic events and clipboard paste cannot substitute for candidate/preedit tests or a screen reader. Request approval before any required system installation/configuration. |
| Separate native environments | Windows and macOS build/WebView/input/file-lock/path behavior; real Wayland, physical display, GPU/scaling, real IME and assistive technology. Record not tested when unavailable; do not block ordinary Ubuntu development or access/change the Fedora host. |

For each new scenario record commit, toolchain/WebView, environment, fixture,
expected/observed result and sanitized artifacts. Keep tests bounded; never use
personal graphs or upload private content. Retain all existing invariant tests.

## Original approval questions (resolved by subsequent user approval)

- Approve this small reliability scope before implementation.
- Agree on durable recovery storage and privacy/lifetime: recommend graph-local
  SQLite records, recognizing that they add retained note content and cannot capture
  while that database is unwritable. A separate recovery store could survive graph
  writer locks, but introduces duplicate storage, identity and cleanup complexity.
- Agree on recovery UX: recommend explicit review/restore/copy/discard, preserving
  both versions on conflict; no silent overwrites or automatic structural replay.
- Agree on the recovery guarantee/loss window and capture cadence after measuring
  the write cost; do not promise zero loss before durable capture acknowledgement.
- Any system packages or user-wide IME/accessibility configuration require separate
  explicit approval. No such changes were made during this review.
