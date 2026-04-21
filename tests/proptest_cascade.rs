//! Property-based tests for cascade correctness.
//!
//! Generates random call graphs with annotations, applies random mutations,
//! and verifies cascade invariants hold across 1000+ random inputs.

use engram::graph::Store;
use engram::memory::cascade;
use proptest::prelude::*;
use std::collections::HashSet;
use std::path::Path;

/// Generate a random DAG as Rust source code with `num_fns` functions
/// and roughly `edge_pct`% chance of each function calling a prior one.
fn generate_dag_source(num_fns: usize, edge_pct: f64, seed: u64) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut lines = Vec::new();

    for i in 0..num_fns {
        let mut body_parts = Vec::new();
        // Each function can call earlier functions (ensuring DAG — no cycles)
        for j in 0..i {
            let mut hasher = DefaultHasher::new();
            (seed, i, j).hash(&mut hasher);
            let roll = (hasher.finish() % 100) as f64;
            if roll < edge_pct * 100.0 {
                body_parts.push(format!("    fn_{}();", j));
            }
        }
        if body_parts.is_empty() {
            body_parts.push(format!("    let _ = {};", i));
        }
        lines.push(format!("fn fn_{}() {{\n{}\n}}", i, body_parts.join("\n")));
    }

    lines.join("\n\n")
}

/// Setup a store with a generated DAG, annotate some symbols, then mutate one.
fn setup_and_cascade(
    num_fns: usize,
    edge_pct: f64,
    annotate_pct: f64,
    mutate_idx: usize,
    seed: u64,
) -> (Store, Vec<cascade::CascadeResult>) {
    let store = Store::open_in_memory().unwrap();
    store.initialize().unwrap();
    let parser = engram::parser::CodeParser::new();

    // Parse and index the DAG
    let source = generate_dag_source(num_fns, edge_pct, seed);
    let result = parser.parse_source(&source, "rust", "dag.rs").unwrap();
    store.sync_file(Path::new("dag.rs"), &result).unwrap();

    // Annotate some symbols
    let symbols = store.get_all_symbols().unwrap();
    for (i, sym) in symbols.iter().enumerate() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        (seed, i, "annotate").hash(&mut hasher);
        let roll = (hasher.finish() % 100) as f64;
        if roll < annotate_pct * 100.0 {
            let _ = store.create_annotation(
                &sym.id,
                "explanation",
                &format!("fn_{} does something important", i),
                &sym.full_hash,
            );
        }
    }

    // Mutate one function
    let mutate_idx = mutate_idx % num_fns;
    let mut mutated_source = generate_dag_source(num_fns, edge_pct, seed);
    // Change the target function's body
    let target = format!("fn fn_{}()", mutate_idx);
    if let Some(pos) = mutated_source.find(&target) {
        // Find the body and modify it
        if let Some(brace) = mutated_source[pos..].find('{') {
            let insert_pos = pos + brace + 1;
            mutated_source.insert_str(insert_pos, "\n    let _mutated = true;");
        }
    }

    let mutated_result = parser
        .parse_source(&mutated_source, "rust", "dag.rs")
        .unwrap();
    store
        .sync_file(Path::new("dag.rs"), &mutated_result)
        .unwrap();

    // Run cascade on the mutated symbol
    let new_symbols = store.get_all_symbols().unwrap();
    let mut results = Vec::new();
    let target_name = format!("fn_{}", mutate_idx);
    if let Some(sym) = new_symbols.iter().find(|s| s.name == target_name)
        && let Ok(r) = cascade::run_cascade(&store, &sym.id)
    {
        results.push(r);
    }

    (store, results)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn cascade_always_terminates(
        num_fns in 5usize..30,
        edge_pct in 0.1f64..0.5,
        annotate_pct in 0.2f64..0.8,
        mutate_idx in 0usize..100,
        seed in 0u64..10000,
    ) {
        let (_store, results) = setup_and_cascade(num_fns, edge_pct, annotate_pct, mutate_idx, seed);
        // If we got here, cascade terminated (didn't infinite loop)
        // results may be empty if no annotations were affected — that's fine
        let _ = results;
    }

    #[test]
    fn cascade_no_negative_confidence(
        num_fns in 5usize..25,
        edge_pct in 0.1f64..0.5,
        annotate_pct in 0.3f64..0.9,
        mutate_idx in 0usize..100,
        seed in 0u64..10000,
    ) {
        let (_store, results) = setup_and_cascade(num_fns, edge_pct, annotate_pct, mutate_idx, seed);
        for result in &results {
            for entry in &result.log {
                prop_assert!(
                    entry.new_confidence >= 0.0,
                    "Confidence must never be negative. Got {} for annotation #{} on symbol {}",
                    entry.new_confidence, entry.annotation_id, entry.affected_symbol
                );
                prop_assert!(
                    entry.new_confidence <= 1.0,
                    "Confidence must never exceed 1.0. Got {} for annotation #{}",
                    entry.new_confidence, entry.annotation_id
                );
            }
        }
    }

    #[test]
    fn cascade_no_duplicate_affected_symbols_per_annotation(
        num_fns in 5usize..20,
        edge_pct in 0.1f64..0.4,
        annotate_pct in 0.3f64..0.9,
        mutate_idx in 0usize..100,
        seed in 0u64..10000,
    ) {
        let (_store, results) = setup_and_cascade(num_fns, edge_pct, annotate_pct, mutate_idx, seed);
        for result in &results {
            // Each (affected_symbol, annotation_id) pair should appear at most once
            let mut seen: HashSet<(String, i64)> = HashSet::new();
            for entry in &result.log {
                let key = (entry.affected_symbol.clone(), entry.annotation_id);
                prop_assert!(
                    seen.insert(key.clone()),
                    "Duplicate cascade entry: symbol={}, annotation=#{}",
                    entry.affected_symbol, entry.annotation_id
                );
            }
        }
    }

    #[test]
    fn cascade_log_entries_have_valid_fields(
        num_fns in 5usize..20,
        edge_pct in 0.1f64..0.4,
        annotate_pct in 0.3f64..0.8,
        mutate_idx in 0usize..100,
        seed in 0u64..10000,
    ) {
        let (_store, results) = setup_and_cascade(num_fns, edge_pct, annotate_pct, mutate_idx, seed);
        for result in &results {
            prop_assert!(!result.trigger_symbol.is_empty(), "trigger_symbol must not be empty");
            for entry in &result.log {
                prop_assert!(!entry.trigger_symbol.is_empty());
                prop_assert!(!entry.affected_symbol.is_empty());
                prop_assert!(entry.annotation_id > 0, "annotation_id must be positive");
                prop_assert!(!entry.reason.is_empty(), "reason must not be empty");
                prop_assert!(
                    entry.old_confidence >= 0.0 && entry.old_confidence <= 1.0,
                    "old_confidence out of range: {}", entry.old_confidence
                );
            }
        }
    }

    #[test]
    fn cascade_confidence_only_decreases_for_transitive(
        num_fns in 5usize..20,
        edge_pct in 0.1f64..0.4,
        annotate_pct in 0.3f64..0.9,
        mutate_idx in 0usize..100,
        seed in 0u64..10000,
    ) {
        let (_store, results) = setup_and_cascade(num_fns, edge_pct, annotate_pct, mutate_idx, seed);
        for result in &results {
            for entry in &result.log {
                if entry.reason.contains("transitive_cascade") {
                    prop_assert!(
                        entry.new_confidence <= entry.old_confidence,
                        "Transitive cascade should only reduce confidence. Old={}, New={}",
                        entry.old_confidence, entry.new_confidence
                    );
                }
            }
        }
    }
}

/// Deterministic test with a known large DAG to verify scale.
#[test]
fn cascade_stress_test_100_node_dag() {
    let (store, results) = setup_and_cascade(100, 0.15, 0.5, 0, 42);

    let stats = store.stats().unwrap();
    assert!(
        stats.symbol_count >= 50,
        "should have many symbols, got {}",
        stats.symbol_count
    );

    // Cascade should have processed without panic
    for result in &results {
        for entry in &result.log {
            assert!(entry.new_confidence >= 0.0);
            assert!(entry.new_confidence <= 1.0);
        }
    }
}

/// Edge case: single-node graph (no edges, no cascade).
#[test]
fn cascade_single_node() {
    let (_, results) = setup_and_cascade(1, 0.0, 1.0, 0, 99);
    // Should complete without error; may or may not have log entries
    for result in &results {
        assert!(
            result.transitive_affected_count == 0,
            "single node has no callers"
        );
    }
}

/// Edge case: fully connected graph (every function calls every prior one).
#[test]
fn cascade_fully_connected() {
    let (_, results) = setup_and_cascade(10, 1.0, 1.0, 0, 77);
    for result in &results {
        for entry in &result.log {
            assert!(entry.new_confidence >= 0.0);
        }
    }
}
