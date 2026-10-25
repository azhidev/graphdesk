//! Single-writer editor. SQLite remains authoritative; history stores only scoped
//! row deltas, including timestamp/metadata guards, never a second persistent model.
use std::collections::{BTreeMap, HashMap, VecDeque};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{decode_block, move_block, parse_id, read_block, require_page,
    sibling_ids, Block, Error, Graph, Result};

const MAX_ENTRIES: usize = 100;
const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EditAction {
    Append,
    Save { id: Uuid, expected: String, content: String },
    SaveText { id: Uuid, expected: String, revision: i64, content: String },
    Task { id: Uuid, expected: Option<crate::TaskStatus>, status: Option<crate::TaskStatus> },
    Split { id: Uuid, start: usize, end: usize },
    Indent { id: Uuid },
    Outdent { id: Uuid },
    Reorder { id: Uuid, direction: i32 },
    RemoveEmpty { id: Uuid },
    Delete { id: Uuid, token: Uuid },
}

#[derive(Debug, Serialize)]
pub struct DeletePreview { pub token: Uuid, pub count: usize, pub requires_confirmation: bool }

struct DeletePlan { token: Uuid, page: Uuid, id: Uuid, before: Snapshot }

#[derive(Debug, Serialize)]
pub struct EditResult {
    pub changed: Vec<Block>,
    pub removed: Vec<Uuid>,
    pub focus_id: Option<Uuid>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub revisions: BTreeMap<Uuid, i64>,
}

/// History is in-memory, page-scoped and bounded across the entire editor by
/// 100 entries / 8 MiB (conservative retained-allocation accounting). Oversized
/// edits still persist, but clear that page's history rather than cross a gap.
pub struct Editor {
    graph: Graph,
    history: History,
    deletion: Option<DeletePlan>,
}

impl Editor {
    pub fn new(graph: Graph) -> Self {
        let history = History { version: data_version(&graph.conn).ok(), ..History::default() };
        Self { graph, history, deletion: None }
    }

    pub fn graph(&self) -> &Graph { &self.graph }

    pub fn prepare_delete(&mut self, page: Uuid, id: Uuid) -> Result<DeletePreview> {
        self.deletion = None;
        let tx = self.graph.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let before = snapshot(&tx, page)?;
        let root = before.get(&id).ok_or(Error::NotFound)?;
        let count = subtree_ids(&before, id)?.len();
        let requires_confirmation = count > 1 || !root.block.content.is_empty()
            || root.block.task_status.is_some() || !root.properties.is_empty()
            || !root.references.is_empty() || !root.inline.is_empty();
        tx.commit()?;
        let token = Uuid::new_v4();
        self.deletion = Some(DeletePlan { token, page, id, before });
        Ok(DeletePreview { token, count, requires_confirmation })
    }

    /// Creating an unrelated empty page cannot invalidate block history.
    pub fn create_page(&mut self, title: &str) -> Result<crate::Page> {
        let page = self.graph.create_page(title)?;
        self.resolve_history(&page);
        Ok(page)
    }

    pub fn rename_page(&mut self, id: Uuid, title: &str) -> Result<crate::Page> {
        self.graph.rename_page(id, title)?;
        let page = self.graph.pages()?.into_iter().find(|p| p.id == id).ok_or(Error::NotFound)?;
        self.resolve_history(&page);
        Ok(page)
    }

    pub fn journal(&mut self, date: &str) -> Result<crate::Page> {
        let page = self.graph.create_journal(date)?;
        self.resolve_history(&page);
        Ok(page)
    }

    pub fn find_journal(&self, date: &str) -> Result<Option<crate::Page>> {
        self.graph.lookup_journal(date)
    }

    fn resolve_history(&mut self, page: &crate::Page) {
        let Ok((_, key)) = crate::title_parts(&page.title) else { return; };
        for history in self.history.pages.values_mut() {
            for entry in history.undo.iter_mut().chain(history.redo.iter_mut()) {
                for delta in &mut entry.delta {
                    for record in delta.before.iter_mut().chain(delta.after.iter_mut()) {
                        for (label_key, _, target) in &mut record.inline {
                            if target.is_none() && *label_key == key { *target = Some(page.id.to_string()); }
                        }
                    }
                }
            }
        }
        self.history.enforce_limits();
    }

    /// Any non-editor mutation may invalidate replay preconditions.
    pub fn graph_mut(&mut self) -> &mut Graph {
        self.history.clear();
        &mut self.graph
    }

    pub fn history_state(&self, page_id: Uuid) -> (bool, bool) {
        // Read-only callers cannot clear history, but never advertise stale entries.
        if data_version(&self.graph.conn).ok() != self.history.version { return (false, false); }
        self.history.state(page_id)
    }

    pub fn apply(&mut self, page_id: Uuid, action: EditAction, group: Option<Uuid>) -> Result<EditResult> {
        let tx = self.graph.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        self.history.check_version(&tx)?;
        let action = if let EditAction::SaveText { id, expected, revision, content } = action {
            let current: Option<i64> = tx.query_row("SELECT r.revision FROM block_revisions r JOIN blocks b ON b.id=r.block_id WHERE b.id=?1 AND b.page_id=?2",
                params![id.to_string(), page_id.to_string()], |r| r.get(0)).optional()?;
            if current != Some(revision) { return Err(Error::Conflict("text revision changed; compare persisted text before recovering".into())); }
            EditAction::Save { id, expected, content }
        } else { action };
        // Debounced text saves read/retain only the touched row and its metadata.
        // Structural actions may need the complete page to compute ordering deltas.
        let scope = match &action { EditAction::Save { id, .. } | EditAction::Task { id, .. } => Some(*id), _ => None };
        let before = snapshot_scope(&tx, page_id, scope)?;
        if let EditAction::Delete { id, token } = &action {
            let plan = self.deletion.take().ok_or_else(|| Error::Conflict("prepare deletion again".into()))?;
            if plan.token != *token || plan.page != page_id || plan.id != *id || plan.before != before {
                return Err(Error::Conflict("outline changed since deletion preview; review it again".into()));
            }
        }
        let save_group = match &action {
            EditAction::Save { id, .. } => group.map(|g| (page_id, *id, g)),
            _ => None,
        };
        let split_bindings = match &action {
            EditAction::Split { id, .. } => before.get(id).map(|r| r.inline.clone()),
            _ => None,
        };
        let (focus_before, focus_after) = apply_action(&tx, page_id, &before, action)?;
        let interim = snapshot_scope(&tx, page_id, scope)?;
        for (id, record) in &interim {
            if before.get(id).map(|r| &r.block.content) != Some(&record.block.content) {
                let saved = before.get(id).map(|r| r.inline.as_slice()).or(split_bindings.as_deref());
                crate::references::sync(&tx, *id, &record.block.content, saved)?;
            }
        }
        let after = snapshot_scope(&tx, page_id, scope)?;
        let delta = differences(&before, &after);
        let mut result = result_diff(&before, &after, focus_after);
        for record in after.values() {
            if scope.is_some() || before.get(&record.block.id) != Some(record) {
                result.revisions.insert(record.block.id, tx.query_row("SELECT revision FROM block_revisions WHERE block_id=?1", [record.block.id.to_string()], |r| r.get(0))?);
            }
        }
        let entry = Entry { delta, focus_before, focus_after, serial: 0 };
        // A destructive action must never commit without a retainable undo entry.
        if before.len() > after.len() && entry.bytes()
            + self.history.container_bytes() + 8192 > MAX_BYTES {
            return Err(Error::Invalid("deletion exceeds the session undo budget; delete a smaller subtree".into()));
        }
        tx.commit()?;
        // History changes only after commit, including coalescing and redo invalidation.
        if !entry.delta.is_empty() {
            self.history.push(page_id, entry, save_group);
        } else if self.history.last_save != save_group {
            self.history.last_save = None;
        }
        (result.can_undo, result.can_redo) = self.history.state(page_id);
        Ok(result)
    }

    pub fn history(&mut self, page_id: Uuid, redo: bool) -> Result<EditResult> {
        let tx = self.graph.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        self.history.check_version(&tx)?;
        let before = snapshot(&tx, page_id)?;
        let entry = self.history.pages.get(&page_id).and_then(|h| {
            if redo { h.redo.last() } else { h.undo.back() }
        });
        let Some(entry) = entry else {
            tx.commit()?;
            let (can_undo, can_redo) = self.history.state(page_id);
            return Ok(EditResult { changed: vec![], removed: vec![], focus_id: None, can_undo, can_redo, revisions: BTreeMap::new() });
        };
        replay(&tx, &before, entry, redo)?;
        let after = snapshot(&tx, page_id)?;
        let focus = if redo { entry.focus_after } else { entry.focus_before };
        let mut result = result_diff(&before, &after, focus);
        result.revisions = crate::text_revisions(&tx, page_id)?;
        tx.commit()?;
        let history = self.history.pages.get_mut(&page_id).expect("entry has page history");
        if redo {
            history.undo.push_back(history.redo.pop().expect("entry exists"));
        } else {
            history.redo.push(history.undo.pop_back().expect("entry exists"));
        }
        self.history.last_save = None;
        // Moving entries between stacks can grow their retained capacities too.
        self.history.enforce_limits();
        (result.can_undo, result.can_redo) = self.history.state(page_id);
        Ok(result)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Record {
    block: Block,
    created_at: i64,
    updated_at: i64,
    properties: Vec<(String, String)>,
    references: Vec<String>,
    inline: Vec<(String, String, Option<String>)>,
}

type Snapshot = BTreeMap<Uuid, Record>;

// Snapshot reads are transient. Only differing rows are retained in history.
fn snapshot(conn: &Connection, page: Uuid) -> Result<Snapshot> {
    snapshot_scope(conn, page, None)
}

fn snapshot_scope(conn: &Connection, page: Uuid, only: Option<Uuid>) -> Result<Snapshot> {
    require_page(conn, page)?;
    let mut records = BTreeMap::new();
    // Both variants use two bindings; the single-row variant can use the UUID index.
    let filter = if only.is_some() { "b.page_id=?1 AND b.id=?2" } else { "b.page_id=?1 AND ?2 IS NULL" };
    let page = page.to_string();
    let only = only.map(|id| id.to_string());
    let mut stmt = conn.prepare(&format!("SELECT b.id,b.page_id,b.parent_id,b.position,b.content,b.task_status,b.created_at,b.updated_at
        FROM blocks b WHERE {filter}"))?;
    let rows = stmt.query_map(params![page, only], |r| Ok((read_block(r)?, r.get(6)?, r.get(7)?)))?;
    for row in rows {
        let (raw, created_at, updated_at) = row?;
        let block = decode_block(raw)?;
        let inline = crate::references::bindings(conn, block.id)?;
        records.insert(block.id, Record { block, created_at, updated_at, properties: vec![], references: vec![], inline });
    }
    let mut stmt = conn.prepare(&format!("SELECT p.block_id,p.key,p.value_json FROM block_properties p
        JOIN blocks b ON b.id=p.block_id WHERE {filter} ORDER BY p.block_id,p.key"))?;
    let rows = stmt.query_map(params![page, only], |r| Ok((r.get::<_, String>(0)?, r.get(1)?, r.get(2)?)))?;
    for row in rows {
        let (id, key, value) = row?;
        records.get_mut(&parse_id(&id)?).expect("joined block").properties.push((key, value));
    }
    let mut stmt = conn.prepare(&format!("SELECT r.source_block_id,r.target_page_id FROM page_references r
        JOIN blocks b ON b.id=r.source_block_id WHERE {filter} ORDER BY r.source_block_id,r.target_page_id"))?;
    let rows = stmt.query_map(params![page, only], |r| Ok((r.get::<_, String>(0)?, r.get(1)?)))?;
    for row in rows {
        let (id, target) = row?;
        records.get_mut(&parse_id(&id)?).expect("joined block").references.push(target);
    }
    Ok(records)
}

fn block(records: &Snapshot, id: Uuid) -> Result<&Block> {
    // A block from a different page is intentionally indistinguishable from missing.
    records.get(&id).map(|r| &r.block).ok_or(Error::NotFound)
}

// Iterative parent-first traversal; no Rust call-stack depth limit.
fn subtree_ids(records: &Snapshot, root: Uuid) -> Result<Vec<Uuid>> {
    block(records, root)?;
    let mut children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for record in records.values() {
        if let Some(parent) = record.block.parent_id { children.entry(parent).or_default().push(record.block.id); }
    }
    let mut result = vec![];
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if !seen.insert(id) { return Err(Error::Invalid("cyclic outline".into())); }
        result.push(id);
        stack.extend(children.get(&id).into_iter().flatten().copied());
    }
    Ok(result)
}

fn delete_rows(conn: &Connection, records: &Snapshot, ids: &[Uuid]) -> Result<()> {
    let selected: std::collections::HashSet<_> = ids.iter().copied().collect();
    if records.values().any(|r| r.block.parent_id.is_some_and(|p| selected.contains(&p)) && !selected.contains(&r.block.id)) {
        return Err(Error::Conflict("history deletion would remove unexpected children".into()));
    }
    // Leaf-first avoids recursive FK cascades even for very deep outlines.
    let mut ordered = vec![];
    for id in ids {
        if records.get(id).is_some_and(|r| r.block.parent_id.is_none_or(|p| !selected.contains(&p))) {
            ordered.extend(subtree_ids(records, *id)?);
        }
    }
    if ordered.len() != ids.len() { return Err(Error::Invalid("invalid deletion topology".into())); }
    for id in ordered.into_iter().rev() { conn.execute("DELETE FROM blocks WHERE id=?1", [id.to_string()])?; }
    Ok(())
}

fn siblings(conn: &Connection, page: Uuid, parent: Option<Uuid>) -> Result<Vec<String>> {
    sibling_ids(conn, &page.to_string(), parent.map(|p| p.to_string()).as_deref())
}

fn insert(conn: &Connection, page: Uuid, parent: Option<Uuid>, content: &str) -> Result<Uuid> {
    let id = Uuid::new_v4();
    conn.execute("INSERT INTO blocks(id,page_id,parent_id,position,content)
        SELECT ?1,?2,?3,coalesce(max(position)+1,0),?4 FROM blocks WHERE page_id=?2 AND parent_id IS ?3",
        params![id.to_string(), page.to_string(), parent.map(|p| p.to_string()), content])?;
    Ok(id)
}

fn apply_action(conn: &Connection, page: Uuid, records: &Snapshot, action: EditAction)
    -> Result<(Option<Uuid>, Option<Uuid>)>
{
    if let EditAction::Append = action {
        let previous = siblings(conn, page, None)?.last().map(|s| parse_id(s)).transpose()?;
        return Ok((previous, Some(insert(conn, page, None, "")?)));
    }
    let id = match &action {
        EditAction::Append | EditAction::SaveText { .. } => unreachable!(),
        EditAction::Save { id, .. } | EditAction::Task { id, .. } | EditAction::Split { id, .. } | EditAction::Indent { id }
        | EditAction::Outdent { id } | EditAction::Reorder { id, .. } | EditAction::RemoveEmpty { id } | EditAction::Delete { id, .. } => *id,
    };
    let original = block(records, id)?;
    let mut focus = id;
    match action {
        EditAction::Append | EditAction::SaveText { .. } => unreachable!(),
        EditAction::Delete { .. } => {
            let ids = subtree_ids(records, id)?;
            let roots = siblings(conn, page, original.parent_id)?;
            let index = roots.iter().position(|s| s == &id.to_string()).expect("original sibling");
            let next = index.checked_sub(1).and_then(|i| roots.get(i)).or_else(|| roots.get(index + 1));
            let focus = next.map(|s| parse_id(s)).transpose()?.or(original.parent_id);
            delete_rows(conn, records, &ids)?;
            return Ok((Some(id), focus));
        }
        EditAction::Task { expected, status, .. } => {
            if original.task_status.as_deref() != expected.map(crate::TaskStatus::as_str) {
                return Err(Error::Conflict("task status changed".into()));
            }
            if original.task_status.as_deref() != status.map(crate::TaskStatus::as_str) {
                conn.execute("UPDATE blocks SET task_status=?2,updated_at=unixepoch() WHERE id=?1",
                    params![id.to_string(), status.map(crate::TaskStatus::as_str)])?;
            }
        }
        EditAction::Save { expected, content, .. } => {
            if original.content != expected { return Err(Error::Conflict("block content changed".into())); }
            if content != expected {
                conn.execute("UPDATE blocks SET content=?2,updated_at=unixepoch() WHERE id=?1", params![id.to_string(), content])?;
            }
        }
        EditAction::Split { start, end, .. } => {
            if start > end { return Err(Error::Invalid("split range is reversed".into())); }
            let start = utf16_boundary(&original.content, start)?;
            let end = utf16_boundary(&original.content, end)?;
            conn.execute("UPDATE blocks SET content=?2,updated_at=unixepoch() WHERE id=?1",
                params![id.to_string(), &original.content[..start]])?;
            focus = insert(conn, page, original.parent_id, &original.content[end..])?;
            let ids = siblings(conn, page, original.parent_id)?;
            let index = ids.iter().position(|s| s == &id.to_string()).expect("original sibling");
            move_block(conn, focus, original.parent_id, index + 1)?;
        }
        EditAction::Indent { .. } => {
            let ids = siblings(conn, page, original.parent_id)?;
            let index = ids.iter().position(|s| s == &id.to_string()).expect("original sibling");
            if index > 0 {
                let parent = parse_id(&ids[index - 1])?;
                move_block(conn, id, Some(parent), siblings(conn, page, Some(parent))?.len())?;
            }
        }
        EditAction::Outdent { .. } => {
            if let Some(parent) = original.parent_id {
                let grandparent = block(records, parent)?.parent_id;
                let ids = siblings(conn, page, grandparent)?;
                let index = ids.iter().position(|s| s == &parent.to_string()).expect("parent sibling");
                move_block(conn, id, grandparent, index + 1)?;
            }
        }
        EditAction::Reorder { direction, .. } => {
            if ![-1, 1].contains(&direction) { return Err(Error::Invalid("direction must be -1 or 1".into())); }
            let ids = siblings(conn, page, original.parent_id)?;
            let index = ids.iter().position(|s| s == &id.to_string()).expect("original sibling");
            if let Some(target) = index.checked_add_signed(direction as isize).filter(|i| *i < ids.len()) {
                move_block(conn, id, original.parent_id, target)?;
            }
        }
        EditAction::RemoveEmpty { .. } => {
            let record = &records[&id];
            if original.parent_id.is_some() || !original.content.is_empty() || original.task_status.is_some()
                || !record.properties.is_empty() || !record.references.is_empty()
                || records.values().any(|r| r.block.parent_id == Some(id)) || records.len() <= 1
            {
                return Err(Error::Invalid("only an empty, metadata-free root leaf other than the last block can be removed".into()));
            }
            let roots = siblings(conn, page, None)?;
            let index = roots.iter().position(|s| s == &id.to_string()).expect("root sibling");
            focus = parse_id(&roots[if index > 0 { index - 1 } else { 1 }])?;
            conn.execute("DELETE FROM blocks WHERE id=?1", [id.to_string()])?;
            // Keep existing positions (harmless gaps); undo restores the exact position.
        }
    }
    Ok((Some(id), Some(focus)))
}

fn utf16_boundary(text: &str, offset: usize) -> Result<usize> {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units == offset { return Ok(byte); }
        units += ch.len_utf16();
        if units > offset { break; }
    }
    if units == offset { return Ok(text.len()); }
    Err(Error::Invalid("split offset is outside text or inside a UTF-16 surrogate pair".into()))
}

#[derive(Debug)]
struct Delta { id: Uuid, before: Option<Record>, after: Option<Record> }

struct Entry {
    delta: Vec<Delta>,
    focus_before: Option<Uuid>,
    focus_after: Option<Uuid>,
    serial: u64,
}

fn differences(before: &Snapshot, after: &Snapshot) -> Vec<Delta> {
    let mut ids: Vec<_> = before.keys().chain(after.keys()).copied().collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter().filter(|id| before.get(id) != after.get(id))
        .map(|id| Delta { id, before: before.get(&id).cloned(), after: after.get(&id).cloned() }).collect()
}

fn result_diff(before: &Snapshot, after: &Snapshot, focus_id: Option<Uuid>) -> EditResult {
    let mut changed: Vec<_> = after.iter().filter(|(id, r)| before.get(id).map(|b| &b.block) != Some(&r.block))
        .map(|(_, r)| r.block.clone()).collect();
    changed.sort_by_key(|b| (b.parent_id, b.position, b.id));
    let removed = before.keys().filter(|id| !after.contains_key(id)).copied().collect();
    EditResult { changed, removed, focus_id, can_undo: false, can_redo: false, revisions: BTreeMap::new() }
}

fn replay(conn: &Connection, current: &Snapshot, entry: &Entry, redo: bool) -> Result<()> {
    for delta in &entry.delta {
        let expected = if redo { &delta.before } else { &delta.after };
        // Late resolution is derived metadata, not a conflicting external text edit.
        let equivalent = match (current.get(&delta.id), expected.as_ref()) {
            (Some(a), Some(b)) => a.block == b.block && a.created_at == b.created_at
                && a.updated_at == b.updated_at && a.properties == b.properties && a.references == b.references,
            (None, None) => true,
            _ => false,
        };
        if !equivalent {
            return Err(Error::Conflict("history row or metadata changed".into()));
        }
    }
    // Park only touched rows at distinct positions before restoring their ordering.
    // A supported graph has nonnegative positions; any collision fails atomically.
    for (i, delta) in entry.delta.iter().enumerate() {
        if current.contains_key(&delta.id) {
            conn.execute("UPDATE blocks SET position=?2 WHERE id=?1", params![delta.id.to_string(), i64::MIN + i as i64])?;
        }
    }
    let deleting: Vec<_> = entry.delta.iter().filter(|d| (if redo { &d.after } else { &d.before }).is_none() && current.contains_key(&d.id)).map(|d| d.id).collect();
    delete_rows(conn, current, &deleting)?;
    // Restore missing parents before their children, independent of UUID ordering.
    let mut pending: Vec<_> = entry.delta.iter().collect();
    let mut existing: std::collections::HashSet<_> = current.keys().filter(|id| !deleting.contains(id)).copied().collect();
    while !pending.is_empty() {
      let previous = pending.len();
      let mut remaining = vec![];
      for delta in pending {
        let target = if redo { &delta.after } else { &delta.before };
        if let Some(record) = target {
            let b = &record.block;
            if b.parent_id.is_some_and(|p| !existing.contains(&p)) { remaining.push(delta); continue; }
            if current.contains_key(&delta.id) {
                conn.execute("UPDATE blocks SET parent_id=?2,position=?3,content=?4,task_status=?5,created_at=?6,updated_at=?7 WHERE id=?1",
                    params![b.id.to_string(), b.parent_id.map(|p| p.to_string()), b.position, b.content, b.task_status, record.created_at, record.updated_at])?;
            } else {
                conn.execute("INSERT INTO blocks(id,page_id,parent_id,position,content,task_status,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![b.id.to_string(), b.page_id.to_string(), b.parent_id.map(|p| p.to_string()), b.position, b.content, b.task_status, record.created_at, record.updated_at])?;
                for (key, value) in &record.properties {
                    conn.execute("INSERT INTO block_properties(block_id,key,value_json) VALUES (?1,?2,?3)", params![b.id.to_string(), key, value])?;
                }
                for target in &record.references {
                    conn.execute("INSERT INTO page_references(source_block_id,target_page_id) VALUES (?1,?2)", params![b.id.to_string(), target])?;
                }
            }
            crate::references::sync(conn, b.id, &b.content, Some(&record.inline))?;
            existing.insert(b.id);
        }
      }
      if remaining.len() == previous { return Err(Error::Conflict("history parents are missing or cyclic".into())); }
      pending = remaining;
    }
    Ok(())
}

#[derive(Default)]
struct PageHistory { undo: VecDeque<Entry>, redo: Vec<Entry> }

#[derive(Default)]
struct History {
    pages: HashMap<Uuid, PageHistory>,
    last_save: Option<(Uuid, Uuid, Uuid)>,
    version: Option<i64>,
    serial: u64,
}

impl History {
    fn container_bytes(&self) -> usize {
        self.pages.capacity() * (std::mem::size_of::<(Uuid, PageHistory)>() + 32)
            + self.pages.values().map(|h| (h.undo.capacity() + h.redo.capacity()) * std::mem::size_of::<Entry>()).sum::<usize>()
    }
    fn clear(&mut self) { self.pages.clear(); self.last_save = None; }

    fn check_version(&mut self, conn: &Connection) -> Result<()> {
        let version = data_version(conn)?;
        if self.version != Some(version) { self.clear(); self.version = Some(version); }
        Ok(())
    }

    fn state(&self, page: Uuid) -> (bool, bool) {
        self.pages.get(&page).map(|h| (!h.undo.is_empty(), !h.redo.is_empty())).unwrap_or_default()
    }

    fn push(&mut self, page: Uuid, mut entry: Entry, group: Option<(Uuid, Uuid, Uuid)>) {
        self.serial += 1;
        entry.serial = self.serial;
        let h = self.pages.entry(page).or_default();
        h.redo.clear();
        let merge = group.is_some() && self.last_save == group && h.undo.back().is_some_and(|previous| {
            previous.delta.len() == 1 && entry.delta.len() == 1
                && previous.delta[0].id == entry.delta[0].id && previous.delta[0].after == entry.delta[0].before
        });
        if merge {
            let previous = h.undo.back_mut().expect("merge entry");
            previous.delta[0].after = entry.delta.pop().expect("one save row").after;
            previous.focus_after = entry.focus_after;
            previous.serial = entry.serial;
        } else {
            h.undo.push_back(entry);
        }
        self.last_save = group;
        if h.undo.back().is_some_and(|e| e.bytes() > MAX_BYTES) {
            // No older undo may jump across an unrecorded edit.
            self.pages.remove(&page);
            self.last_save = None;
        }
        self.enforce_limits();
    }

    fn enforce_limits(&mut self) {
        loop {
            let entries: Vec<_> = self.pages.iter().flat_map(|(page, h)|
                h.undo.iter().map(move |e| (*page, false, e.serial, e.bytes()))
                    .chain(h.redo.iter().map(move |e| (*page, true, e.serial, e.bytes())))).collect();
            let containers = self.container_bytes();
            if entries.is_empty() || (entries.len() <= MAX_ENTRIES && containers + entries.iter().map(|e| e.3).sum::<usize>() <= MAX_BYTES) { break; }
            let (page, redo, _, _) = *entries.iter().min_by_key(|e| e.2).expect("over limit");
            let h = self.pages.get_mut(&page).expect("history page");
            if redo {
                // Dropping the next redo invalidates the rest of that redo chain.
                h.redo.clear();
            } else {
                h.undo.pop_front();
            }
        }
        self.pages.retain(|_, h| !h.undo.is_empty() || !h.redo.is_empty());
    }
}

impl Entry {
    fn bytes(&self) -> usize {
        // Includes allocation capacities, fixed row/entry storage and a conservative
        // allowance for allocator and per-page container overhead.
        1024 + self.delta.capacity() * std::mem::size_of::<Delta>() + self.delta.iter().map(|d|
            d.before.iter().chain(d.after.iter()).map(|r|
                r.block.content.capacity() + r.block.task_status.as_ref().map_or(0, String::capacity)
                + r.properties.capacity() * std::mem::size_of::<(String, String)>()
                + r.properties.iter().map(|(k, v)| k.capacity() + v.capacity() + 64).sum::<usize>()
                + r.references.capacity() * std::mem::size_of::<String>()
                + r.references.iter().map(|s| s.capacity() + 32).sum::<usize>()
                + r.inline.capacity() * std::mem::size_of::<(String, String, Option<String>)>()
                + r.inline.iter().map(|(k,l,t)| k.capacity() + l.capacity() + t.as_ref().map_or(0, String::capacity) + 64).sum::<usize>()
            ).sum::<usize>()
        ).sum::<usize>()
    }
}

fn data_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA data_version", [], |r| r.get(0))?)
}

#[cfg(test)]
mod tests;
