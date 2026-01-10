//! Documentation extraction using Markdown parsing
//!
//! Extracts semantically meaningful sections from documentation files:
//! - Splits documents by heading hierarchy
//! - Each section becomes a DocChunk

use super::{content_hash, Chunk};
use anyhow::Result;
use pulldown_cmark::{Event, HeadingLevel as CmarkHeadingLevel, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Heading level in documentation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HeadingLevel {
    H1 = 1,
    H2 = 2,
    H3 = 3,
    H4 = 4,
    H5 = 5,
    H6 = 6,
}

impl From<CmarkHeadingLevel> for HeadingLevel {
    fn from(level: CmarkHeadingLevel) -> Self {
        match level {
            CmarkHeadingLevel::H1 => HeadingLevel::H1,
            CmarkHeadingLevel::H2 => HeadingLevel::H2,
            CmarkHeadingLevel::H3 => HeadingLevel::H3,
            CmarkHeadingLevel::H4 => HeadingLevel::H4,
            CmarkHeadingLevel::H5 => HeadingLevel::H5,
            CmarkHeadingLevel::H6 => HeadingLevel::H6,
        }
    }
}

impl std::fmt::Display for HeadingLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "H{}", *self as u8)
    }
}

/// A semantic unit extracted from documentation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocChunk {
    /// Unique identifier (file path + heading path)
    pub id: String,
    /// File path relative to repository root
    pub file_path: String,
    /// Heading path (e.g., "Installation > Prerequisites")
    pub heading_path: Vec<String>,
    /// The heading text itself
    pub heading: String,
    /// Heading level
    pub level: HeadingLevel,
    /// Raw content (including the heading)
    pub content: String,
    /// Content hash for change detection
    pub hash: String,
    /// Line number where the section starts
    pub start_line: usize,
    /// Line number where the section ends
    pub end_line: usize,
    /// Embedding vector (populated later)
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,
}

impl Chunk for DocChunk {
    fn id(&self) -> &str {
        &self.id
    }

    fn content(&self) -> &str {
        &self.content
    }

    fn hash(&self) -> &str {
        &self.hash
    }

    fn file_path(&self) -> &str {
        &self.file_path
    }
}

impl DocChunk {
    /// Create a new doc chunk
    pub fn new(
        file_path: &str,
        heading_path: Vec<String>,
        heading: &str,
        level: HeadingLevel,
        content: &str,
        start_line: usize,
        end_line: usize,
    ) -> Self {
        let path_str = heading_path.join(" > ");
        let id = format!("{}#{}", file_path, path_str);
        let hash = content_hash(content);

        Self {
            id,
            file_path: file_path.to_string(),
            heading_path,
            heading: heading.to_string(),
            level,
            content: content.to_string(),
            hash,
            start_line,
            end_line,
            embedding: None,
        }
    }

    /// Get a summary suitable for embedding
    pub fn embedding_text(&self) -> String {
        let path = self.heading_path.join(" > ");
        format!(
            "Documentation section: {}\nFile: {}\n\n{}",
            path, self.file_path, self.content
        )
    }

    /// Get the full heading path as a string
    pub fn full_path(&self) -> String {
        self.heading_path.join(" > ")
    }
}

/// Extracts doc chunks from Markdown files
pub struct DocExtractor {
    /// Minimum section length to extract (in characters)
    min_section_length: usize,
}

impl DocExtractor {
    /// Create a new doc extractor
    pub fn new() -> Self {
        Self {
            min_section_length: 10,
        }
    }

    /// Set minimum section length
    pub fn with_min_length(mut self, length: usize) -> Self {
        self.min_section_length = length;
        self
    }

    /// Extract chunks from a Markdown file
    pub fn extract_file(&self, path: &Path, content: &str) -> Result<Vec<DocChunk>> {
        let file_path = path.to_string_lossy().to_string();
        let lines: Vec<&str> = content.lines().collect();

        let mut chunks = Vec::new();
        let mut sections = self.parse_sections(content);

        // Convert sections to chunks
        for section in sections.drain(..) {
            if section.content.len() >= self.min_section_length {
                let chunk = DocChunk::new(
                    &file_path,
                    section.heading_path,
                    &section.heading,
                    section.level,
                    &section.content,
                    section.start_line,
                    section.end_line,
                );
                chunks.push(chunk);
            }
        }

        // If no sections found, treat the whole file as one chunk
        if chunks.is_empty() && !content.trim().is_empty() {
            let chunk = DocChunk::new(
                &file_path,
                vec![file_path.clone()],
                &file_path,
                HeadingLevel::H1,
                content,
                1,
                lines.len(),
            );
            chunks.push(chunk);
        }

        Ok(chunks)
    }

    /// Parse sections from Markdown content
    fn parse_sections(&self, content: &str) -> Vec<Section> {
        let mut sections = Vec::new();
        let mut current_path: Vec<(HeadingLevel, String)> = Vec::new();
        let mut current_section: Option<Section> = None;

        let lines: Vec<&str> = content.lines().collect();
        let mut line_num = 0;

        let parser = Parser::new(content);
        let mut in_heading = false;
        let mut heading_text = String::new();
        let mut heading_level = HeadingLevel::H1;

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    in_heading = true;
                    heading_level = level.into();
                    heading_text.clear();
                }
                Event::End(TagEnd::Heading(_)) => {
                    in_heading = false;

                    // Find the line number for this heading
                    let heading_line = self.find_heading_line(&lines, &heading_text, line_num);
                    line_num = heading_line;

                    // Close current section if any
                    if let Some(mut section) = current_section.take() {
                        section.end_line = heading_line.saturating_sub(1).max(section.start_line);
                        section.content = self.extract_section_content(&lines, section.start_line, section.end_line);
                        sections.push(section);
                    }

                    // Update heading path
                    while let Some((level, _)) = current_path.last() {
                        if *level >= heading_level {
                            current_path.pop();
                        } else {
                            break;
                        }
                    }
                    current_path.push((heading_level, heading_text.clone()));

                    // Start new section
                    let heading_path: Vec<String> = current_path.iter().map(|(_, h)| h.clone()).collect();
                    current_section = Some(Section {
                        heading_path,
                        heading: heading_text.clone(),
                        level: heading_level,
                        content: String::new(),
                        start_line: heading_line,
                        end_line: heading_line,
                    });
                }
                Event::Text(text) if in_heading => {
                    heading_text.push_str(&text);
                }
                Event::Code(code) if in_heading => {
                    heading_text.push_str(&code);
                }
                _ => {}
            }
        }

        // Close final section
        if let Some(mut section) = current_section.take() {
            section.end_line = lines.len();
            section.content = self.extract_section_content(&lines, section.start_line, section.end_line);
            sections.push(section);
        }

        sections
    }

    /// Find the line number for a heading
    fn find_heading_line(&self, lines: &[&str], heading: &str, start_from: usize) -> usize {
        for (i, line) in lines.iter().enumerate().skip(start_from) {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                let heading_part = trimmed.trim_start_matches('#').trim();
                if heading_part == heading {
                    return i + 1; // 1-indexed
                }
            }
        }
        start_from + 1
    }

    /// Extract content for a section
    fn extract_section_content(&self, lines: &[&str], start: usize, end: usize) -> String {
        let start_idx = start.saturating_sub(1);
        let end_idx = end.min(lines.len());

        if start_idx >= lines.len() {
            return String::new();
        }

        lines[start_idx..end_idx].join("\n")
    }
}

impl Default for DocExtractor {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal representation of a section during parsing
struct Section {
    heading_path: Vec<String>,
    heading: String,
    level: HeadingLevel,
    content: String,
    start_line: usize,
    end_line: usize,
}

/// Extract code blocks from Markdown content
pub fn extract_code_blocks(content: &str) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();
    let parser = Parser::new(content);

    let mut in_code_block = false;
    let mut code_content = String::new();
    let mut code_lang = None;

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_content.clear();
                code_lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        let lang_str = lang.to_string();
                        if lang_str.is_empty() {
                            None
                        } else {
                            Some(lang_str)
                        }
                    }
                    pulldown_cmark::CodeBlockKind::Indented => None,
                };
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                blocks.push(CodeBlock {
                    language: code_lang.take(),
                    content: code_content.clone(),
                });
            }
            Event::Text(text) if in_code_block => {
                code_content.push_str(&text);
            }
            _ => {}
        }
    }

    blocks
}

/// A code block found in documentation
#[derive(Debug, Clone)]
pub struct CodeBlock {
    /// Language identifier (if specified)
    pub language: Option<String>,
    /// Code content
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_markdown_sections() {
        let extractor = DocExtractor::new();
        let content = r#"# Main Title

This is the introduction.

## Installation

How to install the software.

### Prerequisites

What you need before installing.

## Usage

How to use the software.
"#;

        let chunks = extractor
            .extract_file(Path::new("README.md"), content)
            .unwrap();

        assert!(chunks.len() >= 3);

        // Check that we have the expected sections
        let headings: Vec<&str> = chunks.iter().map(|c| c.heading.as_str()).collect();
        assert!(headings.contains(&"Main Title"));
        assert!(headings.contains(&"Installation"));
        assert!(headings.contains(&"Usage"));
    }

    #[test]
    fn test_extract_code_blocks() {
        let content = r#"
# Example

```rust
fn main() {
    println!("Hello");
}
```

```python
print("Hello")
```
"#;

        let blocks = extract_code_blocks(content);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].language, Some("rust".to_string()));
        assert_eq!(blocks[1].language, Some("python".to_string()));
    }

    #[test]
    fn test_heading_path() {
        let extractor = DocExtractor::new();
        let content = r#"# Root

## Child

### Grandchild

Content here.
"#;

        let chunks = extractor
            .extract_file(Path::new("test.md"), content)
            .unwrap();

        // Find the grandchild chunk
        let grandchild = chunks.iter().find(|c| c.heading == "Grandchild");
        assert!(grandchild.is_some());

        let gc = grandchild.unwrap();
        assert_eq!(gc.heading_path, vec!["Root", "Child", "Grandchild"]);
    }
}
