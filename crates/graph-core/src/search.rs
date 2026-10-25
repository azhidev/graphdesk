use rusqlite::{functions::FunctionFlags, Connection};
use unicode_normalization::UnicodeNormalization;
use crate::{Graph, Result, SearchHit};

/// Search-only normalization. ZWNJ is a token boundary, never word joining.
pub fn normalize_persian(text: &str) -> String {
    text.chars().map(|c| match c {
        'ي' => 'ی', 'ك' => 'ک', '\u{200c}' => ' ',
        '۰'..='۹' => char::from_u32('0' as u32 + c as u32 - '۰' as u32).unwrap(),
        '٠'..='٩' => char::from_u32('0' as u32 + c as u32 - '٠' as u32).unwrap(),
        other => other,
    }).collect::<String>().nfc().collect()
}

pub(crate) fn register(conn: &Connection) -> Result<()> {
    conn.create_scalar_function("persian_normalize", 1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC | FunctionFlags::SQLITE_INNOCUOUS,
        |ctx| Ok(normalize_persian(&ctx.get::<String>(0)?)))?;
    Ok(())
}

impl Graph {
    pub fn search_mode(&self, text: &str, persian: bool) -> Result<Vec<SearchHit>> {
        if !persian { return self.search(text); }
        let normalized = normalize_persian(text);
        if normalized.trim().is_empty() { return Ok(vec![]); }
        let query = format!("\"{}\"", normalized.replace('"', "\"\""));
        let mut hits = Vec::new();
        for (sql, kind) in [
            ("SELECT p.id,p.id,p.title FROM pages_persian_fts JOIN pages p ON p.rowid=pages_persian_fts.rowid WHERE pages_persian_fts MATCH ?1 ORDER BY rank LIMIT 100", "page"),
            ("SELECT b.id,b.page_id,b.content FROM blocks_persian_fts JOIN blocks b ON b.rowid=blocks_persian_fts.rowid WHERE blocks_persian_fts MATCH ?1 ORDER BY rank LIMIT 100", "block"),
        ] {
            let mut stmt = self.conn.prepare(sql)?;
            let rows = stmt.query_map([&query], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
            for row in rows {
                let (id, page_id, text) = row?;
                hits.push(SearchHit { id: crate::parse_id(&id)?, page_id: crate::parse_id(&page_id)?, kind: kind.into(), text });
            }
        }
        Ok(hits)
    }

    /// Rebuild from canonical data, not FTS5's retained normalized content.
    pub fn rebuild_persian_search(&mut self) -> Result<()> {
        let tx = self.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        tx.execute_batch("DELETE FROM pages_persian_fts; DELETE FROM blocks_persian_fts;
            INSERT INTO pages_persian_fts(rowid,title) SELECT rowid,persian_normalize(title) FROM pages;
            INSERT INTO blocks_persian_fts(rowid,content) SELECT rowid,persian_normalize(content) FROM blocks;")?;
        tx.commit()?;
        Ok(())
    }
}
