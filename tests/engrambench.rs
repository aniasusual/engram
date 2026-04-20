//! EngramBench — custom benchmark for code intelligence search quality.
//!
//! Tests MRR, Precision@5, Recall@10 on the simple fixture repo
//! with hand-labeled golden queries. Also runs RRF ablation.

use engram::embeddings::{EmbeddingEngine, VectorIndex, search_hybrid};
use engram::graph::Store;
use engram::parser::CodeParser;
use std::path::Path;

/// A golden query with expected relevant symbols.
struct GoldenQuery {
    query: &'static str,
    /// Symbol names that SHOULD appear in results.
    relevant: &'static [&'static str],
    query_type: &'static str,
}

const GOLDEN_QUERIES: &[GoldenQuery] = &[
    // Name queries — exact/fuzzy symbol lookup
    GoldenQuery {
        query: "validate_token",
        relevant: &["validate_token"],
        query_type: "name",
    },
    GoldenQuery {
        query: "AuthService",
        relevant: &["AuthService"],
        query_type: "name",
    },
    GoldenQuery {
        query: "handle_request",
        relevant: &["handle_request"],
        query_type: "name",
    },
    GoldenQuery {
        query: "Database",
        relevant: &["Database"],
        query_type: "name",
    },
    // Semantic queries — should find related symbols
    GoldenQuery {
        query: "authentication validation",
        relevant: &["validate_token", "AuthService", "extract_user_id"],
        query_type: "semantic",
    },
    GoldenQuery {
        query: "database query user",
        relevant: &["get_user", "Database", "save_user"],
        query_type: "semantic",
    },
    GoldenQuery {
        query: "request handler",
        relevant: &["handle_request", "handle_batch"],
        query_type: "semantic",
    },
    GoldenQuery {
        query: "format response output",
        relevant: &["format_response"],
        query_type: "semantic",
    },
    GoldenQuery {
        query: "sanitize input",
        relevant: &["sanitize_input"],
        query_type: "semantic",
    },
    GoldenQuery {
        query: "token secret",
        relevant: &["validate_token", "AuthService", "new"],
        query_type: "semantic",
    },
    // Structural queries
    GoldenQuery {
        query: "connect database",
        relevant: &["connect", "Database"],
        query_type: "structural",
    },
    GoldenQuery {
        query: "save data",
        relevant: &["save_user"],
        query_type: "structural",
    },
];

fn setup_store() -> Store {
    let store = Store::open_in_memory().unwrap();
    store.initialize().unwrap();
    let parser = CodeParser::new();

    let fixtures = [
        ("auth.rs", "tests/fixtures/simple/auth.rs"),
        ("db.rs", "tests/fixtures/simple/db.rs"),
        ("handler.rs", "tests/fixtures/simple/handler.rs"),
        ("utils.rs", "tests/fixtures/simple/utils.rs"),
        ("main.rs", "tests/fixtures/simple/main.rs"),
    ];

    for (name, path) in &fixtures {
        let source = std::fs::read_to_string(path).unwrap();
        let result = parser.parse_source(&source, "rust", name).unwrap();
        store.sync_file(Path::new(name), &result).unwrap();
    }

    store
}

/// Mean Reciprocal Rank: 1/rank of first relevant result, averaged across queries.
fn compute_mrr(results: &[(String, Vec<String>)], queries: &[GoldenQuery]) -> f64 {
    let mut sum = 0.0;
    let mut count = 0;

    for (i, (_, result_names)) in results.iter().enumerate() {
        let relevant = &queries[i].relevant;
        for (rank, name) in result_names.iter().enumerate() {
            if relevant.contains(&name.as_str()) {
                sum += 1.0 / (rank as f64 + 1.0);
                break;
            }
        }
        count += 1;
    }

    if count == 0 { 0.0 } else { sum / count as f64 }
}

/// Precision@k: of the top k results, how many are relevant?
fn compute_precision_at_k(results: &[(String, Vec<String>)], queries: &[GoldenQuery], k: usize) -> f64 {
    let mut sum = 0.0;
    let mut count = 0;

    for (i, (_, result_names)) in results.iter().enumerate() {
        let relevant = &queries[i].relevant;
        let top_k = &result_names[..result_names.len().min(k)];
        let hits = top_k.iter().filter(|n| relevant.contains(&n.as_str())).count();
        sum += hits as f64 / k as f64;
        count += 1;
    }

    if count == 0 { 0.0 } else { sum / count as f64 }
}

/// Recall@k: of all relevant results, how many are in the top k?
fn compute_recall_at_k(results: &[(String, Vec<String>)], queries: &[GoldenQuery], k: usize) -> f64 {
    let mut sum = 0.0;
    let mut count = 0;

    for (i, (_, result_names)) in results.iter().enumerate() {
        let relevant = &queries[i].relevant;
        if relevant.is_empty() { continue; }
        let top_k = &result_names[..result_names.len().min(k)];
        let hits = top_k.iter().filter(|n| relevant.contains(&n.as_str())).count();
        sum += hits as f64 / relevant.len() as f64;
        count += 1;
    }

    if count == 0 { 0.0 } else { sum / count as f64 }
}

fn run_bm25_queries(store: &Store) -> Vec<(String, Vec<String>)> {
    GOLDEN_QUERIES
        .iter()
        .map(|gq| {
            let results = store.search_bm25(gq.query, 10).unwrap_or_default();
            let names: Vec<String> = results
                .iter()
                .filter_map(|(id, _)| store.get_symbol(id).ok().flatten().map(|s| s.name))
                .collect();
            (gq.query.to_string(), names)
        })
        .collect()
}

#[test]
fn engrambench_bm25_metrics() {
    let store = setup_store();
    let results = run_bm25_queries(&store);

    let mrr = compute_mrr(&results, GOLDEN_QUERIES);
    let p5 = compute_precision_at_k(&results, GOLDEN_QUERIES, 5);
    let r10 = compute_recall_at_k(&results, GOLDEN_QUERIES, 10);

    println!("\n=== EngramBench Results (BM25) ===");
    println!("  MRR:          {:.3}", mrr);
    println!("  Precision@5:  {:.3}", p5);
    println!("  Recall@10:    {:.3}", r10);
    println!();

    // Per-query breakdown
    for (i, (query, names)) in results.iter().enumerate() {
        let relevant = GOLDEN_QUERIES[i].relevant;
        let hit = names.iter().any(|n| relevant.contains(&n.as_str()));
        println!(
            "  [{}] {}: {} → [{}] {}",
            GOLDEN_QUERIES[i].query_type,
            if hit { "HIT" } else { "MISS" },
            query,
            names.len(),
            names.join(", ")
        );
    }

    // Targets from plan: MRR >0.85, Precision@5 >0.80
    // These are targets for full RRF (vector+BM25+graph), not BM25 alone.
    // BM25-only is the baseline; fusion should beat it.
    assert!(
        mrr > 0.0,
        "MRR should be > 0 (got {:.3}). BM25 should find at least some relevant results.",
        mrr
    );
}

#[test]
fn engrambench_per_query_type_breakdown() {
    let store = setup_store();
    let results = run_bm25_queries(&store);

    let mut type_hits: std::collections::HashMap<&str, (usize, usize)> = std::collections::HashMap::new();

    for (i, (_, names)) in results.iter().enumerate() {
        let relevant = GOLDEN_QUERIES[i].relevant;
        let hit = names.iter().any(|n| relevant.contains(&n.as_str()));
        let entry = type_hits.entry(GOLDEN_QUERIES[i].query_type).or_insert((0, 0));
        entry.1 += 1;
        if hit {
            entry.0 += 1;
        }
    }

    println!("\n=== Hit Rate by Query Type ===");
    for (qtype, (hits, total)) in &type_hits {
        println!("  {}: {}/{} ({:.0}%)", qtype, hits, total, *hits as f64 / *total as f64 * 100.0);
    }
}

#[test]
fn engrambench_ablation_bm25_vs_name_lookup() {
    let store = setup_store();

    // Signal 1: BM25
    let bm25_results = run_bm25_queries(&store);
    let bm25_mrr = compute_mrr(&bm25_results, GOLDEN_QUERIES);

    // Signal 2: Exact name lookup (baseline)
    let name_results: Vec<(String, Vec<String>)> = GOLDEN_QUERIES
        .iter()
        .map(|gq| {
            // Try exact name match on each word in the query
            let mut names = Vec::new();
            for word in gq.query.split_whitespace() {
                if let Ok(syms) = store.find_symbol_by_name(word) {
                    for s in &syms {
                        if !names.contains(&s.name) {
                            names.push(s.name.clone());
                        }
                    }
                }
            }
            (gq.query.to_string(), names)
        })
        .collect();
    let name_mrr = compute_mrr(&name_results, GOLDEN_QUERIES);

    println!("\n=== Ablation: BM25 vs Name Lookup ===");
    println!("  BM25 MRR:        {:.3}", bm25_mrr);
    println!("  Name Lookup MRR: {:.3}", name_mrr);
    println!(
        "  Winner: {}",
        if bm25_mrr > name_mrr { "BM25" } else { "Name Lookup" }
    );

    // BM25 should beat or match exact name lookup on semantic queries
    // (name lookup only finds exact matches, BM25 does fuzzy keyword matching)
}

// ── Hybrid RRF Tests (Vector + BM25 + Graph) ─────────────────────────────

/// Setup store with embeddings computed for all symbols.
fn setup_store_with_embeddings() -> Option<(Store, EmbeddingEngine, VectorIndex)> {
    let store = setup_store();

    // Try to initialize embedding engine (may fail if model not downloaded)
    let engine = match EmbeddingEngine::new() {
        Ok(e) => e,
        Err(_) => return None, // Skip if model not available
    };

    // Compute embeddings for all symbols
    let symbols = store.get_all_symbols().unwrap();
    for sym in &symbols {
        let callers: Vec<String> = store
            .get_direct_callers(&sym.id)
            .unwrap_or_default()
            .iter()
            .filter_map(|id| store.get_symbol(id).ok().flatten().map(|s| s.name))
            .collect();
        let deps: Vec<String> = store
            .find_dependencies(&sym.id, 1)
            .unwrap_or_default()
            .iter()
            .filter_map(|(id, _)| store.get_symbol(id).ok().flatten().map(|s| s.name))
            .collect();

        let text = EmbeddingEngine::build_embedding_text(&sym, &callers, &deps);
        if let Ok(embedding) = engine.embed_one(&text) {
            let _ = store.save_embedding(&sym.id, &embedding, &sym.body_hash);
        }
    }

    // Load into vector index
    let mut index = VectorIndex::new();
    index.load_from_store(&store).unwrap();

    Some((store, engine, index))
}

fn run_hybrid_queries(store: &Store, engine: &EmbeddingEngine, index: &VectorIndex) -> Vec<(String, Vec<String>)> {
    GOLDEN_QUERIES
        .iter()
        .map(|gq| {
            let results = search_hybrid(gq.query, store, engine, index, 10)
                .unwrap_or_default();
            let names: Vec<String> = results
                .iter()
                .filter_map(|r| store.get_symbol(&r.symbol_id).ok().flatten().map(|s| s.name))
                .collect();
            (gq.query.to_string(), names)
        })
        .collect()
}

#[test]
fn engrambench_hybrid_rrf_metrics() {
    let Some((store, engine, index)) = setup_store_with_embeddings() else {
        println!("Skipping hybrid test — embedding model not available");
        return;
    };

    let results = run_hybrid_queries(&store, &engine, &index);

    let mrr = compute_mrr(&results, GOLDEN_QUERIES);
    let p5 = compute_precision_at_k(&results, GOLDEN_QUERIES, 5);
    let r10 = compute_recall_at_k(&results, GOLDEN_QUERIES, 10);

    println!("\n=== EngramBench Results (Hybrid RRF: Vector + BM25 + Graph) ===");
    println!("  MRR:          {:.3}", mrr);
    println!("  Precision@5:  {:.3}", p5);
    println!("  Recall@10:    {:.3}", r10);
    println!();

    // Per-query breakdown
    for (i, (query, names)) in results.iter().enumerate() {
        let relevant = GOLDEN_QUERIES[i].relevant;
        let hit = names.iter().any(|n| relevant.contains(&n.as_str()));
        println!(
            "  [{}] {}: {} → [{}] {}",
            GOLDEN_QUERIES[i].query_type,
            if hit { "HIT" } else { "MISS" },
            query,
            names.len(),
            names.join(", ")
        );
    }

    // These are the plan targets for hybrid RRF
    assert!(
        mrr >= 0.80,
        "Hybrid MRR should be >= 0.80 (got {:.3}). Vector signal should lift semantic queries.",
        mrr
    );
}

#[test]
fn engrambench_full_ablation() {
    let Some((store, engine, index)) = setup_store_with_embeddings() else {
        println!("Skipping ablation test — embedding model not available");
        return;
    };

    // Signal 1: BM25 only
    let bm25_results = run_bm25_queries(&store);
    let bm25_mrr = compute_mrr(&bm25_results, GOLDEN_QUERIES);

    // Signal 2: Vector only
    let vector_results: Vec<(String, Vec<String>)> = GOLDEN_QUERIES
        .iter()
        .map(|gq| {
            let query_emb = engine.embed_one(gq.query).unwrap();
            let results = index.search(&query_emb, 10);
            let names: Vec<String> = results
                .iter()
                .filter_map(|(id, _)| store.get_symbol(id).ok().flatten().map(|s| s.name))
                .collect();
            (gq.query.to_string(), names)
        })
        .collect();
    let vector_mrr = compute_mrr(&vector_results, GOLDEN_QUERIES);

    // Signal 3: Hybrid RRF
    let hybrid_results = run_hybrid_queries(&store, &engine, &index);
    let hybrid_mrr = compute_mrr(&hybrid_results, GOLDEN_QUERIES);

    println!("\n=== Full Ablation Study ===");
    println!("  BM25-only MRR:    {:.3}", bm25_mrr);
    println!("  Vector-only MRR:  {:.3}", vector_mrr);
    println!("  Hybrid RRF MRR:   {:.3}", hybrid_mrr);
    println!();

    // Per-signal hit rates on semantic queries
    let semantic_idxs: Vec<usize> = GOLDEN_QUERIES
        .iter()
        .enumerate()
        .filter(|(_, gq)| gq.query_type == "semantic")
        .map(|(i, _)| i)
        .collect();

    let bm25_semantic_hits = semantic_idxs.iter()
        .filter(|&&i| bm25_results[i].1.iter().any(|n| GOLDEN_QUERIES[i].relevant.contains(&n.as_str())))
        .count();
    let vector_semantic_hits = semantic_idxs.iter()
        .filter(|&&i| vector_results[i].1.iter().any(|n| GOLDEN_QUERIES[i].relevant.contains(&n.as_str())))
        .count();
    let hybrid_semantic_hits = semantic_idxs.iter()
        .filter(|&&i| hybrid_results[i].1.iter().any(|n| GOLDEN_QUERIES[i].relevant.contains(&n.as_str())))
        .count();

    println!("  Semantic query hit rate:");
    println!("    BM25:   {}/{}", bm25_semantic_hits, semantic_idxs.len());
    println!("    Vector: {}/{}", vector_semantic_hits, semantic_idxs.len());
    println!("    Hybrid: {}/{}", hybrid_semantic_hits, semantic_idxs.len());

    // Hybrid should beat or match the best single signal
    let best_single = bm25_mrr.max(vector_mrr);
    println!("\n  Best single signal: {:.3}", best_single);
    println!("  Hybrid improvement: {:.3} ({:+.1}%)",
        hybrid_mrr,
        (hybrid_mrr - best_single) / best_single * 100.0
    );
}
