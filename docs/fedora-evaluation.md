# Fedora-specific desktop evaluation

**Native Fedora validation is pending.** Ubuntu/Xvfb does not certify Fedora,
Wayland or hardware graphics. See the shared [Linux guide](platforms/linux.md),
[native checklist](testing.md#native-platform-checklist) and [data safety](data.md).
Start with disposable graphs or backed-up copies, never your only important notes.

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
