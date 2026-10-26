use rusqlite::{Connection, TransactionBehavior};
use crate::{Error, Result};

pub(crate) const MIGRATIONS: &[(&str, &str)] = &[
    ("initial", include_str!("../migrations/001_initial.sql")),
    ("search", include_str!("../migrations/002_search.sql")),
    ("inline_references", include_str!("../migrations/003_inline_references.sql")),
    ("reliability", include_str!("../migrations/004_reliability.sql")),
    ("persian_search", include_str!("../migrations/005_persian_search.sql")),
];

#[cfg(test)]
pub(crate) fn migrate(conn: &mut Connection) -> Result<()> {
    migrate_expected(conn, None)
}

pub(crate) fn migrate_expected(conn: &mut Connection, expected: Option<(uuid::Uuid, bool)>) -> Result<()> {
    crate::search::register(conn)?;
    // Acquire the writer lock before inspecting history, including on concurrent opens.
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let has_history: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations')",
        [], |r| r.get(0),
    )?;
    if !has_history {
        if expected.is_some() { return Err(Error::Migration("remembered graph has no migration history".into())); }
        let objects: i64 = tx.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'", [], |r| r.get(0),
        )?;
        if objects != 0 { return Err(Error::Migration("not an empty Graphdesk database".into())); }
    }
    tx.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations (
        version INTEGER PRIMARY KEY, name TEXT NOT NULL, source TEXT NOT NULL,
        applied_at INTEGER NOT NULL DEFAULT (unixepoch())
    ) STRICT;")?;
    let history = {
        let mut stmt = tx.prepare("SELECT version, name, source FROM schema_migrations ORDER BY version")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for (index, (version, name, source)) in history.iter().enumerate() {
        if *version > MIGRATIONS.len() as i64 {
            return Err(Error::Migration(format!("unsupported future schema version {version}")));
        }
        // Git checkouts may use CRLF on Windows; line endings are not schema drift.
        if *version != index as i64 + 1 || MIGRATIONS[index].0 != name.as_str()
            || MIGRATIONS[index].1.replace("\r\n", "\n") != source.replace("\r\n", "\n") {
            return Err(Error::Migration("migration history has gaps or modified source".into()));
        }
    }
    if let Some((expected, current_only)) = expected {
        if history.len() < 4 || (current_only && history.len() != MIGRATIONS.len()) {
            return Err(Error::Migration("remembered graph requires an explicit upgrade".into()));
        }
        let id: String = tx.query_row("SELECT id FROM graph_identity WHERE singleton=1", [], |r| r.get(0))?;
        if crate::parse_id(&id)? != expected {
            return Err(Error::Conflict("remembered path now contains a different graph".into()));
        }
    }
    for (index, (name, source)) in MIGRATIONS.iter().enumerate().skip(history.len()) {
        tx.execute_batch(source)?;
        if *name == "inline_references" { crate::references::backfill(&tx)?; }
        if *name == "reliability" {
            tx.execute("INSERT INTO graph_identity VALUES(1,?1)", [uuid::Uuid::new_v4().to_string()])?;
        }
        tx.execute("INSERT INTO schema_migrations(version, name, source) VALUES (?1, ?2, ?3)",
            rusqlite::params![index as i64 + 1, name, source.replace("\r\n", "\n")])?;
    }
    tx.commit()?;
    Ok(())
}
