use super::*;
use crate::{EditAction, Editor, migrations};
use serde_json::json;
use uuid::Uuid;

const ID: &str = "graphdesk.tally";

fn schema_five(path: &std::path::Path) -> Result<(Connection, Uuid)> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=ON;
        CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,name TEXT NOT NULL,source TEXT NOT NULL,
        applied_at INTEGER NOT NULL DEFAULT(unixepoch())) STRICT;")?;
    crate::search::register(&conn)?;
    let id = Uuid::new_v4();
    for (index, (name, source)) in migrations::MIGRATIONS.iter().take(5).enumerate() {
        conn.execute_batch(source)?;
        if index == 3 { conn.execute("INSERT INTO graph_identity VALUES(1,?1)", [id.to_string()])?; }
        conn.execute("INSERT INTO schema_migrations(version,name,source) VALUES(?1,?2,?3)", params![index as i64 + 1, name, source])?;
    }
    Ok((conn, id))
}

#[test]
fn migration_six_preserves_graph_and_reopens_durable_preferences_and_data() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("upgrade.db");
    let (conn, graph_id) = schema_five(&path)?;
    let page = Uuid::new_v4();
    conn.execute("INSERT INTO pages(id,title,title_key) VALUES(?1,'Retained','retained')", [page.to_string()])?;
    drop(conn);
    let mut graph = Graph::open(&path)?;
    assert_eq!(graph.id()?, graph_id);
    assert_eq!(graph.pages()?[0].id, page);
    assert!(graph.extension_states()?.is_empty());
    graph.extension_configure(ID, true)?;
    let expected = graph.extension_write(ID, 0, json!({"count": 7, "label": "سلام 😀"}))?;
    graph.extension_configure(ID, false)?;
    drop(graph);
    let mut graph = Graph::open(&path)?;
    assert_eq!(graph.id()?, graph_id);
    assert_eq!(graph.extension_states()?, vec![ExtensionState { id: ID.into(), enabled: false }]);
    assert!(graph.extension_read(ID).is_err());
    graph.extension_configure(ID, true)?;
    assert_eq!(graph.extension_read(ID)?, expected);
    assert_eq!(graph.conn.query_row("SELECT max(version) FROM schema_migrations", [], |r| r.get::<_, i64>(0))?, 6);
    drop(graph);
    assert_eq!(Graph::inspect_identity(&path, true)?, graph_id);
    assert_eq!(Graph::open(&path)?.extension_read(ID)?, expected);
    Ok(())
}

#[test]
fn migration_six_failure_rolls_back_and_retry_preserves_identity() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("rollback.db");
    let (conn, id) = schema_five(&path)?;
    conn.execute_batch("CREATE TABLE extensions(conflict TEXT)")?;
    assert!(Graph::open(&path).is_err());
    assert_eq!(conn.query_row("SELECT max(version) FROM schema_migrations", [], |r| r.get::<_, i64>(0))?, 5);
    conn.execute_batch("DROP TABLE extensions")?;
    drop(conn);
    let graph = Graph::open(&path)?;
    assert_eq!(graph.id()?, id);
    assert!(graph.extension_states()?.is_empty());
    Ok(())
}

#[test]
fn default_disabled_retains_document_and_isolates_ids_and_graphs() -> Result<()> {
    let mut first = Graph::in_memory()?;
    let mut second = Graph::in_memory()?;
    assert_ne!(first.id()?, second.id()?);
    assert!(first.extension_states()?.is_empty());
    assert!(first.extension_read(ID).is_err());
    assert!(first.extension_write(ID, 0, json!(1)).is_err());
    assert!(first.extension_states()?.is_empty()); // denied access cannot create a preference
    first.extension_configure(ID, false)?;
    assert!(first.extension_read(ID).is_err());
    first.extension_configure(ID, true)?;
    assert_eq!(first.extension_read(ID)?, ExtensionDocument { revision: 0, value: Value::Null });
    let saved = first.extension_write(ID, 0, json!({"count": 4}))?;
    first.extension_configure("other.extension", true)?;
    second.extension_configure(ID, true)?;
    for document in [first.extension_read("other.extension")?, second.extension_read(ID)?] {
        assert_eq!(document, ExtensionDocument { revision: 0, value: Value::Null });
    }
    first.extension_configure(ID, false)?;
    assert!(first.extension_read(ID).is_err());
    assert!(first.extension_write(ID, 1, json!(99)).is_err());
    first.extension_configure(ID, true)?;
    first.extension_configure(ID, true)?;
    assert_eq!(first.extension_read(ID)?, saved);
    assert!(first.conn.execute("INSERT INTO extensions(graph_id,extension_id) VALUES(?1,'foreign.extension')", [second.id()?.to_string()]).is_err());
    Ok(())
}

#[test]
fn stable_ids_are_validated_on_every_operation() -> Result<()> {
    let mut graph = Graph::in_memory()?;
    for id in ["", "a", "ab", "graphdesk", "Graphdesk.tally", ".a", "a.", "a..b", "-a.b", "a-.b",
        "a.-b", "a.b-", "a.b/c", "a.b_c", "a.é", " a.b", "a.b ", "a.b\0hidden", &format!("a.{}", "b".repeat(99))] {
        assert!(matches!(graph.extension_configure(id, true), Err(Error::Invalid(_))), "{id:?}");
        assert!(matches!(graph.extension_read(id), Err(Error::Invalid(_))), "{id:?}");
        assert!(matches!(graph.extension_write(id, 0, Value::Null), Err(Error::Invalid(_))), "{id:?}");
    }
    assert!(graph.extension_states()?.is_empty());
    for id in ["a.b", ID, "org.example.counter-2", &format!("a.{}", "b".repeat(98))] {
        graph.extension_configure(id, true)?;
        assert_eq!(graph.extension_read(id)?.revision, 0);
    }
    Ok(())
}

#[test]
fn stale_cas_and_cross_connection_disable_cannot_overwrite() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("cas.db");
    let mut graph = Graph::create(&path)?;
    graph.extension_configure(ID, true)?;
    let mut other = Graph::open(&path)?;
    assert_eq!(other.extension_read(ID)?.revision, 0);
    let first = graph.extension_write(ID, 0, json!(1))?;
    assert!(matches!(other.extension_write(ID, 0, json!(2)), Err(Error::Conflict(_))));
    assert_eq!(other.extension_read(ID)?, first);
    let second = other.extension_write(ID, 1, Value::Null)?;
    assert_eq!(second.revision, 2);
    assert!(matches!(graph.extension_write(ID, 1, json!(3)), Err(Error::Conflict(_))));
    assert!(matches!(graph.extension_write(ID, -1, Value::Null), Err(Error::Invalid(_))));
    other.extension_configure(ID, false)?;
    assert!(graph.extension_read(ID).is_err());
    assert!(graph.extension_write(ID, 2, json!(4)).is_err());
    graph.extension_configure(ID, true)?;
    assert_eq!(graph.extension_read(ID)?, second);
    // Exhaustion is a conflict, never an overflow or a wrapped revision.
    graph.conn.execute("UPDATE extensions SET revision=?1", [i64::MAX])?;
    assert!(matches!(graph.extension_write(ID, i64::MAX, json!(5)), Err(Error::Conflict(_))));
    assert_eq!(graph.extension_read(ID)?.revision, i64::MAX);
    assert_eq!(graph.extension_read(ID)?.value, Value::Null);
    Ok(())
}

#[test]
fn quota_counts_utf8_json_bytes_and_failures_leave_revision_and_value_unchanged() -> Result<()> {
    let mut graph = Graph::in_memory()?;
    graph.extension_configure(ID, true)?;
    let exact = graph.extension_write(ID, 0, json!("x".repeat(MAX_DOCUMENT_BYTES - 2)))?;
    assert_eq!(serde_json::to_vec(&exact.value).unwrap().len(), MAX_DOCUMENT_BYTES);
    for value in [json!("x".repeat(MAX_DOCUMENT_BYTES - 1)), json!("😀".repeat(MAX_DOCUMENT_BYTES / 4)), json!("\n".repeat(MAX_DOCUMENT_BYTES / 2))] {
        assert!(matches!(graph.extension_write(ID, 1, value), Err(Error::Invalid(_))));
        assert_eq!(graph.extension_read(ID)?, exact);
    }
    let escaped = graph.extension_write(ID, 1, json!("\n".repeat((MAX_DOCUMENT_BYTES - 2) / 2)))?;
    assert_eq!(escaped.revision, 2);
    assert_eq!(serde_json::to_vec(&escaped.value).unwrap().len(), MAX_DOCUMENT_BYTES);
    Ok(())
}

#[test]
fn extension_mutations_preserve_editor_undo_and_redo_without_undoing_storage() -> Result<()> {
    let mut editor = Editor::new(Graph::in_memory()?);
    let page = editor.create_page("History")?;
    let block = editor.apply(page.id, EditAction::Append, None)?.changed[0].id;
    editor.apply(page.id, EditAction::Save { id: block, expected: "".into(), content: "retained".into() }, None)?;
    editor.extension_configure(ID, true)?;
    let saved = editor.extension_write(ID, 0, json!({"count": 3}))?;
    editor.extension_configure(ID, false)?;
    assert_eq!(editor.history_state(page.id), (true, false));
    editor.history(page.id, false)?;
    assert_eq!(editor.graph().page_blocks(page.id)?[0].content, "");
    assert_eq!(editor.history_state(page.id), (true, true));
    editor.extension_configure(ID, true)?;
    assert_eq!(editor.graph().extension_read(ID)?, saved);
    let latest = editor.extension_write(ID, 1, json!({"count": 5}))?;
    editor.history(page.id, true)?;
    assert_eq!(editor.graph().page_blocks(page.id)?[0].content, "retained");
    assert_eq!(editor.graph().extension_read(ID)?, latest);
    Ok(())
}
