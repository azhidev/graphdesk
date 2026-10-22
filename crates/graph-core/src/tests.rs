use super::*;

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
