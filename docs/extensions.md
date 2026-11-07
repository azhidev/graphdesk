# M9 bundled extension foundation

Implemented: a bundled registry, graph-scoped lifecycle, versioned host contract,
namespaced views/navigation and bounded Rust-owned storage. This is an **internal
foundation, not a public SDK or third-party runtime**. No dynamic imports, local
folder loading, installer, marketplace, Routines or Finance implementation.

## Working example

Open a graph, then Settings → Extensions → enable **Tally**. Its navigation entry
opens a compact counter: Add one, Subtract one and Reset count. The count updates
only after acknowledgement. Enablement and count survive restart; other graphs
start separately disabled with zero. Disable removes navigation, revokes the host
context and retains saved data; enabling restores it. An unavailable view points
to Settings. Extension errors do not block Page/Journal navigation or draft saving.

The example has no unsaved text, timer or background work: no new recovery system.
Its `{schema:1,count:number}` document rejects unknown/invalid versions without
overwriting them. Failures/conflicts retain data; explicit retry re-reads it and
never automatically replays an uncertain write.

## Registration and lifecycle

`contracts.ts` defines JSON-compatible payloads and a typed API. `bundled.ts`
explicitly registers `tally.ts`; registration performs no graph IO or activation.
Manifests contain a lowercase ASCII reverse-DNS ID (`graphdesk.tally`), package
version, exact supported API integer (`1`), English/Persian metadata and capabilities.
Invalid, duplicate and incompatible manifests fail. IDs are independent of paths
and translated labels. Package versions do not silently change API compatibility.

`ExtensionHost` owns one graph-session lifetime. On acknowledged installation it
synchronously revokes/disposes the previous context, clears contributions, reads
preferences and activates enabled definitions. Failed graph preparation retains
the source context. Generation checks reject late state/activation/configuration/
command replies; captured API closures cannot operate after revocation. A late
activation result is immediately disposed. Shutdown detaches too. Cleanup is
synchronous and exception-contained; this example owns no subscriptions.

Each instance permits one action in flight: rapid requests are ignored, not queued
or duplicated. Configuration persists first and changes the instance only after
acknowledgement. Activation/command errors revoke that instance, leaving other
extensions/editor usable. Explicit retry reads persisted state; disabled documents
are retained but inaccessible. Enable preferences are separate from activation:
an enabled extension can be failed/unavailable without losing its preference.

## Host API and UI boundary

| Capability | API v1 | Boundary |
| --- | --- | --- |
| `storage.document` | `read()`, `write(expectedRevision,value)` | One scoped JSON document |
| `ui.views` | `setView(model)` | Text/title/value and up to eight action IDs, no HTML/components |
| `ui.navigation` | `setNavigation(entries)` | Up to four entries referencing this extension's views |

Models are validated, copied and scoped by ID; extensions cannot replace built-in
navigation or other extensions' models. View/action IDs are local strings, never
callbacks embedded in JSON. At most four views, each at most 8,192 JSON characters;
localized strings are bounded to 512 characters. Instance `invoke(action)` and
`dispose()` are the in-process adapter, not Svelte APIs. They can later be RPC
messages without replacing these contracts. No public SDK is promised yet.

Host-rendered views use escaped Svelte text, existing design tokens, English/Persian
labels and RTL-safe values. Extension destinations reuse bounded navigation
history and flush/guard editor drafts before departure, like Tasks. The editor
stays mounted while hidden. History stores only IDs; a failed/disabled destination
does not silently advance history. Actions/settings obey shell/composition locks.

## Rust authority and schema 6

Additive migration `006_extensions.sql` creates a STRICT table keyed by graph UUID
and extension ID, FK-linked to `graph_identity`. It stores enabled, revision and
valid JSON. Defaults: disabled, revision 0, null. Disabling never deletes data.
There is no arbitrary SQL/query/general database API. Documents are bounded to
**16 KiB compact UTF-8 JSON**. Immediate transactions recheck enablement and compare
revision; stale writes conflict. Quota/validation/transaction failures leave data
and revision unchanged. The frontend rejects unsafe numeric revisions instead of
rounding them; Tally also bounds its count to safe JS integers.

Tauri exposes only `extension_state`, `extension_configure`, `extension_request`.
Requests strictly tag `storage_read` or `storage_write` with API 1 and bundled-ID
allowlisting. Existing blocking worker, session mutex and local-main-window ACL
apply. Session check and operation share a lock: old requests cannot write into a
new graph. Narrow editor forwarding deliberately avoids `graph_mut()` (which
clears undo). Extension writes are not block undo operations.

State is **inside the graph**, not private settings/recovery. Physical graph files
are isolated; copies inherit copied UUID/preferences/data, then evolve separately.
Use existing closed-graph/SQLite-aware backups. Historical SQL is unchanged;
migration 6 is transactional. Older binaries reject schema 6. Remembered schema-5
graphs may require explicit Open for the normal upgrade path; never clear profile
or recovery data to bypass upgrade safeguards.

## Trust boundary and future third-party requirements

Bundled modules are **trusted application code** compiled with GraphDesk. The API
provides no stores, SQLite connections, paths, unrestricted Tauri invoke, editor
mutations, filesystem or network handles. Tally imports contracts only. This is an
API boundary, **not a JavaScript sandbox**: same-realm code could still access
browser globals or malicious imports. Never import third-party code here. Hung
callbacks cannot be preempted; exception containment is not resource isolation.

Before local-folder JS/TS plugins, add:

1. Validated package/manifest/entrypoint format, compatibility negotiation,
   canonical-path traversal/symlink restrictions, size/provenance/update checks,
   and safe compilation/bundling. No discovery/evaluation exists now.
2. An isolated runtime without DOM/stores/Tauri bridge, enforced CSP/resource/time
   limits, supervision and termination. Evaluate separately; an ordinary worker
   is not automatically a sufficient sandbox.
3. Versioned RPC envelopes (request ID, principal, activation lease, method,
   payload, structured result/error), bounded queues and runtime input validation.
   Bind identity to authenticated endpoints, never caller-supplied IDs/tokens.
   Terminated/disabled contexts must lose storage capabilities.
4. User-visible per-graph capability grants/revocation and Rust enforcement of
   the resolved registry/grants, replacing the bundled allowlist—not unrestricted
   invoke. v1 declares scoped storage/declarative UI only, not network/fs/editor.
5. A supported SDK only after real consumers; adversarial tests for packages,
   spoofing, stale leases, crashes, hangs and resource abuse.

## Verification and Fedora acceptance

Node tests cover lifecycle/revocation, failures, rapid actions, restart/isolation
and navigation. Real SQLite fixtures cover quotas, CAS guards, constraints,
upgrade/reopen and rollback. Rust tests additionally cover undo preservation,
session/API/ID checks and origin/window ACL, but **Cargo is unavailable in this
VM**: compilation and these Rust tests require native validation. Python fixtures
are not Rust execution; WebKitGTK fixtures use synthetic IPC, not native SQLite.

```sh
xvfb-run -a -s '-screen 0 1300x1000x24 -nolisten tcp' dbus-run-session -- /usr/bin/python3 tests/webview/editor_flicker.py 3 1300 extensions
```

After manual review/commit, transfer the exact source via the approved workflow.
On Fedora run frontend check/build/tests, schema tests and both locked Rust
workspace test configurations, then rebuild Tauri. Use disposable XDG paths/two
disposable graphs, never personal recovery. Enable Tally in A, increment, disable/
enable, switch to B (disabled/zero), enable B, return to A, close/restart normally.
Check counts/preferences, Back/Forward, immediate departure with editor drafts,
Light/Dark, Persian RTL and narrow layout. Hold a writer lock only on a disposable
graph: failures must retain data and leave navigation/editor/recovery usable;
release and retry explicitly. Record source/binary hashes, WebKit and Wayland
details. Ubuntu fixture results are not native Fedora acceptance.
