# macOS development and evaluation

**Target platform; native build, runtime and installer validation pending.**

Follow the [Tauri 2 macOS prerequisites](https://v2.tauri.app/start/prerequisites/)
for Xcode/Command Line Tools and the matching Apple Rust target. macOS provides
WKWebView; Linux GTK/WebKitGTK packages are not needed. Bundled SQLite uses the
Apple C toolchain. Install the pinned Rust and Node versions; Python 3 is needed
only for schema tests (`python3` on PATH).

Run the common [build steps](../../README.md#build-from-source) natively. Output is
`target/release/graphdesk-desktop`. A `.app` or `.dmg` is not currently produced.
Intel and Apple Silicon need separate verification; universal binaries and a
minimum supported deployment version are not established.

Enter an absolute native path in a local writable directory. The UI does not
expand `~` or shell variables. Avoid actively synced folders. Respect standard
macOS directory permissions; do not disable OS protections. See [data safety](../data.md).

Use the [native checklist](../testing.md#native-platform-checklist), especially
Cmd shortcuts, Option-key conflicts, composition, WKWebView selection, Retina,
VoiceOver and normal-close draft flushing. App-bundle lifecycle, signing,
notarization and Gatekeeper checks remain [future release work](../distribution.md).
