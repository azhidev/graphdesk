//! Blocking SQLite work stays off the GUI event loop. Session checks and operations
//! share one lock, so queued writes cannot cross a graph switch.
use graph_core::{Block, EditAction, EditResult, Error, Graph, Page, TaskFilter, TaskHit};
use serde::Serialize;
use tauri::State;
use uuid::Uuid;
use crate::{ActiveGraph, AppState};

#[tauri::command]
pub(crate) async fn editor_search(session: Uuid, text: String, state: State<'_, AppState>) -> Result<Vec<graph_core::SearchHit>, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| g.editor.graph().search(&text))).await
}

#[derive(Debug, Serialize)]
pub(crate) struct EditorError { code: &'static str, message: String }
impl EditorError {
    fn new(code: &'static str, message: impl Into<String>) -> Self { Self { code, message: message.into() } }
}
impl From<Error> for EditorError {
    fn from(error: Error) -> Self {
        let code = match &error {
            Error::Conflict(_) => "conflict",
            Error::Invalid(_) => "invalid",
            Error::NotFound => "not_found",
            Error::Database(e) if e.sqlite_error().is_some_and(|e| matches!(e.extended_code & 255, 5 | 6)) => "busy",
            _ => "storage",
        };
        Self::new(code, error.to_string())
    }
}

#[derive(Serialize)]
pub(crate) struct Session { token: Uuid, pages: Vec<Page>, graph_id: Uuid }
#[derive(Serialize)]
pub(crate) struct Outline { blocks: Vec<Block>, can_undo: bool, can_redo: bool, revisions: std::collections::BTreeMap<Uuid, i64> }

#[derive(Serialize)]
pub(crate) struct Links { references: Vec<graph_core::InlineReference>, backlinks: Vec<Block> }

#[tauri::command]
pub(crate) async fn editor_links(session: Uuid, page_id: Uuid, state: State<'_, AppState>) -> Result<Links, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| Ok(Links {
        references: g.editor.graph().inline_references(page_id)?,
        backlinks: g.editor.graph().incoming_links(page_id)?,
    }))).await
}

async fn blocking<T: Send + 'static>(operation: impl FnOnce() -> Result<T, EditorError> + Send + 'static) -> Result<T, EditorError> {
    tauri::async_runtime::spawn_blocking(operation).await
        .map_err(|_| EditorError::new("internal", "editor worker failed"))?
}

#[tauri::command]
pub(crate) async fn editor_prepare_delete(session: Uuid, page_id: Uuid, id: Uuid, state: State<'_, AppState>) -> Result<graph_core::DeletePreview, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| g.editor.prepare_delete(page_id, id))).await
}

fn with_session<T>(state: &AppState, session: Uuid, operation: impl FnOnce(&mut ActiveGraph) -> Result<T, Error>) -> Result<T, EditorError> {
    let mut guard = state.0.lock().map_err(|_| EditorError::new("internal", "graph lock poisoned"))?;
    let graph = guard.as_mut().ok_or_else(|| EditorError::new("stale_session", "open a graph first"))?;
    if graph.token != session { return Err(EditorError::new("stale_session", "graph session changed; reload before editing")); }
    operation(graph).map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn editor_open(path: String, create: bool, state: State<'_, AppState>, recovery: State<'_, crate::recovery::RecoveryState>) -> Result<Session, EditorError> {
    let state = state.inner().clone();
    let recovery = recovery.inner().clone();
    blocking(move || {
        let mut guard = state.0.lock().map_err(|_| EditorError::new("internal", "graph lock poisoned"))?;
        let graph = if create { Graph::create(path) } else { Graph::open(path) }?;
        let pages = graph.pages()?;
        let graph_id = graph.id()?;
        let active = ActiveGraph::new(graph);
        let session = Session { token: active.token, pages, graph_id };
        recovery.0.lock().map_err(|_| EditorError::new("internal", "recovery lock unavailable"))?.session = Some((active.token, graph_id));
        *guard = Some(active);
        Ok(session)
    }).await
}

#[tauri::command]
pub(crate) async fn editor_page(session: Uuid, page_id: Uuid, state: State<'_, AppState>) -> Result<Outline, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| {
        let (blocks, revisions) = g.editor.graph().text_outline(page_id)?;
        let (can_undo, can_redo) = g.editor.history_state(page_id);
        Ok(Outline { blocks, can_undo, can_redo, revisions })
    })).await
}

#[tauri::command]
pub(crate) async fn editor_apply(session: Uuid, page_id: Uuid, action: EditAction, group: Option<Uuid>, state: State<'_, AppState>) -> Result<EditResult, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| g.editor.apply(page_id, action, group))).await
}

#[tauri::command]
pub(crate) async fn editor_history(session: Uuid, page_id: Uuid, redo: bool, state: State<'_, AppState>) -> Result<EditResult, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| g.editor.history(page_id, redo))).await
}

#[tauri::command]
pub(crate) async fn editor_create_page(session: Uuid, title: String, state: State<'_, AppState>) -> Result<Page, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| g.editor.create_page(&title))).await
}

#[tauri::command]
pub(crate) async fn editor_rename_page(session: Uuid, page_id: Uuid, title: String, state: State<'_, AppState>) -> Result<Page, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| g.editor.rename_page(page_id, &title))).await
}

#[tauri::command]
pub(crate) async fn editor_journal(session: Uuid, date: String, state: State<'_, AppState>) -> Result<Page, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| g.editor.journal(&date))).await
}

#[tauri::command]
pub(crate) async fn editor_find_journal(session: Uuid, date: String, state: State<'_, AppState>) -> Result<Option<Page>, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| g.editor.find_journal(&date))).await
}

#[tauri::command]
pub(crate) async fn editor_tasks(session: Uuid, status: TaskFilter, page_id: Option<Uuid>, limit: Option<usize>, offset: usize, state: State<'_, AppState>) -> Result<Vec<TaskHit>, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| g.editor.graph().tasks(status, page_id, limit.unwrap_or(100), offset))).await
}
