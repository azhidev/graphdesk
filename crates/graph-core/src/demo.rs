//! First-run demonstration content for a brand new installation. This uses only
//! the public `Graph` APIs and the existing database invariants, so the seeded
//! graph is an ordinary, fully editable local graph. It is never applied to an
//! existing graph: callers must create a brand new empty database first.
//!
//! Content is deliberately small (a few pages, a handful of blocks) and mixes
//! English and Persian so first-run defaults (English/LTR, Gregorian) still
//! show how bidi text and tasks behave.

use crate::{Graph, Result, TaskStatus};

/// Seed a small, clearly-labelled demonstration dataset. `date` is the caller's
/// local civil day (YYYY-MM-DD); it becomes the example journal identity.
///
/// Every write goes through `Graph`'s transaction-respecting methods, including
/// reference resolution for `[[page links]]` and task status updates.
pub fn seed_demo(graph: &mut Graph, date: &str) -> Result<()> {
    crate::validate_journal_date(date)?;
    if !graph.pages()?.is_empty() {
        return Err(crate::Error::Invalid("sample content requires a new empty graph".into()));
    }
    let welcome = graph.create_page("Welcome")?;
    let started = graph.create_page("Getting Started")?;
    let tasks = graph.create_page("Tasks")?;

    // Welcome: a short mixed-script intro with one nested branch.
    let intro = graph.create_block(
        welcome.id,
        None,
        "Welcome to GraphDesk — this is a sample graph. Everything here is yours to edit or delete.",
    )?;
    graph.create_block(
        welcome.id,
        Some(intro.id),
        "سلام! این یک گراف نمونه است؛ همه‌چیز را آزادانه ویرایش یا حذف کنید.",
    )?;
    graph.create_block(
        welcome.id,
        Some(intro.id),
        "Blocks nest: press Tab to indent, Shift+Tab to outdent, Enter for a new sibling.",
    )?;
    graph.create_block(
        welcome.id,
        None,
        "Start with [[Getting Started]], or try the examples in [[Tasks]].",
    )?;

    // Getting Started: a few plain blocks plus a nested child.
    let first_tip = graph.create_block(
        started.id,
        None,
        "Create a page from the sidebar, then type to add blocks. Changes autosave.",
    )?;
    graph.create_block(
        started.id,
        Some(first_tip.id),
        "متن با کیبورد، چسباندن و ترکیب حروف (IME) نوشته می‌شود.",
    )?;
    graph.create_block(
        started.id,
        None,
        "Wrap a name in [[double brackets]] to link pages; backlinks appear below the page.",
    )?;
    graph.create_block(
        started.id,
        None,
        "Ctrl/Cmd+K searches, Ctrl/Cmd+P jumps to a page, Ctrl/Cmd+O opens a graph.",
    )?;

    // Tasks: one example per status so the Tasks view is populated immediately.
    graph.create_block(tasks.id, None, "Sample tasks — the status shows in the Tasks view:")?;
    let todo = graph.create_block(tasks.id, None, "Try the graph switcher with Ctrl+Tab")?;
    graph.set_task_status(todo.id, Some(TaskStatus::Todo))?;
    let doing = graph.create_block(tasks.id, None, "ویرایش متن نمونه به فارسی")?;
    graph.set_task_status(doing.id, Some(TaskStatus::Doing))?;
    let done = graph.create_block(tasks.id, None, "Read the Welcome page")?;
    graph.set_task_status(done.id, Some(TaskStatus::Done))?;

    // Today's journal so the first launch lands on populated sample content.
    let journal = graph.create_journal(date)?;
    graph.create_block(
        journal.id,
        None,
        "GraphDesk demo day — this journal entry is sample content you can replace.",
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use uuid::Uuid;

    #[test]
    fn demo_seed_is_small_editable_and_uses_core_apis() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("demo.sqlite3");
        let mut graph = Graph::create(&path).unwrap();
        seed_demo(&mut graph, "2024-02-29").unwrap();

        let pages = graph.pages().unwrap();
        let titles: BTreeSet<&str> = pages.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(titles, BTreeSet::from(["Tasks", "Welcome", "Getting Started", "2024-02-29"]));

        let welcome = pages.iter().find(|p| p.title == "Welcome").unwrap();
        let root = graph.children(welcome.id, None).unwrap();
        assert_eq!(root.len(), 2);
        let intro = root.iter().find(|b| b.content.starts_with("Welcome to GraphDesk")).unwrap();
        let nested = graph.children(welcome.id, Some(intro.id)).unwrap();
        assert_eq!(nested.len(), 2);
        assert!(nested.iter().any(|b| b.content.contains("سلام")));

        // `[[Links]]` resolve to existing pages using the regular inline-reference path.
        let started = pages.iter().find(|p| p.title == "Getting Started").unwrap();
        let backlinks = graph.incoming_links(started.id).unwrap();
        assert_eq!(backlinks.len(), 1);
        assert!(backlinks[0].content.contains("[[Getting Started]]"));

        // Task statuses are real task rows, not just text.
        let tasks = pages.iter().find(|p| p.title == "Tasks").unwrap();
        assert_eq!(graph.tasks(crate::TaskFilter::Active, Some(tasks.id), 100, 0).unwrap().len(), 2);
        assert_eq!(graph.tasks(crate::TaskFilter::Done, Some(tasks.id), 100, 0).unwrap().len(), 1);

        // The example journal exists with a sample block.
        let journal = pages.iter().find(|p| p.journal_date.as_deref() == Some("2024-02-29")).unwrap();
        assert_eq!(graph.page_blocks(journal.id).unwrap().len(), 1);

        // Mixed scripts survive exact round-tripping through SQLite.
        drop(graph);
        let reopened = Graph::open(&path).unwrap();
        let outline = reopened.page_blocks(welcome.id).unwrap();
        assert!(outline.iter().any(|b| b.content.contains("سلام")));
        assert!(outline.iter().any(|b| b.content.contains("GraphDesk")));
    }

    #[test]
    fn demo_seed_never_overwrites_existing_pages() {
        // Re-seeding is rejected before writes instead of duplicating or
        // overwriting the original content.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("demo.sqlite3");
        let mut graph = Graph::create(&path).unwrap();
        seed_demo(&mut graph, "2024-02-29").unwrap();
        let before = graph.pages().unwrap().len();
        assert!(seed_demo(&mut graph, "2024-03-01").is_err());
        assert_eq!(graph.pages().unwrap().len(), before);
        let _: Uuid = graph.id().unwrap();
    }

    #[test]
    fn demo_rejects_invalid_dates_and_user_content_before_writing() {
        let mut graph = Graph::in_memory().unwrap();
        assert!(seed_demo(&mut graph, "2024-02-30").is_err());
        assert!(graph.pages().unwrap().is_empty());
        graph.create_page("My existing page").unwrap();
        let before = graph.pages().unwrap();
        assert!(seed_demo(&mut graph, "2024-02-29").is_err());
        assert_eq!(graph.pages().unwrap(), before);
    }
}
