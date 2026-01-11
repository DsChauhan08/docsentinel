//! LLM client for API communication

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Response from LLM
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// The generated content
    pub content: String,
    /// Number of tokens used
    pub tokens_used: Option<usize>,
}

/// Configuration for LLM client
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// API endpoint URL
    pub endpoint: String,
    /// Model name
    pub model: String,
    /// API key (optional)
    pub api_key: Option<String>,
    /// Maximum tokens for response
    pub max_tokens: usize,
    /// Temperature for generation
    pub temperature: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434".to_string(),
            model: "llama2".to_string(),
            api_key: None,
            max_tokens: 2048,
            temperature: 0.3,
        }
    }
}

/// LLM client for generating analysis and fixes
pub struct LlmClient {
    config: LlmConfig,
    client: reqwest::Client,
}

impl LlmClient {
    /// Create a new LLM client
    pub fn new(config: LlmConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Create with Ollama defaults
    pub fn ollama(model: &str) -> Self {
        Self::new(LlmConfig {
            endpoint: "http://localhost:11434".to_string(),
            model: model.to_string(),
            ..Default::default()
        })
    }

    /// Create with OpenAI-compatible endpoint
    pub fn openai_compatible(endpoint: &str, model: &str, api_key: Option<&str>) -> Self {
        Self::new(LlmConfig {
            endpoint: endpoint.to_string(),
            model: model.to_string(),
            api_key: api_key.map(|s| s.to_string()),
            ..Default::default()
        })
    }

    /// Check if the LLM service is available
    pub async fn is_available(&self) -> bool {
        let url = if self.config.endpoint.contains("11434") {
            // Ollama
            format!("{}/api/tags", self.config.endpoint)
        } else {
            // OpenAI-compatible
            format!("{}/v1/models", self.config.endpoint)
        };

        self.client.get(&url).send().await.is_ok()
    }

    /// Generate a completion
    pub async fn complete(&self, prompt: &str) -> Result<LlmResponse> {
        if self.config.endpoint.contains("11434") {
            self.complete_ollama(prompt).await
        } else {
            self.complete_openai(prompt).await
        }
    }

    /// Generate completion using Ollama API
    async fn complete_ollama(&self, prompt: &str) -> Result<LlmResponse> {
        let url = format!("{}/api/generate", self.config.endpoint);

        let request = OllamaGenerateRequest {
            model: self.config.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
            options: OllamaOptions {
                temperature: self.config.temperature,
                num_predict: self.config.max_tokens as i32,
            },
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Ollama")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama request failed: {} - {}", status, body);
        }

        let result: OllamaGenerateResponse = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        Ok(LlmResponse {
            content: result.response,
            tokens_used: Some(result.eval_count.unwrap_or(0) as usize),
        })
    }

    /// Generate completion using OpenAI-compatible API
    async fn complete_openai(&self, prompt: &str) -> Result<LlmResponse> {
        let url = format!("{}/v1/chat/completions", self.config.endpoint);

        let request = OpenAIChatRequest {
            model: self.config.model.clone(),
            messages: vec![OpenAIMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: Some(self.config.max_tokens),
            temperature: Some(self.config.temperature),
        };

        let mut req_builder = self.client.post(&url).json(&request);

        if let Some(ref key) = self.config.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
        }

        let response = req_builder
            .send()
            .await
            .context("Failed to send request to OpenAI-compatible API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI request failed: {} - {}", status, body);
        }

        let result: OpenAIChatResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        let content = result
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let tokens_used = result.usage.map(|u| u.total_tokens as usize);

        Ok(LlmResponse {
            content,
            tokens_used,
        })
    }

    /// Generate completion with retry
    pub async fn complete_with_retry(
        &self,
        prompt: &str,
        max_retries: usize,
    ) -> Result<LlmResponse> {
        let mut last_error = None;

        for attempt in 0..max_retries {
            match self.complete(prompt).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    tracing::warn!("LLM request failed (attempt {}): {}", attempt + 1, e);
                    last_error = Some(e);

                    // Wait before retry
                    tokio::time::sleep(tokio::time::Duration::from_millis(
                        500 * (attempt as u64 + 1),
                    ))
                    .await;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error")))
    }
}

// Ollama API types

#[derive(Debug, Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: i32,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
    eval_count: Option<i32>,
}

// OpenAI API types

#[derive(Debug, Serialize)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatResponse {
    choices: Vec<OpenAIChoice>,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    total_tokens: i32,
}

/// Mock LLM client for testing
pub struct MockLlmClient {
    responses: std::collections::HashMap<String, String>,
}

impl MockLlmClient {
    /// Create a new mock client
    pub fn new() -> Self {
        Self {
            responses: std::collections::HashMap::new(),
        }
    }

    /// Add a mock response
    pub fn add_response(&mut self, prompt_contains: &str, response: &str) {
        self.responses
            .insert(prompt_contains.to_string(), response.to_string());
    }

    /// Generate a mock completion
    pub fn complete(&self, prompt: &str) -> Result<LlmResponse> {
        for (key, response) in &self.responses {
            if prompt.contains(key) {
                return Ok(LlmResponse {
                    content: response.clone(),
                    tokens_used: Some(100),
                });
            }
        }

        // Default response
        Ok(LlmResponse {
            content: r#"{"summary": "Mock analysis", "reason": "Mock reason", "suggested_fix": null, "confidence": 0.5}"#.to_string(),
            tokens_used: Some(50),
        })
    }
}

impl Default for MockLlmClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_client() {
        let mut client = MockLlmClient::new();
        client.add_response("test", r#"{"result": "success"}"#);

        let response = client.complete("this is a test prompt").unwrap();
        assert!(response.content.contains("success"));
    }

    #[test]
    fn test_default_config() {
        let config = LlmConfig::default();
        assert!(config.endpoint.contains("11434"));
        assert!(config.temperature > 0.0);
    }
}
