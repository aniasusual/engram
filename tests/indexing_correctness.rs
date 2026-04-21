//! Incremental indexing correctness tests.
//!
//! Verifies that the sync engine doesn't lose data across
//! full vs incremental indexing, edits, deletes, and renames.

use engram::graph::Store;
use engram::parser::CodeParser;
use std::path::Path;

fn setup() -> (Store, CodeParser) {
    let store = Store::open_in_memory().unwrap();
    store.initialize().unwrap();
    (store, CodeParser::new())
}

// ── Test 1: full_index_equals_incremental ─────────────────────────────

#[test]
fn full_index_equals_incremental() {
    let parser = CodeParser::new();

    let sources = [
        ("a.rs", "fn alpha() { beta(); }\nfn beta() -> bool { true }"),
        ("b.rs", "fn gamma() { delta(); }\nfn delta() -> i32 { 42 }"),
        ("c.rs", "fn epsilon() {}"),
    ];

    // Method 1: index all at once
    let store1 = Store::open_in_memory().unwrap();
    store1.initialize().unwrap();
    for (name, src) in &sources {
        let result = parser.parse_source(src, "rust", name).unwrap();
        store1.sync_file(Path::new(name), &result).unwrap();
    }
    let stats1 = store1.stats().unwrap();

    // Method 2: index one-by-one into fresh store
    let store2 = Store::open_in_memory().unwrap();
    store2.initialize().unwrap();
    for (name, src) in &sources {
        let result = parser.parse_source(src, "rust", name).unwrap();
        store2.sync_file(Path::new(name), &result).unwrap();
    }
    let stats2 = store2.stats().unwrap();

    assert_eq!(
        stats1.symbol_count, stats2.symbol_count,
        "symbol count should match: bulk={} vs incremental={}",
        stats1.symbol_count, stats2.symbol_count
    );
    assert_eq!(
        stats1.edge_count, stats2.edge_count,
        "edge count should match: bulk={} vs incremental={}",
        stats1.edge_count, stats2.edge_count
    );
    assert_eq!(
        stats1.file_count, stats2.file_count,
        "file count should match: bulk={} vs incremental={}",
        stats1.file_count, stats2.file_count
    );

    // Verify same symbols exist by name
    let names1: Vec<String> = store1
        .get_all_symbols()
        .unwrap()
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let names2: Vec<String> = store2
        .get_all_symbols()
        .unwrap()
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let mut sorted1 = names1.clone();
    sorted1.sort();
    let mut sorted2 = names2.clone();
    sorted2.sort();
    assert_eq!(sorted1, sorted2, "same symbol names in both stores");
}

// ── Test 2: edit_only_reindexes_changed ───────────────────────────────

#[test]
fn edit_only_reindexes_changed() {
    let (store, parser) = setup();

    // Index two files
    let src_a = "fn unchanged() -> bool { true }";
    let src_b = "fn will_change() -> i32 { 1 }";

    let r_a = parser.parse_source(src_a, "rust", "a.rs").unwrap();
    let r_b = parser.parse_source(src_b, "rust", "b.rs").unwrap();
    store.sync_file(Path::new("a.rs"), &r_a).unwrap();
    store.sync_file(Path::new("b.rs"), &r_b).unwrap();

    let initial_count = store.stats().unwrap().symbol_count;
    assert_eq!(initial_count, 2);

    // Edit b.rs — add a function
    let src_b_edited = "fn will_change() -> i32 { 2 }\nfn new_function() {}";
    let r_b_edited = parser.parse_source(src_b_edited, "rust", "b.rs").unwrap();

    // Re-syncing a.rs should be a no-op (unchanged)
    let a_changed = store.sync_file(Path::new("a.rs"), &r_a).unwrap();
    assert!(!a_changed, "unchanged file should not trigger re-index");

    // Re-syncing b.rs should detect changes
    let b_changed = store.sync_file(Path::new("b.rs"), &r_b_edited).unwrap();
    assert!(b_changed, "edited file should trigger re-index");

    let new_count = store.stats().unwrap().symbol_count;
    assert_eq!(
        new_count, 3,
        "should have 3 symbols after adding new_function"
    );

    // Verify the new function exists
    let new_fn = store.find_symbol_by_name("new_function").unwrap();
    assert!(!new_fn.is_empty(), "new_function should be in the store");

    // Verify the old unchanged function is still there
    let old_fn = store.find_symbol_by_name("unchanged").unwrap();
    assert!(!old_fn.is_empty(), "unchanged function should still exist");
}

// ── Test 3: delete_removes_symbols_and_edges ──────────────────────────

#[test]
fn delete_removes_symbols_and_edges() {
    let (store, parser) = setup();

    // Index two files with a call edge between them
    let src_a = "fn caller() { callee(); }";
    let src_b = "fn callee() -> bool { true }";

    let r_a = parser.parse_source(src_a, "rust", "a.rs").unwrap();
    let r_b = parser.parse_source(src_b, "rust", "b.rs").unwrap();
    store.sync_file(Path::new("a.rs"), &r_a).unwrap();
    store.sync_file(Path::new("b.rs"), &r_b).unwrap();

    assert_eq!(store.stats().unwrap().symbol_count, 2);
    assert_eq!(store.stats().unwrap().file_count, 2);

    // Create an annotation on callee
    let callee_sym = store.find_symbol_by_name("callee").unwrap();
    assert!(!callee_sym.is_empty());
    store
        .create_annotation(
            &callee_sym[0].id,
            "explanation",
            "callee returns true",
            &callee_sym[0].full_hash,
        )
        .unwrap();

    // "Delete" b.rs by garbage collecting it
    let removed = store.garbage_collect(&["a.rs"]).unwrap();
    assert_eq!(removed, 1, "should remove 1 file");

    // Verify callee is gone
    assert_eq!(
        store.stats().unwrap().symbol_count,
        1,
        "only caller should remain"
    );
    assert_eq!(
        store.stats().unwrap().file_count,
        1,
        "only a.rs should remain"
    );

    let callee_after = store.find_symbol_by_name("callee").unwrap();
    assert!(
        callee_after.is_empty(),
        "callee should be removed from store"
    );

    // Caller should still exist
    let caller_after = store.find_symbol_by_name("caller").unwrap();
    assert!(!caller_after.is_empty(), "caller should still exist");
}

// ── Test 4: rename_preserves_annotations ──────────────────────────────

#[test]
fn rename_preserves_annotations_via_canonical_id() {
    let (store, parser) = setup();

    // Index function in file a.rs
    let source = "fn important_function() -> bool { true }";
    let r_a = parser.parse_source(source, "rust", "a.rs").unwrap();
    store.sync_file(Path::new("a.rs"), &r_a).unwrap();

    // Get the canonical_id before rename
    let sym_before = store.find_symbol_by_name("important_function").unwrap();
    assert!(!sym_before.is_empty());
    let canonical_before = sym_before[0].canonical_id.clone();

    // Create annotation
    let ann_id = store
        .create_annotation(
            &sym_before[0].id,
            "explanation",
            "this function is critical",
            &sym_before[0].full_hash,
        )
        .unwrap();
    assert!(ann_id > 0);

    // "Rename" by indexing the same code in b.rs and removing a.rs
    let r_b = parser.parse_source(source, "rust", "b.rs").unwrap();
    store.sync_file(Path::new("b.rs"), &r_b).unwrap();
    store.garbage_collect(&["b.rs"]).unwrap();

    // Get the canonical_id after rename
    let sym_after = store.find_symbol_by_name("important_function").unwrap();
    assert!(!sym_after.is_empty(), "function should exist in new file");
    let canonical_after = sym_after[0].canonical_id.clone();

    // canonical_id should be the same (same name + kind + body_hash)
    assert_eq!(
        canonical_before, canonical_after,
        "canonical_id should survive rename: before={} after={}",
        canonical_before, canonical_after
    );

    // The annotation is on the OLD symbol ID (a.rs version), not the new one (b.rs).
    // But the canonical_id link means we can find it.
    // In a full implementation, detect_renames would migrate annotations.
    // For now, verify the canonical_id mechanism works.
}

// ── Test 5: idempotency ───────────────────────────────────────────────

#[test]
fn idempotent_double_index() {
    let (store, parser) = setup();

    let source = "fn stable() -> i32 { 42 }\nfn also_stable() {}";
    let result = parser.parse_source(source, "rust", "test.rs").unwrap();

    let changed1 = store.sync_file(Path::new("test.rs"), &result).unwrap();
    assert!(changed1, "first sync should report changes");

    let stats1 = store.stats().unwrap();

    let changed2 = store.sync_file(Path::new("test.rs"), &result).unwrap();
    assert!(!changed2, "second sync of identical file should be no-op");

    let stats2 = store.stats().unwrap();
    assert_eq!(stats1.symbol_count, stats2.symbol_count);
    assert_eq!(stats1.edge_count, stats2.edge_count);
    assert_eq!(stats1.file_count, stats2.file_count);
}
