use criterion::{criterion_group, criterion_main, Criterion};
use std::path::Path;

fn setup_indexed_store() -> engram::graph::Store {
    let store = engram::graph::Store::open_in_memory().unwrap();
    store.initialize().unwrap();
    let parser = engram::parser::CodeParser::new();

    // Index all simple fixtures
    let fixtures = ["auth.rs", "db.rs", "handler.rs", "utils.rs", "main.rs"];
    for file in &fixtures {
        let path = format!("tests/fixtures/simple/{}", file);
        let source = std::fs::read_to_string(&path).expect("fixture not found");
        let result = parser.parse_source(&source, "rust", file).expect("parse");
        store.sync_file(Path::new(file), &result).unwrap();
    }
    store
}

fn setup_large_indexed_store() -> engram::graph::Store {
    let store = engram::graph::Store::open_in_memory().unwrap();
    store.initialize().unwrap();
    let parser = engram::parser::CodeParser::new();

    let gen_dir = Path::new("tests/fixtures/large/generated");
    if !gen_dir.exists() {
        return store;
    }

    for entry in std::fs::read_dir(gen_dir).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().is_some_and(|ext| ext == "rs") {
            let source = std::fs::read_to_string(entry.path()).unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            let result = parser.parse_source(&source, "rust", &name).unwrap();
            store.sync_file(Path::new(&name), &result).unwrap();
        }
    }
    store
}

fn bench_bm25_search_small(c: &mut Criterion) {
    let store = setup_indexed_store();

    c.bench_function("bm25_search_5_files", |b| {
        b.iter(|| store.search_bm25("validate token", 10).unwrap())
    });
}

fn bench_bm25_search_large(c: &mut Criterion) {
    let store = setup_large_indexed_store();
    let stats = store.stats().unwrap();
    if stats.symbol_count == 0 {
        return;
    }

    c.bench_function(&format!("bm25_search_{}_symbols", stats.symbol_count), |b| {
        b.iter(|| store.search_bm25("process validate service", 10).unwrap())
    });
}

fn bench_find_callers(c: &mut Criterion) {
    let store = setup_indexed_store();
    let syms = store.find_symbol_by_name("validate_token").unwrap();
    if syms.is_empty() {
        return;
    }
    let sym_id = syms[0].id.clone();

    c.bench_function("find_callers_depth_3", |b| {
        b.iter(|| store.find_callers(&sym_id, 3).unwrap())
    });
}

fn bench_find_dependencies(c: &mut Criterion) {
    let store = setup_indexed_store();
    let syms = store.find_symbol_by_name("handle_request").unwrap();
    if syms.is_empty() {
        return;
    }
    let sym_id = syms[0].id.clone();

    c.bench_function("find_dependencies_depth_3", |b| {
        b.iter(|| store.find_dependencies(&sym_id, 3).unwrap())
    });
}

fn bench_cascade_propagation(c: &mut Criterion) {
    let store = setup_indexed_store();
    let parser = engram::parser::CodeParser::new();

    // Create annotations on multiple symbols
    let names = ["validate_token", "handle_request", "format_response"];
    for name in &names {
        if let Ok(syms) = store.find_symbol_by_name(name) {
            if let Some(sym) = syms.first() {
                let _ = store.create_annotation(&sym.id, "explanation", &format!("{} does something", name), &sym.full_hash);
            }
        }
    }

    // Change a leaf function to trigger cascade
    let modified = r#"
pub fn format_response(message: &str) -> String {
    format!("Response[v2]: {}", message)
}
pub fn sanitize_input(input: &str) -> String {
    input.chars().filter(|c| c.is_alphanumeric()).collect()
}
"#;
    let result = parser.parse_source(modified, "rust", "utils.rs").unwrap();
    store.sync_file(Path::new("utils.rs"), &result).unwrap();

    let syms = store.find_symbol_by_name("format_response").unwrap();
    if syms.is_empty() {
        return;
    }
    let sym_id = syms[0].id.clone();

    c.bench_function("cascade_propagation", |b| {
        b.iter(|| {
            engram::memory::cascade::run_cascade(&store, &sym_id).unwrap()
        })
    });
}

fn bench_risk_score(c: &mut Criterion) {
    let store = setup_indexed_store();
    let syms = store.find_symbol_by_name("validate_token").unwrap();
    if syms.is_empty() {
        return;
    }
    let sym_id = syms[0].id.clone();

    c.bench_function("risk_score_computation", |b| {
        b.iter(|| store.get_risk_score(&sym_id).unwrap())
    });
}

fn bench_embedding_text_generation(c: &mut Criterion) {
    let store = setup_indexed_store();
    let syms = store.find_symbol_by_name("validate_token").unwrap();
    if syms.is_empty() {
        return;
    }
    let sym = &syms[0];
    let callers: Vec<String> = store
        .get_direct_callers(&sym.id)
        .unwrap_or_default()
        .iter()
        .filter_map(|id| store.get_symbol(id).ok().flatten().map(|s| s.name))
        .collect();

    c.bench_function("embedding_text_generation", |b| {
        b.iter(|| {
            engram::embeddings::EmbeddingEngine::build_embedding_text(sym, &callers, &[])
        })
    });
}

fn bench_community_detection(c: &mut Criterion) {
    let store = setup_large_indexed_store();
    let stats = store.stats().unwrap();
    if stats.symbol_count == 0 {
        return;
    }

    c.bench_function(&format!("community_detection_{}_symbols", stats.symbol_count), |b| {
        b.iter(|| {
            engram::intelligence::community::detect_communities(&store).unwrap()
        })
    });
}

criterion_group!(benches,
    bench_bm25_search_small,
    bench_bm25_search_large,
    bench_find_callers,
    bench_find_dependencies,
    bench_cascade_propagation,
    bench_risk_score,
    bench_embedding_text_generation,
    bench_community_detection,
);
criterion_main!(benches);
