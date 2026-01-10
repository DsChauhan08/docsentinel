//! Change tracking types for repository modifications

use super::FileType;
use std::path::PathBuf;

/// Represents a changed file in the repository
#[derive(Debug, Clone)]
pub struct ChangedFile {
    /// Path to the file relative to repository root
    pub path: PathBuf,
    /// Type of change
    pub kind: ChangeKind,
    /// Category of the file
    pub file_type: FileType,
}

impl ChangedFile {
    /// Check if this is a code file
    pub fn is_code(&self) -> bool {
        self.file_type == FileType::Code
    }

    /// Check if this is a documentation file
    pub fn is_documentation(&self) -> bool {
        self.file_type == FileType::Documentation
    }
}

/// Type of change made to a file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// File was added
    Added,
    /// File was modified
    Modified,
    /// File was deleted
    Deleted,
    /// File was renamed
    Renamed,
}

impl std::fmt::Display for ChangeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeKind::Added => write!(f, "added"),
            ChangeKind::Modified => write!(f, "modified"),
            ChangeKind::Deleted => write!(f, "deleted"),
            ChangeKind::Renamed => write!(f, "renamed"),
        }
    }
}

/// A collection of changes representing a logical unit of work
#[derive(Debug, Clone)]
pub struct Change {
    /// Unique identifier for this change set
    pub id: String,
    /// Commit hash (if committed)
    pub commit: Option<String>,
    /// List of changed files
    pub files: Vec<ChangedFile>,
    /// Timestamp of the change
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Change {
    /// Create a new change set
    pub fn new(commit: Option<String>, files: Vec<ChangedFile>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            commit,
            files,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Get all code files in this change
    pub fn code_files(&self) -> impl Iterator<Item = &ChangedFile> {
        self.files.iter().filter(|f| f.is_code())
    }

    /// Get all documentation files in this change
    pub fn doc_files(&self) -> impl Iterator<Item = &ChangedFile> {
        self.files.iter().filter(|f| f.is_documentation())
    }

    /// Check if this change includes both code and documentation
    pub fn has_mixed_changes(&self) -> bool {
        let has_code = self.files.iter().any(|f| f.is_code());
        let has_docs = self.files.iter().any(|f| f.is_documentation());
        has_code && has_docs
    }
}
