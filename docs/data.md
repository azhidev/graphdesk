# Data, backup and recovery

## Paths and application state

Each graph is a user-selected SQLite file, with adjacent `-wal` / `-shm` sidecars
while open. Use a native absolute path in an existing private, writable directory
on a local filesystem. Spaces and Unicode are passed unchanged through IPC to
Rust's native `Path` APIs. Windows drive paths use Windows syntax; Unix paths use
Unix syntax. The UI does not expand `~`, environment variables or shell quotes.
Relative paths resolve from the process working directory and are not recommended
because launch methods differ. Non-UTF-8 filenames cannot be entered through the
string-based UI. OS filename, path-length and permission restrictions still apply.

No automatic graph discovery, graph registry, persistent settings or durable
undo/collapse state exists. Re-enter the graph path after restart. Private text
recovery snapshots are described below; they do not replace graph backups.
There is no single-instance enforcement: use only one process per graph.

Tauri/the system WebView may create runtime state or caches in platform-managed
locations. Linux uses XDG data/cache locations, Windows uses per-user AppData,
and macOS uses per-user Library locations. Exact subpaths depend on the backend;
the app identifier is `org.graphdesk.desktop`. These are **not** graphs or backups.
Do not delete them to repair notes. Future application-owned settings should use
Tauri's `app_config_dir` / `app_data_dir` / `app_cache_dir` path resolver, not
hardcoded home paths or environment-variable concatenation. No new settings
abstraction is needed until settings exist.

## Backup and upgrade

Wait for **Saved**, close normally and verify every process using the graph has
exited, then copy to a new backup path. Never copy just a live main database;
committed writes may be in WAL. If sidecars remain, preserve the closed database
and sidecars as a set or use a reviewed SQLite-aware backup procedure. Never mix
database/sidecar versions. No built-in online backup/export exists.

Test restore by opening a separate copy and checking pages, tasks, links and
Unicode. Keep an untouched pre-upgrade backup: opening may migrate a graph and
older binaries may reject it. Binary rollback is not database rollback.
Uninstalling an application must never delete graphs or backups.

## Failure recovery and privacy

Forced termination can lose uncaptured text; SQLite durability is not a guarantee
that every intended edit reached recovery storage. After a crash, stop all instances
and preserve the database, sidecars and any private recovery files.
Try recovery on a separate working copy, not your only original. On errors, stop
writing: do not delete WAL, reset migrations or apply speculative SQL. Optional
integrity/foreign-key checks belong on the copy. Integrity does not prove every
intended edit saved.

Save errors retain drafts and block normal navigation/close. Resolve disk space,
permissions or another writer before retrying; avoid repeated force-quits. Failed
creation can leave an empty reserved file: inspect before removal.

Files are unencrypted; access follows OS permissions. Open only trusted databases;
hostile-file hardening is not implemented. Network filesystems and active file-sync
or multi-writer use are unsupported. There is no persistent application log
collector or crash reporter. Logs, screenshots and databases can contain private
notes and paths: sanitize before sharing, and never upload personal graphs or secrets.

## Text-draft recovery

Recovery uses separate per-graph SQLite files under Tauri's
`app_local_data_dir()/recovery/<graph-UUID>.sqlite3`. This is app-private local
storage, **not** a table in the authoritative graph. On Linux the resolver follows
the guest's XDG local-data location; Windows/macOS use their Tauri platform resolver.
Do not assume a hardcoded cross-platform path. New Unix directories/files use
0700/0600, including SQLite sidecars. Other platforms inherit their app-data ACLs;
native permission behavior remains to be validated. Storage is not encrypted.

Migration 4 assigns a stable graph UUID and monotonic block text revisions.
Recovery follows a renamed/moved graph with the same identity; backups/copies retain
that logical identity too, and may show the same local drafts. Always compare the
intended copy. No graph-path registry or filesystem scanning for graphs is added.
Startup inspects only the application's recovery directory, reports pending graphs,
and asks you to open the original graph before comparing. Missing blocks/pages are
not recreated; their recovery text remains selectable for manual copying.

Finalized changes target a snapshot after 700 ms (not an indefinitely sliding idle
debounce). Recovery IO has its own blocking worker/mutex/SQLite connection, separate
from graph IO, so a graph writer lock does not itself prevent capture. Identical
acknowledged drafts do not require another snapshot. Navigation and normal close
attempt capture and await the graph save/cleanup queue. Canonical saves can still
succeed when recovery storage fails; otherwise failed saves retain in-memory drafts
and block departure. Visible recovery warnings mean text is not protected; keep the
app open, copy important text, and retry. Composition/preedit is deliberately not
captured or saved until finalized. The one-second capture target assumes a responsive
event loop and healthy local recovery storage, not a hard durability deadline.

On reopening, choose **Compare / Recover** to see persisted and recovery text.
**Recover this text** explicitly replaces the displayed block text under both
expected-content and revision guards. If it changed after comparison, read/compare
again; neither version is silently overwritten. Equal text is reconciled without a
duplicate content write. A successful acknowledgement clears only exact matching
recovery content. A failed cleanup leaves the copy available again. Discarding a
recovery copy requires confirmation and rejects a stale capture rather than deleting
a newer one. **Review local recovery drafts** refreshes the list during a session.
Text-save responses time out after ten seconds; timeout does not cancel a running
SQLite operation or automatically replay it. Retry reads actual persisted text and
revision, preserves newer drafts and reconciles an already-committed save without
another content write. For a live save conflict, compare both versions before choosing to replace the
displayed persisted text with the retained local draft. No automatic structural
replay, persistent undo, or background export is performed.

Snapshots of one base/revision coalesce to its latest text; other unresolved base
versions are retained for review. They expire at age 14 days according to local
wall-clock time since capture. Listing/capture/startup performs expiry; reads do not
extend retention. No daemon runs while the app is closed, so expired bytes in unused
files remain until the next cleanup. SQLite `secure_delete` is enabled, but removal
is not a forensic secure-erasure guarantee for WAL, filesystems, backups or snapshots.

Recovery base/draft text never enters normal graph search/exports, telemetry or
application logs. There is no sync or telemetry service. OS backups or independently
configured filesystem synchronization of app-data are outside the app's control;
do not deliberately place recovery storage on a synchronized/shared filesystem.
Do not upload recovery files: they contain private note text, including unresolved
or previously deleted versions. Preserve them separately before troubleshooting.

There is still a loss window before capture, during unfinished composition, after
failed recovery writes, and on disk/full I/O, forced termination or power/hardware
failure. Only acknowledged captures are recovery-protected; **Saved** separately
means the graph write was acknowledged. This is best-effort local text recovery,
not a zero-data-loss promise or a graph backup.
