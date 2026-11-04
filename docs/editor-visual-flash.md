# M8 editor visual-flash investigation

**Follow-up:** Native Fedora testing confirmed the broader shell flash persists
beyond this document-text correction. See the [global navigation/editor flash
investigation](global-ui-flash.md) for the global Loading and shell-control paths,
subsequent fix, current results and native retest. The measurements below remain
valid for the earlier independent BlockRow mechanism, not a full-shell resolution.

**M8 remains open pending rebuilt native Fedora confirmation.** The corrected
observation is a brief visual flash, not an established scroll jump or full reload.
The prior conditional scroll-reveal fix is unchanged. Startup findings are recorded
separately in [startup and graphs](startup-graphs.md#clarified-fedora-startup-finding).

## Code-supported cause and execution trace

The most likely cause of the broad document flash is transiently treating every
inactive block's **document text** as a natively disabled button:

1. `BlockRow.svelte` maps native Enter/Tab/Shift+Tab through `keyIntent` to
   split/indent/outdent, remembering the native selection. Composition and legacy
   keyCode 229 are excluded before dispatch.
2. `EditorController.action` sets `working=true`, notifies Svelte, flushes drafts
   and recovery capture, then sends the serialized mutation IPC. It does not
   clear blocks, change page/graph identity, or call the page-load API.
3. The Rust adapter calls `Editor::apply` under the existing session mutex. Core
   validates and commits the transaction, returning changed/removed block deltas,
   revisions, focus ID and history flags. Structural snapshots of a page in Rust
   support ordering/history correctness; they are not frontend page reloads.
4. The controller merges those deltas, retaining untouched block DTOs, reveals
   acknowledged focus, then clears `working`. Svelte derives the row presentation
   again; the `{#each ... (row.block.id)}` keys retain row component/DOM identity.
   That legitimate update is not itself a defect.
5. Before this correction, `working` propagated as `busy` to every block-content
   button's native `disabled` attribute. `foundation.css` applies
   `button:disabled { opacity: .55 }`. Thus the entire inactive document faded to
   55% and back around each structural operation, independently of DOM remounting.

The application also locks genuine navigation/mutation controls during operations;
those controls still use their normal disabled presentation. The document text is
not an unavailable command result and need not become dim or disappear while its
activation is temporarily blocked.

Other inspected paths:

- `App.svelte` mounts `PageEditor` under `!handoff`; handoff is used for graph
  activation, not ordinary structural editing. Neither selected page nor session
  changes on Enter/Indent. The editor has no loading branch that empties its
  existing outline during these actions.
- Draft notifications remain scoped; acknowledged arrays invalidate derived
  search/links/task data. No graph-open or page-load IPC is issued by normal
  structural acknowledgement.
- The links effect currently clears/refetches reference/backlink data after
  acknowledged changes. This can replace that **local derived region** and is
  a separate possible source of movement on link-bearing pages, not evidence of
  an entire editor remount. It was not changed speculatively in this fix.
- Active-row textarea/button exchange on split is expected: a new block becomes
  active. Indent/outdent retain the active textarea and apply the existing focus
  serial, selection restoration and conditional reveal. Resizing/layout for the
  changed row is expected; no relevant animation/transition was found.
- A compositor-specific paint problem is not established. A remaining Fedora-only
  artifact is not ruled out by JavaScript or DOM evidence.

## Localized production correction

Only two production files change:

- `BlockRow.svelte`: document-text buttons use `aria-disabled={busy}` and an
  explicit `if (!busy)` click guard instead of native `disabled`. Mouse and native
  Enter/Space button activation use that same guarded click path. Text remains
  focusable/readable while activation is unavailable.
- `PageEditor.svelte`: the outline exposes `aria-busy` during the operation.

No CSS opacity override, rendering suppression, artificial delay, timer, new store,
controller/IPC change, optimistic mutation or reload bypass was introduced. Active
textareas remain read-only and actual mutation controls remain disabled while busy.
Rust authority, draft/recovery flow, ordering, history and scroll code are unchanged.

## Executed verification (Ubuntu agent-lab)

A temporary compiled-Svelte fixture mounted the actual `PageEditor`/`BlockRow` with
shared CSS in ephemeral WebKitGTK 2.52.6 under authenticated Xvfb. A fixture transport
held mutation acknowledgements behind an explicit promise gate, allowing observation
of pending DOM state; this is **not** real Rust/Tauri SQLite integration or native
keyboard-event injection. Synthetic Enter/Tab/Shift+Tab events exercised the actual
row handlers. With both 3 and 5,000 initial rows, before and after the correction:

| Check | Before | After |
| --- | --- | --- |
| Untouched text opacity, idle → pending → idle | 1 → .55 → 1 | 1 → 1 → 1 |
| Outline/unrelated row DOM identity | retained | retained |
| Pending content/row count | retained | retained |
| Exactly one new row for split | yes | yes |
| Same textarea for indent/outdent | yes | yes |
| Backward selection 2..4 after indent/outdent | retained | retained |
| Active textarea focus after acknowledgement | retained | retained |
| Click on busy inactive text changes active block | no | no |

The probe was removed, and its temporary loopback Vite server stopped before commit.
No diagnostic hooks remain in production. Xvfb reported missing accessibility-bus
and hardware-render-device/DRI warnings; no sandbox, permission or system changes
were made. Single dev-mode probe durations include scheduling/layout/probe overhead
and are not a native performance benchmark.

Permanent regression tests use the existing Node test runner and Svelte/Vite
compiler (SSR, no listener or file watcher): 5,000-row markup retains all text,
marks activation unavailable without native disabled dimming, and keeps the active
input read-only during IO. A 5,000-block controller fixture checks pending-array/
row preservation, exactly one split ID, indent/outdent depth/order, selection,
untouched DTO identity and no extra page loads/concurrent writes.

Final checks passed: `npm run check` (zero errors/warnings), `npm run build`,
`npm run test:editor` (**73 tests**, including existing undo/redo, composition,
draft/recovery and failed-save regressions), and `npm run test:schema` (**29 tests**).
Both existing synthetic performance scripts ran at 200/1,000/5,000 blocks; scoped
40-draft samples still made zero row passes. These are not native latency guarantees.
The initial SSR test emitted file-watcher-limit warnings; its final configuration
turns watching off, and the final suite passed without those warnings. No OS limits
were altered. Cargo remains unavailable in this VM: Rust tests and a rebuilt Tauri
binary are not verified here.

## Exact native Fedora retest

In a clean checkout of the delivered SHA, with Node 24, Rust 1.99.1 and the approved
[native prerequisites](platforms/linux.md), run:

```sh
git rev-parse HEAD
git status --short
npm ci --ignore-scripts
npm run check
npm run test:editor
npm run test:schema
cargo test --locked --workspace
cargo test --locked --workspace --features tauri/custom-protocol
npm run tauri -- build --no-bundle -- --locked
sha256sum target/release/graphdesk-desktop
```

Close all existing GraphDesk instances **normally** first. In a fresh terminal on
the native Fedora desktop (not Xvfb), from that checkout:

```sh
binary=$(realpath target/release/graphdesk-desktop)
base=$(mktemp -d -t graphdesk-m8-flash-XXXXXXXX)
export XDG_CONFIG_HOME="$base/config" XDG_DATA_HOME="$base/data" XDG_CACHE_HOME="$base/cache"
mkdir -m 700 -p "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME"
printf 'Disposable retest directory: %s\n' "$base"
"$binary"
```

1. Fresh install: confirm both graph-history settings are enabled, with exactly
   one open Demo. Close/restart with `"$binary"`: same graph, no duplicated sample
   content. Turn reopening off, restart, and confirm it remains off. Re-enable it,
   restart, and confirm the last graph returns. Use only this disposable profile;
   do not reset a personal Fedora profile to make it appear fresh.
2. Native Create `$base/flash.sqlite3`; create page `M8 Flash`. Enter three blocks
   with distinct mixed English/Persian content. On the second block, split in the
   middle: one new correctly ordered block, correct prefix/suffix, focus and caret
   at the new block. Watch **untouched document text**, separately from toolbar/
   sidebar control feedback. No broad fade/blanking should occur.
3. Select a range in that block. Tab/Shift+Tab must indent/outdent with correct
   parent/order and retained range/caret. Undo/redo each change and verify exact
   text/structure. Repeat rapid input, task status/reordering, Light/Dark and
   Persian RTL; test real IME candidate commit/cancel without structural dispatch
   during preedit. Close/reopen normally and verify the saved result.
4. In a second terminal, hold a writer lock on **only** the disposable graph:

   ```sh
   python3 - "${base:?Set base to the printed disposable directory}/flash.sqlite3" <<'PY'
   import sqlite3, sys
   db = sqlite3.connect(sys.argv[1])
   db.execute('BEGIN IMMEDIATE')
   input('Disposable lock held. Press Enter to roll back and release: ')
   db.rollback()
   db.close()
   PY
   ```

   Set `base` in that terminal to the previously printed disposable directory.
   Type finalized text and attempt a structural edit: existing content must stay
   visible, failed-save text must survive, and graph departure must remain blocked.
   Release the lock, use Retry, and verify exact text, history and actionable
   recovery comparison. Do not discard unresolved recovery merely to clear a notice.
5. Repeat on a disposable 1,000-row then 5,000-row single-page Core fixture using
   [the existing generator/checklist](large-graph-benchmark.md#single-page-sibling-workload-new).
   Check top/middle/bottom, scrolling, input-to-Saved responsiveness and preserved
   focus/selection. Stop before 5,000 on severe 1,000-row trouble. Compare under the
   same native backend/scale/geometry, not against dev-mode Xvfb timings.
6. With the disposable app closed normally, inspect only its graph:

   ```sh
   sqlite3 -readonly "$base/flash.sqlite3" 'PRAGMA integrity_check; PRAGMA foreign_key_check; SELECT id,parent_id,position,content FROM blocks ORDER BY page_id,parent_id,position;'
   ```

Record commit/binary hash, Fedora/GTK/WebKit versions, actual Wayland/XWayland,
scale, pass/fail/not-tested per step, and whether the flash concerns document text,
links, controls or the entire canvas. If a flash remains, retain a short local clip
using synthetic content and compare link-free versus link-bearing pages. Do not
infer a reload/remount or modify scrolling/compositing without new evidence.
Native visual acceptance, real IME, assistive technology and full Rust integration
remain pending. **Do not mark M8 complete from these Ubuntu results.**
