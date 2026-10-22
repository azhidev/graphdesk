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
    for command in ["open_graph", "create_page", "create_journal", "list_blocks", "create_block", "editor_open", "editor_page", "editor_apply", "editor_history", "editor_create_page"] {
        let error = invoke(&window, command, json!({})).unwrap_err();
        assert!(error.as_str().unwrap().contains("not allowed"), "{error}");
    }
    let main = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    for command in ["open_graph", "create_page", "create_journal", "list_blocks", "create_block", "editor_open", "editor_page", "editor_apply", "editor_history", "editor_create_page"] {
        let error = invoke_from(&main, command, json!({}), "https://example.invalid").unwrap_err();
        assert!(error.as_str().unwrap().contains("not allowed"), "{error}");
    }
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
    let apply = |action: Value| invoke(&window, "editor_apply", json!({"session":session,"pageId":page,"action":action,"group":null}));
    let id = apply(json!({"kind":"append"})).unwrap()["focus_id"].clone();
    assert_eq!(apply(json!({"kind":"save","id":id,"expected":"wrong","content":"lost"})).unwrap_err()["code"], "conflict");
    apply(json!({"kind":"save","id":id,"expected":"","content":"سلام😀دنیا"})).unwrap();
    assert_eq!(apply(json!({"kind":"split","id":id,"start":5,"end":5})).unwrap_err()["code"], "invalid");
    let second = apply(json!({"kind":"split","id":id,"start":4,"end":4})).unwrap()["focus_id"].clone();
    apply(json!({"kind":"indent","id":second})).unwrap();
    let outline = invoke(&window,"editor_page",json!({"session":session,"pageId":page})).unwrap();
    assert_eq!(outline["blocks"].as_array().unwrap().len(),2);
    assert!(outline["blocks"].as_array().unwrap().iter().any(|b| b["id"]==second && b["parent_id"]==id));
    let undone = invoke(&window,"editor_history",json!({"session":session,"pageId":page,"redo":false})).unwrap();
    assert_eq!(undone["can_redo"],true);
    invoke(&window,"editor_history",json!({"session":session,"pageId":page,"redo":true})).unwrap();
    assert!(invoke(&window,"editor_open",json!({"path":path,"create":true})).is_err());
    assert!(invoke(&window,"editor_page",json!({"session":session,"pageId":page})).is_ok());
    invoke(&window,"editor_open",json!({"path":other,"create":true})).unwrap();
    assert_eq!(apply(json!({"kind":"save","id":id,"expected":"سلام","content":"wrong graph"})).unwrap_err()["code"],"stale_session");
    assert_eq!(invoke(&window,"editor_history",json!({"session":session,"pageId":page,"redo":false})).unwrap_err()["code"],"stale_session");
    assert!(invoke(&window,"editor_apply",json!({"session":"invalid","pageId":page,"action":{"kind":"append"}})).is_err());
}
