use super::*;

const ID: &str = "graphdesk.habits";

#[test]
fn timezone_defaults_retries_updates_and_graph_independence() {
    use tauri::Manager;
    let dir = tempfile::tempdir().unwrap();
    let app = app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    app.state::<crate::preferences::PreferencesState>().0.lock().unwrap().replace(dir.path().join("config"));
    let window = tauri::WebviewWindowBuilder::new(&app,"main",Default::default()).build().unwrap();
    let set_zone = |zone: &str| invoke(&window,"preferences_save",json!({"preferences":{
        "language":"en","calendar":"gregorian","search_mode":"original","habit_timezone":zone
    }})).unwrap();
    set_zone("Asia/Tehran");
    // Introspection does not require or modify an open graph.
    assert_eq!(invoke(&window,"habits_timezone",json!({})).unwrap()["resolved"], "Asia/Tehran");
    let path = dir.path().join("zones.db");
    let opened = invoke(&window,"editor_open",json!({"path":path,"create":true})).unwrap();
    let token = &opened["token"];
    invoke(&window,"extension_configure",json!({"session":token,"extensionId":ID,"enabled":true})).unwrap();
    let request = |r:Value| invoke(&window,"habits_request",args(token,r));
    let id = Uuid::new_v4();
    let definition = json!({"name":"Walk","kind":"positive","schedule":{"anchor":0,"every":1,"unit":"days","mode":"fixed"}});
    let create = json!({"op":"create","input":{"id":id,"definition":definition}});
    let created = request(create.clone()).unwrap();
    assert_eq!(created["data"]["definition"]["schedule"]["timezone"], "Asia/Tehran");
    set_zone("America/New_York");
    assert_eq!(request(create.clone()).unwrap(), created);
    let preserved = request(json!({"op":"update","id":id,"expected_revision":0,"definition":definition})).unwrap();
    assert_eq!(preserved["data"]["definition"]["schedule"]["timezone"], "Asia/Tehran");
    let mut explicit = definition.clone();
    explicit["schedule"]["timezone"] = json!("Europe/Berlin");
    let changed = request(json!({"op":"update","id":id,"expected_revision":1,"definition":explicit})).unwrap();
    assert_eq!(changed["data"]["definition"]["schedule"]["timezone"], "Europe/Berlin");
    let next_create = json!({"op":"create","input":{"id":Uuid::new_v4(),"definition":definition}});
    let next = request(next_create.clone()).unwrap();
    assert_eq!(next["data"]["definition"]["schedule"]["timezone"], "America/New_York");
    // Explicit creates do not require readable preferences or system detection.
    std::fs::write(dir.path().join("config/preferences.json"), "invalid").unwrap();
    assert_eq!(request(next_create).unwrap(), next);
    assert_eq!(invoke(&window,"habits_timezone",json!({})).unwrap()["resolved"], Value::Null);
    assert!(request(json!({"op":"create","input":{"id":Uuid::new_v4(),"definition":explicit}})).is_ok());
    assert!(request(json!({"op":"create","input":{"id":Uuid::new_v4(),"definition":definition}})).is_err());
    let reopened = invoke(&window,"editor_open",json!({"path":path,"create":false})).unwrap();
    let retained = invoke(&window,"habits_request",args(&reopened["token"],json!({"op":"get","id":id}))).unwrap();
    assert_eq!(retained, changed);
    let second = invoke(&window,"editor_open",json!({"path":dir.path().join("other.db"),"create":true})).unwrap();
    invoke(&window,"extension_configure",json!({"session":second["token"],"extensionId":ID,"enabled":true})).unwrap();
    assert_eq!(invoke(&window,"habits_request",args(&second["token"],json!({"op":"list","limit":10}))).unwrap()["data"], json!([]));
    let untrusted = tauri::WebviewWindowBuilder::new(&app,"untrusted",Default::default()).build().unwrap();
    assert!(invoke_from(&untrusted,"habits_timezone",json!({}),"tauri://localhost").is_err());
}

fn args(token: &Value, request: Value) -> Value {
    json!({"session":token,"extensionId":ID,"apiVersion":1,"request":request})
}

#[test]
fn m10_typed_habits_receipt_cas_reopen_and_editor_history() {
    let dir=tempfile::tempdir().unwrap(); let path=dir.path().join("habits.db");
    let app=app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    let window=tauri::WebviewWindowBuilder::new(&app,"main",Default::default()).build().unwrap();
    let opened=invoke(&window,"editor_open",json!({"path":path,"create":true})).unwrap(); let token=&opened["token"];
    let request=|r:Value|invoke(&window,"habits_request",args(token,r));
    assert!(request(json!({"op":"list","limit":10})).is_err());
    invoke(&window,"extension_configure",json!({"session":token,"extensionId":ID,"enabled":true})).unwrap();
    let page=invoke(&window,"editor_create_page",json!({"session":token,"title":"Keep history"})).unwrap();
    let edit=invoke(&window,"editor_apply",json!({"session":token,"pageId":page["id"],"action":{"kind":"append"}})).unwrap();
    let block=&edit["changed"][0]["id"];
    invoke(&window,"editor_apply",json!({"session":token,"pageId":page["id"],"action":{"kind":"save","id":block,"expected":"","content":"retained"}})).unwrap();
    let id=Uuid::new_v4(); let event_id=Uuid::new_v4();
    let definition=json!({"name":"Avoid","kind":"negative","schedule":null});
    let create=json!({"op":"create","input":{"id":id,"definition":definition}});
    let created=request(create.clone()).unwrap(); assert_eq!(created["type"],"habit");
    assert_eq!(request(create).unwrap(),created);
    let input=json!({"id":event_id,"habit_id":id,"occurred_at":10}); // default negative lapse
    let event=request(json!({"op":"record","input":input})).unwrap();
    assert_eq!(event["type"],"event"); assert_eq!(event["data"]["kind"],"lapse");
    assert!(event["data"]["recorded_at"].as_i64().unwrap()>10);
    assert_eq!(request(json!({"op":"record","input":input})).unwrap(),event);
    assert!(request(json!({"op":"record","input":{"id":Uuid::new_v4(),"habit_id":id,"occurred_at":10,"recorded_at":11}})).is_err());
    let corrected=request(json!({"op":"correct","event_id":event_id,"expected_revision":0,"voided":true})).unwrap();
    assert_eq!(request(json!({"op":"record","input":input})).unwrap(),corrected);
    assert_eq!(request(json!({"op":"correct","event_id":event_id,"expected_revision":0,"voided":false})).unwrap_err()["code"],"conflict");
    assert_eq!(request(json!({"op":"status","id":id,"now":100})).unwrap()["data"]["negative"]["last_lapse"],Value::Null);
    let archived=json!({"name":"Avoid","kind":"negative","schedule":null,"archived":true});
    request(json!({"op":"update","id":id,"expected_revision":0,"definition":archived})).unwrap();
    assert_eq!(request(json!({"op":"record","input":input})).unwrap(),corrected);
    let undone=invoke(&window,"editor_history",json!({"session":token,"pageId":page["id"],"redo":false})).unwrap();
    assert_eq!(undone["changed"][0]["content"],"");
    let reopened=invoke(&window,"editor_open",json!({"path":path,"create":false})).unwrap();
    assert_eq!(request(json!({"op":"list","limit":10})).unwrap_err()["code"],"stale_session");
    let history=invoke(&window,"habits_request",args(&reopened["token"],json!({"op":"events","habit_id":id,"limit":10}))).unwrap();
    assert_eq!(history["type"],"events"); assert_eq!(history["data"],json!([corrected["data"].clone()]));
}

#[test]
fn m10_session_registry_api_graph_isolation_and_acl() {
    let dir=tempfile::tempdir().unwrap();
    let app=app_builder(mock_builder()).build(tauri::generate_context!()).unwrap();
    let main=tauri::WebviewWindowBuilder::new(&app,"main",Default::default()).build().unwrap();
    let request=json!({"op":"list","limit":10});
    assert_eq!(invoke(&main,"habits_request",args(&json!(Uuid::new_v4()),request.clone())).unwrap_err()["code"],"stale_session");
    let first=invoke(&main,"editor_open",json!({"path":dir.path().join("one.db"),"create":true})).unwrap();
    invoke(&main,"extension_configure",json!({"session":first["token"],"extensionId":ID,"enabled":true})).unwrap();
    for extension in ["graphdesk.tally","other.extension","Graphdesk.habits"] {
        let mut a=args(&first["token"],request.clone()); a["extensionId"]=json!(extension);
        assert!(invoke(&main,"habits_request",a).is_err());
    }
    for version in [json!(0),json!(2),json!(-1),json!(1.5),json!("1")] {
        let mut a=args(&first["token"],request.clone()); a["apiVersion"]=version;
        assert!(invoke(&main,"habits_request",a).is_err());
    }
    let untrusted=tauri::WebviewWindowBuilder::new(&app,"untrusted",Default::default()).build().unwrap();
    for (window,url) in [(&untrusted,"tauri://localhost"),(&main,"https://example.invalid")] {
        let error=invoke_from(window,"habits_request",args(&first["token"],request.clone()),url).unwrap_err();
        assert!(error.as_str().unwrap().contains("not allowed"),"{error}");
    }
    let second=invoke(&main,"editor_open",json!({"path":dir.path().join("two.db"),"create":true})).unwrap();
    assert_eq!(invoke(&main,"habits_request",args(&first["token"],request.clone())).unwrap_err()["code"],"stale_session");
    assert!(invoke(&main,"habits_request",args(&second["token"],request.clone())).is_err());
    invoke(&main,"extension_configure",json!({"session":second["token"],"extensionId":ID,"enabled":true})).unwrap();
    assert_eq!(invoke(&main,"habits_request",args(&second["token"],request)).unwrap(),json!({"type":"habits","data":[]}));
}
