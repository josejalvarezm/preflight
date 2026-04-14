//! OpenAI-compatible HTTP client for LM Studio (or any compatible backend).

use crate::{RuntimeError, RuntimeResult};
use serde::{Deserialize, Serialize};

/// Configuration for the LLM client.
#[derive(Debug, Clone)]
pub struct LlmClientConfig {
    /// Base URL of the OpenAI-compatible API (e.g. "http://localhost:1234/v1").
    pub base_url: String,
    /// Model identifier to use for chat completions.
    pub chat_model: String,
    /// Model identifier to use for embeddings (optional).
    pub embedding_model: Option<String>,
    /// Maximum tokens for chat completion responses.
    pub max_tokens: u32,
    /// Temperature for generation (0.0 = deterministic).
    pub temperature: f32,
    /// Append a reasoning-disable suffix (e.g. `/no_think` for Qwen3 models).
    pub disable_reasoning_tokens: bool,
}

/// Suffix appended to user messages when `disable_reasoning_tokens` is true.
const REASONING_DISABLE_SUFFIX: &str = "/no_think";

impl Default for LlmClientConfig {
    fn default() -> Self {
        let chat_model =
            "huihui-qwen3-30b-a3b-instruct-2507-abliterated-i1@iq3_xs".to_string();
        let disable_reasoning_tokens = chat_model.contains("qwen3");
        LlmClientConfig {
            base_url: "http://localhost:1234/v1".to_string(),
            chat_model,
            embedding_model: Some("text-embedding-nomic-embed-text-v1.5".to_string()),
            max_tokens: 512,
            temperature: 0.0,
            disable_reasoning_tokens,
        }
    }
}

/// A blocking HTTP client for OpenAI-compatible APIs.
pub struct LlmClient {
    config: LlmClientConfig,
    http: reqwest::blocking::Client,
}

// --- Request/Response types (OpenAI-compatible) ---

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Debug, Clone, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

/// The result of a chat completion call.
#[derive(Debug, Clone)]
pub struct ChatCompletion {
    /// The generated text content.
    pub content: String,
    /// Token usage statistics.
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl LlmClient {
    /// Create a new client with the given configuration.
    pub fn new(config: LlmClientConfig) -> RuntimeResult<Self> {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| RuntimeError::LlmRequestFailed(format!("Failed to build HTTP client: {e}")))?;

        Ok(LlmClient { config, http })
    }

    /// Create a client with default configuration (localhost:1234).
    pub fn default_local() -> RuntimeResult<Self> {
        Self::new(LlmClientConfig::default())
    }

    /// Check if the LLM service is reachable.
    pub fn health_check(&self) -> RuntimeResult<Vec<String>> {
        let url = format!("{}/models", self.config.base_url);
        let resp: serde_json::Value = self
            .http
            .get(&url)
            .send()
            .map_err(|e| RuntimeError::ServiceUnavailable {
                url: url.clone(),
                reason: e.to_string(),
            })?
            .json()
            .map_err(|e| RuntimeError::LlmRequestFailed(e.to_string()))?;

        let models: Vec<String> = resp["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|m| m["id"].as_str().map(String::from))
            .collect();

        Ok(models)
    }

    /// Send a chat completion request.
    pub fn chat(&self, system_prompt: &str, user_message: &str) -> RuntimeResult<ChatCompletion> {
        self.chat_internal(system_prompt, user_message, None)
    }

    /// Send a chat completion with JSON response format enforced.
    /// The LLM is constrained to produce valid JSON output.
    pub fn chat_json(
        &self,
        system_prompt: &str,
        user_message: &str,
    ) -> RuntimeResult<ChatCompletion> {
        self.chat_internal(
            system_prompt,
            user_message,
            Some(ResponseFormat {
                format_type: "json_object".to_string(),
            }),
        )
    }

    /// Shared implementation for chat completion requests.
    fn chat_internal(
        &self,
        system_prompt: &str,
        user_message: &str,
        response_format: Option<ResponseFormat>,
    ) -> RuntimeResult<ChatCompletion> {
        let url = format!("{}/chat/completions", self.config.base_url);

        let user_content = if self.config.disable_reasoning_tokens {
            format!("{user_message}\n{REASONING_DISABLE_SUFFIX}")
        } else {
            user_message.to_string()
        };

        let request = ChatRequest {
            model: self.config.chat_model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_content,
                },
            ],
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            response_format,
        };

        let resp: ChatResponse = self
            .http
            .post(&url)
            .json(&request)
            .send()
            .map_err(|e| RuntimeError::ServiceUnavailable {
                url: url.clone(),
                reason: e.to_string(),
            })?
            .json()
            .map_err(|e| RuntimeError::LlmRequestFailed(e.to_string()))?;

        let content = resp
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        if content.trim().is_empty() {
            return Err(RuntimeError::EmptyResponse);
        }

        let usage = resp.usage.unwrap_or(Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        });

        Ok(ChatCompletion {
            content,
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        })
    }

    /// Compute embeddings for a list of texts.
    pub fn embed(&self, texts: &[String]) -> RuntimeResult<Vec<Vec<f32>>> {
        let model = self
            .config
            .embedding_model
            .as_ref()
            .ok_or_else(|| RuntimeError::LlmRequestFailed("No embedding model configured".into()))?;

        let url = format!("{}/embeddings", self.config.base_url);

        let request = EmbeddingRequest {
            model: model.clone(),
            input: texts.to_vec(),
        };

        let resp: EmbeddingResponse = self
            .http
            .post(&url)
            .json(&request)
            .send()
            .map_err(|e| RuntimeError::ServiceUnavailable {
                url: url.clone(),
                reason: e.to_string(),
            })?
            .json()
            .map_err(|e| RuntimeError::LlmRequestFailed(e.to_string()))?;

        Ok(resp.data.into_iter().map(|d| d.embedding).collect())
    }

    /// Get a reference to the config.
    pub fn config(&self) -> &LlmClientConfig {
        &self.config
    }
}

/// Compute cosine similarity between two embedding vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_identical_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_opposite_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }
}
