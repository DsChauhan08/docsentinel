//! Prompt templates for LLM interactions

use crate::drift::DriftEvent;
use crate::extract::{CodeChunk, DocChunk};

/// Prompt for analyzing drift
pub struct AnalysisPrompt;

impl AnalysisPrompt {
    /// Generate a prompt for drift analysis
    pub fn generate(
        drift_event: &DriftEvent,
        old_code: Option<&CodeChunk>,
        new_code: Option<&CodeChunk>,
        doc_chunk: &DocChunk,
    ) -> String {
        let mut prompt = String::new();

        prompt.push_str(ANALYSIS_SYSTEM_PROMPT);
        prompt.push('\n');

        // Add context
        prompt.push_str("## Context\n\n");
        prompt.push_str(&format!("**Drift Type:** {}\n", drift_event.severity));
        prompt.push_str(&format!("**Description:** {}\n", drift_event.description));
        prompt.push_str(&format!("**Evidence:** {}\n\n", drift_event.evidence));

        // Add code information
        if let Some(old) = old_code {
            prompt.push_str("### Previous Code\n\n");
            prompt.push_str(&format!("**File:** `{}`\n", old.file_path));
            prompt.push_str(&format!("**Symbol:** `{}`\n", old.symbol_name));
            if let Some(ref sig) = old.signature {
                prompt.push_str(&format!("**Signature:** `{}`\n", sig));
            }
            if let Some(ref doc) = old.doc_comment {
                prompt.push_str(&format!("**Doc Comment:**\n```\n{}\n```\n", doc));
            }
            prompt.push_str(&format!(
                "\n**Code:**\n```{}\n{}\n```\n\n",
                old.language, old.content
            ));
        }

        if let Some(new) = new_code {
            prompt.push_str("### Current Code\n\n");
            prompt.push_str(&format!("**File:** `{}`\n", new.file_path));
            prompt.push_str(&format!("**Symbol:** `{}`\n", new.symbol_name));
            if let Some(ref sig) = new.signature {
                prompt.push_str(&format!("**Signature:** `{}`\n", sig));
            }
            if let Some(ref doc) = new.doc_comment {
                prompt.push_str(&format!("**Doc Comment:**\n```\n{}\n```\n", doc));
            }
            prompt.push_str(&format!(
                "\n**Code:**\n```{}\n{}\n```\n\n",
                new.language, new.content
            ));
        }

        // Add documentation
        prompt.push_str("### Documentation Section\n\n");
        prompt.push_str(&format!("**File:** `{}`\n", doc_chunk.file_path));
        prompt.push_str(&format!("**Section:** {}\n", doc_chunk.full_path()));
        prompt.push_str(&format!(
            "\n**Content:**\n```markdown\n{}\n```\n\n",
            doc_chunk.content
        ));

        // Add instructions
        prompt.push_str(ANALYSIS_INSTRUCTIONS);

        prompt
    }
}

/// Prompt for generating fixes
pub struct FixPrompt;

impl FixPrompt {
    /// Generate a prompt for fix generation
    pub fn generate(
        drift_event: &DriftEvent,
        new_code: &CodeChunk,
        doc_chunk: &DocChunk,
    ) -> String {
        let mut prompt = String::new();

        prompt.push_str(FIX_SYSTEM_PROMPT);
        prompt.push('\n');

        // Add context
        prompt.push_str("## Issue\n\n");
        prompt.push_str(&format!("{}\n\n", drift_event.description));
        prompt.push_str(&format!("**Evidence:** {}\n\n", drift_event.evidence));

        // Add current code
        prompt.push_str("## Current Code\n\n");
        prompt.push_str(&format!("**Symbol:** `{}`\n", new_code.symbol_name));
        if let Some(ref sig) = new_code.signature {
            prompt.push_str(&format!("**Signature:** `{}`\n", sig));
        }
        if let Some(ref doc) = new_code.doc_comment {
            prompt.push_str(&format!("**Doc Comment:**\n```\n{}\n```\n", doc));
        }
        prompt.push_str(&format!(
            "\n```{}\n{}\n```\n\n",
            new_code.language, new_code.content
        ));

        // Add documentation to fix
        prompt.push_str("## Documentation to Update\n\n");
        prompt.push_str(&format!("**Section:** {}\n\n", doc_chunk.full_path()));
        prompt.push_str(&format!("```markdown\n{}\n```\n\n", doc_chunk.content));

        // Add instructions
        prompt.push_str(FIX_INSTRUCTIONS);

        prompt
    }
}

const ANALYSIS_SYSTEM_PROMPT: &str = r#"You are a documentation drift analyzer. Your task is to analyze potential inconsistencies between code and documentation.

You will be given:
1. Information about a detected drift event
2. The previous and/or current code
3. The related documentation section

Your job is to determine:
1. Whether the documentation is actually inconsistent with the code
2. What specifically is wrong
3. How confident you are in this assessment
"#;

const ANALYSIS_INSTRUCTIONS: &str = r#"## Instructions

Analyze the drift and respond with a JSON object containing exactly these fields:

```json
{
  "summary": "Brief one-line summary of the issue",
  "reason": "Detailed explanation of why the documentation is incorrect",
  "suggested_fix": "The corrected documentation text, or null if no fix is needed",
  "confidence": 0.95
}
```

Guidelines:
- Be specific about what is wrong
- If the documentation is actually correct, set confidence to 0.0 and explain why
- The suggested_fix should be the complete corrected section, not a diff
- Confidence should be between 0.0 and 1.0

Respond ONLY with the JSON object, no additional text.
"#;

const FIX_SYSTEM_PROMPT: &str = r#"You are a technical documentation writer. Your task is to update documentation to accurately reflect code changes.

You will be given:
1. The current code
2. The documentation section that needs updating

Your job is to rewrite the documentation to accurately describe the current code behavior.
"#;

const FIX_INSTRUCTIONS: &str = r#"## Instructions

Rewrite the documentation section to accurately reflect the current code. Respond with a JSON object:

```json
{
  "updated_content": "The complete updated documentation section",
  "changes_made": ["List of specific changes made"],
  "notes": "Any additional notes for the reviewer"
}
```

Guidelines:
- Preserve the original style and tone
- Keep the same heading structure
- Only change what needs to be changed
- Be accurate and precise

Respond ONLY with the JSON object, no additional text.
"#;

/// Generate a simple explanation without LLM
pub fn generate_simple_explanation(
    drift_event: &DriftEvent,
    old_code: Option<&CodeChunk>,
    new_code: Option<&CodeChunk>,
) -> String {
    let mut explanation = String::new();

    explanation.push_str(&format!("## {}\n\n", drift_event.description));
    explanation.push_str(&format!("**Severity:** {}\n", drift_event.severity));
    explanation.push_str(&format!(
        "**Confidence:** {:.0}%\n\n",
        drift_event.confidence * 100.0
    ));

    explanation.push_str("### Evidence\n\n");
    explanation.push_str(&drift_event.evidence);
    explanation.push_str("\n\n");

    // Add code diff if available
    if let (Some(old), Some(new)) = (old_code, new_code) {
        if old.signature != new.signature {
            explanation.push_str("### Signature Change\n\n");
            if let Some(ref old_sig) = old.signature {
                explanation.push_str(&format!("**Before:** `{}`\n", old_sig));
            }
            if let Some(ref new_sig) = new.signature {
                explanation.push_str(&format!("**After:** `{}`\n", new_sig));
            }
            explanation.push('\n');
        }
    } else if old_code.is_some() && new_code.is_none() {
        explanation.push_str("### Removed Code\n\n");
        explanation.push_str("The code symbol has been removed from the codebase.\n\n");
    } else if old_code.is_none() && new_code.is_some() {
        explanation.push_str("### New Code\n\n");
        explanation.push_str("This is a new code symbol that may need documentation.\n\n");
    }

    explanation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift::DriftSeverity;
    use crate::extract::code::{Language, SymbolType};
    use crate::extract::doc::HeadingLevel;

    #[test]
    fn test_analysis_prompt_generation() {
        let event = DriftEvent::new(
            DriftSeverity::High,
            "Function signature changed",
            "Parameter 'name' was added",
            0.9,
        );

        let code = CodeChunk::new(
            "src/lib.rs",
            "greet",
            SymbolType::Function,
            "pub fn greet(name: &str) { }",
            Language::Rust,
            1,
            1,
        );

        let doc = DocChunk::new(
            "README.md",
            vec!["API".to_string(), "Functions".to_string()],
            "Functions",
            HeadingLevel::H2,
            "The greet function says hello.",
            10,
            15,
        );

        let prompt = AnalysisPrompt::generate(&event, None, Some(&code), &doc);

        assert!(prompt.contains("Function signature changed"));
        assert!(prompt.contains("greet"));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn test_fix_prompt_generation() {
        let event = DriftEvent::new(
            DriftSeverity::High,
            "Parameter added",
            "New parameter 'name'",
            0.9,
        );

        let code = CodeChunk::new(
            "src/lib.rs",
            "greet",
            SymbolType::Function,
            "pub fn greet(name: &str) { }",
            Language::Rust,
            1,
            1,
        );

        let doc = DocChunk::new(
            "README.md",
            vec!["API".to_string()],
            "API",
            HeadingLevel::H2,
            "The greet function.",
            1,
            5,
        );

        let prompt = FixPrompt::generate(&event, &code, &doc);

        assert!(prompt.contains("greet"));
        assert!(prompt.contains("updated_content"));
    }
}
