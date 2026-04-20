use engram::parser::CodeParser;

#[test]
fn test_parse_rust_fixture() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/multilang/sample.rs"))
        .expect("parse rust failed");

    let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Config"), "should find Config struct");
    assert!(names.contains(&"Validator"), "should find Validator trait");
    assert!(names.contains(&"process_config"), "should find process_config fn");
    assert!(names.contains(&"Status"), "should find Status enum");

    // Check scope chain for impl methods
    for sym in &result.symbols {
        assert!(!sym.scope_chain.is_empty(), "scope_chain should not be empty for {}", sym.name);
    }
}

#[test]
fn test_parse_python_fixture() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/multilang/sample.py"))
        .expect("parse python failed");

    let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Config"), "should find Config class");
    assert!(names.contains(&"AdvancedConfig"), "should find AdvancedConfig class");
    assert!(names.contains(&"process_config"), "should find process_config fn");

    // Verify inheritance edge
    let inherits = result
        .edges
        .iter()
        .any(|e| e.kind.as_str() == "INHERITS");
    assert!(inherits, "should find INHERITS edge for AdvancedConfig -> Config");
}

#[test]
fn test_parse_typescript_fixture() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/multilang/sample.ts"))
        .expect("parse typescript failed");

    let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"ConfigService"), "should find ConfigService class");
    assert!(names.contains(&"processConfig"), "should find processConfig fn");
    assert!(names.contains(&"Validator"), "should find Validator interface");
}

#[test]
fn test_parse_javascript_fixture() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/multilang/sample.js"))
        .expect("parse javascript failed");

    let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"ConfigService"), "should find ConfigService class");
    assert!(names.contains(&"processConfig"), "should find processConfig fn");
}

#[test]
fn test_parse_go_fixture() {
    let parser = CodeParser::new();
    let result = parser
        .parse_file(std::path::Path::new("tests/fixtures/multilang/sample.go"))
        .expect("parse go failed");

    let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Config"), "should find Config struct: got {:?}", names);
    assert!(names.contains(&"ProcessConfig"), "should find ProcessConfig fn: got {:?}", names);
}

#[test]
fn test_all_languages_produce_symbols() {
    let parser = CodeParser::new();
    let fixtures = [
        ("tests/fixtures/multilang/sample.rs", "rust"),
        ("tests/fixtures/multilang/sample.py", "python"),
        ("tests/fixtures/multilang/sample.ts", "typescript"),
        ("tests/fixtures/multilang/sample.js", "javascript"),
        ("tests/fixtures/multilang/sample.go", "go"),
        ("tests/fixtures/multilang/sample.java", "java"),
        ("tests/fixtures/multilang/sample.c", "c"),
        ("tests/fixtures/multilang/sample.cpp", "cpp"),
        ("tests/fixtures/multilang/sample.rb", "ruby"),
    ];

    for (path, lang) in &fixtures {
        let result = parser
            .parse_file(std::path::Path::new(path))
            .unwrap_or_else(|e| panic!("parse {} failed: {}", lang, e));

        assert!(
            !result.symbols.is_empty(),
            "{} should produce at least one symbol, got none",
            lang
        );

        // Every symbol should have a non-empty scope_chain
        for sym in &result.symbols {
            assert!(
                !sym.scope_chain.is_empty(),
                "{}: symbol '{}' has empty scope_chain",
                lang,
                sym.name
            );
        }

        // Every symbol should have valid hashes
        for sym in &result.symbols {
            assert!(!sym.body_hash.is_empty(), "{}: {} has empty body_hash", lang, sym.name);
            assert!(!sym.full_hash.is_empty(), "{}: {} has empty full_hash", lang, sym.name);
            assert!(!sym.id.is_empty(), "{}: {} has empty id", lang, sym.name);
            assert!(!sym.canonical_id.is_empty(), "{}: {} has empty canonical_id", lang, sym.name);
        }
    }
}
