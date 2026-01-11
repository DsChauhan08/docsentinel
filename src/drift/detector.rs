//! Main drift detection engine
//!
//! Coordinates all drift detection activities:
//! - Semantic similarity comparison
//! - Rule-based detection
//! - Evidence collection

use super::{
    cosine_similarity, DriftEvent, DriftSeverity, HardDriftRules, SimilarityResult, SoftDriftRules,
};
use crate::extract::{CodeChunk, DocChunk};
use crate::storage::Database;
use anyhow::Result;
use std::collections::HashMap;

/// Configuration for drift detection
#[derive(Debug, Clone)]
pub struct DriftConfig {
    /// Similarity threshold below which drift is suspected
    pub similarity_threshold: f64,
    /// Threshold for significant similarity drop
    pub drop_threshold: f64,
    /// Number of nearest doc chunks to consider
    pub top_k: usize,
    /// Whether to use hard rules
    pub use_hard_rules: bool,
    /// Whether to use soft rules
    pub use_soft_rules: bool,
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.7,
            drop_threshold: 0.2,
            top_k: 5,
            use_hard_rules: true,
            use_soft_rules: true,
        }
    }
}

/// Main drift detection engine
pub struct DriftDetector {
    config: DriftConfig,
    hard_rules: HardDriftRules,
    soft_rules: SoftDriftRules,
}

impl DriftDetector {
    /// Create a new drift detector with default configuration
    pub fn new() -> Self {
        Self {
            config: DriftConfig::default(),
            hard_rules: HardDriftRules::new(),
            soft_rules: SoftDriftRules::new(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: DriftConfig) -> Self {
        Self {
            config,
            hard_rules: HardDriftRules::new(),
            soft_rules: SoftDriftRules::new(),
        }
    }

    /// Detect drift for changed code chunks
    pub fn detect_code_drift(
        &self,
        old_chunks: &HashMap<String, CodeChunk>,
        new_chunks: &HashMap<String, CodeChunk>,
        doc_chunks: &[DocChunk],
        db: &Database,
    ) -> Result<Vec<DriftEvent>> {
        let mut events = Vec::new();

        // Find all changed, added, and removed code chunks
        let mut all_ids: std::collections::HashSet<&String> = old_chunks.keys().collect();
        all_ids.extend(new_chunks.keys());

        for id in all_ids {
            let old_chunk = old_chunks.get(id);
            let new_chunk = new_chunks.get(id);

            // Skip if unchanged
            if let (Some(old), Some(new)) = (old_chunk, new_chunk) {
                if old.hash == new.hash {
                    continue;
                }
            }

            // Find related doc chunks
            let related_docs = self.find_related_docs(new_chunk.or(old_chunk).unwrap(), doc_chunks);

            let related_doc_refs: Vec<&DocChunk> = related_docs.iter().collect();

            // Apply hard rules
            if self.config.use_hard_rules {
                let hard_events =
                    self.hard_rules
                        .check_code_change(old_chunk, new_chunk, &related_doc_refs);
                events.extend(hard_events);
            }

            // Apply soft rules
            if self.config.use_soft_rules {
                let soft_events =
                    self.soft_rules
                        .check_code_change(old_chunk, new_chunk, &related_doc_refs);
                events.extend(soft_events);
            }

            // Check semantic similarity drift
            if let Some(new) = new_chunk {
                if let Some(ref embedding) = new.embedding {
                    let similarity_events =
                        self.check_similarity_drift(new, embedding, &related_docs, db)?;
                    events.extend(similarity_events);
                }
            }
        }

        // Deduplicate events by related chunks
        events = self.deduplicate_events(events);

        Ok(events)
    }

    /// Detect drift for changed doc chunks
    pub fn detect_doc_drift(
        &self,
        old_chunks: &HashMap<String, DocChunk>,
        new_chunks: &HashMap<String, DocChunk>,
        code_chunks: &[CodeChunk],
    ) -> Result<Vec<DriftEvent>> {
        let mut events = Vec::new();

        // Find all changed, added, and removed doc chunks
        let mut all_ids: std::collections::HashSet<&String> = old_chunks.keys().collect();
        all_ids.extend(new_chunks.keys());

        for id in all_ids {
            let old_chunk = old_chunks.get(id);
            let new_chunk = new_chunks.get(id);

            // Skip if unchanged
            if let (Some(old), Some(new)) = (old_chunk, new_chunk) {
                if old.hash == new.hash {
                    continue;
                }
            }

            // Find related code chunks
            let related_code =
                self.find_related_code(new_chunk.or(old_chunk).unwrap(), code_chunks);

            let _related_code_refs: Vec<&CodeChunk> = related_code.iter().collect();

            // Check if doc was removed but code still exists
            if old_chunk.is_some() && new_chunk.is_none() && !related_code.is_empty() {
                let old = old_chunk.unwrap();
                let event = DriftEvent::new(
                    DriftSeverity::Medium,
                    &format!("Documentation section removed: {}", old.heading),
                    "Documentation was removed but related code still exists",
                    0.8,
                )
                .with_doc_chunk(&old.id);

                events.push(event);
            }
        }

        Ok(events)
    }

    /// Find doc chunks related to a code chunk using embeddings
    fn find_related_docs(&self, code_chunk: &CodeChunk, doc_chunks: &[DocChunk]) -> Vec<DocChunk> {
        let code_embedding = match &code_chunk.embedding {
            Some(e) => e,
            None => return Vec::new(),
        };

        let mut similarities: Vec<(usize, f64)> = doc_chunks
            .iter()
            .enumerate()
            .filter_map(|(i, doc)| {
                doc.embedding.as_ref().map(|doc_emb| {
                    let sim = cosine_similarity(code_embedding, doc_emb);
                    (i, sim)
                })
            })
            .collect();

        // Sort by similarity descending
        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top K that are above threshold
        similarities
            .into_iter()
            .take(self.config.top_k)
            .filter(|(_, sim)| *sim >= self.config.similarity_threshold)
            .map(|(i, _)| doc_chunks[i].clone())
            .collect()
    }

    /// Find code chunks related to a doc chunk using embeddings
    fn find_related_code(&self, doc_chunk: &DocChunk, code_chunks: &[CodeChunk]) -> Vec<CodeChunk> {
        let doc_embedding = match &doc_chunk.embedding {
            Some(e) => e,
            None => return Vec::new(),
        };

        let mut similarities: Vec<(usize, f64)> = code_chunks
            .iter()
            .enumerate()
            .filter_map(|(i, code)| {
                code.embedding.as_ref().map(|code_emb| {
                    let sim = cosine_similarity(doc_embedding, code_emb);
                    (i, sim)
                })
            })
            .collect();

        // Sort by similarity descending
        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top K that are above threshold
        similarities
            .into_iter()
            .take(self.config.top_k)
            .filter(|(_, sim)| *sim >= self.config.similarity_threshold)
            .map(|(i, _)| code_chunks[i].clone())
            .collect()
    }

    /// Check for similarity-based drift
    fn check_similarity_drift(
        &self,
        code_chunk: &CodeChunk,
        embedding: &[f32],
        related_docs: &[DocChunk],
        _db: &Database,
    ) -> Result<Vec<DriftEvent>> {
        let mut events = Vec::new();

        for doc in related_docs {
            if let Some(ref doc_embedding) = doc.embedding {
                let similarity = cosine_similarity(embedding, doc_embedding);

                // If similarity is low, there might be drift
                if similarity < self.config.similarity_threshold {
                    let event = DriftEvent::new(
                        DriftSeverity::Medium,
                        &format!(
                            "Low semantic similarity between '{}' and '{}'",
                            code_chunk.symbol_name, doc.heading
                        ),
                        &format!(
                            "Similarity score: {:.2}. Documentation may not accurately describe the code.",
                            similarity
                        ),
                        similarity,
                    )
                    .with_code_chunk(&code_chunk.id)
                    .with_doc_chunk(&doc.id);

                    events.push(event);
                }
            }
        }

        Ok(events)
    }

    /// Deduplicate events that refer to the same chunks
    fn deduplicate_events(&self, events: Vec<DriftEvent>) -> Vec<DriftEvent> {
        let mut seen: HashMap<String, DriftEvent> = HashMap::new();

        for event in events {
            // Create a key from related chunks
            let mut key_parts: Vec<String> = event.related_code_chunks.clone();
            key_parts.extend(event.related_doc_chunks.clone());
            key_parts.sort();
            let key = key_parts.join("|");

            // Keep the event with higher severity/confidence
            if let Some(existing) = seen.get(&key) {
                if event.severity > existing.severity
                    || (event.severity == existing.severity
                        && event.confidence > existing.confidence)
                {
                    seen.insert(key, event);
                }
            } else {
                seen.insert(key, event);
            }
        }

        seen.into_values().collect()
    }

    /// Get similarity results for all code-doc pairs
    pub fn compute_all_similarities(
        &self,
        code_chunks: &[CodeChunk],
        doc_chunks: &[DocChunk],
    ) -> Vec<SimilarityResult> {
        let mut results = Vec::new();

        for code in code_chunks {
            if let Some(ref code_emb) = code.embedding {
                for doc in doc_chunks {
                    if let Some(ref doc_emb) = doc.embedding {
                        let similarity = cosine_similarity(code_emb, doc_emb);

                        results.push(SimilarityResult {
                            code_chunk_id: code.id.clone(),
                            doc_chunk_id: doc.id.clone(),
                            similarity,
                            previous_similarity: None,
                        });
                    }
                }
            }
        }

        results
    }

    /// Find the best matching doc chunks for a code chunk
    pub fn find_best_matches(
        &self,
        code_chunk: &CodeChunk,
        doc_chunks: &[DocChunk],
        limit: usize,
    ) -> Vec<(DocChunk, f64)> {
        let code_embedding = match &code_chunk.embedding {
            Some(e) => e,
            None => return Vec::new(),
        };

        let mut matches: Vec<(DocChunk, f64)> = doc_chunks
            .iter()
            .filter_map(|doc| {
                doc.embedding.as_ref().map(|doc_emb| {
                    let sim = cosine_similarity(code_embedding, doc_emb);
                    (doc.clone(), sim)
                })
            })
            .collect();

        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        matches.truncate(limit);

        matches
    }
}

impl Default for DriftDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::code::{Language, SymbolType};
    use crate::extract::doc::HeadingLevel;

    fn create_test_code_chunk(name: &str, embedding: Vec<f32>) -> CodeChunk {
        let mut chunk = CodeChunk::new(
            "test.rs",
            name,
            SymbolType::Function,
            "fn test() {}",
            Language::Rust,
            1,
            1,
        );
        chunk.embedding = Some(embedding);
        chunk.is_public = true;
        chunk
    }

    fn create_test_doc_chunk(heading: &str, embedding: Vec<f32>) -> DocChunk {
        let mut chunk = DocChunk::new(
            "README.md",
            vec![heading.to_string()],
            heading,
            HeadingLevel::H2,
            "Test content",
            1,
            5,
        );
        chunk.embedding = Some(embedding);
        chunk
    }

    #[test]
    fn test_find_related_docs() {
        let detector = DriftDetector::new();

        // Create code chunk with embedding [1, 0, 0]
        let code = create_test_code_chunk("test_func", vec![1.0, 0.0, 0.0]);

        // Create doc chunks with various embeddings
        let doc1 = create_test_doc_chunk("Related", vec![0.9, 0.1, 0.0]); // Similar
        let doc2 = create_test_doc_chunk("Unrelated", vec![0.0, 1.0, 0.0]); // Different

        let docs = vec![doc1, doc2];
        let related = detector.find_related_docs(&code, &docs);

        // Should find the similar doc
        assert!(!related.is_empty());
        assert_eq!(related[0].heading, "Related");
    }

    #[test]
    fn test_compute_similarities() {
        let detector = DriftDetector::new();

        let code = create_test_code_chunk("func", vec![1.0, 0.0, 0.0]);
        let doc = create_test_doc_chunk("Doc", vec![1.0, 0.0, 0.0]);

        let results = detector.compute_all_similarities(&[code], &[doc]);

        assert_eq!(results.len(), 1);
        assert!((results[0].similarity - 1.0).abs() < 0.001);
    }
}
