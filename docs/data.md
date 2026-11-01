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

No automatic graph discovery or durable undo/collapse state exists. Language,
calendar and search-mode preferences are remembered locally. A separate versioned
`graph-state.json` under the adapter's app-config directory stores graph paths and
expected UUIDs **only after explicit opt-in**. Both remembering and automatic startup
reopening default to off. The list is capped at ten successful activations and
contains no page titles, content, searches or navigation history. Paths themselves
can be sensitive. Forget/clear never removes a database or recovery file; disabling
remembering clears paths and last-graph metadata. Private text recovery snapshots
are described below; neither registry nor recovery replaces graph backups.
The registry does not control independent OS/native-file-dialog history or OS
backups. Clearing app metadata is not a forensic secure-erasure guarantee.
Opening a graph now creates/opens today's journal. Browsing other absent dates does
not write records. Their first-note form creates a journal only on explicit content
submission; finalized pre-submit text targets private recovery capture within 700 ms.
Normal close attempts capture but remains blocked until the form is resolved, and
nonempty discard requires confirmation. Journal/initial-block submission is atomic
and retries reconcile a stable intended UUID. See [workflows](workflows.md).
The desktop uses application/user-session single-instance enforcement: a second
launch focuses the existing window and does not process graph arguments. This is
not exclusive per-file locking across other applications or independent sessions;
continue to use only one process per graph.

Tauri/the system WebView may create runtime state or caches in platform-managed
locations. Linux uses XDG data/cache locations, Windows uses per-user AppData,
and macOS uses per-user Library locations. Exact subpaths depend on the backend;
the app identifier is `org.graphdesk.desktop`. These are **not** graphs or backups.
Do not delete them to repair notes. Future application-owned settings should use
Tauri's `app_config_dir` / `app_data_dir` / `app_cache_dir` path resolver, not
hardcoded home paths or environment-variable concatenation. Milestone 5 stores only
the three approved preferences in `preferences.json` under the adapter's app-config directory. It does not
store graph paths, note text or credentials. Missing/malformed settings use safe
defaults with a visible warning where appropriate; failed writes retain prior choices.

## Backup and upgrade

There is no built-in online backup/export. For an **offline graph backup**, wait
for **Saved**, close GraphDesk normally (resolve any blocked-close warning), and
verify **every process** with this graph has exited. Use a local private directory
and new absolute paths. Copy only if the graph has **no** `-wal` or `-shm`
sidecars after close; otherwise stop and use a reviewed SQLite-aware backup
procedure, not a bare main-file copy or sidecar deletion. Never mix a database
with sidecars from another moment. On a disposable graph, the Core regression
test checks that a closed copy restores exact text, identity and constraints.

For a closed, checkpointed graph (`source`) and a **new** backup destination
(`backup`) on a local filesystem, use this no-overwrite copy (Python 3):

```sh
source=/absolute/path/to/closed-graph.sqlite3
backup=/absolute/path/to/private-backups/graph-YYYYMMDD.sqlite3
python3 - "$source" "$backup" <<'PY'
import os, sys
from pathlib import Path
source, backup = sys.argv[1:]
if not Path(source).is_file() or any(Path(source + suffix).exists() for suffix in ('-wal', '-shm')):
    raise SystemExit('Source missing or SQLite sidecars present: stop; do not copy the main file alone')
with open(source, 'rb') as original:
    fd = os.open(backup, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as copy:
        while chunk := original.read(1024 * 1024):
            copy.write(chunk)
        copy.flush()
        os.fsync(copy.fileno())
PY
```

This fails rather than overwriting an existing backup; if it fails mid-copy,
do **not** open the incomplete destination. Verify the copy with a read-only
SQLite connection before treating it as a backup (and repeat on the restored
copy), with GraphDesk closed. `immutable=1` is safe **only** for the closed,
checkpointed copy, never a live or WAL-dependent database:

```sh
python3 - "$backup" <<'PY'
import sqlite3, sys
from pathlib import Path
with sqlite3.connect(Path(sys.argv[1]).resolve().as_uri() + '?mode=ro&immutable=1', uri=True) as db:
    assert db.execute('PRAGMA integrity_check').fetchone() == ('ok',)
    assert db.execute('PRAGMA foreign_key_check').fetchall() == []
    print('graph UUID:', db.execute('SELECT id FROM graph_identity WHERE singleton=1').fetchone()[0])
    print('schema version:', db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0])
PY
```

Restore **only to a new, empty absolute path**, by repeating the guarded copy
with `source="$backup"` and `backup=/absolute/path/to/restore.sqlite3` after
verifying no backup sidecars exist. Check integrity/foreign keys and compare
page, task, link and Unicode content in the restored graph. Never copy over an
existing graph, and keep an untouched pre-upgrade backup: opening may migrate
a graph and older binaries may reject it. Binary rollback is not database
rollback. The graph copy retains the logical UUID, so GraphDesk may offer
existing private recovery drafts for that UUID; **compare**, never automatically
apply them. Recovery is stored separately in app-local data and is **not**
included in a graph backup. Preserve its private store separately if unresolved
drafts matter. A package upgrade/removal must never delete user-selected graphs,
private recovery stores, preferences or backups; verify this on native Fedora.

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

Milestone 5 extends this same store with pending first journal notes. Those records
are keyed by stable draft identity and intended block UUID and include canonical
date, finalized text and a conditional-cleanup version. The private schema upgrade
preserves existing block drafts. Startup inventory includes either record type;
open the graph and explicitly restore a pending note into the form. No journal is
created by capture or restoration. Modified/deleted committed content requires review
and is never automatically overwritten/recreated; keep/copy or explicitly discard
unresolved text. The same expiry, privacy and loss-window limits below apply.

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
intended copy. Recent graphs distinguish files by path rather than merging copies
with the same logical UUID. No filesystem scanning for graphs is performed.
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
