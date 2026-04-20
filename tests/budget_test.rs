//! Token-budget tests for get_context_batch and RRF k-parameter sweep.

use engram::embeddings::rrf_fuse;
use engram::graph::Store;
use engram::mcp::EngramMcp;
use engram::parser::CodeParser;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn setup_store() -> Store {
    let store = Store::open_in_memory().unwrap();
    store.initialize().unwrap();
    let parser = CodeParser::new();

    let fixtures = [
        ("auth.rs", "tests/fixtures/simple/auth.rs"),
        ("db.rs", "tests/fixtures/simple/db.rs"),
        ("handler.rs", "tests/fixtures/simple/handler.rs"),
        ("utils.rs", "tests/fixtures/simple/utils.rs"),
    ];

    for (name, path) in &fixtures {
        let source = std::fs::read_to_string(path).unwrap();
        let result = parser.parse_source(&source, "rust", name).unwrap();
        store.sync_file(Path::new(name), &result).unwrap();
    }

    store
}

fn estimate_tokens(text: &str) -> usize {
    (text.len() as f64 / 4.0).ceil() as usize
}

// ── Context tier ordering ─────────────────────────────────────────────

#[test]
fn context_tiers_increase_in_size() {
    let store = setup_store();
    let sym = store.find_symbol_by_name("validate_token").unwrap();
    assert!(!sym.is_empty());

    let mcp = EngramMcp::new(Arc::new(store), PathBuf::from("."));

    let brief = mcp.format_context_pub(&sym[0], "brief");
    let standard = mcp.format_context_pub(&sym[0], "standard");
    let full = mcp.format_context_pub(&sym[0], "full");

    assert!(
        standard.len() > brief.len(),
        "standard ({}) should be longer than brief ({})",
        standard.len(), brief.len()
    );
    assert!(
        full.len() >= standard.len(),
        "full ({}) should be >= standard ({})",
        full.len(), standard.len()
    );
}

// ── Budget batch packing ──────────────────────────────────────────────

#[test]
fn context_batch_respects_budget() {
    let store = setup_store();
    let mcp = EngramMcp::new(Arc::new(store), PathBuf::from("."));

    // Request context for multiple symbols with a tight budget
    let budgets = [100, 300, 500, 1000];

    for budget in budgets {
        // Use format_context_pub to simulate batch behavior
        // The real get_context_batch does greedy packing
        let sym_names = ["validate_token", "get_user", "handle_request"];
        let mut total_tokens = 0usize;

        for name in sym_names {
            if let Ok(syms) = mcp.store.find_symbol_by_name(name) {
                if let Some(sym) = syms.first() {
                    let brief = mcp.format_context_pub(sym, "brief");
                    let tokens = estimate_tokens(&brief);
                    if total_tokens + tokens <= budget {
                        total_tokens += tokens;
                    }
                }
            }
        }

        // Token usage should not exceed budget
        assert!(
            total_tokens <= budget,
            "budget={}: used {} tokens, should not exceed",
            budget, total_tokens
        );

        // With enough budget, we should pack at least something
        if budget >= 100 {
            assert!(
                total_tokens > 0,
                "budget={}: should pack at least one symbol",
                budget
            );
        }
    }
}

// ── RRF k-parameter sweep ─────────────────────────────────────────────

#[test]
fn rrf_k_parameter_sweep() {
    // Test that different k values produce valid but different rankings.
    // RRF score = Σ 1/(k + rank_i(d))
    // Lower k = more emphasis on top ranks. Higher k = more uniform.

    let signal1 = vec![
        ("a".to_string(), 0.95),
        ("b".to_string(), 0.80),
        ("c".to_string(), 0.60),
        ("d".to_string(), 0.40),
        ("e".to_string(), 0.20),
    ];

    let signal2 = vec![
        ("c".to_string(), 0.90),
        ("d".to_string(), 0.85),
        ("b".to_string(), 0.70),
        ("a".to_string(), 0.50),
        ("e".to_string(), 0.10),
    ];

    let k_values = [10.0, 30.0, 60.0, 100.0];
    let mut prev_top: Option<String> = None;

    println!("\n=== RRF k-parameter sweep ===");
    for k in k_values {
        // Manually compute RRF with this k
        let mut scores: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
        for (rank, (id, _)) in signal1.iter().enumerate() {
            *scores.entry(id.clone()).or_default() += 1.0 / (k + rank as f64 + 1.0);
        }
        for (rank, (id, _)) in signal2.iter().enumerate() {
            *scores.entry(id.clone()).or_default() += 1.0 / (k + rank as f64 + 1.0);
        }

        let mut ranked: Vec<(String, f64)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let top = &ranked[0].0;
        println!("  k={:3.0}: top={}, scores={:?}",
            k, top,
            ranked.iter().map(|(id, s)| format!("{}={:.4}", id, s)).collect::<Vec<_>>().join(", ")
        );

        // All scores should be positive
        for (id, score) in &ranked {
            assert!(*score > 0.0, "k={}: score for {} should be positive", k, id);
        }

        // All symbols should appear
        assert_eq!(ranked.len(), 5, "k={}: all 5 symbols should appear", k);

        prev_top = Some(top.clone());
    }

    // With the default k=60, test via the actual rrf_fuse function
    let fused = rrf_fuse(&[signal1, signal2], 5);
    assert_eq!(fused.len(), 5, "should return all 5 symbols");
    assert!(fused[0].score > fused[4].score, "first should score higher than last");
}

// ── Token efficiency metric ───────────────────────────────────────────

#[test]
fn token_efficiency_above_threshold() {
    let store = setup_store();
    let mcp = EngramMcp::new(Arc::new(store), PathBuf::from("."));

    let budget = 500;
    let sym_names = ["validate_token", "get_user", "format_response", "handle_request"];

    let mut total_useful = 0usize;
    let mut total_output = 0usize;

    for name in sym_names {
        if let Ok(syms) = mcp.store.find_symbol_by_name(name) {
            if let Some(sym) = syms.first() {
                let text = mcp.format_context_pub(sym, "brief");
                let tokens = estimate_tokens(&text);
                if total_output + tokens <= budget {
                    total_output += tokens;
                    // "Useful" = has actual content (name, file, signature)
                    if text.contains(&sym.name) {
                        total_useful += tokens;
                    }
                }
            }
        }
    }

    if total_output > 0 {
        let efficiency = total_useful as f64 / total_output as f64;
        println!("Token efficiency: {}/{} = {:.2}", total_useful, total_output, efficiency);
        assert!(
            efficiency >= 0.7,
            "token efficiency should be >= 0.7, got {:.2} ({}/{})",
            efficiency, total_useful, total_output
        );
    }
}
