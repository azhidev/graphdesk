#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use graph_core::{Block, Editor, Graph, Page};
use tauri::State;
use uuid::Uuid;

mod editor;

struct ActiveGraph { token: Uuid, editor: Editor }
impl ActiveGraph {
    fn new(graph: Graph) -> Self { Self { token: Uuid::new_v4(), editor: Editor::new(graph) } }
}

#[derive(Default, Clone)]
struct AppState(Arc<Mutex<Option<ActiveGraph>>>);

fn with_graph<T>(state: &AppState, operation: impl FnOnce(&mut Graph) -> graph_core::Result<T>) -> Result<T, String> {
    let mut guard = state.0.lock().map_err(|_| "graph lock poisoned".to_string())?;
    let graph = guard.as_mut().ok_or("open a graph first")?;
    operation(graph.editor.graph_mut()).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_graph(path: String, create: bool, state: State<'_, AppState>) -> Result<Vec<Page>, String> {
    let mut guard = state.0.lock().map_err(|_| "graph lock poisoned".to_string())?;
    let graph = if create { Graph::create(path) } else { Graph::open(path) }.map_err(|e| e.to_string())?;
    let pages = graph.pages().map_err(|e| e.to_string())?;
    *guard = Some(ActiveGraph::new(graph));
    Ok(pages)
}

#[tauri::command]
fn create_page(title: String, state: State<'_, AppState>) -> Result<Page, String> {
    with_graph(&state, |g| g.create_page(&title))
}

#[tauri::command]
fn create_journal(date: String, state: State<'_, AppState>) -> Result<Page, String> {
    with_graph(&state, |g| g.create_journal(&date))
}

#[tauri::command]
fn list_blocks(page_id: Uuid, parent_id: Option<Uuid>, state: State<'_, AppState>) -> Result<Vec<Block>, String> {
    let guard = state.0.lock().map_err(|_| "graph lock poisoned".to_string())?;
    guard.as_ref().ok_or("open a graph first")?.editor.graph().children(page_id, parent_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_block(page_id: Uuid, parent_id: Option<Uuid>, content: String, state: State<'_, AppState>) -> Result<Block, String> {
    with_graph(&state, |g| g.create_block(page_id, parent_id, &content))
}

fn app_builder<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![open_graph, create_page, create_journal, list_blocks, create_block,
            editor::editor_open, editor::editor_page, editor::editor_apply, editor::editor_history, editor::editor_create_page])
}

fn main() {
    app_builder(tauri::Builder::default())
        .run(tauri::generate_context!())
        .expect("failed to run Graphdesk");
}

#[cfg(test)]
mod tests;
