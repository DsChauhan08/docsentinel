//! Repository ingestion and change detection module
//!
//! This module handles Git repository operations including:
//! - Repository initialization and metadata reading
//! - Change detection between commits
//! - File categorization (code vs documentation)

mod change;
mod config;

pub use change::{Change, ChangeKind, ChangedFile};
pub use config::RepoConfig;

use anyhow::{Context, Result};
use git2::{DiffOptions, ObjectType, Oid, Repository as GitRepo, StatusOptions};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Represents a Git repository being analyzed
pub struct Repository {
    /// The underlying git2 repository
    repo: GitRepo,
    /// Path to the repository root
    root: PathBuf,
    /// Repository configuration
    config: RepoConfig,
}

impl Repository {
    /// Open an existing repository at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let repo = GitRepo::discover(path)
            .with_context(|| format!("Failed to open Git repository at {:?}", path))?;

        let root = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("Repository has no working directory (bare repo?)"))?
            .to_path_buf();

        let config = RepoConfig::load_or_default(&root)?;

        Ok(Self { repo, root, config })
    }

    /// Get the repository root path
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the path to the .docsentinel directory
    pub fn sentinel_dir(&self) -> PathBuf {
        self.root.join(".docsentinel")
    }

    /// Initialize the .docsentinel directory if it doesn't exist
    pub fn init_sentinel_dir(&self) -> Result<PathBuf> {
        let sentinel_dir = self.sentinel_dir();
        if !sentinel_dir.exists() {
            std::fs::create_dir_all(&sentinel_dir)
                .with_context(|| format!("Failed to create {:?}", sentinel_dir))?;
        }
        Ok(sentinel_dir)
    }

    /// Get the current HEAD commit hash
    pub fn head_commit(&self) -> Result<String> {
        let head = self.repo.head().context("Failed to get HEAD reference")?;
        let commit = head.peel_to_commit().context("Failed to peel HEAD to commit")?;
        Ok(commit.id().to_string())
    }

    /// Get changes between two commits
    pub fn changes_between(&self, from: Option<&str>, to: &str) -> Result<Vec<ChangedFile>> {
        let to_commit = self
            .repo
            .revparse_single(to)
            .with_context(|| format!("Failed to parse revision: {}", to))?
            .peel_to_commit()
            .context("Failed to peel to commit")?;

        let to_tree = to_commit.tree().context("Failed to get tree for 'to' commit")?;

        let from_tree = if let Some(from_ref) = from {
            let from_commit = self
                .repo
                .revparse_single(from_ref)
                .with_context(|| format!("Failed to parse revision: {}", from_ref))?
                .peel_to_commit()
                .context("Failed to peel to commit")?;
            Some(from_commit.tree().context("Failed to get tree for 'from' commit")?)
        } else {
            None
        };

        let mut diff_opts = DiffOptions::new();
        diff_opts.include_untracked(false);

        let diff = self
            .repo
            .diff_tree_to_tree(from_tree.as_ref(), Some(&to_tree), Some(&mut diff_opts))
            .context("Failed to compute diff")?;

        let mut changes = Vec::new();

        diff.foreach(
            &mut |delta, _| {
                let path = delta
                    .new_file()
                    .path()
                    .or_else(|| delta.old_file().path())
                    .map(|p| p.to_path_buf());

                if let Some(path) = path {
                    let kind = match delta.status() {
                        git2::Delta::Added => ChangeKind::Added,
                        git2::Delta::Deleted => ChangeKind::Deleted,
                        git2::Delta::Modified => ChangeKind::Modified,
                        git2::Delta::Renamed => ChangeKind::Renamed,
                        _ => ChangeKind::Modified,
                    };

                    let file_type = self.categorize_file(&path);

                    changes.push(ChangedFile {
                        path,
                        kind,
                        file_type,
                    });
                }
                true
            },
            None,
            None,
            None,
        )
        .context("Failed to iterate diff")?;

        Ok(changes)
    }

    /// Get uncommitted changes in the working directory
    pub fn uncommitted_changes(&self) -> Result<Vec<ChangedFile>> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false);

        let statuses = self
            .repo
            .statuses(Some(&mut opts))
            .context("Failed to get repository status")?;

        let mut changes = Vec::new();

        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                let path = PathBuf::from(path);
                let status = entry.status();

                let kind = if status.is_wt_new() || status.is_index_new() {
                    ChangeKind::Added
                } else if status.is_wt_deleted() || status.is_index_deleted() {
                    ChangeKind::Deleted
                } else if status.is_wt_modified() || status.is_index_modified() {
                    ChangeKind::Modified
                } else if status.is_wt_renamed() || status.is_index_renamed() {
                    ChangeKind::Renamed
                } else {
                    continue;
                };

                let file_type = self.categorize_file(&path);

                changes.push(ChangedFile {
                    path,
                    kind,
                    file_type,
                });
            }
        }

        Ok(changes)
    }

    /// Categorize a file as code, documentation, or other
    fn categorize_file(&self, path: &Path) -> FileType {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check if it's a documentation file
        if self.config.doc_patterns.iter().any(|p| {
            path.to_str()
                .map(|s| glob_match(p, s))
                .unwrap_or(false)
        }) {
            return FileType::Documentation;
        }

        // Check by extension
        match extension.to_lowercase().as_str() {
            // Documentation
            "md" | "mdx" | "rst" | "txt" | "adoc" => FileType::Documentation,
            // Code files
            "rs" | "py" | "js" | "ts" | "go" | "java" | "c" | "cpp" | "h" | "hpp" => FileType::Code,
            // Config files
            "toml" | "yaml" | "yml" | "json" => FileType::Config,
            _ => {
                // Check for special documentation files
                let lower_name = filename.to_lowercase();
                if lower_name == "readme"
                    || lower_name == "changelog"
                    || lower_name == "contributing"
                    || lower_name == "license"
                {
                    FileType::Documentation
                } else {
                    FileType::Other
                }
            }
        }
    }

    /// Read file content at a specific commit
    pub fn read_file_at_commit(&self, path: &Path, commit: &str) -> Result<Option<String>> {
        let commit_obj = self
            .repo
            .revparse_single(commit)
            .with_context(|| format!("Failed to parse revision: {}", commit))?
            .peel_to_commit()
            .context("Failed to peel to commit")?;

        let tree = commit_obj.tree().context("Failed to get tree")?;

        let entry = match tree.get_path(path) {
            Ok(entry) => entry,
            Err(_) => return Ok(None),
        };

        let object = entry
            .to_object(&self.repo)
            .context("Failed to get object")?;

        if let Some(blob) = object.as_blob() {
            if blob.is_binary() {
                return Ok(None);
            }
            let content = std::str::from_utf8(blob.content())
                .context("File content is not valid UTF-8")?
                .to_string();
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    /// Read current file content from working directory
    pub fn read_file_current(&self, path: &Path) -> Result<Option<String>> {
        let full_path = self.root.join(path);
        if !full_path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read file: {:?}", full_path))?;
        Ok(Some(content))
    }

    /// Get the repository configuration
    pub fn config(&self) -> &RepoConfig {
        &self.config
    }

    /// List all files in the repository matching certain criteria
    pub fn list_files(&self, file_type: Option<FileType>) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for entry in walkdir::WalkDir::new(&self.root)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_str().unwrap_or("");
                !name.starts_with('.') && name != "target" && name != "node_modules"
            })
        {
            let entry = entry?;
            if entry.file_type().is_file() {
                let path = entry.path().strip_prefix(&self.root)?.to_path_buf();
                
                if let Some(ref ft) = file_type {
                    if &self.categorize_file(&path) == ft {
                        files.push(path);
                    }
                } else {
                    files.push(path);
                }
            }
        }

        Ok(files)
    }
}

/// Type of file in the repository
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    Code,
    Documentation,
    Config,
    Other,
}

/// Simple glob matching (supports * and **)
fn glob_match(pattern: &str, path: &str) -> bool {
    // Simple implementation - in production, use the `glob` crate
    if pattern.contains("**") {
        let parts: Vec<&str> = pattern.split("**").collect();
        if parts.len() == 2 {
            let prefix = parts[0].trim_end_matches('/');
            let suffix = parts[1].trim_start_matches('/');
            return (prefix.is_empty() || path.starts_with(prefix))
                && (suffix.is_empty() || path.ends_with(suffix));
        }
    }
    
    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            return path.starts_with(parts[0]) && path.ends_with(parts[1]);
        }
    }
    
    path == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match() {
        assert!(glob_match("*.md", "README.md"));
        assert!(glob_match("docs/**/*.md", "docs/api/guide.md"));
        assert!(!glob_match("*.rs", "README.md"));
    }
}
