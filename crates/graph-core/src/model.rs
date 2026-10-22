use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Page {
    pub id: Uuid,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Block {
    pub id: Uuid,
    pub page_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub position: i64,
    pub content: String,
    pub task_status: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus { Todo, Doing, Done, Cancelled }

impl TaskStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self { Self::Todo => "todo", Self::Doing => "doing", Self::Done => "done", Self::Cancelled => "cancelled" }
    }
}

#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub id: Uuid,
    pub page_id: Uuid,
    pub kind: String,
    pub text: String,
}
