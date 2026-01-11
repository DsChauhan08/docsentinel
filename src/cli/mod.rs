//! CLI interface using clap
//!
//! Provides the command-line interface for DocSentinel

mod commands;

pub use commands::*;

use clap::{Parser, Subcommand};

/// DocSentinel - Documentation drift detection tool
#[derive(Parser, Debug)]
#[command(name = "docsentinel")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Path to the repository (defaults to current directory)
    #[arg(short, long, global = true, default_value = ".")]
    pub path: String,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Output format (text, json)
    #[arg(short = 'o', long, global = true, default_value = "text")]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize DocSentinel in a repository
    Init(InitArgs),

    /// Scan the repository for documentation drift
    Scan(ScanArgs),

    /// Show detected drift issues
    Status(StatusArgs),

    /// Launch the interactive TUI
    Tui(TuiArgs),

    /// Apply a suggested fix
    Fix(FixArgs),

    /// Ignore a drift issue
    Ignore(IgnoreArgs),

    /// Install git hooks for automatic scanning
    Hooks(HooksArgs),

    /// Watch for changes and scan automatically
    Watch(WatchArgs),

    /// Show configuration
    Config(ConfigArgs),

    /// Analyze a specific file or symbol
    Analyze(AnalyzeArgs),

    /// Generate documentation from code
    Generate(GenerateArgs),
}

/// Output format
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

/// Arguments for init command
#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Force re-initialization
    #[arg(short, long)]
    pub force: bool,

    /// Skip initial scan
    #[arg(long)]
    pub no_scan: bool,

    /// Quick mode - minimal output
    #[arg(short, long)]
    pub quick: bool,
}

/// Arguments for scan command
#[derive(Parser, Debug)]
pub struct ScanArgs {
    /// Scan all files, not just changed ones
    #[arg(short, long)]
    pub full: bool,

    /// Commit range to scan (e.g., "HEAD~5..HEAD")
    #[arg(short, long)]
    pub range: Option<String>,

    /// Include uncommitted changes
    #[arg(short, long)]
    pub uncommitted: bool,

    /// Skip embedding generation (faster but less accurate)
    #[arg(long)]
    pub no_embeddings: bool,

    /// Use LLM for analysis
    #[arg(long)]
    pub with_llm: bool,
}

/// Arguments for status command
#[derive(Parser, Debug)]
pub struct StatusArgs {
    /// Show all issues, including resolved ones
    #[arg(short, long)]
    pub all: bool,

    /// Filter by severity (critical, high, medium, low)
    #[arg(short, long)]
    pub severity: Option<String>,

    /// Show detailed information
    #[arg(short, long)]
    pub detailed: bool,
}

/// Arguments for TUI command
#[derive(Parser, Debug)]
pub struct TuiArgs {
    /// Start in a specific view (status, issues, config)
    #[arg(short = 'V', long)]
    pub view: Option<String>,
}

/// Arguments for fix command
#[derive(Parser, Debug)]
pub struct FixArgs {
    /// Issue ID to fix
    pub issue_id: String,

    /// Apply fix without confirmation
    #[arg(short, long)]
    pub yes: bool,

    /// Custom fix content (instead of suggested)
    #[arg(short, long)]
    pub content: Option<String>,

    /// Commit the fix automatically
    #[arg(long)]
    pub commit: bool,
}

/// Arguments for ignore command
#[derive(Parser, Debug)]
pub struct IgnoreArgs {
    /// Issue ID to ignore
    pub issue_id: String,

    /// Reason for ignoring
    #[arg(short, long)]
    pub reason: Option<String>,

    /// Ignore permanently (add to config)
    #[arg(long)]
    pub permanent: bool,
}

/// Arguments for hooks command
#[derive(Parser, Debug)]
pub struct HooksArgs {
    /// Install hooks
    #[arg(long)]
    pub install: bool,

    /// Uninstall hooks
    #[arg(long)]
    pub uninstall: bool,

    /// Show hook status
    #[arg(long)]
    pub status: bool,
}

/// Arguments for watch command
#[derive(Parser, Debug)]
pub struct WatchArgs {
    /// Debounce interval in milliseconds
    #[arg(short, long, default_value = "1000")]
    pub debounce: u64,

    /// Run in background
    #[arg(short, long)]
    pub background: bool,
}

/// Arguments for config command
#[derive(Parser, Debug)]
pub struct ConfigArgs {
    /// Show current configuration
    #[arg(long)]
    pub show: bool,

    /// Set a configuration value
    #[arg(long)]
    pub set: Option<String>,

    /// Get a configuration value
    #[arg(long)]
    pub get: Option<String>,

    /// Reset to defaults
    #[arg(long)]
    pub reset: bool,
}

/// Arguments for analyze command
#[derive(Parser, Debug)]
pub struct AnalyzeArgs {
    /// File or symbol to analyze
    pub target: String,

    /// Show related documentation
    #[arg(short, long)]
    pub docs: bool,

    /// Show embedding similarity scores
    #[arg(short, long)]
    pub similarity: bool,
}

/// Arguments for generate command
#[derive(Parser, Debug)]
pub struct GenerateArgs {
    /// Generate README.md
    #[arg(long)]
    pub readme: bool,

    /// Generate full documentation
    #[arg(long)]
    pub docs: bool,

    /// Output file (defaults to stdout)
    #[arg(long)]
    pub output: Option<String>,

    /// Include private symbols
    #[arg(long)]
    pub include_private: bool,

    /// Use LLM to generate natural language descriptions
    #[arg(long)]
    pub with_llm: bool,
}

impl Cli {
    /// Parse command line arguments
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let cli = Cli::parse_from(["docsentinel", "scan", "--full"]);
        assert!(matches!(cli.command, Commands::Scan(_)));

        if let Commands::Scan(args) = cli.command {
            assert!(args.full);
        }
    }

    #[test]
    fn test_init_command() {
        let cli = Cli::parse_from(["docsentinel", "init", "--force"]);
        if let Commands::Init(args) = cli.command {
            assert!(args.force);
        }
    }
}
