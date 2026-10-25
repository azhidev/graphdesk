use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Page {
    pub id: Uuid,
    pub title: String,
    pub journal_date: Option<String>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus { Todo, Doing, Done, Cancelled }

impl TaskStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self { Self::Todo => "todo", Self::Doing => "doing", Self::Done => "done", Self::Cancelled => "cancelled" }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFilter { Active, Todo, Doing, Done }

impl TaskFilter {
    pub(crate) fn statuses(self) -> (&'static str, &'static str) {
        match self {
            Self::Active => ("todo", "doing"),
            Self::Todo => ("todo", "todo"),
            Self::Doing => ("doing", "doing"),
            Self::Done => ("done", "done"),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TaskHit {
    pub id: Uuid,
    pub page_id: Uuid,
    pub page_title: String,
    pub content: String,
    pub task_status: String,
}

#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub id: Uuid,
    pub page_id: Uuid,
    pub kind: String,
    pub text: String,
}
