//! LLM-assisted analysis and fix proposal
//!
//! This module handles:
//! - Generating explanations for detected drift
//! - Proposing documentation fixes
//! - Structured prompt generation

mod client;
mod prompts;

pub use client::{LlmClient, LlmConfig, LlmResponse};
pub use prompts::{AnalysisPrompt, FixPrompt};

use crate::drift::DriftEvent;
use crate::extract::{CodeChunk, DocChunk};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Analysis result from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Summary of the change
    pub summary: String,
    /// Why the documentation is incorrect
    pub reason: String,
    /// Suggested updated text
    pub suggested_fix: Option<String>,
    /// Confidence in the analysis (0.0 - 1.0)
    pub confidence: f64,
}

/// Request for LLM analysis
#[derive(Debug, Clone)]
pub struct AnalysisRequest {
    /// The drift event to analyze
    pub drift_event: DriftEvent,
    /// Old code chunk (if available)
    pub old_code: Option<CodeChunk>,
    /// New code chunk (if available)
    pub new_code: Option<CodeChunk>,
    /// Related documentation chunk
    pub doc_chunk: DocChunk,
}

impl AnalysisRequest {
    /// Create a new analysis request
    pub fn new(
        drift_event: DriftEvent,
        old_code: Option<CodeChunk>,
        new_code: Option<CodeChunk>,
        doc_chunk: DocChunk,
    ) -> Self {
        Self {
            drift_event,
            old_code,
            new_code,
            doc_chunk,
        }
    }

    /// Generate the prompt for this request
    pub fn to_prompt(&self) -> String {
        let mut prompt = String::new();

        prompt.push_str("You are analyzing a potential documentation drift issue.\n\n");

        // Add drift event info
        prompt.push_str(&"## Drift Event\n".to_string());
        prompt.push_str(&format!("Severity: {}\n", self.drift_event.severity));
        prompt.push_str(&format!("Description: {}\n", self.drift_event.description));
        prompt.push_str(&format!("Evidence: {}\n\n", self.drift_event.evidence));

        // Add old code if available
        if let Some(ref old) = self.old_code {
            prompt.push_str("## Previous Code\n");
            prompt.push_str(&format!("Symbol: {}\n", old.symbol_name));
            if let Some(ref sig) = old.signature {
                prompt.push_str(&format!("Signature: {}\n", sig));
            }
            if let Some(ref doc) = old.doc_comment {
                prompt.push_str(&format!("Doc comment:\n{}\n", doc));
            }
            prompt.push_str(&format!("```\n{}\n```\n\n", old.content));
        }

        // Add new code if available
        if let Some(ref new) = self.new_code {
            prompt.push_str("## Current Code\n");
            prompt.push_str(&format!("Symbol: {}\n", new.symbol_name));
            if let Some(ref sig) = new.signature {
                prompt.push_str(&format!("Signature: {}\n", sig));
            }
            if let Some(ref doc) = new.doc_comment {
                prompt.push_str(&format!("Doc comment:\n{}\n", doc));
            }
            prompt.push_str(&format!("```\n{}\n```\n\n", new.content));
        }

        // Add documentation
        prompt.push_str("## Current Documentation\n");
        prompt.push_str(&format!("Section: {}\n", self.doc_chunk.full_path()));
        prompt.push_str(&format!("Content:\n{}\n\n", self.doc_chunk.content));

        // Add instructions
        prompt.push_str("## Instructions\n");
        prompt.push_str("Analyze this drift and respond with a JSON object containing:\n");
        prompt.push_str("- summary: Brief summary of what changed\n");
        prompt.push_str("- reason: Why the documentation is now incorrect\n");
        prompt.push_str(
            "- suggested_fix: The corrected documentation text (or null if no fix needed)\n",
        );
        prompt.push_str("- confidence: Your confidence in this analysis (0.0 to 1.0)\n\n");
        prompt.push_str("Respond ONLY with valid JSON, no other text.\n");

        prompt
    }
}

/// Analyzer that uses LLM for drift analysis
pub struct DriftAnalyzer {
    client: LlmClient,
}

impl DriftAnalyzer {
    /// Create a new drift analyzer
    pub fn new(client: LlmClient) -> Self {
        Self { client }
    }

    /// Analyze a drift event
    pub async fn analyze(&self, request: AnalysisRequest) -> Result<AnalysisResult> {
        let prompt = request.to_prompt();
        let response = self.client.complete(&prompt).await?;

        // Parse the JSON response
        let result: AnalysisResult = serde_json::from_str(&response.content)
            .map_err(|e| anyhow::anyhow!("Failed to parse LLM response: {}", e))?;

        Ok(result)
    }

    /// Analyze multiple drift events
    pub async fn analyze_batch(
        &self,
        requests: Vec<AnalysisRequest>,
    ) -> Result<Vec<AnalysisResult>> {
        let mut results = Vec::with_capacity(requests.len());

        for request in requests {
            match self.analyze(request).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    tracing::warn!("Failed to analyze drift event: {}", e);
                    // Add a placeholder result
                    results.push(AnalysisResult {
                        summary: "Analysis failed".to_string(),
                        reason: e.to_string(),
                        suggested_fix: None,
                        confidence: 0.0,
                    });
                }
            }
        }

        Ok(results)
    }
}

/// Generate a fix suggestion without LLM (rule-based)
pub fn generate_simple_fix(
    _drift_event: &DriftEvent,
    old_code: Option<&CodeChunk>,
    new_code: Option<&CodeChunk>,
    doc_chunk: &DocChunk,
) -> Option<String> {
    // Handle removed function
    if old_code.is_some() && new_code.is_none() {
        return Some(format!(
            "<!-- WARNING: The function '{}' has been removed. This documentation section may need to be updated or removed. -->\n\n{}",
            old_code.unwrap().symbol_name,
            doc_chunk.content
        ));
    }

    // Handle signature change
    if let (Some(old), Some(new)) = (old_code, new_code) {
        if old.signature != new.signature {
            if let Some(ref new_sig) = new.signature {
                // Try to update signature in documentation
                if let Some(ref old_sig) = old.signature {
                    let updated = doc_chunk.content.replace(old_sig, new_sig);
                    if updated != doc_chunk.content {
                        return Some(updated);
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift::DriftSeverity;
    use crate::extract::code::{Language, SymbolType};
    use crate::extract::doc::HeadingLevel;

    #[test]
    fn test_analysis_request_prompt() {
        let event = DriftEvent::new(
            DriftSeverity::High,
            "Signature changed",
            "Parameter added",
            0.9,
        );

        let code = CodeChunk::new(
            "test.rs",
            "my_func",
            SymbolType::Function,
            "fn my_func(x: i32) {}",
            Language::Rust,
            1,
            1,
        );

        let doc = DocChunk::new(
            "README.md",
            vec!["API".to_string()],
            "API",
            HeadingLevel::H2,
            "The my_func function does something.",
            1,
            5,
        );

        let request = AnalysisRequest::new(event, None, Some(code), doc);
        let prompt = request.to_prompt();

        assert!(prompt.contains("Signature changed"));
        assert!(prompt.contains("my_func"));
        assert!(prompt.contains("JSON"));
    }
}
