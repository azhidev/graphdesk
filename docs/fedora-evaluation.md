# Fedora desktop evaluation — before Milestone 4

**Ready to attempt evaluation, not Fedora-certified or production-ready.**
Milestones 1–3 are closed for trusted, local, single-user graphs. The README's
Ubuntu Xvfb/WebKit evidence does not validate Fedora/Wayland, hardware graphics,
real IME services or assistive technology. Host validation remains **pending**.
Run this guide yourself in an authorized Fedora checkout; the development guest
must not access or change the host. Start with disposable notes, then a **copy**
of a backed-up graph, never your only important copy.

## Build on Fedora

Build on the machine/architecture that will run Graphdesk. The Ubuntu binary is
dynamically linked against Ubuntu libraries, not a portable Fedora distribution.
Tauri bundling remains disabled: no RPM, AppImage, Flatpak or signed release is
currently provided. This guide installs a removable user-local executable.

On conventional dnf-based Fedora Workstation, the host administrator should
review these packages for the installed release before installing:

```sh
# Host administrator action, NOT for execution in the development guest:
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config \
  webkit2gtk4.1-devel gtk3-devel openssl-devel \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel file
```

Based on the Fedora list in [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/)
(consulted 2026-10-22), with explicit compiler/pkg-config requirements. Appindicator
and xdo are upstream prerequisites, not promises of tray integration or X11-only
behavior. Silverblue/Kinoite require a separately reviewed build/runtime approach;
these commands do not authorize package layering or deployment changes. A toolbox
build alone does not establish that runtime libraries exist on the host.

Install Node **24.x**, npm and Rust **1.99.1** through reviewed methods; do not pipe
downloaded installers into a shell. No global npm packages are needed. Python 3
is required only for schema tests. Bundled SQLite needs a C compiler, not the host
SQLite library. Use GTK **3** and WebKitGTK **4.1**, not GTK 4/WebKitGTK 4.0.

In the Fedora checkout:

```sh
git status --short
git rev-parse HEAD
node --version
npm --version
rustc --version
cargo --version
pkg-config --modversion gtk+-3.0 webkit2gtk-4.1 openssl
npm ci --ignore-scripts
npm run check
npm run tauri -- build --no-bundle -- --locked
```

Stop on mismatched versions or missing dependencies. Rust, Node and dependencies
are pinned in `rust-toolchain.toml`, `.nvmrc`/`package.json` and both lockfiles.
The guest could not download the named Rust channel but had `stable` reporting
exactly 1.99.1. Do **not** substitute whatever Fedora's `stable` reports. Select
an installed toolchain with `RUSTUP_TOOLCHAIN` only after verifying the exact pinned
version; otherwise record the build as blocked. Keep lockfiles unchanged.
Initial builds need registry access; application use does not. Locked dependencies
improve repeatability, not bit-for-bit reproducibility across system libraries,
npm versions or compilers.

Output: `target/release/graphdesk-desktop`. Tauri builds frontend assets and the
production-protocol executable; plain `cargo build` is not a substitute. Record
Fedora release, architecture, commit, tools and GTK/WebKit versions. The README
lists automated validation commands for a new environment; closed milestones do
not need repeating in the guest for this documentation change.

## Install, launch, upgrade, remove

Close all instances first. From the checkout, install into a **new**, commit-specific
directory without sudo. Stop if the revision directory already exists:

```sh
revision=$(git rev-parse --short=12 HEAD)
appdir="$HOME/.local/opt/graphdesk-eval/$revision"
mkdir -p "$HOME/.local/opt/graphdesk-eval" &&
  mkdir "$appdir" &&
  install -m 755 target/release/graphdesk-desktop "$appdir/graphdesk-desktop" &&
  "$appdir/graphdesk-desktop"
```

Use the full installed path on later launches; `appdir` lasts only for this shell.
No checkout, Vite server, Node or Rust is needed at runtime; compatible Fedora
shared libraries must remain installed. Launch from a terminal first for startup
errors. There is no single-instance enforcement: **one process per graph**. Do not
open the same graph in development and installed builds simultaneously.

Optional application-menu integration: copy the icon using
`install -m 644 src-tauri/icons/128x128.png "$appdir/icon.png"`, then create
`~/.local/share/applications/org.graphdesk.desktop.eval.desktop` (create its parent
directory if necessary). Replace both example paths with actual absolute paths.
This example assumes no spaces/backslashes/reserved characters in those paths;
otherwise follow [desktop entry escaping rules](https://specifications.freedesktop.org/desktop-entry-spec/latest/exec-variables.html).
Neither `$HOME` nor `~` expands in these fields.

```ini
[Desktop Entry]
Type=Application
Name=Graphdesk (evaluation)
Comment=Local knowledge graph evaluation
Exec=/home/YOUR_USER/.local/opt/graphdesk-eval/REVISION/graphdesk-desktop
Icon=/home/YOUR_USER/.local/opt/graphdesk-eval/REVISION/icon.png
Terminal=false
Categories=Office;
StartupNotify=true
```

No file association or command-line graph opening exists: enter the path in the
app. Before upgrades, back up, install in a new revision directory and update the
desktop entry. Opening can migrate a graph; older binaries may reject the new
schema. Binary rollback is **not** database rollback: retain a separate pre-upgrade
backup for the older version.

Removal: close the app, delete only the executable and optional icon in the specific
revision directory, then remove that empty directory. Remove only the evaluation
desktop entry if created. Do not recursively delete graphs or XDG directories.
Notes must survive uninstall. System dependency removal is a separate administrator
decision.

## Data, backup and recovery

- **Graph:** the exact path entered in the UI, with adjacent SQLite `-wal`/`-shm`
  files while open. Use an absolute path in an existing private writable directory
  on a local filesystem, e.g. a directory you create under
  `~/Documents/Graphdesk-evaluation/`. `~` is not expanded in the UI. Files are
  unencrypted; access follows OS permissions/umask. Network filesystems and
  actively file-synchronized databases are unsupported.
- **App state:** no default/recent graph registry, persistent undo/collapse or
  durable unsaved-draft file. Re-enter the graph path after restart. WebKit/Tauri
  may store WebView state/cache under XDG locations (normally `~/.local/share`
  and `~/.cache`, app identifier `org.graphdesk.desktop`). Exact backend subpaths
  vary; these are not graphs/backups. Do not delete them to repair notes.
- **Diagnostics:** no persistent application log collector or crash reporter.
  Terminal errors, screenshots and databases can contain paths/private notes;
  review before sharing. Never publish credentials or personal graph contents.

**Backup:** wait for **Saved**, close normally and verify all processes using the
graph have exited, then copy to a new backup path. Never copy only a live main
database: committed writes may be in WAL. If sidecars remain, do not delete them
or assume checkpointing: preserve the closed database and sidecars as a set, or
use a reviewed SQLite-aware backup procedure. Never mix database/sidecar versions.
No built-in online backup/export exists. Test restore by opening a separate copy
and checking pages, tasks, links and Unicode. Keep an untouched pre-upgrade copy.

**Crash:** unacknowledged text may be lost; SQLite durability is not draft recovery.
Stop all instances and preserve the database and sidecars together. Open a working
copy with matching sidecars for SQLite recovery, not your only original. On errors
stop writing; do not delete WAL, reset migrations or apply speculative SQL.
Optional SQLite integrity/foreign-key checks belong on the copy, not an actively
edited graph. Integrity does not prove every intended edit saved.

Save errors retain drafts and prevent navigation/normal close. Resolve free space,
permissions or another writer, then use **Retry saves / clear error**. Do not
repeatedly force-quit. **Discard unsaved drafts and reload** deliberately loses
drafts; it is not recovery. Failed graph creation can leave an empty reserved file:
inspect before removing. Open only trusted databases; hostile-file hardening is
not implemented.

## Host acceptance checklist

Record pass/fail/not tested, versions and reproduction. This checklist is **not**
a claim of host validation. Use disposable graphs for error/crash tests; never
fill a real disk or damage valuable data to inject failures.

| Area | Required host evidence |
| --- | --- |
| Launch/persistence | Terminal and optional menu launch without dev server; create/reopen after normal exit and restart. Record compositor/session and whether the app is native Wayland or XWayland, not just the desktop session type. |
| Keyboard | Split, Shift+Enter, Tab/Shift+Tab nesting, Alt+Shift+arrows, selection, paste, undo/redo and task changes work; restart retains acknowledged edits. Toolbar alternatives work for compositor shortcut conflicts. |
| Focus exit | Escape focuses the toolbar, then Tab navigates controls. Tab in block text intentionally nests instead of leaving the editor. Native selection/caret remains usable. |
| Persian/bidi | Persian titles/paragraphs, mixed Latin, numbers, punctuation, ZWNJ and emoji render, select, paste and persist. Visually check caret/multiline behavior. Blocks have automatic direction; the interface is not a translated RTL UI. |
| Search | Persian saved-text phrase search and page/block navigation work. Stemming, Arabic/Persian yeh/kaf equivalence, transliteration and general ZWNJ normalization are not promised. |
| Real IME | Actual IBus/Fcitx/input service: preedit, candidates, commit/cancel, Enter/Tab/undo, focus/page change and close during composition. No premature split/truncation or lost committed input. Synthetic composition tests are insufficient. |
| Save/close | Immediate switch/close flushes text. Controlled SQLite writer lock on a disposable graph causes visible error, retained draft and blocked close; retry succeeds after unlock. Forced termination has no draft-loss guarantee. |
| Display | Light/dark desktop settings, 100%/fractional/200% scaling, small window, maximize, long text, nesting, multi-monitor moves and suspend/resume remain legible with reachable controls. App uses a fixed light palette, not native dark mode. |
| Accessibility | Keyboard-only navigation, visible focus, control names, status/error announcements and editor exit work with the user's real screen reader (e.g. Orca). Check large fonts/zoom and contrast. No accessibility conformance claim exists. |
| Offline | Installed executable launches, edits and reopens without network access; no firewall/network configuration changes needed. |
| Backup/removal | Restore into a separate path and verify content; uninstall only evaluation files, preserving graph and backup. |

For blank/crashing windows, retain terminal errors and record Fedora, GTK/WebKit,
driver, compositor and session versions. If XWayland is already available, a
one-launch `GDK_BACKEND=x11 /absolute/path/to/graphdesk-desktop` comparison may
isolate a backend issue; label it XWayland, not a Wayland pass. Do not disable
WebKit sandboxing, SELinux, GPU/device permissions or accessibility isolation to
obtain a pass. No global environment/integration changes are prescribed.

## Evaluation gate and limits

Before important daily-use data, require build/install/reopen, backup restore,
keyboard/IME and save/close checks on the actual desktop. Crashes, lost committed
input, unusable focus/IME, unreadable display or failed restore block evaluation.
Accessibility failures block users depending on that technology. Record failures
before choosing Milestone 4 scope.

Remaining limits: bounded acknowledged-state phrase search; memory-only undo and
collapse; rename not undoable; no rich text, nonempty block merging, drag/drop,
cross-page moves, properties UI, native graph picker or recent-graph list. Sync,
encryption, automatic backups, Markdown import/export and crash-draft recovery
remain future work. See [architecture](architecture.md) for storage constraints.
This preparation neither reopens milestones nor claims Fedora tests have run.

### Guide validation (Ubuntu guest, 2026-10-22)

All three shell blocks were syntax-checked with `bash -n` without executing host
setup/build commands. Local documentation links and executable/icon source paths
were checked. The installation block was exercised in a disposable directory
with a synthetic executable: install/launch passed; a second install refused to
overwrite and did not relaunch. This fixture does not validate Fedora libraries,
menu integration or desktop runtime. No completed milestone suite was rerun.
