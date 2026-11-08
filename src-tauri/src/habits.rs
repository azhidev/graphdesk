//! Typed bundled habits protocol; receipt timestamps are server-owned.
use graph_core::{habits::*, Error};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;
use crate::{AppState, editor::{blocking, with_session, EditorError}, preferences::{PreferencesState, resolved_habit_timezone}};

#[derive(Debug, Deserialize)]
#[serde(tag="op", rename_all="snake_case", deny_unknown_fields)]
pub(crate) enum HabitsRequest {
    Create { input: HabitCreate },
    Update { id: Uuid, expected_revision: i64, definition: HabitDefinition },
    Get { id: Uuid },
    List { after: Option<Uuid>, limit: usize },
    Record { input: EventInput },
    Correct { event_id: Uuid, expected_revision: i64, voided: bool },
    Events { habit_id: Uuid, before: Option<EventCursor>, limit: usize },
    Status { id: Uuid, now: i64 },
}
#[derive(Debug, Serialize)]
#[serde(tag="type", content="data", rename_all="snake_case")]
pub(crate) enum HabitsResponse { Habit(Habit), Habits(Vec<Habit>), Event(HabitEvent), Events(Vec<HabitEvent>), Status(HabitStatus) }

#[tauri::command]
pub(crate) async fn habits_request<R: tauri::Runtime>(app: tauri::AppHandle<R>, session: Uuid, extension_id: String, api_version: u32, request: HabitsRequest, state: State<'_, AppState>, preferences: State<'_, PreferencesState>) -> Result<HabitsResponse, EditorError> {
    let state = state.inner().clone();
    let preferences = preferences.inner().clone();
    blocking(move || {
        let mut request = request;
        // Preflight releases the session lock before resolving app preferences.
        // Replays reuse the stored zone even if Settings (or system detection) changed.
        if let HabitsRequest::Create { input } = &mut request {
            if input.definition.schedule.as_ref().is_some_and(|s| s.timezone.is_none()) {
                let existing = with_session(&state, session, |g| {
                    check_protocol(&extension_id, api_version)?;
                    match g.editor.graph().habit_get(input.id) {
                        Ok(habit) => Ok(Some(habit.definition)),
                        Err(Error::NotFound) => Ok(None),
                        Err(error) => Err(error),
                    }
                })?;
                let timezone = match existing {
                    Some(definition) => definition.schedule.and_then(|s| s.timezone).unwrap_or_else(|| "UTC".into()),
                    None => resolved_habit_timezone(&app, &preferences)?,
                };
                materialize_timezone(&mut input.definition, &timezone)?;
            }
        }
        with_session(&state,session,|g| {
        check_protocol(&extension_id, api_version)?;
        Ok(match request {
            HabitsRequest::Create { input } => HabitsResponse::Habit(g.editor.habit_create(input)?),
            HabitsRequest::Update { id,expected_revision,mut definition } => {
                if definition.schedule.as_ref().is_some_and(|s| s.timezone.is_none()) {
                    let current = g.editor.graph().habit_get(id)?;
                    let timezone = current.definition.schedule.and_then(|s| s.timezone).unwrap_or_else(|| "UTC".into());
                    materialize_timezone(&mut definition, &timezone)?;
                }
                HabitsResponse::Habit(g.editor.habit_update(id,expected_revision,definition)?)
            },
            HabitsRequest::Get { id } => HabitsResponse::Habit(g.editor.graph().habit_get(id)?),
            HabitsRequest::List { after,limit } => HabitsResponse::Habits(g.editor.graph().habit_list(after,limit)?),
            HabitsRequest::Record { input } => {
                let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_| Error::Invalid("system clock predates Unix epoch".into()))?.as_secs();
                let recorded_at = i64::try_from(seconds).map_err(|_| Error::Invalid("system clock out of range".into()))?;
                HabitsResponse::Event(g.editor.habit_record(input,recorded_at)?)
            },
            HabitsRequest::Correct { event_id,expected_revision,voided } => HabitsResponse::Event(g.editor.habit_correct(event_id,expected_revision,voided)?),
            HabitsRequest::Events { habit_id,before,limit } => HabitsResponse::Events(g.editor.graph().habit_events(habit_id,before,limit)?),
            HabitsRequest::Status { id,now } => HabitsResponse::Status(g.editor.graph().habit_status(id,now)?),
        })
        })
    }).await
}

fn check_protocol(extension_id: &str, api_version: u32) -> graph_core::Result<()> {
    if extension_id != "graphdesk.habits" { return Err(Error::Invalid("habits extension ID required".into())); }
    if api_version != 1 { return Err(Error::Invalid("unsupported extension API version".into())); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn strict_typed_protocol() {
        assert_eq!(serde_json::to_value(HabitsResponse::Habits(vec![])).unwrap(), json!({"type":"habits","data":[]}));
        assert!(serde_json::from_value::<HabitsRequest>(json!({"op":"list","limit":100})).is_ok());
        for value in [json!({"op":"list","limit":1,"sql":"SELECT 1"}),json!({"op":"list","limit":1.5}),json!({"op":"storage_read"}),json!({"op":"get","id":"not-uuid"}),json!({"op":"record","input":{},"recorded_at":0})] {
            assert!(serde_json::from_value::<HabitsRequest>(value).is_err());
        }
    }
}
