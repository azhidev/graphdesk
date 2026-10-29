# Single-page input-path optimization (local evidence)

The single-page Fedora measurements at the prior commit were 338.9 MiB PSS
for 1,000 blocks and 650.5 MiB for 5,000 blocks, with noticeable typing lag
at 5,000. They are **before-only** native observations; no Fedora result exists
for this change.

The controller now indexes acknowledged blocks by ID on first lookup after each
block-array replacement. The ordered block array and draft map remain authoritative;
the index is rebuilt after save, page reload, graph reset, and structural edits,
not on individual keystrokes. Search, links and task refreshes now depend on the
acknowledged array's identity plus page/session/filter state rather than serializing
the entire array on every editor revision. A draft does not refresh these
acknowledged-only views; a successful save replaces the array and does. Pending
link responses are still cancelled when a dependency changes. Native textareas
and keyed block rows are unchanged.

Synthetic baseline captured **before edits** and repeat after edits in this
Ubuntu VM (Node 24.21.0):

| Blocks | Before `text` pass | After `text` pass | Before JSON key | After identity check |
| ---: | ---: | ---: | ---: | ---: |
| 200 | 0.32 ms | 0.02–0.07 ms | 0.09 ms | <0.01 ms |
| 1,000 | 5.58 ms | 0.06–0.08 ms | 0.70 ms | <0.01 ms |
| 5,000 | 71.35 ms | 0.86–1.61 ms | 3.00 ms | <0.01 ms |

The baseline `current_text_pass_ms` values and post-change values are from
`node tests/perf/editor_input.mjs`; numbers vary with JIT and machine load.
The script also runs the old linear lookup and JSON serialization against
the same synthetic blocks on each invocation. Twelve warmed passes are averaged
per size; it models synchronous row text lookup, **not** Svelte DOM scheduling,
WebKit layout, actual keystrokes, IPC, or Fedora. The full 5,000-node DOM and
textarea resize/reflow remain potential bottlenecks. No virtualization was added.

## Manual Fedora release comparison

Use disposable **copies** of the same single-page benchmark files for the
before and after release builds, not personal or sole source graphs. Build
the changed source on Fedora with the existing pinned/locked release workflow.
Use the same GNOME session/backend, display scale, language, theme, window size,
graph/page and ambient conditions for both builds. Start with 1,000 blocks;
stop if it is severely impaired before using 5,000.

1. Launch with no existing instance from the Fedora checkout in one terminal:
   `./target/release/graphdesk-desktop & app_pid=$!`. Open the disposable graph
   and explicitly select its benchmark page. Time graph open and page display
   separately (three trials each, note warm versus first). The app may add
   today's journal; the benchmark page still contains 1,000/5,000 blocks.
2. Before typing, collect 30-second idle process-tree data with
   `python3 tests/perf/linux_process.py --pid "$app_pid" --seconds 30`.
   Verify `missing_pss == 0`; record PSS, CPU and process count. Idle CPU
   does **not** measure typing responsiveness.
3. Type a known mixed Persian/English sequence at top, middle and bottom;
   time visible response and Saved status, note stalls when scrolling.
   Observe memory during interaction separately and state the measurement
   method. Do not simulate IME with pasted text: separately test real Persian
   candidate/preedit commit and cancellation without dropping focus/selection.
4. After Saved, close normally and reopen; compare exact text and selection-
   independent block ordering, confirm search results and inline links update
   after acknowledged edits, and check `PRAGMA integrity_check` plus
   `PRAGMA foreign_key_check` on the **closed** disposable file. Check tasks
   refresh after task edits and page creation/rename. Test undo/redo and
   recovery of finalized drafts using the existing disposable recovery workflow.
5. Report each condition's repetitions and variance alongside before/after
   idle PSS, interaction responsiveness and page-load latency. Record whether
   blocks are fully mounted, partially loaded or virtualized; do not infer
   virtualization from the number of blocks visible in the viewport.

The recommended change cannot be credited with a native latency or memory
improvement until the above comparison is run. If input remains slow, profile
Svelte row updates and textarea layout in the WebView before changing rendering.
