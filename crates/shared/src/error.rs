use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiOsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Validation error in '{file}': {message}")]
    Validation { file: String, message: String },

    #[error("Contradiction detected between '{file_a}' and '{file_b}': {description}")]
    Contradiction {
        file_a: String,
        file_b: String,
        description: String,
    },

    #[error("No agent found for task type: {0}")]
    NoAgentForTask(String),

    #[error("Agent '{agent}' returned error: {message}")]
    AgentError { agent: String, message: String },

    #[error("Policy violation on task '{task_id}': boundary '{boundary_id}' — {reason}")]
    PolicyViolation {
        task_id: String,
        boundary_id: String,
        reason: String,
    },
}

pub type Result<T> = std::result::Result<T, AiOsError>;
