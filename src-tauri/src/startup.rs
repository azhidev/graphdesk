//! First-run detection and demonstration seeding.
//!
//! A genuinely fresh installation gets exactly one `GraphDesk Demo` graph, then
//! remembering + startup reopening are enabled so the demo (or the last active
//! graph) returns on the next launch. Existing installations are never seeded,
//! even when graph-history persistence was disabled: any of a saved registry
//! document, saved preferences, private recovery stores, or a completed
//! first-run marker is treated as an existing installation. Corrupt/unknown
//! registry content is never repaired or replaced here.
//!
//! All graph writes go through the ordinary `Graph`/`demo` core APIs, so the
//! demo is a normal, fully editable local graph. It never overwrites an existing
//! database and is never recreated once the marker exists.
use std::{fs, io::Write, path::{Path, PathBuf}, sync::{Arc, Mutex}};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use uuid::Uuid;
use graph_core::Graph;
use crate::{recovery::{self, RecoveryState}, registry::{self, Registry, RegistryState}};

const MARKER: &str = "first-run.json";
const PREFERENCES: &str = "preferences.json";
const DEMO_FILE: &str = "GraphDesk Demo.sqlite3";
const DEMO_DIR: &str = "graphs";

#[derive(Clone, Default)]
pub(crate) struct StartupState(pub Arc<Mutex<Option<PathBuf>>>);

#[derive(Serialize)]
pub(crate) struct Demo { path: String, graph_id: Uuid }
#[derive(Serialize)]
pub(crate) struct Started { fresh: bool, demo: Option<Demo>, warning: bool, registry: registry::Loaded }

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker { version: u32, demo_path: Option<String> }

fn unavailable() -> &'static str { "Startup state is unavailable; no demo graph was created." }

fn demo_root<R: tauri::Runtime>(app: &tauri::AppHandle<R>, state: &StartupState) -> Result<PathBuf, &'static str> {
    let mut guard = state.0.lock().map_err(|_| unavailable())?;
    if guard.is_none() {
        *guard = Some(app.path().app_data_dir().map_err(|_| unavailable())?.join(DEMO_DIR));
    }
    Ok(guard.as_ref().expect("resolved demo directory").clone())
}

/// Existing installation evidence. Deliberately conservative: anything that
/// suggests prior use suppresses first-run seeding.
fn is_existing<R: tauri::Runtime>(config: &Path, app: &tauri::AppHandle<R>, recovery: &RecoveryState) -> bool {
    evidence(&config.join(registry::FILE))
        || evidence(&config.join(PREFERENCES))
        || evidence(&config.join(MARKER))
        || recovery::has_stores(app, recovery)
}

fn evidence(path: &Path) -> bool {
    !matches!(fs::symlink_metadata(path), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
}

fn create_demo_dir(directory: &Path) -> std::io::Result<()> {
    crate::preferences::check_parents(directory)?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; builder.mode(0o700); }
    builder.create(directory)
}

fn write_marker(config: &Path, demo_path: Option<&str>) -> std::io::Result<()> {
    check_private(config)?;
    let bytes = serde_json::to_vec(&Marker { version: 1, demo_path: demo_path.map(str::to_owned) })
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid marker"))?;
    let temporary = config.join(format!(".first-run-{}.tmp", Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let mut file = options.open(&temporary)?;
    let result = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        check_private(config)?;
        fs::rename(&temporary, config.join(MARKER))?;
        #[cfg(unix)] { fs::File::open(config)?.sync_all()?; }
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temporary); }
    result
}

fn check_private(root: &Path) -> std::io::Result<()> {
    crate::preferences::check_parents(root)?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; builder.mode(0o700); }
    builder.create(root)?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if fs::symlink_metadata(root)?.permissions().mode() & 0o077 != 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "startup storage is not private"));
        }
    }
    Ok(())
}

fn run<R: tauri::Runtime>(app: &tauri::AppHandle<R>, registry_state: &RegistryState, startup: &StartupState,
    recovery: &RecoveryState, date: &str) -> Started {
    // Serialize detection/seeding with registry settings/record writes, including
    // repeated IPC requests. Never hold graph or recovery locks during graph IO.
    let Ok(mut root) = registry_state.0.lock() else {
        return Started { fresh: false, demo: None, warning: true, registry: Registry::default().loaded(true) };
    };
    if root.is_none() { *root = app.path().app_config_dir().ok(); }
    let Some(config) = root.as_ref() else {
        return Started { fresh: false, demo: None, warning: true, registry: Registry::default().loaded(true) };
    };
    // Let an explicit opt-out document win before anything else is considered.
    let current = match registry::load(&config) {
        Ok(registry) => registry,
        // Corrupt or unknown future content is preserved untouched and reported.
        Err(_) => return Started { fresh: false, demo: None, warning: true, registry: Registry::default().loaded(true) },
    };
    if is_existing(&config, app, recovery) {
        return Started { fresh: false, demo: None, warning: false, registry: current.loaded(false) };
    }
    if graph_core::validate_journal_date(date).is_err() {
        return Started { fresh: false, demo: None, warning: true, registry: current.loaded(false) };
    }

    let directory = match demo_root(app, startup) {
        Ok(directory) => directory,
        Err(_) => return Started { fresh: false, demo: None, warning: true, registry: current.loaded(false) },
    };
    let path = directory.join(DEMO_FILE);
    let demo = if evidence(&path) {
        // Leftover from an interrupted first run: never seed or overwrite it,
        // but reuse it if it is a current, valid GraphDesk graph.
        Graph::inspect_identity(&path, true).ok().map(|id| (path, id))
    } else if create_demo_dir(&directory).is_ok() {
        match Graph::create(&path).and_then(|mut graph| { graph_core::demo::seed_demo(&mut graph, date)?; graph.id() }) {
            Ok(id) => Some((path, id)),
            Err(_) => None,
        }
    } else { None };

    // New installations default to persistent history + reopening; explicit
    // settings from existing installations were returned above unchanged.
    let mut registry = current;
    registry.remember = true;
    registry.reopen = true;
    registry.entries.clear();
    let mut demo_out = None;
    if let Some((path, graph_id)) = &demo {
        if let Some(path) = path.to_str() {
            demo_out = Some(Demo { path: path.to_owned(), graph_id: *graph_id });
        }
    }
    let saved = registry::save(&config, &registry).is_ok();
    let marker = write_marker(&config, demo_out.as_ref().map(|d| d.path.as_str())).is_ok();
    let warning = !(saved && marker) || demo_out.is_none();
    // Last-active is recorded by the normal activation flow, never by seeding.
    Started { fresh: true, demo: demo_out, warning, registry: registry.loaded(false) }
}

#[tauri::command]
pub(crate) async fn graph_first_run<R: tauri::Runtime>(app: tauri::AppHandle<R>, registry_state: State<'_, RegistryState>,
    startup: State<'_, StartupState>, recovery: State<'_, RecoveryState>, date: String) -> Result<Started, &'static str> {
    let registry_state = registry_state.inner().clone();
    let startup = startup.inner().clone();
    let recovery = recovery.inner().clone();
    tauri::async_runtime::spawn_blocking(move || run(&app, &registry_state, &startup, &recovery, &date))
        .await.map_err(|_| unavailable())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded(path: &Path, date: &str) -> Uuid {
        let mut graph = Graph::create(path).unwrap();
        graph_core::demo::seed_demo(&mut graph, date).unwrap();
        graph.id().unwrap()
    }

    #[test]
    fn marker_storage_is_private_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        assert!(!config.exists());
        write_marker(&config, Some("/tmp/گراف Demo.sqlite3")).unwrap();
        assert!(config.join(MARKER).exists());
        let parsed: Marker = serde_json::from_slice(&fs::read(config.join(MARKER)).unwrap()).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.demo_path.as_deref(), Some("/tmp/گراف Demo.sqlite3"));
        // Repeating is idempotent: exactly one marker document remains.
        write_marker(&config, None).unwrap();
        assert_eq!(fs::read_dir(&config).unwrap().count(), 1);
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::symlink_metadata(&config).unwrap().permissions().mode() & 0o777, 0o700);
            assert_eq!(fs::metadata(config.join(MARKER)).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn marker_document_round_trips_without_unknown_fields() {
        let marker = Marker { version: 1, demo_path: Some("/tmp/گراف Demo.sqlite3".into()) };
        let bytes = serde_json::to_vec(&marker).unwrap();
        let parsed: Marker = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.demo_path.as_deref(), Some("/tmp/گراف Demo.sqlite3"));
        assert!(serde_json::from_slice::<Marker>(br#"{"version":1,"demo_path":null,"extra":true}"#).is_err());
    }

    #[test]
    fn demo_seed_keeps_identity_and_separate_databases_independent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("demo.sqlite3");
        let id = seeded(&path, "2024-02-29");
        assert_eq!(Graph::inspect_identity(&path, true).unwrap(), id);
        // A second fresh database elsewhere is independent.
        let other = dir.path().join("other.sqlite3");
        let second = seeded(&other, "2024-03-01");
        assert_ne!(id, second);
    }
}
