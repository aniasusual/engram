//! Markdown/documentation parser.
//!
//! Extracts headings and sections from .md files as searchable symbols.
//! These are stored alongside code symbols so agents can search docs and code together.

use crate::parser::treesitter::{ParseResult, Symbol, SymbolKind};
use sha2::{Digest, Sha256};

/// Parse a markdown file into sections (headings become symbols).
pub fn parse_markdown(content: &str, file_path: &str) -> ParseResult {
    let mut symbols = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    let mut current_heading: Option<(String, usize, usize)> = None; // (name, level, start_line)

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Detect ATX headings: # Heading, ## Heading, etc.
        if let Some(heading) = parse_heading(trimmed) {
            // Close previous heading
            if let Some((name, _level, start)) = current_heading.take() {
                let body = lines[start..i].join("\n");
                symbols.push(make_doc_symbol(&name, file_path, start + 1, i, &body));
            }
            current_heading = Some((heading.1, heading.0, i));
        }
    }

    // Close last heading
    if let Some((name, _level, start)) = current_heading {
        let body = lines[start..].join("\n");
        symbols.push(make_doc_symbol(
            &name,
            file_path,
            start + 1,
            lines.len(),
            &body,
        ));
    }

    // If no headings found, treat the whole file as one document section
    if symbols.is_empty() && !content.trim().is_empty() {
        let name = std::path::Path::new(file_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "document".to_string());
        symbols.push(make_doc_symbol(&name, file_path, 1, lines.len(), content));
    }

    ParseResult {
        symbols,
        edges: Vec::new(),
        language: "markdown".to_string(),
    }
}

/// Parse ATX heading: returns (level, title) or None.
fn parse_heading(line: &str) -> Option<(usize, String)> {
    if !line.starts_with('#') {
        return None;
    }
    let level = line.chars().take_while(|c| *c == '#').count();
    if level > 6 || level == 0 {
        return None;
    }
    let title = line[level..].trim().to_string();
    if title.is_empty() {
        return None;
    }
    Some((level, title))
}

fn sha256_hex(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

fn make_doc_symbol(
    name: &str,
    file: &str,
    line_start: usize,
    line_end: usize,
    body: &str,
) -> Symbol {
    let body_hash = sha256_hex(body);
    let full_hash = sha256_hex(&format!("{}:{}", name, body));

    Symbol {
        id: sha256_hex(&format!("{}:{}:doc", file, name)),
        canonical_id: sha256_hex(&format!("{}:doc:{}", name, body_hash)),
        name: name.to_string(),
        kind: SymbolKind::Module, // Use Module kind for doc sections
        file: file.to_string(),
        line_start,
        line_end,
        signature: format!("# {}", name),
        docstring: Some(body.chars().take(500).collect()), // First 500 chars as docstring
        body: body.to_string(),
        body_hash,
        full_hash,
        language: "markdown".to_string(),
        scope_chain: vec![name.to_string()],
        parent_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_headings() {
        let content = "# Title\n\nSome intro text.\n\n## Section One\n\nContent here.\n\n## Section Two\n\nMore content.\n";
        let result = parse_markdown(content, "README.md");

        let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Title"), "should find Title: {:?}", names);
        assert!(
            names.contains(&"Section One"),
            "should find Section One: {:?}",
            names
        );
        assert!(
            names.contains(&"Section Two"),
            "should find Section Two: {:?}",
            names
        );
    }

    #[test]
    fn test_parse_markdown_no_headings() {
        let content = "Just some plain text without any headings.\nAnother line.\n";
        let result = parse_markdown(content, "notes.md");

        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "notes");
    }

    #[test]
    fn test_parse_markdown_empty() {
        let result = parse_markdown("", "empty.md");
        assert!(result.symbols.is_empty());
    }

    #[test]
    fn test_parse_markdown_nested_headings() {
        let content = "# Top\n\n## Sub\n\n### Deep\n\nContent.\n";
        let result = parse_markdown(content, "doc.md");

        assert_eq!(result.symbols.len(), 3);
    }

    #[test]
    fn test_doc_symbol_has_searchable_content() {
        let content = "# API Reference\n\nThe authentication endpoint accepts JWT tokens.\nUse POST /api/auth with Bearer header.\n";
        let result = parse_markdown(content, "api.md");

        let sym = &result.symbols[0];
        assert!(
            sym.docstring.as_ref().unwrap().contains("JWT tokens"),
            "docstring should contain section content for FTS indexing"
        );
        assert_eq!(sym.language, "markdown");
    }
}
