# Windows development and evaluation

**Target platform; native build, runtime and installer validation pending.**

Use the [Tauri 2 Windows prerequisites](https://v2.tauri.app/start/prerequisites/):
Microsoft C++ Build Tools with the desktop C++ workload and Windows SDK, the MSVC
Rust target, and Microsoft Edge WebView2 Runtime. Bundled SQLite uses the native
compiler. Follow upstream requirements for the selected architecture. WSL testing
does not establish native Windows or Windows WebView compatibility.

Install the README's pinned Rust and Node versions. Python is optional except for
schema tests; `npm run test:schema` uses `py -3` on Windows. Python's SQLite needs
STRICT tables (3.37+), FTS5 and JSON. Without the launcher, run
`python -m unittest discover -s tests -v` with a suitable Python 3 installation.

Run the common [build steps](../../README.md#build-from-source) in PowerShell or
a developer terminal configured for MSVC. No Unix shell commands are required.
The unbundled output is `target\release\graphdesk-desktop.exe`.

Enter a full native drive path without shell quotes or `%VARIABLE%` expansion.
Use a local writable directory, not a network share or actively synced folder.
See [data safety](../data.md) and the [native checklist](../testing.md#native-platform-checklist).
Specifically test WebView2 availability, Unicode/spaced paths, locking/sidecars,
Ctrl shortcuts, IME, high DPI and screen readers. Packaging/signing remains
[future work](../distribution.md).
