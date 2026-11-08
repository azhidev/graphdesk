use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use uuid::Uuid;
use crate::{Error, Graph, Result};
use super::*;

const SCOPE: &str = "(SELECT id FROM graph_identity WHERE singleton=1)";
const EVENT_COLUMNS: &str = "id,habit_id,kind,occurred_at,recorded_at,period_start,period_end,revision,voided";
const MAX_REVISION: i64 = 9_007_199_254_740_991;

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;

fn enabled(conn: &Connection) -> Result<()> {
    let yes: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM extensions WHERE graph_id=(SELECT id FROM graph_identity WHERE singleton=1) AND extension_id='graphdesk.habits' AND enabled=1)", [], |r| r.get(0))?;
    if !yes { return Err(Error::Invalid("extension is disabled".into())); }
    Ok(())
}
fn limit_ok(limit: usize) -> Result<()> {
    if !(1..=100).contains(&limit) { return Err(Error::Invalid("habit page limit must be 1..100".into())); }
    Ok(())
}
fn revision_next(revision: i64) -> Result<i64> {
    if !(0..MAX_REVISION).contains(&revision) { return Err(Error::Conflict("habit revision exhausted or invalid".into())); }
    Ok(revision + 1)
}
fn get(conn: &Connection, id: Uuid) -> Result<Habit> {
    valid_id(id)?;
    let row: Option<(i64, String)> = conn.query_row(&format!("SELECT revision,definition_json FROM habits WHERE graph_id={SCOPE} AND id=?1"), [id.to_string()], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    let (revision, json) = row.ok_or(Error::NotFound)?;
    let mut definition = serde_json::from_str(&json).map_err(|_| Error::Invalid("invalid stored habit definition".into()))?;
    materialize_timezone(&mut definition, "UTC")?;
    Ok(Habit { id, revision, definition })
}
fn valid_id(id: Uuid) -> Result<()> {
    if id.is_nil() { return Err(Error::Invalid("nil habit/event UUID is not allowed".into())); }
    Ok(())
}
fn kind_name(kind: EventKind) -> &'static str {
    match kind { EventKind::Completion => "completion", EventKind::Lapse => "lapse", EventKind::Confirmation => "confirmation" }
}
type RawEvent = (String,String,String,i64,i64,Option<i64>,Option<i64>,i64,bool);
fn read_event(r: &rusqlite::Row<'_>) -> rusqlite::Result<RawEvent> {
    Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?))
}
fn decode_event(r: RawEvent) -> Result<HabitEvent> {
    let kind = match r.2.as_str() { "completion" => EventKind::Completion, "lapse" => EventKind::Lapse, "confirmation" => EventKind::Confirmation, _ => return Err(Error::Invalid("invalid stored event kind".into())) };
    Ok(HabitEvent { id: crate::parse_id(&r.0)?, habit_id: crate::parse_id(&r.1)?, kind, occurred_at:r.3, recorded_at:r.4, period_start:r.5, period_end:r.6, revision:r.7, voided:r.8 })
}
fn event(conn: &Connection, id: Uuid) -> Result<Option<HabitEvent>> {
    valid_id(id)?;
    conn.query_row(&format!("SELECT {EVENT_COLUMNS} FROM habit_events WHERE graph_id={SCOPE} AND id=?1"), [id.to_string()], read_event).optional()?.map(decode_event).transpose()
}
fn latest(conn: &Connection, id: Uuid, kind: EventKind, now: i64, since: i64) -> Result<Option<HabitEvent>> {
    conn.query_row(&format!("SELECT {EVENT_COLUMNS} FROM habit_events WHERE graph_id={SCOPE} AND habit_id=?1 AND kind=?2 AND voided=0 AND occurred_at<=?3 AND occurred_at>=?4 ORDER BY occurred_at DESC,id DESC LIMIT 1"), params![id.to_string(),kind_name(kind),now,since], read_event).optional()?.map(decode_event).transpose()
}

impl Graph {
    pub fn habit_create(&mut self, mut input: HabitCreate) -> Result<Habit> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        enabled(&tx)?;
        valid_id(input.id)?;
        materialize_timezone(&mut input.definition, "UTC")?;
        validate_definition(&input.definition)?;
        match get(&tx,input.id) {
            Ok(current) => {
                if current.revision != 0 || current.definition != input.definition { return Err(Error::Conflict("habit ID already exists".into())); }
                tx.commit()?; return Ok(current);
            },
            Err(Error::NotFound) => (), Err(e) => return Err(e),
        }
        let json = serde_json::to_string(&input.definition).map_err(|_| Error::Invalid("invalid habit definition".into()))?;
        tx.execute(&format!("INSERT INTO habits(graph_id,id,definition_json) VALUES({SCOPE},?1,?2)"), params![input.id.to_string(),json])?;
        tx.execute(&format!("INSERT INTO habit_versions(graph_id,habit_id,revision,definition_json) SELECT graph_id,id,revision,definition_json FROM habits WHERE graph_id={SCOPE} AND id=?1"),[input.id.to_string()])?;
        let habit = get(&tx,input.id)?; tx.commit()?; Ok(habit)
    }
    pub fn habit_update(&mut self, id: Uuid, expected_revision: i64, mut definition: HabitDefinition) -> Result<Habit> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?; enabled(&tx)?;
        let current = get(&tx,id)?;
        if current.revision != expected_revision { return Err(Error::Conflict("habit changed".into())); }
        let timezone = current.definition.schedule.as_ref().and_then(|s| s.timezone.as_deref()).unwrap_or("UTC");
        materialize_timezone(&mut definition, timezone)?;
        validate_definition(&definition)?;
        if current.definition.kind != definition.kind { return Err(Error::Invalid("habit kind is immutable".into())); }
        let revision = revision_next(expected_revision)?;
        let json = serde_json::to_string(&definition).map_err(|_| Error::Invalid("invalid habit definition".into()))?;
        crate::changed(tx.execute(&format!("UPDATE habits SET revision=?2,definition_json=?3 WHERE graph_id={SCOPE} AND id=?1 AND revision=?4"),params![id.to_string(),revision,json,expected_revision])?)?;
        tx.execute(&format!("INSERT INTO habit_versions(graph_id,habit_id,revision,definition_json) SELECT graph_id,id,revision,definition_json FROM habits WHERE graph_id={SCOPE} AND id=?1"),[id.to_string()])?;
        tx.commit()?; Ok(Habit { id,revision,definition })
    }
    pub fn habit_get(&self, id: Uuid) -> Result<Habit> {
        let tx = self.conn.unchecked_transaction()?; enabled(&tx)?;
        let habit = get(&tx,id)?; tx.commit()?; Ok(habit)
    }
    pub fn habit_list(&self, after: Option<Uuid>, limit: usize) -> Result<Vec<Habit>> {
        let tx = self.conn.unchecked_transaction()?; enabled(&tx)?; limit_ok(limit)?;
        if let Some(id) = after { valid_id(id)?; }
        let habits = {
            let mut stmt = tx.prepare(&format!("SELECT id FROM habits WHERE graph_id={SCOPE} AND id>?1 ORDER BY id LIMIT ?2"))?;
            let ids = stmt.query_map(params![after.map(|id|id.to_string()).unwrap_or_default(),limit as i64], |r|r.get::<_,String>(0))?;
            ids.map(|id| get(&tx,crate::parse_id(&id?)?)).collect::<Result<Vec<_>>>()?
        };
        tx.commit()?; Ok(habits)
    }
    pub fn habit_record(&mut self, input: EventInput, recorded_at: i64) -> Result<HabitEvent> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?; enabled(&tx)?;
        // Retry identity ignores server receipt time and subsequent corrections/archive.
        valid_id(input.habit_id)?;
        if let Some(existing) = event(&tx,input.id)? {
            if existing.habit_id != input.habit_id || existing.kind != input.kind || existing.occurred_at != input.occurred_at || existing.period_start != input.period_start || existing.period_end != input.period_end {
                return Err(Error::Conflict("event ID already has a different payload".into()));
            }
            tx.commit()?; return Ok(existing);
        }
        let habit = get(&tx,input.habit_id)?;
        if habit.definition.archived { return Err(Error::Invalid("habit is archived".into())); }
        validate_event(&habit.definition,&input,recorded_at)?;
        tx.execute(&format!("INSERT INTO habit_events(graph_id,id,habit_id,kind,occurred_at,recorded_at,period_start,period_end,habit_revision) VALUES({SCOPE},?1,?2,?3,?4,?5,?6,?7,?8)"),params![input.id.to_string(),input.habit_id.to_string(),kind_name(input.kind),input.occurred_at,recorded_at,input.period_start,input.period_end,habit.revision])?;
        let result = event(&tx,input.id)?.ok_or(Error::NotFound)?; tx.commit()?; Ok(result)
    }
    pub fn habit_correct(&mut self, event_id: Uuid, expected_revision: i64, voided: bool) -> Result<HabitEvent> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?; enabled(&tx)?;
        let mut current = event(&tx,event_id)?.ok_or(Error::NotFound)?;
        if current.revision != expected_revision { return Err(Error::Conflict("event changed".into())); }
        let revision = revision_next(expected_revision)?;
        crate::changed(tx.execute(&format!("UPDATE habit_events SET revision=?2,voided=?3 WHERE graph_id={SCOPE} AND id=?1 AND revision=?4"),params![event_id.to_string(),revision,voided,expected_revision])?)?;
        current.revision=revision; current.voided=voided; tx.commit()?; Ok(current)
    }
    pub fn habit_events(&self, habit_id: Uuid, before: Option<EventCursor>, limit: usize) -> Result<Vec<HabitEvent>> {
        let tx = self.conn.unchecked_transaction()?; enabled(&tx)?; limit_ok(limit)?; get(&tx,habit_id)?;
        if let Some(cursor) = &before { validate_time(cursor.occurred_at)?; valid_id(cursor.id)?; }
        let events = {
            let base = format!("SELECT {EVENT_COLUMNS} FROM habit_events WHERE graph_id={SCOPE} AND habit_id=?1");
            let sql = format!("{base} AND (occurred_at,id)<(?2,?3) ORDER BY occurred_at DESC,id DESC LIMIT ?4");
            let (at, id) = match before { Some(c) => (c.occurred_at,c.id.to_string()), None => (253402300800,String::new()) };
            let mut stmt = tx.prepare(&sql)?;
            let rows = stmt.query_map(params![habit_id.to_string(),at,id,limit as i64],read_event)?;
            rows.map(|r|decode_event(r?)).collect::<Result<Vec<_>>>()?
        };
        tx.commit()?; Ok(events)
    }
    pub fn habit_status(&self, id: Uuid, now: i64) -> Result<HabitStatus> {
        let tx = self.conn.unchecked_transaction()?; enabled(&tx)?; validate_time(now)?;
        let habit = get(&tx,id)?;
        let status = match habit.definition.kind {
            HabitKind::Positive => {
                let schedule = habit.definition.schedule.as_ref().ok_or_else(||Error::Invalid("positive habit requires schedule".into()))?;
                let completion = latest(&tx,id,EventKind::Completion,now,schedule.anchor)?.map(|e|e.occurred_at);
                HabitStatus { habit_id:id,positive:Some(due_state(schedule,completion,now)?),negative:None }
            },
            HabitKind::Negative => HabitStatus { habit_id:id,positive:None,negative:Some(NegativeState { confirmation:latest(&tx,id,EventKind::Confirmation,now,-62135596800)?,last_lapse:latest(&tx,id,EventKind::Lapse,now,-62135596800)? }) },
        };
        tx.commit()?; Ok(status)
    }
}
