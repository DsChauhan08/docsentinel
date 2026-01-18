//! SQLite storage layer for DocSentinel
//!
//! This module handles persistent storage of:
//! - Code chunks and their embeddings
//! - Documentation chunks and their embeddings
//! - Scan history and drift events
//! - Configuration state

mod schema;

pub use schema::SCHEMA;

use crate::drift::{DriftEvent, DriftSeverity};
use crate::extract::{CodeChunk, DocChunk};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

/// Database connection wrapper
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create a database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("Failed to open database at {:?}", path.as_ref()))?;

        let db = Self { conn };
        db.initialize()?;

        Ok(db)
    }

    /// Open an in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("Failed to open in-memory database")?;

        let db = Self { conn };
        db.initialize()?;

        Ok(db)
    }

    /// Initialize the database schema
    fn initialize(&self) -> Result<()> {
        self.conn
            .execute_batch(SCHEMA)
            .context("Failed to initialize database schema")?;
        Ok(())
    }

    // ==================== Scan State ====================

    /// Get the last scanned commit hash
    pub fn get_last_scan_commit(&self) -> Result<Option<String>> {
        let result = self
            .conn
            .query_row(
                "SELECT commit_hash FROM scan_state WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .context("Failed to get last scan commit")?;

        Ok(result)
    }

    /// Update the last scanned commit hash
    pub fn set_last_scan_commit(&self, commit: &str) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO scan_state (id, commit_hash, scanned_at) VALUES (1, ?1, datetime('now'))",
                params![commit],
            )
            .context("Failed to set last scan commit")?;
        Ok(())
    }

    // ==================== Code Chunks ====================

    /// Insert or update a code chunk
    pub fn upsert_code_chunk(&self, chunk: &CodeChunk) -> Result<()> {
        let embedding_blob = chunk
            .embedding
            .as_ref()
            .map(|e| e.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>());

        self.conn
            .execute(
                r#"
                INSERT INTO code_chunks (
                    id, file_path, symbol_name, symbol_type, content, hash,
                    language, start_line, end_line, doc_comment, signature,
                    is_public, embedding, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, datetime('now'))
                ON CONFLICT(id) DO UPDATE SET
                    file_path = excluded.file_path,
                    symbol_name = excluded.symbol_name,
                    symbol_type = excluded.symbol_type,
                    content = excluded.content,
                    hash = excluded.hash,
                    language = excluded.language,
                    start_line = excluded.start_line,
                    end_line = excluded.end_line,
                    doc_comment = excluded.doc_comment,
                    signature = excluded.signature,
                    is_public = excluded.is_public,
                    embedding = excluded.embedding,
                    updated_at = datetime('now')
                "#,
                params![
                    chunk.id,
                    chunk.file_path,
                    chunk.symbol_name,
                    format!("{:?}", chunk.symbol_type),
                    chunk.content,
                    chunk.hash,
                    chunk.language.to_string(),
                    chunk.start_line as i64,
                    chunk.end_line as i64,
                    chunk.doc_comment,
                    chunk.signature,
                    chunk.is_public,
                    embedding_blob,
                ],
            )
            .context("Failed to upsert code chunk")?;

        Ok(())
    }

    /// Get a code chunk by ID
    pub fn get_code_chunk(&self, id: &str) -> Result<Option<CodeChunk>> {
        let result = self
            .conn
            .query_row(
                r#"
                SELECT id, file_path, symbol_name, symbol_type, content, hash,
                       language, start_line, end_line, doc_comment, signature,
                       is_public, embedding
                FROM code_chunks WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(CodeChunkRow {
                        id: row.get(0)?,
                        file_path: row.get(1)?,
                        symbol_name: row.get(2)?,
                        symbol_type: row.get(3)?,
                        content: row.get(4)?,
                        hash: row.get(5)?,
                        language: row.get(6)?,
                        start_line: row.get(7)?,
                        end_line: row.get(8)?,
                        doc_comment: row.get(9)?,
                        signature: row.get(10)?,
                        is_public: row.get(11)?,
                        embedding: row.get(12)?,
                    })
                },
            )
            .optional()
            .context("Failed to get code chunk")?;

        Ok(result.map(|r| r.into_chunk()))
    }

    /// Get all code chunks for a file
    pub fn get_code_chunks_for_file(&self, file_path: &str) -> Result<Vec<CodeChunk>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, file_path, symbol_name, symbol_type, content, hash,
                   language, start_line, end_line, doc_comment, signature,
                   is_public, embedding
            FROM code_chunks WHERE file_path = ?1
            "#,
        )?;

        let rows = stmt.query_map(params![file_path], |row| {
            Ok(CodeChunkRow {
                id: row.get(0)?,
                file_path: row.get(1)?,
                symbol_name: row.get(2)?,
                symbol_type: row.get(3)?,
                content: row.get(4)?,
                hash: row.get(5)?,
                language: row.get(6)?,
                start_line: row.get(7)?,
                end_line: row.get(8)?,
                doc_comment: row.get(9)?,
                signature: row.get(10)?,
                is_public: row.get(11)?,
                embedding: row.get(12)?,
            })
        })?;

        let mut chunks = Vec::new();
        for row in rows {
            chunks.push(row?.into_chunk());
        }

        Ok(chunks)
    }

    /// Get all code chunks with embeddings
    pub fn get_all_code_chunks_with_embeddings(&self) -> Result<Vec<CodeChunk>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, file_path, symbol_name, symbol_type, content, hash,
                   language, start_line, end_line, doc_comment, signature,
                   is_public, embedding
            FROM code_chunks WHERE embedding IS NOT NULL
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(CodeChunkRow {
                id: row.get(0)?,
                file_path: row.get(1)?,
                symbol_name: row.get(2)?,
                symbol_type: row.get(3)?,
                content: row.get(4)?,
                hash: row.get(5)?,
                language: row.get(6)?,
                start_line: row.get(7)?,
                end_line: row.get(8)?,
                doc_comment: row.get(9)?,
                signature: row.get(10)?,
                is_public: row.get(11)?,
                embedding: row.get(12)?,
            })
        })?;

        let mut chunks = Vec::new();
        for row in rows {
            chunks.push(row?.into_chunk());
        }

        Ok(chunks)
    }

    /// Get all code chunks
    pub fn get_all_code_chunks(&self) -> Result<Vec<CodeChunk>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, file_path, symbol_name, symbol_type, content, hash,
                   language, start_line, end_line, doc_comment, signature,
                   is_public, embedding
            FROM code_chunks
            ORDER BY file_path, start_line
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(CodeChunkRow {
                id: row.get(0)?,
                file_path: row.get(1)?,
                symbol_name: row.get(2)?,
                symbol_type: row.get(3)?,
                content: row.get(4)?,
                hash: row.get(5)?,
                language: row.get(6)?,
                start_line: row.get(7)?,
                end_line: row.get(8)?,
                doc_comment: row.get(9)?,
                signature: row.get(10)?,
                is_public: row.get(11)?,
                embedding: row.get(12)?,
            })
        })?;

        let mut chunks = Vec::new();
        for row in rows {
            chunks.push(row?.into_chunk());
        }

        Ok(chunks)
    }

    /// Delete code chunks for a file
    pub fn delete_code_chunks_for_file(&self, file_path: &str) -> Result<usize> {
        let count = self
            .conn
            .execute(
                "DELETE FROM code_chunks WHERE file_path = ?1",
                params![file_path],
            )
            .context("Failed to delete code chunks")?;

        Ok(count)
    }

    // ==================== Doc Chunks ====================

    /// Insert or update a doc chunk
    pub fn upsert_doc_chunk(&self, chunk: &DocChunk) -> Result<()> {
        let embedding_blob = chunk
            .embedding
            .as_ref()
            .map(|e| e.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>());

        let heading_path_json = serde_json::to_string(&chunk.heading_path)?;

        self.conn
            .execute(
                r#"
                INSERT INTO doc_chunks (
                    id, file_path, heading_path, heading, level, content, hash,
                    start_line, end_line, embedding, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))
                ON CONFLICT(id) DO UPDATE SET
                    file_path = excluded.file_path,
                    heading_path = excluded.heading_path,
                    heading = excluded.heading,
                    level = excluded.level,
                    content = excluded.content,
                    hash = excluded.hash,
                    start_line = excluded.start_line,
                    end_line = excluded.end_line,
                    embedding = excluded.embedding,
                    updated_at = datetime('now')
                "#,
                params![
                    chunk.id,
                    chunk.file_path,
                    heading_path_json,
                    chunk.heading,
                    chunk.level as i32,
                    chunk.content,
                    chunk.hash,
                    chunk.start_line as i64,
                    chunk.end_line as i64,
                    embedding_blob,
                ],
            )
            .context("Failed to upsert doc chunk")?;

        Ok(())
    }

    /// Get a doc chunk by ID
    pub fn get_doc_chunk(&self, id: &str) -> Result<Option<DocChunk>> {
        let result = self
            .conn
            .query_row(
                r#"
                SELECT id, file_path, heading_path, heading, level, content, hash,
                       start_line, end_line, embedding
                FROM doc_chunks WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(DocChunkRow {
                        id: row.get(0)?,
                        file_path: row.get(1)?,
                        heading_path: row.get(2)?,
                        heading: row.get(3)?,
                        level: row.get(4)?,
                        content: row.get(5)?,
                        hash: row.get(6)?,
                        start_line: row.get(7)?,
                        end_line: row.get(8)?,
                        embedding: row.get(9)?,
                    })
                },
            )
            .optional()
            .context("Failed to get doc chunk")?;

        Ok(result.and_then(|r| r.into_chunk().ok()))
    }

    /// Get all doc chunks for a file
    pub fn get_doc_chunks_for_file(&self, file_path: &str) -> Result<Vec<DocChunk>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, file_path, heading_path, heading, level, content, hash,
                   start_line, end_line, embedding
            FROM doc_chunks WHERE file_path = ?1
            "#,
        )?;

        let rows = stmt.query_map(params![file_path], |row| {
            Ok(DocChunkRow {
                id: row.get(0)?,
                file_path: row.get(1)?,
                heading_path: row.get(2)?,
                heading: row.get(3)?,
                level: row.get(4)?,
                content: row.get(5)?,
                hash: row.get(6)?,
                start_line: row.get(7)?,
                end_line: row.get(8)?,
                embedding: row.get(9)?,
            })
        })?;

        let mut chunks = Vec::new();
        for row in rows {
            if let Ok(chunk) = row?.into_chunk() {
                chunks.push(chunk);
            }
        }

        Ok(chunks)
    }

    /// Get all doc chunks with embeddings
    pub fn get_all_doc_chunks_with_embeddings(&self) -> Result<Vec<DocChunk>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, file_path, heading_path, heading, level, content, hash,
                   start_line, end_line, embedding
            FROM doc_chunks WHERE embedding IS NOT NULL
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(DocChunkRow {
                id: row.get(0)?,
                file_path: row.get(1)?,
                heading_path: row.get(2)?,
                heading: row.get(3)?,
                level: row.get(4)?,
                content: row.get(5)?,
                hash: row.get(6)?,
                start_line: row.get(7)?,
                end_line: row.get(8)?,
                embedding: row.get(9)?,
            })
        })?;

        let mut chunks = Vec::new();
        for row in rows {
            if let Ok(chunk) = row?.into_chunk() {
                chunks.push(chunk);
            }
        }

        Ok(chunks)
    }

    /// Delete doc chunks for a file
    pub fn delete_doc_chunks_for_file(&self, file_path: &str) -> Result<usize> {
        let count = self
            .conn
            .execute(
                "DELETE FROM doc_chunks WHERE file_path = ?1",
                params![file_path],
            )
            .context("Failed to delete doc chunks")?;

        Ok(count)
    }

    // ==================== Drift Events ====================

    /// Insert a drift event
    pub fn insert_drift_event(&self, event: &DriftEvent) -> Result<()> {
        let related_code_json = serde_json::to_string(&event.related_code_chunks)?;
        let related_doc_json = serde_json::to_string(&event.related_doc_chunks)?;

        self.conn
            .execute(
                r#"
                INSERT INTO drift_events (
                    id, severity, description, evidence, confidence,
                    related_code_chunks, related_doc_chunks, suggested_fix,
                    status, detected_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))
                "#,
                params![
                    event.id,
                    format!("{:?}", event.severity),
                    event.description,
                    event.evidence,
                    event.confidence,
                    related_code_json,
                    related_doc_json,
                    event.suggested_fix,
                    format!("{:?}", event.status),
                ],
            )
            .context("Failed to insert drift event")?;

        Ok(())
    }

    /// Get all unresolved drift events
    pub fn get_unresolved_drift_events(&self) -> Result<Vec<DriftEvent>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, severity, description, evidence, confidence,
                   related_code_chunks, related_doc_chunks, suggested_fix,
                   status, detected_at
            FROM drift_events WHERE status = 'Pending'
            ORDER BY confidence DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(DriftEventRow {
                id: row.get(0)?,
                severity: row.get(1)?,
                description: row.get(2)?,
                evidence: row.get(3)?,
                confidence: row.get(4)?,
                related_code_chunks: row.get(5)?,
                related_doc_chunks: row.get(6)?,
                suggested_fix: row.get(7)?,
                status: row.get(8)?,
                detected_at: row.get(9)?,
            })
        })?;

        let mut events = Vec::new();
        for row in rows {
            if let Ok(event) = row?.into_event() {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Update drift event status
    pub fn update_drift_event_status(&self, id: &str, status: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE drift_events SET status = ?1, resolved_at = datetime('now') WHERE id = ?2",
                params![status, id],
            )
            .context("Failed to update drift event status")?;

        Ok(())
    }

    /// Get drift event by ID
    pub fn get_drift_event(&self, id: &str) -> Result<Option<DriftEvent>> {
        let result = self
            .conn
            .query_row(
                r#"
                SELECT id, severity, description, evidence, confidence,
                       related_code_chunks, related_doc_chunks, suggested_fix,
                       status, detected_at
                FROM drift_events WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(DriftEventRow {
                        id: row.get(0)?,
                        severity: row.get(1)?,
                        description: row.get(2)?,
                        evidence: row.get(3)?,
                        confidence: row.get(4)?,
                        related_code_chunks: row.get(5)?,
                        related_doc_chunks: row.get(6)?,
                        suggested_fix: row.get(7)?,
                        status: row.get(8)?,
                        detected_at: row.get(9)?,
                    })
                },
            )
            .optional()
            .context("Failed to get drift event")?;

        Ok(result.and_then(|r| r.into_event().ok()))
    }

    // ==================== Statistics ====================

    /// Get database statistics
    pub fn get_stats(&self) -> Result<DatabaseStats> {
        let code_chunks: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM code_chunks", [], |row| row.get(0))?;

        let doc_chunks: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM doc_chunks", [], |row| row.get(0))?;

        let drift_events: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM drift_events", [], |row| row.get(0))?;

        let pending_events: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM drift_events WHERE status = 'Pending'",
            [],
            |row| row.get(0),
        )?;

        Ok(DatabaseStats {
            code_chunks: code_chunks as usize,
            doc_chunks: doc_chunks as usize,
            drift_events: drift_events as usize,
            pending_events: pending_events as usize,
        })
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub code_chunks: usize,
    pub doc_chunks: usize,
    pub drift_events: usize,
    pub pending_events: usize,
}

// Internal row types for database mapping

struct CodeChunkRow {
    id: String,
    file_path: String,
    symbol_name: String,
    symbol_type: String,
    content: String,
    hash: String,
    language: String,
    start_line: i64,
    end_line: i64,
    doc_comment: Option<String>,
    signature: Option<String>,
    is_public: bool,
    embedding: Option<Vec<u8>>,
}

impl CodeChunkRow {
    fn into_chunk(self) -> CodeChunk {
        use crate::extract::code::{Language, SymbolType};

        let language = match self.language.as_str() {
            "rust" => Language::Rust,
            "python" => Language::Python,
            _ => Language::Rust,
        };

        let symbol_type = match self.symbol_type.as_str() {
            "Function" => SymbolType::Function,
            "Method" => SymbolType::Method,
            "Struct" => SymbolType::Struct,
            "Class" => SymbolType::Class,
            "Enum" => SymbolType::Enum,
            "Trait" => SymbolType::Trait,
            "Impl" => SymbolType::Impl,
            "Module" => SymbolType::Module,
            "Constant" => SymbolType::Constant,
            _ => SymbolType::Function,
        };

        let embedding = self.embedding.map(|bytes| {
            bytes
                .chunks(4)
                .map(|chunk| {
                    let arr: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
                    f32::from_le_bytes(arr)
                })
                .collect()
        });

        CodeChunk {
            id: self.id,
            file_path: self.file_path,
            symbol_name: self.symbol_name,
            symbol_type,
            content: self.content,
            hash: self.hash,
            language,
            start_line: self.start_line as usize,
            end_line: self.end_line as usize,
            doc_comment: self.doc_comment,
            signature: self.signature,
            is_public: self.is_public,
            embedding,
        }
    }
}

struct DocChunkRow {
    id: String,
    file_path: String,
    heading_path: String,
    heading: String,
    level: i32,
    content: String,
    hash: String,
    start_line: i64,
    end_line: i64,
    embedding: Option<Vec<u8>>,
}

impl DocChunkRow {
    fn into_chunk(self) -> Result<DocChunk> {
        use crate::extract::doc::HeadingLevel;

        let heading_path: Vec<String> = serde_json::from_str(&self.heading_path)?;

        let level = match self.level {
            1 => HeadingLevel::H1,
            2 => HeadingLevel::H2,
            3 => HeadingLevel::H3,
            4 => HeadingLevel::H4,
            5 => HeadingLevel::H5,
            6 => HeadingLevel::H6,
            _ => HeadingLevel::H1,
        };

        let embedding = self.embedding.map(|bytes| {
            bytes
                .chunks(4)
                .map(|chunk| {
                    let arr: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
                    f32::from_le_bytes(arr)
                })
                .collect()
        });

        Ok(DocChunk {
            id: self.id,
            file_path: self.file_path,
            heading_path,
            heading: self.heading,
            level,
            content: self.content,
            hash: self.hash,
            start_line: self.start_line as usize,
            end_line: self.end_line as usize,
            embedding,
        })
    }
}

struct DriftEventRow {
    id: String,
    severity: String,
    description: String,
    evidence: String,
    confidence: f64,
    related_code_chunks: String,
    related_doc_chunks: String,
    suggested_fix: Option<String>,
    status: String,
    #[allow(dead_code)]
    detected_at: String,
}

impl DriftEventRow {
    fn into_event(self) -> Result<DriftEvent> {
        use crate::drift::DriftStatus;

        let severity = match self.severity.as_str() {
            "Critical" => DriftSeverity::Critical,
            "High" => DriftSeverity::High,
            "Medium" => DriftSeverity::Medium,
            "Low" => DriftSeverity::Low,
            _ => DriftSeverity::Medium,
        };

        let status = match self.status.as_str() {
            "Pending" => DriftStatus::Pending,
            "Accepted" => DriftStatus::Accepted,
            "Ignored" => DriftStatus::Ignored,
            "Fixed" => DriftStatus::Fixed,
            _ => DriftStatus::Pending,
        };

        let related_code_chunks: Vec<String> = serde_json::from_str(&self.related_code_chunks)?;
        let related_doc_chunks: Vec<String> = serde_json::from_str(&self.related_doc_chunks)?;

        Ok(DriftEvent {
            id: self.id,
            severity,
            description: self.description,
            evidence: self.evidence,
            confidence: self.confidence,
            related_code_chunks,
            related_doc_chunks,
            suggested_fix: self.suggested_fix,
            status,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_creation() {
        let db = Database::open_in_memory().unwrap();
        let stats = db.get_stats().unwrap();
        assert_eq!(stats.code_chunks, 0);
        assert_eq!(stats.doc_chunks, 0);
    }

    #[test]
    fn test_scan_state() {
        let db = Database::open_in_memory().unwrap();

        assert!(db.get_last_scan_commit().unwrap().is_none());

        db.set_last_scan_commit("abc123").unwrap();
        assert_eq!(
            db.get_last_scan_commit().unwrap(),
            Some("abc123".to_string())
        );
    }
}
