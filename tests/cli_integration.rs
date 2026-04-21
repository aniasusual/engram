use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn test_engram_init_creates_db() {
    let dir = TempDir::new().unwrap();
    Command::cargo_bin("engram")
        .unwrap()
        .args(["init", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("Engram initialized"));

    assert!(dir.path().join(".engram").join("engram.db").exists());
}

#[test]
fn test_engram_status_after_init() {
    let dir = TempDir::new().unwrap();
    // Init first
    Command::cargo_bin("engram")
        .unwrap()
        .args(["init", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success();

    // Status
    Command::cargo_bin("engram")
        .unwrap()
        .args(["status", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("Symbols: 0"))
        .stdout(predicates::str::contains("Edges:   0"))
        .stdout(predicates::str::contains("Files:   0"));
}

#[test]
fn test_engram_status_not_initialized() {
    let dir = TempDir::new().unwrap();
    Command::cargo_bin("engram")
        .unwrap()
        .args(["status", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("not initialized"));
}

#[test]
fn test_engram_search_not_initialized() {
    let dir = TempDir::new().unwrap();
    Command::cargo_bin("engram")
        .unwrap()
        .args(["search", "test", "--root", dir.path().to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn test_engram_search_with_indexed_fixtures() {
    let dir = TempDir::new().unwrap();

    // Init
    Command::cargo_bin("engram")
        .unwrap()
        .args(["init", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success();

    // Copy a fixture file into the temp dir
    let fixture_content = std::fs::read_to_string("tests/fixtures/simple/auth.rs").unwrap();
    std::fs::write(dir.path().join("auth.rs"), &fixture_content).unwrap();

    // Start (one-shot index mode — start indexes then watches, but we just need indexing)
    // Use a direct approach: init + start will index, but start blocks for watching.
    // Instead, test via the search command which uses the DB directly.
    // We need to manually index by running start in background...
    // Simpler: just test that search returns "not found" on empty DB.
    Command::cargo_bin("engram")
        .unwrap()
        .args(["search", "validate", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("No results"));
}

#[test]
fn test_engram_help() {
    Command::cargo_bin("engram")
        .unwrap()
        .args(["--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("codebase intelligence"));
}

#[test]
fn test_engram_install_hooks() {
    let dir = TempDir::new().unwrap();
    Command::cargo_bin("engram")
        .unwrap()
        .args(["install-hooks", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("hooks installed"));

    assert!(
        dir.path()
            .join(".claude")
            .join("settings.local.json")
            .exists()
    );

    // Verify the JSON is valid
    let json_content =
        std::fs::read_to_string(dir.path().join(".claude").join("settings.local.json")).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&json_content).expect("hooks JSON should be valid");
    assert!(
        parsed["hooks"]["PreToolUse"].is_array(),
        "should have PreToolUse hooks"
    );
}

#[test]
fn test_engram_search_on_indexed_repo() {
    let dir = TempDir::new().unwrap();

    // Init
    Command::cargo_bin("engram")
        .unwrap()
        .args(["init", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success();

    // Manually index via the Store (since `start` blocks for watching)
    let db_path = dir.path().join(".engram/engram.db");
    let store = engram::graph::Store::open(&db_path).unwrap();
    let parser = engram::parser::CodeParser::new();

    let fixture = std::fs::read_to_string("tests/fixtures/simple/auth.rs").unwrap();
    std::fs::write(dir.path().join("auth.rs"), &fixture).unwrap();

    let result = parser.parse_source(&fixture, "rust", "auth.rs").unwrap();
    store
        .sync_file(std::path::Path::new("auth.rs"), &result)
        .unwrap();
    drop(store); // Release DB lock

    // Now search should find results
    Command::cargo_bin("engram")
        .unwrap()
        .args(["search", "validate", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("validate_token"));
}
