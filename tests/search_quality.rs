//! Search quality tests and cascade edge cases.
//!
//! Tests disambiguation, RRF fusion superiority, NDCG computation,
//! and cascade boundary correctness.

use engram::embeddings::{EmbeddingEngine, VectorIndex, search_hybrid, rrf_fuse};
use engram::graph::Store;
use engram::memory::cascade;
use engram::parser::CodeParser;
use std::path::Path;

fn setup() -> (Store, CodeParser) {
    let store = Store::open_in_memory().unwrap();
    store.initialize().unwrap();
    (store, CodeParser::new())
}

// ── Cascade: does_not_cross_unchanged ─────────────────────────────────

#[test]
fn cascade_does_not_cross_unchanged() {
    // A calls B calls C. Annotate A about A. Change C.
    // C's annotations should go stale. B reduced. A's annotation about A should NOT be affected
    // because the cascade reduces confidence on callers, but A's annotation
    // is about A itself — it should still be affected since A is a transitive caller of C.
    //
    // The real invariant: cascade should NOT visit symbols that are NOT in the
    // reverse call graph of the changed symbol.
    let (store, parser) = setup();

    let source = r#"
fn leaf() -> bool { true }
fn middle() -> bool { leaf() }
fn caller_of_middle() -> bool { middle() }
fn unrelated() -> i32 { 42 }
"#;
    let result = parser.parse_source(source, "rust", "test.rs").unwrap();
    store.sync_file(Path::new("test.rs"), &result).unwrap();

    // Annotate unrelated function
    let unrelated = store.find_symbol_by_name("unrelated").unwrap();
    store.create_annotation(
        &unrelated[0].id, "explanation", "this is totally independent",
        &unrelated[0].full_hash
    ).unwrap();

    // Change leaf
    let new_source = r#"
fn leaf() -> bool { false }
fn middle() -> bool { leaf() }
fn caller_of_middle() -> bool { middle() }
fn unrelated() -> i32 { 42 }
"#;
    let new_result = parser.parse_source(new_source, "rust", "test.rs").unwrap();
    store.sync_file(Path::new("test.rs"), &new_result).unwrap();

    let new_leaf = store.find_symbol_by_name("leaf").unwrap();
    let cascade_result = cascade::run_cascade(&store, &new_leaf[0].id).unwrap();

    // Check that unrelated was NOT in the cascade log
    let affected_symbols: Vec<String> = cascade_result.log.iter()
        .map(|e| e.affected_symbol.clone())
        .collect();

    let unrelated_affected = affected_symbols.iter().any(|id| {
        store.get_symbol(id).ok().flatten()
            .map(|s| s.name == "unrelated")
            .unwrap_or(false)
    });

    assert!(
        !unrelated_affected,
        "cascade should NOT affect 'unrelated' which is not in leaf's call graph"
    );
}

// ── Search: ambiguous names disambiguated by scope ────────────────────

#[test]
fn ambiguous_names_disambiguated() {
    let (store, parser) = setup();

    // Two functions named "process" in different contexts
    let source = r#"
fn process_auth() { }
fn process_data() { }
"#;
    // We use different file contexts to simulate scope
    let r1 = parser.parse_source("fn process() { /* auth logic */ }", "rust", "auth/processor.rs").unwrap();
    let r2 = parser.parse_source("fn process() { /* data logic */ }", "rust", "data/processor.rs").unwrap();
    store.sync_file(Path::new("auth/processor.rs"), &r1).unwrap();
    store.sync_file(Path::new("data/processor.rs"), &r2).unwrap();

    // BM25 search for "process" should find both
    let results = store.search_bm25("process", 10).unwrap();
    assert!(results.len() >= 2, "should find both process functions");

    // BM25 search for "auth process" should prefer the auth one
    let auth_results = store.search_bm25("auth process", 10).unwrap();
    if !auth_results.is_empty() {
        let first = store.get_symbol(&auth_results[0].0).unwrap().unwrap();
        assert!(
            first.file.contains("auth"),
            "searching 'auth process' should rank auth/processor.rs higher, got {}",
            first.file
        );
    }
}

// ── Search: RRF beats single signal ───────────────────────────────────

#[test]
fn rrf_fusion_outperforms_or_matches_single_signals() {
    // RRF should never be significantly worse than the best single signal.
    // With two signals that have complementary strengths, fusion should match or beat.

    let signal1 = vec![
        ("a".to_string(), 0.95),  // signal1 is confident about A
        ("b".to_string(), 0.80),
        ("c".to_string(), 0.60),
        ("d".to_string(), 0.40),
    ];

    let signal2 = vec![
        ("c".to_string(), 0.90),  // signal2 is confident about C
        ("b".to_string(), 0.85),
        ("a".to_string(), 0.50),
        ("e".to_string(), 0.30),
    ];

    let fused = rrf_fuse(&[signal1.clone(), signal2.clone()], 10);

    // B appears in top-2 of both signals — should be boosted by RRF
    let b_rank = fused.iter().position(|r| r.symbol_id == "b");
    assert!(
        b_rank.is_some_and(|r| r <= 2),
        "symbol 'b' (top-2 in both signals) should be in top-3 of RRF, got rank {:?}",
        b_rank
    );

    // A and C should both appear (each is #1 in one signal)
    assert!(fused.iter().any(|r| r.symbol_id == "a"), "a should be in fused results");
    assert!(fused.iter().any(|r| r.symbol_id == "c"), "c should be in fused results");

    // E (only in signal2 at rank 4) should have lower score than B
    let b_score = fused.iter().find(|r| r.symbol_id == "b").map(|r| r.score).unwrap_or(0.0);
    let e_score = fused.iter().find(|r| r.symbol_id == "e").map(|r| r.score).unwrap_or(0.0);
    assert!(b_score > e_score, "b (multi-signal) should score higher than e (single-signal)");
}

// ── NDCG@10 computation ──────────────────────────────────────────────

fn ndcg_at_k(ranked_relevances: &[f64], k: usize) -> f64 {
    let k = k.min(ranked_relevances.len());
    if k == 0 { return 0.0; }

    let dcg: f64 = ranked_relevances[..k].iter().enumerate()
        .map(|(i, &rel)| (2f64.powf(rel) - 1.0) / (i as f64 + 2.0).log2())
        .sum();

    let mut ideal = ranked_relevances.to_vec();
    ideal.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let idcg: f64 = ideal[..k].iter().enumerate()
        .map(|(i, &rel)| (2f64.powf(rel) - 1.0) / (i as f64 + 2.0).log2())
        .sum();

    if idcg == 0.0 { 0.0 } else { dcg / idcg }
}

#[test]
fn ndcg_perfect_ranking() {
    // Perfect ranking: most relevant first
    let ranked = vec![3.0, 2.0, 1.0, 0.0];
    let ndcg = ndcg_at_k(&ranked, 4);
    assert!((ndcg - 1.0).abs() < 1e-6, "perfect ranking should give NDCG=1.0, got {}", ndcg);
}

#[test]
fn ndcg_worst_ranking() {
    // Worst ranking: irrelevant first, relevant last
    let ranked = vec![0.0, 0.0, 0.0, 3.0];
    let ndcg = ndcg_at_k(&ranked, 4);
    assert!(ndcg < 0.5, "worst ranking should give low NDCG, got {}", ndcg);
}

#[test]
fn ndcg_empty() {
    let ranked: Vec<f64> = vec![];
    let ndcg = ndcg_at_k(&ranked, 10);
    assert!((ndcg - 0.0).abs() < 1e-6, "empty should give NDCG=0");
}

#[test]
fn ndcg_all_irrelevant() {
    let ranked = vec![0.0, 0.0, 0.0];
    let ndcg = ndcg_at_k(&ranked, 3);
    assert!((ndcg - 0.0).abs() < 1e-6, "all-irrelevant should give NDCG=0");
}

// ── Scope-enriched vs plain embedding text ────────────────────────────

#[test]
fn scope_enriched_text_contains_structural_context() {
    let sym = engram::graph::SymbolRow {
        id: "abc".into(),
        canonical_id: "def".into(),
        name: "validate_token".into(),
        kind: "function".into(),
        file: "auth.rs".into(),
        line_start: 10,
        line_end: 20,
        signature: "pub fn validate_token(token: &str) -> bool".into(),
        docstring: Some("Validates JWT tokens".into()),
        body_hash: "xxx".into(),
        full_hash: "yyy".into(),
        language: "rust".into(),
        scope_chain: r#"["AuthService", "validate_token"]"#.into(),
        parent_id: None,
    };

    let enriched = EmbeddingEngine::build_embedding_text(
        &sym,
        &["handle_request".into(), "middleware".into()],
        &["jsonwebtoken".into()],
    );

    // Plain text would just be "validate_token"
    // Enriched should contain all structural context
    assert!(enriched.contains("function validate_token"), "should contain kind + name");
    assert!(enriched.contains("AuthService"), "should contain scope chain");
    assert!(enriched.contains("pub fn validate_token"), "should contain signature");
    assert!(enriched.contains("Validates JWT tokens"), "should contain docstring");
    assert!(enriched.contains("handle_request"), "should contain callers");
    assert!(enriched.contains("jsonwebtoken"), "should contain dependencies");

    // A plain "validate_token" string is much shorter
    let plain = "validate_token";
    assert!(enriched.len() > plain.len() * 5,
        "enriched text ({} chars) should be much longer than plain ({} chars)",
        enriched.len(), plain.len()
    );
}
