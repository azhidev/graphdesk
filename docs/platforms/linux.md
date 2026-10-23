# Linux development and evaluation

Linux is a target across distributions, not a single tested support tier. Existing
runtime evidence is Ubuntu/GTK/WebKit under Xvfb; native Wayland, other distributions,
hardware graphics, real IME services and screen readers remain unverified.

Install the packages for your distribution/release listed in the
[Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/). Typical build
requirements include a C/C++ compiler, make, pkg-config, GTK 3, WebKitGTK 4.1,
OpenSSL and upstream appindicator/librsvg/xdo dependencies. GTK 4 or WebKitGTK 4.0
are not substitutes. Bundled SQLite needs a C compiler, not system SQLite.
Package names differ across Debian/Ubuntu, Fedora and other distributions: review
the upstream list rather than applying one distro's package command everywhere.
Immutable distributions need a separately reviewed build/runtime approach; a
container build does not prove native runtime libraries exist.

Follow the common [build steps](../../README.md#build-from-source). Output is
`target/release/graphdesk-desktop`. Launch from a terminal first. The executable
depends on compatible shared libraries; copying between distributions is not a
portability guarantee. No installer is provided. Optional Fedora-specific setup
is in [the Fedora guide](../fedora-evaluation.md).

Use [the native checklist](../testing.md#native-platform-checklist). Record distro,
architecture, GTK/WebKit, driver and compositor versions. Distinguish native Wayland
from XWayland; test shortcut conflicts, scaling, graphics and actual IBus/Fcitx and
screen-reader behavior. Do not disable WebKit sandboxing or OS/device protections.
The Linux-only WebView fixture is not a replacement for actual desktop checks.
