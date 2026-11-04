//! Real command dispatch with Tauri's mock runtime, not desktop/WebView validation.
use super::*;
use serde_json::{json, Value};
use tauri::{ipc::{CallbackFn, InvokeBody}, test::{get_ipc_response, mock_builder, MockRuntime, INVOKE_KEY}, webview::InvokeRequest};

fn invoke(window: &tauri::WebviewWindow<MockRuntime>, command: &str, args: Value) -> Result<Value, Value> {
    invoke_from(window, command, args, "tauri://localhost")
}

fn invoke_from(window: &tauri::WebviewWindow<MockRuntime>, command: &str, args: Value, url: &str) -> Result<Value, Value> {
    get_ipc_response(window, InvokeRequest {
        cmd: command.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: url.parse().unwrap(),
        body: InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.into(),
    }).map(|body| body.deserialize::<Value>().unwrap())
}

#[test]
fn m6_registry_staged_switch_cancel_reconcile_and_failures_ipc() {
    use tauri::Manager;
    let dir = tempfile::tempdir().unwrap();
    let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    let config = dir.path().join("config");
    app.state::<crate::registry::RegistryState>().0.lock().unwrap().replace(config.clone());
    app.state::<crate::recovery::RecoveryState>().0.lock().unwrap().root = Some(dir.path().join("recovery"));
    let window = tauri::WebviewWindowBuilder::new(&app,"main",Default::default()).build().unwrap();
    let defaults = json!({"remember":false,"reopen":false,"entries":[],"warning":false});
    assert_eq!(invoke(&window,"graph_registry_load",json!({})).unwrap(),defaults);
    assert!(!config.exists());
    let path = dir.path().join("گراف اول 😀.db");
    let prepared = invoke(&window,"graph_prepare",json!({"path":path,"create":true,"sourceSession":null,"expectedGraph":null,"automatic":false})).unwrap();
    assert_eq!(invoke(&window,"graph_reconcile",json!({"ticket":prepared["ticket"]})).unwrap(),Value::Null);
    assert!(app.state::<AppState>().0.lock().unwrap().is_none());
    let first = invoke(&window,"graph_activate",json!({"ticket":prepared["ticket"],"sourceSession":null})).unwrap();
    assert_eq!(first["graph_id"],prepared["graph_id"]);
    assert_eq!(first["path"],json!(path.canonicalize().unwrap()));
    // Lost activation acknowledgement: re-request only the still-current receipt.
    for _ in 0..2 { assert_eq!(invoke(&window,"graph_reconcile",json!({"ticket":prepared["ticket"]})).unwrap(),first); }
    assert!(invoke(&window,"graph_activate",json!({"ticket":prepared["ticket"],"sourceSession":null})).is_err());
    assert_eq!(invoke(&window,"graph_registry_record",json!({"session":first["token"]})).unwrap(),defaults);
    assert!(!config.exists());
    let enabled = invoke(&window,"graph_registry_settings",json!({"remember":true,"reopen":true})).unwrap();
    assert_eq!(enabled["entries"],json!([]));
    let recorded = invoke(&window,"graph_registry_record",json!({"session":first["token"]})).unwrap();
    assert_eq!(recorded["entries"],json!([{"path":first["path"],"graph_id":first["graph_id"]}]));
    assert_eq!(invoke(&window,"graph_registry_record",json!({"session":Uuid::new_v4()})).is_err(),true);
    let page = invoke(&window,"editor_create_page",json!({"session":first["token"],"title":"source retained"})).unwrap();
    let before_failure = std::fs::read(config.join("graph-state.json")).unwrap();
    for args in [json!({"path":dir.path().join("missing.db"),"create":false,"sourceSession":first["token"],"expectedGraph":null,"automatic":false}),
        json!({"path":path,"create":false,"sourceSession":first["token"],"expectedGraph":Uuid::new_v4(),"automatic":true})] {
        assert!(invoke(&window,"graph_prepare",args).is_err());
        assert!(invoke(&window,"editor_page",json!({"session":first["token"],"pageId":page["id"]})).is_ok());
    }
    assert!(!dir.path().join("missing.db").exists());
    assert_eq!(std::fs::read(config.join("graph-state.json")).unwrap(), before_failure);
    let second_path = dir.path().join("second.db");
    let candidate = invoke(&window,"graph_prepare",json!({"path":second_path,"create":true,"sourceSession":first["token"],"expectedGraph":null,"automatic":false})).unwrap();
    // A stale cancellation must not release another candidate.
    invoke(&window,"graph_cancel",json!({"ticket":Uuid::new_v4()})).unwrap();
    invoke(&window,"graph_cancel",json!({"ticket":candidate["ticket"]})).unwrap();
    assert!(invoke(&window,"graph_activate",json!({"ticket":candidate["ticket"],"sourceSession":first["token"]})).is_err());
    assert!(second_path.exists()); // Cancel releases the handle; never deletes a newly created graph.
    let candidate = invoke(&window,"graph_prepare",json!({"path":second_path,"create":false,"sourceSession":first["token"],"expectedGraph":null,"automatic":false})).unwrap();
    let second = invoke(&window,"graph_activate",json!({"ticket":candidate["ticket"],"sourceSession":first["token"]})).unwrap();
    assert_eq!(invoke(&window,"graph_reconcile",json!({"ticket":prepared["ticket"]})).unwrap(),Value::Null);
    assert_eq!(invoke(&window,"graph_reconcile",json!({"ticket":candidate["ticket"]})).unwrap(),second);
    // Successful activation is independent of a failed registry write.
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&config,std::fs::Permissions::from_mode(0o500)).unwrap();
        assert!(invoke(&window,"graph_registry_record",json!({"session":second["token"]})).is_err());
        assert_eq!(invoke(&window,"graph_reconcile",json!({"ticket":candidate["ticket"]})).unwrap(),second);
        std::fs::set_permissions(&config,std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let entries = invoke(&window,"graph_registry_record",json!({"session":second["token"]})).unwrap();
    assert_eq!(entries["entries"].as_array().unwrap().len(),2);
    assert_eq!(entries["entries"][0]["path"],second["path"]);
    // Registry never scans graph paths or opens missing files while loading.
    let forgotten = invoke(&window,"graph_registry_forget",json!({"path":first["path"]})).unwrap();
    assert_eq!(forgotten["entries"].as_array().unwrap().len(),1);
    assert_eq!(invoke(&window,"graph_registry_forget",json!({"path":null})).unwrap()["entries"],json!([]));
    assert_eq!(invoke(&window,"graph_registry_settings",json!({"remember":false,"reopen":true})).unwrap(),defaults);
    let unknown = br#"{"version":99,"remember":true,"reopen":true,"entries":[]}"#;
    std::fs::write(config.join("graph-state.json"),unknown).unwrap();
    assert_eq!(invoke(&window,"graph_registry_load",json!({})).unwrap()["warning"],true);
    assert_eq!(invoke(&window,"graph_registry_record",json!({"session":second["token"]})).unwrap()["warning"],true);
    assert!(invoke(&window,"graph_registry_forget",json!({"path":second["path"]})).is_err());
    assert_eq!(std::fs::read(config.join("graph-state.json")).unwrap(),unknown);
    assert_eq!(invoke(&window,"graph_registry_forget",json!({"path":null})).unwrap(),defaults);
    std::fs::write(config.join("graph-state.json"),b"broken").unwrap();
    assert_eq!(invoke(&window,"graph_registry_settings",json!({"remember":true,"reopen":false})).unwrap()["warning"],false);
    // Any legacy open invalidates the activation receipt and pending source.
    let pending = invoke(&window,"graph_prepare",json!({"path":path,"create":false,"sourceSession":second["token"],"expectedGraph":first["graph_id"],"automatic":true})).unwrap();
    let reopened = invoke(&window,"editor_open",json!({"path":path,"create":false})).unwrap();
    assert_eq!(invoke(&window,"graph_reconcile",json!({"ticket":candidate["ticket"]})).unwrap(),Value::Null);
    assert_eq!(invoke(&window,"graph_activate",json!({"ticket":pending["ticket"],"sourceSession":second["token"]})).unwrap_err()["code"],"stale_session");
    assert_eq!(invoke(&window,"recovery_notes",json!({"session":second["token"]})).unwrap_err()["code"],"stale_session");
    assert_eq!(invoke(&window,"recovery_notes",json!({"session":reopened["token"]})).unwrap(),json!([]));
}

#[test]
fn m8_first_run_seeds_one_demo_graph_and_never_reseeds() {
    use tauri::Manager;
    let dir = tempfile::tempdir().unwrap();
    let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    app.state::<crate::registry::RegistryState>().0.lock().unwrap().replace(dir.path().join("config"));
    app.state::<crate::startup::StartupState>().0.lock().unwrap().replace(dir.path().join("graphs"));
    app.state::<crate::recovery::RecoveryState>().0.lock().unwrap().root = Some(dir.path().join("recovery"));
    let window = tauri::WebviewWindowBuilder::new(&app,"main",Default::default()).build().unwrap();

    let first = invoke(&window,"graph_first_run",json!({"date":"2024-02-29"})).unwrap();
    assert_eq!(first["fresh"], json!(true));
    assert_eq!(first["warning"], json!(false));
    assert_eq!(first["registry"]["remember"], json!(true));
    assert_eq!(first["registry"]["reopen"], json!(true));
    let demo = first["demo"].clone();
    assert!(demo["path"].as_str().unwrap().ends_with("GraphDesk Demo.sqlite3"));
    assert_eq!(first["registry"]["entries"], json!([])); // not active yet

    // A second launch is an existing installation: no new demo, no duplication.
    let second = invoke(&window,"graph_first_run",json!({"date":"2024-03-01"})).unwrap();
    assert_eq!(second["fresh"], json!(false));
    assert_eq!(second["demo"], Value::Null);
    assert_eq!(second["registry"], first["registry"]);

    // The seeded graph opens through the ordinary staged pipeline.
    let prepared = invoke(&window,"graph_prepare",json!({"path":demo["path"],"create":false,"sourceSession":null,"expectedGraph":demo["graph_id"],"automatic":false})).unwrap();
    let active = invoke(&window,"graph_activate",json!({"ticket":prepared["ticket"],"sourceSession":null})).unwrap();
    let recorded = invoke(&window,"graph_registry_record",json!({"session":active["token"]})).unwrap();
    assert_eq!(recorded["entries"], json!([demo.clone()]));
    assert_eq!(invoke(&window,"graph_first_run",json!({"date":"2024-03-02"})).unwrap()["registry"], recorded);
    let titles: Vec<String> = active["pages"].as_array().unwrap().iter().map(|p| p["title"].as_str().unwrap().to_owned()).collect();
    for expected in ["Welcome", "Getting Started", "Tasks", "2024-02-29"] {
        assert!(titles.iter().any(|t| t == expected), "{expected}");
    }
    let sqlite_files = std::fs::read_dir(dir.path().join("graphs")).unwrap()
        .filter(|entry| entry.as_ref().unwrap().path().extension().and_then(|e| e.to_str()) == Some("sqlite3")).count();
    assert_eq!(sqlite_files, 1);

    // A distinct app instance loads the persisted last-active descriptor and
    // can reopen through the same identity-checked automatic path as the UI.
    let restarted = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    restarted.state::<crate::registry::RegistryState>().0.lock().unwrap().replace(dir.path().join("config"));
    restarted.state::<crate::startup::StartupState>().0.lock().unwrap().replace(dir.path().join("graphs"));
    restarted.state::<crate::recovery::RecoveryState>().0.lock().unwrap().root = Some(dir.path().join("recovery"));
    let window = tauri::WebviewWindowBuilder::new(&restarted,"main",Default::default()).build().unwrap();
    let startup = invoke(&window,"graph_first_run",json!({"date":"2024-03-03"})).unwrap();
    assert_eq!(startup["fresh"], false);
    let last = &startup["registry"]["entries"][0];
    let prepared = invoke(&window,"graph_prepare",json!({"path":last["path"],"create":false,"sourceSession":null,"expectedGraph":last["graph_id"],"automatic":true})).unwrap();
    let reopened = invoke(&window,"graph_activate",json!({"ticket":prepared["ticket"],"sourceSession":null})).unwrap();
    assert_eq!(reopened["graph_id"], active["graph_id"]);
    assert_eq!(reopened["pages"], active["pages"]);
}

#[test]
fn m8_startup_preserves_preferences_recovery_and_existing_demo_files() {
    use tauri::Manager;
    for evidence in ["preferences", "recovery", "demo", "marker"] {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let graphs = dir.path().join("graphs");
        let recovery = dir.path().join("recovery");
        let (path, bytes) = match evidence {
            "preferences" => (config.join("preferences.json"), br#"{"language":"fa","calendar":"jalali","search_mode":"persian","theme":"dark"}"#.to_vec()),
            "marker" => (config.join("first-run.json"), br#"{"version":1,"demo_path":null}"#.to_vec()),
            "recovery" => (recovery.join("retained.sqlite3"), b"untouched recovery fixture".to_vec()),
            _ => (graphs.join("GraphDesk Demo.sqlite3"), b"never replace an existing database".to_vec()),
        };
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path.parent().unwrap(), std::fs::Permissions::from_mode(0o700)).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
        app.state::<crate::registry::RegistryState>().0.lock().unwrap().replace(config);
        app.state::<crate::startup::StartupState>().0.lock().unwrap().replace(graphs.clone());
        app.state::<crate::recovery::RecoveryState>().0.lock().unwrap().root = Some(recovery);
        let window = tauri::WebviewWindowBuilder::new(&app,"main",Default::default()).build().unwrap();
        let started = invoke(&window,"graph_first_run",json!({"date":"2024-02-29"})).unwrap();
        assert_eq!(started["demo"], Value::Null, "{evidence}");
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "{evidence}");
        if evidence != "demo" {
            assert_eq!(started["fresh"], false, "{evidence}");
            assert!(!graphs.exists(), "{evidence}");
        } else { assert_eq!(started["warning"], true); }
    }
}

#[test]
fn m8_existing_optout_and_corrupt_registries_are_never_seeded() {
    use tauri::Manager;
    let cases: [(&str, Vec<u8>); 2] = [
        ("opt-out", serde_json::to_vec(&crate::registry::Registry::default()).unwrap()),
        ("future", br#"{"version":99,"remember":true,"reopen":true,"entries":[]}"#.to_vec()),
    ];
    for (label, content) in cases {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join("graph-state.json"), &content).unwrap();
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o700)).unwrap();
            std::fs::set_permissions(config.join("graph-state.json"), std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
        app.state::<crate::registry::RegistryState>().0.lock().unwrap().replace(config.clone());
        app.state::<crate::startup::StartupState>().0.lock().unwrap().replace(dir.path().join("graphs"));
        app.state::<crate::recovery::RecoveryState>().0.lock().unwrap().root = Some(dir.path().join("recovery"));
        let window = tauri::WebviewWindowBuilder::new(&app,"main",Default::default()).build().unwrap();
        let started = invoke(&window,"graph_first_run",json!({"date":"2024-02-29"})).unwrap();
        assert_eq!(started["fresh"], json!(false), "{label}");
        assert_eq!(started["demo"], Value::Null, "{label}");
        assert!(!dir.path().join("graphs").exists(), "{label}");
        // Invalid registry content is preserved byte-for-byte; the valid opt-out
        // is returned unchanged and no first-run marker is written.
        assert_eq!(std::fs::read(config.join("graph-state.json")).unwrap(), content, "{label}");
        assert!(!config.join("first-run.json").exists(), "{label}");
        if label == "opt-out" { assert_eq!(started["registry"]["remember"], json!(false)); }
        else { assert_eq!(started["warning"], json!(true)); }
    }
}

#[test]
fn native_dialog_plugin_has_no_frontend_permission_even_in_main() {
    let app = app_builder(mock_builder().plugin(tauri_plugin_dialog::init())).build(tauri::generate_context!()).unwrap();
    let main = tauri::WebviewWindowBuilder::new(&app,"main",Default::default()).build().unwrap();
    let untrusted = tauri::WebviewWindowBuilder::new(&app,"untrusted",Default::default()).build().unwrap();
    for command in ["plugin:dialog|open","plugin:dialog|save","plugin:dialog|message", "plugin:fs|read_file", "plugin:fs|write_file"] {
        for (window,url) in [(&main,"tauri://localhost"),(&untrusted,"tauri://localhost"),(&main,"https://example.invalid")] {
            let error = invoke_from(window,command,json!({}),url).unwrap_err();
            assert!(error.as_str().unwrap().contains("not allowed"),"{command}: {error}");
        }
    }
}

#[test]
fn m5_notes_search_preferences_and_stale_sessions_ipc() {
    use tauri::Manager;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("m5.db");
    let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    app.state::<crate::recovery::RecoveryState>().0.lock().unwrap().root = Some(directory.path().join("recovery"));
    app.state::<crate::preferences::PreferencesState>().0.lock().unwrap().replace(directory.path().join("config"));
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    let defaults = json!({"language":"en","calendar":"gregorian","search_mode":"original","theme":"system"});
    assert_eq!(invoke(&window,"preferences_load",json!({})).unwrap(), json!({"preferences":defaults,"warning":false}));
    let legacy = json!({"language":"fa","calendar":"jalali","search_mode":"persian"});
    invoke(&window,"preferences_save",json!({"preferences":legacy})).unwrap();
    assert_eq!(invoke(&window,"preferences_load",json!({})).unwrap()["preferences"],json!({"language":"fa","calendar":"jalali","search_mode":"persian","theme":"system"}));
    let prefs = json!({"language":"fa","calendar":"jalali","search_mode":"persian","theme":"dark"});
    invoke(&window,"preferences_save",json!({"preferences":prefs})).unwrap();
    assert_eq!(invoke(&window,"preferences_load",json!({})).unwrap(), json!({"preferences":prefs,"warning":false}));
    assert!(invoke(&window,"preferences_save",json!({"preferences":{"language":"unknown","calendar":"gregorian","search_mode":"original"}})).is_err());
    assert_eq!(invoke(&window,"preferences_load",json!({})).unwrap()["preferences"], prefs);
    std::fs::write(directory.path().join("config/preferences.json"),b"malformed").unwrap();
    assert_eq!(invoke(&window,"preferences_load",json!({})).unwrap(),json!({"preferences":defaults,"warning":true}));
    invoke(&window,"preferences_save",json!({"preferences":prefs})).unwrap();
    let session = invoke(&window,"editor_open",json!({"path":path,"create":true})).unwrap();
    let token = &session["token"];
    let id = Uuid::new_v4();
    let block = Uuid::new_v4();
    let raw = "كتاب می\u{200c}روم ۱۲٣ 😀";
    let capture_args = json!({"session":token,"id":id,"blockId":block,"date":"2024-02-29","content":raw});
    let first = invoke(&window,"recovery_note_put",capture_args.clone()).unwrap();
    assert_eq!(first["graph_id"], session["graph_id"]);
    assert_eq!(first["journal_date"], "2024-02-29");
    assert_eq!(first["block_id"], block.to_string());
    assert_eq!(invoke(&window,"recovery_inventory",json!({})).unwrap(), 1);
    let newer = invoke(&window,"recovery_note_put",capture_args.clone()).unwrap();
    assert!(newer["version"].as_i64().unwrap() > first["version"].as_i64().unwrap());
    assert_eq!(invoke(&window,"recovery_note_remove",json!({"session":token,"id":id,"version":first["version"]})).unwrap_err()["code"], "conflict");
    assert_eq!(invoke(&window,"editor_note_state",json!({"session":token,"blockId":block})).unwrap(),json!({"block":null,"revision":null,"deleted":false}));
    let submit_args = json!({"session":token,"date":"2024-02-29","blockId":block,"content":raw});
    let submitted = invoke(&window,"editor_submit_note",submit_args.clone()).unwrap();
    assert_eq!(submitted["block"]["id"], block.to_string());
    assert_eq!(submitted["revision"], 0);
    assert_eq!(invoke(&window,"editor_submit_note",submit_args.clone()).unwrap(),submitted);
    let page = &submitted["page"]["id"];
    let hits = invoke(&window,"editor_search",json!({"session":token,"text":"کتاب می روم 123","persian":true})).unwrap();
    assert_eq!(hits[0]["text"],raw);
    assert_eq!(invoke(&window,"editor_search",json!({"session":token,"text":"کتاب"})).unwrap(),json!([]));
    assert_eq!(invoke(&window,"editor_search",json!({"session":token,"text":"کتاب","persian":false})).unwrap(),json!([]));
    let outline = invoke(&window,"editor_page",json!({"session":token,"pageId":page})).unwrap();
    assert_eq!(outline["blocks"].as_array().unwrap().len(),1);
    assert_eq!(outline["can_undo"],true);
    invoke(&window,"editor_history",json!({"session":token,"pageId":page,"redo":false})).unwrap();
    assert_eq!(invoke(&window,"editor_note_state",json!({"session":token,"blockId":block})).unwrap(),json!({"block":null,"revision":1,"deleted":true}));
    assert_eq!(invoke(&window,"editor_submit_note",submit_args.clone()).unwrap_err()["code"], "conflict");
    // Recovery is independent of graph state and persists after canonical undo.
    assert_eq!(invoke(&window,"recovery_notes",json!({"session":token})).unwrap()[0]["id"], id.to_string());
    let reopened = invoke(&window,"editor_open",json!({"path":path,"create":false})).unwrap();
    for (command, args) in [
        ("editor_submit_note", submit_args),
        ("editor_note_state", json!({"session":token,"blockId":block})),
        ("recovery_note_put", capture_args),
        ("recovery_notes", json!({"session":token})),
        ("recovery_note_remove", json!({"session":token,"id":id,"version":newer["version"]})),
    ] {
        assert_eq!(invoke(&window,command,args).unwrap_err()["code"],"stale_session", "{command}");
    }
    assert_eq!(invoke(&window,"preferences_load",json!({})).unwrap()["preferences"],prefs);
    invoke(&window,"recovery_note_remove",json!({"session":reopened["token"],"id":id,"version":newer["version"]})).unwrap();
    assert_eq!(invoke(&window,"recovery_inventory",json!({})).unwrap(),0);
    invoke(&window,"open_graph",json!({"path":path,"create":false})).unwrap();
    assert_eq!(invoke(&window,"recovery_notes",json!({"session":reopened["token"]})).unwrap_err()["code"], "stale_session");
}

#[test]
fn pending_note_capture_does_not_wait_for_graph_mutex() {
    use tauri::Manager;
    let directory = tempfile::tempdir().unwrap();
    let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    app.state::<crate::recovery::RecoveryState>().0.lock().unwrap().root = Some(directory.path().join("recovery"));
    let window = tauri::WebviewWindowBuilder::new(&app,"main",Default::default()).build().unwrap();
    let session = invoke(&window,"editor_open",json!({"path":directory.path().join("g.db"),"create":true})).unwrap();
    let state = app.state::<AppState>();
    let _held = state.0.lock().unwrap();
    assert!(invoke(&window,"recovery_note_put",json!({"session":session["token"],"id":Uuid::new_v4(),"blockId":Uuid::new_v4(),"date":"2024-02-29","content":"independent"})).is_ok());
    assert!(invoke(&window,"recovery_note_put",json!({"session":session["token"],"id":Uuid::new_v4(),"blockId":Uuid::new_v4(),"date":"2023-02-29","content":"invalid"})).is_err());
}

#[test]
fn commands_roundtrip_and_preserve_state_on_errors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("graph.sqlite3");
    let other = directory.path().join("other.sqlite3");
    let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();

    assert_eq!(invoke(&window, "create_page", json!({"title": "Before open"})), Err(json!("open a graph first")));
    assert_eq!(invoke(&window, "open_graph", json!({"path": path, "create": true})).unwrap(), json!([]));
    let page = invoke(&window, "create_page", json!({"title": " Notes "})).unwrap();
    assert_eq!(page["title"], "Notes");
    assert!(Uuid::parse_str(page["id"].as_str().unwrap()).is_ok());
    let block = invoke(&window, "create_block", json!({"pageId": page["id"], "parentId": null, "content": "سلام دنیا"})).unwrap();
    assert_eq!(block["page_id"], page["id"]);
    assert_eq!(block["position"], 0);
    let child = invoke(&window, "create_block", json!({"pageId": page["id"], "parentId": block["id"], "content": "child"})).unwrap();
    assert_eq!(invoke(&window, "list_blocks", json!({"pageId": page["id"], "parentId": block["id"]})).unwrap(), json!([child]));
    assert_eq!(invoke(&window, "list_blocks", json!({"pageId": page["id"], "parentId": null})).unwrap(), json!([block]));
    assert!(invoke(&window, "list_blocks", json!({"pageId": "invalid", "parentId": null})).is_err());
    assert!(invoke(&window, "create_page", json!({"title": "notes"})).is_err());
    assert!(invoke(&window, "create_page", json!({})).is_err());
    assert!(invoke(&window, "create_journal", json!({"date": "2023-02-29"})).is_err());
    let journal = invoke(&window, "create_journal", json!({"date": "2024-02-29"})).unwrap();
    assert_eq!(invoke(&window, "create_journal", json!({"date": "2024-02-29"})).unwrap(), journal);

    // Failed open/create must not replace the current graph or overwrite its file.
    assert!(invoke(&window, "open_graph", json!({"path": path, "create": true})).is_err());
    assert!(invoke(&window, "open_graph", json!({"path": other, "create": false})).is_err());
    assert_eq!(invoke(&window, "list_blocks", json!({"pageId": page["id"], "parentId": null})).unwrap(), json!([block]));
    assert_eq!(invoke(&window, "open_graph", json!({"path": other, "create": true})).unwrap(), json!([]));
    let reopened = invoke(&window, "open_graph", json!({"path": path, "create": false})).unwrap();
    assert_eq!(reopened.as_array().unwrap().len(), 2);
    assert!(reopened.as_array().unwrap().contains(&page));
    assert!(reopened.as_array().unwrap().contains(&journal));
    assert_eq!(invoke(&window, "list_blocks", json!({"pageId": page["id"], "parentId": null})).unwrap(), json!([block]));
}

#[test]
fn graph_commands_are_denied_outside_main_window() {
    let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "untrusted", Default::default()).build().unwrap();
    // Read the explicit ACL so every newly allowed command joins both denial checks.
    let acl = include_str!("../permissions/graph.toml");
    let commands: Vec<&str> = acl.lines().find(|l| l.starts_with("commands.allow")).unwrap()
        .split('"').enumerate().filter_map(|(i, s)| (i % 2 == 1).then_some(s)).collect();
    for command in &commands {
        let error = invoke(&window, command, json!({})).unwrap_err();
        assert!(error.as_str().unwrap().contains("not allowed"), "{error}");
    }
    let main = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    for command in &commands {
        let error = invoke_from(&main, command, json!({}), "https://example.invalid").unwrap_err();
        assert!(error.as_str().unwrap().contains("not allowed"), "{error}");
    }
}

#[test]
fn journal_lookup_ipc_never_creates_and_returns_metadata_after_rename_and_reopen() {
    use tauri::Manager;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("journal-lookup.sqlite3");
    let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    assert_eq!(invoke(&window, "editor_find_journal", json!({"session":Uuid::new_v4(),"date":"2024-02-29"})).unwrap_err()["code"], "stale_session");
    let session = invoke(&window, "editor_open", json!({"path":path,"create":true})).unwrap()["token"].clone();
    let find = |date: &str| invoke(&window, "editor_find_journal", json!({"session":session,"date":date}));
    assert_eq!(find("2024-02-29").unwrap(), Value::Null);
    let ordinary = invoke(&window, "editor_create_page", json!({"session":session,"title":"2024-02-29"})).unwrap();
    assert_eq!(ordinary.get("journal_date"), Some(&Value::Null));
    assert_eq!(find("2024-02-29").unwrap(), Value::Null);
    for date in ["2023-02-29", "2024-2-29", "2024-02-30", "2024-02-29 ", "+10000-01-01"] {
        assert_eq!(find(date).unwrap_err()["code"], "invalid");
    }
    // Neither absent/invalid lookups nor a date-shaped ordinary page are converted.
    assert_eq!(app.state::<AppState>().0.lock().unwrap().as_ref().unwrap().editor.graph().pages().unwrap().len(), 1);
    assert!(invoke(&window, "editor_journal", json!({"session":session,"date":"2024-02-29"})).is_err());
    invoke(&window, "editor_rename_page", json!({"session":session,"pageId":ordinary["id"],"title":"Ordinary"})).unwrap();
    let journal = invoke(&window, "editor_journal", json!({"session":session,"date":"2024-02-29"})).unwrap();
    assert_eq!(journal["journal_date"], "2024-02-29");
    let renamed = invoke(&window, "editor_rename_page", json!({"session":session,"pageId":journal["id"],"title":"Leap day سلام"})).unwrap();
    assert_eq!(renamed["id"], journal["id"]);
    assert_eq!(renamed["journal_date"], journal["journal_date"]);
    assert_eq!(find("2024-02-29").unwrap(), renamed);
    assert_eq!(invoke(&window, "editor_journal", json!({"session":session,"date":"2024-02-29"})).unwrap(), renamed);
    let reused = invoke(&window, "editor_create_page", json!({"session":session,"title":"2024-02-29"})).unwrap();
    assert_eq!(reused.get("journal_date"), Some(&Value::Null));
    assert_eq!(find("2024-02-29").unwrap(), renamed);
    let reopened = invoke(&window, "editor_open", json!({"path":path,"create":false})).unwrap();
    assert!(reopened["pages"].as_array().unwrap().contains(&renamed));
    assert!(reopened["pages"].as_array().unwrap().contains(&reused));
    assert_eq!(find("2024-02-29").unwrap_err()["code"], "stale_session");
    assert_eq!(invoke(&window, "editor_find_journal", json!({"session":reopened["token"],"date":"2024-02-29"})).unwrap(), renamed);
    assert_eq!(invoke(&window, "editor_find_journal", json!({"session":reopened["token"],"date":"2024-03-01"})).unwrap(), Value::Null);
}

#[test]
fn task_query_ipc_filters_pages_paginates_and_preserves_editor_history() {
    use graph_core::{TaskHit, TaskStatus};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("task-query.sqlite3");
    let mut graph = Graph::create(&path).unwrap();
    let alpha = graph.create_page("alpha").unwrap();
    let beta = graph.create_page("Beta").unwrap();
    let journal = graph.create_journal("2024-02-29").unwrap();
    graph.rename_page(journal.id, "Zulu").unwrap();
    let root = graph.create_block(alpha.id, None, "not a task").unwrap();
    let cancelled = graph.create_block(alpha.id, None, "cancelled").unwrap();
    graph.set_task_status(cancelled.id, Some(TaskStatus::Cancelled)).unwrap();
    let mut expected = Vec::new();
    for n in 0..103 {
        let parent = (n == 0).then_some(root.id);
        let block = graph.create_block(alpha.id, parent, &format!("task {n} سلام😀")).unwrap();
        graph.set_task_status(block.id, Some(TaskStatus::Todo)).unwrap();
        expected.push(TaskHit { id: block.id, page_id: alpha.id, page_title: alpha.title.clone(), content: block.content, task_status: "todo".into() });
    }
    expected.sort_by_key(|hit| hit.id);
    let doing = graph.create_block(journal.id, None, "journal task").unwrap();
    graph.set_task_status(doing.id, Some(TaskStatus::Doing)).unwrap();
    expected.push(TaskHit { id: doing.id, page_id: journal.id, page_title: "Zulu".into(), content: doing.content, task_status: "doing".into() });
    let done = graph.create_block(beta.id, None, "completed").unwrap();
    graph.set_task_status(done.id, Some(TaskStatus::Done)).unwrap();
    drop(graph);
    let expected = serde_json::to_value(expected).unwrap();
    let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    assert_eq!(invoke(&window, "editor_tasks", json!({"session":Uuid::new_v4(),"status":"active","limit":100,"offset":0})).unwrap_err()["code"], "stale_session");
    let session = invoke(&window, "editor_open", json!({"path":path,"create":false})).unwrap()["token"].clone();
    let tasks = |status: &str, page: Option<Uuid>, limit: usize, offset: usize| invoke(&window, "editor_tasks",
        json!({"session":session,"status":status,"pageId":page,"limit":limit,"offset":offset}));
    let first = tasks("active", None, 101, 0).unwrap();
    let second = tasks("active", None, 100, 100).unwrap();
    assert_eq!(first.as_array().unwrap(), &expected.as_array().unwrap()[..100]);
    assert_eq!(second.as_array().unwrap(), &expected.as_array().unwrap()[100..]);
    assert_eq!(tasks("active", None, 100, 0).unwrap(), first);
    assert_eq!(tasks("active", None, 100, 104).unwrap(), json!([]));
    assert_eq!(tasks("active", None, 0, 0).unwrap(), json!([]));
    assert_eq!(tasks("todo", Some(alpha.id), 100, 100).unwrap().as_array().unwrap(), &expected.as_array().unwrap()[100..103]);
    assert_eq!(tasks("doing", None, 100, 0).unwrap(), json!([expected[103]]));
    assert_eq!(tasks("active", Some(journal.id), 100, 0).unwrap(), json!([expected[103]]));
    assert_eq!(tasks("doing", Some(alpha.id), 100, 0).unwrap(), json!([]));
    assert_eq!(tasks("active", Some(Uuid::new_v4()), 100, 0).unwrap(), json!([]));
    let completed = tasks("done", None, 100, 0).unwrap();
    assert_eq!(completed.as_array().unwrap().len(), 1);
    assert_eq!(completed[0]["id"], done.id.to_string());
    assert_eq!(completed[0]["task_status"], "done");
    for status in ["cancelled", "all", "Active", "bogus"] {
        assert!(tasks(status, None, 100, 0).is_err());
    }
    for (field, value) in [("pageId", json!("invalid")), ("limit", json!(-1)), ("offset", json!(-1))] {
        let mut args = json!({"session":session,"status":"active","limit":100,"offset":0});
        args[field] = value;
        assert!(invoke(&window, "editor_tasks", args).is_err());
    }
    // Page scope is optional, not an implicit restriction to the active outline.
    assert_eq!(invoke(&window, "editor_tasks", json!({"session":session,"status":"active","limit":100,"offset":100})).unwrap(), second);
    assert_eq!(invoke(&window, "editor_tasks", json!({"session":session,"status":"active","pageId":null,"offset":0})).unwrap(), first);
    let apply = |action: Value| invoke(&window, "editor_apply", json!({"session":session,"pageId":journal.id,"action":action}));
    apply(json!({"kind":"task","id":doing.id,"expected":"doing","status":"done"})).unwrap();
    assert_eq!(tasks("active", Some(journal.id), 100, 0).unwrap(), json!([]));
    assert_eq!(tasks("done", Some(journal.id), 100, 0).unwrap()[0]["id"], doing.id.to_string());
    let history = |redo: bool| invoke(&window, "editor_history", json!({"session":session,"pageId":journal.id,"redo":redo}));
    history(false).unwrap();
    assert_eq!(tasks("doing", Some(journal.id), 100, 0).unwrap(), json!([expected[103]]));
    // Read queries must retain redo and must not clear page-scoped history.
    history(true).unwrap();
    assert_eq!(tasks("active", Some(journal.id), 100, 0).unwrap(), json!([]));
    history(false).unwrap();
    apply(json!({"kind":"save","id":doing.id,"expected":"journal task","content":"updated text"})).unwrap();
    assert_eq!(tasks("doing", Some(journal.id), 100, 0).unwrap()[0]["content"], "updated text");
    history(false).unwrap();
    assert_eq!(tasks("doing", Some(journal.id), 100, 0).unwrap(), json!([expected[103]]));
    let reopened = invoke(&window, "editor_open", json!({"path":path,"create":false})).unwrap();
    assert_eq!(tasks("active", None, 100, 0).unwrap_err()["code"], "stale_session");
    assert_eq!(invoke(&window, "editor_tasks", json!({"session":reopened["token"],"status":"active","limit":100,"offset":100})).unwrap(), second);
}

#[test]
fn editor_session_transactions_and_structured_errors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("editor.sqlite3");
    let other = directory.path().join("other.sqlite3");
    let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    let session = invoke(&window, "editor_open", json!({"path":path,"create":true})).unwrap()["token"].clone();
    let page = invoke(&window, "editor_create_page", json!({"session":session,"title":"Outline"})).unwrap()["id"].clone();
    let renamed = invoke(&window, "editor_rename_page", json!({"session":session,"pageId":page,"title":"Renamed"})).unwrap();
    assert_eq!(renamed["id"], page);
    assert_eq!(renamed["title"], "Renamed");
    let journal = invoke(&window, "editor_journal", json!({"session":session,"date":"2026-10-22"})).unwrap();
    assert_eq!(invoke(&window, "editor_journal", json!({"session":session,"date":"2026-10-22"})).unwrap(), journal);
    assert!(invoke(&window, "editor_journal", json!({"session":session,"date":"2026-02-30"})).is_err());
    let apply = |action: Value| invoke(&window, "editor_apply", json!({"session":session,"pageId":page,"action":action,"group":null}));
    let id = apply(json!({"kind":"append"})).unwrap()["focus_id"].clone();
    assert_eq!(apply(json!({"kind":"save","id":id,"expected":"wrong","content":"lost"})).unwrap_err()["code"], "conflict");
    apply(json!({"kind":"save","id":id,"expected":"","content":"سلام😀دنیا"})).unwrap();
    let hits = invoke(&window,"editor_search",json!({"session":session,"text":"سلام"})).unwrap();
    assert_eq!(hits[0]["id"], id);
    assert_eq!(hits[0]["page_id"], page);
    apply(json!({"kind":"task","id":id,"expected":null,"status":"todo"})).unwrap();
    assert_eq!(apply(json!({"kind":"task","id":id,"expected":null,"status":"done"})).unwrap_err()["code"],"conflict");
    invoke(&window,"editor_history",json!({"session":session,"pageId":page,"redo":false})).unwrap();
    assert!(apply(json!({"kind":"task","id":id,"expected":null,"status":"bogus"})).is_err());
    let reference_block = apply(json!({"kind":"append"})).unwrap()["focus_id"].clone();
    apply(json!({"kind":"save","id":reference_block,"expected":"","content":"[[Renamed]] [[Missing]]"})).unwrap();
    let links = invoke(&window,"editor_links",json!({"session":session,"pageId":page})).unwrap();
    assert_eq!(links["references"].as_array().unwrap().len(),2);
    assert_eq!(links["backlinks"][0]["id"],reference_block);
    invoke(&window,"editor_history",json!({"session":session,"pageId":page,"redo":false})).unwrap();
    invoke(&window,"editor_history",json!({"session":session,"pageId":page,"redo":false})).unwrap();
    assert_eq!(apply(json!({"kind":"split","id":id,"start":5,"end":5})).unwrap_err()["code"], "invalid");
    let second = apply(json!({"kind":"split","id":id,"start":4,"end":4})).unwrap()["focus_id"].clone();
    apply(json!({"kind":"indent","id":second})).unwrap();
    let outline = invoke(&window,"editor_page",json!({"session":session,"pageId":page})).unwrap();
    assert_eq!(outline["blocks"].as_array().unwrap().len(),2);
    assert!(outline["blocks"].as_array().unwrap().iter().any(|b| b["id"]==second && b["parent_id"]==id));
    let undone = invoke(&window,"editor_history",json!({"session":session,"pageId":page,"redo":false})).unwrap();
    assert_eq!(undone["can_redo"],true);
    invoke(&window,"editor_history",json!({"session":session,"pageId":page,"redo":true})).unwrap();
    let preview = invoke(&window,"editor_prepare_delete",json!({"session":session,"pageId":page,"id":id})).unwrap();
    assert_eq!(preview["count"], 2); assert_eq!(preview["requires_confirmation"], true);
    assert_eq!(apply(json!({"kind":"delete","id":id,"token":Uuid::new_v4()})).unwrap_err()["code"], "conflict");
    let preview = invoke(&window,"editor_prepare_delete",json!({"session":session,"pageId":page,"id":id})).unwrap();
    let removed = apply(json!({"kind":"delete","id":id,"token":preview["token"]})).unwrap();
    assert_eq!(removed["removed"].as_array().unwrap().len(), 2);
    invoke(&window,"editor_history",json!({"session":session,"pageId":page,"redo":false})).unwrap();
    assert_eq!(invoke(&window,"editor_page",json!({"session":session,"pageId":page})).unwrap()["blocks"], outline["blocks"]);
    invoke(&window,"editor_history",json!({"session":session,"pageId":page,"redo":true})).unwrap();
    invoke(&window,"editor_history",json!({"session":session,"pageId":page,"redo":false})).unwrap();
    assert!(invoke(&window,"editor_open",json!({"path":path,"create":true})).is_err());
    assert!(invoke(&window,"editor_page",json!({"session":session,"pageId":page})).is_ok());
    invoke(&window,"editor_open",json!({"path":other,"create":true})).unwrap();
    assert_eq!(invoke(&window,"editor_prepare_delete",json!({"session":session,"pageId":page,"id":id})).unwrap_err()["code"],"stale_session");
    assert_eq!(invoke(&window,"editor_links",json!({"session":session,"pageId":page})).unwrap_err()["code"],"stale_session");
    assert_eq!(invoke(&window,"editor_search",json!({"session":session,"text":"سلام"})).unwrap_err()["code"],"stale_session");
    assert_eq!(invoke(&window,"editor_rename_page",json!({"session":session,"pageId":page,"title":"stale"})).unwrap_err()["code"],"stale_session");
    assert_eq!(invoke(&window,"editor_journal",json!({"session":session,"date":"2026-10-23"})).unwrap_err()["code"],"stale_session");
    assert_eq!(apply(json!({"kind":"save","id":id,"expected":"سلام","content":"wrong graph"})).unwrap_err()["code"],"stale_session");
    assert_eq!(invoke(&window,"editor_history",json!({"session":session,"pageId":page,"redo":false})).unwrap_err()["code"],"stale_session");
    assert!(invoke(&window,"editor_apply",json!({"session":"invalid","pageId":page,"action":{"kind":"append"}})).is_err());
}

#[test]
fn private_recovery_ipc_reopen_conflicts_cleanup_and_stale_sessions() {
    use tauri::Manager;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("recovery-graph.sqlite3");
    let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    app.state::<crate::recovery::RecoveryState>().0.lock().unwrap().root = Some(directory.path().join("private"));
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    let session = invoke(&window,"editor_open",json!({"path":path,"create":true})).unwrap();
    let token = &session["token"];
    let page = invoke(&window,"editor_create_page",json!({"session":token,"title":"Recovery"})).unwrap()["id"].clone();
    let block = invoke(&window,"editor_apply",json!({"session":token,"pageId":page,"action":{"kind":"append"}})).unwrap()["focus_id"].clone();
    let put = |content: &str| invoke(&window,"recovery_put",json!({"session":token,"pageId":page,"blockId":block,"base":"","revision":0,"content":content}));
    let original = put("بازیابی 😀").unwrap();
    assert_eq!(invoke(&window,"recovery_inventory",json!({})).unwrap(),1);
    let fresh = put("newer").unwrap();
    assert!(invoke(&window,"recovery_remove",json!({"session":token,"id":original["id"],"content":original["content"],"updatedAt":original["updated_at"]})).is_err());
    let apply = |expected: &str, revision: i64, content: &str| invoke(&window,"editor_apply",json!({"session":token,"pageId":page,
        "action":{"kind":"save_text","id":block,"expected":expected,"revision":revision,"content":content}}));
    assert_eq!(apply("",0,"persisted").unwrap()["revisions"][block.as_str().unwrap()],1);
    assert_eq!(apply("",0,"stale recovery").unwrap_err()["code"],"conflict");
    assert_eq!(invoke(&window,"recovery_list",json!({"session":token})).unwrap()[0]["content"],"newer");
    let reopened = invoke(&window,"editor_open",json!({"path":path,"create":false})).unwrap();
    assert_eq!(session["graph_id"], reopened["graph_id"]);
    assert!(invoke(&window,"recovery_list",json!({"session":token})).is_err());
    let token = &reopened["token"];
    assert_eq!(invoke(&window,"recovery_list",json!({"session":token})).unwrap()[0]["id"],fresh["id"]);
    invoke(&window,"editor_apply",json!({"session":token,"pageId":page,"action":{"kind":"save_text","id":block,"expected":"persisted","revision":1,"content":"newer"}})).unwrap();
    invoke(&window,"recovery_clear",json!({"session":token,"blockId":block,"content":"newer"})).unwrap();
    assert_eq!(invoke(&window,"recovery_list",json!({"session":token})).unwrap(),json!([]));
    assert_eq!(invoke(&window,"recovery_inventory",json!({})).unwrap(),0);
}
