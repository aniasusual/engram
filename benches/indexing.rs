use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::path::Path;

fn bench_parse_rust_file(c: &mut Criterion) {
    let source =
        std::fs::read_to_string("tests/fixtures/simple/auth.rs").expect("fixture not found");
    let parser = engram::parser::CodeParser::new();

    c.bench_function("parse_single_rust_file", |b| {
        b.iter(|| parser.parse_source(&source, "rust", "auth.rs").unwrap())
    });
}

fn bench_sync_file(c: &mut Criterion) {
    let source =
        std::fs::read_to_string("tests/fixtures/simple/auth.rs").expect("fixture not found");
    let parser = engram::parser::CodeParser::new();
    let result = parser.parse_source(&source, "rust", "auth.rs").unwrap();

    c.bench_function("sync_file_to_db", |b| {
        b.iter_batched(
            || {
                let store = engram::graph::Store::open_in_memory().unwrap();
                store.initialize().unwrap();
                store
            },
            |store| store.sync_file(Path::new("auth.rs"), &result).unwrap(),
            BatchSize::SmallInput,
        )
    });
}

fn bench_incremental_update(c: &mut Criterion) {
    // Setup: index all simple fixtures, then re-index one changed file
    let store = engram::graph::Store::open_in_memory().unwrap();
    store.initialize().unwrap();
    let parser = engram::parser::CodeParser::new();

    let fixtures = ["auth.rs", "db.rs", "handler.rs", "utils.rs", "main.rs"];
    for file in &fixtures {
        let path = format!("tests/fixtures/simple/{}", file);
        let source = std::fs::read_to_string(&path).unwrap();
        let result = parser.parse_source(&source, "rust", file).unwrap();
        store.sync_file(Path::new(file), &result).unwrap();
    }

    // Modified version of auth.rs
    let modified_source = r#"
pub struct AuthService { secret: String }
impl AuthService {
    pub fn new(secret: &str) -> Self { Self { secret: secret.to_string() } }
    pub fn validate_token(&self, token: &str) -> bool { !token.is_empty() && token.starts_with(&self.secret) }
    pub fn extract_user_id(&self, token: &str) -> Option<String> { Some(token.to_string()) }
    pub fn revoke_token(&self, _token: &str) -> bool { true }
}
"#;
    let modified_result = parser
        .parse_source(modified_source, "rust", "auth.rs")
        .unwrap();

    c.bench_function("incremental_single_file_change", |b| {
        b.iter(|| {
            store
                .sync_file(Path::new("auth.rs"), &modified_result)
                .unwrap()
        })
    });
}

fn bench_parse_200_files(c: &mut Criterion) {
    let parser = engram::parser::CodeParser::new();
    let gen_dir = Path::new("tests/fixtures/large/generated");

    if !gen_dir.exists() {
        eprintln!("Large fixture not generated. Run tests/fixtures/large/generate.sh first.");
        return;
    }

    let files: Vec<_> = std::fs::read_dir(gen_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        .map(|e| e.path())
        .collect();

    c.bench_function(&format!("parse_{}_rust_files", files.len()), |b| {
        b.iter(|| {
            for file in &files {
                let source = std::fs::read_to_string(file).unwrap();
                let name = file.file_name().unwrap().to_str().unwrap();
                parser.parse_source(&source, "rust", name).unwrap();
            }
        })
    });
}

fn bench_index_200_files(c: &mut Criterion) {
    let parser = engram::parser::CodeParser::new();
    let gen_dir = Path::new("tests/fixtures/large/generated");

    if !gen_dir.exists() {
        return;
    }

    let files: Vec<(String, engram::parser::ParseResult)> = std::fs::read_dir(gen_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        .map(|e| {
            let path = e.path();
            let source = std::fs::read_to_string(&path).unwrap();
            let name = path.file_name().unwrap().to_str().unwrap().to_string();
            let result = parser.parse_source(&source, "rust", &name).unwrap();
            (name, result)
        })
        .collect();

    c.bench_function(&format!("index_{}_files_full", files.len()), |b| {
        b.iter_batched(
            || {
                let store = engram::graph::Store::open_in_memory().unwrap();
                store.initialize().unwrap();
                store
            },
            |store| {
                for (name, result) in &files {
                    store.sync_file(Path::new(name), result).unwrap();
                }
            },
            BatchSize::LargeInput,
        )
    });
}

criterion_group!(
    benches,
    bench_parse_rust_file,
    bench_sync_file,
    bench_incremental_update,
    bench_parse_200_files,
    bench_index_200_files,
);
criterion_main!(benches);
