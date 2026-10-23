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

Typing autosaves after 300 ms idle, with a one-second maximum wait. Navigation
and normal close flush drafts. Save failures retain drafts and prevent silent
departure. Resolve the cause and use **Retry saves / clear error**. **Discard
unsaved drafts and reload** deliberately loses drafts. During IME composition,
structural shortcuts and leaving unfinished composition are blocked. End-to-end
IME behavior still needs native testing.

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
