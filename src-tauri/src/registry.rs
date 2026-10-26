//! Opt-in graph paths only, separate from preferences/recovery. Invalid or newer
//! files are never repaired by load/record. Explicit settings resets invalid
//! content; forget(null) is the explicit reset, defaulting opt-ins off if invalid.
use std::{fs::{self, OpenOptions}, io::{self, Read, Write}, path::{Path, PathBuf}, sync::{Arc, Mutex}};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use uuid::Uuid;
use crate::{AppState, preferences::{check_parents, check_root, check_file}};

const FILE: &str = "graph-state.json";
const LIMIT: usize = 10;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Entry { path: String, graph_id: Uuid }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry { version: u32, remember: bool, reopen: bool, entries: Vec<Entry> }
impl Default for Registry {
    fn default() -> Self { Self { version: 1, remember: false, reopen: false, entries: Vec::new() } }
}
#[derive(Debug, Serialize)]
pub(crate) struct Loaded { remember: bool, reopen: bool, entries: Vec<Entry>, warning: bool }
impl Registry {
    fn loaded(self, warning: bool) -> Loaded { Loaded { remember: self.remember, reopen: self.reopen, entries: self.entries, warning } }
    fn record(&mut self, entry: Entry) {
        if !self.remember { return; }
        self.entries.retain(|e| e.path != entry.path);
        self.entries.insert(0, entry);
        self.entries.truncate(LIMIT);
    }
}
#[derive(Clone, Default)]
pub(crate) struct RegistryState(pub Arc<Mutex<Option<PathBuf>>>);

fn invalid() -> io::Error { io::Error::new(io::ErrorKind::InvalidData, "invalid graph registry storage") }
fn unavailable() -> &'static str { "Graph registry unavailable; paths/settings were not saved." }

fn load(root: &Path) -> io::Result<Registry> {
    check_parents(root)?;
    if !root.try_exists()? { return Ok(Registry::default()); }
    check_root(root)?;
    let path = root.join(FILE);
    match check_file(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Registry::default()),
        result => result?,
    }
    let file = fs::File::open(&path)?;
    check_root(root)?;
    check_file(&path)?;
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        let opened = file.metadata()?;
        let current = fs::symlink_metadata(&path)?;
        if (opened.dev(), opened.ino()) != (current.dev(), current.ino()) { return Err(invalid()); }
    }
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes)?;
    if bytes.len() > 65536 { return Err(invalid()); }
    let registry: Registry = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    let mut paths = std::collections::HashSet::new();
    if registry.version != 1 || registry.entries.len() > LIMIT
        || (!registry.remember && (registry.reopen || !registry.entries.is_empty()))
        || registry.entries.iter().any(|e| !Path::new(&e.path).is_absolute() || !paths.insert(&e.path)) {
        return Err(invalid());
    }
    Ok(registry)
}

fn save(root: &Path, registry: &Registry) -> io::Result<()> {
    check_parents(root)?;
    let mut directory = fs::DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; directory.mode(0o700); }
    directory.create(root)?;
    check_root(root)?;
    let target = root.join(FILE);
    match check_file(&target) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {},
        result => result?,
    }
    let temporary = root.join(format!(".graph-state-{}.tmp", Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let mut file = options.open(&temporary)?;
    let result = (|| {
        let bytes = serde_json::to_vec(registry).map_err(|_| invalid())?;
        // Never acknowledge a registry that the bounded loader would reject.
        if bytes.len() > 65536 { return Err(invalid()); }
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        check_root(root)?;
        match check_file(&target) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {},
            result => result?,
        }
        fs::rename(&temporary, &target)?;
        #[cfg(unix)] { fs::File::open(root)?.sync_all()?; }
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temporary); }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versioned_private_roundtrip_path_mru_and_opt_out() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("config");
        assert!(!load(&root).unwrap().remember);
        assert!(!root.exists());
        let mut registry = Registry { remember: true, reopen: true, ..Registry::default() };
        let identity = Uuid::new_v4();
        // Copies share UUID but occupy separate MRU entries; path is the key.
        for n in 0..12 {
            registry.record(Entry { path: dir.path().join(format!("گراف {n} 😀.db")).to_str().unwrap().into(), graph_id: identity });
        }
        assert_eq!(registry.entries.len(), LIMIT);
        let repeat = registry.entries[5].clone();
        registry.record(repeat.clone());
        assert_eq!(registry.entries[0], repeat);
        assert_eq!(registry.entries.len(), LIMIT);
        let mut replacement = repeat.clone();
        replacement.graph_id = Uuid::new_v4();
        registry.record(replacement.clone());
        assert_eq!(registry.entries[0], replacement);
        save(&root, &registry).unwrap();
        assert_eq!(load(&root).unwrap().entries, registry.entries);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&root).unwrap().permissions().mode() & 0o777, 0o700);
            assert_eq!(fs::metadata(root.join(FILE)).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let mut disabled = Registry::default();
        disabled.record(repeat);
        assert!(disabled.entries.is_empty());
    }

    #[test]
    fn invalid_unknown_oversized_and_invariant_violations_never_load() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("config");
        save(&root, &Registry::default()).unwrap();
        for content in ["broken".to_owned(), "{}".to_owned(),
            r#"{"version":2,"remember":true,"reopen":true,"entries":[]}"#.into(),
            r#"{"version":1,"remember":false,"reopen":true,"entries":[]}"#.into(),
            format!(r#"{{"version":1,"remember":true,"reopen":false,"entries":[{{"path":"relative.db","graph_id":"{}"}}]}}"#,Uuid::new_v4()),
            " ".repeat(65537)] {
            fs::write(root.join(FILE), &content).unwrap();
            assert!(load(&root).is_err());
            assert_eq!(fs::read_to_string(root.join(FILE)).unwrap(), content);
        }
    }

    #[test]
    fn serialized_writes_leave_one_complete_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("config");
        let state = RegistryState::default();
        std::thread::scope(|scope| {
            for n in 0..12 {
                let root = &root;
                let state = state.clone();
                scope.spawn(move || {
                    let _guard = state.0.lock().unwrap();
                    let registry = Registry { remember: true, reopen: n % 2 == 0, ..Registry::default() };
                    save(root, &registry).unwrap();
                    assert_eq!(load(root).unwrap().reopen, registry.reopen);
                });
            }
        });
        assert!(load(&root).is_ok());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }

    #[test]
    fn oversized_serialization_preserves_previous_registry_and_cleans_temporary() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("config");
        save(&root, &Registry::default()).unwrap();
        let before = fs::read(root.join(FILE)).unwrap();
        let too_large = Registry { remember: true, entries: vec![Entry { path: format!("/{}", "x".repeat(65536)), graph_id: Uuid::new_v4() }], ..Registry::default() };
        assert!(save(&root, &too_large).is_err());
        assert_eq!(fs::read(root.join(FILE)).unwrap(), before);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_nonprivate_and_unwritable_storage_are_rejected() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("config");
        save(&root, &Registry::default()).unwrap();
        let before = fs::read(root.join(FILE)).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o500)).unwrap();
        assert!(save(&root, &Registry::default()).is_err());
        assert_eq!(fs::read(root.join(FILE)).unwrap(), before);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let link = dir.path().join("linked");
        symlink(&root, &link).unwrap();
        assert!(load(&link).is_err());
        assert!(save(&link, &Registry::default()).is_err());
        assert!(save(&link.join("nested"), &Registry::default()).is_err());
        let target = dir.path().join("target.json");
        fs::rename(root.join(FILE), &target).unwrap();
        symlink(&target, root.join(FILE)).unwrap();
        assert!(load(&root).is_err());
        assert!(save(&root, &Registry::default()).is_err());
        fs::remove_file(root.join(FILE)).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(load(&root).is_err());
        assert!(save(&root, &Registry::default()).is_err());
    }
}

async fn run<R: tauri::Runtime, T: Send + 'static>(app: tauri::AppHandle<R>, state: RegistryState,
    work: impl FnOnce(&Path) -> Result<T, &'static str> + Send + 'static) -> Result<T, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut root = state.0.lock().map_err(|_| unavailable())?;
        if root.is_none() { *root = Some(app.path().app_config_dir().map_err(|_| unavailable())?); }
        work(root.as_ref().expect("resolved config directory"))
    }).await.map_err(|_| unavailable())?
}

#[tauri::command]
pub(crate) async fn graph_registry_load<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, RegistryState>) -> Result<Loaded, &'static str> {
    // Loading is fail-safe even if resolving/locking the private directory fails.
    Ok(run(app, state.inner().clone(), |root| Ok(match load(root) {
        Ok(registry) => registry.loaded(false), Err(_) => Registry::default().loaded(true),
    })).await.unwrap_or_else(|_| Registry::default().loaded(true)))
}

#[tauri::command]
pub(crate) async fn graph_registry_settings<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, RegistryState>, remember: bool, reopen: bool) -> Result<Loaded, &'static str> {
    run(app, state.inner().clone(), move |root| {
        // Explicit settings acknowledgement authorizes replacing invalid/unknown
        // schema content with version 1. No entries survive an invalid document.
        let mut registry = load(root).unwrap_or_default();
        registry.remember = remember;
        registry.reopen = remember && reopen;
        if !remember { registry.entries.clear(); }
        save(root, &registry).map_err(|_| unavailable())?;
        Ok(registry.loaded(false))
    }).await
}

#[tauri::command]
pub(crate) async fn graph_registry_forget<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, RegistryState>, path: Option<String>) -> Result<Loaded, &'static str> {
    run(app, state.inner().clone(), move |root| {
        let mut registry = match load(root) {
            Ok(registry) => registry,
            Err(_) if path.is_none() => Registry::default(),
            Err(_) => return Err(unavailable()),
        };
        if let Some(path) = path { registry.entries.retain(|e| e.path != path); }
        else { registry.entries.clear(); }
        save(root, &registry).map_err(|_| unavailable())?;
        Ok(registry.loaded(false))
    }).await
}

#[tauri::command]
pub(crate) async fn graph_registry_record<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, RegistryState>,
    graph: State<'_, AppState>, session: Uuid) -> Result<Loaded, &'static str> {
    let graph = graph.inner().clone();
    let entry = tauri::async_runtime::spawn_blocking(move || {
        let guard = graph.0.lock().map_err(|_| unavailable())?;
        let active = guard.as_ref().filter(|g| g.token == session).ok_or("Graph session changed; path was not remembered.")?;
        let path = active.path.as_ref().and_then(|p| p.to_str()).ok_or("Active graph has no resolved path.")?.to_owned();
        let graph_id = active.editor.graph().id().map_err(|_| unavailable())?;
        Ok::<_, &'static str>(Entry { path, graph_id })
    }).await.map_err(|_| unavailable())??;
    // The authoritative descriptor has been copied. Never hold graph/recovery
    // locks while touching registry files or waiting for its serialization lock.
    run(app, state.inner().clone(), move |root| {
        let mut registry = match load(root) {
            Ok(registry) => registry,
            Err(_) => return Ok(Registry::default().loaded(true)),
        };
        if registry.remember {
            registry.record(entry);
            save(root, &registry).map_err(|_| unavailable())?;
        }
        Ok(registry.loaded(false))
    }).await
}
