# Milestone 8 Fedora alpha evidence (developer-only)

The runner is **read-only with respect to RPM/system installation**. It launches
the production executable only under authenticated Xvfb/X11 with fresh temporary
XDG config/data/cache directories and synthetic graphs. It never opens personal
graphs or app directories, changes SELinux/sandboxing, installs, upgrades, removes
packages, or reaches into the Fedora host from the Ubuntu VM. The P3 recovery
recurrence remains open; the fixture is not an investigation of personal data.

Use a clean Fedora checkout at `/home/azhidev/projects/graphdesk` (adjust only
if the actual approved checkout differs); first verify its committed SHA and
the RPM's SHA-256. Do **not** transfer the Ubuntu binary as a Fedora build.
Review dependency/lifecycle scripts before local `npm ci` and follow the
[Linux native prerequisites](platforms/linux.md). For the Fedora-native RPM:

```sh
cd /home/azhidev/projects/graphdesk
git status --short --branch
npm ci
npm run check && npm run test:editor && npm run test:schema
cargo test --locked --workspace
npm run tauri -- build --config src-tauri/tauri.linux-alpha.conf.json --bundles rpm -- --locked
rpm=target/release/bundle/rpm/Graphdesk-0.1.0-1.x86_64.rpm  # use actual build name/arch
sha256sum "$rpm"
rpm -qip "$rpm"; rpm -qpl "$rpm"; rpm -qp --scripts "$rpm"; rpm -qp --requires "$rpm"
python3 tests/fedora_alpha.py --mode inspect --rpm "$rpm" --output test-artifacts/fedora-alpha-inspection
```

`--mode inspect` makes **no installation**. It checks package/version/arch, required
binary/desktop/icon paths and unexpected scriptlets. A missing `rpm` utility or
package is BLOCKED, not passed. Each invocation requires a *fresh* output name;
the runner refuses to overwrite an existing directory or archive. The inspection
report/archive (`test-artifacts/fedora-alpha-inspection*`) is separate from the
full review; it does not prove the launcher works in GNOME.

## Individually authorized system steps (manual, on Fedora only)

1. **Offline install** (only after explicit local authorization and manifest
   review): `sudo dnf install ./target/release/bundle/rpm/Graphdesk-0.1.0-1.x86_64.rpm`.
   Do not run this runner with `sudo`. Check `rpm -q graphdesk`,
   `rpm -ql graphdesk`, `cat /usr/share/applications/Graphdesk.desktop`, and
   `/usr/share/icons/hicolor/128x128/apps/graphdesk-desktop.png`. Do not use a
   normal user session's existing app data to test the launcher: use an isolated
   test login/session for GNOME menu verification, or run the executable through
   the runner's isolated XDG session. Record a **manual native** launcher screenshot
   separately; the Xvfb fixture cannot screenshot the real GNOME application menu.
2. **Automated installed-binary trial** (no privilege, no install/remove):

   ```sh
   cd /home/azhidev/projects/graphdesk
   python3 tests/fedora_alpha.py --mode full --rpm "$rpm"
   sha256sum test-artifacts/graphdesk-fedora-alpha-review.tar.gz
   ```

   Use a fresh checkout `test-artifacts/fedora-alpha-review/` (or explicitly move
   an older report aside first). The runner captures production `--review`,
   `--reliability`, `--shell` and `--navigation` assertions/screenshots under
   Xvfb/X11, closed-graph backup and
   separate restore with read-only integrity/FK checks, and idle samples for the
   existing 1,000/5,000 single-page fixtures when available. It refuses to copy
   a WAL-dependent main file. The 5,000 sample is skipped if 1,000 fails; `--no-perf`
   records NOT TESTED. Its 30-second PSS/CPU readings are measurements, **not** a
   pass against an established performance threshold. To generate *new* fixtures
   if missing, follow [the Core fixture procedure](large-graph-benchmark.md#single-page-sibling-workload-new)
   in an empty ignored fixture directory; never regenerate over existing files.
3. **Backup and verify** a *different* disposable graph you created interactively
   using the guarded offline procedure in [data](data.md#backup-and-upgrade).
   Save, close all graph processes, require no WAL/SHM, verify hashes and SQLite
   integrity/FKs, restore to a new path, reopen and compare exact text. Private
   recovery snapshots are separate; preserve their store separately if needed.
   Record any unknown/failed checks as NOT TESTED/FAIL, not as a passed backup.
4. **Uninstall/reinstall** (only after separate authorization): while the app is
   closed, record hashes of disposable graph, backup, private recovery and config
   files under **your isolated test account**; `sudo dnf remove graphdesk`, then
   inspect that those paths remain. Reinstall the reviewed package using the
   install command above; reopen the same test graph and compare metadata and
   saved text. Never purge app data or target existing personal files.
5. **Real upgrade** requires two *different versioned, compatible native RPMs*.
   Preserve an untouched pre-upgrade graph backup and private recovery copy,
   install the newer package as a separately authorized operation and repeat
   the above checks. Reinstalling `0.1.0` is **not** upgrade validation. To roll
   back an executable, first check schema compatibility; if a migration occurred,
   restore the untouched older graph backup to a **new** path before opening with
   the older binary. Never overwrite the migrated graph or silently discard drafts.

## Evidence and remaining manual gates

`test-artifacts/fedora-alpha-review/` contains only curated synthetic
`screenshots/`, `results.json`, `report.md` and safe metadata/fixture logs under
`logs/` (including hashes and row counts). Raw app/Xvfb logs and disposable DBs
remain in temporary directories and are not archived; review screenshots before
sharing. The runner creates `test-artifacts/graphdesk-fedora-alpha-review.tar.gz`
and prints its SHA-256; independently verify with `sha256sum`. A separate archive
command, if needed **before an archive exists**, is:

```sh
umask 077
test ! -e test-artifacts/graphdesk-fedora-alpha-review.tar.gz &&
  test ! -L test-artifacts/graphdesk-fedora-alpha-review.tar.gz &&
  tar -czf test-artifacts/graphdesk-fedora-alpha-review.tar.gz -C test-artifacts fedora-alpha-review &&
  sha256sum test-artifacts/graphdesk-fedora-alpha-review.tar.gz
```

The runner does **not** perform the following: real Persian IME candidate/preedit
commit/cancel and caret/selection, native Wayland focus and narrow overlay with a
real compositor, GNOME launcher/menu screenshot, assistive-technology behavior,
system install/remove, version upgrade or power-loss testing. On Fedora, record
pass/fail/not-tested for these manually; if an installed binary cannot launch,
data is lost, or a restore fails, stop and preserve private evidence.

For an Ubuntu-only rehearsal with an existing unbundled binary, without claiming
native Fedora coverage:

```sh
python3 tests/fedora_alpha.py --mode full --local --no-perf --output test-artifacts/ubuntu-alpha-rehearsal
```

The Xvfb/Python GTK, xauth, xdotool, dbus-run-session and `smaps_rollup`
dependencies are the same as the [existing WebView fixture](testing.md).
Only the installed binary under `/usr/bin/graphdesk-desktop` is tested by default
on Fedora; `--local` is an explicitly labeled source-build rehearsal.

## Targeted Review click diagnosis (installed binary; no package changes)

If Review stops after the English edit but the Persian edit does not persist,
run the **direct** WebView fixture twice from the Fedora checkout, using new
artifact directory names each time. Do not run `fedora_alpha.py --mode full` for
this probe: its normal Review run does not forward diagnostic coordinates.

```sh
cd /home/azhidev/projects/graphdesk
umask 077
python3 tests/webview/smoke.py --review --diagnose-review \
  --review-persian-click 690,518 --review-nested-click 700,590 \
  --binary /usr/bin/graphdesk-desktop \
  --artifacts test-artifacts/fedora-review-diag-original
python3 tests/webview/smoke.py --review --diagnose-review \
  --review-persian-click 690,530 --review-nested-click 700,607 \
  --binary /usr/bin/graphdesk-desktop \
  --artifacts test-artifacts/fedora-review-diag-alternative
```

If either directory already exists, choose a **different fresh name** instead;
the probe refuses to overwrite it. A failed assertion returns nonzero but still
writes `diagnostic-results.json` and `diag-*-after-click.png`,
`diag-*-after-paste.png`, and `diag-*-assertion-failed.png` for the failing
stage. Read each result file locally to compare `outcome`, `target_id`,
`changed_ids` and `suffix_received_ids`: `intended_block_exact` means the
expected content reached the target; `another_block` means the suffix appeared
elsewhere; `no_block_persisted_edit` means no SQLite block changed. Other
non-exact outcomes need screenshot inspection. The nested stage runs only if
the Persian stage passes. A PASS here covers only these two edits, not Review.

These ignored private artifacts contain only synthetic fixture data; keep the
screenshots local and do not add them, raw app logs or databases to any shared
archive. Compare both runs before attributing the failure to coordinate drift.
If Fedora evidence confirms drift, focus the editor using the rendered target
block's stable `data-block-id` (or an accessible target selector) and verify
the active editable element before pasting, rather than replacing one fixed
coordinate with another. Do not change production code based on this probe.
