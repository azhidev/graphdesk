//! Candidate IO never replaces the active graph. Lock order is pending -> graph
//! -> recovery; legacy opens only take graph -> recovery. Dialogs take no locks.
use std::{path::{Path, PathBuf}, sync::{Arc, Mutex}};
use graph_core::{Graph, Page};
use serde::Serialize;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
use crate::{ActiveGraph, AppState, editor::EditorError, recovery::RecoveryState};

struct Candidate { ticket: Uuid, source: Option<Uuid>, active: ActiveGraph }
#[derive(Clone, Default)]
pub(crate) struct PendingState(Arc<Mutex<Option<Candidate>>>);

#[derive(Serialize)]
pub(crate) struct Prepared { ticket: Uuid, path: String, graph_id: Uuid }
#[derive(Serialize)]
pub(crate) struct GraphSession { token: Uuid, pages: Vec<Page>, graph_id: Uuid, path: String }

fn internal() -> EditorError { EditorError::new("internal", "graph switch state unavailable") }
fn stale() -> EditorError { EditorError::new("stale_session", "graph session changed; prepare again") }
fn source_matches(active: &Option<ActiveGraph>, source: Option<Uuid>) -> Result<(), EditorError> {
    if active.as_ref().map(|g| g.token) != source { return Err(stale()); }
    Ok(())
}

/// Resolve before opening. Creation still uses Graph::create's exclusive reserve;
/// canonicalizing a parent must never turn Create into an overwrite.
pub(crate) fn resolve_path(path: &str, create: bool) -> graph_core::Result<PathBuf> {
    if path.is_empty() { return Err(graph_core::Error::Invalid("graph path is empty".into())); }
    let path = Path::new(path);
    if !create { return Ok(path.canonicalize()?); }
    let filename = path.file_name().ok_or_else(|| graph_core::Error::Invalid("graph filename required".into()))?;
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    Ok(parent.canonicalize()?.join(filename))
}
fn native_path(active: &ActiveGraph) -> Result<String, EditorError> {
    active.path.as_ref().and_then(|p| p.to_str()).map(str::to_owned)
        .ok_or_else(|| EditorError::new("invalid", "graph path is not representable as Unicode"))
}
fn session(active: &ActiveGraph) -> Result<GraphSession, EditorError> {
    Ok(GraphSession { token: active.token, pages: active.editor.graph().pages()?, graph_id: active.editor.graph().id()?, path: native_path(active)? })
}

fn prepare(state: &AppState, pending: &PendingState, path: String, create: bool,
    source: Option<Uuid>, expected: Option<Uuid>, automatic: bool) -> Result<Prepared, EditorError> {
    let mut candidate = pending.0.lock().map_err(|_| internal())?;
    source_matches(&*state.0.lock().map_err(|_| internal())?, source)?;
    if automatic && (create || expected.is_none()) {
        return Err(EditorError::new("invalid", "automatic opening requires a remembered graph identity"));
    }
    if create && expected.is_some() {
        return Err(EditorError::new("invalid", "creation cannot use a remembered identity"));
    }
    let path = resolve_path(&path, create)?;
    // Reject unrepresentable canonical parents before creating a file.
    let display = path.to_str().ok_or_else(|| EditorError::new("invalid", "graph path is not representable as Unicode"))?.to_owned();
    if let Some(expected) = expected {
        // Critically this is READONLY and occurs BEFORE Graph::open/migrations.
        if Graph::inspect_identity(&path, automatic)? != expected {
            return Err(EditorError::new("conflict", "remembered path now contains a different graph"));
        }
    }
    // At most one candidate connection, even while preparing a replacement.
    *candidate = None;
    let graph = if create { Graph::create(&path) }
        else if let Some(expected) = expected { Graph::open_expected(&path, expected, automatic) }
        else { Graph::open(&path) }?;
    let graph_id = graph.id()?;
    if expected.is_some_and(|id| id != graph_id) {
        return Err(EditorError::new("conflict", "graph identity changed while opening"));
    }
    graph.pages()?;
    source_matches(&*state.0.lock().map_err(|_| internal())?, source)?;
    let ticket = Uuid::new_v4();
    *candidate = Some(Candidate { ticket, source, active: ActiveGraph::at_path(graph, path) });
    Ok(Prepared { ticket, path: display, graph_id })
}

fn activate(state: &AppState, pending: &PendingState, recovery: &RecoveryState,
    ticket: Uuid, source: Option<Uuid>) -> Result<GraphSession, EditorError> {
    let mut prepared = pending.0.lock().map_err(|_| internal())?;
    let candidate = prepared.as_ref().filter(|p| p.ticket == ticket)
        .ok_or_else(|| EditorError::new("stale_ticket", "graph preparation expired; prepare again"))?;
    if candidate.source != source { return Err(stale()); }
    let mut graph = state.0.lock().map_err(|_| internal())?;
    source_matches(&graph, source)?;
    // All fallible reads/locks happen before replacing either active session.
    let response = session(&candidate.active)?;
    let mut access = recovery.0.lock().map_err(|_| internal())?;
    let mut candidate = prepared.take().expect("checked candidate");
    candidate.active.activation_ticket = Some(ticket);
    access.session = Some((candidate.active.token, response.graph_id));
    *graph = Some(candidate.active);
    Ok(response)
}

#[tauri::command]
pub(crate) async fn graph_prepare(path: String, create: bool, source_session: Option<Uuid>, expected_graph: Option<Uuid>, automatic: bool,
    state: State<'_, AppState>, pending: State<'_, PendingState>) -> Result<Prepared, EditorError> {
    let state = state.inner().clone();
    let pending = pending.inner().clone();
    tauri::async_runtime::spawn_blocking(move || prepare(&state, &pending, path, create, source_session, expected_graph, automatic))
        .await.map_err(|_| internal())?
}

#[tauri::command]
pub(crate) async fn graph_activate(ticket: Uuid, source_session: Option<Uuid>, state: State<'_, AppState>,
    pending: State<'_, PendingState>, recovery: State<'_, RecoveryState>) -> Result<GraphSession, EditorError> {
    let state = state.inner().clone();
    let pending = pending.inner().clone();
    let recovery = recovery.inner().clone();
    tauri::async_runtime::spawn_blocking(move || activate(&state, &pending, &recovery, ticket, source_session)).await.map_err(|_| internal())?
}

#[tauri::command]
pub(crate) async fn graph_cancel(ticket: Uuid, pending: State<'_, PendingState>) -> Result<(), EditorError> {
    let pending = pending.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = pending.0.lock().map_err(|_| internal())?;
        if guard.as_ref().is_some_and(|p| p.ticket == ticket) { *guard = None; }
        Ok(())
    }).await.map_err(|_| internal())?
}

#[tauri::command]
pub(crate) async fn graph_reconcile(ticket: Uuid, state: State<'_, AppState>) -> Result<Option<GraphSession>, EditorError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let guard = state.0.lock().map_err(|_| internal())?;
        guard.as_ref().filter(|g| g.activation_ticket == Some(ticket)).map(session).transpose()
    }).await.map_err(|_| internal())?
}

#[tauri::command]
pub(crate) async fn graph_pick<R: tauri::Runtime>(app: tauri::AppHandle<R>, create: bool,
    language: Option<crate::preferences::Language>) -> Result<Option<String>, EditorError> {
    let parent = app.get_webview_window("main").ok_or_else(internal)?;
    let persian = language == Some(crate::preferences::Language::Fa);
    let title = match (create, persian) { (true, true) => "ساخت گراف", (false, true) => "باز کردن گراف", (true, false) => "Create graph", (false, false) => "Open graph" };
    let dialog = app.dialog().file().set_parent(&parent).set_title(title)
        .add_filter(if persian { "پایگاه داده" } else { "SQLite database" }, &["sqlite3", "sqlite", "db", "db3"])
        .add_filter(if persian { "همهٔ فایل‌ها" } else { "All files" }, &["*"]);
    // Plugin schedules native UI on the main thread. Only the worker waits; no
    // graph, recovery, registry or pending mutex is held while a dialog is open.
    tauri::async_runtime::spawn_blocking(move || {
        let picked = if create { dialog.set_file_name("graph.sqlite3").blocking_save_file() } else { dialog.blocking_pick_file() };
        picked.map(|p| {
            let path = p.into_path().map_err(|_| EditorError::new("invalid", "native dialog did not return a filesystem path"))?;
            path.into_os_string().into_string().map_err(|_| EditorError::new("invalid", "selected path is not representable as Unicode"))
        }).transpose()
    }).await.map_err(|_| internal())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_activate_cancel_and_legacy_source_guards() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::default();
        let pending = PendingState::default();
        let recovery = RecoveryState::default();
        let first_path = dir.path().join("گراف اول 😀.db");
        let first = prepare(&state, &pending, first_path.to_str().unwrap().into(), true, None, None, false).unwrap();
        assert!(state.0.lock().unwrap().is_none());
        assert!(recovery.0.lock().unwrap().session.is_none());
        let first = activate(&state, &pending, &recovery, first.ticket, None).unwrap();
        assert_eq!(first.path, first_path.canonicalize().unwrap().to_str().unwrap());
        assert_eq!(recovery.0.lock().unwrap().session, Some((first.token, first.graph_id)));
        let old_page = state.0.lock().unwrap().as_mut().unwrap().editor.create_page("retained").unwrap();
        let candidate_path = dir.path().join("candidate.db");
        let candidate = prepare(&state, &pending, candidate_path.to_str().unwrap().into(), true, Some(first.token), None, false).unwrap();
        assert_eq!(state.0.lock().unwrap().as_ref().unwrap().token, first.token);
        assert_eq!(recovery.0.lock().unwrap().session, Some((first.token, first.graph_id)));
        assert!(activate(&state, &pending, &recovery, Uuid::new_v4(), Some(first.token)).is_err());
        assert!(activate(&state, &pending, &recovery, candidate.ticket, None).is_err());
        // A legacy open has no pending-state dependency but always changes token.
        *state.0.lock().unwrap() = Some(ActiveGraph::new(Graph::in_memory().unwrap()));
        assert!(activate(&state, &pending, &recovery, candidate.ticket, Some(first.token)).is_err());
        assert!(prepare(&state, &pending, first.path.clone(), false, Some(first.token), None, false).is_err());
        // Failed preparations do not alter the active editor or recovery session.
        let token = state.0.lock().unwrap().as_ref().unwrap().token;
        assert!(prepare(&state, &pending, dir.path().join("missing.db").to_str().unwrap().into(), false, Some(token), None, false).is_err());
        assert_eq!(state.0.lock().unwrap().as_ref().unwrap().token, token);
        assert_eq!(Graph::open(&first.path).unwrap().pages().unwrap()[0], old_page);
        // Replacing a preparation invalidates the older ticket.
        let newer = prepare(&state, &pending, first.path, false, Some(token), Some(first.graph_id), true).unwrap();
        assert!(activate(&state, &pending, &recovery, candidate.ticket, Some(token)).is_err());
        let next = activate(&state, &pending, &recovery, newer.ticket, Some(token)).unwrap();
        assert_eq!(next.pages, vec![old_page]);
        assert!(pending.0.lock().unwrap().is_none());
        assert!(activate(&state, &pending, &recovery, newer.ticket, Some(token)).is_err());
    }

    #[test]
    fn identity_errors_automatic_rules_and_exclusive_creation_preserve_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph.db");
        let state = AppState::default();
        let pending = PendingState::default();
        let recovery = RecoveryState::default();
        let first = prepare(&state, &pending, path.to_str().unwrap().into(), true, None, None, false).unwrap();
        let active = activate(&state, &pending, &recovery, first.ticket, None).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        for (create, expected, automatic) in [(true,None,false),(false,None,true),
            (false,Some(Uuid::new_v4()),false),(false,Some(Uuid::new_v4()),true),
            (true,Some(active.graph_id),false),(true,Some(active.graph_id),true)] {
            assert!(prepare(&state, &pending, path.to_str().unwrap().into(), create, Some(active.token), expected, automatic).is_err());
            assert_eq!(state.0.lock().unwrap().as_ref().unwrap().token, active.token);
            assert_eq!(recovery.0.lock().unwrap().session, Some((active.token,active.graph_id)));
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        assert!(prepare(&state, &pending, "".into(), false, Some(active.token), None, false).is_err());
        assert!(prepare(&state, &pending, dir.path().join("missing/graph.db").to_str().unwrap().into(), true, Some(active.token), None, false).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn native_resolution_canonicalizes_aliases_without_overwriting_links() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let alias = dir.path().join("alias");
        symlink(&real, &alias).unwrap();
        let path = real.join("گراف 😀.db");
        drop(Graph::create(&path).unwrap());
        assert_eq!(resolve_path(alias.join("گراف 😀.db").to_str().unwrap(), false).unwrap(), path);
        assert_eq!(resolve_path(alias.join("new.db").to_str().unwrap(), true).unwrap(), real.join("new.db"));
        let link = real.join("existing-link.db");
        symlink(&path, &link).unwrap();
        assert!(Graph::create(resolve_path(link.to_str().unwrap(), true).unwrap()).is_err());
        assert_eq!(Graph::inspect_identity(&path, true).unwrap(), Graph::open(&link).unwrap().id().unwrap());
    }

    #[test]
    fn failed_recovery_lock_does_not_partially_activate() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::default();
        let pending = PendingState::default();
        let recovery = RecoveryState::default();
        let prepared = prepare(&state, &pending, dir.path().join("g.db").to_str().unwrap().into(), true, None, None, false).unwrap();
        let copy = recovery.clone();
        let _ = std::thread::spawn(move || { let _held = copy.0.lock().unwrap(); panic!("disposable poisoned recovery lock"); }).join();
        assert!(activate(&state, &pending, &recovery, prepared.ticket, None).is_err());
        assert!(state.0.lock().unwrap().is_none());
        assert_eq!(pending.0.lock().unwrap().as_ref().unwrap().ticket, prepared.ticket);
    }
}
