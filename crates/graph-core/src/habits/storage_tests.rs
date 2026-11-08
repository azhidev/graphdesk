use super::*;

fn definition() -> HabitDefinition {
    HabitDefinition { name:"Walk".into(),kind:HabitKind::Positive,schedule:Some(Schedule { anchor:0,every:1,unit:IntervalUnit::Days,mode:RecurrenceMode::Fixed,timezone:Some("UTC".into()) }),archived:false }
}
fn setup() -> (Graph,HabitCreate) {
    let mut g = Graph::in_memory().unwrap();
    g.extension_configure("graphdesk.habits",true).unwrap();
    let create = HabitCreate { id:Uuid::new_v4(),definition:definition() };
    g.habit_create(create.clone()).unwrap(); (g,create)
}
fn completion(habit_id:Uuid,at:i64) -> EventInput {
    EventInput { id:Uuid::new_v4(),habit_id,kind:EventKind::Completion,occurred_at:at,period_start:None,period_end:None }
}

#[test]
fn legacy_timezone_reads_and_retries_do_not_rewrite_history() {
    let (mut g, create) = setup();
    let mut legacy = serde_json::to_value(&create.definition).unwrap();
    legacy["schedule"].as_object_mut().unwrap().remove("timezone");
    let json = serde_json::to_string(&legacy).unwrap();
    g.conn.execute("UPDATE habits SET definition_json=?1", [&json]).unwrap();
    g.conn.execute("UPDATE habit_versions SET definition_json=?1", [&json]).unwrap();
    let read = g.habit_get(create.id).unwrap();
    assert_eq!(read.definition.schedule.as_ref().unwrap().timezone.as_deref(), Some("UTC"));
    let mut retry = create.clone();
    retry.definition = serde_json::from_value(legacy).unwrap();
    assert_eq!(g.habit_create(retry).unwrap(), read);
    assert_eq!(g.conn.query_row("SELECT definition_json FROM habits", [], |r| r.get::<_,String>(0)).unwrap(), json);
    assert_eq!(g.conn.query_row("SELECT definition_json FROM habit_versions", [], |r| r.get::<_,String>(0)).unwrap(), json);

    let input = completion(create.id, 10);
    let event = g.habit_record(input.clone(), 20).unwrap();
    let mut updated = read.definition;
    updated.schedule.as_mut().unwrap().timezone = Some("Asia/Tehran".into());
    g.habit_update(create.id, 0, updated).unwrap();
    assert_eq!(g.habit_record(input, 30).unwrap(), event);
    assert_eq!(g.conn.query_row("SELECT definition_json FROM habit_versions WHERE revision=0", [], |r| r.get::<_,String>(0)).unwrap(), json);
    let new_json: String = g.conn.query_row("SELECT definition_json FROM habit_versions WHERE revision=1", [], |r|r.get(0)).unwrap();
    assert_eq!(serde_json::from_str::<serde_json::Value>(&new_json).unwrap()["schedule"]["timezone"], "Asia/Tehran");
    let mut omitted = definition();
    omitted.schedule.as_mut().unwrap().timezone = None;
    assert_eq!(g.habit_update(create.id, 1, omitted).unwrap().definition.schedule.unwrap().timezone.as_deref(), Some("Asia/Tehran"));
}

#[test]
fn direct_core_create_omission_materializes_utc_before_retry_comparison() {
    let (mut g, _) = setup();
    let mut definition = definition();
    definition.schedule.as_mut().unwrap().timezone = None;
    let create = HabitCreate { id: Uuid::new_v4(), definition };
    let created = g.habit_create(create.clone()).unwrap();
    assert_eq!(created.definition.schedule.as_ref().unwrap().timezone.as_deref(), Some("UTC"));
    assert_eq!(g.habit_create(create).unwrap(), created);
}

#[test]
fn reopen_scoping_idempotency_and_archive_replay() {
    let dir = tempfile::tempdir().unwrap(); let path=dir.path().join("habits.db");
    let mut g=Graph::create(&path).unwrap(); g.extension_configure("graphdesk.habits",true).unwrap();
    let create=HabitCreate { id:Uuid::new_v4(),definition:definition() };
    let habit=g.habit_create(create.clone()).unwrap();
    assert_eq!(g.habit_create(create.clone()).unwrap(),habit);
    let input=completion(create.id,10); let recorded=g.habit_record(input.clone(),20).unwrap();
    assert_eq!(g.habit_record(input.clone(),30).unwrap(),recorded);
    let corrected=g.habit_correct(input.id,0,true).unwrap();
    let mut archived=create.definition.clone(); archived.archived=true;
    g.habit_update(create.id,0,archived).unwrap();
    assert_eq!(g.habit_record(input.clone(),40).unwrap(),corrected);
    assert!(g.habit_record(completion(create.id,15),40).is_err());
    let mut different=input.clone(); different.occurred_at=11;
    assert!(matches!(g.habit_record(different,40),Err(Error::Conflict(_))));
    assert!(matches!(g.habit_create(create.clone()),Err(Error::Conflict(_))));
    drop(g);
    let g=Graph::open(&path).unwrap();
    assert!(g.habit_get(create.id).unwrap().definition.archived);
    assert_eq!(g.habit_events(create.id,None,100).unwrap(),vec![corrected]);
    assert_eq!(g.conn.query_row("SELECT count(*) FROM habit_versions",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    let (other,_) = setup(); assert!(matches!(other.habit_get(create.id),Err(Error::NotFound)));
}

#[test]
fn cas_rollbacks_kind_immutability_and_safe_revision_ceiling() {
    let (mut g,create)=setup(); let input=completion(create.id,10);
    g.habit_record(input.clone(),20).unwrap();
    assert!(g.habit_record(completion(create.id,30),20).is_err());
    let mut def=create.definition.clone(); def.name="Edited".into();
    g.habit_update(create.id,0,def.clone()).unwrap();
    assert!(matches!(g.habit_update(create.id,0,def.clone()),Err(Error::Conflict(_))));
    def.kind=HabitKind::Negative; def.schedule=None;
    assert!(g.habit_update(create.id,1,def).is_err());
    assert_eq!(g.habit_get(create.id).unwrap().revision,1);
    assert_eq!(g.conn.query_row("SELECT count(*) FROM habit_versions",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    g.habit_correct(input.id,0,true).unwrap();
    assert!(matches!(g.habit_correct(input.id,0,false),Err(Error::Conflict(_))));
    g.habit_correct(input.id,1,false).unwrap();
    assert_eq!(g.habit_events(create.id,None,100).unwrap().len(),1);
    g.conn.execute("UPDATE habit_events SET revision=9007199254740991",[]).unwrap();
    assert!(g.habit_correct(input.id,MAX_REVISION,true).is_err());
    assert!(!g.habit_events(create.id,None,100).unwrap()[0].voided);
    g.conn.execute("UPDATE habits SET revision=9007199254740991",[]).unwrap();
    assert!(g.habit_update(create.id,MAX_REVISION,create.definition).is_err());
}

#[test]
fn all_operations_guard_disabled_without_erasing_history() {
    let (mut g,create)=setup(); let input=completion(create.id,1); g.habit_record(input.clone(),2).unwrap();
    g.extension_configure("graphdesk.habits",false).unwrap();
    assert!(g.habit_get(create.id).is_err()); assert!(g.habit_list(None,10).is_err());
    assert!(g.habit_status(create.id,2).is_err()); assert!(g.habit_events(create.id,None,10).is_err());
    assert!(g.habit_create(create.clone()).is_err()); assert!(g.habit_update(create.id,0,create.definition.clone()).is_err());
    assert!(g.habit_record(input.clone(),2).is_err()); assert!(g.habit_correct(input.id,0,true).is_err());
    g.extension_configure("graphdesk.habits",true).unwrap();
    assert_eq!(g.habit_events(create.id,None,10).unwrap().len(),1);
    assert!(g.habit_get(Uuid::nil()).is_err());
    assert!(g.habit_list(Some(Uuid::nil()),1).is_err());
    for n in [0,101,usize::MAX] { assert!(g.habit_list(None,n).is_err()); assert!(g.habit_events(create.id,None,n).is_err()); }
}

#[test]
fn pagination_status_ties_voids_future_and_reanchoring() {
    let (mut g,create)=setup();
    let mut ids=Vec::new();
    for at in [10,10,20,100] { let input=completion(create.id,at); ids.push(input.id); g.habit_record(input,100).unwrap(); }
    let all=g.habit_events(create.id,None,100).unwrap();
    let mut history=g.habit_events(create.id,None,2).unwrap();
    let last=history.last().unwrap();
    history.extend(g.habit_events(create.id,Some(EventCursor { occurred_at:last.occurred_at,id:last.id }),2).unwrap());
    assert_eq!(history,all);
    g.habit_correct(ids[2],0,true).unwrap();
    let expected=due_state(create.definition.schedule.as_ref().unwrap(),Some(10),50).unwrap();
    assert_eq!(g.habit_status(create.id,50).unwrap().positive,Some(expected));
    let mut def=create.definition.clone(); def.schedule.as_mut().unwrap().anchor=30;
    g.habit_update(create.id,0,def.clone()).unwrap();
    assert_eq!(g.habit_status(create.id,50).unwrap().positive,Some(due_state(def.schedule.as_ref().unwrap(),None,50).unwrap()));
    let mut expected_ids=vec![create.id];
    for _ in 0..3 { let id=Uuid::new_v4(); expected_ids.push(id); g.habit_create(HabitCreate { id,definition:definition() }).unwrap(); }
    expected_ids.sort(); let first=g.habit_list(None,2).unwrap();
    let second=g.habit_list(Some(first[1].id),2).unwrap();
    assert_eq!(first.into_iter().chain(second).map(|h|h.id).collect::<Vec<_>>(),expected_ids);
}

#[test]
fn negative_status_preserves_explicit_confirmation_and_overlapping_lapse() {
    let (mut g,_) = setup(); let id=Uuid::new_v4();
    g.habit_create(HabitCreate { id,definition:HabitDefinition { name:"Avoid".into(),kind:HabitKind::Negative,schedule:None,archived:false } }).unwrap();
    let empty=g.habit_status(id,100).unwrap().negative.unwrap();
    assert!(empty.confirmation.is_none() && empty.last_lapse.is_none());
    let confirmation=EventInput { id:Uuid::new_v4(),habit_id:id,kind:EventKind::Confirmation,occurred_at:30,period_start:Some(0),period_end:Some(20) };
    let confirmed=g.habit_record(confirmation.clone(),100).unwrap();
    let lapse=EventInput { id:Uuid::new_v4(),habit_id:id,kind:EventKind::Lapse,occurred_at:10,period_start:None,period_end:None };
    let lapsed=g.habit_record(lapse,100).unwrap();
    let status=g.habit_status(id,100).unwrap().negative.unwrap();
    assert_eq!(status.confirmation,Some(confirmed)); assert_eq!(status.last_lapse,Some(lapsed));
    g.habit_correct(confirmation.id,0,true).unwrap();
    assert!(g.habit_status(id,100).unwrap().negative.unwrap().confirmation.is_none());
}

#[test]
fn failed_version_insert_rolls_back_definition_update() {
    let (mut g,create)=setup();
    g.conn.execute_batch("CREATE TRIGGER fail_versions BEFORE INSERT ON habit_versions BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
    let mut def=create.definition.clone(); def.name="must rollback".into();
    assert!(g.habit_update(create.id,0,def).is_err());
    assert_eq!(g.habit_get(create.id).unwrap().definition,create.definition);
    assert_eq!(g.habit_get(create.id).unwrap().revision,0);
}

#[test]
fn backdated_event_captures_definition_known_at_recording() {
    let (mut g,create)=setup();
    let old=completion(create.id,1); g.habit_record(old.clone(),10).unwrap();
    let mut def=create.definition.clone(); def.name="Renamed".into();
    g.habit_update(create.id,0,def).unwrap();
    let backdated=completion(create.id,1); g.habit_record(backdated.clone(),20).unwrap();
    for (id,expected) in [(old.id,0),(backdated.id,1)] {
        assert_eq!(g.conn.query_row("SELECT habit_revision FROM habit_events WHERE id=?1",[id.to_string()],|r|r.get::<_,i64>(0)).unwrap(),expected);
    }
    assert_eq!(g.conn.query_row("SELECT count(*) FROM habit_versions WHERE recorded_at IS NOT NULL",[],|r|r.get::<_,i64>(0)).unwrap(),2);
}
