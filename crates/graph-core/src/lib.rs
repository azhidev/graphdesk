//! Tauri-independent graph storage. Every Graph owns one SQLite connection.
mod migrations;
mod model;
mod editor;
mod references;
pub mod recovery;
pub use references::InlineReference;
pub use editor::{Editor, EditAction, EditResult, DeletePreview};
pub use model::{Block, Page, SearchHit, TaskFilter, TaskHit, TaskStatus};

use std::{fs::OpenOptions, path::Path, time::Duration};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("filesystem: {0}")]
    Io(#[from] std::io::Error),
    #[error("migration: {0}")]
    Migration(String),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("edit conflict: {0}")]
    Conflict(String),
    #[error("entity not found")]
    NotFound,
}
pub type Result<T> = std::result::Result<T, Error>;

pub struct Graph { conn: Connection }

impl Graph {
    pub fn id(&self) -> Result<Uuid> {
        let id: String = self.conn.query_row("SELECT id FROM graph_identity WHERE singleton=1", [], |r| r.get(0))?;
        parse_id(&id)
    }

    pub fn text_revisions(&self, page: Uuid) -> Result<std::collections::BTreeMap<Uuid, i64>> {
        text_revisions(&self.conn, page)
    }

    pub fn text_outline(&self, page: Uuid) -> Result<(Vec<Block>, std::collections::BTreeMap<Uuid, i64>)> {
        let tx = self.conn.unchecked_transaction()?;
        let blocks = page_blocks(&tx, page)?;
        let revisions = text_revisions(&tx, page)?;
        tx.commit()?;
        Ok((blocks, revisions))
    }
    /// Atomically reserve a new database path; never overwrite an existing graph.
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        OpenOptions::new().write(true).create_new(true).open(path)?;
        Self::open(path)
    }

    /// Open an existing database, applying pending migrations before returning.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::configure(Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?)
    }

    pub fn in_memory() -> Result<Self> { Self::configure(Connection::open_in_memory()?) }

    fn configure(mut conn: Connection) -> Result<Self> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=ON;")?;
        migrations::migrate(&mut conn)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")?;
        Ok(Self { conn })
    }

    pub fn create_page(&mut self, title: &str) -> Result<Page> {
        let (title, key) = title_parts(title)?;
        let id = Uuid::new_v4();
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("INSERT INTO pages(id, title, title_key) VALUES (?1, ?2, ?3)",
            params![id.to_string(), title, key])?;
        references::resolve(&tx)?;
        tx.commit()?;
        Ok(Page { id, title, journal_date: None })
    }

    pub fn rename_page(&mut self, id: Uuid, title: &str) -> Result<()> {
        let (title, key) = title_parts(title)?;
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        changed(tx.execute("UPDATE pages SET title=?2, title_key=?3, updated_at=unixepoch() WHERE id=?1",
            params![id.to_string(), title, key])?)?;
        references::resolve(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn pages(&self) -> Result<Vec<Page>> {
        let mut stmt = self.conn.prepare("SELECT p.id,p.title,j.date FROM pages p
            LEFT JOIN journals j ON j.page_id=p.id ORDER BY p.title_key,p.id")?;
        let rows = stmt.query_map([], read_page)?;
        rows.map(|r| decode_page(r?)).collect()
    }

    pub fn delete_page(&mut self, id: Uuid) -> Result<()> {
        changed(self.conn.execute("DELETE FROM pages WHERE id=?1", [id.to_string()])?)
    }

    pub fn create_journal(&mut self, date: &str) -> Result<Page> {
        validate_journal_date(date)?;
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let page = if let Some(page) = lookup_journal(&tx, date)? {
            page
        } else {
            let id = Uuid::new_v4();
            // An ordinary page with the same title is not silently converted.
            tx.execute("INSERT INTO pages(id, title, title_key) VALUES (?1, ?2, ?2)", params![id.to_string(), date])?;
            tx.execute("INSERT INTO journals(page_id, date) VALUES (?1, ?2)", params![id.to_string(), date])?;
            Page { id, title: date.into(), journal_date: Some(date.into()) }
        };
        references::resolve(&tx)?;
        tx.commit()?;
        Ok(page)
    }

    /// Find by journal identity, not title. Never creates pages or resolves links.
    pub fn lookup_journal(&self, date: &str) -> Result<Option<Page>> {
        validate_journal_date(date)?;
        lookup_journal(&self.conn, date)
    }

    pub fn create_block(&mut self, page: Uuid, parent: Option<Uuid>, content: &str) -> Result<Block> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let position: i64 = tx.query_row(
            "SELECT coalesce(max(position)+1, 0) FROM blocks WHERE page_id=?1 AND parent_id IS ?2",
            params![page.to_string(), parent.map(|p| p.to_string())], |r| r.get(0))?;
        let id = Uuid::new_v4();
        tx.execute("INSERT INTO blocks(id,page_id,parent_id,position,content) VALUES (?1,?2,?3,?4,?5)",
            params![id.to_string(), page.to_string(), parent.map(|p| p.to_string()), position, content])?;
        references::sync(&tx, id, content, None)?;
        tx.commit()?;
        Ok(Block { id, page_id: page, parent_id: parent, position, content: content.into(), task_status: None })
    }

    /// Direct children, always in stable sibling order. Missing pages/parents yield an empty list.
    pub fn children(&self, page: Uuid, parent: Option<Uuid>) -> Result<Vec<Block>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,page_id,parent_id,position,content,task_status FROM blocks
             WHERE page_id=?1 AND parent_id IS ?2 ORDER BY position, id")?;
        let rows = stmt.query_map(params![page.to_string(), parent.map(|p| p.to_string())], read_block)?;
        rows.map(|r| decode_block(r?)).collect()
    }

    /// All blocks in stable parent/sibling order (not a flattened outline traversal).
    /// Unlike `children`, a missing page is an error.
    pub fn page_blocks(&self, page_id: Uuid) -> Result<Vec<Block>> {
        page_blocks(&self.conn, page_id)
    }

    pub fn update_block(&mut self, id: Uuid, content: &str) -> Result<()> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        changed(tx.execute("UPDATE blocks SET content=?2,updated_at=unixepoch() WHERE id=?1",
            params![id.to_string(), content])?)?;
        references::sync(&tx, id, content, None)?;
        tx.commit()?;
        Ok(())
    }

    /// Move within a page. Index is zero-based in destination siblings AFTER removal.
    /// The complete reorder is atomic; descendants keep their identities and parent links.
    pub fn move_block(&mut self, id: Uuid, parent: Option<Uuid>, index: usize) -> Result<()> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        move_block(&tx, id, parent, index)?;
        tx.commit()?;
        Ok(())
    }

    /// Delete a subtree, including its properties and outgoing references.
    pub fn delete_block(&mut self, id: Uuid) -> Result<()> {
        changed(self.conn.execute("DELETE FROM blocks WHERE id=?1", [id.to_string()])?)
    }

    pub fn set_task_status(&mut self, id: Uuid, status: Option<TaskStatus>) -> Result<()> {
        changed(self.conn.execute("UPDATE blocks SET task_status=?2,updated_at=unixepoch() WHERE id=?1",
            params![id.to_string(), status.map(TaskStatus::as_str)])?)
    }

    /// Cross-page task query, including journals and nested blocks. Each read returns
    /// at most 100 rows; offsets refer to the current, deterministic result order.
    pub fn tasks(&self, status: TaskFilter, page: Option<Uuid>, limit: usize, offset: usize) -> Result<Vec<TaskHit>> {
        let offset = i64::try_from(offset).map_err(|_| Error::Invalid("task offset is too large".into()))?;
        let (first, second) = status.statuses();
        let mut stmt = self.conn.prepare(TASKS_SQL)?;
        let rows = stmt.query_map(params![first, second, page.map(|p| p.to_string()), limit.min(100) as i64, offset],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
        rows.map(|r| {
            let (id, page_id, page_title, content, task_status) = r?;
            Ok(TaskHit { id: parse_id(&id)?, page_id: parse_id(&page_id)?, page_title, content, task_status })
        }).collect()
    }

    pub fn add_reference(&mut self, source: Uuid, target: Uuid) -> Result<()> {
        self.conn.execute("INSERT INTO page_references(source_block_id,target_page_id) VALUES (?1,?2) ON CONFLICT DO NOTHING",
            params![source.to_string(), target.to_string()])?;
        Ok(())
    }

    pub fn remove_reference(&mut self, source: Uuid, target: Uuid) -> Result<()> {
        self.conn.execute("DELETE FROM page_references WHERE source_block_id=?1 AND target_page_id=?2",
            params![source.to_string(), target.to_string()])?;
        Ok(())
    }

    pub fn backlinks(&self, target: Uuid) -> Result<Vec<Uuid>> {
        let mut stmt = self.conn.prepare("SELECT source_block_id FROM page_references WHERE target_page_id=?1 ORDER BY source_block_id")?;
        let rows = stmt.query_map([target.to_string()], |r| r.get::<_, String>(0))?;
        rows.map(|r| parse_id(&r?)).collect()
    }

    pub fn set_page_property(&mut self, id: Uuid, key: &str, value: &serde_json::Value) -> Result<()> {
        self.conn.execute("INSERT INTO page_properties(page_id,key,value_json) VALUES (?1,?2,?3)
            ON CONFLICT(page_id,key) DO UPDATE SET value_json=excluded.value_json", params![id.to_string(), key, value.to_string()])?;
        Ok(())
    }

    pub fn set_block_property(&mut self, id: Uuid, key: &str, value: &serde_json::Value) -> Result<()> {
        self.conn.execute("INSERT INTO block_properties(block_id,key,value_json) VALUES (?1,?2,?3)
            ON CONFLICT(block_id,key) DO UPDATE SET value_json=excluded.value_json", params![id.to_string(), key, value.to_string()])?;
        Ok(())
    }

    /// Literal token/phrase search, not raw FTS syntax. Results capped at 100 per kind.
    pub fn search(&self, text: &str) -> Result<Vec<SearchHit>> {
        if text.trim().is_empty() { return Ok(Vec::new()); }
        let query = format!("\"{}\"", text.replace('"', "\"\""));
        let mut hits = Vec::new();
        for (sql, kind) in [
            ("SELECT p.id,p.id,p.title FROM pages_fts JOIN pages p ON p.rowid=pages_fts.rowid WHERE pages_fts MATCH ?1 ORDER BY rank LIMIT 100", "page"),
            ("SELECT b.id,b.page_id,b.content FROM blocks_fts JOIN blocks b ON b.rowid=blocks_fts.rowid WHERE blocks_fts MATCH ?1 ORDER BY rank LIMIT 100", "block"),
        ] {
            let mut stmt = self.conn.prepare(sql)?;
            let rows = stmt.query_map([&query], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
            for row in rows {
                let (id, page_id, text) = row?;
                hits.push(SearchHit { id: parse_id(&id)?, page_id: parse_id(&page_id)?, kind: kind.into(), text });
            }
        }
        Ok(hits)
    }
}

const TASKS_SQL: &str = "SELECT b.id,b.page_id,p.title,b.content,b.task_status FROM blocks b
    JOIN pages p ON p.id=b.page_id WHERE b.task_status IN (?1,?2) AND (?3 IS NULL OR b.page_id=?3)
    ORDER BY p.title_key,p.id,b.id LIMIT ?4 OFFSET ?5";

fn validate_journal_date(date: &str) -> Result<()> {
    if date.len() != 10 { return Err(Error::Invalid("expected canonical ISO date YYYY-MM-DD".into())); }
    let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| Error::Invalid("expected valid ISO Gregorian date YYYY-MM-DD".into()))?;
    if parsed.format("%Y-%m-%d").to_string() != date {
        return Err(Error::Invalid("expected canonical ISO date".into()));
    }
    Ok(())
}

type RawPage = (String, String, Option<String>);
fn read_page(r: &rusqlite::Row<'_>) -> rusqlite::Result<RawPage> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
}
fn decode_page(raw: RawPage) -> Result<Page> {
    Ok(Page { id: parse_id(&raw.0)?, title: raw.1, journal_date: raw.2 })
}
fn lookup_journal(conn: &Connection, date: &str) -> Result<Option<Page>> {
    conn.query_row("SELECT p.id,p.title,j.date FROM journals j JOIN pages p ON p.id=j.page_id WHERE j.date=?1",
        [date], read_page).optional()?.map(decode_page).transpose()
}

fn text_revisions(conn: &Connection, page: Uuid) -> Result<std::collections::BTreeMap<Uuid, i64>> {
    let mut stmt = conn.prepare("SELECT b.id,r.revision FROM blocks b JOIN block_revisions r ON b.id=r.block_id WHERE b.page_id=?1")?;
    let rows = stmt.query_map([page.to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    rows.map(|r| { let (id, version) = r?; Ok((parse_id(&id)?, version)) }).collect()
}

// Shared transaction-compatible move primitive. Callers own the transaction.
fn move_block(tx: &Connection, id: Uuid, parent: Option<Uuid>, index: usize) -> Result<()> {
    let (page, old_parent): (String, Option<String>) = tx.query_row(
        "SELECT page_id,parent_id FROM blocks WHERE id=?1", [id.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?))).optional()?.ok_or(Error::NotFound)?;
    let parent = parent.map(|p| p.to_string());
    if let Some(ref parent_id) = parent {
        let parent_page: String = tx.query_row("SELECT page_id FROM blocks WHERE id=?1", [parent_id], |r| r.get(0))
            .optional()?.ok_or(Error::NotFound)?;
        if parent_page != page { return Err(Error::Invalid("cross-page block moves are not supported".into())); }
    }
    let id = id.to_string();
    let mut source = sibling_ids(tx, &page, old_parent.as_deref())?;
    source.retain(|b| b != &id);
    let same = parent == old_parent;
    let mut destination = if same { source.clone() } else { sibling_ids(tx, &page, parent.as_deref())? };
    if index > destination.len() { return Err(Error::Invalid("destination index out of bounds".into())); }
    destination.insert(index, id.clone());
    // Park the moved block separately, then both groups at unique negative positions.
    // Only nonnegative, compact positions are committed by this operation.
    tx.execute("UPDATE blocks SET position=?2 WHERE id=?1", params![id, i64::MIN])?;
    park(tx, &source)?;
    if !same {
        let other: Vec<_> = destination.iter().filter(|b| *b != &id).cloned().collect();
        park(tx, &other)?;
    }
    tx.execute("UPDATE blocks SET parent_id=?2,updated_at=unixepoch() WHERE id=?1", params![id, parent])?;
    if !same { order(tx, &source)?; }
    order(tx, &destination)?;
    Ok(())
}

fn require_page(conn: &Connection, page: Uuid) -> Result<()> {
    conn.query_row("SELECT 1 FROM pages WHERE id=?1", [page.to_string()], |_| Ok(()))
        .optional()?.ok_or(Error::NotFound)
}

fn page_blocks(conn: &Connection, page: Uuid) -> Result<Vec<Block>> {
    require_page(conn, page)?;
    let mut stmt = conn.prepare("SELECT id,page_id,parent_id,position,content,task_status FROM blocks
        WHERE page_id=?1 ORDER BY parent_id,position,id")?;
    let rows = stmt.query_map([page.to_string()], read_block)?;
    rows.map(|r| decode_block(r?)).collect()
}

fn title_parts(title: &str) -> Result<(String, String)> {
    let title: String = title.trim().nfc().collect();
    if title.is_empty() { return Err(Error::Invalid("page title cannot be empty".into())); }
    let key: String = title.to_lowercase().nfc().collect();
    Ok((title, key))
}
fn parse_id(id: &str) -> Result<Uuid> {
    Uuid::parse_str(id).map_err(|_| Error::Invalid("invalid stored UUID".into()))
}
fn changed(count: usize) -> Result<()> { if count == 0 { Err(Error::NotFound) } else { Ok(()) } }
type RawBlock = (String, String, Option<String>, i64, String, Option<String>);
fn read_block(r: &rusqlite::Row<'_>) -> rusqlite::Result<RawBlock> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
}
fn decode_block(raw: RawBlock) -> Result<Block> {
    Ok(Block { id: parse_id(&raw.0)?, page_id: parse_id(&raw.1)?, parent_id: raw.2.as_deref().map(parse_id).transpose()?,
        position: raw.3, content: raw.4, task_status: raw.5 })
}
fn sibling_ids(conn: &Connection, page: &str, parent: Option<&str>) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM blocks WHERE page_id=?1 AND parent_id IS ?2 ORDER BY position")?;
    let rows = stmt.query_map(params![page, parent], |r| r.get(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}
fn park(conn: &Connection, ids: &[String]) -> Result<()> {
    for (i, id) in ids.iter().enumerate() {
        conn.execute("UPDATE blocks SET position=?2 WHERE id=?1", params![id, -(i as i64)-1])?;
    }
    Ok(())
}
fn order(conn: &Connection, ids: &[String]) -> Result<()> {
    for (i, id) in ids.iter().enumerate() {
        conn.execute("UPDATE blocks SET position=?2 WHERE id=?1", params![id, i as i64])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
