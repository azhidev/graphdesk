use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HabitKind { Positive, Negative }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntervalUnit { Hours, Days, Weeks, Months, Years }

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecurrenceMode { #[default] Fixed, AfterCompletion }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    /// Unix seconds UTC. No implicit device timezone or presentation calendar.
    pub anchor: i64,
    pub every: u32,
    pub unit: IntervalUnit,
    #[serde(default)]
    pub mode: RecurrenceMode,
    /// Resolved IANA zone. Missing in pre-timezone JSON means legacy UTC only;
    /// new IPC creates snapshot the application default before persistence.
    #[serde(default)]
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HabitDefinition {
    pub name: String,
    pub kind: HabitKind,
    pub schedule: Option<Schedule>,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HabitCreate { pub id: Uuid, pub definition: HabitDefinition }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Habit { pub id: Uuid, pub revision: i64, pub definition: HabitDefinition }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind { Completion, Lapse, Confirmation }
impl Default for EventKind { fn default() -> Self { Self::Lapse } }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventInput {
    /// Caller-generated retry identity, reused after a lost acknowledgement.
    pub id: Uuid,
    pub habit_id: Uuid,
    #[serde(default)]
    pub kind: EventKind,
    pub occurred_at: i64,
    pub period_start: Option<i64>,
    pub period_end: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HabitEvent {
    pub id: Uuid, pub habit_id: Uuid, pub kind: EventKind,
    pub occurred_at: i64, pub recorded_at: i64,
    pub period_start: Option<i64>, pub period_end: Option<i64>,
    pub revision: i64, pub voided: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventCursor { pub occurred_at: i64, pub id: Uuid }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DueState {
    pub due: bool,
    /// Current fixed window start, or the outstanding After Completion deadline.
    pub occurrence: i64,
    /// Fixed: next anchor window. After Completion: outstanding deadline (even
    /// when overdue); no speculative future deadline before completion.
    pub next_occurrence: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegativeState {
    /// An explicit reported period, NOT an inferred success/absence of lapses.
    pub confirmation: Option<HabitEvent>,
    pub last_lapse: Option<HabitEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HabitStatus {
    pub habit_id: Uuid,
    pub positive: Option<DueState>,
    pub negative: Option<NegativeState>,
}
