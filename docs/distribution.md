# Build and distribution direction

Bundling is disabled in `src-tauri/tauri.conf.json`; current builds produce an
unbundled executable. Platform icons exist, but this is not installer validation.
No official packages, signed artifacts, updater or file associations exist.
The desktop has a user-session single-instance plugin, but second-launch window
activation still needs validation on a real compositor; it is not per-graph locking.
Keep packaging opt-in until native runtime/data-safety checks pass. Do not change
IPC/CSP to make packaging work.

## Incremental recommendations

1. Validate locked source on each native OS/architecture. Record tool versions and
   establish explicit minimum OS/runtime baselines; none is established yet.
2. Trial Tauri's native bundler manually on each OS. Isolate platform-specific
   settings in per-platform Tauri configuration files when needed, not shared
   scripts full of OS branches. Review dependency licenses and installed files.
3. Test clean install, offline launch, upgrade and uninstall using disposable
   graphs. Notes/backups must survive uninstall; schema upgrades need a restore path.
4. Only then select published formats, signing identities and release automation.
   No CI matrix, updater or signing infrastructure is needed for this review.

| Platform | Candidate formats | Required follow-up |
| --- | --- | --- |
| Linux | Tauri `.deb` / `.rpm`; evaluate AppImage | Deliberate glibc/library baseline, distro/architecture runtime tests. AppImage is not universal compatibility. Consider Flatpak later if demand warrants portal/sandbox work. |
| Windows | Tauri NSIS or MSI | WebView2 deployment/offline behavior, native architectures, signing/SmartScreen, upgrades and removal. |
| macOS | Signed/notarized `.app` in `.dmg` | Intel/Apple Silicon, deployment target, signing/notarization, Gatekeeper and bundle lifecycle. Universal binaries are optional later. |

Native dependencies remain in platform guides. No new packaging framework or
runtime dependency is needed now. Never publish certificates, credentials,
private notes or personal build paths.
