//! Command implementations

use crate::drift::{DriftDetector, DriftEvent, DriftSeverity};
use crate::extract::{CodeExtractor, DocExtractor};
use crate::repo::{FileType, Repository};
use crate::storage::Database;
use anyhow::{Context, Result};
use std::path::Path;

/// Initialize DocSentinel in a repository
pub fn init(path: &Path, force: bool) -> Result<()> {
    let repo = Repository::open(path)?;

    let sentinel_dir = repo.sentinel_dir();
    if sentinel_dir.exists() && !force {
        anyhow::bail!(
            "DocSentinel already initialized. Use --force to re-initialize."
        );
    }

    // Create sentinel directory
    repo.init_sentinel_dir()?;

    // Initialize database
    let db_path = sentinel_dir.join("docsentinel.db");
    let _db = Database::open(&db_path)?;

    // Save default config
    repo.config().save(repo.root())?;

    println!("✓ Initialized DocSentinel in {:?}", repo.root());
    println!("  Database: {:?}", db_path);
    println!("  Config: {:?}", sentinel_dir.join("config.toml"));

    Ok(())
}

/// Scan the repository for drift
pub fn scan(
    path: &Path,
    full: bool,
    range: Option<&str>,
    uncommitted: bool,
) -> Result<Vec<DriftEvent>> {
    let repo = Repository::open(path)?;
    let sentinel_dir = repo.sentinel_dir();

    if !sentinel_dir.exists() {
        anyhow::bail!("DocSentinel not initialized. Run 'docsentinel init' first.");
    }

    let db_path = sentinel_dir.join("docsentinel.db");
    let db = Database::open(&db_path)?;

    // Determine what to scan
    let (from_commit, to_commit) = if let Some(range_str) = range {
        // Parse range like "HEAD~5..HEAD"
        let parts: Vec<&str> = range_str.split("..").collect();
        if parts.len() == 2 {
            (Some(parts[0].to_string()), parts[1].to_string())
        } else {
            (None, range_str.to_string())
        }
    } else if full {
        (None, repo.head_commit()?)
    } else {
        let last_scan = db.get_last_scan_commit()?;
        (last_scan, repo.head_commit()?)
    };

    println!("Scanning repository...");
    if let Some(ref from) = from_commit {
        println!("  From: {}", from);
    }
    println!("  To: {}", to_commit);

    // Get changed files
    let changes = repo.changes_between(from_commit.as_deref(), &to_commit)?;

    let code_changes: Vec<_> = changes.iter().filter(|c| c.is_code()).collect();
    let doc_changes: Vec<_> = changes.iter().filter(|c| c.is_documentation()).collect();

    println!("  Code files changed: {}", code_changes.len());
    println!("  Doc files changed: {}", doc_changes.len());

    // Extract code chunks
    let mut code_extractor = CodeExtractor::new()?;
    let mut all_code_chunks = Vec::new();

    for change in &code_changes {
        if let Some(content) = repo.read_file_current(&change.path)? {
            match code_extractor.extract_file(&change.path, &content) {
                Ok(chunks) => {
                    for chunk in chunks {
                        db.upsert_code_chunk(&chunk)?;
                        all_code_chunks.push(chunk);
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to extract {:?}: {}", change.path, e);
                }
            }
        }
    }

    // Extract doc chunks
    let doc_extractor = DocExtractor::new();
    let mut all_doc_chunks = Vec::new();

    for change in &doc_changes {
        if let Some(content) = repo.read_file_current(&change.path)? {
            match doc_extractor.extract_file(&change.path, &content) {
                Ok(chunks) => {
                    for chunk in chunks {
                        db.upsert_doc_chunk(&chunk)?;
                        all_doc_chunks.push(chunk);
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to extract {:?}: {}", change.path, e);
                }
            }
        }
    }

    println!("  Code chunks: {}", all_code_chunks.len());
    println!("  Doc chunks: {}", all_doc_chunks.len());

    // Detect drift
    let detector = DriftDetector::new();

    // For now, use a simplified detection without embeddings
    let mut events = Vec::new();

    // Check for code changes without corresponding doc changes
    if !code_changes.is_empty() && doc_changes.is_empty() {
        for code_change in &code_changes {
            // Check if this is a public API file
            let chunks: Vec<_> = all_code_chunks
                .iter()
                .filter(|c| c.file_path == code_change.path.to_string_lossy())
                .filter(|c| c.is_public)
                .collect();

            if !chunks.is_empty() {
                let event = DriftEvent::new(
                    DriftSeverity::Medium,
                    &format!(
                        "Code changed in {:?} but no documentation was updated",
                        code_change.path
                    ),
                    &format!(
                        "{} public symbols modified: {}",
                        chunks.len(),
                        chunks
                            .iter()
                            .map(|c| c.symbol_name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    0.7,
                );
                events.push(event);
            }
        }
    }

    // Store drift events
    for event in &events {
        db.insert_drift_event(event)?;
    }

    // Update last scan commit
    db.set_last_scan_commit(&to_commit)?;

    println!("\n✓ Scan complete");
    println!("  Drift events detected: {}", events.len());

    Ok(events)
}

/// Show status of drift issues
pub fn status(path: &Path, all: bool, severity: Option<&str>) -> Result<()> {
    let repo = Repository::open(path)?;
    let sentinel_dir = repo.sentinel_dir();

    if !sentinel_dir.exists() {
        anyhow::bail!("DocSentinel not initialized. Run 'docsentinel init' first.");
    }

    let db_path = sentinel_dir.join("docsentinel.db");
    let db = Database::open(&db_path)?;

    let stats = db.get_stats()?;
    let events = db.get_unresolved_drift_events()?;

    println!("DocSentinel Status");
    println!("==================\n");

    println!("Repository: {:?}", repo.root());
    println!("Code chunks: {}", stats.code_chunks);
    println!("Doc chunks: {}", stats.doc_chunks);
    println!("Total drift events: {}", stats.drift_events);
    println!("Pending events: {}", stats.pending_events);

    if events.is_empty() {
        println!("\n✓ No pending drift issues!");
        return Ok(());
    }

    println!("\nPending Issues:");
    println!("---------------\n");

    for event in &events {
        // Filter by severity if specified
        if let Some(sev) = severity {
            let event_sev = format!("{:?}", event.severity).to_lowercase();
            if !event_sev.contains(&sev.to_lowercase()) {
                continue;
            }
        }

        let severity_icon = match event.severity {
            DriftSeverity::Critical => "🔴",
            DriftSeverity::High => "🟠",
            DriftSeverity::Medium => "🟡",
            DriftSeverity::Low => "🟢",
        };

        println!("{} [{}] {}", severity_icon, event.severity, event.description);
        println!("   ID: {}", &event.id[..8]);
        println!("   Confidence: {:.0}%", event.confidence * 100.0);
        println!("   Evidence: {}", event.evidence);
        println!();
    }

    Ok(())
}

/// Apply a fix to a drift issue
pub fn fix(path: &Path, issue_id: &str, content: Option<&str>, commit: bool) -> Result<()> {
    let repo = Repository::open(path)?;
    let sentinel_dir = repo.sentinel_dir();

    if !sentinel_dir.exists() {
        anyhow::bail!("DocSentinel not initialized. Run 'docsentinel init' first.");
    }

    let db_path = sentinel_dir.join("docsentinel.db");
    let db = Database::open(&db_path)?;

    // Find the event
    let event = db
        .get_drift_event(issue_id)
        .context("Failed to find drift event")?
        .ok_or_else(|| anyhow::anyhow!("Drift event not found: {}", issue_id))?;

    println!("Fixing: {}", event.description);

    // Get the fix content
    let fix_content = if let Some(c) = content {
        c.to_string()
    } else if let Some(ref suggested) = event.suggested_fix {
        suggested.clone()
    } else {
        anyhow::bail!("No fix content provided and no suggested fix available");
    };

    // Apply the fix
    if let Some(doc_id) = event.related_doc_chunks.first() {
        if let Some(doc_chunk) = db.get_doc_chunk(doc_id)? {
            let file_path = repo.root().join(&doc_chunk.file_path);

            // Read current content
            let current = std::fs::read_to_string(&file_path)?;

            // Replace the section
            let updated = current.replace(&doc_chunk.content, &fix_content);

            // Write back
            std::fs::write(&file_path, updated)?;

            println!("✓ Updated {:?}", file_path);

            // Update event status
            db.update_drift_event_status(issue_id, "Fixed")?;

            if commit {
                // TODO: Implement git commit
                println!("Note: Auto-commit not yet implemented");
            }
        }
    }

    Ok(())
}

/// Ignore a drift issue
pub fn ignore(path: &Path, issue_id: &str, reason: Option<&str>) -> Result<()> {
    let repo = Repository::open(path)?;
    let sentinel_dir = repo.sentinel_dir();

    if !sentinel_dir.exists() {
        anyhow::bail!("DocSentinel not initialized. Run 'docsentinel init' first.");
    }

    let db_path = sentinel_dir.join("docsentinel.db");
    let db = Database::open(&db_path)?;

    db.update_drift_event_status(issue_id, "Ignored")?;

    println!("✓ Ignored drift event: {}", issue_id);
    if let Some(r) = reason {
        println!("  Reason: {}", r);
    }

    Ok(())
}

/// Install or manage git hooks
pub fn hooks(path: &Path, install: bool, uninstall: bool) -> Result<()> {
    let repo = Repository::open(path)?;
    let hooks_dir = repo.root().join(".git").join("hooks");

    if install {
        let post_commit = hooks_dir.join("post-commit");

        let hook_content = r#"#!/bin/sh
# DocSentinel post-commit hook
docsentinel scan --uncommitted
"#;

        std::fs::write(&post_commit, hook_content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&post_commit)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&post_commit, perms)?;
        }

        println!("✓ Installed post-commit hook");
    }

    if uninstall {
        let post_commit = hooks_dir.join("post-commit");
        if post_commit.exists() {
            std::fs::remove_file(&post_commit)?;
            println!("✓ Removed post-commit hook");
        }
    }

    if !install && !uninstall {
        // Show status
        let post_commit = hooks_dir.join("post-commit");
        if post_commit.exists() {
            println!("post-commit hook: installed");
        } else {
            println!("post-commit hook: not installed");
        }
    }

    Ok(())
}

/// Print events in JSON format
pub fn print_events_json(events: &[DriftEvent]) -> Result<()> {
    let json = serde_json::to_string_pretty(events)?;
    println!("{}", json);
    Ok(())
}

/// Print events in text format
pub fn print_events_text(events: &[DriftEvent]) {
    if events.is_empty() {
        println!("No drift events detected.");
        return;
    }

    println!("\nDetected Drift Events:");
    println!("======================\n");

    for event in events {
        let severity_icon = match event.severity {
            DriftSeverity::Critical => "🔴",
            DriftSeverity::High => "🟠",
            DriftSeverity::Medium => "🟡",
            DriftSeverity::Low => "🟢",
        };

        println!("{} [{}] {}", severity_icon, event.severity, event.description);
        println!("   Confidence: {:.0}%", event.confidence * 100.0);
        println!("   Evidence: {}", event.evidence);
        println!();
    }
}
