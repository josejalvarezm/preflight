pub mod client;
pub mod executor;

use ai_os_shared::error::AiOsError;

/// Errors specific to the LLM runtime.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("LLM request failed: {0}")]
    LlmRequestFailed(String),

    #[error("LLM returned no content")]
    EmptyResponse,

    #[error("LLM service unreachable at {url}: {reason}")]
    ServiceUnavailable { url: String, reason: String },

    #[error(transparent)]
    AiOs(#[from] AiOsError),
}

pub type RuntimeResult<T> = std::result::Result<T, RuntimeError>;
