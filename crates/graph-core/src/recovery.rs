//! Durable text drafts and pending journal notes, separate from the graph.
//!
//! The caller chooses one app-private local file per graph and creates its private
//! parent directory. This module never creates/chmods parents or opens graph data.
//! Times are UTC Unix seconds. Unresolved versions are retained until explicit
//! removal, an exact-content acknowledgement, or 14 days after their last `put`.
//! Expiry is performed by `list`; reads never refresh it. An unused store therefore
//! retains expired rows until it is listed again.
//!
//! Storage is unencrypted. `secure_delete` is not a secure-erasure guarantee for
//! WAL, filesystem snapshots, or backups. Never log draft/base text.

use std::{fmt, fs::OpenOptions, io, path::Path, time::Duration};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

const RETENTION_SECONDS: i64 = 14 * 24 * 60 * 60;
const APPLICATION_ID: i64 = 0x47524452; // GRDR; deliberately not a graph database.
const STORE_VERSION: i64 = 2;
const NOTES_SQL: &str = "CREATE TABLE pending_journal_notes (
    id TEXT PRIMARY KEY NOT NULL,
    graph_id TEXT NOT NULL,
    block_id TEXT NOT NULL,
    journal_date TEXT NOT NULL,
    content TEXT NOT NULL,
    version INTEGER NOT NULL CHECK(version > 0),
    updated_at INTEGER NOT NULL
) STRICT";
const NOTES_INDEX_SQL: &str = "CREATE INDEX pending_journal_notes_expiry ON pending_journal_notes(graph_id, updated_at, id)";
// Persistent generation survives conditional removal/expiry and ID reuse (ABA).
const GENERATION_SQL: &str = "CREATE TABLE pending_note_generation (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    version INTEGER NOT NULL CHECK(version >= 0)
) STRICT";
const TABLE_SQL: &str = "CREATE TABLE recovery_drafts (
    id TEXT PRIMARY KEY NOT NULL,
    graph_id TEXT NOT NULL,
    page_id TEXT NOT NULL,
    block_id TEXT NOT NULL,
    base TEXT NOT NULL,
    revision INTEGER NOT NULL,
    content TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE (graph_id, block_id, base, revision)
) STRICT";
const INDEX_SQL: &str =
    "CREATE INDEX recovery_drafts_expiry ON recovery_drafts(graph_id, updated_at, id)";

#[derive(Clone, Serialize, Deserialize)]
pub struct RecoveryDraft {
    pub id: Uuid,
    pub graph_id: Uuid,
    pub page_id: Uuid,
    pub block_id: Uuid,
    pub base: String,
    pub revision: i64,
    pub content: String,
    pub updated_at: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PendingJournalNote {
    pub id: Uuid,
    pub graph_id: Uuid,
    pub block_id: Uuid,
    pub journal_date: String,
    pub content: String,
    pub version: i64,
    pub updated_at: i64,
}

impl fmt::Debug for PendingJournalNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingJournalNote").field("id", &self.id)
            .field("content", &"[redacted]").field("version", &self.version).finish_non_exhaustive()
    }
}

// Keep note text out of incidental diagnostics, including assertion failures.
impl fmt::Debug for RecoveryDraft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecoveryDraft")
            .field("id", &self.id)
            .field("graph_id", &self.graph_id)
            .field("page_id", &self.page_id)
            .field("block_id", &self.block_id)
            .field("base", &"[redacted]")
            .field("revision", &self.revision)
            .field("content", &"[redacted]")
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

pub struct RecoveryStore {
    conn: Connection,
}

impl RecoveryStore {
    /// Capture pre-submission text without opening or locking the graph. The
    /// generation changes even for identical text and survives removal/ID reuse.
    pub fn note_put(&mut self, graph: Uuid, id: Uuid, block: Uuid, date: &str, content: &str, now: i64) -> Result<PendingJournalNote> {
        crate::validate_journal_date(date)?;
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // An ID is bound to the intended graph/block/date while it exists.
        let identity: Option<(String, String, String)> = tx.query_row(
            "SELECT graph_id,block_id,journal_date FROM pending_journal_notes WHERE id=?1", [id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
        if identity.is_some_and(|v| v != (graph.to_string(), block.to_string(), date.into())) {
            return Err(Error::Conflict("pending note identity changed".into()));
        }
        if let Some(cutoff) = now.checked_sub(RETENTION_SECONDS) {
            tx.execute("DELETE FROM pending_journal_notes WHERE graph_id=?1 AND updated_at<=?2", params![graph.to_string(), cutoff])?;
        }
        let version: i64 = tx.query_row("UPDATE pending_note_generation SET version=version+1 WHERE singleton=1 RETURNING version", [], |r| r.get(0))?;
        let note = tx.query_row("INSERT INTO pending_journal_notes(id,graph_id,block_id,journal_date,content,version,updated_at)
            VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET content=excluded.content,version=excluded.version,updated_at=excluded.updated_at
            RETURNING id,graph_id,block_id,journal_date,content,version,updated_at",
            params![id.to_string(), graph.to_string(), block.to_string(), date, content, version, now], read_note)?;
        tx.commit()?;
        Ok(note)
    }

    /// Expire at age >= 14 days. Reads never extend retention.
    pub fn notes(&mut self, graph: Uuid, now: i64) -> Result<Vec<PendingJournalNote>> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(cutoff) = now.checked_sub(RETENTION_SECONDS) {
            tx.execute("DELETE FROM pending_journal_notes WHERE graph_id=?1 AND updated_at<=?2", params![graph.to_string(), cutoff])?;
        }
        let notes = {
            let mut stmt = tx.prepare("SELECT id,graph_id,block_id,journal_date,content,version,updated_at FROM pending_journal_notes
                WHERE graph_id=?1 ORDER BY updated_at DESC,id DESC")?;
            let rows = stmt.query_map([graph.to_string()], read_note)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        tx.commit()?;
        Ok(notes)
    }

    /// Same generation guard for acknowledged submission and confirmed discard.
    pub fn note_remove(&mut self, graph: Uuid, id: Uuid, version: i64) -> Result<()> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let removed = tx.execute("DELETE FROM pending_journal_notes WHERE graph_id=?1 AND id=?2 AND version=?3",
            params![graph.to_string(), id.to_string(), version])?;
        if removed == 0 && tx.query_row("SELECT EXISTS(SELECT 1 FROM pending_journal_notes WHERE graph_id=?1 AND id=?2)",
            params![graph.to_string(), id.to_string()], |r| r.get::<_, bool>(0))? {
            return Err(Error::Conflict("pending note changed; refresh before discarding".into()));
        }
        tx.commit()?;
        Ok(())
    }

    /// Open/create a local recovery file. Missing parents and final-file symlinks
    /// are errors. Unix files are created mode 0600; existing files must already
    /// have no group/other permissions. The caller owns parent-directory privacy.
    /// Unknown identities/versions, changed schemas and corrupt stores fail closed;
    /// no reset, salvage, or deletion is attempted on failure.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path == Path::new(":memory:") {
            return Err(Error::Invalid("recovery requires a durable file".into()));
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(path) {
            Ok(file) => drop(file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() {
            return Err(Error::Invalid("recovery path must be a regular file".into()));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 || metadata.permissions().mode() & 0o600 != 0o600 {
                return Err(Error::Invalid("recovery file must be private (mode 0600)".into()));
            }
        }
        // NOFOLLOW also closes the final-component symlink race after inspection.
        // Do not pass CREATE: the private file was exclusively reserved above.
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        conn.busy_timeout(Duration::from_millis(250))?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "secure_delete", "ON")?;
        initialize(&mut conn)?;
        // Validate identity/version before changing the database's journal mode.
        let mode: String = conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(Error::Invalid("recovery store requires WAL mode".into()));
        }
        Ok(Self { conn })
    }

    /// Durably capture a version. Only the same graph/block/base/revision is
    /// coalesced; other unresolved versions survive. Success means commit, not a
    /// canonical graph save. Failed writes leave the previous capture intact.
    #[allow(clippy::too_many_arguments)]
    pub fn put(
        &mut self,
        graph: Uuid,
        page: Uuid,
        block: Uuid,
        base: &str,
        revision: i64,
        content: &str,
        now: i64,
    ) -> Result<RecoveryDraft> {
        if revision < 0 { return Err(Error::Invalid("invalid recovery text revision".into())); }
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(cutoff) = now.checked_sub(RETENTION_SECONDS) {
            tx.execute("DELETE FROM recovery_drafts WHERE graph_id=?1 AND updated_at<=?2", params![graph.to_string(), cutoff])?;
        }
        let draft = tx.query_row(
            "INSERT INTO recovery_drafts
             (id, graph_id, page_id, block_id, base, revision, content, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(graph_id, block_id, base, revision) DO UPDATE SET
                 page_id=excluded.page_id, content=excluded.content, updated_at=excluded.updated_at
             RETURNING id, graph_id, page_id, block_id, base, revision, content, updated_at",
            params![Uuid::new_v4().to_string(), graph.to_string(), page.to_string(),
                block.to_string(), base, revision, content, now],
            read_draft,
        )?;
        tx.commit()?;
        Ok(draft)
    }

    /// Expire this graph's captures at age >= 14 days, then list newest first.
    /// Cleanup and decoding commit together: any failure rolls cleanup back.
    /// Listing neither refreshes timestamps nor expires another graph's rows.
    pub fn list(&mut self, graph: Uuid, now: i64) -> Result<Vec<RecoveryDraft>> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(cutoff) = now.checked_sub(RETENTION_SECONDS) {
            tx.execute(
                "DELETE FROM recovery_drafts WHERE graph_id=?1 AND updated_at<=?2",
                params![graph.to_string(), cutoff],
            )?;
        }
        let drafts = {
            let mut stmt = tx.prepare(
                "SELECT id, graph_id, page_id, block_id, base, revision, content, updated_at
                 FROM recovery_drafts WHERE graph_id=?1 ORDER BY updated_at DESC, id DESC",
            )?;
            let rows = stmt.query_map([graph.to_string()], read_draft)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        tx.commit()?;
        Ok(drafts)
    }

    /// Explicit discard, scoped to the owning graph. Missing rows are harmless.
    pub fn remove(&mut self, graph: Uuid, id: Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM recovery_drafts WHERE graph_id=?1 AND id=?2",
            params![graph.to_string(), id.to_string()],
        )?;
        Ok(())
    }

    /// UI discard must not remove a newer snapshot that reused this row ID.
    pub fn remove_matching(&mut self, graph: Uuid, id: Uuid, content: &str, updated_at: i64) -> Result<()> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let removed = tx.execute("DELETE FROM recovery_drafts WHERE graph_id=?1 AND id=?2 AND content=?3 AND updated_at=?4",
            params![graph.to_string(), id.to_string(), content, updated_at])?;
        if removed == 0 && tx.query_row("SELECT EXISTS(SELECT 1 FROM recovery_drafts WHERE graph_id=?1 AND id=?2)",
            params![graph.to_string(), id.to_string()], |r| r.get::<_, bool>(0))? {
            return Err(Error::Conflict("recovery draft changed; refresh before discarding".into()));
        }
        tx.commit()?; Ok(())
    }

    /// Clear only captures whose exact content has been acknowledged/discarded.
    /// A late acknowledgement must not delete a newer, different capture.
    pub fn clear_matching(&mut self, graph: Uuid, block: Uuid, content: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM recovery_drafts WHERE graph_id=?1 AND block_id=?2 AND content=?3",
            params![graph.to_string(), block.to_string(), content],
        )?;
        Ok(())
    }
}

fn initialize(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let identity: i64 = tx.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let objects = {
        let mut stmt = tx.prepare(
            "SELECT type, name, sql FROM sqlite_schema
             WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let empty = version == 0 && identity == 0 && objects.is_empty();
    if !empty {
        let mut expected = vec![
            ("table".to_owned(), "recovery_drafts".to_owned(), TABLE_SQL.to_owned()),
            ("index".to_owned(), "recovery_drafts_expiry".to_owned(), INDEX_SQL.to_owned()),
        ];
        if version == STORE_VERSION {
            expected.extend([
                ("table".into(), "pending_journal_notes".into(), NOTES_SQL.into()),
                ("index".into(), "pending_journal_notes_expiry".into(), NOTES_INDEX_SQL.into()),
                ("table".into(), "pending_note_generation".into(), GENERATION_SQL.into()),
            ]);
            expected.sort_by(|a, b| a.1.cmp(&b.1));
        }
        if identity != APPLICATION_ID || !matches!(version, 1 | STORE_VERSION) || objects != expected {
            return Err(Error::Migration("unknown recovery store identity, version or schema".into()));
        }
    }
    // Do not return SQLite's integrity diagnostics: they may contain stored text.
    let healthy: String = tx.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if healthy != "ok" {
        return Err(Error::Invalid("recovery store failed integrity check".into()));
    }
    if empty {
        tx.execute_batch(TABLE_SQL)?;
        tx.execute_batch(INDEX_SQL)?;
        tx.pragma_update(None, "application_id", APPLICATION_ID)?;
    }
    if empty || version == 1 {
        tx.execute_batch(NOTES_SQL)?;
        tx.execute_batch(NOTES_INDEX_SQL)?;
        tx.execute_batch(GENERATION_SQL)?;
        tx.execute("INSERT INTO pending_note_generation VALUES(1,0)", [])?;
        tx.pragma_update(None, "user_version", STORE_VERSION)?;
    }
    let generation: Option<i64> = tx.query_row("SELECT version FROM pending_note_generation WHERE singleton=1", [], |r| r.get(0)).optional()?;
    if generation.is_none() { return Err(Error::Migration("missing pending note generation".into())); }
    tx.commit()?;
    Ok(())
}

fn id(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Uuid> {
    let text: String = row.get(index)?;
    Uuid::parse_str(&text).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(io::Error::new(io::ErrorKind::InvalidData, "invalid recovery identifier")),
        )
    })
}

fn read_note(row: &rusqlite::Row<'_>) -> rusqlite::Result<PendingJournalNote> {
    let date: String = row.get(3)?;
    crate::validate_journal_date(&date).map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(PendingJournalNote { id: id(row, 0)?, graph_id: id(row, 1)?, block_id: id(row, 2)?,
        journal_date: date, content: row.get(4)?, version: row.get(5)?, updated_at: row.get(6)? })
}

fn read_draft(row: &rusqlite::Row<'_>) -> rusqlite::Result<RecoveryDraft> {
    Ok(RecoveryDraft {
        id: id(row, 0)?,
        graph_id: id(row, 1)?,
        page_id: id(row, 2)?,
        block_id: id(row, 3)?,
        base: row.get(4)?,
        revision: row.get(5)?,
        content: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> (Uuid, Uuid, Uuid) {
        (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4())
    }

    #[test]
    fn v1_upgrade_preserves_drafts_and_pending_notes_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v1.db");
        let (graph, page, block) = ids();
        let draft_id = Uuid::new_v4();
        {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
            drop(options.open(&path).unwrap());
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(TABLE_SQL).unwrap();
            conn.execute_batch(INDEX_SQL).unwrap();
            conn.pragma_update(None, "application_id", APPLICATION_ID).unwrap();
            conn.pragma_update(None, "user_version", 1).unwrap();
            conn.execute("INSERT INTO recovery_drafts VALUES(?1,?2,?3,?4,'base',7,'retained 😀',100)",
                params![draft_id.to_string(), graph.to_string(), page.to_string(), block.to_string()]).unwrap();
        }
        let note_id = Uuid::new_v4();
        let note = {
            let mut store = RecoveryStore::open(&path).unwrap();
            assert_eq!(store.list(graph, 101).unwrap()[0].id, draft_id);
            let draft = &store.list(graph, 101).unwrap()[0];
            assert_eq!((&*draft.base, draft.revision, &*draft.content), ("base", 7, "retained 😀"));
            store.note_put(graph, note_id, block, "2024-02-29", "پیش\u{200c}نویس 😀\n", 101).unwrap()
        };
        let mut store = RecoveryStore::open(&path).unwrap();
        let recovered = &store.notes(graph, 102).unwrap()[0];
        assert_eq!(recovered.id, note.id);
        assert_eq!(recovered.journal_date, "2024-02-29");
        assert_eq!(recovered.content, note.content);
        assert!(!format!("{recovered:?}").contains("پیش"));
        assert_eq!(store.list(graph, 102).unwrap()[0].id, draft_id);
    }

    #[test]
    fn pending_note_generation_identity_expiry_and_failure_are_safe() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = RecoveryStore::open(dir.path().join("notes.db")).unwrap();
        let (graph, _, block) = ids();
        let id = Uuid::new_v4();
        let first = store.note_put(graph, id, block, "2024-02-29", "same", 100).unwrap();
        let second = store.note_put(graph, id, block, "2024-02-29", "same", 100).unwrap();
        assert!(second.version > first.version);
        assert!(store.note_remove(graph, id, first.version).is_err());
        assert_eq!(store.notes(graph, 101).unwrap().len(), 1);
        store.note_remove(Uuid::new_v4(), id, second.version).unwrap();
        assert_eq!(store.notes(graph, 101).unwrap().len(), 1);
        store.note_remove(graph, id, second.version).unwrap();
        let third = store.note_put(graph, id, block, "2024-02-29", "same", 100).unwrap();
        assert!(third.version > second.version);
        assert!(store.note_remove(graph, id, second.version).is_err());
        for (g, b, date) in [(Uuid::new_v4(), block, "2024-02-29"), (graph, Uuid::new_v4(), "2024-02-29"), (graph, block, "2024-03-01")] {
            assert!(store.note_put(g, id, b, date, "wrong identity", 101).is_err());
        }
        assert!(store.note_put(graph, Uuid::new_v4(), block, "2023-02-29", "bad date", 101).is_err());
        store.conn.execute_batch("CREATE TEMP TRIGGER fail_note BEFORE UPDATE ON pending_journal_notes BEGIN SELECT RAISE(ABORT,'injected'); END").unwrap();
        assert!(store.note_put(graph, id, block, "2024-02-29", "not captured", 101).is_err());
        assert_eq!(store.notes(graph, 102).unwrap()[0].version, third.version);
        assert_eq!(store.notes(graph, 100 + RETENTION_SECONDS - 1).unwrap()[0].updated_at, 100);
        assert!(store.notes(graph, 100 + RETENTION_SECONDS).unwrap().is_empty());
        store.note_remove(graph, id, third.version).unwrap();
    }

    #[test]
    fn v1_unknown_schema_is_not_upgraded_and_failed_upgrade_rolls_back() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(TABLE_SQL).unwrap();
        conn.execute_batch(INDEX_SQL).unwrap();
        conn.pragma_update(None, "application_id", APPLICATION_ID).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute_batch("CREATE TABLE unexpected(x TEXT)").unwrap();
        assert!(initialize(&mut conn).is_err());
        assert_eq!(conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 1);
        assert!(!conn.prepare("SELECT 1 FROM sqlite_schema WHERE name='pending_journal_notes'").unwrap().exists([]).unwrap());
        conn.execute_batch("DROP TABLE unexpected; PRAGMA query_only=ON").unwrap();
        assert!(initialize(&mut conn).is_err());
        assert_eq!(conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 1);
    }

    #[test]
    fn unicode_roundtrip_reopen_and_private_debug() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("بازیابی.sqlite3");
        let (graph, page, block) = ids();
        let base = "پایه\u{200c}ی متن\n🦀";
        let content = "ویرایش\u{200c}شده\nMixed RTL فارسی 👩🏽‍💻\0tail";
        let draft = {
            let mut store = RecoveryStore::open(&path).unwrap();
            let mode: String = store.conn.pragma_query_value(None, "journal_mode", |r| r.get(0)).unwrap();
            let sync: i64 = store.conn.pragma_query_value(None, "synchronous", |r| r.get(0)).unwrap();
            let secure: i64 = store.conn.pragma_query_value(None, "secure_delete", |r| r.get(0)).unwrap();
            assert_eq!((mode.as_str(), sync, secure), ("wal", 2, 1));
            store.put(graph, page, block, base, 7, content, 100).unwrap()
        };
        let mut store = RecoveryStore::open(&path).unwrap();
        let rows = store.list(graph, 101).unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!((row.id, row.graph_id, row.page_id, row.block_id), (draft.id, graph, page, block));
        assert_eq!((&*row.base, row.revision, &*row.content, row.updated_at), (base, 7, content, 100));
        let debug = format!("{:?}", row.clone());
        assert!(!debug.contains(base));
        assert!(!debug.contains("ویرایش"));
        assert!(debug.contains("[redacted]"));
    }

    #[test]
    fn coalesces_only_identical_base_and_revision() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = RecoveryStore::open(dir.path().join("drafts.db")).unwrap();
        let (graph, page, block) = ids();
        let first = store.put(graph, page, block, "base one", 1, "older", 100).unwrap();
        let second = store.put(graph, page, block, "base two", 1, "unresolved", 101).unwrap();
        let third = store.put(graph, page, block, "base one", 2, "other revision", 102).unwrap();
        let updated = store.put(graph, page, block, "base one", 1, "newest", 103).unwrap();
        assert_eq!(first.id, updated.id);
        assert_ne!(second.id, updated.id);
        assert_ne!(third.id, updated.id);
        let rows = store.list(graph, 104).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].id, first.id);
        assert_eq!(rows[0].content, "newest");
        assert!(rows.iter().any(|r| r.id == second.id && r.content == "unresolved"));
        assert!(rows.iter().any(|r| r.id == third.id && r.content == "other revision"));
    }

    #[test]
    fn cleanup_is_exact_and_graph_block_scoped() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = RecoveryStore::open(dir.path().join("drafts.db")).unwrap();
        let (graph, page, block) = ids();
        let other_graph = Uuid::new_v4();
        let other_block = Uuid::new_v4();
        let current = store.put(graph, page, block, "base", 1, "old", 100).unwrap();
        store.put(graph, page, block, "base", 1, "new", 101).unwrap();
        store.put(graph, page, block, "other base", 2, "old", 100).unwrap();
        store.put(other_graph, page, block, "base", 1, "old", 100).unwrap();
        store.put(graph, page, other_block, "base", 1, "old", 100).unwrap();
        store.clear_matching(graph, block, "old").unwrap();
        let rows = store.list(graph, 102).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.id == current.id && r.content == "new"));
        assert_eq!(store.list(other_graph, 102).unwrap().len(), 1);
        store.remove(other_graph, current.id).unwrap();
        assert_eq!(store.list(graph, 102).unwrap().len(), 2);
        store.remove(graph, current.id).unwrap();
        store.remove(graph, current.id).unwrap();
        assert_eq!(store.list(graph, 102).unwrap()[0].block_id, other_block);
    }

    #[test]
    fn stale_discard_cannot_delete_a_newer_capture() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = RecoveryStore::open(dir.path().join("drafts.db")).unwrap();
        let (graph, page, block) = ids();
        let first = store.put(graph, page, block, "base", 1, "first", 100).unwrap();
        store.put(graph, page, block, "base", 1, "newer", 100).unwrap();
        assert!(store.remove_matching(graph, first.id, "first", 100).is_err());
        assert_eq!(store.list(graph, 101).unwrap()[0].content, "newer");
        store.remove_matching(graph, first.id, "newer", 100).unwrap();
        assert!(store.list(graph, 101).unwrap().is_empty());
    }

    #[test]
    fn expiration_boundary_never_refreshes_on_read() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = RecoveryStore::open(dir.path().join("drafts.db")).unwrap();
        let (graph, page, block) = ids();
        let other_graph = Uuid::new_v4();
        store.put(graph, page, block, "base", 1, "draft", 100).unwrap();
        store.put(other_graph, page, block, "base", 1, "other graph", 100).unwrap();
        assert_eq!(store.list(graph, 100 + RETENTION_SECONDS - 1).unwrap()[0].updated_at, 100);
        assert!(store.list(graph, 100 + RETENTION_SECONDS).unwrap().is_empty());
        // Expiration only affects the requested graph, even if another has expired.
        let count: i64 = store.conn.query_row("SELECT count(*) FROM recovery_drafts", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
        store.put(graph, page, block, "base", 1, "edge", i64::MIN).unwrap();
        assert_eq!(store.list(graph, i64::MIN).unwrap().len(), 1);
        assert!(store.list(graph, i64::MIN + RETENTION_SECONDS).unwrap().is_empty());
    }

    #[test]
    fn failed_update_and_failed_list_roll_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = RecoveryStore::open(dir.path().join("drafts.db")).unwrap();
        let (graph, page, block) = ids();
        let original = store.put(graph, page, block, "base", 1, "captured", 100).unwrap();
        store.conn.execute_batch(
            "CREATE TEMP TRIGGER fail_capture BEFORE UPDATE ON recovery_drafts
             BEGIN SELECT RAISE(ABORT, 'injected capture failure'); END;",
        ).unwrap();
        assert!(store.put(graph, page, block, "base", 1, "uncaptured", 101).is_err());
        let rows = store.list(graph, 101).unwrap();
        assert_eq!((rows[0].id, rows[0].content.as_str(), rows[0].updated_at), (original.id, "captured", 100));
        store.conn.execute_batch("DROP TRIGGER fail_capture").unwrap();

        // A bad unexpired row fails decoding after DELETE ran: the expired row
        // must still survive because cleanup and the read share a transaction.
        let bad = store.put(graph, page, Uuid::new_v4(), "base", 1, "recent", 100 + RETENTION_SECONDS - 1).unwrap();
        store.conn.execute("UPDATE recovery_drafts SET page_id='invalid' WHERE id=?1", [bad.id.to_string()]).unwrap();
        assert!(store.list(graph, 100 + RETENTION_SECONDS).is_err());
        let count: i64 = store.conn.query_row("SELECT count(*) FROM recovery_drafts", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 2);
        store.remove(graph, bad.id).unwrap();
        assert!(store.list(graph, 100 + RETENTION_SECONDS).unwrap().is_empty());
    }

    #[test]
    fn read_only_and_busy_errors_preserve_capture() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("drafts.db");
        let mut store = RecoveryStore::open(&path).unwrap();
        let (graph, page, block) = ids();
        let original = store.put(graph, page, block, "base", 1, "captured", 100).unwrap();
        store.conn.pragma_update(None, "query_only", "ON").unwrap();
        assert!(store.put(graph, page, block, "base", 1, "uncaptured", 101).is_err());
        assert!(store.remove(graph, original.id).is_err());
        assert!(store.clear_matching(graph, block, "captured").is_err());
        assert!(store.list(graph, 100 + RETENTION_SECONDS).is_err());
        store.conn.pragma_update(None, "query_only", "OFF").unwrap();
        let writer = Connection::open(&path).unwrap();
        writer.execute_batch("BEGIN IMMEDIATE").unwrap();
        let error = store.put(graph, page, block, "base", 1, "blocked", 102).unwrap_err();
        assert!(matches!(error, Error::Database(rusqlite::Error::SqliteFailure(ref e, _))
            if e.code == rusqlite::ErrorCode::DatabaseBusy));
        writer.execute_batch("ROLLBACK").unwrap();
        assert_eq!(store.list(graph, 103).unwrap()[0].content, "captured");
    }

    #[test]
    fn bounded_sqlite_full_rolls_back_and_reopens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("drafts.db");
        let (graph, page, block) = ids();
        let original = {
            let mut store = RecoveryStore::open(&path).unwrap();
            let original = store.put(graph, page, block, "base", 1, "captured", 100).unwrap();
            let pages: i64 = store.conn.pragma_query_value(None, "page_count", |r| r.get(0)).unwrap();
            store.conn.pragma_update(None, "max_page_count", pages + 2).unwrap();
            // Database-local page cap, not disk exhaustion. Allocation is 128 KiB.
            let large = "x".repeat(128 * 1024);
            let error = store.put(graph, page, block, "base", 1, &large, 101).unwrap_err();
            assert!(matches!(error, Error::Database(rusqlite::Error::SqliteFailure(ref e, _))
                if e.code == rusqlite::ErrorCode::DiskFull));
            let rows = store.list(graph, 102).unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!((rows[0].id, rows[0].content.as_str(), rows[0].updated_at), (original.id, "captured", 100));
            original
        };
        let mut store = RecoveryStore::open(&path).unwrap();
        assert_eq!(store.list(graph, 103).unwrap()[0].id, original.id);
        store.put(graph, page, block, "base", 1, "retry", 104).unwrap();
        assert_eq!(store.list(graph, 105).unwrap()[0].content, "retry");
    }

    #[test]
    fn missing_parent_does_not_create_directories() {
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("missing");
        assert!(RecoveryStore::open(parent.join("drafts.db")).is_err());
        assert!(!parent.exists());
    }

    #[test]
    fn unknown_version_schema_and_corruption_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("drafts.db");
        drop(RecoveryStore::open(&path).unwrap());
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA user_version=99").unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        assert!(RecoveryStore::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA user_version=2; DROP INDEX recovery_drafts_expiry").unwrap();
        }
        assert!(RecoveryStore::open(&path).is_err());
        std::fs::write(&path, b"not a SQLite database").unwrap();
        assert!(RecoveryStore::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"not a SQLite database");
    }

    #[cfg(unix)]
    #[test]
    fn private_files_read_only_open_and_symlink_rejection() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("drafts.db");
        let parent_mode = std::fs::metadata(dir.path()).unwrap().permissions().mode();
        let mut store = RecoveryStore::open(&path).unwrap();
        let (graph, page, block) = ids();
        store.put(graph, page, block, "base", 1, "captured", 100).unwrap();
        for file in [&path, &dir.path().join("drafts.db-wal"), &dir.path().join("drafts.db-shm")] {
            assert_eq!(std::fs::metadata(file).unwrap().permissions().mode() & 0o777, 0o600);
        }
        assert_eq!(std::fs::metadata(dir.path()).unwrap().permissions().mode(), parent_mode);
        drop(store);
        let link = dir.path().join("link.db");
        symlink(&path, &link).unwrap();
        assert!(RecoveryStore::open(&link).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
        // The authorized test user is unprivileged; no privileged permission bypass.
        assert!(RecoveryStore::open(&path).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(RecoveryStore::open(&path).is_err());
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o644);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(RecoveryStore::open(&path).unwrap().list(graph, 101).unwrap()[0].content, "captured");
    }
}
