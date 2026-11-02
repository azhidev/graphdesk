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

**Milestone 8 is preparing the disposable Fedora alpha install and data-safety
trial.** No new editor features, visual redesign, updater or public release:

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
record, plus an explicit remaining-risk list. Ubuntu-built bundles do not satisfy
the native Fedora acceptance criteria.

## Linux alpha build and validation

The developer-only [Fedora evidence runner](fedora-alpha-runner.md) supplies
read-only RPM inspection, isolated production Xvfb checks, backup/restore
summaries and a review archive. Installation and native Wayland/IME checks remain
separate authorized Fedora steps.

### Milestone 8 native status (reported Fedora 44, before final UX changes)

The Fedora operator reports a native RPM build/install, the production Review
and Shell suites, Reliability with synthetic recovery and a closed-graph
backup/restore as **PASS**. Manual GNOME launch, two-graph switching, native
Open/Create dialog cancellation, bilingual text persistence after restart and
subjectively acceptable large-graph responsiveness also passed. The Navigation
Xvfb fixture **FAIL** is not classified as a product defect. These results
predate the final editor scroll, switcher and startup changes; they do not
validate the new binary. The older VM-only packaging status below is historical.

Three separate commits implement a conditional editor focus scroll, an anchored
graph-switch popover and a compact startup chooser. The scroll behavior is a
code-supported **likely** cause of the reported jump, not a confirmed Fedora
fix. Keyed block IDs, native caret restoration and the existing structural
save/undo paths are retained. The startup screen now prioritizes native
Open/Create; manual paths are advanced, graph history is opt-in, and the
recovery inventory notice is next to the action that opens its original graph.
No graph paths or preferences are migrated.

**M8 is still open.** Verify the rebuilt Fedora binary on Wayland for editor
indent/outdent (including a visible block midway through 5,000 siblings),
focus/caret/selection and real Persian IME, the switcher and the startup
screen in Light/Dark and LTR/RTL, cancel/no-overwrite, recovery and restart.
The old coordinate-based Xvfb Review/Shell/Navigation fixtures target the
previous chooser layout and cannot certify this UI without focused fixture
adaptation; do not interpret their startup failure as an app defect. The
Navigation Xvfb failure remains unclassified. The older recovery-draft
recurrence is **unresolved**. Real-version upgrades, native assistive
technology and native Wayland checks for the changed UI are outstanding.

The opt-in `src-tauri/tauri.linux-alpha.conf.json` turns on Tauri's existing
bundler **only** when passed with `--config`. The normal unbundled developer
build is unchanged. It preserves version `0.1.0`, identifier
`org.graphdesk.desktop`, embedded production frontend, icon and native launcher.
No dev server, filesystem permission, post-install scripts, file associations,
auto-update or app-directory overrides are added. Each target builds for the
**build host's** Linux architecture/ABI; an Ubuntu-generated RPM is a package
format check, **not** a Fedora-compatible release binary. On a clean Fedora
checkout of the committed revision, with approved local build prerequisites:

```sh
npm ci
npm run check
npm run test:editor
npm run test:schema
cargo test --locked --workspace
npm run tauri -- build --config src-tauri/tauri.linux-alpha.conf.json --bundles rpm -- --locked
```

The package is under `target/release/bundle/rpm/Graphdesk-0.1.0-1.x86_64.rpm`
on x86_64; use the actual generated name on other architectures. Ubuntu-side
`--bundles deb` produces `target/release/bundle/deb/Graphdesk_0.1.0_amd64.deb`.
Inspect package metadata, payload, desktop launcher (`Exec=graphdesk-desktop`),
icon, library dependencies and checksum before installing. The Tauri-generated
packages install a binary under `/usr/bin` and icons/launcher under `/usr/share`;
graphs, preferences and drafts are not package-owned files. The VM work did
not install or launch the RPM on Fedora; the separate Fedora operator report
above records a successful native install.

Native Fedora trial (disposable graph paths and isolated XDG config/data/cache):

1. Record Fedora release/arch, Wayland vs XWayland, SHA-256, package manifest and
   dependencies. Install locally with `sudo dnf install ./Graphdesk-*.rpm` **on
   Fedora only**, after reviewing the package; do not disable SELinux/sandboxing.
   Launch from the desktop menu and a different working directory, offline and
   without Vite, then create/open a disposable graph and restart normally.
2. Wait for Saved and close all instances; make and independently verify a
   no-overwrite graph backup and restore using [data safety](data.md#backup-and-upgrade).
   Compare exact text/identities/tasks/links, SQLite integrity and foreign keys.
   Compare private recovery separately; do not mistake the graph backup for a
   copy of unresolved drafts. Preserve a pre-upgrade backup and XDG directories.
3. Upgrade to a **later, separately versioned** compatible Fedora build and
   verify graphs/preferences/recovery still exist; don't pretend a same-version
   reinstall validates upgrade. Record a schema migration only if the two builds
   actually differ. On Fedora, `sudo dnf remove graphdesk` (not a purge or data
   cleanup), check the disposable XDG paths and external graphs, reinstall the
   reviewed RPM, and reopen the same graph. Roll back the binary by reinstalling
   the old package only after checking schema compatibility; if the newer build
   migrated the graph, restore the untouched old backup to a **new** path and
   open it with the old binary. Never overwrite a live/newer graph.
4. Exercise real Persian IME candidate commit/cancel, RTL/LTR edits, native
   dialogs, window focus, accessibility where available and recovery comparison;
   mark unavailable checks *not tested*. Stop on lost text, failed restore or
   persistent incorrect recovery, and retain evidence privately. The P3 issue
   remains open; do not suppress warnings.

VM-side builds and regression tests are not a native Fedora package install,
upgrade, uninstall, display, Persian IME or accessibility result. No signing or
publishing is included.

Bundling is disabled in `src-tauri/tauri.conf.json` by default; the opt-in
Linux alpha config above enables local package builds. No official packages,
signed artifacts, updater or file associations exist.
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
