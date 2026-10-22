//! Plain [[title]] grammar: no nesting, aliases, escaping or multiline titles.
//! SQLite is authoritative; bindings are written in the same content transaction.
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use uuid::Uuid;
use crate::{parse_id, title_parts, Block, Result};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InlineReference {
    pub source_block_id: Uuid,
    pub title_key: String,
    pub label: String,
    pub target_page_id: Option<Uuid>,
    pub target_title: Option<String>,
}

pub(crate) fn labels(text: &str) -> Vec<(String, String)> {
    let mut result = std::collections::BTreeMap::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("]]") else { break; };
        let label = &rest[..end];
        if !label.contains(['[', ']', '\n', '\r']) {
            if let Ok((label, key)) = title_parts(label) { result.entry(key).or_insert(label); }
        }
        rest = &rest[end + 2..];
    }
    result.into_iter().collect()
}

pub(crate) fn bindings(conn: &Connection, id: Uuid) -> Result<Vec<(String, String, Option<String>)>> {
    let mut stmt = conn.prepare("SELECT title_key,label,target_page_id FROM inline_references WHERE source_block_id=?1 ORDER BY title_key")?;
    let rows = stmt.query_map([id.to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub(crate) fn sync(conn: &Connection, id: Uuid, text: &str, saved: Option<&[(String, String, Option<String>)]>) -> Result<()> {
    let previous = if let Some(saved) = saved { saved.to_vec() } else { bindings(conn, id)? };
    conn.execute("DELETE FROM inline_references WHERE source_block_id=?1", [id.to_string()])?;
    for (key, label) in labels(text) {
        let retained = previous.iter().find(|r| r.0 == key).and_then(|r| r.2.clone());
        let target: Option<String> = if let Some(target) = retained {
            conn.query_row("SELECT id FROM pages WHERE id=?1", [target], |r| r.get(0)).optional()?
        } else { None };
        let target = match target {
            Some(target) => Some(target),
            None => conn.query_row("SELECT id FROM pages WHERE title_key=?1", [&key], |r| r.get(0)).optional()?,
        };
        conn.execute("INSERT INTO inline_references(source_block_id,title_key,label,target_page_id) VALUES (?1,?2,?3,?4)", params![id.to_string(),key,label,target])?;
    }
    Ok(())
}

pub(crate) fn resolve(conn: &Connection) -> Result<()> {
    conn.execute("UPDATE inline_references SET target_page_id=(SELECT id FROM pages WHERE pages.title_key=inline_references.title_key) WHERE target_page_id IS NULL", [])?;
    Ok(())
}

pub(crate) fn backfill(conn: &Connection) -> Result<()> {
    let blocks = {
        let mut stmt = conn.prepare("SELECT id,content FROM blocks")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for (id, text) in blocks { sync(conn, parse_id(&id)?, &text, None)?; }
    Ok(())
}

impl crate::Graph {
    pub fn inline_references(&self, page: Uuid) -> Result<Vec<InlineReference>> {
        crate::require_page(&self.conn, page)?;
        let mut stmt = self.conn.prepare("SELECT r.source_block_id,r.title_key,r.label,r.target_page_id,p.title
            FROM inline_references r JOIN blocks b ON b.id=r.source_block_id LEFT JOIN pages p ON p.id=r.target_page_id
            WHERE b.page_id=?1 ORDER BY r.source_block_id,r.title_key")?;
        let rows = stmt.query_map([page.to_string()], |r| Ok((r.get::<_, String>(0)?,r.get(1)?,r.get(2)?,r.get::<_, Option<String>>(3)?,r.get(4)?)))?;
        rows.map(|r| { let (id,key,label,target,title) = r?; Ok(InlineReference { source_block_id: parse_id(&id)?, title_key:key,label,target_page_id:target.map(|t| parse_id(&t)).transpose()?,target_title:title }) }).collect()
    }

    pub fn incoming_links(&self, page: Uuid) -> Result<Vec<Block>> {
        crate::require_page(&self.conn, page)?;
        let mut stmt = self.conn.prepare("SELECT b.id,b.page_id,b.parent_id,b.position,b.content,b.task_status FROM blocks b
            WHERE b.id IN (SELECT source_block_id FROM page_references WHERE target_page_id=?1
            UNION SELECT source_block_id FROM inline_references WHERE target_page_id=?1) ORDER BY b.page_id,b.position,b.id")?;
        let rows = stmt.query_map([page.to_string()], crate::read_block)?;
        rows.map(|r| crate::decode_block(r?)).collect()
    }
}
