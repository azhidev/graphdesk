# M8 startup and graph management

M8 remains **open until the rebuilt application passes native Fedora acceptance**.
Editor Jump and M9 plugins are not included. No editor/schema migration, dependency,
framework, filesystem IPC permission or networking change is needed here.

## Cause and behavior

The graph chooser/popover previously used only the on-disk MRU, and hid it entirely
when remembering was off. Successful Create/Open therefore looked as though it had
not registered a graph. Both registry flags defaulted to false, so a new installation
neither remembered graphs nor reopened them. The existing staged switch already
recorded an acknowledged activation before journal landing, not just at shutdown;
that safe ordering is retained.

- Every acknowledged activation immediately updates a memory-only session MRU,
  independent of disk persistence. It merges with the saved MRU by backend-canonical
  absolute path, not graph UUID (copies can share UUIDs). The existing ten-entry
  bound is retained. Paths are shown in tooltips; file stems are the normal labels.
  Current schema metadata has identity but **no graph display-name field**, so no
  speculative schema migration is added. Duplicate names include parent context;
  identical parent names/UUIDs fall back to the canonical location.
- A genuinely new installation exclusively creates `GraphDesk Demo.sqlite3` under
  Tauri `app_data_dir()/graphs` (Linux: normally
  `$XDG_DATA_HOME/org.graphdesk.desktop/graphs`). It is an ordinary editable local
  graph. Welcome, Getting Started, Tasks and today's Gregorian journal demonstrate
  nested blocks, links, task statuses and mixed Persian/English text, explicitly
  labelled sample content. English/LTR and Gregorian defaults are unchanged.
- Startup checks the private registry, preferences, first-run marker and recovery
  directory before seeding. Any prior evidence suppresses seeding; inspection
  failures are conservative, not evidence of freshness. First-run detection never
  reads/deletes recovery rows. Existing/corrupt/future registry data is not reset.
  An existing demo path is never seeded or overwritten, including after an
  interrupted first run. A valid leftover can be opened normally. Partial creation
  failures keep the file and show a warning, not destructive cleanup/retry seeding.
- New installations enable remembering and reopening and store a private
  `first-run.json` completion marker in app-config. Removing or moving the demo
  later does not recreate it. Seed creation itself does **not** record last-active:
  only successful staged activation does, before recovery/journal landing.
- Automatic reopening uses the first saved MRU entry and its expected UUID, never
  Create. Missing, inaccessible, identity-replaced or unsupported-format graphs
  leave the chooser with an error. Use native Open to locate a moved graph; nothing
  relocates/deletes files or silently substitutes a new database.
- Ctrl+Tab/Shift+Ctrl+Tab reuse the anchored popover, selecting forward/backward
  without activating. Enter activates; Escape restores originating focus without
  switching; mouse selection and arrow keys work too. Current graph is separately
  marked. Capture-phase cancellation and WebKit reverse-Tab event variants are
  handled, while IME/preedit, AltGr, busy operations and other dialogs are excluded.
  Native WebKitGTK/compositor delivery still requires the acceptance below.
- Switches still flush/check drafts, pending first-note forms and composition,
  stage candidates, detach inputs, activate with session guards and reconcile
  uncertain acknowledgements. Failed saves retain the source. Successful switches
  focus the new journal; cancel/failure returns to the originating control when
  still mounted, otherwise the graph header. Dialog cancellation never invokes
  preparation/activation. Absolute positioning avoids workspace layout shift.

## Privacy and migration

### Clarified Fedora startup finding

The Fedora operator has verified that reopening works when enabled; the tested
profile had reopening disabled. Commit `0c21ace` already sets **both** flags to
true after the genuinely-fresh check. No production startup/activation/persistence
change is needed for this clarification. Older default-off profiles and explicit
opt-outs are intentionally preserved, as are preferences-only/recovery-only
profiles. Enabling remembering in Settings on an existing profile deliberately
passes `reopen:false`; reopening remains a separate choice. Any of these can
explain an off setting, but which happened on Fedora is not established without
that profile's history. Do not infer a reopening defect or overwrite its settings.

The first-run/defaults and distinct-instance last-active reopen IPC tests already
cover new installs. A separate parameterized IPC regression now covers all valid
persisted flag combinations across distinct app instances, asserting byte-for-byte
registry preservation and no demo/marker creation. Cargo remains unavailable in
the VM, so these Rust tests still require the Fedora build/test commands below.

There is **no automatic settings/schema migration** for existing installations.
An existing `remember:false`, `reopen:false`, saved preferences-only installation,
or recovery-only installation stays as it was, even if those were old defaults
rather than a deliberate choice. This conservative policy cannot infer consent
from an absent history list. Enable the existing Settings controls explicitly to
change it. A valid existing `remember:true/reopen:false` is likewise preserved.

Remembering persists local paths/UUIDs (not notes/page names/searches) privately;
paths can themselves be sensitive. Turning it off clears persisted entries and
disables reopening, but graphs used in the running session remain selectable.
Forget/clear removes matching noncurrent session entries as well; the open graph
remains identifiable. No graph/recovery file is deleted. OS picker history, backups
and secure erasure are outside that setting. The first-run marker retains only
the standard demo location and completion state, not subsequent user graph paths.

## Verification in Ubuntu agent-lab

Executed: Svelte/TypeScript check (zero errors/warnings), production frontend build,
71 Node tests and 29 Python/SQLite/fixture-helper tests passed. The new Node cases
exercise session merging, duplicate names, keyboard classification/cycling and
A → B → A through the actual editor controller with a fixture transport. They
are not native input, native dialogs or Rust SQLite integration evidence.

Rust demo/startup/IPC cases were added for one-time seeding, distinct-instance
reopen, opt-outs, corrupt registry, existing preferences/recovery/marker/demo files,
and failed missing-file opens leaving history untouched. **Not executed:** Cargo
is unavailable (`cargo: command not found`). No updated native executable was built
or launched. Existing Rust editor/recovery regressions and actual WebKitGTK,
Light/Dark/RTL, dialog cancel and real IME checks remain unverified for this change.
The unrelated pre-existing README edits and `skills/` directory are not part of
these commits. Prior Fedora evidence does not certify this rebuilt revision.

## Exact Fedora build and native acceptance

In a clean Fedora checkout of the delivered commit, using Node 24, pinned Rust
1.99.1 and the approved [native prerequisites](platforms/linux.md), run:

```sh
git rev-parse HEAD
git status --short
node --version
cargo --version
npm ci --ignore-scripts
npm run check
npm run test:editor
npm run test:schema
cargo test --locked --workspace
cargo test --locked --workspace --features tauri/custom-protocol
npm run tauri -- build --no-bundle -- --locked
file target/release/graphdesk-desktop
ldd target/release/graphdesk-desktop
sha256sum target/release/graphdesk-desktop
```

If also evaluating an RPM (review/install on Fedora only; no push/publish):

```sh
npm run tauri -- build --config src-tauri/tauri.linux-alpha.conf.json --bundles rpm -- --locked
rpm -qip target/release/bundle/rpm/Graphdesk-0.1.0-1.x86_64.rpm
rpm -qlp target/release/bundle/rpm/Graphdesk-0.1.0-1.x86_64.rpm
rpm -qp --scripts target/release/bundle/rpm/Graphdesk-0.1.0-1.x86_64.rpm
sha256sum target/release/bundle/rpm/Graphdesk-0.1.0-1.x86_64.rpm
```

Use the actual generated filename for non-x86_64. Stop on failed tests, missing
libraries or unexpected package scripts. Installation/data-safety acceptance remains
in [distribution](distribution.md#linux-alpha-build-and-validation).

Close **all** existing GraphDesk instances normally first (single-instance behavior
otherwise forwards to the existing application). From the checkout, in a fresh
terminal, run on the native Fedora desktop, not Xvfb:

```sh
binary=$(realpath target/release/graphdesk-desktop)
base=$(mktemp -d -t graphdesk-m8-startup-XXXXXXXX)
export XDG_CONFIG_HOME="$base/config" XDG_DATA_HOME="$base/data" XDG_CACHE_HOME="$base/cache"
mkdir -m 700 -p "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME"
printf 'Disposable acceptance directory: %s\n' "$base"
"$binary"
```

1. Expect one open **GraphDesk Demo**, English/LTR, Gregorian. Inspect Welcome,
   Tasks, nesting and mixed text; edit a sample block and wait for Saved. Close
   normally. Inspect only this disposable directory:

   ```sh
   demo="$XDG_DATA_HOME/org.graphdesk.desktop/graphs/GraphDesk Demo.sqlite3"
   test -f "$demo"
   find "$base/data" -type f -name 'GraphDesk Demo.sqlite3' -print
   sqlite3 -readonly "$demo" 'SELECT id FROM graph_identity; SELECT count(*) FROM pages; SELECT count(*) FROM blocks; PRAGMA integrity_check; PRAGMA foreign_key_check;'
   "$binary"
   ```

   Expect the same active demo and edited text. Close and repeat the SELECT query:
   identity/counts must be unchanged on a same-day restart; sample data must not
   duplicate. A subsequent day's ordinary journal creation is not demo reseeding.
2. Launch `"$binary"`; via the anchored switcher/native Create, create
   `$base/A.sqlite3` and `$base/B.sqlite3`. Both must appear immediately. Enter
   distinct text including Persian in each; A → B → A must preserve exact text,
   correct current badge and active database. Switch immediately after typing,
   then test repeated Ctrl+Tab, Ctrl+Shift+Tab, Enter, Escape, arrows and clicking.
   Escape must return caret/focus without mutation; switching must focus the new
   graph. Test with a real IME candidate/preedit cycle: no graph shortcut during
   composition. Test Settings/recovery dialogs do not trigger the switcher.
3. Open both native Open/Create dialogs and cancel each. Graph identity, list,
   focus, text and files must remain unchanged. Try Create on the existing A path:
   no overwrite, even after the native replacement prompt. Test Light/Dark, Persian
   RTL and a small window: no workspace shift or clipped controls; recovery notices
   stay accessible. Create same-named graphs in different disposable subdirectories
   and confirm distinct labels/tooltips and correct target activation.
4. Activate A, close normally, and move **only the closed disposable A**:

   ```sh
   test ! -e "$base/A.sqlite3-wal" && test ! -e "$base/A.sqlite3-shm"
   test ! -e "$base/A-moved.sqlite3" && mv -n -- "$base/A.sqlite3" "$base/A-moved.sqlite3"
   "$binary"
   ```

   Expect chooser/error, no new A file and no fallback to another database. Native
   Open of `$base/A-moved.sqlite3` must preserve identity/text and update the list.
5. Disable reopening but keep remembering, then restart: chooser, same registry,
   no demo regeneration. Disable remembering, open A/B: session switching still
   works; restart: no stored paths, no demo regeneration. Persist Persian/Jalali
   preferences and restart: unchanged. For recovery, use the existing disposable
   writer-lock procedure in [testing](testing.md#recovery-recurrence-investigation--disposable-data-only):
   retain a captured draft, restart, verify actionable inventory/comparison and no
   replacement/deletion of its unresolved text. A blocked save or pending first-note
   form must block graph departure without discarding text. Do not inject failures
   into real graphs or personal recovery stores.

Record commit/hash, Fedora/GTK/WebKit versions, actual Wayland/XWayland backend,
pass/fail/not-tested per step, and exact text/identity/integrity after closing.
Legacy coordinate-based WebView suites target the previous chooser; they require
focused adaptation and are **not** passing evidence for this UI. No native Fedora,
IME or package-install result is claimed by this handoff. Do not mark M8 complete.
