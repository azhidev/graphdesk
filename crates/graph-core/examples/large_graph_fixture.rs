//! Developer-only fixture: cargo run -p graph-core --example large_graph_fixture --release --locked
//! Writes only to the repository's ignored test-artifacts/benchmark-large-graphs directory.
use graph_core::Graph;
use rusqlite::{Connection, OpenFlags};
use std::{collections::HashMap, error::Error, fs, path::Path};
use uuid::Uuid;

const BLOCKS_PER_PAGE: usize = 200;
const SIZES: [usize; 2] = [1_000, 5_000];

fn text(page: usize, index: usize) -> String {
    let label = match (page * BLOCKS_PER_PAGE + index) % 6 {
        0 => "برنامه‌ریزی جلسه، مرحله ۲: بررسی نتیجه.",
        1 => "Project update: review the draft, then follow up at 10:30.",
        2 => "یادداشت روزانه: کارهای انجام‌شده و گام بعدی، ۱۴۰۵/۰۸/۰۶.",
        3 => "Release checklist: test, document, and verify 3 open items.",
        4 => "همکاری تیم: متن فارسی و English در یک خط؛ نسخه ۲.۵.",
        _ => "Meeting notes: 2 decisions, 1 follow-up; آماده‌سازی گزارش.",
    };
    let prefix = format!("P{:02} · {:03} — {label}", page + 1, index + 1);
    if index % 5 == 0 {
        format!("{prefix} Context: compare the previous plan with current observations; هماهنگی با تیم، بازبینی جزئیات و ثبت نتیجه برای جلسه بعدی. Next action: review item {}.", index + 1)
    } else {
        prefix
    }
}

// Repeat a small outline with depths 0..4. All parents are created before children;
// Graph::create_block allocates the next sibling position under that parent.
fn depth(index: usize) -> usize {
    const SHAPE: [usize; 20] = [0, 1, 2, 3, 4, 3, 2, 1, 1, 2, 3, 2, 1, 0, 1, 2, 3, 4, 2, 1];
    SHAPE[index % SHAPE.len()]
}

fn generate(path: &Path, count: usize) -> Result<(), Box<dyn Error>> {
    assert!(SIZES.contains(&count));
    let mut graph = Graph::create(path)?; // create_new refuses an existing file
    for page_index in 0..count / BLOCKS_PER_PAGE {
        let page = graph.create_page(&format!("Benchmark {:02} — برنامه‌ریزی / Planning", page_index + 1))?;
        let mut ancestors: [Option<Uuid>; 5] = [None; 5];
        for index in 0..BLOCKS_PER_PAGE {
            let level = depth(index);
            let parent = if level == 0 { None } else { ancestors[level - 1] };
            let block = graph.create_block(page.id, parent, &text(page_index, index))?;
            ancestors[level] = Some(block.id);
        }
    }
    drop(graph); // last writer closes, checkpointing WAL before verification/transfer
    verify(path, count)
}

fn verify(path: &Path, count: usize) -> Result<(), Box<dyn Error>> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" || conn.prepare("PRAGMA foreign_key_check")?.exists([])? {
        return Err(format!("integrity or foreign keys failed: {integrity}").into());
    }
    let (pages, blocks): (i64, i64) = conn.query_row(
        "SELECT (SELECT count(*) FROM pages), (SELECT count(*) FROM blocks)", [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if (pages, blocks) != ((count / BLOCKS_PER_PAGE) as i64, count as i64) {
        return Err(format!("unexpected page/block counts: {pages}/{blocks}").into());
    }
    drop(conn);

    // Exercise the same core open and outline reads used by the desktop adapter.
    let graph = Graph::open(path)?;
    let listed = graph.pages()?;
    if listed.len() != count / BLOCKS_PER_PAGE {
        return Err("core page listing mismatch".into());
    }
    for page in &listed {
        let outline = graph.page_blocks(page.id)?;
        if outline.len() != BLOCKS_PER_PAGE { return Err("core outline count mismatch".into()); }
        let mut parents = HashMap::new();
        let mut siblings: HashMap<Option<Uuid>, i64> = HashMap::new();
        for block in outline {
            if block.position < 0 { return Err("invalid sibling position".into()); }
            let next = siblings.entry(block.parent_id).or_insert(0);
            if block.position != *next { return Err("sibling position gap".into()); }
            *next += 1;
            parents.insert(block.id, block.parent_id);
        }
        let mut saw_depth_four = false;
        for &id in parents.keys() {
            let mut current = Some(id);
            let mut level = 0;
            while let Some(block) = current {
                current = *parents.get(&block).ok_or("missing parent in outline")?;
                if current.is_some() { level += 1; }
                if level > 4 { return Err("invalid outline depth".into()); }
            }
            saw_depth_four |= level == 4;
        }
        if !saw_depth_four { return Err("missing nested hierarchy".into()); }
    }
    drop(graph);
    for extension in ["-wal", "-shm"] {
        if path.with_file_name(format!("{}{}", path.file_name().unwrap().to_string_lossy(), extension)).exists() {
            return Err(format!("live SQLite sidecar remains: {extension}").into());
        }
    }
    println!("{}: {pages} pages, {blocks} blocks; integrity ok, foreign keys ok, core reopen ok", path.display());
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let artifacts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-artifacts");
    let directory = artifacts.join("benchmark-large-graphs");
    for component in [&artifacts, &directory] {
        if component.is_symlink() { return Err(format!("refusing symlinked fixture directory: {}", component.display()).into()); }
    }
    fs::create_dir_all(&directory)?;
    for count in SIZES {
        let path = directory.join(format!("graphdesk-{count}-blocks.sqlite3"));
        if path.exists() { return Err(format!("refusing existing fixture: {}", path.display()).into()); }
        generate(&path, count)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeatable_distribution_and_safe_creation() -> Result<(), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("fixture.sqlite3");
        assert_eq!(depth(4), 4);
        assert_eq!(depth(20), 0);
        assert!(text(0, 0).contains('‌'));
        assert!(text(0, 1).contains("10:30"));
        generate(&path, 1_000)?;
        assert!(generate(&path, 1_000).is_err());
        verify(&path, 1_000)?;
        Ok(())
    }
}
