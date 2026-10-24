# Workflows and keyboard reference

Create/open a graph, create/select a page, then choose **Add first block**.
Click a block to edit. Markdown is literal source; multiline paste stays in one block.

| Key | Behavior |
| --- | --- |
| Enter | Split at the caret/selection; children and metadata stay on the original UUID. |
| Shift+Enter | Newline within the block. |
| Tab / Shift+Tab | Indent under the previous sibling / outdent after the parent, with descendants. |
| Alt+Shift+Up / Down | Reorder siblings; toolbar alternatives are available. |
| Up / Down | Native movement, crossing blocks only at absolute text start/end without selection. |
| Backspace at nonempty text start | Focus previous visible block; never merge automatically. |
| Backspace on empty block | Outdent nested blocks; remove only metadata-free root leaves when another block exists. |
| Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z, Ctrl+Y | Undo/redo text and structural changes in the graph session. |
| Escape | Leave textarea for toolbar; Tab then navigates controls. |

Undo is page-scoped, globally bounded to 100 entries / approximately 8 MiB. Typing
coalesces; oversized edits save but clear that page's history. Reopening clears
history. External writes invalidate history; concurrent external editing is unsupported.
Collapse is session-local. Platform/compositor shortcuts may conflict: use toolbar controls.

The toolbar's **Delete…** action deletes the selected block and all descendants.
Nonempty blocks, subtrees, and blocks carrying task/property/reference metadata
require confirmation showing the total affected block count. Cancel (or Escape)
does nothing. A simple empty leaf needs no confirmation, including the last block
on a page; **Add first block** is then available. This explicit action is separate
from the conservative Backspace behavior above. Delete/undo/redo preserve UUIDs,
content, hierarchy, positions, timestamps, tasks, properties and reference bindings.
Undo is available only in the current graph session and remains subject to normal
history eviction. Deletions exceeding the retained undo budget are rejected without
changing data; delete smaller subtrees instead. No trash/recycle bin is implemented.

Deletion flushes drafts before preparing its count and locks editor navigation and
editing while confirmation is open. The core revalidates the acknowledged page and
its block metadata transactionally; a changed preview fails visibly and must be
requested again. A failed/uncertain deletion is never automatically repeated by Retry.

Typing autosaves after 300 ms idle, with a one-second maximum wait. Navigation
and normal close flush drafts. Save failures retain drafts and prevent silent
departure. Resolve the cause and use **Retry saves / clear error**. **Discard
unsaved drafts and reload** deliberately loses drafts. During IME composition,
structural shortcuts and leaving unfinished composition are blocked. End-to-end
IME behavior still needs native testing.

Finalized unsaved text also targets an app-private recovery snapshot within about
one second. The save status distinguishes snapshot-pending/protected state and
reports recovery failures. Preedit is neither saved nor recovery-captured. Startup
detects pending graph recovery stores; reopen the original graph and use **Compare /
Recover**. Recovery is explicit, text-only and revision-guarded; discard requires
confirmation. **Review local recovery drafts** refreshes the list. Unresolved
versions expire after 14 days; see [data and recovery limits](data.md#text-draft-recovery).

Journals use local Gregorian civil dates even if renamed. Search returns up to
100 page and 100 block phrase matches from acknowledged text, not drafts. Block
results reveal ancestors and focus the UUID. Unicode tokenization does not promise
stemming, transliteration, ZWNJ normalization or Arabic/Persian letter equivalence.

Type `[[` for up to eight page suggestions. Saved links offer navigation;
unresolved links offer explicit creation, never creation while typing. Resolved
links retain target UUIDs across rename without rewriting source text. New
occurrences resolve by current title; old titles are not aliases. Removing syntax
removes bindings; undo/redo restores them. Nested brackets, multiline titles,
aliases and escaping are unsupported. Duplicate normalized labels produce one edge.

Task changes (No task / TODO / DOING / DONE) flush text, save transactionally and
participate in undo/redo. Existing core CANCELLED values remain readable; the UI
does not create them. Properties remain core APIs, not a UI workflow.

There is no fixed model nesting-depth cap. Visual indentation stops increasing after
20 levels to preserve editing space, without flattening stored hierarchy. Traversal
and editor subtree deletion are iterative; deletion uses leaf-first SQL statements
inside one transaction. A 1,101-level delete/undo/redo fixture passes, not a guarantee
for arbitrary size/depth or interactive performance. SQLite/resource limits still
apply; failed writes roll back. Legacy core subtree/page cascade deletion can hit
SQLite's trigger-depth limit (typically around 1,000 levels). Large outlines render
without virtualization and structural actions read the page; use practical sizes.
