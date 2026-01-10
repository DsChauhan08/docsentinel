//! Database schema definition

/// SQL schema for the DocSentinel database
pub const SCHEMA: &str = r#"
-- Scan state tracking
CREATE TABLE IF NOT EXISTS scan_state (
    id INTEGER PRIMARY KEY,
    commit_hash TEXT NOT NULL,
    scanned_at TEXT NOT NULL
);

-- Code chunks extracted from source files
CREATE TABLE IF NOT EXISTS code_chunks (
    id TEXT PRIMARY KEY,
    file_path TEXT NOT NULL,
    symbol_name TEXT NOT NULL,
    symbol_type TEXT NOT NULL,
    content TEXT NOT NULL,
    hash TEXT NOT NULL,
    language TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    doc_comment TEXT,
    signature TEXT,
    is_public INTEGER NOT NULL DEFAULT 0,
    embedding BLOB,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_code_chunks_file ON code_chunks(file_path);
CREATE INDEX IF NOT EXISTS idx_code_chunks_hash ON code_chunks(hash);
CREATE INDEX IF NOT EXISTS idx_code_chunks_symbol ON code_chunks(symbol_name);

-- Documentation chunks extracted from markdown files
CREATE TABLE IF NOT EXISTS doc_chunks (
    id TEXT PRIMARY KEY,
    file_path TEXT NOT NULL,
    heading_path TEXT NOT NULL,
    heading TEXT NOT NULL,
    level INTEGER NOT NULL,
    content TEXT NOT NULL,
    hash TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    embedding BLOB,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_doc_chunks_file ON doc_chunks(file_path);
CREATE INDEX IF NOT EXISTS idx_doc_chunks_hash ON doc_chunks(hash);
CREATE INDEX IF NOT EXISTS idx_doc_chunks_heading ON doc_chunks(heading);

-- Drift events detected between code and documentation
CREATE TABLE IF NOT EXISTS drift_events (
    id TEXT PRIMARY KEY,
    severity TEXT NOT NULL,
    description TEXT NOT NULL,
    evidence TEXT NOT NULL,
    confidence REAL NOT NULL,
    related_code_chunks TEXT NOT NULL,
    related_doc_chunks TEXT NOT NULL,
    suggested_fix TEXT,
    status TEXT NOT NULL DEFAULT 'Pending',
    detected_at TEXT NOT NULL,
    resolved_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_drift_events_status ON drift_events(status);
CREATE INDEX IF NOT EXISTS idx_drift_events_severity ON drift_events(severity);
CREATE INDEX IF NOT EXISTS idx_drift_events_detected ON drift_events(detected_at);

-- Chunk relationships (code <-> doc associations)
CREATE TABLE IF NOT EXISTS chunk_relationships (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code_chunk_id TEXT NOT NULL,
    doc_chunk_id TEXT NOT NULL,
    similarity REAL NOT NULL,
    relationship_type TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (code_chunk_id) REFERENCES code_chunks(id) ON DELETE CASCADE,
    FOREIGN KEY (doc_chunk_id) REFERENCES doc_chunks(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_relationships_code ON chunk_relationships(code_chunk_id);
CREATE INDEX IF NOT EXISTS idx_relationships_doc ON chunk_relationships(doc_chunk_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_relationships_pair ON chunk_relationships(code_chunk_id, doc_chunk_id);

-- Historical snapshots for tracking changes over time
CREATE TABLE IF NOT EXISTS chunk_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chunk_id TEXT NOT NULL,
    chunk_type TEXT NOT NULL,
    content TEXT NOT NULL,
    hash TEXT NOT NULL,
    commit_hash TEXT,
    recorded_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_history_chunk ON chunk_history(chunk_id);
CREATE INDEX IF NOT EXISTS idx_history_commit ON chunk_history(commit_hash);

-- Configuration key-value store
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;
