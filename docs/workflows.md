# Workflows and keyboard reference

Create/open a graph to land on today's local-date journal. Choose **Capture today**
(Ctrl/Cmd+Shift+J) to focus an empty root leaf or append a new block. Create/select
other pages through the sidebar; empty pages offer **Add first block**.
Click a block to edit. Markdown is literal source; multiline paste stays in one block.

**Previous day / Today / Next day** follow journal identity, including renamed
journals. The sidebar's **Journal date** uses the calendar selected in Settings. Browsing
an absent day shows an empty preview without creating a page, block or reference.
**Add a note** opens a small first-note form: write text and choose **Save first note**
to create the journal and its initial block atomically. Finalized form input targets
private recovery capture within 700 ms without creating graph records. Normal close
attempts capture but remains blocked until the form is resolved; discarding nonempty
text requires confirmation. Recovery status distinguishes pending/stored/failed.
Preedit and input not yet captured can still be lost on forced termination. After
reopening the graph, **Pending journal notes → Restore first-note form** explicitly
restores text for review, never automatic submission. Persisted text is compared;
modified/deleted committed notes are never silently overwritten/recreated. Retry
uses the same intended block UUID, not another append.

Graph opening creates today's journal even if empty. No automatic page switch occurs
at midnight; Today/Capture calculate the local day when invoked. A date-shaped ordinary
page is never silently converted: rename it before creating a journal for that date.
**Switch graph** opens the same chooser used at startup. Native **Open graph…** and
**Create graph…** dialogs supplement manual paths. Cancelling a picker does not
switch graphs. Creation never overwrites a file, even if a native dialog offers
replacement. Only trusted local graph files are supported.

**Remember recent graphs** and **Reopen last graph on startup** are initially off.
Remembering is explicit opt-in and retains at most ten successfully activated graph
paths/UUIDs privately on this computer. Reopening requires remembering. Forgetting
one entry, clearing the list, or turning remembering off never deletes graph or
recovery data. Turning it off clears all stored paths. Missing graphs are not
recreated; select their new path explicitly. Unexpected file identity blocks a
remembered open. Automatic reopening also refuses old/unsupported graph formats;
explicit opening retains the normal migration/backup precautions.

Actual page/date/task/graph departures flush drafts and respect unfinished
composition/errors. A pending first-note form must be submitted or explicitly
discarded first. Failed candidate opening keeps the old graph; successful opening
followed by a landing error shows the new graph and **Retry Today**. Registry errors
are reported separately. A second desktop launch focuses the existing window,
ignoring launch arguments rather than switching graphs behind an unfinished edit.

**Back / Forward** follows pages, source-block jumps, canonical journal dates
(including read-only previews), and Tasks with its filters. History is bounded to
100 locations, belongs to the current graph session, and resets only on successful
graph switching. A new departure after Back drops the forward branch. Same-page
block jumps update the current anchor, not a new entry. Rename and calendar/language
changes preserve destinations. Missing blocks fall back to the page heading; a
missing page or failed load leaves the current history position unchanged.

**Go to page** (Ctrl/Cmd+P) filters existing titles and displayed journal labels;
it does not search content or create pages. Use arrows/Enter or Escape to cancel.
It shows up to 100 matches. Ctrl/Cmd+O opens the native graph picker; existing
Ctrl/Cmd+K and Ctrl/Cmd+Shift+J retain search and capture. Alt+Left/Right navigates
history only outside editable fields, preserving native text navigation including
macOS Option+Arrow. Visible controls remain available for every essential action.

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

Journal identity uses local Gregorian civil dates even if renamed. Search returns up to
100 page and 100 block phrase matches from acknowledged text, not drafts. Block
results reveal ancestors and focus the UUID. Unicode tokenization does not promise
stemming or transliteration. Original mode preserves existing token/phrase semantics.
Optional Persian-friendly mode folds Arabic Yeh/Kaf and Persian/Arabic digits in
queries and derived indexes only. ZWNJ and spaces are boundaries; joined words
still differ from separated words. Title uniqueness and link resolution are unchanged.

Ctrl/Cmd+K focuses search. Input is debounced 200 ms and paused during composition;
query/graph changes immediately invalidate earlier results. Up/Down choose a result,
Enter opens it, and Escape clears search. Results remain ordinary keyboard-focusable
buttons, show the owning page and bounded previews, and refresh after acknowledged
edits/page changes. Search indexes persisted text, not drafts or private recovery.
Pages precede blocks; each group retains its existing FTS5 relevance ordering. A
100-result group shows a refinement hint rather than silently implying completeness.

The **Tasks** workspace defaults to Active (TODO + DOING), across pages, journals
and nested blocks. Filter by Active/TODO/DOING/DONE and optionally page. Open a result
to reveal/focus the original block; use its existing task selector and undo/redo.
There is no second task editor. Reads use the existing task index, at most 100 rows
per request; **Load more tasks** fetches the next batch. Ordering is page title key,
page UUID, block UUID, not priority or outline order. Refresh restarts pagination;
concurrent external writers remain unsupported. CANCELLED is readable in the source
editor but excluded from these filters.

Gregorian ISO civil days remain storage identity. Settings independently selects
English/Persian, Gregorian/Jalali and original/Persian-friendly search and remembers
each choice locally. Language changes never change the other choices. Year/month/day
entry accepts ASCII, Persian and Arabic digits and validates before navigation.
Jalali labels and selection use one conversion implementation. Unsupported attempts
show a localized message and retain the calendar/date; existing journals remain
accessible with their Gregorian identity displayed. Notes, page titles/UUIDs and
references are never rewritten for presentation. Today follows the computer's local
date, not a forced Tehran timezone. Interface direction follows language; text remains
automatic per block, while paths and technical identifiers stay LTR. Copy/paste never
inserts formatting or directional-control characters into notes.

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
