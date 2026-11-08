//! Bundled-only extension protocol. All checks and storage operations share the
//! existing session lock, and SQLite work runs on the blocking worker pool.
use graph_core::{Error, ExtensionDocument, ExtensionState};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;
use uuid::Uuid;

use crate::{AppState, editor::{blocking, with_session, EditorError}};

const BUNDLED_ID: &str = "graphdesk.tally";
const BUNDLED_IDS: &[&str] = &[BUNDLED_ID, "graphdesk.habits"];
const API_VERSION: u32 = 1;

#[derive(Serialize)]
pub(crate) struct ExtensionStatus { graph_id: Uuid, extensions: Vec<ExtensionState> }

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ExtensionRequest {
    StorageRead {},
    StorageWrite { expected_revision: i64, value: Value },
}

fn registered(id: &str) -> graph_core::Result<()> {
    if !BUNDLED_IDS.contains(&id) { return Err(Error::Invalid("extension is not bundled or registered".into())); }
    Ok(())
}

#[tauri::command]
pub(crate) async fn extension_state(session: Uuid, state: State<'_, AppState>) -> Result<ExtensionStatus, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| {
        let states = g.editor.graph().extension_states()?;
        Ok(ExtensionStatus {
            graph_id: g.editor.graph().id()?,
            extensions: BUNDLED_IDS.iter().map(|id| ExtensionState { id: (*id).into(), enabled: states.iter().any(|s| s.id == *id && s.enabled) }).collect(),
        })
    })).await
}

#[tauri::command]
pub(crate) async fn extension_configure(session: Uuid, extension_id: String, enabled: bool, state: State<'_, AppState>) -> Result<(), EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| {
        registered(&extension_id)?;
        g.editor.extension_configure(&extension_id, enabled)
    })).await
}

#[tauri::command]
pub(crate) async fn extension_request(session: Uuid, extension_id: String, api_version: u32, request: ExtensionRequest, state: State<'_, AppState>) -> Result<ExtensionDocument, EditorError> {
    let state = state.inner().clone();
    blocking(move || with_session(&state, session, |g| {
        registered(&extension_id)?;
        if api_version != API_VERSION { return Err(Error::Invalid("unsupported extension API version".into())); }
        // Core rechecks enablement in the read/write transaction, while this same
        // session lock excludes configuration changes and graph activation.
        match request {
            ExtensionRequest::StorageRead {} => g.editor.graph().extension_read(&extension_id),
            ExtensionRequest::StorageWrite { expected_revision, value } => g.editor.extension_write(&extension_id, expected_revision, value),
        }
    })).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_protocol_has_only_two_strict_operations() {
        for value in [json!({"op":"storage_read"}), json!({"op":"storage_write","expected_revision":0,"value":null})] {
            assert!(serde_json::from_value::<ExtensionRequest>(value).is_ok());
        }
        for value in [json!({"op":"storage_read","path":"arbitrary"}),
            json!({"op":"storage_write","expected_revision":0,"value":null,"sql":"arbitrary"}),
            json!({"op":"storage_write","value":null}), json!({"op":"storage_write","expected_revision":0}),
            json!({"op":"storage_write","expected_revision":0.5,"value":null}),
            json!({"op":"sql"}), json!({"op":"storageRead"}), json!({})] {
            assert!(serde_json::from_value::<ExtensionRequest>(value.clone()).is_err(), "{value}");
        }
        assert!(registered(BUNDLED_ID).is_ok());
        assert!(registered("other.extension").is_err());
    }
}
