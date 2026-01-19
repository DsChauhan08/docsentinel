# DocSentinel

**Local-first documentation drift detection and fixing tool**

[![Latest Release](https://img.shields.io/badge/v/release-0.1.0-blue)](https://github.com/docsentinel/docsentinel/releases)
[![License](https://img.shields.io/badge/license-MIT%20%7C%20Apache-2.0-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange)](https://www.rust-lang.org/)
[![CI](https://img.shields.io/badge/CI-passing-success)](.github/workflows/rust.yml)

DocSentinel detects when documentation no longer matches code, explains why, and optionally proposes fixes using a locally-run or user-supplied LLM.
Currently a bit stupid but working on making it more accurate and more perfect without an LLM model.
## Quick Summary

- **Purpose**: Detect semantic drift between code and documentation using AST-based extraction and vector similarity
- **Key Features**: Git-native workflow, multi-language support (Rust/Python), local-first operation, LLM-assisted analysis, TUI interface
- **Status**: Production-ready v0.1.0 | All CLI commands tested and functional | See [Competitive Analysis](#competitive-analysis) for positioning

## 🚧 Current Tasks & Known Issues

### ✅ Completed
- **Phase 1**: TUI module removal
  - Removed `src/tui/` directory (app.rs, mod.rs, ui.rs, widgets.rs)
  - Updated `src/lib.rs` (removed `pub mod tui`)
  - Updated `src/cli/mod.rs` (removed `Tui(TuiArgs)` from Commands enum)
  - Updated `src/main.rs` (removed TUI command handler)
  - Removed TUI dependencies from `Cargo.toml` (ratatui, crossterm)
  - ✅ Code compiles and builds cleanly

- **Phase 2**: Variable naming refactoring
  - Renamed `similarities` → `similarity_scores` (src/drift/detector.rs)
  - Renamed `i` → `doc_index`, `code_index` (src/drift/detector.rs)
  - Renamed `added`, `removed` → `added_params`, `removed_params` (src/drift/rules.rs)
  - Renamed `a`, `b`, `c`, `d` → `vec1`, `vec2`, `vec3`, `vec4` (src/drift/mod.rs tests)
  - Renamed `byte` → `hash_byte` (src/drift/embedding.rs)
  - Renamed `arr` → `byte_array` (src/storage/mod.rs)
  - Removed unused `_idx` variables (src/repo/mod.rs, repo/config.rs)
  - Used `next_back()` instead of `.last()` (src/cli/commands.rs)
  - ✅ Code compiles cleanly

### ⚠️ Partially Complete (Issues Present)
- **Phase 3**: Documentation generation overhaul
  - ✅ CLI arguments updated (`--human`, `--ai`, `--human-path`, `--ai-path`, `--architecture`, `--examples`)
  - ✅ `src/cli/mod.rs` GenerateArgs struct updated
  - ❌ Issue: Could not add `GenerateConfig` struct to `src/cli/commands.rs` due to file edit conflicts
  - ❌ Issue: Could not update `generate()` function signature in `src/cli/commands.rs` to use new `GenerateConfig`
  - ❌ Issue: Could not add helper functions (`generate_human_docs`, `generate_human_docs_with_llm`, `generate_ai_docs`)
  - ❌ Issue: Code compiles but using old `generate()` function
  - Current state: CLI args pass to main.rs, which calls old generate() with 6 positional params instead of GenerateConfig

### ❌ Not Started
- **Phase 4**: Code quality cleanup (remove dead code, fix unused warnings)
- **Phase 5**: Update Cargo.toml (remove TUI deps - already done in Phase 1)
- **Phase 6**: Testing & validation (add tests for new features)
- **Phase 7**: Documentation updates (README.md, CHANGELOG.md)
- **Agent.md update**: Add detailed plans for remaining phases

### 🔧 Action Required for Phase 3 Completion

**Problem**: Phase 3 has file edit conflicts preventing clean application of changes.

**Solution Steps**:
1. Restore `src/cli/commands.rs` to clean state
2. Add `GenerateConfig` struct at top of file
3. Update `generate()` function signature to accept `GenerateConfig`
4. Add helper functions at end of file:
   - `generate_human_docs()` - Generates human-readable OnboardDocs.md
   - `generate_human_docs_with_llm()` - LLM-enhanced version
   - `generate_ai_docs()` - Generates machine-readable OnboardAIdocs.md
5. Update `src/main.rs` to create `GenerateConfig` and pass to `generate()`
6. Test compilation: `cargo check`
7. Test functionality: `docsentinel generate --help` should show new flags

**Expected Outcomes**:
- Two separate documentation files: `OnboardDocs.md` (human) and `OnboardAIdocs.md` (AI)
- Human docs include architecture diagrams, examples, module overviews
- AI docs include structured type definitions, function references, cross-index
- Default behavior generates both if neither `--human` nor `--ai` specified

**Notes**:
- Current CLI still uses old `generate()` signature - needs careful manual update
- Consider starting fresh with restored file to avoid accumulated conflicts

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
- Dashboard with repository statistics (chunks, events, confidence scores)
- Issue list with navigation and filtering
- Detailed issue view with evidence display
- Fix editor with side-by-side diff preview
- Keyboard-driven workflow (see [Keyboard Shortcuts](#tui-keyboard-shortcuts))

**Note**: TUI requires terminal with cursor support and 256-color support. Windows Terminal may have limitations.

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

When `--docs` is provided, performs embedding-based search to find related documentation sections:
- Shows top 5 most similar doc chunks by cosine similarity
- Displays file paths and content previews
- Requires embeddings to be generated (use `--with-llm` or configure LLM)

### `generate`

Generate documentation from code chunks.

```bash
docsentinel generate --readme           # Generate README.md
docsentinel generate --docs             # Generate full documentation
docsentinel generate --include-private  # Include private symbols
docsentinel generate --with-llm         # Use LLM for descriptions
```

**Performance Notes:**
- Initialization: ~1s for small repos, ~10s for large repos (first scan)
- Incremental scan: <1s for small changes
- LLM analysis: ~2-5s per drift event (depends on model speed)
- Database: SQLite (sufficient for repos up to ~50K chunks)

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
- Identifies commit range since last scan (stored in SQLite)
- Extracts changed files using `git2` library
- Categorizes changes into code and documentation via glob patterns

### 2. Code Extraction

Uses **tree-sitter** to parse AST (Abstract Syntax Tree) and extract semantically meaningful units:
- Public function definitions
- Method signatures and their parameters
- Structs / classes / traits
- Doc comments (Rustdoc / Python docstrings)
- Signature extraction for drift comparison

**Supported languages (v1):**
- Rust (via tree-sitter-rust)
- Python (via tree-sitter-python)
- *(Extensible architecture for more languages)*

### 3. Documentation Extraction

Parses Markdown files using **pulldown-cmark** by heading hierarchy. Each section becomes a "Doc Chunk" with:
- File path and line range
- Heading path (e.g., `["API", "Functions", "user_create"]`)
- Section level (H1-H6)
- Raw content and SHA-256 hash
- Optional embedding vectors (384-dim for similarity search)

### 4. Embedding Generation (Optional)

When LLM is configured, DocSentinel generates embeddings:
- Code chunks: Symbol name + signature + content
- Doc chunks: Heading path + section content
- Stored as binary blobs in SQLite (f32 arrays)
- Enables semantic similarity search via cosine distance

**Embedding providers:**
- Ollama (local, default: `http://localhost:11434`)
- OpenAI-compatible endpoints (customizable)
- Mock embeddings (for testing without LLM)

### 5. Drift Detection Engine

Drift is detected through a hybrid approach:

**Hard Rules (Rule-based):**
- Public API signature changed → Check signature hash mismatch
- Function removed → Code chunk exists now, doc chunk deleted
- New function added → Code chunk exists, no related doc found
- Parameter count changed → Signature comparison

**Soft Rules (Semantic similarity):**
- Compute cosine similarity between code embedding and doc embeddings
- Similarity threshold: 0.7 (configurable)
- Top-K nearest docs: 5 (configurable)
- Significant drop detection (≥10% similarity decrease)

**Drift Event Structure:**
```json
{
  "id": "uuid",
  "severity": "High|Medium|Low|Critical",
  "description": "Human-readable summary",
  "evidence": "Technical details",
  "confidence": 0.0-1.0,
  "related_code_chunks": ["id1", "id2"],
  "related_doc_chunks": ["id1"],
  "suggested_fix": "LLM-generated (optional)",
  "status": "Pending|Accepted|Ignored|Fixed"
}
```

### 6. LLM-Assisted Analysis (Optional)

When drift is detected and LLM is configured:
- **Trigger**: Only after rule-based detection, not for every scan
- **Context provided**: Old code, new code, related docs, drift evidence
- **Prompt engineering**: Optimized for drift explanation + fix generation
- **Response format**: JSON with summary, reason, suggested_fix, confidence

**Supported providers:**
- Ollama (local, default: `llama2`)
- OpenAI-compatible (Anthropic, Together, local APIs)
- Custom endpoint support with API key authentication

**Use cases:**
- **`docsentinel scan --with-llm`**: Run drift analysis with LLM
- **`docsentinel fix <id>`**: Use LLM to generate fix suggestions
- **`docsentinel generate --with-llm`**: Generate natural language docs from code

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

## Known Limitations

### Current v0.1.0

- **Language Support**: Only Rust and Python (JavaScript/TypeScript, Go, Java planned)
- **LLM Required**: Advanced drift explanation requires Ollama or compatible LLM (basic rules work without it)
- **TUI**: Terminal UI requires terminal with cursor support (not tested in Windows Terminal)
- **Large Repositories**: Performance untested on >10K files (potential optimization needed)
- **Binary Compatibility**: Release binary tested on Linux, macOS/Windows support expected
- **Drift Detection**: Currently signature-based (behavioral drift via embeddings requires LLM)

### Planned Improvements (Roadmap)

See [Roadmap](#future-phases) for upcoming features addressing these limitations.

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

## Competitive Analysis

### Positioning in the Market

DocSentinel occupies a unique niche as a **local-first, AST-based documentation drift detection tool**. Unlike most documentation tools that focus on validation or linting, DocSentinel detects **semantic inconsistency between code and documentation over time**.

| Tool | Approach | Core Strength | Limitations | Local-First |
|-------|----------|--------------|--------------|--------------|
| **DocSentinel** | AST extraction + semantic embeddings + drift rules | Multi-language (Rust/Python), Git-native, TUI, offline-capable | ✅ Yes |
| **GenLint** | Change watching + consistency checks | Cloud integration (GitHub/Jira/Confluence), automated scanning | ❌ No (SaaS) |
| **Optic** | OpenAPI spec diffing | Breaking change prevention, accurate API docs | OpenAPI only, not general code | ❌ No |
| **Spectral** | OpenAPI linter with custom rules | Highly configurable, quality enforcement | OpenAPI only | ❌ No |
| **docsig** | Signature validation | Simple, focused approach | Rust only, semantic-only | ✅ Yes |
| **checkdoc** | Markdown quality linting | Format enforcement, basic checks | No code awareness | ✅ Yes |
| **diffsitter** | AST-based semantic diffs | Tree-sitter powered, ignores formatting | Diff tool only, no drift tracking | ✅ Yes |
| **resemble** | AST + cosine similarity (Rust) | Structural code comparison | Rust only, library not full tool | ✅ Yes |
| **tree-sitter-mcp** | Code structure for AI | Fast search, 15+ languages | Analysis only, no drift detection | ✅ Yes |

### Key Differentiators

1. **Git-Native Workflow**: Operates on commit ranges, not just file snapshots
2. **Semantic Understanding**: Uses tree-sitter AST extraction, not regex patterns
3. **Embedding-Powered Search**: Finds related docs via vector similarity (not just keyword matching)
4. **Explainability Over Automation**: Every drift event shows evidence, no silent fixes
5. **Local-First**: Full functionality without network/Cloud dependencies (LLM optional)
6. **Language Coverage**: Supports Rust and Python (v1), with extensible architecture

### Gaps vs Competitors

| Feature | DocSentinel | GenLint | Action |
|---------|-------------|---------|--------|
| CI/CD Integration | ❌ Missing | ✅ GitHub Actions | Add workflow examples |
| Pre-commit Hooks | ⚠️ Manual install | ✅ Auto-install | Document hooks integration |
| Web Dashboard | ❌ CLI only | ✅ Available | Could add in future phase |
| Multi-repo Support | ❌ Single repo | ❌ Single repo | Design choice, not gap |
| Slack/Discord Notifications | ❌ Missing | ✅ Available | Could add webhook support |

## Roadmap

### Current Status
- [x] Phase 1: Core scanning and drift detection
- [x] Phase 2: LLM explanation and fix proposal
- [x] Phase 3: TUI refinement

### Future Phases
- [ ] Phase 4: Ecosystem Integration
  - GitHub Actions workflow for drift checking
  - Pre-commit hook auto-installation
  - Webhook notifications for drift events
  - VS Code extension for inline warnings
  
- [ ] Phase 5: Enhanced Detection
  - Additional language support (JavaScript/TypeScript, Go, Java)
  - Configurable hard rules (custom drift patterns)
  - Diff visualization in TUI
  - Historical drift trends and analytics
  
- [ ] Phase 6: Collaboration Features
  - Team drift dashboards (self-hosted)
  - Pull request integration with drift summaries
  - Drift review approval workflows
  
- [ ] Phase 7: Enterprise (Open Core + Paid)
  - Self-hosted cloud version for teams
  - Advanced role-based permissions
  - Audit logs and compliance reporting
  - Priority support and SLAs

## License

MIT OR Apache-2.0

## Contributing

We welcome contributions! DocSentinel is designed with modularity in mind, making it easy to extend with new languages, drift rules, and embedding providers.

### Areas for Contribution

**Language Support:**
- Add new tree-sitter parsers in `src/extract/code.rs`
- Implement language-specific signature extraction logic
- Add tests for new language parsing

**Drift Rules:**
- Add custom hard rules in `src/drift/rules.rs`
- Implement new soft rule patterns
- Improve rule confidence scoring

**Integration:**
- Add pre-commit hook installation scripts
- Implement GitHub Actions workflow examples
- Add CI/CD pipeline detection examples

**Documentation:**
- Update this README when adding new commands
- Add usage examples for new features
- Test `--help` output for clarity

### Development Workflow

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes following existing code style
4. Run tests: `cargo test`
5. Run clippy: `cargo clippy -- -D warnings`
6. Commit changes: `git commit -m "Add amazing feature"`
7. Push: `git push origin feature/amazing-feature`
8. Open a Pull Request

### Testing

```bash
# Run all tests
cargo test

# Run with logging
RUST_LOG=debug cargo test

# Test specific module
cargo test extract::code::tests

# Run clippy (must pass)
cargo clippy -- -D warnings
```

## Dogfooding

We use [DocSentinel](https://github.com/docsentinel/docsentinel) to document the DocSentinel codebase. This ensures our own documentation remains up-to-date and verifies the tool's functionality in a real-world scenario.

---

*This tool succeeds if developers trust it enough to run it regularly. Every design decision biases toward correctness, transparency, and respect for the user's workflow. Automation comes second. Trust comes first.*
