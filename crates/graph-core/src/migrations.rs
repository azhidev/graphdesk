use rusqlite::{Connection, TransactionBehavior};
use crate::{Error, Result};

pub(crate) const MIGRATIONS: &[(&str, &str)] = &[
    ("initial", include_str!("../migrations/001_initial.sql")),
    ("search", include_str!("../migrations/002_search.sql")),
];

pub(crate) fn migrate(conn: &mut Connection) -> Result<()> {
    // Acquire the writer lock before inspecting history, including on concurrent opens.
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let has_history: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations')",
        [], |r| r.get(0),
    )?;
    if !has_history {
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
    for (index, (name, source)) in MIGRATIONS.iter().enumerate().skip(history.len()) {
        tx.execute_batch(source)?;
        tx.execute("INSERT INTO schema_migrations(version, name, source) VALUES (?1, ?2, ?3)",
            rusqlite::params![index as i64 + 1, name, source.replace("\r\n", "\n")])?;
    }
    tx.commit()?;
    Ok(())
}
