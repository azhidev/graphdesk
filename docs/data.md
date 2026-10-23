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

No automatic graph discovery, graph registry, persistent settings, undo/collapse
state or durable unsaved-draft file exists. Re-enter the path after restart.
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

Forced termination can lose unacknowledged text; SQLite durability is not draft
recovery. After a crash, stop all instances and preserve the database and sidecars.
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
