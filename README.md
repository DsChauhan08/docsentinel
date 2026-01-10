# DocSentinel

**Local-first documentation drift detection and fixing tool**

DocSentinel detects when documentation no longer matches code, explains why, and optionally proposes fixes using a locally-run or user-supplied LLM.

## The Problem

In real codebases, documentation does not fail loudly. It rots quietly. APIs change, function behavior shifts, flags are added, defaults change, and the docs continue to assert something that is no longer true. This causes onboarding friction, bugs, and operational mistakes.

**The real problem is not writing documentation. It is detecting when documentation is wrong.**

DocSentinel answers one question reliably:
> Which parts of my documentation are now inconsistent with the code, and why?

## Core Principles

- **Local-first by default** - Runs entirely on your machine with no network dependency unless explicitly enabled
- **Explainability over automation** - Every detected issue shows evidence. Silent fixes are forbidden
- **Narrow scope** - Does not manage documentation. Detects drift and proposes changes
- **Open core** - Free version is fully usable. Paid features provide automation, hosting, and convenience

## Installation

### From Source

```bash
git clone https://github.com/docsentinel/docsentinel
cd docsentinel
cargo build --release
```

The binary will be at `target/release/docsentinel`.

### From Cargo

```bash
cargo install docsentinel
```

## Quick Start

```bash
# Initialize DocSentinel in your repository
docsentinel init

# Scan for documentation drift
docsentinel scan

# View detected issues
docsentinel status

# Launch interactive TUI
docsentinel tui
```

## Commands

### `init`

Initialize DocSentinel in a repository.

```bash
docsentinel init [--force] [--no-scan]
```

Creates a `.docsentinel` directory with:
- SQLite database for storing chunks and drift events
- Configuration file (`config.toml`)

### `scan`

Scan the repository for documentation drift.

```bash
docsentinel scan [--full] [--range <RANGE>] [--uncommitted] [--with-llm]
```

Options:
- `--full`: Scan all files, not just changed ones
- `--range`: Commit range to scan (e.g., "HEAD~5..HEAD")
- `--uncommitted`: Include uncommitted changes
- `--with-llm`: Use LLM for analysis

### `status`

Show detected drift issues.

```bash
docsentinel status [--all] [--severity <LEVEL>] [--detailed]
```

### `tui`

Launch the interactive terminal user interface.

```bash
docsentinel tui
```

The TUI provides:
- Dashboard with repository statistics
- Issue list with navigation
- Detailed issue view
- Fix editor with side-by-side diff

### `fix`

Apply a suggested fix to a drift issue.

```bash
docsentinel fix <ISSUE_ID> [--yes] [--content <TEXT>] [--commit]
```

### `ignore`

Ignore a drift issue.

```bash
docsentinel ignore <ISSUE_ID> [--reason <TEXT>] [--permanent]
```

### `hooks`

Install or manage git hooks.

```bash
docsentinel hooks [--install] [--uninstall] [--status]
```

### `watch`

Watch for changes and scan automatically.

```bash
docsentinel watch [--debounce <MS>] [--background]
```

### `config`

Show or modify configuration.

```bash
docsentinel config [--show] [--set <KEY=VALUE>] [--get <KEY>] [--reset]
```

### `analyze`

Analyze a specific file or symbol.

```bash
docsentinel analyze <TARGET> [--docs] [--similarity]
```

## Configuration

Configuration is stored in `.docsentinel/config.toml`:

```toml
# Patterns for documentation files
doc_patterns = ["*.md", "*.mdx", "*.rst", "docs/**/*"]

# Patterns for code files
code_patterns = ["*.rs", "*.py", "src/**/*.rs"]

# Patterns to ignore
ignore_patterns = ["target/**", "node_modules/**"]

# Languages to analyze
languages = ["rust", "python"]

# Similarity threshold for drift detection (0.0 - 1.0)
similarity_threshold = 0.7

# Number of nearest doc chunks to consider
top_k = 5

# LLM configuration
[llm]
endpoint = "http://localhost:11434"
model = "llama2"
max_tokens = 2048
temperature = 0.3
```

## How It Works

### 1. Repository Ingestion

DocSentinel operates on Git repositories. On each scan:
- Identifies commit range since last scan
- Extracts changed files
- Categorizes changes into code and documentation

### 2. Code Extraction

Uses tree-sitter to extract semantically meaningful units:
- Public function definitions
- Method signatures
- Structs / classes
- Doc comments

Supported languages (v1): **Rust** and **Python**

### 3. Documentation Extraction

Parses Markdown files by heading hierarchy. Each section becomes a "Doc Chunk" with:
- File path
- Heading path
- Raw content
- Content hash

### 4. Drift Detection

Drift is detected by:

**Hard Rules:**
- Public API signature change without corresponding doc change
- Removed functions still documented
- Parameter changes not reflected in docs

**Soft Rules:**
- Behavioral changes inferred from code comments or logic
- Doc comment changes without external doc updates

### 5. LLM-Assisted Analysis (Optional)

When enabled, the LLM is invoked only after drift is detected:
- Analyzes old code, new code, and related documentation
- Explains why documentation is incorrect
- Suggests updated text

Supports:
- Ollama (local)
- Any OpenAI-compatible endpoint

## TUI Keyboard Shortcuts

### Global
- `Ctrl+C`, `Ctrl+Q` - Quit
- `?` - Show help

### Dashboard
- `i`, `Enter` - View issues
- `s` - Run scan
- `q` - Quit

### Issues List
- `↑/k`, `↓/j` - Navigate
- `Enter` - View details
- `f` - Open fix editor
- `x` - Ignore issue
- `Esc` - Back to dashboard

### Fix Editor
- `e` - Edit fix
- `a` - Apply fix
- `Esc` - Cancel

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        DocSentinel                          │
├─────────────────────────────────────────────────────────────┤
│  CLI (clap)                    TUI (ratatui)                │
├─────────────────────────────────────────────────────────────┤
│                     Drift Detection Engine                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Hard Rules  │  │ Soft Rules  │  │ Semantic Similarity │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────────────────────┐   │
│  │ Code Extraction │  │ Documentation Extraction        │   │
│  │ (tree-sitter)   │  │ (pulldown-cmark)                │   │
│  └─────────────────┘  └─────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────────────────────┐   │
│  │ Git Integration │  │ SQLite Storage                  │   │
│  │ (git2)          │  │ (rusqlite)                      │   │
│  └─────────────────┘  └─────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────┐    │
│  │ LLM Integration (Ollama / OpenAI-compatible)        │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Running

```bash
cargo run -- init
cargo run -- scan
cargo run -- tui
```

## Roadmap

- [x] Phase 1: Core scanning and drift detection
- [x] Phase 2: LLM explanation and fix proposal
- [x] Phase 3: TUI refinement
- [ ] Phase 4: GitHub integration
- [ ] Phase 5: Paid automation

## License

MIT OR Apache-2.0

## Contributing

Contributions are welcome! Please read our contributing guidelines before submitting PRs.

---

*This tool succeeds if developers trust it enough to run it regularly. Every design decision biases toward correctness, transparency, and respect for the user's workflow. Automation comes second. Trust comes first.*
