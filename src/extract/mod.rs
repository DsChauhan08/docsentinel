//! Code and documentation semantic extraction module
//!
//! This module handles extracting semantically meaningful units from:
//! - Code files (using tree-sitter)
//! - Documentation files (using Markdown parsing)

pub mod code;
pub mod doc;

pub use code::{CodeChunk, CodeExtractor, Language, SymbolType};
pub use doc::{DocChunk, DocExtractor, HeadingLevel};

use sha2::{Digest, Sha256};

/// Compute a stable hash for content
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// Common trait for all extractable chunks
pub trait Chunk {
    /// Get the unique identifier for this chunk
    fn id(&self) -> &str;

    /// Get the raw text content
    fn content(&self) -> &str;

    /// Get the content hash
    fn hash(&self) -> &str;

    /// Get the file path this chunk came from
    fn file_path(&self) -> &str;
}
