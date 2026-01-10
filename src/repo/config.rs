//! Repository configuration for DocSentinel

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Configuration for a repository being analyzed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    /// Patterns for documentation files (glob patterns)
    #[serde(default = "default_doc_patterns")]
    pub doc_patterns: Vec<String>,

    /// Patterns for code files to analyze (glob patterns)
    #[serde(default = "default_code_patterns")]
    pub code_patterns: Vec<String>,

    /// Patterns to ignore (glob patterns)
    #[serde(default = "default_ignore_patterns")]
    pub ignore_patterns: Vec<String>,

    /// Languages to analyze
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,

    /// Similarity threshold for drift detection (0.0 - 1.0)
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,

    /// Number of nearest doc chunks to consider
    #[serde(default = "default_top_k")]
    pub top_k: usize,

    /// LLM endpoint configuration
    #[serde(default)]
    pub llm: LlmConfig,
}

/// LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmConfig {
    /// API endpoint URL (e.g., http://localhost:11434/api for Ollama)
    pub endpoint: Option<String>,

    /// Model name to use
    pub model: Option<String>,

    /// API key (if required)
    pub api_key: Option<String>,

    /// Maximum tokens for response
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Temperature for generation
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_doc_patterns() -> Vec<String> {
    vec![
        "*.md".to_string(),
        "*.mdx".to_string(),
        "*.rst".to_string(),
        "docs/**/*".to_string(),
        "README*".to_string(),
        "CHANGELOG*".to_string(),
    ]
}

fn default_code_patterns() -> Vec<String> {
    vec![
        "*.rs".to_string(),
        "*.py".to_string(),
        "src/**/*.rs".to_string(),
        "src/**/*.py".to_string(),
        "lib/**/*.rs".to_string(),
        "lib/**/*.py".to_string(),
    ]
}

fn default_ignore_patterns() -> Vec<String> {
    vec![
        "target/**".to_string(),
        "node_modules/**".to_string(),
        ".git/**".to_string(),
        ".docsentinel/**".to_string(),
        "*.lock".to_string(),
        "*.min.js".to_string(),
    ]
}

fn default_languages() -> Vec<String> {
    vec!["rust".to_string(), "python".to_string()]
}

fn default_similarity_threshold() -> f32 {
    0.7
}

fn default_top_k() -> usize {
    5
}

fn default_max_tokens() -> usize {
    2048
}

fn default_temperature() -> f32 {
    0.3
}

impl Default for RepoConfig {
    fn default() -> Self {
        Self {
            doc_patterns: default_doc_patterns(),
            code_patterns: default_code_patterns(),
            ignore_patterns: default_ignore_patterns(),
            languages: default_languages(),
            similarity_threshold: default_similarity_threshold(),
            top_k: default_top_k(),
            llm: LlmConfig::default(),
        }
    }
}

impl RepoConfig {
    /// Load configuration from the repository or return defaults
    pub fn load_or_default(repo_root: &Path) -> Result<Self> {
        let config_path = repo_root.join(".docsentinel").join("config.toml");

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config file: {:?}", config_path))?;
            let config: RepoConfig = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config file: {:?}", config_path))?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Save configuration to the repository
    pub fn save(&self, repo_root: &Path) -> Result<()> {
        let sentinel_dir = repo_root.join(".docsentinel");
        std::fs::create_dir_all(&sentinel_dir)?;

        let config_path = sentinel_dir.join("config.toml");
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize configuration")?;

        std::fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

        Ok(())
    }

    /// Check if a path should be ignored
    pub fn should_ignore(&self, path: &str) -> bool {
        self.ignore_patterns.iter().any(|pattern| {
            glob_match_simple(pattern, path)
        })
    }

    /// Check if a path is a documentation file
    pub fn is_doc_file(&self, path: &str) -> bool {
        self.doc_patterns.iter().any(|pattern| {
            glob_match_simple(pattern, path)
        })
    }

    /// Check if a path is a code file
    pub fn is_code_file(&self, path: &str) -> bool {
        self.code_patterns.iter().any(|pattern| {
            glob_match_simple(pattern, path)
        })
    }
}

/// Simple glob matching helper
fn glob_match_simple(pattern: &str, path: &str) -> bool {
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

    path == pattern || path.ends_with(&format!("/{}", pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RepoConfig::default();
        assert!(!config.doc_patterns.is_empty());
        assert!(!config.code_patterns.is_empty());
        assert!(config.similarity_threshold > 0.0);
    }

    #[test]
    fn test_glob_matching() {
        assert!(glob_match_simple("*.md", "README.md"));
        assert!(glob_match_simple("docs/**/*.md", "docs/api/guide.md"));
        assert!(!glob_match_simple("*.rs", "README.md"));
    }
}
