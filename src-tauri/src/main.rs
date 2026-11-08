#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use graph_core::{Block, Editor, Graph, Page};
use tauri::State;
use uuid::Uuid;

mod editor;
mod extensions;
mod habits;
mod recovery;
mod preferences;
mod registry;
mod switching;
mod startup;

struct ActiveGraph { token: Uuid, editor: Editor, path: Option<std::path::PathBuf>, activation_ticket: Option<Uuid> }
impl ActiveGraph {
    fn new(graph: Graph) -> Self { Self { token: Uuid::new_v4(), editor: Editor::new(graph), path: None, activation_ticket: None } }
    fn at_path(graph: Graph, path: std::path::PathBuf) -> Self { Self { path: Some(path), ..Self::new(graph) } }
}

#[derive(Default, Clone)]
struct AppState(Arc<Mutex<Option<ActiveGraph>>>);

fn with_graph<T>(state: &AppState, operation: impl FnOnce(&mut Graph) -> graph_core::Result<T>) -> Result<T, String> {
    let mut guard = state.0.lock().map_err(|_| "graph lock poisoned".to_string())?;
    let graph = guard.as_mut().ok_or("open a graph first")?;
    operation(graph.editor.graph_mut()).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_graph(path: String, create: bool, state: State<'_, AppState>, recovery: State<'_, recovery::RecoveryState>) -> Result<Vec<Page>, String> {
    let mut guard = state.0.lock().map_err(|_| "graph lock poisoned".to_string())?;
    let path = switching::resolve_path(&path, create).map_err(|e| e.to_string())?;
    let graph = if create { Graph::create(&path) } else { Graph::open(&path) }.map_err(|e| e.to_string())?;
    let pages = graph.pages().map_err(|e| e.to_string())?;
    // Legacy graph switching must invalidate independent recovery access too.
    recovery.0.lock().map_err(|_| "recovery lock unavailable".to_string())?.session = None;
    *guard = Some(ActiveGraph::at_path(graph, path));
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
        .manage(recovery::RecoveryState::default())
        .manage(preferences::PreferencesState::default())
        .manage(registry::RegistryState::default())
        .manage(switching::PendingState::default())
        .manage(startup::StartupState::default())
        .invoke_handler(tauri::generate_handler![open_graph, create_page, create_journal, list_blocks, create_block,
            editor::editor_open, editor::editor_page, editor::editor_apply, editor::editor_history, editor::editor_create_page,
            editor::editor_rename_page, editor::editor_journal, editor::editor_find_journal, editor::editor_tasks, editor::editor_search, editor::editor_links, editor::editor_prepare_delete,
            editor::editor_submit_note, editor::editor_note_state,
            extensions::extension_state, extensions::extension_configure, extensions::extension_request,
            habits::habits_request,
            recovery::recovery_note_put, recovery::recovery_notes, recovery::recovery_note_remove,
            preferences::preferences_load, preferences::preferences_save, preferences::habits_timezone,
            registry::graph_registry_load, registry::graph_registry_settings, registry::graph_registry_forget, registry::graph_registry_record,
            switching::graph_pick, switching::graph_prepare, switching::graph_activate, switching::graph_cancel, switching::graph_reconcile,
            startup::graph_first_run,
            recovery::recovery_put, recovery::recovery_list, recovery::recovery_remove, recovery::recovery_clear, recovery::recovery_inventory])
}

fn main() {
    use tauri::Manager;
    // First plugin: subsequent launches only raise this window. Arguments are
    // deliberately ignored; single-instance is not a cross-process graph lock.
    app_builder(tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init()))
        .run(tauri::generate_context!())
        .expect("failed to run Graphdesk");
}

#[cfg(test)]
mod tests;
