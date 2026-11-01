use super::*;

#[test]
fn journal_lookup_is_read_only_and_preserves_renamed_identity() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("journals.sqlite3");
    let mut g = Graph::create(&path)?;
    let ordinary = g.create_page("2026-10-24")?;
    assert_eq!(ordinary.journal_date, None);
    assert_eq!(serde_json::to_value(&ordinary).unwrap()["journal_date"], serde_json::Value::Null);
    assert_eq!(g.lookup_journal("2026-10-24")?, None); // A date title is not journal identity.
    let journal = g.create_journal("2024-02-29")?;
    assert_eq!(journal.journal_date.as_deref(), Some("2024-02-29"));
    g.rename_page(journal.id, "Leap day سلام")?;
    let reused_title = g.create_page("2024-02-29")?;
    assert_eq!(reused_title.journal_date, None);
    let renamed = g.lookup_journal("2024-02-29")?.unwrap();
    assert_eq!(renamed.id, journal.id);
    assert_eq!(renamed.title, "Leap day سلام");
    assert_eq!(renamed.journal_date, journal.journal_date);
    assert_eq!(g.create_journal("2024-02-29")?, renamed);
    drop(g);
    let mut g = Graph::open(&path)?;
    assert!(g.pages()?.contains(&renamed));
    let before = g.pages()?;
    let changes = g.conn.total_changes();
    g.conn.execute_batch("PRAGMA query_only=ON")?;
    assert_eq!(g.lookup_journal("2024-02-29")?, Some(renamed));
    assert_eq!(g.lookup_journal("2026-10-25")?, None);
    assert_eq!(g.lookup_journal("2026-10-24")?, None);
    for date in ["2023-02-29", "1900-02-29", "2026-04-31", "2026-2-09", "2026-02-9",
        "2026-02-30", "2026-13-01", "2026-00-01", "2026-01-00", "2026-10-24T00:00:00Z",
        " 2026-10-24", "2026-10-24 ", "+10000-01-01", ""] {
        assert!(matches!(g.lookup_journal(date), Err(Error::Invalid(_))), "{date}");
        assert!(matches!(g.create_journal(date), Err(Error::Invalid(_))), "{date}");
    }
    assert_eq!(g.lookup_journal("2000-02-29")?, None);
    assert_eq!(g.pages()?, before);
    assert_eq!(g.conn.total_changes(), changes);
    g.conn.execute_batch("PRAGMA query_only=OFF")?;
    g.delete_page(journal.id)?;
    assert_eq!(g.lookup_journal("2024-02-29")?, None);
    // Reused ordinary titles still cannot be silently converted into a journal.
    assert!(g.create_journal("2024-02-29").is_err());
    Ok(())
}

#[test]
fn task_queries_include_journals_nested_blocks_and_only_selected_statuses() -> Result<()> {
    let mut g = Graph::in_memory()?;
    let beta = g.create_page("Beta")?;
    let alpha = g.create_page("alpha")?;
    let journal = g.create_journal("2026-10-24")?;
    let plain = g.create_block(alpha.id, None, "not a task")?;
    let todo = g.create_block(alpha.id, Some(plain.id), "nested سلام😀")?;
    let doing = g.create_block(beta.id, None, "in progress")?;
    let journal_todo = g.create_block(journal.id, None, "journal task")?;
    let done = g.create_block(beta.id, None, "completed")?;
    let cancelled = g.create_block(alpha.id, None, "cancelled")?;
    for (id, status) in [(todo.id, TaskStatus::Todo), (doing.id, TaskStatus::Doing),
        (journal_todo.id, TaskStatus::Todo), (done.id, TaskStatus::Done), (cancelled.id, TaskStatus::Cancelled)] {
        g.set_task_status(id, Some(status))?;
    }
    let expected = vec![
        TaskHit { id: journal_todo.id, page_id: journal.id, page_title: journal.title.clone(), content: "journal task".into(), task_status: "todo".into() },
        TaskHit { id: todo.id, page_id: alpha.id, page_title: alpha.title.clone(), content: "nested سلام😀".into(), task_status: "todo".into() },
        TaskHit { id: doing.id, page_id: beta.id, page_title: beta.title.clone(), content: "in progress".into(), task_status: "doing".into() },
    ];
    let changes = g.conn.total_changes();
    g.conn.execute_batch("PRAGMA query_only=ON")?;
    assert_eq!(g.tasks(TaskFilter::Active, None, 100, 0)?, expected);
    assert_eq!(g.tasks(TaskFilter::Todo, None, 100, 0)?, expected[..2]);
    assert_eq!(g.tasks(TaskFilter::Doing, None, 100, 0)?, expected[2..]);
    let completed = g.tasks(TaskFilter::Done, None, 100, 0)?;
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].id, done.id);
    assert_eq!(completed[0].task_status, "done");
    assert_eq!(g.tasks(TaskFilter::Active, Some(alpha.id), 100, 0)?, expected[1..2]);
    assert_eq!(g.tasks(TaskFilter::Todo, Some(journal.id), 100, 0)?, expected[..1]);
    assert!(g.tasks(TaskFilter::Doing, Some(alpha.id), 100, 0)?.is_empty());
    assert!(g.tasks(TaskFilter::Active, Some(Uuid::new_v4()), 100, 0)?.is_empty());
    assert_eq!(g.conn.total_changes(), changes);
    g.conn.execute_batch("PRAGMA query_only=OFF")?;
    g.rename_page(journal.id, "Zulu")?;
    let renamed = g.tasks(TaskFilter::Active, None, 100, 0)?;
    assert_eq!(renamed.iter().map(|hit| hit.id).collect::<Vec<_>>(), vec![todo.id, doing.id, journal_todo.id]);
    assert_eq!(renamed[2].page_title, "Zulu");
    g.set_task_status(todo.id, None)?;
    g.update_block(doing.id, "current text")?;
    g.delete_page(journal.id)?;
    let remaining = g.tasks(TaskFilter::Active, None, 100, 0)?;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, doing.id);
    assert_eq!(remaining[0].content, "current text");
    Ok(())
}

#[test]
fn task_pagination_is_bounded_stable_and_uses_existing_task_index() -> Result<()> {
    let mut g = Graph::in_memory()?;
    let page = g.create_page("Paging")?;
    let mut expected = Vec::new();
    for n in 0..205 {
        let block = g.create_block(page.id, None, &format!("task {n}"))?;
        g.set_task_status(block.id, Some(TaskStatus::Todo))?;
        expected.push(block.id);
    }
    expected.sort_unstable(); // Within a page, order is block UUID, not outline position.
    let first = g.tasks(TaskFilter::Active, None, usize::MAX, 0)?;
    let second = g.tasks(TaskFilter::Active, None, 100, 100)?;
    let last = g.tasks(TaskFilter::Active, None, 100, 200)?;
    assert_eq!((first.len(), second.len(), last.len()), (100, 100, 5));
    assert_eq!(g.tasks(TaskFilter::Active, None, 100, 0)?, first);
    let actual: Vec<_> = first.iter().chain(&second).chain(&last).map(|hit| hit.id).collect();
    assert_eq!(actual, expected);
    assert_eq!(g.tasks(TaskFilter::Todo, Some(page.id), 7, 99)?.iter().map(|hit| hit.id).collect::<Vec<_>>(), expected[99..106]);
    assert!(g.tasks(TaskFilter::Active, None, 0, 0)?.is_empty());
    assert!(g.tasks(TaskFilter::Active, None, 100, 205)?.is_empty());
    assert!(g.tasks(TaskFilter::Active, None, 100, i64::MAX as usize)?.is_empty());
    if usize::BITS > 63 {
        assert!(matches!(g.tasks(TaskFilter::Active, None, 100, usize::MAX), Err(Error::Invalid(_))));
    }
    let mut stmt = g.conn.prepare(&format!("EXPLAIN QUERY PLAN {TASKS_SQL}"))?;
    let plan = stmt.query_map(params!["todo", "doing", Option::<String>::None, 100, 0], |r| r.get::<_, String>(3))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert!(plan.iter().any(|line| line.contains("blocks_tasks")), "{plan:?}");
    Ok(())
}

#[test]
fn native_paths_with_spaces_and_unicode_preserve_graph_data() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let parent = dir.path().join("Graph notes سلام");
    std::fs::create_dir(&parent)?;
    let path = parent.join("Café notes.sqlite3");
    let mut graph = Graph::create(&path)?;
    let page = graph.create_page("Path test")?;
    let block = graph.create_block(page.id, None, "سلام persisted")?;
    assert!(Graph::create(&path).is_err());
    drop(graph);
    let graph = Graph::open(&path)?;
    assert_eq!(graph.children(page.id, None)?, vec![block]);
    assert_eq!(graph.search("persisted")?.len(), 1);
    assert_eq!(graph.conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))?, "ok");
    assert!(!graph.conn.prepare("PRAGMA foreign_key_check")?.exists([])?);
    Ok(())
}

#[test]
fn closed_graph_backup_and_separate_restore_preserve_text_identity_and_constraints() -> Result<()> {
    use std::{fs::{self, OpenOptions}, io};
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("graph سلام.sqlite3");
    let backup = dir.path().join("backup.sqlite3");
    let restored = dir.path().join("restored.sqlite3");
    let mut graph = Graph::create(&source)?;
    let id = graph.id()?;
    let page = graph.create_page("Notes")?;
    let block = graph.create_block(page.id, None, "سلام ZWNJ\u{200c} 😀")?;
    // A live main-file copy cannot be trusted: committed WAL frames may be absent.
    assert!(source.with_extension("sqlite3-wal").exists());
    drop(graph);

    fn copy_new(from: &Path, to: &Path) -> io::Result<()> {
        let mut input = fs::File::open(from)?;
        let mut output = OpenOptions::new().write(true).create_new(true).open(to)?;
        io::copy(&mut input, &mut output)?;
        output.sync_all()
    }
    copy_new(&source, &backup)?; // Only after all graph connections exit.
    assert!(copy_new(&source, &backup).is_err()); // No silent overwrite.
    let mut reopened = Graph::open(&source)?;
    reopened.create_block(page.id, None, "later edit")?;
    drop(reopened);
    copy_new(&backup, &restored)?;
    let restored_graph = Graph::open(&restored)?;
    assert_eq!(restored_graph.id()?, id);
    assert_eq!(restored_graph.children(page.id, None)?, vec![block]);
    assert_eq!(restored_graph.conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))?, "ok");
    assert!(!restored_graph.conn.prepare("PRAGMA foreign_key_check")?.exists([])?);
    Ok(())
}

#[test]
fn inline_bindings_reopen_and_legacy_mutations_are_transactional() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("links.sqlite3");
    let mut g = Graph::create(&path)?;
    let source = g.create_page("Source")?;
    let target = g.create_page("Target")?;
    let b = g.create_block(source.id, None, "[[Target]] [[Missing]]")?;
    g.add_reference(b.id, target.id)?;
    g.rename_page(target.id, "New title")?;
    drop(g);
    let mut g = Graph::open(&path)?;
    assert_eq!(g.incoming_links(target.id)?.len(), 1);
    assert_eq!(g.inline_references(source.id)?.iter().find(|r| r.label == "Target").unwrap().target_page_id, Some(target.id));
    g.conn.execute_batch("CREATE TRIGGER fail_resolve BEFORE UPDATE ON inline_references BEGIN SELECT RAISE(ABORT,'injected'); END;")?;
    assert!(g.create_page("Missing").is_err());
    assert!(!g.pages()?.iter().any(|p| p.title == "Missing"));
    g.conn.execute_batch("DROP TRIGGER fail_resolve;")?;
    g.update_block(b.id, "plain")?;
    assert!(g.inline_references(source.id)?.is_empty());
    assert_eq!(g.incoming_links(target.id)?.len(), 1); // explicit edge is independent
    Ok(())
}

#[test]
fn graph_roundtrip_and_independence() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("graph.sqlite3");
    assert!(Graph::open(&path).is_err());
    let mut g = Graph::create(&path)?;
    assert!(Graph::create(&path).is_err());
    let p = g.create_page(" Notes ")?;
    let b = g.create_block(p.id, None, "persisted")?;
    g.set_page_property(p.id, "color", &serde_json::json!("blue"))?;
    drop(g);
    let reopened = Graph::open(&path)?;
    assert_eq!(reopened.pages()?, vec![p.clone()]);
    assert_eq!(reopened.children(p.id, None)?, vec![b]);
    assert_eq!(reopened.search("persisted")?.len(), 1);
    let other = Graph::create(dir.path().join("other.sqlite3"))?;
    assert!(other.pages()?.is_empty());
    Ok(())
}

#[test]
fn nesting_reordering_and_subtree_deletion() -> Result<()> {
    let mut g = Graph::in_memory()?;
    let p = g.create_page("Outline")?;
    let a = g.create_block(p.id, None, "a")?;
    let b = g.create_block(p.id, None, "b")?;
    let c = g.create_block(p.id, None, "c")?;
    let child = g.create_block(p.id, Some(a.id), "child")?;
    let grandchild = g.create_block(p.id, Some(child.id), "grandchild")?;
    g.move_block(c.id, None, 0)?;
    assert_eq!(g.children(p.id, None)?.iter().map(|b| b.id).collect::<Vec<_>>(), vec![c.id, a.id, b.id]);
    g.move_block(child.id, Some(b.id), 0)?;
    assert!(g.children(p.id, Some(a.id))?.is_empty());
    assert_eq!(g.children(p.id, Some(child.id))?[0].id, grandchild.id);
    g.move_block(c.id, Some(b.id), 0)?;
    assert_eq!(g.children(p.id, Some(b.id))?.iter().map(|b| b.id).collect::<Vec<_>>(), vec![c.id, child.id]);
    g.update_block(child.id, "updated")?;
    assert_eq!(g.search("updated")?[0].id, child.id);
    g.set_block_property(child.id, "priority", &serde_json::json!(1))?;
    g.add_reference(grandchild.id, p.id)?;
    g.delete_block(b.id)?;
    assert!(g.search("updated")?.is_empty());
    assert!(g.backlinks(p.id)?.is_empty());
    assert_eq!(g.children(p.id, None)?.len(), 1);
    Ok(())
}

#[test]
fn invalid_moves_are_atomic() -> Result<()> {
    let mut g = Graph::in_memory()?;
    let p = g.create_page("One")?;
    let q = g.create_page("Two")?;
    let a = g.create_block(p.id, None, "a")?;
    let b = g.create_block(p.id, Some(a.id), "b")?;
    let foreign = g.create_block(q.id, None, "foreign")?;
    let before = g.children(p.id, None)?;
    assert!(g.move_block(a.id, Some(b.id), 0).is_err());
    assert!(g.move_block(a.id, Some(a.id), 0).is_err());
    assert!(g.move_block(a.id, Some(foreign.id), 0).is_err());
    assert!(g.move_block(a.id, None, 99).is_err());
    assert!(g.move_block(a.id, Some(Uuid::new_v4()), 0).is_err());
    assert_eq!(g.children(p.id, None)?, before);
    assert_eq!(g.children(p.id, Some(a.id))?[0].id, b.id);
    assert!(g.create_block(p.id, Some(foreign.id), "bad").is_err());
    assert!(g.create_block(Uuid::new_v4(), None, "bad").is_err());
    Ok(())
}

#[test]
fn renaming_references_journals_properties_and_tasks() -> Result<()> {
    let mut g = Graph::in_memory()?;
    let p = g.create_page("Target")?;
    let q = g.create_page("Source")?;
    let b = g.create_block(q.id, None, "سلام دنیا")?;
    g.add_reference(b.id, p.id)?;
    g.add_reference(b.id, p.id)?;
    g.rename_page(p.id, "Renamed")?;
    assert_eq!(g.backlinks(p.id)?, vec![b.id]);
    assert!(g.search("Target")?.is_empty());
    assert_eq!(g.search("Renamed")?[0].id, p.id);
    assert_eq!(g.search("سلام")?[0].id, b.id);
    assert!(g.create_page(" renamed ").is_err());
    assert!(g.create_page("   ").is_err());
    let accent = g.create_page("Café")?;
    assert!(g.create_page("Cafe\u{301}").is_err());
    assert!(g.rename_page(accent.id, "Source").is_err());
    g.set_task_status(b.id, Some(TaskStatus::Doing))?;
    assert_eq!(g.children(q.id, None)?[0].task_status.as_deref(), Some("doing"));
    g.set_task_status(b.id, None)?;
    g.set_page_property(p.id, "tags", &serde_json::json!(["a", "b"]))?;
    g.set_page_property(p.id, "tags", &serde_json::json!(["new"]))?;
    let stored: String = g.conn.query_row("SELECT value_json FROM page_properties WHERE page_id=?1", [p.id.to_string()], |r| r.get(0))?;
    assert_eq!(stored, "[\"new\"]");
    assert!(g.set_block_property(Uuid::new_v4(), "x", &serde_json::json!(1)).is_err());
    let journal = g.create_journal("2024-02-29")?;
    assert_eq!(journal, g.create_journal("2024-02-29")?);
    assert!(g.create_journal("2023-02-29").is_err());
    assert!(g.create_journal("2024-2-9").is_err());
    g.create_page("2025-01-01")?;
    assert!(g.create_journal("2025-01-01").is_err());
    g.remove_reference(b.id, p.id)?;
    assert!(g.backlinks(p.id)?.is_empty());
    g.add_reference(b.id, p.id)?;
    g.delete_page(p.id)?;
    assert!(g.backlinks(p.id)?.is_empty());
    Ok(())
}

#[test]
fn sqlite_enforces_invariants_even_without_domain_api() -> Result<()> {
    let mut g = Graph::in_memory()?;
    let p = g.create_page("One")?;
    let q = g.create_page("Two")?;
    let a = g.create_block(p.id, None, "a")?;
    let b = g.create_block(p.id, Some(a.id), "b")?;
    assert!(g.conn.execute("UPDATE blocks SET parent_id=?2 WHERE id=?1", params![a.id.to_string(), b.id.to_string()]).is_err());
    assert!(g.conn.execute("UPDATE blocks SET page_id=?2 WHERE id=?1", params![a.id.to_string(), q.id.to_string()]).is_err());
    assert!(g.conn.execute("UPDATE blocks SET task_status='bogus' WHERE id=?1", [b.id.to_string()]).is_err());
    let x = Uuid::new_v4().to_string();
    let y = Uuid::new_v4().to_string();
    assert!(g.conn.execute("INSERT INTO blocks(id,page_id,parent_id,position) VALUES (?1,?3,?2,0),(?2,?3,?1,0)",
        params![x, y, p.id.to_string()]).is_err());
    assert!(g.conn.execute("INSERT INTO blocks(id,page_id,position) VALUES (?1,?2,0)", params![Uuid::new_v4().to_string(), p.id.to_string()]).is_err());
    assert!(g.conn.execute("INSERT INTO journals(page_id,date) VALUES (?1,'2023-02-29')", [q.id.to_string()]).is_err());
    let bad_fk: i64 = g.conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
    assert_eq!(bad_fk, 0);
    Ok(())
}

#[test]
fn migrations_upgrade_preserve_data_and_rerun() -> Result<()> {
    let mut conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    conn.execute_batch("CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,name TEXT NOT NULL,source TEXT NOT NULL,applied_at INTEGER NOT NULL DEFAULT(unixepoch())) STRICT;")?;
    let (name, source) = migrations::MIGRATIONS[0];
    conn.execute_batch(source)?;
    conn.execute("INSERT INTO schema_migrations(version,name,source) VALUES(1,?1,?2)", params![name, source])?;
    let id = Uuid::new_v4().to_string();
    conn.execute("INSERT INTO pages(id,title,title_key) VALUES(?1,'Existing','existing')", [&id])?;
    let block = Uuid::new_v4().to_string();
    conn.execute("INSERT INTO blocks(id,page_id,position,content) VALUES(?1,?2,0,'[[Existing]] [[Missing]]')", params![block, id])?;
    migrations::migrate(&mut conn)?;
    migrations::migrate(&mut conn)?;
    assert_eq!(conn.query_row("SELECT count(*) FROM schema_migrations", [], |r| r.get::<_, i64>(0))?, migrations::MIGRATIONS.len() as i64);
    assert_eq!(conn.query_row("SELECT id FROM pages", [], |r| r.get::<_, String>(0))?, id);
    assert_eq!(conn.query_row("SELECT count(*) FROM pages_fts WHERE pages_fts MATCH 'Existing'", [], |r| r.get::<_, i64>(0))?, 1);
    assert_eq!(conn.query_row("SELECT count(*) FROM inline_references", [], |r| r.get::<_, i64>(0))?, 2);
    assert_eq!(conn.query_row("SELECT target_page_id FROM inline_references WHERE title_key='existing'", [], |r| r.get::<_, String>(0))?, id);
    Ok(())
}

#[test]
fn migrations_reject_future_modified_gapped_and_unknown_databases() -> Result<()> {
    let mut g = Graph::in_memory()?;
    g.conn.execute("UPDATE schema_migrations SET source=replace(source,char(10),char(13)||char(10))", [])?;
    migrations::migrate(&mut g.conn)?;
    g.conn.execute("INSERT INTO schema_migrations(version,name,source) VALUES(99,'future','future')", [])?;
    assert!(matches!(migrations::migrate(&mut g.conn), Err(Error::Migration(_))));
    g.conn.execute("DELETE FROM schema_migrations WHERE version=99", [])?;
    g.conn.execute("UPDATE schema_migrations SET source='changed' WHERE version=1", [])?;
    assert!(migrations::migrate(&mut g.conn).is_err());
    let mut g = Graph::in_memory()?;
    g.conn.execute("DELETE FROM schema_migrations WHERE version=1", [])?;
    assert!(migrations::migrate(&mut g.conn).is_err());
    let mut unknown = Connection::open_in_memory()?;
    unknown.execute_batch("CREATE TABLE unrelated(id INTEGER);")?;
    assert!(migrations::migrate(&mut unknown).is_err());
    assert_eq!(unknown.query_row("SELECT count(*) FROM sqlite_master WHERE name='schema_migrations'", [], |r| r.get::<_, i64>(0))?, 0);
    Ok(())
}

#[test]
fn migration_failure_rolls_back_schema_and_history() -> Result<()> {
    let mut conn = Connection::open_in_memory()?;
    conn.execute_batch("CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,name TEXT NOT NULL,source TEXT NOT NULL,applied_at INTEGER NOT NULL DEFAULT(unixepoch())) STRICT;")?;
    let (name, source) = migrations::MIGRATIONS[0];
    conn.execute_batch(source)?;
    conn.execute("INSERT INTO schema_migrations(version,name,source) VALUES(1,?1,?2)", params![name, source])?;
    conn.execute_batch("CREATE TABLE blocks_fts(conflict TEXT);")?;
    assert!(migrations::migrate(&mut conn).is_err());
    assert_eq!(conn.query_row("SELECT count(*) FROM schema_migrations", [], |r| r.get::<_, i64>(0))?, 1);
    assert_eq!(conn.query_row("SELECT count(*) FROM sqlite_master WHERE name='pages_fts'", [], |r| r.get::<_, i64>(0))?, 0);
    assert_eq!(conn.query_row("SELECT count(*) FROM sqlite_master WHERE name='blocks_fts'", [], |r| r.get::<_, i64>(0))?, 1);
    Ok(())
}

#[test]
fn randomized_reorders_match_a_vector_model() -> Result<()> {
    let mut g = Graph::in_memory()?;
    let p = g.create_page("Ordering")?;
    let mut expected = Vec::new();
    for i in 0..12 { expected.push(g.create_block(p.id, None, &i.to_string())?.id); }
    let mut seed = 42_u64;
    for _ in 0..150 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let from = (seed % expected.len() as u64) as usize;
        let to = ((seed >> 32) % expected.len() as u64) as usize;
        let id = expected.remove(from);
        expected.insert(to, id);
        g.move_block(id, None, to)?;
        let actual = g.children(p.id, None)?;
        assert_eq!(actual.iter().map(|b| b.id).collect::<Vec<_>>(), expected);
        assert_eq!(actual.iter().map(|b| b.position).collect::<Vec<_>>(), (0..12).collect::<Vec<_>>());
    }
    Ok(())
}
#[test]
fn persian_search_normalization_backfill_rebuild_and_rollback() -> Result<()> {
    assert_eq!(normalize_persian("ي ك ۰۱۲۳۴۵۶۷۸۹ ٠١٢٣٤٥٦٧٨٩ می\u{200c}روم e\u{301}"), "ی ک 0123456789 0123456789 می روم é");
    let mut conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=ON;
        CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,name TEXT NOT NULL,source TEXT NOT NULL,applied_at INTEGER NOT NULL DEFAULT(unixepoch())) STRICT;")?;
    for (i, (name, sql)) in migrations::MIGRATIONS.iter().take(4).enumerate() {
        conn.execute_batch(sql)?;
        conn.execute("INSERT INTO schema_migrations(version,name,source) VALUES(?1,?2,?3)", params![i as i64+1, name, sql])?;
    }
    let id = Uuid::new_v4();
    let raw = "كتاب يكي ۱۲٣ می\u{200c}روم";
    conn.execute("INSERT INTO pages(id,title,title_key) VALUES(?1,?2,?2)", params![id.to_string(), raw])?;
    conn.execute_batch("CREATE TABLE blocks_persian_fts(conflict TEXT)")?;
    assert!(migrations::migrate(&mut conn).is_err());
    assert_eq!(conn.query_row("SELECT count(*) FROM schema_migrations", [], |r| r.get::<_, i64>(0))?, 4);
    assert!(!conn.prepare("SELECT 1 FROM sqlite_schema WHERE name='pages_persian_fts'")?.exists([])?);
    conn.execute_batch("DROP TABLE blocks_persian_fts")?;
    migrations::migrate(&mut conn)?;
    let mut g = Graph { conn };
    assert_eq!(g.search_mode("کتاب یکی 123 می روم", true)?[0].text, raw);
    assert!(g.search("کتاب")?.is_empty());
    assert_eq!(g.search("كتاب")?.len(), 1);
    assert!(g.search_mode("میروم", true)?.is_empty());
    g.conn.execute_batch("DELETE FROM pages_persian_fts")?;
    assert!(g.search_mode("کتاب", true)?.is_empty());
    g.rebuild_persian_search()?;
    assert_eq!(g.search_mode("کتاب", true)?.len(), 1);
    g.rename_page(id, "تازه كاری")?;
    assert!(g.search_mode("کتاب", true)?.is_empty());
    assert_eq!(g.search_mode("کاری", true)?.len(), 1);
    g.delete_page(id)?;
    assert!(g.search_mode("کاری", true)?.is_empty());
    Ok(())
}
