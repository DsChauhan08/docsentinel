//! Code extraction using tree-sitter
//!
//! Extracts semantically meaningful units from code files:
//! - Public function definitions
//! - Method signatures
//! - Structs / classes
//! - Doc comments

use super::{content_hash, Chunk};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    Python,
}

impl Language {
    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "rs" => Some(Language::Rust),
            "py" => Some(Language::Python),
            _ => None,
        }
    }

    /// Get the tree-sitter language for this language
    pub fn tree_sitter_language(&self) -> tree_sitter::Language {
        match self {
            Language::Rust => tree_sitter_rust::LANGUAGE.into(),
            Language::Python => tree_sitter_python::LANGUAGE.into(),
        }
    }

    /// Get file extension for this language
    pub fn extension(&self) -> &'static str {
        match self {
            Language::Rust => "rs",
            Language::Python => "py",
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Rust => write!(f, "rust"),
            Language::Python => write!(f, "python"),
        }
    }
}

/// A semantic unit extracted from code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    /// Unique identifier (path + symbol name)
    pub id: String,
    /// File path relative to repository root
    pub file_path: String,
    /// Symbol name (function, class, struct name)
    pub symbol_name: String,
    /// Type of symbol
    pub symbol_type: SymbolType,
    /// Raw text content
    pub content: String,
    /// Content hash for change detection
    pub hash: String,
    /// Programming language
    pub language: Language,
    /// Line number where the symbol starts
    pub start_line: usize,
    /// Line number where the symbol ends
    pub end_line: usize,
    /// Associated doc comment (if any)
    pub doc_comment: Option<String>,
    /// Function/method signature (if applicable)
    pub signature: Option<String>,
    /// Whether this is a public symbol
    pub is_public: bool,
    /// Embedding vector (populated later)
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,
}

impl Chunk for CodeChunk {
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

impl CodeChunk {
    /// Create a new code chunk
    pub fn new(
        file_path: &str,
        symbol_name: &str,
        symbol_type: SymbolType,
        content: &str,
        language: Language,
        start_line: usize,
        end_line: usize,
    ) -> Self {
        let id = format!("{}::{}", file_path, symbol_name);
        let hash = content_hash(content);

        Self {
            id,
            file_path: file_path.to_string(),
            symbol_name: symbol_name.to_string(),
            symbol_type,
            content: content.to_string(),
            hash,
            language,
            start_line,
            end_line,
            doc_comment: None,
            signature: None,
            is_public: false,
            embedding: None,
        }
    }

    /// Get a summary suitable for embedding
    pub fn embedding_text(&self) -> String {
        let mut parts = Vec::new();

        // Include doc comment if present
        if let Some(ref doc) = self.doc_comment {
            parts.push(doc.clone());
        }

        // Include signature if present
        if let Some(ref sig) = self.signature {
            parts.push(format!("Signature: {}", sig));
        }

        // Include symbol info
        parts.push(format!(
            "{} {} in {}",
            self.symbol_type, self.symbol_name, self.file_path
        ));

        parts.join("\n")
    }
}

/// Type of code symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolType {
    Function,
    Method,
    Struct,
    Class,
    Enum,
    Trait,
    Impl,
    Module,
    Constant,
}

impl std::fmt::Display for SymbolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolType::Function => write!(f, "function"),
            SymbolType::Method => write!(f, "method"),
            SymbolType::Struct => write!(f, "struct"),
            SymbolType::Class => write!(f, "class"),
            SymbolType::Enum => write!(f, "enum"),
            SymbolType::Trait => write!(f, "trait"),
            SymbolType::Impl => write!(f, "impl"),
            SymbolType::Module => write!(f, "module"),
            SymbolType::Constant => write!(f, "constant"),
        }
    }
}

/// Extracts code chunks from source files
pub struct CodeExtractor {
    rust_parser: tree_sitter::Parser,
    python_parser: tree_sitter::Parser,
}

impl CodeExtractor {
    /// Create a new code extractor
    pub fn new() -> Result<Self> {
        let mut rust_parser = tree_sitter::Parser::new();
        rust_parser
            .set_language(&Language::Rust.tree_sitter_language())
            .context("Failed to set Rust language")?;

        let mut python_parser = tree_sitter::Parser::new();
        python_parser
            .set_language(&Language::Python.tree_sitter_language())
            .context("Failed to set Python language")?;

        Ok(Self {
            rust_parser,
            python_parser,
        })
    }

    /// Extract chunks from a file
    pub fn extract_file(&mut self, path: &Path, content: &str) -> Result<Vec<CodeChunk>> {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let language = Language::from_extension(extension)
            .ok_or_else(|| anyhow::anyhow!("Unsupported language: {}", extension))?;

        match language {
            Language::Rust => self.extract_rust(path, content),
            Language::Python => self.extract_python(path, content),
        }
    }

    /// Extract chunks from Rust code
    fn extract_rust(&mut self, path: &Path, content: &str) -> Result<Vec<CodeChunk>> {
        let tree = self
            .rust_parser
            .parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Rust file"))?;

        let mut chunks = Vec::new();
        let file_path = path.to_string_lossy().to_string();

        self.walk_rust_tree(tree.root_node(), content, &file_path, &mut chunks);

        Ok(chunks)
    }

    /// Walk the Rust AST and extract chunks
    fn walk_rust_tree(
        &self,
        node: tree_sitter::Node,
        source: &str,
        file_path: &str,
        chunks: &mut Vec<CodeChunk>,
    ) {
        let kind = node.kind();

        match kind {
            "function_item" => {
                if let Some(chunk) = self.extract_rust_function(node, source, file_path) {
                    chunks.push(chunk);
                }
            }
            "struct_item" => {
                if let Some(chunk) = self.extract_rust_struct(node, source, file_path) {
                    chunks.push(chunk);
                }
            }
            "enum_item" => {
                if let Some(chunk) = self.extract_rust_enum(node, source, file_path) {
                    chunks.push(chunk);
                }
            }
            "trait_item" => {
                if let Some(chunk) = self.extract_rust_trait(node, source, file_path) {
                    chunks.push(chunk);
                }
            }
            "impl_item" => {
                // Extract methods from impl blocks
                self.extract_rust_impl_methods(node, source, file_path, chunks);
            }
            _ => {}
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_rust_tree(child, source, file_path, chunks);
        }
    }

    /// Extract a Rust function
    fn extract_rust_function(
        &self,
        node: tree_sitter::Node,
        source: &str,
        file_path: &str,
    ) -> Option<CodeChunk> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?;

        let content = node.utf8_text(source.as_bytes()).ok()?;
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        let mut chunk = CodeChunk::new(
            file_path,
            name,
            SymbolType::Function,
            content,
            Language::Rust,
            start_line,
            end_line,
        );

        // Check for pub visibility
        chunk.is_public = self.has_rust_visibility(node, source);

        // Extract doc comment
        chunk.doc_comment = self.extract_rust_doc_comment(node, source);

        // Extract signature
        chunk.signature = self.extract_rust_function_signature(node, source);

        Some(chunk)
    }

    /// Extract a Rust struct
    fn extract_rust_struct(
        &self,
        node: tree_sitter::Node,
        source: &str,
        file_path: &str,
    ) -> Option<CodeChunk> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?;

        let content = node.utf8_text(source.as_bytes()).ok()?;
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        let mut chunk = CodeChunk::new(
            file_path,
            name,
            SymbolType::Struct,
            content,
            Language::Rust,
            start_line,
            end_line,
        );

        chunk.is_public = self.has_rust_visibility(node, source);
        chunk.doc_comment = self.extract_rust_doc_comment(node, source);

        Some(chunk)
    }

    /// Extract a Rust enum
    fn extract_rust_enum(
        &self,
        node: tree_sitter::Node,
        source: &str,
        file_path: &str,
    ) -> Option<CodeChunk> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?;

        let content = node.utf8_text(source.as_bytes()).ok()?;
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        let mut chunk = CodeChunk::new(
            file_path,
            name,
            SymbolType::Enum,
            content,
            Language::Rust,
            start_line,
            end_line,
        );

        chunk.is_public = self.has_rust_visibility(node, source);
        chunk.doc_comment = self.extract_rust_doc_comment(node, source);

        Some(chunk)
    }

    /// Extract a Rust trait
    fn extract_rust_trait(
        &self,
        node: tree_sitter::Node,
        source: &str,
        file_path: &str,
    ) -> Option<CodeChunk> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?;

        let content = node.utf8_text(source.as_bytes()).ok()?;
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        let mut chunk = CodeChunk::new(
            file_path,
            name,
            SymbolType::Trait,
            content,
            Language::Rust,
            start_line,
            end_line,
        );

        chunk.is_public = self.has_rust_visibility(node, source);
        chunk.doc_comment = self.extract_rust_doc_comment(node, source);

        Some(chunk)
    }

    /// Extract methods from a Rust impl block
    fn extract_rust_impl_methods(
        &self,
        node: tree_sitter::Node,
        source: &str,
        file_path: &str,
        chunks: &mut Vec<CodeChunk>,
    ) {
        // Get the type being implemented
        let type_name = node
            .child_by_field_name("type")
            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
            .unwrap_or("Unknown");

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "declaration_list" {
                let mut inner_cursor = child.walk();
                for item in child.children(&mut inner_cursor) {
                    if item.kind() == "function_item" {
                        if let Some(name_node) = item.child_by_field_name("name") {
                            if let Ok(method_name) = name_node.utf8_text(source.as_bytes()) {
                                let full_name = format!("{}::{}", type_name, method_name);
                                let content = item.utf8_text(source.as_bytes()).unwrap_or("");
                                let start_line = item.start_position().row + 1;
                                let end_line = item.end_position().row + 1;

                                let mut chunk = CodeChunk::new(
                                    file_path,
                                    &full_name,
                                    SymbolType::Method,
                                    content,
                                    Language::Rust,
                                    start_line,
                                    end_line,
                                );

                                chunk.is_public = self.has_rust_visibility(item, source);
                                chunk.doc_comment = self.extract_rust_doc_comment(item, source);
                                chunk.signature =
                                    self.extract_rust_function_signature(item, source);

                                chunks.push(chunk);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Check if a Rust node has pub visibility
    fn has_rust_visibility(&self, node: tree_sitter::Node, source: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    return text.starts_with("pub");
                }
            }
        }
        false
    }

    /// Extract doc comment for a Rust node
    fn extract_rust_doc_comment(&self, node: tree_sitter::Node, source: &str) -> Option<String> {
        // Look for preceding line comments starting with ///
        let start_byte = node.start_byte();
        let prefix = &source[..start_byte];

        let mut doc_lines = Vec::new();
        for line in prefix.lines().rev() {
            let trimmed = line.trim();
            if trimmed.starts_with("///") {
                doc_lines.push(trimmed.trim_start_matches("///").trim());
            } else if trimmed.starts_with("//!") {
                // Module-level doc, skip
                break;
            } else if trimmed.is_empty() {
                continue;
            } else {
                break;
            }
        }

        if doc_lines.is_empty() {
            None
        } else {
            doc_lines.reverse();
            Some(doc_lines.join("\n"))
        }
    }

    /// Extract function signature from a Rust function node
    fn extract_rust_function_signature(
        &self,
        node: tree_sitter::Node,
        source: &str,
    ) -> Option<String> {
        // Get everything up to the block
        let mut signature_end = node.end_byte();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "block" {
                signature_end = child.start_byte();
                break;
            }
        }

        let signature = &source[node.start_byte()..signature_end];
        Some(signature.trim().to_string())
    }

    /// Extract chunks from Python code
    fn extract_python(&mut self, path: &Path, content: &str) -> Result<Vec<CodeChunk>> {
        let tree = self
            .python_parser
            .parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Python file"))?;

        let mut chunks = Vec::new();
        let file_path = path.to_string_lossy().to_string();

        self.walk_python_tree(tree.root_node(), content, &file_path, &mut chunks, None);

        Ok(chunks)
    }

    /// Walk the Python AST and extract chunks
    fn walk_python_tree(
        &self,
        node: tree_sitter::Node,
        source: &str,
        file_path: &str,
        chunks: &mut Vec<CodeChunk>,
        class_name: Option<&str>,
    ) {
        let kind = node.kind();

        match kind {
            "function_definition" => {
                if let Some(chunk) =
                    self.extract_python_function(node, source, file_path, class_name)
                {
                    chunks.push(chunk);
                }
            }
            "class_definition" => {
                if let Some(chunk) = self.extract_python_class(node, source, file_path) {
                    chunks.push(chunk);
                }
                // Extract methods within the class
                let class_name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok());

                if let Some(body) = node.child_by_field_name("body") {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        self.walk_python_tree(child, source, file_path, chunks, class_name);
                    }
                }
                return; // Don't recurse normally for classes
            }
            _ => {}
        }

        // Recurse into children (except for classes which we handle specially)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_python_tree(child, source, file_path, chunks, class_name);
        }
    }

    /// Extract a Python function or method
    fn extract_python_function(
        &self,
        node: tree_sitter::Node,
        source: &str,
        file_path: &str,
        class_name: Option<&str>,
    ) -> Option<CodeChunk> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?;

        let content = node.utf8_text(source.as_bytes()).ok()?;
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        let (full_name, symbol_type) = if let Some(class) = class_name {
            (format!("{}.{}", class, name), SymbolType::Method)
        } else {
            (name.to_string(), SymbolType::Function)
        };

        let mut chunk = CodeChunk::new(
            file_path,
            &full_name,
            symbol_type,
            content,
            Language::Python,
            start_line,
            end_line,
        );

        // Python functions without underscore prefix are considered public
        chunk.is_public = !name.starts_with('_') || name.starts_with("__") && name.ends_with("__");

        // Extract docstring
        chunk.doc_comment = self.extract_python_docstring(node, source);

        // Extract signature
        chunk.signature = self.extract_python_signature(node, source);

        Some(chunk)
    }

    /// Extract a Python class
    fn extract_python_class(
        &self,
        node: tree_sitter::Node,
        source: &str,
        file_path: &str,
    ) -> Option<CodeChunk> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?;

        let content = node.utf8_text(source.as_bytes()).ok()?;
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        let mut chunk = CodeChunk::new(
            file_path,
            name,
            SymbolType::Class,
            content,
            Language::Python,
            start_line,
            end_line,
        );

        chunk.is_public = !name.starts_with('_');
        chunk.doc_comment = self.extract_python_docstring(node, source);

        Some(chunk)
    }

    /// Extract docstring from a Python function or class
    fn extract_python_docstring(&self, node: tree_sitter::Node, source: &str) -> Option<String> {
        // Look for the body and check if first statement is a string
        let body = node.child_by_field_name("body")?;

        let mut cursor = body.walk();
        // Only check first statement for docstring
        if let Some(child) = body.children(&mut cursor).next() {
            if child.kind() == "expression_statement" {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "string" {
                        let text = inner.utf8_text(source.as_bytes()).ok()?;
                        // Remove quotes
                        let trimmed = text
                            .trim_start_matches("\"\"\"")
                            .trim_start_matches("'''")
                            .trim_end_matches("\"\"\"")
                            .trim_end_matches("'''")
                            .trim();
                        return Some(trimmed.to_string());
                    }
                }
            }
        }

        None
    }

    /// Extract signature from a Python function
    fn extract_python_signature(&self, node: tree_sitter::Node, source: &str) -> Option<String> {
        let name = node.child_by_field_name("name")?;
        let params = node.child_by_field_name("parameters")?;

        let name_text = name.utf8_text(source.as_bytes()).ok()?;
        let params_text = params.utf8_text(source.as_bytes()).ok()?;

        // Check for return type annotation
        let return_type = node
            .child_by_field_name("return_type")
            .and_then(|n| n.utf8_text(source.as_bytes()).ok());

        let signature = if let Some(ret) = return_type {
            format!("def {}{} -> {}", name_text, params_text, ret)
        } else {
            format!("def {}{}", name_text, params_text)
        };

        Some(signature)
    }
}

impl Default for CodeExtractor {
    fn default() -> Self {
        Self::new().expect("Failed to create CodeExtractor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_function() {
        let mut extractor = CodeExtractor::new().unwrap();
        let code = r#"
/// This is a doc comment
/// with multiple lines
pub fn hello_world(name: &str) -> String {
    format!("Hello, {}!", name)
}
"#;

        let chunks = extractor.extract_file(Path::new("test.rs"), code).unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol_name, "hello_world");
        assert_eq!(chunks[0].symbol_type, SymbolType::Function);
        assert!(chunks[0].is_public);
        assert!(chunks[0].doc_comment.is_some());
    }

    #[test]
    fn test_extract_python_function() {
        let mut extractor = CodeExtractor::new().unwrap();
        let code = r#"
def hello_world(name: str) -> str:
    """This is a docstring."""
    return f"Hello, {name}!"
"#;

        let chunks = extractor.extract_file(Path::new("test.py"), code).unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol_name, "hello_world");
        assert_eq!(chunks[0].symbol_type, SymbolType::Function);
        assert!(chunks[0].is_public);
        assert!(chunks[0].doc_comment.is_some());
    }
}
