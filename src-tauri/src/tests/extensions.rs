use super::*;

const ID: &str = "graphdesk.tally";

#[test]
fn m9_bundled_protocol_persists_filters_registry_and_preserves_editor_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("extensions.db");
    let mut graph = Graph::create(&path).unwrap();
    graph.extension_configure("other.extension", true).unwrap();
    graph.extension_write("other.extension", 0, json!({"private":"other ID"})).unwrap();
    drop(graph);
    let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    let opened = invoke(&window, "editor_open", json!({"path":path,"create":false})).unwrap();
    let token = opened["token"].clone();
    let request = |request: Value| invoke(&window, "extension_request", json!({"session":token,"extensionId":ID,"apiVersion":1,"request":request}));
    let configure = |enabled: bool| invoke(&window, "extension_configure", json!({"session":token,"extensionId":ID,"enabled":enabled}));
    assert_eq!(invoke(&window, "extension_state", json!({"session":token})).unwrap(),
        json!({"graph_id":opened["graph_id"],"extensions":[{"id":ID,"enabled":false}]}));
    assert!(request(json!({"op":"storage_read"})).is_err());
    assert!(request(json!({"op":"storage_write","expected_revision":0,"value":3})).is_err());
    for id in ["other.extension", "Graphdesk.tally", "graphdesk.unknown", "../graphdesk.tally"] {
        assert!(invoke(&window, "extension_configure", json!({"session":token,"extensionId":id,"enabled":true})).is_err());
        assert!(invoke(&window, "extension_request", json!({"session":token,"extensionId":id,"apiVersion":1,"request":{"op":"storage_read"}})).is_err());
    }
    configure(true).unwrap();
    assert_eq!(request(json!({"op":"storage_read"})).unwrap(), json!({"revision":0,"value":null}));
    for version in [json!(0), json!(2), json!(u32::MAX), json!(-1), json!(1.5), json!("1"), json!(4294967296_u64)] {
        assert!(invoke(&window, "extension_request", json!({"session":token,"extensionId":ID,"apiVersion":version,"request":{"op":"storage_read"}})).is_err());
    }
    // Unknown operations and arbitrary SQL/filesystem payloads never reach storage.
    for request in [json!({"op":"sql","sql":"DELETE FROM pages"}), json!({"op":"storage_read","path":"arbitrary"}),
        json!({"op":"storage_write","expected_revision":0,"value":null,"sql":"DELETE FROM pages"})] {
        assert!(invoke(&window, "extension_request", json!({"session":token,"extensionId":ID,"apiVersion":1,"request":request})).is_err());
    }
    let page = invoke(&window, "editor_create_page", json!({"session":token,"title":"Undo retained"})).unwrap();
    let appended = invoke(&window, "editor_apply", json!({"session":token,"pageId":page["id"],"action":{"kind":"append"}})).unwrap();
    let block = appended["changed"][0]["id"].clone();
    invoke(&window, "editor_apply", json!({"session":token,"pageId":page["id"],"action":{"kind":"save","id":block,"expected":"","content":"retained"}})).unwrap();
    let saved = request(json!({"op":"storage_write","expected_revision":0,"value":{"count":7,"label":"سلام 😀"}})).unwrap();
    assert_eq!(saved, json!({"revision":1,"value":{"count":7,"label":"سلام 😀"}}));
    assert_eq!(request(json!({"op":"storage_write","expected_revision":0,"value":99})).unwrap_err()["code"], "conflict");
    assert_eq!(request(json!({"op":"storage_write","expected_revision":1,"value":"x".repeat(16383)})).unwrap_err()["code"], "invalid");
    assert_eq!(request(json!({"op":"storage_read"})).unwrap(), saved);
    configure(false).unwrap();
    assert!(request(json!({"op":"storage_read"})).is_err());
    assert!(request(json!({"op":"storage_write","expected_revision":1,"value":99})).is_err());
    let undone = invoke(&window, "editor_history", json!({"session":token,"pageId":page["id"],"redo":false})).unwrap();
    assert_eq!(undone["changed"][0]["content"], "");
    configure(true).unwrap();
    invoke(&window, "editor_history", json!({"session":token,"pageId":page["id"],"redo":true})).unwrap();
    assert_eq!(request(json!({"op":"storage_read"})).unwrap(), saved);
    // Reopening the same graph preserves enabled preference and data, not token.
    let reopened = invoke(&window, "editor_open", json!({"path":path,"create":false})).unwrap();
    assert_ne!(reopened["token"], token);
    assert_eq!(invoke(&window, "extension_state", json!({"session":reopened["token"]})).unwrap(),
        json!({"graph_id":opened["graph_id"],"extensions":[{"id":ID,"enabled":true}]}));
    assert_eq!(invoke(&window, "extension_request", json!({"session":reopened["token"],"extensionId":ID,"apiVersion":1,"request":{"op":"storage_read"}})).unwrap(), saved);
    assert_eq!(request(json!({"op":"storage_write","expected_revision":1,"value":99})).unwrap_err()["code"], "stale_session");
    let retained = Graph::open(&path).unwrap();
    assert_eq!(retained.extension_read("other.extension").unwrap().value, json!({"private":"other ID"}));
}

#[test]
fn m9_all_extension_commands_reject_stale_sessions_and_switches_are_graph_scoped() {
    let dir = tempfile::tempdir().unwrap();
    let first_path = dir.path().join("first.db");
    let second_path = dir.path().join("second.db");
    let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    let main = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    let commands = |token: Value| [
        ("extension_state", json!({"session":token})),
        ("extension_configure", json!({"session":token,"extensionId":ID,"enabled":true})),
        ("extension_request", json!({"session":token,"extensionId":ID,"apiVersion":1,"request":{"op":"storage_read"}})),
        ("extension_request", json!({"session":token,"extensionId":ID,"apiVersion":1,"request":{"op":"storage_write","expected_revision":0,"value":42}})),
    ];
    for (command, args) in commands(json!(Uuid::new_v4())) {
        assert_eq!(invoke(&main, command, args).unwrap_err()["code"], "stale_session");
    }
    let first = invoke(&main, "editor_open", json!({"path":first_path,"create":true})).unwrap();
    invoke(&main, "extension_configure", json!({"session":first["token"],"extensionId":ID,"enabled":true})).unwrap();
    invoke(&main, "extension_request", json!({"session":first["token"],"extensionId":ID,"apiVersion":1,"request":{"op":"storage_write","expected_revision":0,"value":7}})).unwrap();
    let second = invoke(&main, "editor_open", json!({"path":second_path,"create":true})).unwrap();
    assert_ne!(first["graph_id"], second["graph_id"]);
    for (command, args) in commands(first["token"].clone()) {
        assert_eq!(invoke(&main, command, args).unwrap_err()["code"], "stale_session");
    }
    assert_eq!(invoke(&main, "extension_state", json!({"session":second["token"]})).unwrap()["extensions"], json!([{"id":ID,"enabled":false}]));
    invoke(&main, "extension_configure", json!({"session":second["token"],"extensionId":ID,"enabled":true})).unwrap();
    assert_eq!(invoke(&main, "extension_request", json!({"session":second["token"],"extensionId":ID,"apiVersion":1,"request":{"op":"storage_read"}})).unwrap(), json!({"revision":0,"value":null}));
    // Valid payloads are still ACL-denied from non-main windows and remote URLs.
    let untrusted = tauri::WebviewWindowBuilder::new(&app, "untrusted", Default::default()).build().unwrap();
    for (command, args) in commands(second["token"].clone()) {
        for (window, url) in [(&untrusted, "tauri://localhost"), (&main, "https://example.invalid")] {
            let error = invoke_from(window, command, args.clone(), url).unwrap_err();
            assert!(error.as_str().unwrap().contains("not allowed"), "{error}");
        }
    }
    assert_eq!(Graph::open(first_path).unwrap().extension_read(ID).unwrap().value, json!(7));
    assert_eq!(Graph::open(second_path).unwrap().extension_read(ID).unwrap().revision, 0);
}
