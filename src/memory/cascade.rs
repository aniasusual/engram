//! Self-healing memory cascade.
//!
//! When a symbol changes, its annotations are marked stale and a BFS cascade
//! propagates confidence reductions through the reverse call graph. If code
//! reverts to its original hash, annotations are reactivated.

use anyhow::Result;
use std::collections::{HashSet, VecDeque};

use crate::graph::Store;

/// Base confidence reduction applied to direct callers of a changed symbol.
const BASE_REDUCTION: f64 = 0.3;

/// Decay factor per BFS hop. confidence_reduction = BASE_REDUCTION × DECAY^distance
const DECAY: f64 = 0.5;

/// Minimum confidence — never reduce below this.
const MIN_CONFIDENCE: f64 = 0.0;

/// Maximum confidence — verified annotations cap at this.
const MAX_CONFIDENCE: f64 = 1.0;

/// A single entry in the cascade log.
#[derive(Debug, Clone)]
pub struct CascadeEntry {
    pub trigger_symbol: String,
    pub affected_symbol: String,
    pub annotation_id: i64,
    pub old_confidence: f64,
    pub new_confidence: f64,
    pub reason: String,
}

/// Result of running a cascade.
#[derive(Debug)]
pub struct CascadeResult {
    pub trigger_symbol: String,
    pub direct_stale_count: usize,
    pub transitive_affected_count: usize,
    pub reactivated_count: usize,
    pub log: Vec<CascadeEntry>,
}

/// Run the staleness cascade for a single symbol that has changed.
///
/// 1. Check all annotations on this symbol — mark stale if full_hash_at != current full_hash,
///    or reactivate if full_hash_at matches again.
/// 2. BFS through reverse call graph (callers), reducing confidence on their annotations
///    with exponential decay by distance.
/// 3. Log every change to cascade_log.
pub fn run_cascade(store: &Store, changed_symbol_id: &str) -> Result<CascadeResult> {
    let mut log = Vec::new();
    let mut direct_stale = 0usize;
    let mut reactivated = 0usize;

    // Get the current symbol state
    let symbol = store
        .get_symbol(changed_symbol_id)?
        .ok_or_else(|| anyhow::anyhow!("symbol not found: {}", changed_symbol_id))?;

    let current_full_hash = &symbol.full_hash;
    let now = chrono::Utc::now().timestamp();

    // Phase 1: Direct staleness / reactivation on the changed symbol itself
    let annotations = store.get_annotations(changed_symbol_id)?;
    for (ann_id, _ann_type, _content, confidence, status) in &annotations {
        let ann_hash = store.get_annotation_hash(*ann_id)?;

        if ann_hash.as_deref() == Some(current_full_hash.as_str()) {
            // Hash matches — reactivate if stale
            if status == "stale" {
                store.update_annotation_status(*ann_id, "active", MAX_CONFIDENCE)?;
                log.push(CascadeEntry {
                    trigger_symbol: changed_symbol_id.to_string(),
                    affected_symbol: changed_symbol_id.to_string(),
                    annotation_id: *ann_id,
                    old_confidence: *confidence,
                    new_confidence: MAX_CONFIDENCE,
                    reason: "revert_reactivation: full_hash matches full_hash_at".to_string(),
                });
                reactivated += 1;
            }
        } else {
            // Hash mismatch — mark stale
            if status == "active" {
                store.update_annotation_status(*ann_id, "stale", *confidence)?;
                log.push(CascadeEntry {
                    trigger_symbol: changed_symbol_id.to_string(),
                    affected_symbol: changed_symbol_id.to_string(),
                    annotation_id: *ann_id,
                    old_confidence: *confidence,
                    new_confidence: *confidence,
                    reason: "direct_staleness: symbol body changed".to_string(),
                });
                direct_stale += 1;
            }
        }
    }

    // Phase 2: BFS cascade through reverse call graph (callers)
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(changed_symbol_id.to_string());

    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    // Seed with direct callers
    let direct_callers = store.get_direct_callers(changed_symbol_id)?;
    for caller_id in &direct_callers {
        if visited.insert(caller_id.clone()) {
            queue.push_back((caller_id.clone(), 1));
        }
    }

    let mut transitive_affected = 0usize;

    while let Some((current_id, distance)) = queue.pop_front() {
        let reduction = BASE_REDUCTION * DECAY.powi(distance as i32 - 1);

        if reduction < 0.01 {
            continue; // Too small to matter
        }

        // Reduce confidence on this symbol's annotations
        let caller_annotations = store.get_annotations(&current_id)?;
        for (ann_id, _ann_type, _content, confidence, status) in &caller_annotations {
            if status == "active" || status == "stale" {
                let new_confidence = (*confidence - reduction).max(MIN_CONFIDENCE);
                if (new_confidence - *confidence).abs() > 0.001 {
                    store.reduce_annotation_confidence(*ann_id, new_confidence)?;
                    log.push(CascadeEntry {
                        trigger_symbol: changed_symbol_id.to_string(),
                        affected_symbol: current_id.clone(),
                        annotation_id: *ann_id,
                        old_confidence: *confidence,
                        new_confidence,
                        reason: format!(
                            "transitive_cascade: distance={}, reduction={:.3}",
                            distance, reduction
                        ),
                    });
                    transitive_affected += 1;
                }
            }
        }

        // Continue BFS to callers of this caller
        let next_callers = store.get_direct_callers(&current_id)?;
        for next_id in &next_callers {
            if visited.insert(next_id.clone()) {
                queue.push_back((next_id.clone(), distance + 1));
            }
        }
    }

    // Phase 3: Write cascade log to database
    for entry in &log {
        store.write_cascade_log(entry, now)?;
    }

    Ok(CascadeResult {
        trigger_symbol: changed_symbol_id.to_string(),
        direct_stale_count: direct_stale,
        transitive_affected_count: transitive_affected,
        reactivated_count: reactivated,
        log,
    })
}

/// Run cascade for all symbols in a file that changed during sync.
/// Call this after sync_file returns true.
pub fn cascade_file(store: &Store, file: &str) -> Result<Vec<CascadeResult>> {
    let symbols = store.get_file_symbols(file)?;
    let mut results = Vec::new();

    for sym in &symbols {
        // Check if this symbol has any annotations that might be affected
        let annotations = store.get_annotations(&sym.id)?;
        if annotations.is_empty() {
            // Also check if any callers have annotations
            let callers = store.get_direct_callers(&sym.id)?;
            let has_caller_annotations = callers.iter().any(|c| {
                store.get_annotations(c).map(|a| !a.is_empty()).unwrap_or(false)
            });
            if !has_caller_annotations {
                continue;
            }
        }

        let result = run_cascade(store, &sym.id)?;
        if result.direct_stale_count > 0
            || result.transitive_affected_count > 0
            || result.reactivated_count > 0
        {
            results.push(result);
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::CodeParser;
    use std::path::Path;

    fn setup() -> Store {
        let store = Store::open_in_memory().unwrap();
        store.initialize().unwrap();
        store
    }

    #[test]
    fn test_single_hop_cascade() {
        let store = setup();
        let parser = CodeParser::new();

        // A calls B
        let source = r#"
fn callee() -> bool { true }
fn caller() { callee(); }
"#;
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        // Annotate callee
        let callee = store.find_symbol_by_name("callee").unwrap();
        let callee_id = &callee[0].id;
        let callee_hash = &callee[0].full_hash;
        store.create_annotation(callee_id, "explanation", "callee always returns true", callee_hash).unwrap();

        // Annotate caller
        let caller = store.find_symbol_by_name("caller").unwrap();
        let caller_id = &caller[0].id;
        let caller_hash = &caller[0].full_hash;
        store.create_annotation(caller_id, "context", "caller depends on callee", caller_hash).unwrap();

        // Now change callee
        let new_source = r#"
fn callee() -> bool { false }
fn caller() { callee(); }
"#;
        let new_result = parser.parse_source(new_source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &new_result).unwrap();

        // Run cascade on callee
        let new_callee = store.find_symbol_by_name("callee").unwrap();
        let cascade = run_cascade(&store, &new_callee[0].id).unwrap();

        // callee's annotation should be stale
        assert!(cascade.direct_stale_count >= 1, "callee annotation should be stale");

        // caller's annotation confidence should be reduced
        assert!(cascade.transitive_affected_count >= 1, "caller annotation should be affected");

        // Cascade log should be populated
        assert!(!cascade.log.is_empty(), "cascade log should not be empty");
    }

    #[test]
    fn test_multi_hop_cascade() {
        let store = setup();
        let parser = CodeParser::new();

        // A calls B calls C
        let source = r#"
fn leaf() -> i32 { 42 }
fn middle() -> i32 { leaf() }
fn top() -> i32 { middle() }
"#;
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        // Annotate all three
        for name in &["leaf", "middle", "top"] {
            let syms = store.find_symbol_by_name(name).unwrap();
            let sym = &syms[0];
            store.create_annotation(&sym.id, "explanation", &format!("{} does something", name), &sym.full_hash).unwrap();
        }

        // Change leaf
        let new_source = r#"
fn leaf() -> i32 { 99 }
fn middle() -> i32 { leaf() }
fn top() -> i32 { middle() }
"#;
        let new_result = parser.parse_source(new_source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &new_result).unwrap();

        let new_leaf = store.find_symbol_by_name("leaf").unwrap();
        let cascade = run_cascade(&store, &new_leaf[0].id).unwrap();

        // leaf: stale, middle: reduced at distance 1, top: reduced at distance 2
        assert!(cascade.direct_stale_count >= 1);
        assert!(cascade.transitive_affected_count >= 1);

        // Check decay: middle should lose more confidence than top
        let middle_entry = cascade.log.iter().find(|e| {
            let sym = store.get_symbol(&e.affected_symbol).unwrap();
            sym.map(|s| s.name == "middle").unwrap_or(false)
        });
        let top_entry = cascade.log.iter().find(|e| {
            let sym = store.get_symbol(&e.affected_symbol).unwrap();
            sym.map(|s| s.name == "top").unwrap_or(false)
        });

        if let (Some(m), Some(t)) = (middle_entry, top_entry) {
            let middle_reduction = m.old_confidence - m.new_confidence;
            let top_reduction = t.old_confidence - t.new_confidence;
            assert!(
                middle_reduction > top_reduction,
                "middle should lose more confidence ({}) than top ({})",
                middle_reduction, top_reduction
            );
        }
    }

    #[test]
    fn test_diamond_cascade_no_double_count() {
        let store = setup();
        let parser = CodeParser::new();

        // Diamond: top -> b -> leaf, top -> c -> leaf
        let source = r#"
fn leaf() -> bool { true }
fn route_b() -> bool { leaf() }
fn route_c() -> bool { leaf() }
fn top() -> bool { route_b() && route_c() }
"#;
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        // Annotate top
        let top = store.find_symbol_by_name("top").unwrap();
        store.create_annotation(&top[0].id, "context", "top combines routes", &top[0].full_hash).unwrap();

        // Change leaf
        let new_source = r#"
fn leaf() -> bool { false }
fn route_b() -> bool { leaf() }
fn route_c() -> bool { leaf() }
fn top() -> bool { route_b() && route_c() }
"#;
        let new_result = parser.parse_source(new_source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &new_result).unwrap();

        let new_leaf = store.find_symbol_by_name("leaf").unwrap();
        let cascade = run_cascade(&store, &new_leaf[0].id).unwrap();

        // Top should only appear ONCE in cascade log (BFS visited set prevents double-counting)
        let top_entries: Vec<_> = cascade.log.iter()
            .filter(|e| {
                store.get_symbol(&e.affected_symbol).ok()
                    .flatten()
                    .map(|s| s.name == "top")
                    .unwrap_or(false)
            })
            .collect();

        assert!(
            top_entries.len() <= 1,
            "top should appear at most once in cascade (got {})",
            top_entries.len()
        );
    }

    #[test]
    fn test_revert_reactivates() {
        let store = setup();
        let parser = CodeParser::new();

        let original_source = r#"fn greet() { println!("hello"); }"#;
        let result = parser.parse_source(original_source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        // Annotate
        let sym = store.find_symbol_by_name("greet").unwrap();
        let _ann_id = store.create_annotation(&sym[0].id, "explanation", "greets the user", &sym[0].full_hash).unwrap();

        // Change the function
        let changed_source = r#"fn greet() { println!("goodbye"); }"#;
        let changed_result = parser.parse_source(changed_source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &changed_result).unwrap();

        let changed_sym = store.find_symbol_by_name("greet").unwrap();
        let cascade1 = run_cascade(&store, &changed_sym[0].id).unwrap();
        assert!(cascade1.direct_stale_count >= 1, "should detect staleness");

        // Revert to original
        let reverted_result = parser.parse_source(original_source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &reverted_result).unwrap();

        let reverted_sym = store.find_symbol_by_name("greet").unwrap();
        let cascade2 = run_cascade(&store, &reverted_sym[0].id).unwrap();
        assert!(cascade2.reactivated_count >= 1, "should reactivate annotation on revert");
    }

    #[test]
    fn test_cascade_log_audit_trail() {
        let store = setup();
        let parser = CodeParser::new();

        let source = r#"
fn helper() -> i32 { 1 }
fn main_fn() -> i32 { helper() }
"#;
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        let helper = store.find_symbol_by_name("helper").unwrap();
        store.create_annotation(&helper[0].id, "explanation", "returns 1", &helper[0].full_hash).unwrap();

        let main_fn = store.find_symbol_by_name("main_fn").unwrap();
        store.create_annotation(&main_fn[0].id, "context", "uses helper", &main_fn[0].full_hash).unwrap();

        // Change helper
        let new_source = r#"
fn helper() -> i32 { 2 }
fn main_fn() -> i32 { helper() }
"#;
        let new_result = parser.parse_source(new_source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &new_result).unwrap();

        let new_helper = store.find_symbol_by_name("helper").unwrap();
        let cascade = run_cascade(&store, &new_helper[0].id).unwrap();

        // Every log entry should have required fields
        for entry in &cascade.log {
            assert!(!entry.trigger_symbol.is_empty());
            assert!(!entry.affected_symbol.is_empty());
            assert!(entry.annotation_id > 0);
            assert!(!entry.reason.is_empty());
        }

        // Database cascade_log should match
        let db_log = store.get_cascade_log(&new_helper[0].id).unwrap();
        assert_eq!(db_log.len(), cascade.log.len(),
            "database cascade_log should match in-memory log");
    }

    #[test]
    fn test_no_annotations_no_cascade() {
        let store = setup();
        let parser = CodeParser::new();

        let source = r#"fn lonely() { }"#;
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        let sym = store.find_symbol_by_name("lonely").unwrap();
        let cascade = run_cascade(&store, &sym[0].id).unwrap();

        assert_eq!(cascade.direct_stale_count, 0);
        assert_eq!(cascade.transitive_affected_count, 0);
        assert_eq!(cascade.reactivated_count, 0);
        assert!(cascade.log.is_empty());
    }

    #[test]
    fn test_confidence_never_negative() {
        let store = setup();
        let parser = CodeParser::new();

        let source = r#"
fn leaf() {}
fn caller() { leaf(); }
"#;
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        let caller = store.find_symbol_by_name("caller").unwrap();
        // Create annotation with very low confidence
        store.create_annotation(&caller[0].id, "context", "low confidence note", &caller[0].full_hash).unwrap();
        // Downvote it multiple times to get confidence near 0
        let anns = store.get_annotations(&caller[0].id).unwrap();
        for _ in 0..10 {
            store.downvote_annotation(anns[0].0).unwrap();
        }

        // Change leaf to trigger cascade
        let new_source = r#"
fn leaf() { println!("changed"); }
fn caller() { leaf(); }
"#;
        let new_result = parser.parse_source(new_source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &new_result).unwrap();

        let new_leaf = store.find_symbol_by_name("leaf").unwrap();
        let cascade = run_cascade(&store, &new_leaf[0].id).unwrap();

        // Confidence should never be negative
        for entry in &cascade.log {
            assert!(
                entry.new_confidence >= 0.0,
                "confidence should never be negative, got {}",
                entry.new_confidence
            );
        }
    }
}
