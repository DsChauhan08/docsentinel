//! DocSentinel - Local-first documentation drift detection and fixing tool
//!
//! This library provides the core functionality for detecting when documentation
//! no longer matches code, explaining why, and optionally proposing fixes.

pub mod cli;
pub mod drift;
pub mod extract;
pub mod llm;
pub mod repo;
pub mod storage;
pub mod tui;

/// Re-export commonly used types
pub use drift::{DriftDetector, DriftEvent, DriftSeverity};
pub use extract::{CodeChunk, DocChunk};
pub use repo::Repository;
pub use storage::Database;

/// Application-wide error type
pub use anyhow::Result;

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_NAME: &str = "docsentinel";
