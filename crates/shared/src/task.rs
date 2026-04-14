use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A task descriptor submitted to the Orchestration Kernel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDescriptor {
    pub id: String,
    pub task_type: String,
    pub payload: serde_json::Value,
    pub submitted_at: DateTime<Utc>,
}

/// The result returned by an agent after executing a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub agent_id: String,
    pub status: TaskStatus,
    pub output_path: Option<String>,
    pub errors: Vec<String>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Success,
    Failed,
    /// The task was refused because it violated a policy boundary.
    Refused,
}

/// A decision log entry produced by the Kernel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionLogEntry {
    pub timestamp: DateTime<Utc>,
    pub task_id: String,
    pub selected_agent: String,
    pub rationale: String,
    pub outcome: Option<TaskStatus>,
    /// SHA-256 of the previous log entry's JSON line (hex).
    /// Genesis value: 64 zeros.
    #[serde(default = "default_prev_hash")]
    pub prev_hash: String,
}

fn default_prev_hash() -> String {
    "0".repeat(64)
}
