# M8 remaining structural-editor flicker

**Fixed a reproduced link-bearing-page mechanism; M8 is not complete.** Fedora
44 GNOME/Wayland visual acceptance remains required. This does not establish that
all remaining native flicker, especially on link-free pages, has this cause.

## Diagnosis and lifecycle

Enter/Tab/Shift+Tab in `BlockRow` remember native selection, exclude composition,
and dispatch split/indent/outdent to `EditorController.action`. The controller
locks structural input, flushes drafts/recovery, serializes mutation IPC, merges
the Rust-authoritative delta, restores acknowledged focus/selection and unlocks.
No page load or global Loading notice is requested. Rows remain keyed by UUID.

The changed acknowledged array triggers App's links effect. Previously it **set
all references and backlinks to empty before awaiting `editor_links`**. PageEditor's
keyed reference buttons were consequently removed and recreated, collapsing their
normal-flow regions throughout the editor. Separately, those buttons used native
`disabled` while working/dirty, invoking shared opacity .55. This is not the earlier
BlockRow text-opacity or shell-navigation bug: those fixes remain unchanged.

A compiled full-App/WebKitGTK probe, using synthetic promise-gated IPC and actual
row key handlers, reproduced this before production edits:

| Observation (3-row linked fixture) | Before | After |
| --- | --- | --- |
| Unrelated text opacity during mutation | 1 | 1 |
| Unrelated inline-link opacity during mutation | .55 | 1 |
| Unrelated link connected during refresh | no | yes |
| Neighbor top delta during indent/outdent refresh | -126 px | 0 px |
| Neighbor top delta during split refresh | -48 px | +36 px (new row) |
| Outline, neighboring row and text DOM identity | retained | retained |
| Active textarea identity on indent/outdent | retained | retained |
| Focus / backward selection 2..4 on moves | retained | retained |

Thus the reproduced issue involves **local DOM replacement, CSS state and layout**,
not whole-editor remounting. Active textarea height stayed 27 px in this single-line
fixture, including pending IPC; split correctly activates a new input at 0..0.
No empty-editor/placeholder interval occurred. Normal depth movement, new-row
insertion, and genuinely added/removed references remain legitimate layout changes.
There is no evidence justifying a compositor, textarea-resize or scroll workaround.

## Small correction and safety

- App retains the last acknowledged links during **same-session, same-page**
  refreshes. Page/session changes still clear them. Existing cancellation guards
  reject stale successes/errors; cancelled requests cannot unlock a newer request.
- Retained references/backlinks remain inert during refresh **and after failure**,
  until a later refresh succeeds. Errors remain visible. Another edit or navigation
  initiates a new refresh; there is no automatic failure-retry loop.
- PageEditor uses the existing capture-phase `busyControl` for inline references,
  covering working, dirty, refresh and failure states without native dimming.
  `view.dirty` tracks draft notifications explicitly. Keyboard-generated clicks
  use the same guard. Text buttons and shell guards retain their prior behavior.

Only derived link presentation changes. No controller, Rust, schema, IPC, recovery,
history, save-before-navigation or native composition code changes. Old links are
not optimistic canonical data and cannot be followed while stale. Actual mutation
controls (task/disclosure/toolbar) retain their native disabled behavior.

## Verification (Ubuntu agent-lab, not Fedora acceptance)

- `npm run check`: PASS, zero errors/warnings.
- `npm run build`: PASS.
- `npm run test:editor`: 77 PASS, zero failures. Existing cases cover serialized
  rapid structure, undo/redo, exact Persian input, composition, retained failed
  drafts/recovery, navigation, and 5,000-block delta identity/hierarchy.
- `npm run test:schema`: 29 PASS (Python SQLite, not Rust IPC).
- Both `tests/perf/editor_{revision,input}.mjs`: PASS at 200/1,000/5,000 blocks;
  40 scoped drafts still cause zero row passes. These are synthetic timings.
- Full-App WebKitGTK fixture: PASS at 3 and 5,000 initial rows (sparse links),
  including one split only, correct move parent, native DOM identity/computed styles,
  pending-input height, focus/range, blocked stale clicks, retained inert links on
  refresh failure, and page-change cancellation of an older reply.

Run the fixture sequentially, with the existing Linux WebView prerequisites:

```sh
xvfb-run -a -s '-screen 0 1300x1000x24 -nolisten tcp' dbus-run-session -- /usr/bin/python3 tests/webview/editor_flicker.py 3
xvfb-run -a -s '-screen 0 1300x1000x24 -nolisten tcp' dbus-run-session -- /usr/bin/python3 tests/webview/editor_flicker.py 5000
```

It uses an ephemeral WebKit context, synthetic graph/IPC, loopback-only Vite on
5179 and stops its server. It does not open databases or recovery data. No new
dependencies, system changes, sandbox overrides or diagnostic production hooks.
Missing accessibility-bus/DRI warnings remain. This is DOM/layout evidence, not
Rust integration, physical keyboard/IME, paint/compositor tracing or latency proof.

Verification limitations/failures: Node SSR checks passed but emitted ENOSPC file
watcher warnings; no OS limits were changed. Early probe attempts timed out from
fixture routing/serving errors, corrected in the retained runner. An initial dense
5,000-reference probe failed with WebKit `Unsupported result type (601)`; its cause
was not established and dense-link performance is **not certified**. Final large
coverage uses 5,000 rows with links in only the first three original blocks. Cargo
is not on PATH, so Rust tests and a rebuilt Tauri binary were not run here.

## Fedora acceptance after manual review and commit

Transfer the **reviewed committed source** through the existing approved mechanism
to a clean Fedora checkout; verify its SHA includes all files below. No bundle of
the current uncommitted work is supplied or implied. Use Node 24 and pinned Rust
with the existing Fedora prerequisites; run frontend checks/tests, schema tests,
`cargo test --locked --workspace`, and
`cargo test --locked --workspace --features tauri/custom-protocol`, then
`npm run tauri -- build --no-bundle -- --locked`. Stop on failure; record source
SHA and binary hash. Close other instances normally. Launch the rebuilt native
binary with **fresh disposable** XDG config/data/cache and a disposable graph as in
[the existing isolated-profile procedure](global-ui-flash.md#native-retest-isolated-disposable-profile).

1. Compare three-block pages **without links**, with `[[Target]]` in each block,
   and with task statuses. Split at a caret and across a selection: exactly one
   new row, correct prefix/suffix, caret at the new block. Unaffected text/links
   must not dim or disappear; distinguish legitimate insertion from a flash.
2. Select a backward range, Tab/Shift+Tab repeatedly, then undo/redo. Check correct
   parent/order, focus and retained range; repeat rapid operations. Repeat Light,
   Dark, Persian RTL and real IME preedit/commit/cancel; no structure during preedit.
3. Switch Page A/B, browse existing/absent Journal dates and return. No old-page
   links, global Loading flash, or Header/Sidebar regression. Check immediate
   departure after typing. Use the existing disposable writer-lock procedure to
   confirm failed saves keep drafts and block unsafe departure; release and Retry,
   then close/reopen and verify exact text/structure. Never clear personal recovery.
4. Repeat top/middle/bottom on disposable 1,000 then 5,000-block outlines using
   [the existing generator](large-graph-benchmark.md#single-page-sibling-workload-new).
   Compare responsiveness at the same scale/window/backend; stop on severe trouble.

Record Fedora/WebKit versions, actual Wayland/XWayland, scale and per-step results.
If flicker remains, record a short synthetic-only clip of linked versus link-free
pages. In a development build with the already available inspector, retain DOM
references to `.outline`, the active textarea, an unrelated `[data-block-id]`, its
text and following reference button before the operation. Compare `isConnected`,
identity, computed opacity/visibility/background, bounds, `document.activeElement`
and selection through mutation and links acknowledgements. A link-free residual
would require new evidence; do not apply remount/scroll/compositing guesses.

**Do not mark M8 complete until the rebuilt Fedora application passes.**
