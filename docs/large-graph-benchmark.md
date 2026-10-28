# Disposable large-graph benchmark

This developer-only workload is **not** an app feature. It creates two new SQLite
files under ignored `test-artifacts/benchmark-large-graphs/` using Rust Core
`Graph::create`, `create_page` and `create_block`. Core assigns UUIDs and append
sibling positions; no raw SQL mutations or schema changes are used. Content and
outline shapes are repeatable, but UUIDs/timestamps and database bytes need not
match across runs. Each graph has 200 blocks per page (5 or 25 pages), depths
0–4, mixed English/Persian, ZWNJ, numbers, punctuation and short/long text.
These files contain only synthetic notes; do not replace personal graphs or
recovery files with them.

From the repository root in the Ubuntu guest:

```sh
cargo test -p graph-core --example large_graph_fixture --locked
cargo run -p graph-core --example large_graph_fixture --release --locked
sha256sum test-artifacts/benchmark-large-graphs/graphdesk-{1000,5000}-blocks.sqlite3
```

The generator refuses an existing file (including on rerun); it never cleans up
an earlier run. Use a fresh disposable directory only after inspecting any
existing files. It closes the writer, checks SQLite `integrity_check` and
`foreign_key_check` on a read-only connection, confirms page/block counts, then
reopens through Core and reads every page outline. Transfer only after successful
completion and no `-wal`/`-shm` sidecars. The actual generated snapshots from the
2026-10-28 Ubuntu run are:

| File | Pages | Blocks | SHA-256 |
| --- | ---: | ---: | --- |
| `graphdesk-1000-blocks.sqlite3` | 5 | 1,000 | `da9dcf5afe59f443407c02faf3832857754db98351e652fd13714586e4a4fa0a` |
| `graphdesk-5000-blocks.sqlite3` | 25 | 5,000 | `718c24f50f48ca1222c71e860f89690dc45565fe4bd0346cbbce678912a08cb4` |

## Fedora transfer (manual, not performed by the guest)

On the **Fedora host**, with the GraphDesk application closed, use the existing
approved Incus workflow. Replace `YOUR_VM_INSTANCE_NAME` with the actual instance
name (inspect `incus list` if necessary). Refuse existing destination files:

```sh
vm=YOUR_VM_INSTANCE_NAME
dest="$HOME/Downloads/graphdesk-benchmark"
mkdir -p "$dest"
test ! -e "$dest/graphdesk-1000-blocks.sqlite3" && test ! -L "$dest/graphdesk-1000-blocks.sqlite3" && incus file pull "$vm/home/worker1/projects/graphdesk/test-artifacts/benchmark-large-graphs/graphdesk-1000-blocks.sqlite3" "$dest/graphdesk-1000-blocks.sqlite3"
test ! -e "$dest/graphdesk-5000-blocks.sqlite3" && test ! -L "$dest/graphdesk-5000-blocks.sqlite3" && incus file pull "$vm/home/worker1/projects/graphdesk/test-artifacts/benchmark-large-graphs/graphdesk-5000-blocks.sqlite3" "$dest/graphdesk-5000-blocks.sqlite3"
sha256sum "$dest/graphdesk-1000-blocks.sqlite3" "$dest/graphdesk-5000-blocks.sqlite3"
```

Compare both digests against the table **before opening**. If Incus is unavailable,
use an already approved SCP transfer from Fedora that pulls the same two guest
paths into new destination files, with the same no-overwrite and hash checks;
do not change VM networking or authentication to enable it. Do not mount the host
filesystem in the guest. These are graph files, not recovery data. An app open
may add sidecars; close normally before post-edit integrity checking or copying.

## Native release checklist (stop if 1,000 blocks are severely problematic)

Use the already-built native Fedora **release** executable and one app instance.
Record commit, Fedora/GNOME/WebKit versions, Wayland/XWayland, display scale,
window dimensions, language, theme and ambient conditions; keep these equivalent
for each graph and the existing no-graph/one-block baselines. Open each file by
its absolute path in GraphDesk; opening may create today's journal, so note the
**post-open** page count separately from the fixture's original count. Explicitly
select the benchmark page for UI tests (200 blocks per page, not 1,000/5,000
visible blocks). Note whether all 200 are fully rendered, partially loaded or
virtualized; don't infer this from total graph size.

1. Time from selecting Open until the chosen page is displayed and responsive;
   record repeated trials and how timing was taken. Record page load separately
   from initial graph opening if necessary.
2. With the window idle for 30 seconds, use
   `python3 tests/perf/linux_process.py --pid "$app_pid" --seconds 30`
   from the Fedora source checkout (launch
   `./target/release/graphdesk-desktop & app_pid=$!` in the **same terminal**,
   with no prior instance). Confirm PID, `missing_pss == 0`, process count,
   process-tree PSS KiB and one-core CPU %. Keep the window idle during sampling.
3. On the same page, record smoothness/stalls for scrolling, expanding/collapsing
   branches, editing mixed-script text and Saved acknowledgement time. Use
   approximate repeated timings where possible; distinguish observer impressions.
4. Switch between the 1,000-block graph and another disposable graph, then remeasure
   idle PSS/CPU; record whether memory returns, rises or remains stable. Close
   normally and reopen edited graph; check exact saved text and run SQLite
   `PRAGMA integrity_check` and `PRAGMA foreign_key_check` **after closing**.
5. Only if the 1,000-block workload is usable, repeat under matching conditions
   for 5,000 blocks. If severe lag, crash, save loss or runaway memory occurs,
   stop and report without trying 5,000 in the UI.

For closed disposable files on Fedora, run:

```sh
sqlite3 "$file" 'PRAGMA integrity_check; PRAGMA foreign_key_check; SELECT count(*) FROM pages; SELECT count(*) FROM blocks;'
```

This should print `ok`, no foreign-key rows, and counts (the page count may increase
after today's journal is opened). Never delete live WAL sidecars to force a check.

## Report template

| Build / environment | Graph / page size | Open + page-display time (method) | 30s PSS KiB / CPU % / processes / missing_pss | Scroll / collapse / edit / Saved | After-switch memory | Reopen + integrity / FK | Rendering mode / notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| commit, Fedora, WebKit, backend, scale, window, language, theme | fixture blocks, pages, visible page blocks | seconds, repetitions | sampler JSON summary | observed stalls and timings | before/after PSS | pass/fail | full/partial/virtualized, issues |

Report failures with a minimal reproduction, expected versus actual, sanitized
error output and whether 5,000 was skipped. Do not attach personal data.
