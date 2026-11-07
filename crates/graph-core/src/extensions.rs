//! Graph-local extension preferences and bounded, compare-and-set JSON storage.
//! This generic store does not grant IPC access; the adapter owns its allowlist.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use serde_json::Value;

use crate::{Error, Graph, Result};

const MAX_DOCUMENT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExtensionState { pub id: String, pub enabled: bool }

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExtensionDocument { pub revision: i64, pub value: Value }

impl Graph {
    /// Explicitly configured IDs only. Adapters supply defaults for their registry.
    pub fn extension_states(&self) -> Result<Vec<ExtensionState>> {
        let mut stmt = self.conn.prepare("SELECT extension_id,enabled FROM extensions
            WHERE graph_id=(SELECT id FROM graph_identity WHERE singleton=1) ORDER BY extension_id")?;
        let rows = stmt.query_map([], |r| Ok(ExtensionState { id: r.get(0)?, enabled: r.get(1)? }))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Atomic durable preference change; never resets the document or revision.
    pub fn extension_configure(&mut self, id: &str, enabled: bool) -> Result<()> {
        validate_id(id)?;
        crate::changed(self.conn.execute("INSERT INTO extensions(graph_id,extension_id,enabled)
            SELECT id,?1,?2 FROM graph_identity WHERE singleton=1
            ON CONFLICT(graph_id,extension_id) DO UPDATE SET enabled=excluded.enabled", params![id, enabled])?)
    }

    /// An absent record is disabled. Reads never create or enable an extension.
    pub fn extension_read(&self, id: &str) -> Result<ExtensionDocument> {
        validate_id(id)?;
        let tx = self.conn.unchecked_transaction()?;
        let document = read_enabled(&tx, id)?;
        tx.commit()?;
        Ok(document)
    }

    /// Rechecks enablement and revision under the same SQLite writer lock.
    /// The quota counts the compact UTF-8 JSON encoding, including JSON syntax.
    pub fn extension_write(&mut self, id: &str, expected_revision: i64, value: Value) -> Result<ExtensionDocument> {
        validate_id(id)?;
        if expected_revision < 0 { return Err(Error::Invalid("extension revision cannot be negative".into())); }
        let json = serde_json::to_string(&value).map_err(|_| Error::Invalid("invalid extension JSON".into()))?;
        if json.len() > MAX_DOCUMENT_BYTES { return Err(Error::Invalid("extension document exceeds 16 KiB".into())); }
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = read_enabled(&tx, id)?;
        if current.revision != expected_revision {
            return Err(Error::Conflict("extension document changed; read before retrying".into()));
        }
        let revision = current.revision.checked_add(1)
            .ok_or_else(|| Error::Conflict("extension revision exhausted".into()))?;
        let count = tx.execute("UPDATE extensions SET revision=?3,value_json=?4
            WHERE graph_id=(SELECT id FROM graph_identity WHERE singleton=1)
                AND extension_id=?1 AND enabled=1 AND revision=?2", params![id, expected_revision, revision, json])?;
        if count != 1 { return Err(Error::Conflict("extension document changed".into())); }
        tx.commit()?;
        Ok(ExtensionDocument { revision, value })
    }
}

// Lowercase ASCII reverse-DNS style: 3..100 bytes, at least two nonempty labels,
// alphanumeric label boundaries and only alphanumerics/hyphens inside labels.
fn validate_id(id: &str) -> Result<()> {
    let alphanumeric = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    if !(3..=100).contains(&id.len()) || !id.contains('.') || !id.split('.').all(|label| {
        let bytes = label.as_bytes();
        !bytes.is_empty() && alphanumeric(bytes[0]) && alphanumeric(bytes[bytes.len() - 1])
            && bytes.iter().all(|b| alphanumeric(*b) || *b == b'-')
    }) {
        return Err(Error::Invalid("extension ID must be lowercase ASCII reverse DNS, 3..100 bytes".into()));
    }
    Ok(())
}

fn read_enabled(conn: &Connection, id: &str) -> Result<ExtensionDocument> {
    let row: Option<(bool, i64, String)> = conn.query_row(
        "SELECT enabled,revision,value_json FROM extensions
         WHERE graph_id=(SELECT id FROM graph_identity WHERE singleton=1) AND extension_id=?1",
        [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
    let (enabled, revision, json) = row.ok_or_else(|| Error::Invalid("extension is disabled".into()))?;
    if !enabled { return Err(Error::Invalid("extension is disabled".into())); }
    let value = serde_json::from_str(&json).map_err(|_| Error::Invalid("invalid stored extension JSON".into()))?;
    Ok(ExtensionDocument { revision, value })
}

#[cfg(test)]
mod tests;
