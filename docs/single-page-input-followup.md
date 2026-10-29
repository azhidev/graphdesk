# Second single-page input pass (Ubuntu verification; Fedora pending)

Fedora observations after `b0a735c` were 561.3 MiB idle PSS, approximately
three seconds to display a 5,000-block page, smooth scrolling and delayed rapid
typing at either end of the page. The earlier 650.5 MiB result is not a controlled
memory-causality experiment. Neither result measures interactive CPU or layout.

## Mechanism and scope

Before this change, every `setDraft` called the same notification used for
acknowledged/structural changes. The resulting `PageEditor` revision updated
the entire keyed row loop, including row props and text checks for up to 5,000
blocks. `BlockRow` also called `resize()` synchronously in the input handler and
again from its `text`-dependent effect after the parent supplied the new draft.
Each resize resets the height and reads `scrollHeight`, a possible synchronous
layout barrier. These are source-level findings, **not** a Fedora WebKit flame
graph or measured forced-reflow time.

Draft notifications now update status/navigation state but not the row-loop
revision. The active row retains its finalized text in local Svelte state and
the native textarea; its acknowledgement and activation sync from the controller's
latest draft. Acknowledgements, composition transitions, focus, loading, recovery
and structural operations still refresh row props. The text-dependent resize
effect no longer runs on every draft input; the input handler still resizes once.
No virtualization, Core/SQLite changes or production profiling hooks were added.

Run `node tests/perf/editor_revision.mjs` to compare 40 draft notifications
using a synthetic 200/1,000/5,000-row loop (Node 24; median of 10 warmed batches).
Two runs on this Ubuntu VM produced:

| Blocks | Legacy page-wide model / 40 drafts | Scoped model / 40 drafts | Modeled row passes |
| ---: | ---: | ---: | ---: |
| 200 | 0.43 ms | 0.02–0.03 ms | 40 → 0 |
| 1,000 | 0.90–0.94 ms | 0.02–0.03 ms | 40 → 0 |
| 5,000 | 5.86–6.80 ms | 0.01 ms | 40 → 0 |

This is **not a Svelte/WebKit render or keystroke benchmark**: it models the
callback contract and JavaScript row-text evaluation without DOM updates,
layout, painting, or input-event dispatch. Svelte check/build, editor tests and
the Ubuntu release WebView daily/Persian fixtures verify code paths and persistence,
not actual Fedora responsiveness. Native IME candidate/preedit services were not
exercised by clipboard-based Persian checks. Character loss during rapid Fedora
typing has not been established or ruled out; late-save tests cover the controller,
not native event delivery under rendering pressure.

## Manual Fedora comparison

On Fedora, build the release app from this commit using the documented locked
native build. Use fresh disposable **copies** of the same 1,000- and 5,000-block
single-page databases for before/after builds; don't reopen the only originals
for editing. Match compositor/backend, screen, scale, theme, language, window size,
and power/activity conditions. Start at 1,000 blocks; stop before the larger page
if it becomes unsafe or unusable.

1. With no existing app instance, in the source checkout terminal run
   `./target/release/graphdesk-desktop & app_pid=$!`. Open the disposable copy,
   select its benchmark page, time graph open and page display separately (three
   trials). Record the page's fully mounted/partial/virtualized state.
2. After 30 idle seconds run
   `python3 tests/perf/linux_process.py --pid "$app_pid" --seconds 30` in the
   **same terminal**, check `missing_pss == 0`, record PSS/processes/idle CPU.
   Also capture process CPU/memory **while typing** with a recorded sampling
   method; idle CPU cannot establish interactive responsiveness.
3. Type an exact, known mixed-script string rapidly at top and bottom, record
   displayed response latency/stalls, textarea value, time to **Saved**, and
   final caret/selection. Test real Persian IME candidate/preedit commit and
   cancellation separately. Scroll and switch pages; verify drafts, focus,
   shortcuts, undo/redo and recovery are not silently lost.
4. Close normally, reopen and compare exact input, search results, links and
   tasks after their acknowledged edits. On the **closed copy**, run
   `sqlite3 "$file" 'PRAGMA integrity_check; PRAGMA foreign_key_check; SELECT count(*) FROM blocks;'`.
   Expected: `ok`, no FK rows, and the expected block count (adjust if a
   structural action intentionally changed it).
5. If the Fedora WebKit inspector is already available for a debug build,
   capture its Timeline around a single keypress and record scripting, style/
   layout and painting separately; count mounted nodes with
   `document.querySelectorAll('[data-block-id]').length`. Do not compare debug
   timings as release performance, enable external inspection listeners, or
   disable the sandbox to obtain a profile. Report unavailable tooling as such.

Remaining potential costs include the unchanged 5,000-row initial mount,
status-driven updates on saves, and the one remaining synchronous `scrollHeight`
read on input. Profile these in native WebKit before further changes.
