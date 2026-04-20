//! Contradiction detection between annotations.
//!
//! Flags when two active annotations on the same symbol make conflicting assertions.

use anyhow::Result;

use crate::graph::Store;

/// A detected contradiction between two annotations.
#[derive(Debug, Clone)]
pub struct Contradiction {
    pub symbol_id: String,
    pub annotation_a_id: i64,
    pub annotation_b_id: i64,
    pub reason: String,
}

/// Common contradiction indicators — pairs of words/phrases that suggest conflict.
const CONTRADICTION_PAIRS: &[(&str, &str)] = &[
    ("always", "never"),
    ("always", "sometimes"),
    ("true", "false"),
    ("returns true", "returns false"),
    ("mutable", "immutable"),
    ("synchronous", "asynchronous"),
    ("sync", "async"),
    ("safe", "unsafe"),
    ("pure", "impure"),
    ("stateless", "stateful"),
    ("required", "optional"),
    ("deprecated", "recommended"),
    ("public", "private"),
    ("todo", "done"),
    ("broken", "working"),
    ("slow", "fast"),
    ("should not", "must"),
    ("do not", "always"),
];

/// Check for contradictions among active annotations on the same symbol.
pub fn detect_contradictions(store: &Store, symbol_id: &str) -> Result<Vec<Contradiction>> {
    let annotations = store.get_annotations(symbol_id)?;
    let active: Vec<_> = annotations
        .iter()
        .filter(|(_, _, _, _, status)| status == "active")
        .collect();

    if active.len() < 2 {
        return Ok(vec![]);
    }

    let mut contradictions = Vec::new();

    for i in 0..active.len() {
        for j in (i + 1)..active.len() {
            let (id_a, _, content_a, _, _) = &active[i];
            let (id_b, _, content_b, _, _) = &active[j];

            let content_a_lower = content_a.to_lowercase();
            let content_b_lower = content_b.to_lowercase();

            for (word_a, word_b) in CONTRADICTION_PAIRS {
                let conflict = (content_a_lower.contains(word_a) && content_b_lower.contains(word_b))
                    || (content_a_lower.contains(word_b) && content_b_lower.contains(word_a));

                if conflict {
                    contradictions.push(Contradiction {
                        symbol_id: symbol_id.to_string(),
                        annotation_a_id: *id_a,
                        annotation_b_id: *id_b,
                        reason: format!(
                            "Possible contradiction: '{}' vs '{}' (keywords: {} / {})",
                            truncate(content_a, 50),
                            truncate(content_b, 50),
                            word_a,
                            word_b,
                        ),
                    });
                    break; // One reason per pair is enough
                }
            }
        }
    }

    Ok(contradictions)
}

/// Scan all symbols for contradictions and surface as insights.
pub fn scan_all_contradictions(store: &Store) -> Result<usize> {
    let all_symbols = store.get_all_symbols()?;
    let mut total = 0;

    for sym in &all_symbols {
        let contradictions = detect_contradictions(store, &sym.id)?;
        for contradiction in &contradictions {
            store.create_insight(
                "contradiction",
                &contradiction.reason,
                &[contradiction.symbol_id.clone()],
            )?;
            total += 1;
        }
    }

    Ok(total)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::CodeParser;
    use std::path::Path;

    #[test]
    fn test_detect_contradiction() {
        let store = Store::open_in_memory().unwrap();
        store.initialize().unwrap();
        let parser = CodeParser::new();

        let source = "fn validate() -> bool { true }";
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        let sym = store.find_symbol_by_name("validate").unwrap();
        let sym_id = &sym[0].id;
        let hash = &sym[0].full_hash;

        // Two contradicting annotations
        store.create_annotation(sym_id, "explanation", "This function always returns true", hash).unwrap();
        store.create_annotation(sym_id, "warning", "This function sometimes returns false", hash).unwrap();

        let contradictions = detect_contradictions(&store, sym_id).unwrap();
        assert!(
            !contradictions.is_empty(),
            "should detect contradiction between 'always' and 'sometimes'"
        );
    }

    #[test]
    fn test_no_contradiction_for_compatible_annotations() {
        let store = Store::open_in_memory().unwrap();
        store.initialize().unwrap();
        let parser = CodeParser::new();

        let source = "fn process() {}";
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        let sym = store.find_symbol_by_name("process").unwrap();
        let sym_id = &sym[0].id;
        let hash = &sym[0].full_hash;

        store.create_annotation(sym_id, "explanation", "Processes input data", hash).unwrap();
        store.create_annotation(sym_id, "context", "Called by the main handler", hash).unwrap();

        let contradictions = detect_contradictions(&store, sym_id).unwrap();
        assert!(contradictions.is_empty(), "compatible annotations should not be flagged");
    }

    #[test]
    fn test_single_annotation_no_contradiction() {
        let store = Store::open_in_memory().unwrap();
        store.initialize().unwrap();
        let parser = CodeParser::new();

        let source = "fn lonely() {}";
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        let sym = store.find_symbol_by_name("lonely").unwrap();
        store.create_annotation(&sym[0].id, "explanation", "Does nothing", &sym[0].full_hash).unwrap();

        let contradictions = detect_contradictions(&store, &sym[0].id).unwrap();
        assert!(contradictions.is_empty());
    }
}
