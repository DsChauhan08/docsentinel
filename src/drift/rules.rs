//! Drift detection rules
//!
//! Hard rules: Definite drift (API changes, removed functions)
//! Soft rules: Possible drift (behavioral changes, comment changes)

use super::{DriftEvent, DriftSeverity};
use crate::extract::{CodeChunk, DocChunk};

/// Trait for drift detection rules
pub trait DriftRule: Send + Sync {
    /// Rule name
    fn name(&self) -> &str;

    /// Check if this rule applies to the given code change
    fn check_code_change(
        &self,
        old_chunk: Option<&CodeChunk>,
        new_chunk: Option<&CodeChunk>,
        related_docs: &[&DocChunk],
    ) -> Option<DriftEvent>;

    /// Check if this rule applies to the given doc change
    fn check_doc_change(
        &self,
        old_chunk: Option<&DocChunk>,
        new_chunk: Option<&DocChunk>,
        related_code: &[&CodeChunk],
    ) -> Option<DriftEvent>;
}

/// Collection of hard drift rules
pub struct HardDriftRules {
    rules: Vec<Box<dyn DriftRule>>,
}

impl HardDriftRules {
    /// Create default hard drift rules
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(SignatureChangeRule),
                Box::new(RemovedFunctionRule),
                Box::new(ParameterChangeRule),
                Box::new(ReturnTypeChangeRule),
            ],
        }
    }

    /// Check all rules against a code change
    pub fn check_code_change(
        &self,
        old_chunk: Option<&CodeChunk>,
        new_chunk: Option<&CodeChunk>,
        related_docs: &[&DocChunk],
    ) -> Vec<DriftEvent> {
        self.rules
            .iter()
            .filter_map(|rule| rule.check_code_change(old_chunk, new_chunk, related_docs))
            .collect()
    }

    /// Check all rules against a doc change
    pub fn check_doc_change(
        &self,
        old_chunk: Option<&DocChunk>,
        new_chunk: Option<&DocChunk>,
        related_code: &[&CodeChunk],
    ) -> Vec<DriftEvent> {
        self.rules
            .iter()
            .filter_map(|rule| rule.check_doc_change(old_chunk, new_chunk, related_code))
            .collect()
    }
}

impl Default for HardDriftRules {
    fn default() -> Self {
        Self::new()
    }
}

/// Collection of soft drift rules
pub struct SoftDriftRules {
    rules: Vec<Box<dyn DriftRule>>,
}

impl SoftDriftRules {
    /// Create default soft drift rules
    pub fn new() -> Self {
        Self {
            rules: vec![Box::new(DocCommentChangeRule), Box::new(BehaviorChangeRule)],
        }
    }

    /// Check all rules against a code change
    pub fn check_code_change(
        &self,
        old_chunk: Option<&CodeChunk>,
        new_chunk: Option<&CodeChunk>,
        related_docs: &[&DocChunk],
    ) -> Vec<DriftEvent> {
        self.rules
            .iter()
            .filter_map(|rule| rule.check_code_change(old_chunk, new_chunk, related_docs))
            .collect()
    }
}

impl Default for SoftDriftRules {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== Hard Rules ====================

/// Detects when a function signature changes without doc update
struct SignatureChangeRule;

impl DriftRule for SignatureChangeRule {
    fn name(&self) -> &str {
        "signature_change"
    }

    fn check_code_change(
        &self,
        old_chunk: Option<&CodeChunk>,
        new_chunk: Option<&CodeChunk>,
        related_docs: &[&DocChunk],
    ) -> Option<DriftEvent> {
        let old = old_chunk?;
        let new = new_chunk?;

        // Only check public functions/methods
        if !new.is_public {
            return None;
        }

        // Check if signature changed
        let old_sig = old.signature.as_ref()?;
        let new_sig = new.signature.as_ref()?;

        if old_sig == new_sig {
            return None;
        }

        // Check if any related docs mention this function
        let has_related_docs = !related_docs.is_empty();

        if has_related_docs {
            let evidence = format!("Signature changed from:\n  {}\nto:\n  {}", old_sig, new_sig);

            let mut event = DriftEvent::new(
                DriftSeverity::High,
                &format!("Public API signature changed: {}", new.symbol_name),
                &evidence,
                0.95,
            )
            .with_code_chunk(&new.id);

            for doc in related_docs {
                event = event.with_doc_chunk(&doc.id);
            }

            return Some(event);
        }

        None
    }

    fn check_doc_change(
        &self,
        _old_chunk: Option<&DocChunk>,
        _new_chunk: Option<&DocChunk>,
        _related_code: &[&CodeChunk],
    ) -> Option<DriftEvent> {
        None
    }
}

/// Detects when a documented function is removed
struct RemovedFunctionRule;

impl DriftRule for RemovedFunctionRule {
    fn name(&self) -> &str {
        "removed_function"
    }

    fn check_code_change(
        &self,
        old_chunk: Option<&CodeChunk>,
        new_chunk: Option<&CodeChunk>,
        related_docs: &[&DocChunk],
    ) -> Option<DriftEvent> {
        let old = old_chunk?;

        // Function was removed (new_chunk is None)
        if new_chunk.is_some() {
            return None;
        }

        // Only care about public functions
        if !old.is_public {
            return None;
        }

        // Check if any docs reference this function
        let has_related_docs = !related_docs.is_empty();

        if has_related_docs {
            let evidence = format!(
                "Function '{}' was removed but is still documented",
                old.symbol_name
            );

            let mut event = DriftEvent::new(
                DriftSeverity::Critical,
                &format!("Documented function removed: {}", old.symbol_name),
                &evidence,
                1.0,
            )
            .with_code_chunk(&old.id);

            for doc in related_docs {
                event = event.with_doc_chunk(&doc.id);
            }

            return Some(event);
        }

        None
    }

    fn check_doc_change(
        &self,
        _old_chunk: Option<&DocChunk>,
        _new_chunk: Option<&DocChunk>,
        _related_code: &[&CodeChunk],
    ) -> Option<DriftEvent> {
        None
    }
}

/// Detects when function parameters change
struct ParameterChangeRule;

impl DriftRule for ParameterChangeRule {
    fn name(&self) -> &str {
        "parameter_change"
    }

    fn check_code_change(
        &self,
        old_chunk: Option<&CodeChunk>,
        new_chunk: Option<&CodeChunk>,
        related_docs: &[&DocChunk],
    ) -> Option<DriftEvent> {
        let old = old_chunk?;
        let new = new_chunk?;

        if !new.is_public {
            return None;
        }

        let old_sig = old.signature.as_ref()?;
        let new_sig = new.signature.as_ref()?;

        // Extract parameters from signatures
        let old_params = extract_parameters(old_sig);
        let new_params = extract_parameters(new_sig);

        if old_params == new_params {
            return None;
        }

        // Find specific changes
        let added: Vec<String> = new_params
            .iter()
            .filter(|p| !old_params.contains(p))
            .cloned()
            .collect();
        let removed: Vec<String> = old_params
            .iter()
            .filter(|p| !new_params.contains(p))
            .cloned()
            .collect();

        if added.is_empty() && removed.is_empty() {
            return None;
        }

        let has_related_docs = !related_docs.is_empty();

        if has_related_docs {
            let mut evidence_parts = Vec::new();
            if !added.is_empty() {
                evidence_parts.push(format!("Added parameters: {}", added.join(", ")));
            }
            if !removed.is_empty() {
                evidence_parts.push(format!("Removed parameters: {}", removed.join(", ")));
            }

            let mut event = DriftEvent::new(
                DriftSeverity::High,
                &format!("Parameters changed for: {}", new.symbol_name),
                &evidence_parts.join("\n"),
                0.9,
            )
            .with_code_chunk(&new.id);

            for doc in related_docs {
                event = event.with_doc_chunk(&doc.id);
            }

            return Some(event);
        }

        None
    }

    fn check_doc_change(
        &self,
        _old_chunk: Option<&DocChunk>,
        _new_chunk: Option<&DocChunk>,
        _related_code: &[&CodeChunk],
    ) -> Option<DriftEvent> {
        None
    }
}

/// Detects when return type changes
struct ReturnTypeChangeRule;

impl DriftRule for ReturnTypeChangeRule {
    fn name(&self) -> &str {
        "return_type_change"
    }

    fn check_code_change(
        &self,
        old_chunk: Option<&CodeChunk>,
        new_chunk: Option<&CodeChunk>,
        related_docs: &[&DocChunk],
    ) -> Option<DriftEvent> {
        let old = old_chunk?;
        let new = new_chunk?;

        if !new.is_public {
            return None;
        }

        let old_sig = old.signature.as_ref()?;
        let new_sig = new.signature.as_ref()?;

        let old_return = extract_return_type(old_sig);
        let new_return = extract_return_type(new_sig);

        if old_return == new_return {
            return None;
        }

        let has_related_docs = !related_docs.is_empty();

        if has_related_docs {
            let evidence = format!(
                "Return type changed from '{}' to '{}'",
                old_return.as_deref().unwrap_or("none"),
                new_return.as_deref().unwrap_or("none")
            );

            let mut event = DriftEvent::new(
                DriftSeverity::High,
                &format!("Return type changed for: {}", new.symbol_name),
                &evidence,
                0.9,
            )
            .with_code_chunk(&new.id);

            for doc in related_docs {
                event = event.with_doc_chunk(&doc.id);
            }

            return Some(event);
        }

        None
    }

    fn check_doc_change(
        &self,
        _old_chunk: Option<&DocChunk>,
        _new_chunk: Option<&DocChunk>,
        _related_code: &[&CodeChunk],
    ) -> Option<DriftEvent> {
        None
    }
}

// ==================== Soft Rules ====================

/// Detects when doc comments change significantly
struct DocCommentChangeRule;

impl DriftRule for DocCommentChangeRule {
    fn name(&self) -> &str {
        "doc_comment_change"
    }

    fn check_code_change(
        &self,
        old_chunk: Option<&CodeChunk>,
        new_chunk: Option<&CodeChunk>,
        related_docs: &[&DocChunk],
    ) -> Option<DriftEvent> {
        let old = old_chunk?;
        let new = new_chunk?;

        if !new.is_public {
            return None;
        }

        let old_doc = old.doc_comment.as_ref()?;
        let new_doc = new.doc_comment.as_ref()?;

        if old_doc == new_doc {
            return None;
        }

        // Check if the change is significant (more than just whitespace)
        let old_normalized: String = old_doc.split_whitespace().collect();
        let new_normalized: String = new_doc.split_whitespace().collect();

        if old_normalized == new_normalized {
            return None;
        }

        let has_related_docs = !related_docs.is_empty();

        if has_related_docs {
            let evidence = format!(
                "Doc comment changed for '{}'. External documentation may need update.",
                new.symbol_name
            );

            let mut event = DriftEvent::new(
                DriftSeverity::Medium,
                &format!("Doc comment changed: {}", new.symbol_name),
                &evidence,
                0.7,
            )
            .with_code_chunk(&new.id);

            for doc in related_docs {
                event = event.with_doc_chunk(&doc.id);
            }

            return Some(event);
        }

        None
    }

    fn check_doc_change(
        &self,
        _old_chunk: Option<&DocChunk>,
        _new_chunk: Option<&DocChunk>,
        _related_code: &[&CodeChunk],
    ) -> Option<DriftEvent> {
        None
    }
}

/// Detects potential behavioral changes from code modifications
struct BehaviorChangeRule;

impl DriftRule for BehaviorChangeRule {
    fn name(&self) -> &str {
        "behavior_change"
    }

    fn check_code_change(
        &self,
        old_chunk: Option<&CodeChunk>,
        new_chunk: Option<&CodeChunk>,
        related_docs: &[&DocChunk],
    ) -> Option<DriftEvent> {
        let old = old_chunk?;
        let new = new_chunk?;

        if !new.is_public {
            return None;
        }

        // Signature didn't change but content did
        if old.signature == new.signature && old.content != new.content {
            // Look for behavioral indicators
            let behavior_keywords = [
                "default", "error", "panic", "return", "throw", "raise", "assert", "expect",
                "unwrap", "if", "else", "match",
            ];

            let old_has_keyword = behavior_keywords.iter().any(|k| old.content.contains(k));
            let new_has_keyword = behavior_keywords.iter().any(|k| new.content.contains(k));

            // If behavioral keywords changed, might indicate behavior change
            if old_has_keyword != new_has_keyword {
                let has_related_docs = !related_docs.is_empty();

                if has_related_docs {
                    let evidence = format!(
                        "Implementation of '{}' changed. Behavior may have changed.",
                        new.symbol_name
                    );

                    let mut event = DriftEvent::new(
                        DriftSeverity::Low,
                        &format!("Potential behavior change: {}", new.symbol_name),
                        &evidence,
                        0.5,
                    )
                    .with_code_chunk(&new.id);

                    for doc in related_docs {
                        event = event.with_doc_chunk(&doc.id);
                    }

                    return Some(event);
                }
            }
        }

        None
    }

    fn check_doc_change(
        &self,
        _old_chunk: Option<&DocChunk>,
        _new_chunk: Option<&DocChunk>,
        _related_code: &[&CodeChunk],
    ) -> Option<DriftEvent> {
        None
    }
}

// ==================== Helper Functions ====================

/// Extract parameter names from a function signature
fn extract_parameters(signature: &str) -> Vec<String> {
    // Find content between parentheses
    let start = signature.find('(').unwrap_or(0);
    let end = signature.rfind(')').unwrap_or(signature.len());

    if start >= end {
        return Vec::new();
    }

    let params_str = &signature[start + 1..end];

    params_str
        .split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| {
            // Extract just the parameter name (before : or =)
            p.split(':')
                .next()
                .unwrap_or(p)
                .split('=')
                .next()
                .unwrap_or(p)
                .trim()
                .to_string()
        })
        .collect()
}

/// Extract return type from a function signature
fn extract_return_type(signature: &str) -> Option<String> {
    // Look for -> in the signature
    if let Some(arrow_pos) = signature.find("->") {
        let return_part = &signature[arrow_pos + 2..];
        // Clean up the return type
        let return_type = return_part.trim().trim_end_matches('{').trim().to_string();
        if !return_type.is_empty() {
            return Some(return_type);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_parameters() {
        let sig = "fn hello(name: &str, age: u32) -> String";
        let params = extract_parameters(sig);
        assert_eq!(params, vec!["name", "age"]);

        let sig2 = "def greet(name, greeting='Hello')";
        let params2 = extract_parameters(sig2);
        assert_eq!(params2, vec!["name", "greeting"]);
    }

    #[test]
    fn test_extract_return_type() {
        let sig = "fn hello(name: &str) -> String";
        assert_eq!(extract_return_type(sig), Some("String".to_string()));

        let sig2 = "fn void_func()";
        assert_eq!(extract_return_type(sig2), None);
    }
}
