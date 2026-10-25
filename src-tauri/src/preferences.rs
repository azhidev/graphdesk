//! App-only presentation preferences. No graph paths, registry, or note content.
use std::{fs::{self, OpenOptions}, io::{self, Read, Write}, path::{Path, PathBuf}, sync::{Arc, Mutex}};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use uuid::Uuid;

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Language { #[default] En, Fa }
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Calendar { #[default] Gregorian, Jalali }
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SearchMode { #[default] Original, Persian }
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Preferences { pub language: Language, pub calendar: Calendar, pub search_mode: SearchMode }
#[derive(Serialize)]
pub(crate) struct Loaded { preferences: Preferences, warning: bool }
#[derive(Default, Clone)]
pub(crate) struct PreferencesState(pub Arc<Mutex<Option<PathBuf>>>);

fn unavailable() -> &'static str { "Preferences unavailable; changes were not saved." }
fn invalid() -> io::Error { io::Error::new(io::ErrorKind::InvalidData, "invalid preferences storage") }

// Reject existing symlinks in the config path, including parents. The directory
// is app-private; no attempt is made to repair ownership/permissions silently.
fn check_parents(root: &Path) -> io::Result<()> {
    for path in root.ancestors() {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {},
            Ok(_) => return Err(invalid()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {},
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
fn check_root(root: &Path) -> io::Result<()> {
    check_parents(root)?;
    let metadata = fs::symlink_metadata(root)?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 { return Err(invalid()); }
    }
    let _ = metadata;
    Ok(())
}
fn check_file(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() { return Err(invalid()); }
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 { return Err(invalid()); }
    }
    Ok(())
}
fn load(root: &Path) -> io::Result<Preferences> {
    check_parents(root)?;
    if !root.try_exists()? { return Ok(Preferences::default()); }
    check_root(root)?;
    let path = root.join("preferences.json");
    match check_file(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Preferences::default()),
        result => result?,
    }
    let mut bytes = Vec::new();
    let file = fs::File::open(&path)?;
    // Validate the opened handle before reading, not only the pathname inspected
    // above. A final-component swap must not make us read a different file.
    check_root(root)?;
    check_file(&path)?;
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        let opened = file.metadata()?;
        let current = fs::symlink_metadata(&path)?;
        if (opened.dev(), opened.ino()) != (current.dev(), current.ino()) { return Err(invalid()); }
    }
    file.take(65537).read_to_end(&mut bytes)?;
    if bytes.len() > 65536 { return Err(invalid()); }
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}
fn save(root: &Path, preferences: &Preferences) -> io::Result<()> {
    check_parents(root)?;
    let mut directory = fs::DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; directory.mode(0o700); }
    directory.create(root)?;
    check_root(root)?;
    let target = root.join("preferences.json");
    match check_file(&target) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {},
        result => result?,
    }
    let temporary = root.join(format!(".preferences-{}.tmp", Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let mut file = options.open(&temporary)?;
    let result = (|| {
        let bytes = serde_json::to_vec(preferences).map_err(|_| invalid())?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        // Recheck before replacing: symlinks are an error, not an overwrite target.
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

#[tauri::command]
pub(crate) async fn preferences_load<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, PreferencesState>) -> Result<Loaded, &'static str> {
    let state = state.inner().clone();
    Ok(tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            let mut guard = state.0.lock().map_err(|_| invalid())?;
            if guard.is_none() { *guard = Some(app.path().app_config_dir().map_err(|_| invalid())?); }
            load(guard.as_ref().expect("config directory"))
        })();
        match result { Ok(preferences) => Loaded { preferences, warning: false }, Err(_) => Loaded { preferences: Preferences::default(), warning: true } }
    }).await.unwrap_or(Loaded { preferences: Preferences::default(), warning: true }))
}

#[tauri::command]
pub(crate) async fn preferences_save<R: tauri::Runtime>(app: tauri::AppHandle<R>, state: State<'_, PreferencesState>, preferences: Preferences) -> Result<(), &'static str> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = state.0.lock().map_err(|_| unavailable())?;
        if guard.is_none() { *guard = Some(app.path().app_config_dir().map_err(|_| unavailable())?); }
        save(guard.as_ref().expect("config directory"), &preferences).map_err(|_| unavailable())
    }).await.map_err(|_| unavailable())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_atomic_roundtrip_defaults_malformed_and_concurrent_writes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("config");
        assert_eq!(load(&root).unwrap(), Preferences::default());
        assert!(!root.exists());
        let preferences = Preferences { language: Language::Fa, calendar: Calendar::Jalali, search_mode: SearchMode::Persian };
        save(&root, &preferences).unwrap();
        assert_eq!(load(&root).unwrap(),preferences);
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&root).unwrap().permissions().mode() & 0o777,0o700);
            assert_eq!(fs::metadata(root.join("preferences.json")).unwrap().permissions().mode() & 0o777,0o600);
        }
        for content in ["invalid", "{}", r#"{"language":"bogus","calendar":"gregorian","search_mode":"original"}"#, r#"{"language":"en","calendar":"gregorian","search_mode":"original","graph_path":"never"}"#] {
            fs::write(root.join("preferences.json"),content).unwrap();
            assert!(load(&root).is_err());
        }
        let state = PreferencesState::default();
        std::thread::scope(|scope| {
            for n in 0..12 {
                let root = &root;
                let state = state.clone();
                let preferences = preferences.clone();
                scope.spawn(move || {
                    let _guard = state.0.lock().unwrap();
                    let prefs = if n % 2 == 0 { preferences.clone() } else { Preferences::default() };
                    save(root,&prefs).unwrap();
                    assert_eq!(load(root).unwrap(),prefs);
                });
            }
        });
        assert!(load(&root).is_ok());
        assert_eq!(fs::read_dir(&root).unwrap().count(),1);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_and_nonprivate_storage_fail_closed() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("config");
        save(&root,&Preferences::default()).unwrap();
        let link = dir.path().join("link");
        symlink(&root,&link).unwrap();
        assert!(load(&link).is_err());
        assert!(save(&link,&Preferences::default()).is_err());
        assert!(save(&link.join("nested"),&Preferences::default()).is_err());
        let target = dir.path().join("target.json");
        fs::rename(root.join("preferences.json"),&target).unwrap();
        symlink(&target,root.join("preferences.json")).unwrap();
        assert!(load(&root).is_err());
        assert!(save(&root,&Preferences::default()).is_err());
        fs::remove_file(root.join("preferences.json")).unwrap();
        fs::set_permissions(&root,fs::Permissions::from_mode(0o755)).unwrap();
        assert!(load(&root).is_err());
        assert!(save(&root,&Preferences::default()).is_err());
    }
}
