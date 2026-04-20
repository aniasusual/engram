//! Code clone detection.
//!
//! Finds exact clones (same body_hash) and near-clones (high embedding cosine similarity).

use anyhow::Result;
use std::collections::HashMap;

use crate::embeddings::EmbeddingEngine;
use crate::graph::Store;

/// A detected code clone pair.
#[derive(Debug, Clone)]
pub struct ClonePair {
    pub symbol_a_id: String,
    pub symbol_b_id: String,
    pub similarity: f64,
    pub clone_type: CloneType,
}

#[derive(Debug, Clone)]
pub enum CloneType {
    /// Identical body_hash — exact clone
    Exact,
    /// High embedding cosine — near-clone (similar structure/semantics)
    Near,
}

/// Find exact clones: symbols with identical body_hash in different files.
pub fn find_exact_clones(store: &Store) -> Result<Vec<ClonePair>> {
    let all_symbols = store.get_all_symbols()?;

    // Group by body_hash
    let mut by_hash: HashMap<String, Vec<(String, String)>> = HashMap::new(); // hash -> [(id, file)]
    for sym in &all_symbols {
        by_hash
            .entry(sym.body_hash.clone())
            .or_default()
            .push((sym.id.clone(), sym.file.clone()));
    }

    let mut clones = Vec::new();
    for (_hash, members) in &by_hash {
        if members.len() < 2 {
            continue;
        }
        // Only flag cross-file clones (same-file clones are likely overloads)
        for i in 0..members.len() {
            for j in (i + 1)..members.len() {
                if members[i].1 != members[j].1 {
                    clones.push(ClonePair {
                        symbol_a_id: members[i].0.clone(),
                        symbol_b_id: members[j].0.clone(),
                        similarity: 1.0,
                        clone_type: CloneType::Exact,
                    });
                }
            }
        }
    }

    Ok(clones)
}

/// Find near-clones using embedding cosine similarity.
/// Requires embeddings to be computed first.
pub fn find_near_clones(store: &Store, threshold: f64) -> Result<Vec<ClonePair>> {
    let embeddings = store.get_all_embeddings()?;
    if embeddings.len() < 2 {
        return Ok(vec![]);
    }

    let mut clones = Vec::new();

    // Brute-force pairwise comparison (fine for <10K symbols)
    for i in 0..embeddings.len() {
        for j in (i + 1)..embeddings.len() {
            let sim = EmbeddingEngine::cosine_similarity(&embeddings[i].1, &embeddings[j].1) as f64;
            if sim >= threshold {
                // Check they're in different files
                let sym_a = store.get_symbol(&embeddings[i].0)?;
                let sym_b = store.get_symbol(&embeddings[j].0)?;
                if let (Some(a), Some(b)) = (sym_a, sym_b) {
                    if a.file != b.file {
                        clones.push(ClonePair {
                            symbol_a_id: embeddings[i].0.clone(),
                            symbol_b_id: embeddings[j].0.clone(),
                            similarity: sim,
                            clone_type: CloneType::Near,
                        });
                    }
                }
            }
        }
    }

    // Sort by similarity descending
    clones.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
    Ok(clones)
}

/// Find symbols similar to a given symbol.
pub fn find_similar_to(store: &Store, symbol_id: &str, top_k: usize) -> Result<Vec<ClonePair>> {
    let target_sym = store.get_symbol(symbol_id)?
        .ok_or_else(|| anyhow::anyhow!("symbol not found"))?;

    let all_symbols = store.get_all_symbols()?;
    let mut results = Vec::new();

    // Exact body_hash matches
    for sym in &all_symbols {
        if sym.id == target_sym.id {
            continue;
        }
        if sym.body_hash == target_sym.body_hash && sym.file != target_sym.file {
            results.push(ClonePair {
                symbol_a_id: symbol_id.to_string(),
                symbol_b_id: sym.id.clone(),
                similarity: 1.0,
                clone_type: CloneType::Exact,
            });
        }
    }

    // If we have embeddings, also find near-clones
    let target_embedding = store.get_all_embeddings()?
        .into_iter()
        .find(|(id, _)| id == symbol_id);

    if let Some((_, target_emb)) = target_embedding {
        let all_embeddings = store.get_all_embeddings()?;
        for (id, emb) in &all_embeddings {
            if id == symbol_id {
                continue;
            }
            // Skip if already found as exact clone
            if results.iter().any(|r| r.symbol_b_id == *id) {
                continue;
            }
            let sim = EmbeddingEngine::cosine_similarity(&target_emb, emb) as f64;
            if sim >= 0.8 {
                results.push(ClonePair {
                    symbol_a_id: symbol_id.to_string(),
                    symbol_b_id: id.clone(),
                    similarity: sim,
                    clone_type: CloneType::Near,
                });
            }
        }
    }

    results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::CodeParser;
    use std::path::Path;

    #[test]
    fn test_find_exact_clones() {
        let store = Store::open_in_memory().unwrap();
        store.initialize().unwrap();
        let parser = CodeParser::new();

        // Same function in two files
        let source = "fn duplicate() { println!(\"hello\"); }";
        let r1 = parser.parse_source(source, "rust", "a.rs").unwrap();
        let r2 = parser.parse_source(source, "rust", "b.rs").unwrap();
        store.sync_file(Path::new("a.rs"), &r1).unwrap();
        store.sync_file(Path::new("b.rs"), &r2).unwrap();

        let clones = find_exact_clones(&store).unwrap();
        assert!(
            !clones.is_empty(),
            "should find exact clone across files"
        );
        assert!(matches!(clones[0].clone_type, CloneType::Exact));
        assert!((clones[0].similarity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_no_same_file_clones() {
        let store = Store::open_in_memory().unwrap();
        store.initialize().unwrap();
        let parser = CodeParser::new();

        // Two different functions in the same file
        let source = "fn a() {}\nfn b() {}";
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        let clones = find_exact_clones(&store).unwrap();
        assert!(clones.is_empty(), "should not flag same-file as clones");
    }
}
