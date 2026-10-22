//! Blocking SQLite work stays off the GUI event loop. Session checks and operations
//! share one lock, so queued writes cannot cross a graph switch.
use graph_core::{Block, EditAction, EditResult, Error, Graph, Page};
use serde::Serialize;
use tauri::State;
use uuid::Uuid;
use crate::{ActiveGraph, AppState};

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
pub(crate) struct Session { token: Uuid, pages: Vec<Page> }
#[derive(Serialize)]
pub(crate) struct Outline { blocks: Vec<Block>, can_undo: bool, can_redo: bool }

async fn blocking<T: Send + 'static>(operation: impl FnOnce() -> Result<T, EditorError> + Send + 'static) -> Result<T, EditorError> {
    tauri::async_runtime::spawn_blocking(operation).await
        .map_err(|_| EditorError::new("internal", "editor worker failed"))?
}

fn with_session<T>(state: &AppState, session: Uuid, operation: impl FnOnce(&mut ActiveGraph) -> Result<T, Error>) -> Result<T, EditorError> {
    let mut guard = state.0.lock().map_err(|_| EditorError::new("internal", "graph lock poisoned"))?;
    let graph = guard.as_mut().ok_or_else(|| EditorError::new("stale_session", "open a graph first"))?;
    if graph.token != session { return Err(EditorError::new("stale_session", "graph session changed; reload before editing")); }
    operation(graph).map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn editor_open(path: String, create: bool, state: State<'_, AppState>) -> Result<Session, EditorError> {
    let state = state.inner().clone();
    blocking(move || {
        let mut guard = state.0.lock().map_err(|_| EditorError::new("internal", "graph lock poisoned"))?;
        let graph = if create { Graph::create(path) } else { Graph::open(path) }?;
        let pages = graph.pages()?;
        let active = ActiveGraph::new(graph);
        let session = Session { token: active.token, pages };
        *guard = Some(active);
        Ok(session)
    }).await
}

#[tauri::command]
pub(crate) async fn editor_page(session: Uuid, page_id: Uuid, state: State<'_, AppState>) -> Result<Outline, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| {
        let blocks = g.editor.graph().page_blocks(page_id)?;
        let (can_undo, can_redo) = g.editor.history_state(page_id);
        Ok(Outline { blocks, can_undo, can_redo })
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
