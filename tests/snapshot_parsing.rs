use engram::parser::CodeParser;

/// Snapshot test for Rust symbol extraction.
#[test]
fn snapshot_rust_symbols() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/multilang/sample.rs"))
        .expect("parse failed");

    let symbols: Vec<_> = result
        .symbols
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "kind": s.kind,
                "line_start": s.line_start,
                "line_end": s.line_end,
                "scope_chain": s.scope_chain,
                "has_docstring": s.docstring.is_some(),
            })
        })
        .collect();

    insta::assert_yaml_snapshot!("rust_symbols", symbols);
}

/// Snapshot test for Python symbol extraction.
#[test]
fn snapshot_python_symbols() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/multilang/sample.py"))
        .expect("parse failed");

    let symbols: Vec<_> = result
        .symbols
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "kind": s.kind,
                "line_start": s.line_start,
                "line_end": s.line_end,
                "scope_chain": s.scope_chain,
                "has_docstring": s.docstring.is_some(),
            })
        })
        .collect();

    insta::assert_yaml_snapshot!("python_symbols", symbols);
}

/// Snapshot test for TypeScript symbol extraction.
#[test]
fn snapshot_typescript_symbols() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/multilang/sample.ts"))
        .expect("parse failed");

    let symbols: Vec<_> = result
        .symbols
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "kind": s.kind,
                "line_start": s.line_start,
                "line_end": s.line_end,
                "scope_chain": s.scope_chain,
            })
        })
        .collect();

    insta::assert_yaml_snapshot!("typescript_symbols", symbols);
}

/// Snapshot test for Go symbol extraction.
#[test]
fn snapshot_go_symbols() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/multilang/sample.go"))
        .expect("parse failed");

    let symbols: Vec<_> = result
        .symbols
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "kind": s.kind,
                "line_start": s.line_start,
                "line_end": s.line_end,
                "scope_chain": s.scope_chain,
            })
        })
        .collect();

    insta::assert_yaml_snapshot!("go_symbols", symbols);
}

/// Snapshot test for Java symbol extraction.
#[test]
fn snapshot_java_symbols() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/multilang/sample.java"))
        .expect("parse failed");

    let symbols: Vec<_> = result
        .symbols
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "kind": s.kind,
                "line_start": s.line_start,
                "line_end": s.line_end,
                "scope_chain": s.scope_chain,
            })
        })
        .collect();

    insta::assert_yaml_snapshot!("java_symbols", symbols);
}

/// Snapshot of call edges from the simple fixture.
#[test]
fn snapshot_call_edges() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/simple/handler.rs"))
        .expect("parse failed");

    let edges: Vec<_> = result
        .edges
        .iter()
        .map(|e| {
            serde_json::json!({
                "kind": e.kind.as_str(),
                "confidence": e.confidence,
            })
        })
        .collect();

    insta::assert_yaml_snapshot!("handler_call_edges", edges);
}

/// Snapshot test for C symbol extraction.
#[test]
fn snapshot_c_symbols() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/multilang/sample.c"))
        .expect("parse failed");

    let symbols: Vec<_> = result
        .symbols
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "kind": s.kind,
                "line_start": s.line_start,
                "line_end": s.line_end,
            })
        })
        .collect();

    insta::assert_yaml_snapshot!("c_symbols", symbols);
}

/// Snapshot test for C++ symbol extraction.
#[test]
fn snapshot_cpp_symbols() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/multilang/sample.cpp"))
        .expect("parse failed");

    let symbols: Vec<_> = result
        .symbols
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "kind": s.kind,
                "line_start": s.line_start,
                "line_end": s.line_end,
            })
        })
        .collect();

    insta::assert_yaml_snapshot!("cpp_symbols", symbols);
}

/// Snapshot test for Ruby symbol extraction.
#[test]
fn snapshot_ruby_symbols() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/multilang/sample.rb"))
        .expect("parse failed");

    let symbols: Vec<_> = result
        .symbols
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "kind": s.kind,
                "line_start": s.line_start,
                "line_end": s.line_end,
            })
        })
        .collect();

    insta::assert_yaml_snapshot!("ruby_symbols", symbols);
}
