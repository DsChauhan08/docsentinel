//! DocSentinel - Documentation drift detection tool
//!
//! A local-first tool that detects when documentation no longer matches code,
//! explains why, and optionally proposes fixes.

use anyhow::Result;
use docsentinel::cli::{
    fix, generate, hooks, ignore, init, print_events_json, print_events_text, scan, status, Cli,
    Commands, OutputFormat,
};
use std::path::Path;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse_args();

    // Setup logging
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    // Get repository path
    let repo_path = Path::new(&cli.path);

    // Execute command
    match cli.command {
        Commands::Init(args) => {
            init(repo_path, args.force, args.quick)?;

            if !args.no_scan && !args.quick {
                println!("Running initial scan...\n");
                let events = scan(repo_path, true, None, false)?;

                match cli.format {
                    OutputFormat::Json => print_events_json(&events)?,
                    OutputFormat::Text => print_events_text(&events),
                }
            }
        }

        Commands::Scan(args) => {
            let events = scan(
                repo_path,
                args.full,
                args.range.as_deref(),
                args.uncommitted,
            )?;

            match cli.format {
                OutputFormat::Json => print_events_json(&events)?,
                OutputFormat::Text => print_events_text(&events),
            }
        }

        Commands::Status(args) => {
            status(repo_path, args.all, args.severity.as_deref())?;
        }

        Commands::Tui(_args) => {
            docsentinel::tui::run(repo_path)?;
        }

        Commands::Fix(args) => {
            fix(
                repo_path,
                &args.issue_id,
                args.content.as_deref(),
                args.commit,
            )?;
        }

        Commands::Ignore(args) => {
            ignore(repo_path, &args.issue_id, args.reason.as_deref())?;
        }

        Commands::Hooks(args) => {
            hooks(repo_path, args.install, args.uninstall)?;
        }

        Commands::Watch(args) => {
            run_watch(repo_path, args.debounce)?;
        }

        Commands::Config(args) => {
            handle_config(repo_path, &args)?;
        }

        Commands::Analyze(args) => {
            analyze(repo_path, &args.target, args.docs, args.similarity)?;
        }

        Commands::Generate(args) => {
            generate(
                repo_path,
                args.readme,
                args.docs,
                args.output.as_deref(),
                args.include_private,
                args.with_llm,
            )?;
        }
    }

    Ok(())
}

/// Run in watch mode
fn run_watch(path: &Path, debounce_ms: u64) -> Result<()> {
    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    println!("Watching for changes in {:?}...", path);
    println!("Press Ctrl+C to stop.\n");

    let (tx, rx) = channel();

    let config = Config::default().with_poll_interval(Duration::from_millis(debounce_ms));

    let mut watcher = RecommendedWatcher::new(tx, config)?;
    watcher.watch(path, RecursiveMode::Recursive)?;

    let mut last_scan = std::time::Instant::now();
    let debounce = Duration::from_millis(debounce_ms);

    loop {
        match rx.recv() {
            Ok(event) => {
                if let Ok(event) = event {
                    // Debounce
                    if last_scan.elapsed() < debounce {
                        continue;
                    }

                    // Check if it's a relevant file
                    let dominated_paths: Vec<_> = event
                        .paths
                        .iter()
                        .filter(|p| {
                            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                            matches!(ext, "rs" | "py" | "md" | "mdx" | "rst")
                        })
                        .collect();

                    if !dominated_paths.is_empty() {
                        println!("\n📝 Changes detected, scanning...");

                        match scan(path, false, None, true) {
                            Ok(events) => {
                                if events.is_empty() {
                                    println!("✓ No drift detected");
                                } else {
                                    println!("⚠ {} drift event(s) detected", events.len());
                                    print_events_text(&events);
                                }
                            }
                            Err(e) => {
                                eprintln!("Scan error: {}", e);
                            }
                        }

                        last_scan = std::time::Instant::now();
                    }
                }
            }
            Err(e) => {
                eprintln!("Watch error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Handle config command
fn handle_config(path: &Path, args: &docsentinel::cli::ConfigArgs) -> Result<()> {
    use docsentinel::repo::Repository;

    let repo = Repository::open(path)?;
    let config = repo.config();

    if args.show || (!args.reset && args.set.is_none() && args.get.is_none()) {
        println!("DocSentinel Configuration");
        println!("=========================\n");

        println!("Documentation patterns:");
        for pattern in &config.doc_patterns {
            println!("  - {}", pattern);
        }

        println!("\nCode patterns:");
        for pattern in &config.code_patterns {
            println!("  - {}", pattern);
        }

        println!("\nIgnore patterns:");
        for pattern in &config.ignore_patterns {
            println!("  - {}", pattern);
        }

        println!("\nLanguages: {:?}", config.languages);
        println!("Similarity threshold: {}", config.similarity_threshold);
        println!("Top K: {}", config.top_k);

        if let Some(ref endpoint) = config.llm.endpoint {
            println!("\nLLM endpoint: {}", endpoint);
        }
        if let Some(ref model) = config.llm.model {
            println!("LLM model: {}", model);
        }
    }

    if let Some(ref key) = args.get {
        match key.as_str() {
            "similarity_threshold" => println!("{}", config.similarity_threshold),
            "top_k" => println!("{}", config.top_k),
            _ => println!("Unknown config key: {}", key),
        }
    }

    if args.reset {
        let default_config = docsentinel::repo::RepoConfig::default();
        default_config.save(repo.root())?;
        println!("✓ Configuration reset to defaults");
    }

    Ok(())
}

/// Analyze a specific file or symbol
fn analyze(path: &Path, target: &str, show_docs: bool, _show_similarity: bool) -> Result<()> {
    use docsentinel::extract::{CodeExtractor, DocExtractor};
    use docsentinel::repo::Repository;
    use docsentinel::storage::Database;

    let repo = Repository::open(path)?;
    let sentinel_dir = repo.sentinel_dir();

    if !sentinel_dir.exists() {
        anyhow::bail!("DocSentinel not initialized. Run 'docsentinel init' first.");
    }

    let db_path = sentinel_dir.join("docsentinel.db");
    let db = Database::open(&db_path)?;

    let target_path = Path::new(target);

    if target_path.exists() {
        // Analyze a file
        let content = std::fs::read_to_string(target_path)?;
        let ext = target_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if matches!(ext, "rs" | "py") {
            let mut extractor = CodeExtractor::new()?;
            let chunks = extractor.extract_file(target_path, &content)?;

            println!("Code Analysis: {:?}", target_path);
            println!("================\n");

            for chunk in &chunks {
                println!("Symbol: {} ({})", chunk.symbol_name, chunk.symbol_type);
                println!("  Lines: {}-{}", chunk.start_line, chunk.end_line);
                println!("  Public: {}", chunk.is_public);
                if let Some(ref sig) = chunk.signature {
                    println!("  Signature: {}", sig);
                }
                if let Some(ref doc) = chunk.doc_comment {
                    println!("  Doc: {}", doc.lines().next().unwrap_or(""));
                }
                println!();
            }
        } else if matches!(ext, "md" | "mdx" | "rst") {
            let extractor = DocExtractor::new();
            let chunks = extractor.extract_file(target_path, &content)?;

            println!("Documentation Analysis: {:?}", target_path);
            println!("======================\n");

            for chunk in &chunks {
                println!("Section: {}", chunk.full_path());
                println!("  Lines: {}-{}", chunk.start_line, chunk.end_line);
                println!("  Level: {}", chunk.level);
                println!();
            }
        }
    } else {
        // Try to find as a symbol
        if let Some(chunk) = db.get_code_chunk(target)? {
            println!("Symbol: {}", chunk.symbol_name);
            println!("  File: {}", chunk.file_path);
            println!("  Type: {}", chunk.symbol_type);
            println!("  Lines: {}-{}", chunk.start_line, chunk.end_line);

            if show_docs {
                println!("\nRelated documentation:");

                let doc_chunks = db.get_all_doc_chunks_with_embeddings()?;

                if chunk.embedding.is_none() {
                    println!("  (No embeddings available for this code chunk)");
                } else if doc_chunks.is_empty() {
                    println!("  (No document chunks with embeddings found)");
                } else {
                    let code_embedding = chunk.embedding.as_ref().unwrap();
                    let mut similarities: Vec<_> = doc_chunks
                        .into_iter()
                        .filter_map(|doc| {
                            if let Some(ref doc_emb) = doc.embedding {
                                let similarity =
                                    docsentinel::drift::cosine_similarity(code_embedding, doc_emb);
                                Some((doc, similarity))
                            } else {
                                None
                            }
                        })
                        .collect();

                    similarities
                        .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                    for (doc, similarity) in similarities.iter().take(5) {
                        println!("  • {} ({:.1}%)", doc.full_path(), *similarity * 100.0);
                        println!("    File: {}", doc.file_path);
                        if *similarity > 0.7 {
                            println!("    {}", doc.content.lines().next().unwrap_or(""));
                        }
                        println!();
                    }

                    if similarities.is_empty() {
                        println!("  (No similar documentation found)");
                    }
                }
            }
        } else {
            println!("Target not found: {}", target);
        }
    }

    Ok(())
}
