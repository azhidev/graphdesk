# Fedora-specific desktop evaluation

**Native Fedora validation is pending.** Ubuntu/Xvfb does not certify Fedora,
Wayland or hardware graphics. See the shared [Linux guide](platforms/linux.md),
[native checklist](testing.md#native-platform-checklist) and [data safety](data.md).
Start with disposable graphs or backed-up copies, never your only important notes.

## Transfer the committed source (not the Ubuntu binary)

The Ubuntu VM must first commit the evaluation instructions, then create a **full**
Git bundle of `refs/heads/main` under ignored `test-artifacts/`; record its full
HEAD SHA and SHA-256. `git bundle verify` and `git bundle list-heads` must report
that same branch/commit. The bundle contains Git history, not ignored notes or
build files; treat it as source code, review before sharing and do not publish it.
Copy the bundle **from Fedora**, using an existing approved transfer mechanism;
never mount Fedora into the VM. For a Fedora host already managing this VM through
Incus, an optional host-side transfer is:

```sh
vm=YOUR_VM_INSTANCE_NAME  # inspect `incus list` if the name is unknown
bundle="$HOME/Downloads/graphdesk-m7-SHORT_SHA.bundle"
test ! -e "$bundle" && incus file pull "$vm/home/worker1/projects/graphdesk/test-artifacts/fedora-m7/graphdesk-m7-SHORT_SHA.bundle" "$bundle"
sha256sum "$bundle"
```

Replace `SHORT_SHA` with the bundle name reported by the VM and stop rather than
overwriting an existing download. If Incus is unavailable, transfer this **one
bundle** via a user-approved offline mechanism instead; no guest networking or
authentication changes are needed. On Fedora, before cloning, compare `sha256sum
"$bundle"` with the VM's reported digest. Then, from a Fedora terminal:

```sh
checkout="$HOME/graphdesk-fedora-m7-SHORT_SHA"
test ! -e "$checkout" && git clone --branch main --no-local "$bundle" "$checkout"
git -C "$checkout" bundle verify "$bundle"
git -C "$checkout" rev-parse HEAD
git -C "$checkout" status --porcelain
cd "$checkout"
```

Stop if the SHA-256 or full HEAD SHA differs, the bundle is incomplete, the
checkout already exists, or the status is not clean. Never clone over the
separate `graphdesk-gpui-poc` project. The bundle names `main` but has no
symbolic remote HEAD, so explicitly select `--branch main`; `--no-local` makes
the clone independent of the transferred bundle. The exact SHA/digest will be
supplied alongside the bundle after its creation, not predicted here.

## Updating an existing evaluation checkout for UI Foundation

For Phase 2, use the same guarded bundle/update procedure below but substitute
`phase2` in the artifact name and use the **Phase 2 committed revision**. On the
clean Fedora checkout verify the full SHA and digest, run `npm ci --ignore-scripts`,
`npm run check`, `npm run test:editor`, `npm run test:schema`, and
`npm run tauri -- build --no-bundle -- --locked`. Launch the newly built
`target/release/graphdesk-desktop` there, not the Ubuntu binary. In addition to
the checklist below, inspect Settings focus/Escape/return, persisted independent
preferences across restart, narrow overlay focus/outside click in both directions,
search/source focus, active drafts and recovery during navigation. Do not report
Ubuntu/Xvfb captures as Fedora validation.

After Phase 1 is committed in the Ubuntu guest, create a **new** full bundle from
the reviewed `main` branch (do not include untracked prototypes or notes):

```sh
revision=$(git rev-parse HEAD)
mkdir -p test-artifacts/fedora-phase1
bundle="test-artifacts/fedora-phase1/graphdesk-ui-phase1-$revision.bundle"
test ! -e "$bundle" && git bundle create "$bundle" main
git bundle verify "$bundle"
git bundle list-heads "$bundle"
sha256sum "$bundle"
```

Transfer that single bundle using the existing approved Fedora-side mechanism
above, substituting the displayed bundle path and name. Verify its SHA-256 and
full commit against the Ubuntu output **before** using it. For an existing clean
Fedora evaluation checkout (do not overwrite work or another project):

```sh
checkout="$HOME/graphdesk-fedora-m7-SHORT_SHA"  # your existing evaluation checkout
bundle="$HOME/Downloads/graphdesk-ui-phase1-FULL_SHA.bundle"
test -z "$(git -C "$checkout" status --porcelain)"
sha256sum "$bundle"  # must match the Ubuntu digest
git -C "$checkout" bundle verify "$bundle"
git -C "$checkout" fetch "$bundle" main
git -C "$checkout" switch --detach FETCH_HEAD
git -C "$checkout" rev-parse HEAD  # must equal FULL_SHA
cd "$checkout"
node --version
rustc --version
pkg-config --modversion gtk+-3.0 webkit2gtk-4.1
npm ci --ignore-scripts
npm run check
npm run test:editor
npm run tauri -- build --no-bundle -- --locked
file target/release/graphdesk-desktop
ldd target/release/graphdesk-desktop  # no 'not found' entries
```

Use the actual checkout path, bundle name, full SHA and digest reported from
the guest; stop if any guard fails. Rebuild on Fedora, not from the Ubuntu ELF.
From separate Wayland and X11-backed launches of this newly built binary (as
available), create a disposable graph; compare English LTR/System and forced
Light/Dark, switch to Persian RTL/Jalali and back, and verify independent saved
preferences after restart. Exercise narrow sidebar/search, Tasks and journal
dates, page creation/graph switching, editor focus and keyboard navigation,
autosave/error/recovery with disposable data, mixed-direction text and a real
Persian IME candidate/preedit cycle. Compare a representative 5,000-block page
for typing and scroll responsiveness against the prior release. Record actual
backend, WebKit/GTK versions, scale, pass/fail/not-tested and sanitized results;
the Ubuntu/Xvfb screenshots are not Fedora evidence.

## Prerequisites

On conventional dnf-based Fedora, an administrator can review these packages
against the installed release and current
[Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/):

```sh
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config \
  webkit2gtk4.1-devel gtk3-devel openssl-devel \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel file
```

Use GTK 3 / WebKitGTK 4.1. Appindicator and xdo are upstream prerequisites, not
tray features or an X11-only architecture. Silverblue/Kinoite need a separately
reviewed build/runtime approach; these commands are not package-layering guidance.

Follow the pinned toolchain/locked builds in the [README](../README.md). Record
Fedora release, architecture, commit and GTK/WebKit versions. Build on the target
system: another distro's dynamically linked binary is not a Fedora package.

In the verified checkout, **check** prerequisites before changing the host:

```sh
cat /etc/fedora-release
uname -m
echo "$XDG_SESSION_TYPE"
node --version
rustc --version
pkg-config --modversion gtk+-3.0 webkit2gtk-4.1
npm ci --ignore-scripts
npm run tauri -- build --no-bundle -- --locked
file target/release/graphdesk-desktop
ldd target/release/graphdesk-desktop
./target/release/graphdesk-desktop
```

Expect Node 24.x, Rust 1.99.1, a native Fedora ELF, no `not found` libraries,
and a normal GraphDesk window without a dev server. If anything is absent,
stop and report it; only install dependencies after reviewing the existing
Fedora/Tauri prerequisites. Do not substitute GTK 4/WebKitGTK 4.0, disable
WebKit sandboxing/SELinux, or run the Ubuntu executable as a compatibility test.

## Manual native checks and baseline (not yet performed)

Use only new disposable local graphs and the [native checklist](testing.md#native-platform-checklist).
From the Wayland login, test separate `GDK_BACKEND=wayland` and
`GDK_BACKEND=x11` launches; verify which backend really ran, and report unavailable
backends as untested. Open/cancel/create using native dialogs. With another app
focused or GraphDesk minimized, launch the same binary a second time: it must
activate the existing window, not open an argument-supplied graph. Check opt-in
recent graphs, automatic reopen, two-graph isolation, normal close, autosave,
explicit recovery, journal/page/task navigation, shortcuts and focus. Test real
IME candidate/preedit commit/cancel separately from Persian layout and pasted text;
check mixed RTL/LTR caret movement, scaling, narrow window and available screen
reader facilities. Do not use personal notes for failure injection. Record
pass/fail/not-tested and sanitized observations; do not claim IME or accessibility
coverage when the relevant facilities are absent.

**Performance:** use this release binary and the same display, scale and graph
for each comparison. After login, time first launch to usable window by a
monotonic stopwatch (first-session launch, **not** a cold-cache benchmark); close
normally and repeat twice for warm launches. With a fresh disposable graph and
then a representative, recorded-size disposable graph (report page/block counts),
sample 30 seconds of idle process-tree PSS/CPU using the repository's lightweight
`tests/perf/linux_process.py` (requires only Python 3 and readable `/proc`):

```sh
./target/release/graphdesk-desktop &
app_pid=$!
python3 tests/perf/linux_process.py --pid "$app_pid" --seconds 30
```

Run those lines in **the same terminal**, with no existing instance; otherwise
single-instance will exit the new process. Confirm the measured PID belongs to
the running app and check the script's `missing_pss` count; if nonzero, the PSS
total is incomplete. The reported process tree includes only current descendants,
not unrelated desktop services. Keep the app idle (no terminal/window activity)
during sampling. Separately record human-observed scrolling a long page, typing
mixed text and resizing between normal/narrow widths: graph size, scale, display
backend, visible stalls, and repeated approximate latency if observable. No
artificial cache clearing, power tweaks, root privileges or speculative optimization.
These instructions are a measurement plan, **not Fedora results**.

## Optional removable local install

No RPM/AppImage or signed release is provided. After a native build, close all
instances. Install into a new commit-specific directory, refusing an existing one:

```sh
revision=$(git rev-parse --short=12 HEAD)
appdir="$HOME/.local/opt/graphdesk-eval/$revision"
mkdir -p "$HOME/.local/opt/graphdesk-eval" &&
  mkdir "$appdir" &&
  install -m 755 target/release/graphdesk-desktop "$appdir/graphdesk-desktop" &&
  "$appdir/graphdesk-desktop"
```

Launch via the installed full path later. Node/Rust and a dev server are not needed
at runtime; compatible native libraries are. Use one process per graph. Back up
before upgrades and install in a new revision directory. To remove, close the app
and delete only the evaluation executable and empty revision directory; preserve
graphs/backups. App-menu integration and installer removal remain unverified.

Record native Wayland versus XWayland, not just session type. Test real IME,
GPU/scaling, accessibility and normal-close persistence. Do not disable WebKit
sandboxing, SELinux or OS/device protections. Crashes, lost committed input,
unusable IME/focus or failed restore block daily-use evaluation; accessibility
failures block users relying on that technology.
