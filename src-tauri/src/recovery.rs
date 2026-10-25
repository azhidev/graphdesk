//! Private recovery IO has its own mutex/connection: a busy graph must not block
//! capture into the independent store. Never log draft text or include it in errors.
use std::{path::PathBuf, sync::{Arc, Mutex}, time::{SystemTime, UNIX_EPOCH}};
use graph_core::recovery::{PendingJournalNote, RecoveryDraft, RecoveryStore};
use serde::Serialize;
use tauri::{Manager, State};
use uuid::Uuid;

#[derive(Default)]
pub(crate) struct Access {
    pub session: Option<(Uuid, Uuid)>,
    pub root: Option<PathBuf>,
    pub store: Option<(Uuid, RecoveryStore)>,
}
#[derive(Clone, Default)]
pub(crate) struct RecoveryState(pub Arc<Mutex<Access>>);

#[derive(Serialize)]
pub(crate) struct RecoveryError { code: &'static str, message: &'static str }
fn failure(_: impl std::fmt::Display) -> RecoveryError {
    RecoveryError { code: "recovery", message: "Recovery storage unavailable; unsaved text is not protected. Keep the app open and retry." }
}
fn storage_error(error: graph_core::Error) -> RecoveryError {
    if matches!(error, graph_core::Error::Conflict(_)) {
        RecoveryError { code: "conflict", message: "The recovery draft changed. Refresh the list before discarding; no copy was removed." }
    } else { failure(error) }
}
fn now() -> i64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64 }
fn check_directory(root: &std::path::Path) -> Result<(), RecoveryError> {
    let metadata = std::fs::symlink_metadata(root).map_err(failure)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() { return Err(failure("invalid recovery directory")); }
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 { return Err(failure("recovery directory is not private")); }
    }
    Ok(())
}

async fn run<R: tauri::Runtime, T: Send + 'static>(app: tauri::AppHandle<R>, state: RecoveryState, session: Uuid,
    work: impl FnOnce(&mut RecoveryStore, Uuid) -> graph_core::Result<T> + Send + 'static) -> Result<T, RecoveryError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut access = state.0.lock().map_err(failure)?;
        let (token, graph) = access.session.ok_or(RecoveryError { code: "stale_session", message: "Open a graph before accessing recovery." })?;
        if token != session { return Err(RecoveryError { code: "stale_session", message: "The graph session changed. Reopen its recovery list." }); }
        let root = match &access.root {
            Some(root) => root.clone(),
            None => {
                let root = app.path().app_local_data_dir().map_err(failure)?.join("recovery");
                access.root = Some(root.clone()); root
            }
        };
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; builder.mode(0o700); }
        builder.create(&root).map_err(failure)?;
        check_directory(&root)?;
        if access.store.as_ref().map(|s| s.0) != Some(graph) {
            access.store = Some((graph, RecoveryStore::open(root.join(format!("{graph}.sqlite3"))).map_err(failure)?));
        }
        work(&mut access.store.as_mut().expect("opened store").1, graph).map_err(storage_error)
    }).await.map_err(failure)?
}

#[tauri::command]
pub(crate) async fn recovery_put<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, RecoveryState>, session: Uuid,
    page_id: Uuid, block_id: Uuid, base: String, revision: i64, content: String) -> Result<RecoveryDraft, RecoveryError> {
    run(app, state.inner().clone(), session, move |store, graph| store.put(graph, page_id, block_id, &base, revision, &content, now())).await
}

#[tauri::command]
pub(crate) async fn recovery_list<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, RecoveryState>, session: Uuid) -> Result<Vec<RecoveryDraft>, RecoveryError> {
    run(app, state.inner().clone(), session, move |store, graph| store.list(graph, now())).await
}

#[tauri::command]
pub(crate) async fn recovery_remove<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, RecoveryState>, session: Uuid, id: Uuid, content: String, updated_at: i64) -> Result<(), RecoveryError> {
    run(app, state.inner().clone(), session, move |store, graph| store.remove_matching(graph, id, &content, updated_at)).await
}

#[tauri::command]
pub(crate) async fn recovery_clear<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, RecoveryState>, session: Uuid, block_id: Uuid, content: String) -> Result<(), RecoveryError> {
    run(app, state.inner().clone(), session, move |store, graph| store.clear_matching(graph, block_id, &content)).await
}

#[tauri::command]
pub(crate) async fn recovery_note_put<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, RecoveryState>, session: Uuid,
    id: Uuid, block_id: Uuid, date: String, content: String) -> Result<PendingJournalNote, RecoveryError> {
    run(app, state.inner().clone(), session, move |store, graph| store.note_put(graph, id, block_id, &date, &content, now())).await
}

#[tauri::command]
pub(crate) async fn recovery_notes<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, RecoveryState>, session: Uuid) -> Result<Vec<PendingJournalNote>, RecoveryError> {
    run(app, state.inner().clone(), session, move |store, graph| store.notes(graph, now())).await
}

#[tauri::command]
pub(crate) async fn recovery_note_remove<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, RecoveryState>, session: Uuid,
    id: Uuid, version: i64) -> Result<(), RecoveryError> {
    run(app, state.inner().clone(), session, move |store, graph| store.note_remove(graph, id, version)).await
}

#[tauri::command]
pub(crate) async fn recovery_inventory<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, RecoveryState>) -> Result<usize, RecoveryError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let access = state.0.lock().map_err(failure)?;
        let root = match &access.root { Some(root) => root.clone(), None => app.path().app_local_data_dir().map_err(failure)?.join("recovery") };
        if !root.exists() { return Ok(0); }
        check_directory(&root)?;
        let mut count = 0;
        for entry in std::fs::read_dir(root).map_err(failure)? {
            let entry = entry.map_err(failure)?;
            if !entry.file_type().map_err(failure)?.is_file() { continue; }
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("sqlite3") { continue; }
            let Some(graph) = path.file_stem().and_then(|s| s.to_str()).and_then(|s| Uuid::parse_str(s).ok()) else { continue; };
            let mut store = RecoveryStore::open(path).map_err(failure)?;
            let drafts = store.list(graph, now()).map_err(failure)?;
            let notes = store.notes(graph, now()).map_err(failure)?;
            if !drafts.is_empty() || !notes.is_empty() { count += 1; }
        }
        Ok(count)
    }).await.map_err(failure)?
}
