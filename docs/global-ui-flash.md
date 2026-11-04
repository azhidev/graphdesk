# M8 global navigation/editor flash

**M8 remains open pending rebuilt Fedora 44 GNOME/Wayland confirmation.** This
investigation follows native evidence that the global flash persists after
`8944755`. That earlier BlockRow correction remains intact. No startup, registry,
Demo, Rust/SQLite, editor-controller or scroll-restoration behavior changes here.

## Triggering paths and evidence

Two independent global visual mechanisms were reproduced in the current compiled
**full App**, not an old embedded executable:

1. `select(page)` calls `run(() => navigate(...))`. `run` previously defaulted its
   label to `Loading…`, set application `busy=true`, and assigned `operation`.
   `journal(date)` used the same wrapper with `Opening journal…`. The shared
   `.shell-notices` rendered a new `<p role="status">` immediately below the
   header whenever `operation` was nonempty. Its normal flow height/margins
   temporarily reduced/moved the entire workspace. In the 1300×1000 fixture,
   workspace top moved **37 px** during each navigation. This is the source of
   the recorded Loading label, not a page-loading overlay or reload.
2. `locked = busy || editor.working` fed native `disabled` on Header/Sidebar
   navigation controls, including the current graph and every Page entry.
   `foundation.css` applies `button:disabled { opacity: .55 }`. Thus ordinary
   page-local operations dimmed unrelated shell navigation. Enter/Indent/Outdent
   call `EditorController.action`, not `run`: they toggled `editor.working` and
   dimmed the same shell without inserting the global label. This remains distinct
   from the inactive **document text** issue already fixed in `8944755`.

Page loading remains serialized: draft/recovery flush → destination resolution
(`editor_find_journal` where applicable) → `editor.load`/`editor_page` → install
acknowledged blocks → `rememberPage`/history commit → focus heading. The source
page, selection indication and blocks remain installed until the load succeeds.
A missing journal produces the intended date-only preview only after resolution.
Structural editing still receives Rust deltas and merges them without page/graph
reloads. Row keys remain block UUIDs. App graph `handoff` is not entered during
normal Page/Journal/structural actions.

### Focused runtime observations

A temporary fixture mounted the actual `App.svelte`, shared CSS and existing IPC
wrappers in ephemeral WebKitGTK 2.52.6 under authenticated Xvfb. Tauri's existing
`mockIPC`/`mockWindows` helpers supplied synthetic data and promise-gated replies;
no real graphs/recovery stores were read. Programmatic clicks and keyboard events
exercised real frontend handlers. This is compiled Svelte/WebKit DOM evidence,
**not Rust IPC integration, native physical input, GPU/compositor tracing or Fedora
visual acceptance**.

| Pending operation | Before: shell label / graph-button opacity / workspace shift | After |
| --- | --- | --- |
| Page A, Page B, return Page A | Loading… / .55 / 37 px | no shell label / 1 / 0 px |
| Journal and absent journal | Opening journal… / .55 / 37 px | no shell label / 1 / 0 px |
| Enter/split, indent, outdent | none / .55 / 0 px | none / 1 / 0 px |
| Injected failed Page read | Loading… / .55 / 37 px | no shell label / 1 / 0 px |

Header, Sidebar and pending outline nodes retained identity/connectivity before
and after. Therefore the reproduced global transition is **layout and native
button-state styling**, not an unnecessary shell remount. The corrected frontend
also passed with **5,000 blocks per fixture page**: primary/Page/Header navigation
opacity remained 1; local Loading/Opening journal feedback remained visible; busy
Page/Graph clicks dispatched no extra IPC; failures retained the source and showed
an error. Enter added one row; indent/outdent retained the active textarea, range
and focus. Blank preview and new-page rows replaced old data only on acknowledged
navigation. These observations do not prove a Fedora compositor artifact impossible.

Temporary diagnostics were removed and the loopback server stopped. Expected
Xvfb DRI/render-device and accessibility-bus warnings were not bypassed.

## Small production correction

- `run` defaults to **content-local feedback**. Explicit graph opening/native-picker
  operations retain shell-level messages. `OperationStatus.svelte` reserves one
  28 px content feedback slot, so ordinary status insertion cannot change shell
  geometry or repeatedly move the document. It does not clear/hide existing data.
- `busyControl` is a small Svelte action for transient navigation unavailability.
  It sets `aria-disabled` and capture-blocks clicks while busy, without invoking
  native disabled dimming/focus removal on unrelated navigation. Native mouse and
  Enter/Space button activation share that guarded click path. Existing handler,
  operation, controller, composition and graph-handoff guards remain in force.
- Intrinsic unavailability (no history destination or calendar boundary) still uses
  native `disabled`. `NavigationHistory.hasBack/hasForward` describes destination
  availability independently of transient pending state; `canBack/canForward` and
  `perform` still enforce serialization. This avoids repainting intrinsic history
  availability merely because a request began.
- The notes region exposes `aria-busy`. Its local status is a sibling outside that
  busy region, so accessibility feedback is not deferred by its own busy ancestor.
  Actual mutation controls and editor readonly locks remain unchanged.

Rapid requests during a pending operation are **blocked, not queued or optimistically
selected**, matching the existing policy. A later accepted click starts only after
completion; no new concurrent requests or stale-response races were introduced.
Actual error/recovery notices stay prominent in shell notices; legitimate errors
may occupy space. There are no opacity overrides, artificial delays, render
suppression, new dependencies, general state framework or mutation optimizations.

## Executed checks

- `npm run check`: PASS, zero errors/warnings.
- `npm run build`: PASS.
- `npm run test:editor`: **77 PASS, 0 FAIL**. Four new cases cover App scope/action
  wiring, guarded click/update/destroy behavior, intrinsic history availability
  during serialized/failing loads, and rendered local versus shell feedback.
  Existing draft, IME/composition, recovery, undo/redo, graph handoff and 5,000-block
  delta/markup cases remain green.
- `npm run test:schema`: **29 PASS, 0 FAIL** (Python SQLite/fixture helpers, not Rust).
- `node tests/perf/editor_revision.mjs` and `node tests/perf/editor_input.mjs`: PASS
  at 200/1,000/5,000 rows. Scoped 40-draft runs still make zero row passes. Synthetic
  timings are not WebView latency/paint or Fedora performance guarantees.
- Temporary full-App WebKitGTK probes: baseline at 3 rows; corrected frontend at
  3 and 5,000 rows; enhanced final 5,000-row checks included all primary/Page/Header
  navigation opacities and local feedback. All fixture assertions passed.

Cargo remains unavailable in the established VM environment. No Rust tests,
rebuilt native Tauri executable, Fedora release, real IME or assistive-technology
acceptance is claimed. Node fake-event/SSR/source-contract tests are distinguished
from the separate actual WebKit DOM fixture above.

## Exact source transfer and Fedora build

Use the source bundle supplied with this change, **not** the Ubuntu executable.
On Fedora only, through the existing approved transfer mechanism:

```sh
vm=YOUR_VM_INSTANCE_NAME
bundle="$HOME/Downloads/graphdesk-m8-global-flash.bundle"
test ! -e "$bundle" && test ! -L "$bundle" && incus file pull "$vm/home/worker1/projects/graphdesk/test-artifacts/m8-global-flash/graphdesk-m8-global-flash.bundle" "$bundle"
sha256sum "$bundle"
```

Compare that digest with the developer's handoff. If Incus is unavailable, use an
already approved offline transfer of this single bundle; do not alter networking,
authentication or mount the host. Choose a **new** checkout and verify its full
HEAD matches the supplied SHA before building:

```sh
checkout="$HOME/graphdesk-fedora-m8-global-flash"
test ! -e "$checkout" && test ! -L "$checkout" && git clone --branch main --no-local "$bundle" "$checkout"
git -C "$checkout" bundle verify "$bundle"
git -C "$checkout" rev-parse HEAD
git -C "$checkout" status --short
cd "$checkout"
node --version
cargo --version
npm ci --ignore-scripts
npm run check
npm run test:editor
npm run test:schema
cargo test --locked --workspace
cargo test --locked --workspace --features tauri/custom-protocol
npm run tauri -- build --no-bundle -- --locked
file target/release/graphdesk-desktop
ldd target/release/graphdesk-desktop
sha256sum target/release/graphdesk-desktop
```

Stop on any failed command, mismatched commit/digest or missing libraries. Use Node
24, pinned Rust 1.99.1 and the approved [Fedora prerequisites](platforms/linux.md).
For an RPM, use the separately reviewed [opt-in package build](distribution.md#linux-alpha-build-and-validation).
Do not install the Ubuntu build on Fedora or weaken sandbox/SELinux settings.

## Native retest: isolated disposable profile

Close every existing GraphDesk instance **normally** first; otherwise single-instance
forwarding may select an unrelated process. In the native Fedora desktop terminal:

```sh
binary=$(realpath target/release/graphdesk-desktop)
base=$(mktemp -d -t graphdesk-m8-global-flash-XXXXXXXX)
export XDG_CONFIG_HOME="$base/config" XDG_DATA_HOME="$base/data" XDG_CACHE_HOME="$base/cache"
mkdir -m 700 -p "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME"
printf 'Disposable acceptance directory: %s\n' "$base"
"$binary"
```

1. Create `$base/native-flash.sqlite3` through the native dialog. Create Pages A
   and B, each with at least three distinct English/Persian blocks. Repeatedly
   switch A → B → A, then browse existing and absent journal dates and use Today.
   Header and Sidebar must not dim, move or disappear. Ordinary navigation must
   not insert global Loading under the header; longer loads may show only the
   reserved content-local feedback. Active-page indication changes on success.
2. Click rapidly between destinations and use Back/Forward. Pending controls
   announce unavailability and ignore activation until complete. The final display
   must match the last **accepted** navigation; no stale data may replace it later.
3. On the second block, select a range and press Enter: exactly one correctly
   ordered new block, proper prefix/suffix and new-block caret. Indent/outdent,
   undo/redo, type rapidly and navigate away immediately. Text/structure must
   survive, existing row focus/selection must remain correct where applicable,
   and shell opacity/geometry must stay stable. Test Light/Dark, English/Persian
   RTL, real IME candidate commit/cancel and native keyboard shortcuts separately.
4. In a second terminal set `base` to the printed disposable directory, then hold
   a writer lock on **only** this disposable graph:

   ```sh
   python3 - "${base:?Set base to the disposable directory}/native-flash.sqlite3" <<'PY'
   import sqlite3, sys
   db = sqlite3.connect(sys.argv[1])
   db.execute('BEGIN IMMEDIATE')
   input('Disposable writer locked; press Enter to roll back and release: ')
   db.rollback()
   db.close()
   PY
   ```

   Type finalized text and navigate/indent. A save failure must retain the source,
   draft and error feedback and block unsafe departure. Release the lock and use
   Retry. Review recovery snapshots without discarding unresolved text; verify
   exact saved content and normal close/reopen. Error/recovery notices must remain
   visible; this fix must not suppress them to achieve stable geometry.
5. Run the existing disposable Core fixture generator in the clean checkout (it
   refuses existing destinations), then open the benchmark page explicitly:

   ```sh
   cargo test -p graph-core --example large_graph_fixture --locked
   cargo run -p graph-core --example large_graph_fixture --locked -- --single-page
   ```

   Follow [the 1,000/5,000 single-page checklist](large-graph-benchmark.md#single-page-sibling-workload-new).
   Repeat navigation, Enter/Indent/Outdent and typing at top/middle/bottom with
   comparable native geometry/backend. Stop before 5,000 on severe 1,000-row
   trouble. Record actual responsiveness, not synthetic Node timing equivalence.
6. Close normally and inspect only the disposable graph:

   ```sh
   sqlite3 -readonly "$base/native-flash.sqlite3" 'PRAGMA integrity_check; PRAGMA foreign_key_check; SELECT id,page_id,parent_id,position,content FROM blocks ORDER BY page_id,parent_id,position;'
   ```

Record commit/binary hash, Fedora/GTK/WebKit versions, actual Wayland/XWayland,
scale, window dimensions, pass/fail/not-tested, and a short synthetic-only native
clip if the flash persists. Compare whether it concerns shell navigation, local
links, actual mutation buttons or the entire canvas. Do not infer remounts or
change compositor/scroll settings merely from a remaining visual symptom.

## Independent mechanisms still outside this fix

The links effect still clears/refetches reference/backlink data after an acknowledged
edit, potentially replacing that **local** region. Actual unavailable mutation
controls still use native disabled styling. Both are distinguishable from the
fixed global shell transition. Neither was broadened into a speculative editor or
link-refresh refactor. GPU/compositor-only behavior, native accessibility, real IME
and native 5,000-block responsiveness remain Fedora verification requirements.
**Do not mark M8 complete from the Ubuntu checks. Do not begin M9.**
