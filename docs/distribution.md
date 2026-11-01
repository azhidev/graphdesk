# Build and distribution direction

## UI Foundation handoff and next milestone (2026-11-01)

UI Foundation implementation is complete; all six Ubuntu/Xvfb production WebView
regression suites and the relevant editor/recovery/Core/IPC tests pass. The user
reported completed **native Fedora visual and 5,000-block responsiveness checks**
with acceptable behavior. This is scoped evaluation evidence, not an official
Fedora performance baseline or certification of every item in the
[native checklist](testing.md#native-platform-checklist); detailed reproducible
post-Foundation measurements are not recorded here. The
[P3 recovery recurrence](reliability.md#known-issue-fedora-recovery-drafts-after-restart-p3--deferred)
remains open/not reproduced, not fixed. Real Persian IME candidate/preedit
commit and cancellation, screen-reader behavior, native compositor focus/overlay
behavior across setups, Windows/macOS, power-loss and upgrade/uninstall data
preservation remain unverified or incompletely recorded. See the
[Fedora handoff](fedora-evaluation.md) and [testing evidence](testing.md).

**Recommended smallest next milestone: disposable Fedora alpha install and data
safety trial.** No new editor features, visual redesign, updater or public release:

1. Produce one opt-in, local Fedora-native package from the locked commit
   (evaluate Tauri RPM; keep the existing unbundled executable as comparison).
   Document prerequisites and inspect installed files; keep signing/publishing
   and other OS packages out of scope. Accept only if clean install and fully
   offline launch work without a dev server or weakened IPC/sandbox settings.
2. With disposable graphs, private recovery storage and a **separate closed**
   backup copy, verify normal save/restart, backup/restore to an independent
   path, migration/upgrade using a known older disposable schema fixture, and
   uninstall/reinstall without touching graphs, recovery copies or backups.
   Check exact text, `integrity_check`, foreign keys, and recovery comparison
   after restore.
   Stop and escalate on missing acknowledged text or failed restore.
3. Run a bounded real daily-workflow trial on native Fedora: create/open,
   search/tasks/journal, mixed LTR/RTL editing, keyboard focus, failure and
   recovery, graph switching and normal exit. Record actual Wayland/XWayland,
   versions, pass/fail/not tested and the open P3 issue. Test a real Persian IME
   candidate cycle and available assistive technology separately; mark either
   untested if unavailable. Do not call it a daily-driver alpha for affected
   users if critical input, restore or accessibility checks fail.

Acceptance is a reproducible local install/rollback and data-preserving restore
record, plus an explicit remaining-risk list. Packaging is **proposed**, not
implemented or approved for publication by this handoff.

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
