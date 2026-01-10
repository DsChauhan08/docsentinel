//! Embedding generation for semantic similarity
//!
//! Supports local embedding via Ollama or compatible OpenAI-style endpoints

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Trait for embedding providers
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embeddings for a batch of texts
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;

    /// Generate embedding for a single text
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let results = self.embed_batch(&[text.to_string()]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No embedding returned"))
    }

    /// Get the embedding dimension
    fn dimension(&self) -> usize;
}

/// Local embedding provider using Ollama or compatible API
pub struct LocalEmbedding {
    /// API endpoint URL
    endpoint: String,
    /// Model name
    model: String,
    /// HTTP client
    client: reqwest::Client,
    /// Embedding dimension
    dimension: usize,
}

impl LocalEmbedding {
    /// Create a new local embedding provider
    pub fn new(endpoint: &str, model: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            model: model.to_string(),
            client: reqwest::Client::new(),
            dimension: 384, // Default for many sentence-transformer models
        }
    }

    /// Create with Ollama defaults
    pub fn ollama(model: &str) -> Self {
        Self::new("http://localhost:11434", model)
    }

    /// Set the embedding dimension
    pub fn with_dimension(mut self, dim: usize) -> Self {
        self.dimension = dim;
        self
    }

    /// Check if the embedding service is available
    pub async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.endpoint);
        self.client.get(&url).send().await.is_ok()
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for LocalEmbedding {
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::with_capacity(texts.len());

        for text in texts {
            let embedding = self.embed_single(text).await?;
            embeddings.push(embedding);
        }

        Ok(embeddings)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

impl LocalEmbedding {
    /// Embed a single text using Ollama API
    async fn embed_single(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.endpoint);

        let request = OllamaEmbeddingRequest {
            model: self.model.clone(),
            prompt: text.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send embedding request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Embedding request failed: {} - {}", status, body);
        }

        let result: OllamaEmbeddingResponse = response
            .json()
            .await
            .context("Failed to parse embedding response")?;

        Ok(result.embedding)
    }
}

/// Ollama embedding request
#[derive(Debug, Serialize)]
struct OllamaEmbeddingRequest {
    model: String,
    prompt: String,
}

/// Ollama embedding response
#[derive(Debug, Deserialize)]
struct OllamaEmbeddingResponse {
    embedding: Vec<f32>,
}

/// OpenAI-compatible embedding provider
pub struct OpenAIEmbedding {
    /// API endpoint URL
    endpoint: String,
    /// Model name
    model: String,
    /// API key
    api_key: Option<String>,
    /// HTTP client
    client: reqwest::Client,
    /// Embedding dimension
    dimension: usize,
}

impl OpenAIEmbedding {
    /// Create a new OpenAI-compatible embedding provider
    pub fn new(endpoint: &str, model: &str, api_key: Option<&str>) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            model: model.to_string(),
            api_key: api_key.map(|s| s.to_string()),
            client: reqwest::Client::new(),
            dimension: 1536, // Default for text-embedding-ada-002
        }
    }

    /// Set the embedding dimension
    pub fn with_dimension(mut self, dim: usize) -> Self {
        self.dimension = dim;
        self
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for OpenAIEmbedding {
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let url = format!("{}/v1/embeddings", self.endpoint);

        let request = OpenAIEmbeddingRequest {
            model: self.model.clone(),
            input: texts.to_vec(),
        };

        let mut req_builder = self.client.post(&url).json(&request);

        if let Some(ref key) = self.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
        }

        let response = req_builder
            .send()
            .await
            .context("Failed to send embedding request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Embedding request failed: {} - {}", status, body);
        }

        let result: OpenAIEmbeddingResponse = response
            .json()
            .await
            .context("Failed to parse embedding response")?;

        let mut embeddings: Vec<_> = result
            .data
            .into_iter()
            .map(|d| (d.index, d.embedding))
            .collect();

        // Sort by index to maintain order
        embeddings.sort_by_key(|(idx, _)| *idx);

        Ok(embeddings.into_iter().map(|(_, e)| e).collect())
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

/// OpenAI embedding request
#[derive(Debug, Serialize)]
struct OpenAIEmbeddingRequest {
    model: String,
    input: Vec<String>,
}

/// OpenAI embedding response
#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<OpenAIEmbeddingData>,
}

/// OpenAI embedding data item
#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingData {
    index: usize,
    embedding: Vec<f32>,
}

/// Mock embedding provider for testing
pub struct MockEmbedding {
    dimension: usize,
}

impl MockEmbedding {
    /// Create a new mock embedding provider
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for MockEmbedding {
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // Generate deterministic embeddings based on text hash
        Ok(texts
            .iter()
            .map(|text| {
                let hash = crate::extract::content_hash(text);
                let bytes = hash.as_bytes();

                (0..self.dimension)
                    .map(|i| {
                        let byte = bytes[i % bytes.len()] as f32;
                        (byte / 255.0) * 2.0 - 1.0 // Normalize to [-1, 1]
                    })
                    .collect()
            })
            .collect())
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

// We need to add async-trait to Cargo.toml
// For now, let's use a simpler synchronous approach

/// Synchronous embedding trait for simpler usage
pub trait SyncEmbeddingProvider {
    /// Generate embedding for a single text
    fn embed_sync(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embeddings for a batch of texts
    fn embed_batch_sync(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.embed_sync(t)).collect()
    }

    /// Get the embedding dimension
    fn dimension(&self) -> usize;
}

impl SyncEmbeddingProvider for MockEmbedding {
    fn embed_sync(&self, text: &str) -> Result<Vec<f32>> {
        let hash = crate::extract::content_hash(text);
        let bytes = hash.as_bytes();

        Ok((0..self.dimension)
            .map(|i| {
                let byte = bytes[i % bytes.len()] as f32;
                (byte / 255.0) * 2.0 - 1.0
            })
            .collect())
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_embedding() {
        let provider = MockEmbedding::new(384);
        let embedding = provider.embed_sync("Hello, world!").unwrap();
        assert_eq!(embedding.len(), 384);

        // Same text should produce same embedding
        let embedding2 = provider.embed_sync("Hello, world!").unwrap();
        assert_eq!(embedding, embedding2);

        // Different text should produce different embedding
        let embedding3 = provider.embed_sync("Goodbye, world!").unwrap();
        assert_ne!(embedding, embedding3);
    }
}
