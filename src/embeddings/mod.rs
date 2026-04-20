use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::collections::HashMap;

use crate::graph::{Store, SymbolRow};

// ── Types ─────────────────────────────────────────────────────────────────

/// A single search result with its score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub symbol_id: String,
    pub score: f64,
    /// Which signal contributed: "vector", "bm25", "graph", "rrf"
    #[allow(dead_code)]
    pub source: String,
}

// ── Embedding Trait (pluggable model) ─────────────────────────────────────

/// Trait for embedding model providers. Implement this to swap in a different
/// model (CodeBERT, StarCoder embeddings, OpenAI, etc.).
#[allow(dead_code)]
pub trait Embedder: Send + Sync {
    /// Embed a batch of texts. Returns Vec<Vec<f32>>.
    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;

    /// Embedding dimension count.
    fn dimensions(&self) -> usize;
}

// ── Default Embedding Engine (fastembed AllMiniLM-L6-V2) ──────────────────

pub struct EmbeddingEngine {
    model: TextEmbedding,
}

impl EmbeddingEngine {
    /// Initialize with AllMiniLM-L6-V2 (384 dims).
    pub fn new() -> Result<Self> {
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(true),
        )
        .context("initializing fastembed model")?;

        Ok(Self { model })
    }

    /// Build scope-enriched embedding text for a symbol.
    /// Format adapted from code-chunk's contextualizedText:
    ///   kind name in scope_chain
    ///   signature
    ///   docstring
    ///   called by: caller1, caller2
    pub fn build_embedding_text(sym: &SymbolRow, callers: &[String], deps: &[String]) -> String {
        let scope: Vec<String> = serde_json::from_str(&sym.scope_chain).unwrap_or_default();
        let scope_str = if scope.len() > 1 {
            format!(" in {}", scope[..scope.len() - 1].join(" > "))
        } else {
            String::new()
        };

        let mut parts = Vec::new();
        parts.push(format!("{} {}{}", sym.kind, sym.name, scope_str));

        if !sym.signature.is_empty() {
            parts.push(sym.signature.clone());
        }

        if let Some(ref doc) = sym.docstring {
            if !doc.is_empty() {
                parts.push(doc.clone());
            }
        }

        if !callers.is_empty() {
            parts.push(format!("called by: {}", callers.join(", ")));
        }

        if !deps.is_empty() {
            parts.push(format!("uses: {}", deps.join(", ")));
        }

        parts.join("\n")
    }

    /// Embed a batch of texts. Returns Vec<Vec<f32>> (384 dims each).
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        let embeddings = self
            .model
            .embed(texts.to_vec(), None)
            .context("embedding batch")?;
        Ok(embeddings)
    }

    /// Embed a single text.
    pub fn embed_one(&self, text: &str) -> Result<Vec<f32>> {
        let results = self
            .model
            .embed(vec![text], None)
            .context("embedding text")?;
        results
            .into_iter()
            .next()
            .context("empty embedding result")
    }

    /// Compute cosine similarity between two vectors.
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        dot / (norm_a * norm_b)
    }
}

impl Embedder for EmbeddingEngine {
    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        self.embed_batch(texts)
    }

    fn dimensions(&self) -> usize {
        384 // AllMiniLM-L6-V2
    }
}

// ── Vector Index ──────────────────────────────────────────────────────────

/// In-memory brute-force vector index.
/// At 100K symbols × 384 dims × 4 bytes = ~150MB — <10ms search.
pub struct VectorIndex {
    /// symbol_id → embedding vector
    vectors: HashMap<String, Vec<f32>>,
}

impl VectorIndex {
    pub fn new() -> Self {
        Self {
            vectors: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn insert(&mut self, symbol_id: String, embedding: Vec<f32>) {
        self.vectors.insert(symbol_id, embedding);
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Brute-force cosine search. Returns (symbol_id, similarity) sorted descending.
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Vec<(String, f32)> {
        let mut scores: Vec<(String, f32)> = self
            .vectors
            .iter()
            .map(|(id, emb)| (id.clone(), EmbeddingEngine::cosine_similarity(query_embedding, emb)))
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);
        scores
    }

    /// Load all embeddings from the store into memory.
    pub fn load_from_store(&mut self, store: &Store) -> Result<usize> {
        let rows = store.get_all_embeddings()?;
        for (symbol_id, embedding) in rows {
            self.vectors.insert(symbol_id, embedding);
        }
        Ok(self.vectors.len())
    }
}

// ── Reciprocal Rank Fusion ────────────────────────────────────────────────

/// RRF constant k. Standard is 60, but for code search with smaller result sets
/// k=30 may work better. Tunable via ablation.
const RRF_K: f64 = 60.0;

/// Fuse multiple ranked lists using Reciprocal Rank Fusion.
///
/// Each input is a ranked list of (symbol_id, score) — score is only used for
/// initial ranking, RRF is rank-based.
///
/// RRF score: final_score(d) = Σ 1/(k + rank_i(d)) for each signal i
pub fn rrf_fuse(
    signals: &[Vec<(String, f64)>],
    top_k: usize,
) -> Vec<SearchResult> {
    let mut rrf_scores: HashMap<String, f64> = HashMap::new();

    for signal in signals {
        for (rank, (symbol_id, _score)) in signal.iter().enumerate() {
            let rrf_contribution = 1.0 / (RRF_K + rank as f64 + 1.0);
            *rrf_scores.entry(symbol_id.clone()).or_default() += rrf_contribution;
        }
    }

    let mut results: Vec<SearchResult> = rrf_scores
        .into_iter()
        .map(|(symbol_id, score)| SearchResult {
            symbol_id,
            score,
            source: "rrf".to_string(),
        })
        .collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    results
}

// ── Hybrid Search ─────────────────────────────────────────────────────────

/// Perform hybrid search combining vector + BM25 + graph proximity via RRF.
pub fn search_hybrid(
    query: &str,
    store: &Store,
    engine: &EmbeddingEngine,
    index: &VectorIndex,
    top_k: usize,
) -> Result<Vec<SearchResult>> {
    let over_fetch = top_k * 3; // Over-fetch for better RRF fusion

    // Signal 1: Vector similarity
    let query_embedding = engine.embed_one(query)?;
    let vector_results: Vec<(String, f64)> = index
        .search(&query_embedding, over_fetch)
        .into_iter()
        .map(|(id, score)| (id, score as f64))
        .collect();

    // Signal 2: BM25 keyword match via FTS5
    let bm25_results = store.search_bm25(query, over_fetch)?;
    // BM25 returns negative rank scores (lower = better), invert for RRF
    let bm25_results: Vec<(String, f64)> = bm25_results
        .into_iter()
        .map(|(id, score)| (id, -score)) // FTS5 rank is negative, negate for ascending
        .collect();

    // Signal 3: Graph proximity — symbols close to high-attention symbols in the call graph
    let graph_results: Vec<(String, f64)> = match store.get_hot_symbols(over_fetch) {
        Ok(hot) if !hot.is_empty() => {
            // BFS outward from hot symbols, score by inverse distance
            let mut graph_scores: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
            for (hot_id, importance) in &hot {
                // Expand 2 hops from each hot symbol
                if let Ok(neighbors) = store.find_callers(hot_id, 2) {
                    for (neighbor_id, dist) in &neighbors {
                        let score = importance / (*dist as f64 + 1.0);
                        let entry = graph_scores.entry(neighbor_id.clone()).or_default();
                        *entry = entry.max(score);
                    }
                }
                if let Ok(deps) = store.find_dependencies(hot_id, 2) {
                    for (dep_id, dist) in &deps {
                        let score = importance / (*dist as f64 + 1.0);
                        let entry = graph_scores.entry(dep_id.clone()).or_default();
                        *entry = entry.max(score);
                    }
                }
            }
            let mut sorted: Vec<(String, f64)> = graph_scores.into_iter().collect();
            sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            sorted.truncate(over_fetch);
            sorted
        }
        _ => Vec::new(),
    };

    // Fuse with RRF
    let mut signals: Vec<Vec<(String, f64)>> = vec![vector_results, bm25_results];
    if !graph_results.is_empty() {
        signals.push(graph_results);
    }

    Ok(rrf_fuse(&signals, top_k))
}

// Embedding persistence methods are in graph::store (save_embedding, get_all_embeddings, get_stale_embeddings)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = EmbeddingEngine::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = EmbeddingEngine::cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = EmbeddingEngine::cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_rrf_fusion() {
        let signal1 = vec![
            ("a".to_string(), 0.9),
            ("b".to_string(), 0.8),
            ("c".to_string(), 0.7),
        ];
        let signal2 = vec![
            ("b".to_string(), 10.0),
            ("c".to_string(), 8.0),
            ("a".to_string(), 5.0),
        ];

        let results = rrf_fuse(&[signal1, signal2], 3);
        assert_eq!(results.len(), 3);

        // 'b' is rank 2 in signal1 and rank 1 in signal2 — should score well
        // 'a' is rank 1 in signal1 and rank 3 in signal2
        // All should have "rrf" source
        assert!(results.iter().all(|r| r.source == "rrf"));
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_rrf_single_signal() {
        let signal = vec![
            ("x".to_string(), 1.0),
            ("y".to_string(), 0.5),
        ];
        let results = rrf_fuse(&[signal], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].symbol_id, "x");
    }

    #[test]
    fn test_vector_index() {
        let mut index = VectorIndex::new();
        index.insert("a".to_string(), vec![1.0, 0.0, 0.0]);
        index.insert("b".to_string(), vec![0.0, 1.0, 0.0]);
        index.insert("c".to_string(), vec![0.7, 0.7, 0.0]);

        let query = vec![1.0, 0.0, 0.0]; // identical to "a"
        let results = index.search(&query, 3);

        assert_eq!(results[0].0, "a");
        assert!((results[0].1 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_build_embedding_text() {
        let sym = SymbolRow {
            id: "abc".to_string(),
            canonical_id: "def".to_string(),
            name: "validate_token".to_string(),
            kind: "function".to_string(),
            file: "auth.rs".to_string(),
            line_start: 10,
            line_end: 20,
            signature: "pub fn validate_token(token: &str) -> bool".to_string(),
            docstring: Some("Validates the JWT token".to_string()),
            body_hash: "xxx".to_string(),
            full_hash: "yyy".to_string(),
            language: "rust".to_string(),
            scope_chain: r#"["AuthService", "validate_token"]"#.to_string(),
            parent_id: None,
        };

        let text = EmbeddingEngine::build_embedding_text(
            &sym,
            &["handle_request".to_string()],
            &["jsonwebtoken".to_string()],
        );

        assert!(text.contains("function validate_token in AuthService"));
        assert!(text.contains("pub fn validate_token"));
        assert!(text.contains("Validates the JWT token"));
        assert!(text.contains("called by: handle_request"));
        assert!(text.contains("uses: jsonwebtoken"));
    }
}
