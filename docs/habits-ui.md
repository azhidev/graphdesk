# M10.2 minimal Habits interface

Enable **Habits** in Settings → Extensions for each graph. Navigation appears only
after activation; disabling removes navigation, not definitions or history. Existing
enablement choices are unchanged. No migrations, dependencies, reminders, services,
analytics, editor-store access or new IPC commands are introduced.

## Use

- **Due and negative** shows due positive habits and active negative habits.
  **All active** includes future/satisfied positive habits; **Archived** retains
  access to archived definitions/history. Lists use 30-row keyset pages; filters
  apply to the displayed page, not a background scan of the graph. First page /
  Refresh starts over. There are no accumulated missed-occurrence tasks.
- **New habit** chooses positive/negative. Positive schedules default to Fixed,
  snapshot the adapter-resolved Settings/System IANA zone, and show interval/mode
  and first-occurrence previews before save. Negative definitions have no schedule,
  due status or reminders. Kind is immutable. Edit/Archive are in habit details.
- Timestamp input deliberately requires a **Gregorian ISO timestamp with `Z` or
  explicit offset**, e.g. `2026-11-09T09:00:00+03:30`. Unzoned civil times are not
  accepted or interpreted in the machine zone. Previews use the selected display
  calendar/language and named schedule zone. Monthly/yearly recurrence is explicitly
  Gregorian, not Jalali. This minimal UI does not add a zoned civil-time picker.
- Offset-bearing timestamps identify explicit instants: either fold side is
  preserved, including the later side. An offset inconsistent with the IANA zone
  can produce a different local clock in the preview: review it before saving.
  No frontend gap/fold guessing is performed. Core recurrence still uses its
  earlier-fold/exact-gap policy. Changing the zone preserves the UTC anchor;
  change the timestamp too if a different local clock is intended. Existing zone
  snapshots are unaffected by later Settings changes.
- **Complete** records positive completions. **Record lapse** defaults to lapse;
  negative events can instead explicitly confirm a completed half-open period.
  Event time initially means now; past entries are deliberate, future entries fail.
  Positive event previews/history use the schedule zone; negative events and
  confirmation periods explicitly use UTC. Missing reports are unknown, not success.
- Details show 20 events per keyset page, with Latest/Older controls. **Void** and
  **Restore** change only correction state under the displayed revision. To correct
  time/kind, explicitly void the original and record a replacement; this is not an
  atomic compound action and original payloads remain retained. Archived habits
  retain history, allow correction/restoration, and reject new events.

## Boundaries and recovery

`src/lib/habits/` owns the renderer, forms, offset adapter and controller. The
bundled definition contributes only a bounded navigation/view marker through M9.
The generic view contract has **not** become a form/list/plugin framework. The shell
selects this renderer by the trusted bundled ID and gives it only the host's captured
`habits.core` lease. It receives no raw invoke, SQL, filesystem or editor handles.
The host and Rust retain session/enablement/capability protections. The only settings
read in the host-owned create form is the existing resolved-timezone command.

One operation runs at a time; displayed mutations require acknowledgement. A ten
second response timeout does not cancel a SQLite operation. Failed/uncertain
mutations retain the original copied request, IDs and expected revisions. **Retry
same mutation** is explicit: create/record use existing identity checks; update and
correction retries retain CAS guards and may conflict after a lost acknowledgement.
There is no automatic replay or overwrite. **Stop retrying and reload (discard
form)** explicitly abandons that retry/input and reads current definitions/history;
it does not roll back a possibly committed operation. Review before another edit.

Open forms, composition, in-flight work and uncertain mutations block normal
navigation/graph switch/close. Escape/Cancel offers explicit discard or continued
editing and restores focus. Input remains mounted on errors; submission is disabled
while a mutation needs resolution. Forms are **not private text recovery drafts**;
forced termination can lose unsaved form input and retained retry IDs. Do not force
quit to resolve an uncertain outcome. Existing editor departure/save guards remain
in force; the main editor is not remounted for this feature.

There is one 60-second refresh timer for at most 30 displayed positive statuses,
paused for hidden documents/forms/disabled controls. No per-row polling, history
scanning or definition refresh on keystrokes occurs. Disposal rejects late replies
and clears graph-owned state; disable/re-enable receives a fresh lease/controller.
Client Intl and backend timezone databases can differ; Rust remains authoritative
for persisted recurrence. Native verification is still required.

## Evidence and disposable Fedora acceptance

Historical operator report: M10.1 passed 21 graph-core and four desktop Habits tests,
existing `graphdesk test` checks and a basic two-graph/Tally/restart trial. These
are not evidence for this UI revision. Current agent checks are in [testing](testing.md).
Synthetic IPC/WebKitGTK and SQLite fixtures are not native Fedora/Wayland/IME or
Rust production-protocol acceptance. No updated native binary has been built here.

On Fedora, use pinned prerequisites and a fresh disposable XDG profile/two graphs:

1. Run frontend check/build/editor/schema tests, locked Habits core/desktop tests,
   then both locked workspace configurations (including `tauri/custom-protocol`)
   and rebuild Tauri. Do not install/repair tooling as part of this checklist.
2. Enable Habits in A. Create Fixed and After Completion positives and a negative.
   Check resolved zone, offset preview, past/future validation, Gregorian/Jalali,
   monthly disclosure, Light/Dark, RTL, narrow layout and real keyboard/IME/focus.
3. Complete a positive; record lapse/confirmation with UTC period boundaries.
   Void/restore, then void-and-replace an incorrect event. Archive: no new events;
   unarchive: history retained. Browse more than 20 events/30 habits with cursors.
4. Verify Fixed only exposes the current window after missed intervals; After
   Completion follows the latest nonvoid completion, including a late completion.
   Leave open across a due boundary and background/foreground it. Negatives must
   never acquire deadlines, overdue styling or reminders.
5. Try dirty-form departure/normal close, Escape/discard/focus return and a
   disposable writer-lock error. Retain input, avoid duplicate submissions, release
   the lock and explicitly retry/reload. Never inject failure into personal data.
6. A→B→A: B starts independently disabled, no A data; enabling B starts empty.
   Disable/re-enable A: data retained. Close normally/restart and verify exact
   definitions, zones, revisions, events, correction state and per-graph enablement.
   Verify existing editor save/undo/recovery guards. Inspect closed disposable
   graphs with SQLite integrity/foreign-key checks using normal backup precautions.

Record source/binary hashes, Fedora/WebKit/compositor details and pass/fail/not-tested.
Native acceptance remains open until this rebuilt revision passes those steps.
