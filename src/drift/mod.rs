//! Drift detection engine
//!
//! This module detects when documentation no longer matches code by:
//! - Comparing semantic relationships before and after changes
//! - Applying hard drift rules (API changes, removed functions)
//! - Applying soft drift rules (behavioral changes)

mod detector;
mod embedding;
mod rules;

pub use detector::DriftDetector;
pub use embedding::{EmbeddingProvider, LocalEmbedding};
pub use rules::{DriftRule, HardDriftRules, SoftDriftRules};

use serde::{Deserialize, Serialize};

/// Severity level of a drift event
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DriftSeverity {
    /// Critical: Public API changed without doc update
    Critical,
    /// High: Function signature changed
    High,
    /// Medium: Behavioral change detected
    Medium,
    /// Low: Minor inconsistency
    Low,
}

impl std::fmt::Display for DriftSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriftSeverity::Critical => write!(f, "CRITICAL"),
            DriftSeverity::High => write!(f, "HIGH"),
            DriftSeverity::Medium => write!(f, "MEDIUM"),
            DriftSeverity::Low => write!(f, "LOW"),
        }
    }
}

/// Status of a drift event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftStatus {
    /// Pending review
    Pending,
    /// Fix accepted
    Accepted,
    /// Ignored by user
    Ignored,
    /// Fixed and committed
    Fixed,
}

impl std::fmt::Display for DriftStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriftStatus::Pending => write!(f, "Pending"),
            DriftStatus::Accepted => write!(f, "Accepted"),
            DriftStatus::Ignored => write!(f, "Ignored"),
            DriftStatus::Fixed => write!(f, "Fixed"),
        }
    }
}

/// A detected drift event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftEvent {
    /// Unique identifier
    pub id: String,
    /// Severity level
    pub severity: DriftSeverity,
    /// Human-readable description
    pub description: String,
    /// Evidence supporting the drift detection
    pub evidence: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Related code chunk IDs
    pub related_code_chunks: Vec<String>,
    /// Related doc chunk IDs
    pub related_doc_chunks: Vec<String>,
    /// Suggested fix (if available)
    pub suggested_fix: Option<String>,
    /// Current status
    pub status: DriftStatus,
}

impl DriftEvent {
    /// Create a new drift event
    pub fn new(
        severity: DriftSeverity,
        description: &str,
        evidence: &str,
        confidence: f64,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            severity,
            description: description.to_string(),
            evidence: evidence.to_string(),
            confidence,
            related_code_chunks: Vec::new(),
            related_doc_chunks: Vec::new(),
            suggested_fix: None,
            status: DriftStatus::Pending,
        }
    }

    /// Add a related code chunk
    pub fn with_code_chunk(mut self, chunk_id: &str) -> Self {
        self.related_code_chunks.push(chunk_id.to_string());
        self
    }

    /// Add a related doc chunk
    pub fn with_doc_chunk(mut self, chunk_id: &str) -> Self {
        self.related_doc_chunks.push(chunk_id.to_string());
        self
    }

    /// Set the suggested fix
    pub fn with_suggested_fix(mut self, fix: &str) -> Self {
        self.suggested_fix = Some(fix.to_string());
        self
    }
}

/// Result of comparing two chunks
#[derive(Debug, Clone)]
pub struct SimilarityResult {
    /// Code chunk ID
    pub code_chunk_id: String,
    /// Doc chunk ID
    pub doc_chunk_id: String,
    /// Similarity score (0.0 - 1.0)
    pub similarity: f64,
    /// Previous similarity (if known)
    pub previous_similarity: Option<f64>,
}

impl SimilarityResult {
    /// Check if similarity dropped significantly
    pub fn has_significant_drop(&self, threshold: f64) -> bool {
        if let Some(prev) = self.previous_similarity {
            (prev - self.similarity) > threshold
        } else {
            false
        }
    }
}

/// Compute cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot_product: f64 = a.iter().zip(b.iter()).map(|(x, y)| (*x as f64) * (*y as f64)).sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_drift_event_creation() {
        let event = DriftEvent::new(
            DriftSeverity::High,
            "Function signature changed",
            "Parameter 'name' was removed",
            0.95,
        )
        .with_code_chunk("src/lib.rs::my_function")
        .with_doc_chunk("README.md#Usage");

        assert_eq!(event.severity, DriftSeverity::High);
        assert_eq!(event.related_code_chunks.len(), 1);
        assert_eq!(event.related_doc_chunks.len(), 1);
    }
}
