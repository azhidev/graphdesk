# M10.1 Habits: headless core and timezone scheduling

This document describes the M10.1 core boundary. M10.2 now supplies the separate
[minimal Habits interface](habits-ui.md); statements below about no UI/navigation
describe the historical first phase, not current shell behavior. Core storage,
recurrence, transport and timezone semantics remain unchanged.

This phase adds definitions, events, recurrence and a scoped API, **not a dashboard,
reminders, notifications, background service, streak/analytics engine or Routines**.
`graphdesk.habits` is bundled using the M9 registry/lifecycle and is disabled per
graph by default. Enabling it in the existing extension settings validates read
access only; it contributes no navigation/UI and creates no habits/events. No
third-party execution or public SDK is added. The interface is internal API v1.

## Model and time decisions

- Positive habits require a schedule and accept **completion** events only.
- Negative habits have no schedule/due dates/reminders. Events default to **lapse**;
  optional **confirmation** explicitly describes a completed half-open period
  `[period_start,period_end)`. An unreported period is unknown, not successful.
- Immutable UUID event payloads record occurrence time plus a server recording
  time. Caller-generated IDs must be retained/reused across uncertain replies.
  Event history is not stored in M9's 16 KiB JSON document.
- Instants are **integer Unix seconds UTC**, supported Gregorian years 1..9999
  (both resolved UTC and local calendar boundaries must fit). Clients convert any
  offset-bearing event/anchor timestamps to instants before dispatch. Future events are
  rejected at record time. Core calculations receive explicit `now`; no hidden
  wall clock is consulted by recurrence.
- **Hours** are elapsed UTC intervals of 3,600s, even across DST. **Days, weeks,
  months and years** advance the local Gregorian calendar in the persisted IANA
  timezone, preserving local clock time. Daily intervals can be 23 or 25 elapsed
  hours; weeks are seven local days, not necessarily 604,800s.
- Months/years use local **Gregorian** boundaries. Each fixed occurrence is computed
  directly from the original anchor; an invalid day clamps to month end. Jan 31 →
  Feb 28 → Mar 31, and leap-day yearly anchors recover Feb 29 in leap years.
  After Completion clamps from that completion's actual date instead.
- Ambiguous local times choose the **earlier UTC instant**. Nonexistent local
  times shift **forward by the exact gap**: New York 02:30 becomes 03:30 in a
  one-hour gap; Lord Howe 02:15 becomes 02:45 in a half-hour gap. Later fixed
  occurrences return to the anchor's intended clock time. An entirely skipped
  date follows the same rule; coincident boundaries collapse into one due window.
  An explicit UTC anchor, including the later side of a fold, is preserved exactly.
- Gregorian/Jalali is presentation only. Selecting Jalali in GraphDesk does not
  turn a stored Gregorian month interval into a Jalali month or change event IDs.
  Jalali-calendar recurrence itself is not implemented.

### Defaults, persistence and compatibility

Settings → **Habit timezone** accepts an IANA override; blank means **System**.
Rust resolves the system's named IANA zone, not just its current offset. If named
zone detection/lookup fails, implicit-zone creation fails visibly: choose a valid
override such as UTC rather than silently guessing. Settings shows the resolved
default; invalid input cannot replace the previous atomically saved preference.

New creates may omit/null `schedule.timezone`: the adapter snapshots the resolved
default. Explicit IANA overrides are accepted. Subsequent Settings/device changes
affect **new habits only**. Omitted timezone on an update preserves the habit's
zone; an explicit zone change requires a new revision. Create retries reuse the
saved zone even if Settings changed or became unavailable after the first write.
An explicit zone edit keeps the anchor's UTC instant unless the caller also edits
the anchor; callers must set both deliberately to request a different local clock.

**Migration 007 is unchanged; no new schema migration is needed.** Timezone is an
additive definition-JSON field. Pre-timezone JSON missing/null timezone means
**UTC**, never the current default. Reads expose UTC without rewriting stored JSON,
historical revisions, events or audits. New definitions/revisions persist explicit
zones. Direct core creates without an adapter default also use UTC. Thus old
UTC schedules retain their previous semantics, including day/week intervals.

Jiff 0.2.38 supplies IANA lookup, system detection and gap/fold resolution. It was
already locked/cached; this change adds a direct core dependency on that package,
not another timezone implementation. It uses the OS timezone database and Jiff's
platform fallback where applicable. Calculations are deterministic for the saved
definition, completion, now and timezone rules in use. OS/tzdb updates can change
future resolved instants when authorities change rules; previously recorded UTC
instants are never recomputed. No tzdb snapshot or due-row materialization is stored.

## Recurrence semantics

A schedule has `anchor`, `every` (1..10000), `unit` (hours/days/weeks/months/years),
and `mode` (`fixed`, default, or `after_completion`). The anchor is the first due
instant, not a last-completed time. Completion before that anchor is invalid.

**Fixed:** evaluate only the current anchor window `[occurrence,next_occurrence)`.
It is due if no nonvoided completion belongs to that window. A late completion
satisfies the current window and never changes the anchor. A completion exactly
on a boundary belongs to the new window. No backlog tasks or occurrence rows are
generated; missed windows remain derivable from retained definitions and events.

**After Completion:** before any completion, the anchor stays the one outstanding
deadline however late it becomes. Otherwise the latest nonvoided completion plus
one interval determines the deadline. A 14-day habit completed 60 days after its
anchor next becomes due on day 74. Backdated older events do not override a newer
completion. Voiding the latest event reveals the preceding valid cycle.

`DueState` returns one `due` boolean, `occurrence`, and `next_occurrence`. For Fixed,
`occurrence` is the current window start and `next_occurrence` is the next boundary.
For After Completion, both timestamps name the outstanding deadline, **including
when overdue**; no hypothetical next completion is invented. Before the anchor,
both are the anchor and due=false. If computing a required next boundary would
exceed year 9999, the API returns a range error, not an overflow or fake date.
No history scan is needed: hourly arithmetic is constant-time, calendar boundary
search is bounded/logarithmic, and skipped-date ties cannot generate backlog tasks.

Negative status returns the latest explicit confirmation and latest lapse (or
nulls), never a success boolean or inferred streak. A confirmation is a user
assertion, not proof that no lapse occurred in its period; overlapping reports
remain in history for later correction/analysis. Historical events are retrieved
separately rather than collapsed into a misleading clean-period result.

## Mutations and corrections

Create uses a caller-generated habit UUID; updates use `expected_revision` and
retain definition versions and UTC edit audit times. Events also retain the
definition revision known at recording (not guessed from backdated occurrence
time). Category is immutable so an old lapse cannot become
a completion by changing the habit. Archive retains data and prevents new events;
it does not erase history. Existing event retries are identity checks, not new
events, even after a correction or archival.

Status evaluates the current definition and ignores completions before its current
anchor. Edits may therefore change the present due state without rewriting old
events. Archived definitions remain readable and their raw schedule can still be
evaluated; future active lists/reminders must explicitly exclude archived habits.
Definition versions preserve past anchors/modes; no analytics are calculated now.

Record with the **same UUID and identical payload** returns the existing event;
the same UUID with a different payload conflicts. Recording time is server-owned
and is not part of the retry payload. One mutation cannot append multiple events
because of an acknowledgement retry. Different IDs are intentionally different
events (the host cannot infer whether two user actions are accidental duplicates).

Correction uses event ID, expected revision and `voided` true/false. It never
deletes or rewrites the original payload/time; restore is explicit. To fix an
incorrect timestamp/kind, void the original and record a new ID. Correction is
not editor undo/redo. No automatic uncertain mutation replay is implemented.

## Minimal versioned protocol

The M9 API adds the explicit **`habits.core`** capability, available only to the
bundled `graphdesk.habits` identity. `api.habits.request(request)` uses copied
serializable data, not Svelte stores, SQLite connections, SQL or filesystem APIs.
The shell-only transport invokes `habits_request` with session, extension ID and
API version 1. Each request is a strict `op`-tagged union:

| Operation | Payload | Result `type` |
| --- | --- | --- |
| `create` | `input:{id,definition}` | `habit` |
| `update` | `id,expected_revision,definition` | `habit` |
| `get` | `id` | `habit` |
| `list` | `after: UUID|null,limit` | `habits` |
| `record` | `input:{id,habit_id,kind?,occurred_at,period_start,period_end}` | `event` |
| `correct` | `event_id,expected_revision,voided` | `event` |
| `events` | `habit_id,before:{occurred_at,id}|null,limit` | `events` |
| `status` | `id,now` | `status` |

Responses are `{type,data}`. Lists are keyset-paginated, not unbounded full-history
loads. Definitions use name, kind, nullable schedule and optional archived=false.
Schedules accept nullable `timezone` with the compatibility rules above. The
local-main-window `habits_timezone` Settings command returns `{system,resolved}`,
nullable when a named zone is unavailable. It neither mutates graphs nor exposes
timezone files to extensions.
Positive callers explicitly use kind=completion; omitted kind means lapse. Supply
null period fields except for negative confirmations. All expected revisions are
integer compare-and-set guards; stale edits conflict rather than overwrite.

Host revocation prevents captured APIs/late replies crossing disable or graph
switch. Rust independently enforces the active session, bundled ID, version and
enabled state under existing locks/transactions/main-window ACL. Dedicated narrow
editor forwarding preserves block history and recovery. Disabled data remains
in the graph for re-enable/restart; copies inherit it under normal graph-backup
semantics. M9's existing trusted-code/not-a-sandbox boundary still applies.

## Verification and remaining acceptance

Rust tests cover recurrence boundaries, fixed/after-completion overdue behavior,
clamping/leap years, IANA folds/gaps/half-hour/skipped-date cases, legacy UTC
compatibility, default/override snapshots, validation and extreme dates. Storage/
adapter tests cover persistence, isolation, duplicate retries, correction, CAS,
failure/rollback, enablement/session/ID/API/ACL guards and editor-history safety.
**Cargo is unavailable in this VM**, so these Rust tests must be compiled/run on
Fedora; independent SQLite and Node tests do not execute the Rust recurrence code.

After review/commit, on Fedora with pinned prerequisites:

```sh
cargo test --locked -p graph-core habits
cargo test --locked -p graphdesk-desktop habits
cargo test --locked --workspace
cargo test --locked --workspace --features tauri/custom-protocol
```

Use disposable migrated graphs/profile data for native checks:
enable Habits, create positive/negative definitions via the internal typed
contract/test harness, record events, retry the same IDs, void/restore, switch
A→B→A, disable/re-enable and close/restart. Confirm preservation of acknowledged
history, per-graph isolation, unchanged editor undo/recovery and no reminders/UI.
Do not use personal graphs or recovery to validate migrations/failure injection.
