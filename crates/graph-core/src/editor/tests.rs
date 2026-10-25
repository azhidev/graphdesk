use super::*;

#[test]
fn oversized_initial_note_saves_like_other_nondestructive_edits() -> Result<()> {
    let mut e = Editor::new(Graph::in_memory()?);
    let id = Uuid::new_v4();
    let content = "x".repeat(MAX_BYTES);
    let first = e.submit_note("2024-02-29", id, &content)?;
    assert_eq!(first.block.content, content);
    assert_eq!(e.history_state(first.page.id), (false, false));
    let retry = e.submit_note("2024-02-29", id, &content)?;
    assert_eq!(retry.block.id, id);
    assert_eq!(e.graph.page_blocks(first.page.id)?.len(), 1);
    assert_eq!(e.note_state(id)?.revision, Some(0));
    Ok(())
}

#[test]
fn atomic_note_submission_retry_undo_revisions_and_reopen() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("notes.db");
    let mut e = Editor::new(Graph::create(&path)?);
    let target = e.create_page("Target")?;
    let id = Uuid::new_v4();
    let raw = "كتاب می\u{200c}روم ۱۲٣ [[Target]] 😀";
    let first = e.submit_note("2024-02-29", id, raw)?;
    assert_eq!(first.revision, 0);
    assert_eq!(first.block.id, id);
    assert_eq!(first.block.content, raw);
    assert_eq!(first.page.journal_date.as_deref(), Some("2024-02-29"));
    // Simulate a lost acknowledgement: exact retry returns identities, no write/history.
    let retry = e.submit_note("2024-02-29", id, raw)?;
    assert_eq!(retry.page, first.page);
    assert_eq!(retry.block, first.block);
    assert_eq!(e.graph.incoming_links(target.id)?.len(), 1);
    assert_eq!(e.graph.page_blocks(first.page.id)?.len(), 1);
    assert_eq!(e.graph.search_mode("کتاب می روم 123", true)?.len(), 1);
    assert!(e.graph.search("کتاب")?.is_empty());
    e.history(first.page.id, false)?;
    assert!(e.graph.page_blocks(first.page.id)?.is_empty());
    assert!(e.graph.lookup_journal("2024-02-29")?.is_some());
    assert!(e.note_state(id)?.deleted);
    assert_eq!(e.note_state(id)?.revision, Some(1));
    assert!(matches!(e.submit_note("2024-02-29", id, raw), Err(Error::Conflict(_))));
    assert!(e.graph.search_mode("کتاب", true)?.is_empty());
    e.history(first.page.id, true)?;
    assert_eq!(e.note_state(id)?.revision, Some(2));
    assert!(matches!(e.submit_note("2024-02-29", id, raw), Err(Error::Conflict(_))));
    assert_eq!(e.graph.search_mode("کتاب", true)?.len(), 1);
    drop(e);
    let mut e = Editor::new(Graph::open(path)?);
    assert_eq!(e.note_state(id)?.block, Some(first.block));
    assert!(matches!(e.submit_note("2024-02-29", id, raw), Err(Error::Conflict(_))));
    assert!(e.note_state(Uuid::new_v4())?.revision.is_none());
    Ok(())
}

#[test]
fn initial_note_lost_ack_reconciles_after_reopen_without_duplicate() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("lost-ack.db");
    let id = Uuid::new_v4();
    let first = {
        let mut e = Editor::new(Graph::create(&path)?);
        e.submit_note("2024-02-29", id, "durable initial text")?
    };
    let mut e = Editor::new(Graph::open(path)?);
    let retry = e.submit_note("2024-02-29", id, "durable initial text")?;
    assert_eq!(retry.page,first.page);
    assert_eq!(retry.block,first.block);
    assert_eq!(retry.revision,0);
    assert_eq!(e.graph.pages()?.len(),1);
    assert_eq!(e.graph.page_blocks(first.page.id)?.len(),1);
    // Session-only history must not be fabricated by a reconciliation read.
    assert_eq!(e.history_state(first.page.id),(false,false));
    Ok(())
}

#[test]
fn note_retries_reject_modified_deleted_aba_and_wrong_page() -> Result<()> {
    let mut e = Editor::new(Graph::in_memory()?);
    let id = Uuid::new_v4();
    let note = e.submit_note("2024-02-29", id, "first")?;
    assert!(matches!(e.submit_note("2024-03-01", id, "first"), Err(Error::Conflict(_))));
    assert!(e.graph.lookup_journal("2024-03-01")?.is_none());
    assert!(matches!(e.submit_note("2024-02-29", id, "other"), Err(Error::Conflict(_))));
    e.apply(note.page.id, EditAction::SaveText { id, expected: "first".into(), revision: 0, content: "other".into() }, None)?;
    assert!(matches!(e.submit_note("2024-02-29", id, "first"), Err(Error::Conflict(_))));
    e.history(note.page.id, false)?;
    assert!(matches!(e.submit_note("2024-02-29", id, "first"), Err(Error::Conflict(_))));
    delete(&mut e, note.page.id, id)?;
    assert!(matches!(e.submit_note("2024-02-29", id, "first"), Err(Error::Conflict(_))));
    let ordinary = e.create_page("Ordinary")?;
    let block = e.graph.create_block(ordinary.id, None, "same")?;
    assert!(matches!(e.submit_note("2024-03-02", block.id, "same"), Err(Error::Conflict(_))));
    assert!(e.graph.lookup_journal("2024-03-02")?.is_none());
    let collision = e.create_page("2024-03-03")?;
    assert!(e.submit_note("2024-03-03", Uuid::new_v4(), "no conversion").is_err());
    assert_eq!(e.graph.page_blocks(collision.id)?.len(), 0);
    for date in ["2024-2-29", "2023-02-29", "2024-02-30"] {
        assert!(e.submit_note(date, Uuid::new_v4(), "text").is_err());
    }
    assert!(e.submit_note("2024-03-04", Uuid::new_v4(), " \n ").is_err());
    Ok(())
}

#[test]
fn note_transaction_rolls_back_page_block_references_indexes_and_history() -> Result<()> {
    let mut e = Editor::new(Graph::in_memory()?);
    let target = e.create_page("Target")?;
    let id = Uuid::new_v4();
    e.graph.conn.execute_batch("CREATE TEMP TRIGGER fail_note AFTER INSERT ON inline_references BEGIN SELECT RAISE(ABORT,'injected'); END")?;
    assert!(e.submit_note("2024-02-29", id, "كتاب [[Target]]").is_err());
    assert_eq!(e.graph.pages()?.len(), 1);
    assert!(e.graph.lookup_journal("2024-02-29")?.is_none());
    assert!(e.note_state(id)?.revision.is_none());
    assert!(e.graph.search("2024-02-29")?.is_empty());
    assert!(e.graph.search_mode("کتاب", true)?.is_empty());
    assert!(e.graph.incoming_links(target.id)?.is_empty());
    e.graph.conn.execute_batch("DROP TRIGGER fail_note")?;
    let note = e.submit_note("2024-02-29", id, "كتاب [[Target]]")?;
    let renamed = e.rename_page(note.page.id, "Renamed day")?;
    let retry = e.submit_note("2024-02-29", id, "كتاب [[Target]]")?;
    assert_eq!(retry.page, renamed);
    let next = e.submit_note("2024-02-29", Uuid::new_v4(), "second")?;
    assert_eq!(next.page, renamed);
    assert_eq!(next.block.position, 1);
    e.history(renamed.id, false)?;
    assert_eq!(e.graph.page_blocks(renamed.id)?.len(), 1);
    e.history(renamed.id, false)?;
    assert!(e.graph.page_blocks(renamed.id)?.is_empty());
    Ok(())
}

#[test]
fn reliable_text_revisions_reject_stale_aba_and_noops_avoid_writes() -> Result<()> {
    let (mut e, page, id) = setup("base")?;
    let graph_id = e.graph.id()?;
    assert_eq!(e.graph.text_revisions(page)?[&id], 0);
    let r = e.apply(page, EditAction::SaveText { id, expected: "base".into(), revision: 0, content: "next".into() }, None)?;
    assert_eq!(r.revisions[&id], 1);
    assert!(e.apply(page, EditAction::SaveText { id, expected: "next".into(), revision: 0, content: "lost".into() }, None).is_err());
    e.history(page, false)?;
    assert_eq!(e.graph.text_revisions(page)?[&id], 2);
    assert!(e.apply(page, EditAction::SaveText { id, expected: "base".into(), revision: 0, content: "stale".into() }, None).is_err());
    let r = e.apply(page, EditAction::SaveText { id, expected: "base".into(), revision: 2, content: "base".into() }, None)?;
    assert_eq!(r.revisions[&id], 2);
    assert_eq!(e.graph.id()?, graph_id);
    Ok(())
}

fn delete(e: &mut Editor, page: Uuid, id: Uuid) -> Result<EditResult> {
    let preview = e.prepare_delete(page, id)?;
    e.apply(page, EditAction::Delete { id, token: preview.token }, None)
}

#[test]
fn subtree_delete_restores_exact_metadata_order_and_reopens() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("delete.sqlite3");
    let mut g = Graph::create(&path)?;
    let p = g.create_page("Outline")?;
    let target = g.create_page("Target")?;
    let a = g.create_block(p.id, None, "before")?;
    let b = g.create_block(p.id, None, "[[Target]]")?;
    let child = g.create_block(p.id, Some(b.id), "child")?;
    let grand = g.create_block(p.id, Some(child.id), "grandchild")?;
    g.create_block(p.id, None, "after")?;
    g.set_block_property(child.id, "data", &serde_json::json!({"x": [1, "سلام"]}))?;
    g.set_task_status(grand.id, Some(crate::TaskStatus::Doing))?;
    g.add_reference(grand.id, target.id)?;
    let mut e = Editor::new(g);
    let before = snapshot(&e.graph.conn, p.id)?;
    let preview = e.prepare_delete(p.id, b.id)?;
    assert_eq!(preview.count, 3); assert!(preview.requires_confirmation);
    let result = e.apply(p.id, EditAction::Delete { id: b.id, token: preview.token }, None)?;
    assert_eq!(result.removed.len(), 3); assert_eq!(result.focus_id, Some(a.id));
    let after = snapshot(&e.graph.conn, p.id)?;
    assert!(e.graph.incoming_links(target.id)?.is_empty());
    e.history(p.id, false)?;
    assert_eq!(snapshot(&e.graph.conn, p.id)?, before);
    assert_eq!(e.graph.incoming_links(target.id)?.len(), 2);
    e.history(p.id, true)?; assert_eq!(snapshot(&e.graph.conn, p.id)?, after);
    drop(e);
    let g = Graph::open(&path)?;
    assert_eq!(snapshot(&g.conn, p.id)?, after);
    assert!(!g.conn.prepare("PRAGMA foreign_key_check")?.exists([])?);
    assert_eq!(g.conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))?, "ok");
    Ok(())
}

#[test]
fn deletion_previews_reject_stale_content_topology_metadata_and_tokens() -> Result<()> {
    for change in ["content", "child", "property", "token", "page"] {
        let (mut e, page, id) = setup("old")?;
        let preview = e.prepare_delete(page, id)?;
        match change {
            "content" => { save(&mut e, page, id, "old", "new", None)?; }
            "child" => { e.graph.create_block(page, Some(id), "new")?; }
            "property" => e.graph.set_block_property(id, "x", &serde_json::json!(1))?,
            _ => {}
        }
        let before = snapshot(&e.graph.conn, page)?;
        let token = if change == "token" { Uuid::new_v4() } else { preview.token };
        let request_page = if change == "page" { e.graph.create_page("Other")?.id } else { page };
        assert!(e.apply(request_page, EditAction::Delete { id, token }, None).is_err());
        assert_eq!(snapshot(&e.graph.conn, page)?, before);
    }
    Ok(())
}

#[test]
fn deletion_empty_last_nested_and_rollback() -> Result<()> {
    let (mut e, page, root) = setup("")?;
    assert!(!e.prepare_delete(page, root)?.requires_confirmation);
    delete(&mut e, page, root)?; assert!(e.graph.page_blocks(page)?.is_empty());
    e.history(page, false)?;
    let child = e.graph.create_block(page, Some(root), "")?.id;
    assert!(!e.prepare_delete(page, child)?.requires_confirmation);
    delete(&mut e, page, child)?; e.history(page, false)?;
    let before = snapshot(&e.graph.conn, page)?;
    let state = e.history_state(page);
    e.graph.conn.execute_batch("CREATE TRIGGER fail_delete BEFORE DELETE ON blocks WHEN OLD.parent_id IS NULL BEGIN SELECT RAISE(ABORT,'injected'); END;")?;
    assert!(delete(&mut e, page, root).is_err());
    assert_eq!(snapshot(&e.graph.conn, page)?, before); assert_eq!(e.history_state(page), state);
    e.graph.conn.execute_batch("DROP TRIGGER fail_delete;")?;
    delete(&mut e, page, root)?;
    e.graph.conn.execute_batch("CREATE TRIGGER fail_restore BEFORE INSERT ON blocks WHEN NEW.parent_id IS NOT NULL BEGIN SELECT RAISE(ABORT,'injected'); END;")?;
    assert!(e.history(page, false).is_err()); assert!(e.graph.page_blocks(page)?.is_empty());
    assert!(e.history_state(page).0);
    e.graph.conn.execute_batch("DROP TRIGGER fail_restore;")?;
    e.history(page, false)?; assert_eq!(snapshot(&e.graph.conn, page)?, before);
    e.graph.conn.execute_batch("CREATE TRIGGER fail_redo BEFORE DELETE ON blocks WHEN OLD.parent_id IS NULL BEGIN SELECT RAISE(ABORT,'injected'); END;")?;
    assert!(e.history(page, true).is_err()); assert_eq!(snapshot(&e.graph.conn, page)?, before);
    assert!(e.history_state(page).1);
    Ok(())
}

#[test]
fn nested_deletion_keeps_siblings_and_rejects_unexpected_redo_descendants() -> Result<()> {
    let (mut e, page, root) = setup("root")?;
    let first = e.graph.create_block(page, Some(root), "first")?.id;
    let middle = e.graph.create_block(page, Some(root), "middle")?.id;
    let last = e.graph.create_block(page, Some(root), "last")?.id;
    e.graph.create_block(page, Some(middle), "child")?;
    let before = snapshot(&e.graph.conn, page)?;
    let result = delete(&mut e, page, middle)?;
    assert_eq!(result.focus_id, Some(first));
    assert_eq!(ids(&e, page, Some(root))?, vec![first, last]);
    e.history(page, false)?; assert_eq!(snapshot(&e.graph.conn, page)?, before);
    e.graph.create_block(page, Some(middle), "unexpected")?;
    let changed = snapshot(&e.graph.conn, page)?;
    assert!(matches!(e.history(page, true), Err(Error::Conflict(_))));
    assert_eq!(snapshot(&e.graph.conn, page)?, changed);
    assert!(e.history_state(page).1);
    Ok(())
}

#[test]
fn deep_subtree_delete_undo_redo_and_oversized_deletion_fail_safely() -> Result<()> {
    let (mut e, page, root) = setup("root")?;
    let mut parent = root;
    for _ in 0..1100 { parent = e.graph.create_block(page, Some(parent), "deep")?.id; }
    let before = snapshot(&e.graph.conn, page)?;
    assert_eq!(e.prepare_delete(page, root)?.count, 1101);
    delete(&mut e, page, root)?;
    e.history(page, false)?; assert_eq!(snapshot(&e.graph.conn, page)?, before);
    e.history(page, true)?; assert!(e.graph.page_blocks(page)?.is_empty());
    let id = e.graph.create_block(page, None, &"x".repeat(MAX_BYTES))?.id;
    assert!(delete(&mut e, page, id).is_err());
    assert_eq!(e.graph.page_blocks(page)?.len(), 1);
    assert!(!e.graph.conn.prepare("PRAGMA foreign_key_check")?.exists([])?);
    Ok(())
}

#[test]
fn task_history_conflicts_rollback_split_and_reopen() -> Result<()> {
    use crate::TaskStatus;
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("tasks.sqlite3");
    let mut graph = Graph::create(&path)?;
    let page = graph.create_page("Tasks")?.id;
    let id = graph.create_block(page, None, "[[Tasks]]")?.id;
    let mut editor = Editor::new(graph);
    let task = |expected, status| EditAction::Task { id, expected, status };
    editor.apply(page, task(None, Some(TaskStatus::Todo)), None)?;
    assert!(editor.apply(page, task(None, Some(TaskStatus::Done)), None).is_err());
    editor.apply(page, task(Some(TaskStatus::Todo), Some(TaskStatus::Doing)), None)?;
    editor.apply(page, task(Some(TaskStatus::Doing), Some(TaskStatus::Done)), None)?;
    editor.history(page, false)?;
    assert_eq!(editor.graph().page_blocks(page)?[0].task_status.as_deref(), Some("doing"));
    editor.history(page, true)?;
    editor.graph.conn.execute_batch("CREATE TRIGGER fail_task BEFORE UPDATE OF task_status ON blocks BEGIN SELECT RAISE(ABORT,'injected'); END;")?;
    assert!(editor.apply(page, task(Some(TaskStatus::Done), None), None).is_err());
    assert!(editor.history(page, false).is_err());
    editor.graph.conn.execute_batch("DROP TRIGGER fail_task;")?;
    assert_eq!(editor.history_state(page), (true, false));
    let new = editor.apply(page, EditAction::Split { id, start:0, end:0 }, None)?.focus_id.unwrap();
    let blocks = editor.graph().page_blocks(page)?;
    assert_eq!(blocks.iter().find(|b| b.id == id).unwrap().task_status.as_deref(), Some("done"));
    assert_eq!(blocks.iter().find(|b| b.id == new).unwrap().task_status, None);
    editor.history(page, false)?;
    drop(editor);
    let graph = Graph::open(&path)?;
    assert_eq!(graph.page_blocks(page)?[0].task_status.as_deref(), Some("done"));
    assert_eq!(graph.incoming_links(page)?[0].id, id);
    assert!(serde_json::from_str::<EditAction>(&format!(r#"{{"kind":"task","id":"{id}","expected":null,"status":"bogus"}}"#)).is_err());
    Ok(())
}

#[test]
fn inline_reference_resolution_rename_split_and_history() -> Result<()> {
    let (mut editor, page, id) = setup("")?;
    let text = "[[سلام]] [[ سلام ]] [[Future]] [[ ]] [[bad\nlink]]";
    save(&mut editor, page, id, "", text, None)?;
    assert_eq!(editor.graph().inline_references(page)?.len(), 2);
    let target = editor.create_page("سلام")?;
    assert_eq!(editor.graph().incoming_links(target.id)?[0].id, id);
    editor.rename_page(target.id, "Renamed")?;
    let reused = editor.create_page("سلام")?;
    assert_ne!(reused.id, target.id);
    editor.history(page, false)?;
    assert!(editor.graph().incoming_links(target.id)?.is_empty());
    editor.history(page, true)?;
    assert_eq!(editor.graph().incoming_links(target.id)?[0].id, id);
    assert!(editor.graph().incoming_links(reused.id)?.is_empty());
    let future = editor.create_page("Future")?;
    let split = editor.apply(page, EditAction::Split { id, start:0, end:0 }, None)?.focus_id.unwrap();
    assert_eq!(editor.graph().incoming_links(target.id)?[0].id, split);
    assert_eq!(editor.graph().incoming_links(future.id)?[0].id, split);
    editor.history(page, false)?;
    assert_eq!(editor.graph().incoming_links(target.id)?[0].id, id);
    editor.history(page, true)?;
    assert_eq!(editor.graph().incoming_links(target.id)?[0].id, split);
    Ok(())
}

#[test]
fn inline_reference_failure_rolls_back_text_edges_and_history() -> Result<()> {
    let (mut editor, page, id) = setup("before")?;
    let target = editor.create_page("Target")?;
    editor.graph.conn.execute_batch("CREATE TRIGGER fail_links BEFORE INSERT ON inline_references BEGIN SELECT RAISE(ABORT,'injected'); END;")?;
    assert!(save(&mut editor, page, id, "before", "[[Target]]", None).is_err());
    assert_eq!(editor.graph().page_blocks(page)?[0].content, "before");
    assert!(editor.graph().incoming_links(target.id)?.is_empty());
    assert_eq!(editor.history_state(page), (false, false));
    editor.graph.conn.execute_batch("DROP TRIGGER fail_links;")?;
    save(&mut editor, page, id, "before", "[[Target]]", None)?;
    editor.graph.conn.execute_batch("CREATE TRIGGER fail_links BEFORE INSERT ON inline_references BEGIN SELECT RAISE(ABORT,'injected'); END;")?;
    // Redo will reinsert the link; failed replay leaves the undo/redo stack intact.
    editor.history(page, false)?;
    assert!(editor.history(page, true).is_err());
    assert_eq!(editor.graph().page_blocks(page)?[0].content, "before");
    assert_eq!(editor.history_state(page), (false, true));
    Ok(())
}

#[test]
fn page_workflows_preserve_identity_and_block_history() -> Result<()> {
    let (mut editor, page, id) = setup("before")?;
    save(&mut editor, page, id, "before", "after", None)?;
    assert_eq!(editor.rename_page(page, "  سلام  ")?.id, page);
    assert!(editor.rename_page(page, " ").is_err());
    let journal = editor.journal("2024-02-29")?;
    let renamed = editor.rename_page(journal.id, "Leap day")?;
    assert_eq!(renamed.journal_date.as_deref(), Some("2024-02-29"));
    assert_eq!(editor.journal("2024-02-29")?, renamed);
    let changes = editor.graph.conn.total_changes();
    assert_eq!(editor.find_journal("2024-02-29")?, Some(renamed));
    assert_eq!(editor.find_journal("2024-03-01")?, None);
    assert!(matches!(editor.find_journal("2023-02-29"), Err(Error::Invalid(_))));
    assert!(editor.graph().tasks(crate::TaskFilter::Active, None, 100, 0)?.is_empty());
    assert_eq!(editor.graph.conn.total_changes(), changes);
    assert_eq!(editor.history_state(page), (true, false));
    assert!(editor.journal("2023-02-29").is_err());
    editor.history(page, false)?;
    assert_eq!(editor.graph().page_blocks(page)?[0].content, "before");
    assert_eq!(editor.find_journal("2024-03-01")?, None);
    assert_eq!(editor.history_state(page), (false, true));
    editor.history(page, true)?;
    assert_eq!(editor.graph().page_blocks(page)?[0].content, "after");
    Ok(())
}
use crate::TaskStatus;

fn setup(content: &str) -> Result<(Editor, Uuid, Uuid)> {
    let mut graph = Graph::in_memory()?;
    let page = graph.create_page("Editor")?.id;
    let id = graph.create_block(page, None, content)?.id;
    Ok((Editor::new(graph), page, id))
}

fn save(editor: &mut Editor, page: Uuid, id: Uuid, expected: &str, content: &str, group: Option<Uuid>) -> Result<EditResult> {
    editor.apply(page, EditAction::Save { id, expected: expected.into(), content: content.into() }, group)
}

fn ids(editor: &Editor, page: Uuid, parent: Option<Uuid>) -> Result<Vec<Uuid>> {
    Ok(editor.graph().children(page, parent)?.iter().map(|b| b.id).collect())
}

#[test]
fn serde_contract_and_stable_page_read() -> Result<()> {
    let (mut e, page, id) = setup("سلام")?;
    let action: EditAction = serde_json::from_value(serde_json::json!({"kind":"save", "id": id, "expected":"سلام", "content":"دنیا"})).unwrap();
    let result = e.apply(page, action, None)?;
    let encoded = serde_json::to_value(result).unwrap();
    assert_eq!(encoded["changed"][0]["id"], id.to_string());
    assert_eq!(encoded["removed"], serde_json::json!([]));
    assert_eq!(encoded["can_undo"], true);
    assert_eq!(encoded["can_redo"], false);
    assert_eq!(encoded["focus_id"], id.to_string());
    assert!(matches!(e.graph().page_blocks(Uuid::new_v4()), Err(Error::NotFound)));
    let child = e.graph_mut().create_block(page, Some(id), "nested")?.id;
    let a = e.graph().page_blocks(page)?;
    assert_eq!(a, e.graph().page_blocks(page)?);
    assert_eq!(a.iter().map(|b| b.id).collect::<Vec<_>>(), vec![id, child]);
    Ok(())
}

#[test]
fn text_delta_conflict_page_scope_and_noops() -> Result<()> {
    let (mut e, page, id) = setup("before")?;
    let other = e.graph_mut().create_page("Other")?.id;
    e.graph_mut().create_block(page, None, "untouched")?;
    let before = snapshot(&e.graph.conn, page)?;
    assert!(matches!(save(&mut e, page, id, "stale", "new", None), Err(Error::Conflict(_))));
    assert!(matches!(save(&mut e, other, id, "before", "new", None), Err(Error::NotFound)));
    assert_eq!(e.history_state(page), (false, false));
    assert_eq!(snapshot(&e.graph.conn, page)?, before);
    let result = save(&mut e, page, id, "before", "after", None)?;
    assert_eq!(result.changed.len(), 1);
    assert_eq!(result.changed[0].id, id);
    assert!(result.removed.is_empty());
    let after = snapshot(&e.graph.conn, page)?;
    assert!(save(&mut e, page, id, "after", "after", None)?.changed.is_empty());
    assert_eq!(e.history.pages[&page].undo.len(), 1);
    for action in [EditAction::Indent { id }, EditAction::Outdent { id }, EditAction::Reorder { id, direction: -1 }] {
        assert!(e.apply(page, action, None)?.changed.is_empty());
    }
    assert!(matches!(e.apply(page, EditAction::Reorder { id, direction: 0 }, None), Err(Error::Invalid(_))));
    assert_eq!(snapshot(&e.graph.conn, page)?, after);
    e.history(page, false)?;
    assert_eq!(snapshot(&e.graph.conn, page)?, before);
    e.history(page, true)?;
    assert_eq!(snapshot(&e.graph.conn, page)?, after);
    assert_eq!(e.history_state(other), (false, false));
    Ok(())
}

#[test]
fn split_unicode_selection_metadata_children_and_identity() -> Result<()> {
    let (mut e, page, id) = setup("سلام😀دنیا")?;
    let child = e.graph_mut().create_block(page, Some(id), "child")?.id;
    let next = e.graph_mut().create_block(page, None, "next")?.id;
    e.graph_mut().set_task_status(id, Some(TaskStatus::Doing))?;
    e.graph_mut().set_block_property(id, "nested", &serde_json::json!({"tags":["فارسی"]}))?;
    e.graph_mut().add_reference(id, page)?;
    let before = snapshot(&e.graph.conn, page)?;
    for (start, end) in [(5, 5), (4, 5), (5, 6), (11, 11), (6, 4), (usize::MAX, usize::MAX)] {
        assert!(matches!(e.apply(page, EditAction::Split { id, start, end }, None), Err(Error::Invalid(_))));
        assert_eq!(snapshot(&e.graph.conn, page)?, before);
        assert_eq!(e.history_state(page), (false, false));
    }
    let result = e.apply(page, EditAction::Split { id, start: 4, end: 6 }, None)?;
    let new = result.focus_id.unwrap();
    assert_ne!(new, id);
    assert_eq!(ids(&e, page, None)?, vec![id, new, next]);
    let after = snapshot(&e.graph.conn, page)?;
    assert_eq!(after[&id].block.content, "سلام");
    assert_eq!(after[&new].block.content, "دنیا");
    assert_eq!(after[&id].block.task_status.as_deref(), Some("doing"));
    assert_eq!(after[&child], before[&child]);
    assert_eq!(after[&id].properties, before[&id].properties);
    assert_eq!(after[&id].references, before[&id].references);
    assert!(after[&new].block.task_status.is_none());
    assert!(after[&new].properties.is_empty());
    assert!(after[&new].references.is_empty());
    let undone = e.history(page, false)?;
    assert_eq!(undone.removed, vec![new]);
    assert_eq!(snapshot(&e.graph.conn, page)?, before);
    let redone = e.history(page, true)?;
    assert_eq!(redone.focus_id, Some(new));
    assert_eq!(snapshot(&e.graph.conn, page)?, after);
    assert_eq!(e.graph().backlinks(page)?, vec![id]);
    Ok(())
}

#[test]
fn split_endpoints_empty_and_full_selection() -> Result<()> {
    for (text, start, end, prefix, suffix) in [
        ("😀سلام", 0, 0, "", "😀سلام"),
        ("سلام😀", 6, 6, "سلام😀", ""),
        ("😀سلام", 0, 6, "", ""),
        ("", 0, 0, "", ""),
    ] {
        let (mut e, page, id) = setup(text)?;
        let result = e.apply(page, EditAction::Split { id, start, end }, None)?;
        let blocks = e.graph().children(page, None)?;
        assert_eq!(blocks[0].content, prefix);
        assert_eq!(blocks[1].content, suffix);
        assert_eq!(blocks[1].id, result.focus_id.unwrap());
        e.history(page, false)?;
        assert_eq!(e.graph().children(page, None)?.len(), 1);
        assert_eq!(e.graph().children(page, None)?[0].content, text);
    }
    Ok(())
}

#[test]
fn nested_split_keeps_existing_child_groups_and_history_identity() -> Result<()> {
    let (mut e, page, parent) = setup("parent")?;
    let id = e.graph_mut().create_block(page, Some(parent), "a😀b")?.id;
    let next = e.graph_mut().create_block(page, Some(parent), "next")?.id;
    let child = e.graph_mut().create_block(page, Some(id), "child")?.id;
    let before = snapshot(&e.graph.conn, page)?;
    let new = e.apply(page, EditAction::Split { id, start: 1, end: 1 }, None)?.focus_id.unwrap();
    assert_eq!(ids(&e, page, Some(parent))?, vec![id, new, next]);
    assert_eq!(ids(&e, page, Some(id))?, vec![child]);
    assert!(ids(&e, page, Some(new))?.is_empty());
    let after = snapshot(&e.graph.conn, page)?;
    e.history(page, false)?;
    assert_eq!(snapshot(&e.graph.conn, page)?, before);
    e.history(page, true)?;
    assert_eq!(snapshot(&e.graph.conn, page)?, after);
    Ok(())
}

#[test]
fn append_to_empty_page_undo_redo_preserves_uuid_and_timestamps() -> Result<()> {
    let mut graph = Graph::in_memory()?;
    let page = graph.create_page("Empty")?.id;
    let mut e = Editor::new(graph);
    let result = e.apply(page, EditAction::Append, None)?;
    let id = result.focus_id.unwrap();
    assert_eq!(result.changed.len(), 1);
    assert_eq!(result.changed[0].id, id);
    assert_eq!(result.changed[0].content, "");
    let after = snapshot(&e.graph.conn, page)?;
    assert_eq!(after[&id].block.position, 0);
    assert!(after[&id].block.parent_id.is_none());
    assert!(after[&id].block.task_status.is_none());
    let result = e.history(page, false)?;
    assert_eq!(result.removed, vec![id]);
    assert_eq!(result.focus_id, None);
    assert!(e.graph().page_blocks(page)?.is_empty());
    assert_eq!(e.history(page, true)?.focus_id, Some(id));
    assert_eq!(snapshot(&e.graph.conn, page)?, after);
    Ok(())
}

#[test]
fn indent_outdent_reorder_subtree_and_history() -> Result<()> {
    let (mut e, page, a) = setup("a")?;
    let existing = e.graph_mut().create_block(page, Some(a), "existing")?.id;
    let b = e.graph_mut().create_block(page, None, "b")?.id;
    let c = e.graph_mut().create_block(page, None, "c")?.id;
    let child = e.graph_mut().create_block(page, Some(b), "child")?.id;
    let grandchild = e.graph_mut().create_block(page, Some(child), "grandchild")?.id;
    e.graph_mut().set_task_status(b, Some(TaskStatus::Done))?;
    e.graph_mut().set_block_property(child, "p", &serde_json::json!(42))?;
    e.graph_mut().add_reference(grandchild, page)?;
    let before = snapshot(&e.graph.conn, page)?;
    let mut states = vec![before.clone()];
    e.apply(page, EditAction::Indent { id: b }, None)?;
    assert_eq!(ids(&e, page, Some(a))?, vec![existing, b]);
    assert_eq!(ids(&e, page, Some(b))?, vec![child]);
    states.push(snapshot(&e.graph.conn, page)?);
    e.apply(page, EditAction::Outdent { id: child }, None)?;
    assert_eq!(ids(&e, page, Some(a))?, vec![existing, b, child]);
    assert_eq!(ids(&e, page, Some(child))?, vec![grandchild]);
    states.push(snapshot(&e.graph.conn, page)?);
    e.apply(page, EditAction::Reorder { id: b, direction: -1 }, None)?;
    assert_eq!(ids(&e, page, Some(a))?, vec![b, existing, child]);
    states.push(snapshot(&e.graph.conn, page)?);
    e.apply(page, EditAction::Outdent { id: b }, None)?;
    assert_eq!(ids(&e, page, None)?, vec![a, b, c]);
    states.push(snapshot(&e.graph.conn, page)?);
    for state in states.iter().rev().skip(1) {
        e.history(page, false)?;
        assert_eq!(&snapshot(&e.graph.conn, page)?, state);
    }
    for state in states.iter().skip(1) {
        e.history(page, true)?;
        assert_eq!(&snapshot(&e.graph.conn, page)?, state);
    }
    assert_eq!(e.graph().backlinks(page)?, vec![grandchild]);
    Ok(())
}

#[test]
fn remove_empty_is_safe_root_leaf_only_and_reversible() -> Result<()> {
    for guard in ["last", "content", "whitespace", "child", "nested", "task", "property", "reference"] {
        let (mut e, page, id) = setup("")?;
        if guard != "last" { e.graph_mut().create_block(page, None, "other")?; }
        match guard {
            "content" => e.graph_mut().update_block(id, "text")?,
            "whitespace" => e.graph_mut().update_block(id, " ")?,
            "child" => { e.graph_mut().create_block(page, Some(id), "child")?; }
            "nested" => { let parent = e.graph_mut().create_block(page, None, "parent")?.id; e.graph_mut().move_block(id, Some(parent), 0)?; }
            "task" => e.graph_mut().set_task_status(id, Some(TaskStatus::Todo))?,
            "property" => e.graph_mut().set_block_property(id, "p", &serde_json::json!(null))?,
            "reference" => e.graph_mut().add_reference(id, page)?,
            _ => {}
        }
        let before = snapshot(&e.graph.conn, page)?;
        assert!(matches!(e.apply(page, EditAction::RemoveEmpty { id }, None), Err(Error::Invalid(_))), "{guard}");
        assert_eq!(snapshot(&e.graph.conn, page)?, before);
        assert_eq!(e.history_state(page), (false, false));
    }
    let (mut e, page, first) = setup("")?;
    let second = e.graph_mut().create_block(page, None, "")?.id;
    let third = e.graph_mut().create_block(page, None, "third")?.id;
    let before = snapshot(&e.graph.conn, page)?;
    let result = e.apply(page, EditAction::RemoveEmpty { id: second }, None)?;
    assert_eq!(result.removed, vec![second]);
    assert_eq!(result.focus_id, Some(first));
    assert!(result.changed.is_empty());
    assert_eq!(ids(&e, page, None)?, vec![first, third]);
    let after = snapshot(&e.graph.conn, page)?;
    e.history(page, false)?;
    assert_eq!(snapshot(&e.graph.conn, page)?, before);
    e.history(page, true)?;
    assert_eq!(snapshot(&e.graph.conn, page)?, after);
    let result = e.apply(page, EditAction::RemoveEmpty { id: first }, None)?;
    assert_eq!(result.focus_id, Some(third));
    Ok(())
}

#[test]
fn saves_group_only_consecutively_persist_and_invalidate_redo() -> Result<()> {
    let (mut e, page, id) = setup("a")?;
    let second = e.graph_mut().create_block(page, None, "x")?.id;
    let group = Some(Uuid::new_v4());
    save(&mut e, page, id, "a", "ab", group)?;
    assert_eq!(e.graph().children(page, None)?[0].content, "ab");
    save(&mut e, page, id, "ab", "abc", group)?;
    assert_eq!(e.history.pages[&page].undo.len(), 1);
    assert_eq!(e.graph().children(page, None)?[0].content, "abc");
    let before_failure = snapshot(&e.graph.conn, page)?;
    assert!(save(&mut e, page, id, "bad", "oops", group).is_err());
    assert_eq!(snapshot(&e.graph.conn, page)?, before_failure);
    assert_eq!(e.history.pages[&page].undo.len(), 1);
    e.history(page, false)?;
    assert_eq!(e.graph().children(page, None)?[0].content, "a");
    assert_eq!(e.history_state(page), (false, true));
    assert!(save(&mut e, page, id, "bad", "oops", None).is_err());
    assert_eq!(e.history_state(page), (false, true));
    e.history(page, true)?;
    assert_eq!(e.graph().children(page, None)?[0].content, "abc");
    save(&mut e, page, second, "x", "y", group)?;
    save(&mut e, page, id, "abc", "abcd", group)?;
    assert_eq!(e.history.pages[&page].undo.len(), 3);
    e.history(page, false)?;
    save(&mut e, page, id, "abc", "branch", group)?;
    assert_eq!(e.history_state(page), (true, false));
    e.history(page, false)?;
    assert_eq!(e.graph().children(page, None)?[0].content, "abc");
    Ok(())
}

#[test]
fn history_is_page_scoped_but_mutable_graph_access_clears_every_page() -> Result<()> {
    let (mut e, page, id) = setup("a")?;
    let other = e.graph_mut().create_page("Other")?.id;
    let other_id = e.graph_mut().create_block(other, None, "b")?.id;
    let group = Some(Uuid::new_v4());
    save(&mut e, page, id, "a", "aa", group)?;
    e.create_page("New empty page preserves history")?;
    assert_eq!(e.history_state(page), (true, false));
    save(&mut e, other, other_id, "b", "bb", group)?;
    save(&mut e, page, id, "aa", "aaa", group)?;
    assert_eq!(e.history.pages[&page].undo.len(), 2);
    e.history(page, false)?;
    assert_eq!(e.graph().children(other, None)?[0].content, "bb");
    e.history(other, false)?;
    assert_eq!(e.graph().children(page, None)?[0].content, "aa");
    assert!(matches!(e.history(Uuid::new_v4(), false), Err(Error::NotFound)));
    assert_eq!(e.history_state(page), (true, true));
    let _ = e.graph_mut();
    assert_eq!(e.history_state(page), (false, false));
    assert_eq!(e.history_state(other), (false, false));
    Ok(())
}

#[test]
fn history_metadata_preconditions_and_cascade_guard_fail_without_changes() -> Result<()> {
    let (mut e, page, id) = setup("a")?;
    e.graph_mut().set_block_property(id, "p", &serde_json::json!(1))?;
    save(&mut e, page, id, "a", "b", None)?;
    let saved = snapshot(&e.graph.conn, page)?;
    for column in ["created_at", "updated_at"] {
        e.graph.conn.execute(&format!("UPDATE blocks SET {column}={column}+1 WHERE id=?1"), [id.to_string()])?;
        let mutated = snapshot(&e.graph.conn, page)?;
        assert!(matches!(e.history(page, false), Err(Error::Conflict(_))));
        assert_eq!(snapshot(&e.graph.conn, page)?, mutated);
        assert_eq!(e.history_state(page), (true, false));
        e.graph.conn.execute(&format!("UPDATE blocks SET {column}={column}-1 WHERE id=?1"), [id.to_string()])?;
    }
    e.graph.set_task_status(id, Some(TaskStatus::Todo))?;
    assert!(matches!(e.history(page, false), Err(Error::Conflict(_))));
    e.graph.conn.execute("UPDATE blocks SET task_status=NULL,updated_at=?2 WHERE id=?1", params![id.to_string(), saved[&id].updated_at])?;
    // Private test-only bypass: simulate a caller missing invalidation on this connection.
    e.graph.set_block_property(id, "p", &serde_json::json!(2))?;
    let changed = snapshot(&e.graph.conn, page)?;
    assert!(matches!(e.history(page, false), Err(Error::Conflict(_))));
    assert_eq!(snapshot(&e.graph.conn, page)?, changed);
    assert_eq!(e.history_state(page), (true, false));
    e.graph.set_block_property(id, "p", &serde_json::json!(1))?;
    e.history(page, false)?;
    e.graph.add_reference(id, page)?;
    assert!(matches!(e.history(page, true), Err(Error::Conflict(_))));
    assert_eq!(e.history_state(page), (false, true));
    e.graph.remove_reference(id, page)?;
    e.history(page, true)?;
    let added = e.apply(page, EditAction::Append, None)?.focus_id.unwrap();
    let child = e.graph.create_block(page, Some(added), "must survive")?.id;
    let before = snapshot(&e.graph.conn, page)?;
    assert!(matches!(e.history(page, false), Err(Error::Conflict(_))));
    assert_eq!(snapshot(&e.graph.conn, page)?, before);
    assert_eq!(ids(&e, page, Some(added))?, vec![child]);
    assert_eq!(e.history.pages[&page].undo.len(), 2);
    Ok(())
}

#[test]
fn apply_and_history_injected_failure_roll_back_data_and_stacks() -> Result<()> {
    let (mut e, page, id) = setup("before")?;
    save(&mut e, page, id, "before", "after", None)?;
    e.graph.conn.execute_batch("CREATE TEMP TRIGGER fail_insert AFTER INSERT ON blocks BEGIN SELECT RAISE(ABORT,'injected'); END;")?;
    let before = snapshot(&e.graph.conn, page)?;
    assert!(e.apply(page, EditAction::Split { id, start: 2, end: 2 }, None).is_err());
    assert_eq!(snapshot(&e.graph.conn, page)?, before);
    assert_eq!(e.history.pages[&page].undo.len(), 1);
    assert_eq!(e.graph().search("after")?[0].id, id);
    e.graph.conn.execute_batch("DROP TRIGGER fail_insert;")?;
    let new = e.apply(page, EditAction::Split { id, start: 2, end: 2 }, None)?.focus_id.unwrap();
    e.history(page, false)?;
    let before = snapshot(&e.graph.conn, page)?;
    e.graph.conn.execute_batch("CREATE TEMP TRIGGER fail_insert AFTER INSERT ON blocks BEGIN SELECT RAISE(ABORT,'injected'); END;")?;
    assert!(e.history(page, true).is_err());
    assert_eq!(snapshot(&e.graph.conn, page)?, before);
    assert_eq!(e.history_state(page), (true, true));
    e.graph.conn.execute_batch("DROP TRIGGER fail_insert;")?;
    e.history(page, true)?;
    assert!(e.graph().page_blocks(page)?.iter().any(|b| b.id == new));
    e.graph.conn.execute_batch("CREATE TEMP TRIGGER fail_order AFTER UPDATE OF position ON blocks WHEN NEW.position>=0 BEGIN SELECT RAISE(ABORT,'order failure'); END;")?;
    let before = snapshot(&e.graph.conn, page)?;
    assert!(e.apply(page, EditAction::Reorder { id, direction: 1 }, None).is_err());
    assert_eq!(snapshot(&e.graph.conn, page)?, before);
    assert_eq!(e.history_state(page), (true, false));
    assert!(e.history(page, false).is_err());
    assert_eq!(snapshot(&e.graph.conn, page)?, before);
    assert_eq!(e.history_state(page), (true, false));
    Ok(())
}

#[test]
fn disk_reopen_and_external_writer_invalidate_without_overwrite() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("editor.sqlite3");
    let mut graph = Graph::create(&path)?;
    let page = graph.create_page("Disk")?.id;
    let id = graph.create_block(page, None, "سلام😀")?.id;
    graph.set_task_status(id, Some(TaskStatus::Doing))?;
    graph.set_block_property(id, "p", &serde_json::json!([1, 2]))?;
    graph.add_reference(id, page)?;
    let mut e = Editor::new(graph);
    let new = e.apply(page, EditAction::Split { id, start: 4, end: 4 }, None)?.focus_id.unwrap();
    let expected = snapshot(&e.graph.conn, page)?;
    drop(e);
    let mut e = Editor::new(Graph::open(&path)?);
    assert_eq!(snapshot(&e.graph.conn, page)?, expected);
    assert_eq!(e.history_state(page), (false, false));
    save(&mut e, page, new, "😀", "saved", None)?;
    let mut external = Graph::open(&path)?;
    external.set_block_property(new, "external", &serde_json::json!(true))?;
    external.update_block(new, "external text")?;
    assert_eq!(e.history_state(page), (false, false));
    let result = e.history(page, false)?;
    assert!(result.changed.is_empty());
    assert_eq!(e.history_state(page), (false, false));
    assert_eq!(snapshot(&e.graph.conn, page)?[&new].block.content, "external text");
    assert!(matches!(save(&mut e, page, new, "saved", "bad", None), Err(Error::Conflict(_))));
    save(&mut e, page, new, "external text", "accepted", None)?;
    e.history(page, false)?;
    assert_eq!(snapshot(&e.graph.conn, page)?[&new].block.content, "external text");
    assert_eq!(snapshot(&e.graph.conn, page)?[&new].properties.len(), 1);
    drop(external);
    drop(e);
    let graph = Graph::open(&path)?;
    let integrity: String = graph.conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    assert_eq!(integrity, "ok");
    assert_eq!(graph.conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get::<_, i64>(0))?, 0);
    Ok(())
}

#[test]
fn history_is_bounded_by_count_and_bytes_and_cannot_cross_oversized_edits() -> Result<()> {
    let (mut e, page, id) = setup("0")?;
    for n in 1..=110 {
        save(&mut e, page, id, &(n - 1).to_string(), &n.to_string(), None)?;
    }
    assert_eq!(e.history.pages[&page].undo.len(), MAX_ENTRIES);
    for _ in 0..MAX_ENTRIES { e.history(page, false)?; }
    assert_eq!(e.graph().children(page, None)?[0].content, "10");
    assert_eq!(e.history_state(page), (false, true));
    save(&mut e, page, id, "10", &"x".repeat(MAX_BYTES), None)?;
    assert_eq!(e.history_state(page), (false, false));
    assert!(e.history.pages.is_empty());
    // Several individually bounded edits must also respect the aggregate budget.
    let medium = "m".repeat(MAX_BYTES / 8);
    save(&mut e, page, id, &"x".repeat(MAX_BYTES), &medium, None)?;
    assert_eq!(e.history_state(page), (false, false));
    let mut content = medium;
    for n in 0..8 {
        let next = format!("{}{}", n, "m".repeat(MAX_BYTES / 8));
        save(&mut e, page, id, &content, &next, None)?;
        content = next;
    }
    let retained: Vec<_> = e.history.pages.values().flat_map(|h| h.undo.iter().chain(h.redo.iter())).collect();
    assert!(retained.iter().map(|entry| entry.bytes()).sum::<usize>() <= MAX_BYTES);
    assert!(retained.len() < 8);
    Ok(())
}

#[test]
fn count_bound_is_global_and_eviction_never_leaves_a_broken_redo_chain() -> Result<()> {
    let (mut e, page, id) = setup("0")?;
    let other = e.graph_mut().create_page("Other")?.id;
    let other_id = e.graph_mut().create_block(other, None, "0")?.id;
    for n in 1..=60 { save(&mut e, page, id, &(n - 1).to_string(), &n.to_string(), None)?; }
    for n in 1..=60 { save(&mut e, other, other_id, &(n - 1).to_string(), &n.to_string(), None)?; }
    assert_eq!(e.history.pages[&page].undo.len(), 40);
    assert_eq!(e.history.pages[&other].undo.len(), 60);
    for _ in 0..40 { e.history(page, false)?; }
    assert_eq!(e.graph().children(page, None)?[0].content, "20");
    assert_eq!(e.history_state(page), (false, true));
    save(&mut e, other, other_id, "60", "61", None)?;
    assert_eq!(e.history_state(page), (false, false));
    assert_eq!(e.history.pages[&other].undo.len(), 61);
    assert!(e.history(page, true)?.changed.is_empty());
    assert_eq!(e.graph().children(page, None)?[0].content, "20");
    Ok(())
}

#[test]
fn deterministic_ordering_sequences_match_tree_model_and_history() -> Result<()> {
    let (mut e, page, first) = setup("0")?;
    let mut model = vec![(first, None)];
    for n in 1..10 { model.push((e.graph_mut().create_block(page, None, &n.to_string())?.id, None)); }
    let mut states = vec![snapshot(&e.graph.conn, page)?];
    let mut seed = 42_u64;
    for step in 0..80 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let index = (seed % model.len() as u64) as usize;
        let (id, parent) = model[index];
        let sibling_indexes: Vec<_> = model.iter().enumerate().filter(|(_, b)| b.1 == parent).map(|(i, _)| i).collect();
        let sibling_index = sibling_indexes.iter().position(|i| *i == index).unwrap();
        let action = match step % 4 {
            0 => {
                if sibling_index > 0 { model[index].1 = Some(model[sibling_indexes[sibling_index - 1]].0); }
                EditAction::Indent { id }
            }
            1 => {
                if let Some(parent) = parent {
                    let parent_index = model.iter().position(|b| b.0 == parent).unwrap();
                    let grandparent = model[parent_index].1;
                    let row = model.remove(index);
                    let parent_index = model.iter().position(|b| b.0 == parent).unwrap();
                    model.insert(parent_index + 1, (row.0, grandparent));
                }
                EditAction::Outdent { id }
            }
            _ => {
                let direction = if step % 4 == 2 { -1 } else { 1 };
                if let Some(other) = sibling_index.checked_add_signed(direction as isize).filter(|i| *i < sibling_indexes.len()) {
                    model.swap(index, sibling_indexes[other]);
                }
                EditAction::Reorder { id, direction }
            }
        };
        e.apply(page, action, None)?;
        let actual = e.graph().page_blocks(page)?;
        for parent in std::iter::once(None).chain(model.iter().map(|b| Some(b.0))) {
            let expected: Vec<_> = model.iter().filter(|b| b.1 == parent).map(|b| b.0).collect();
            assert_eq!(ids(&e, page, parent)?, expected, "step {step}");
        }
        assert_eq!(actual.len(), model.len());
        assert!(actual.iter().all(|b| b.position >= 0));
        let state = snapshot(&e.graph.conn, page)?;
        if states.last() != Some(&state) { states.push(state); }
    }
    for expected in states.iter().rev().skip(1) {
        e.history(page, false)?;
        assert_eq!(&snapshot(&e.graph.conn, page)?, expected);
    }
    for expected in states.iter().skip(1) {
        e.history(page, true)?;
        assert_eq!(&snapshot(&e.graph.conn, page)?, expected);
    }
    Ok(())
}
