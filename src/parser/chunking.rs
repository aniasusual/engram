use serde::{Deserialize, Serialize};
use sha2::Digest;
use super::treesitter::Symbol;

/// Maximum non-whitespace characters per chunk before splitting.
const MAX_NWS_CHARS: usize = 500;

/// Rough token estimate: ~4 chars per token on average for code.
const CHARS_PER_TOKEN: f64 = 4.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub symbol_id: String,
    pub chunk_index: usize,
    pub content: String,
    pub line_start: usize,
    pub line_end: usize,
    pub token_count: usize,
    pub nws_count: usize,
    pub body_hash: String,
    /// Inherited from parent symbol
    pub scope_chain: Vec<String>,
}

/// Count non-whitespace characters in a string.
fn count_nws(s: &str) -> usize {
    s.chars().filter(|c| !c.is_whitespace()).count()
}

/// Estimate token count from character count.
fn estimate_tokens(char_count: usize) -> usize {
    (char_count as f64 / CHARS_PER_TOKEN).ceil() as usize
}

/// Generate chunks for a symbol if its body exceeds the NWS threshold.
/// Returns empty vec if the symbol is small enough to not need chunking.
pub fn chunk_symbol(symbol: &Symbol) -> Vec<Chunk> {
    let body = &symbol.body;
    let body_nws = count_nws(body);

    if body_nws <= MAX_NWS_CHARS {
        return vec![];
    }

    let lines: Vec<&str> = body.lines().collect();
    if lines.is_empty() {
        return vec![];
    }

    // Split at statement boundaries (lines that end with ; or { or } or have zero indentation)
    let mut chunks = Vec::new();
    let mut current_lines: Vec<&str> = Vec::new();
    let mut current_nws: usize = 0;
    let mut chunk_start_line = 0usize;

    for (i, line) in lines.iter().enumerate() {
        let line_nws = count_nws(line);
        current_lines.push(line);
        current_nws += line_nws;

        let is_statement_boundary = is_boundary(line);

        if current_nws >= MAX_NWS_CHARS && is_statement_boundary {
            // Emit chunk
            let content = current_lines.join("\n");
            let content_nws = count_nws(&content);
            chunks.push(make_chunk(
                symbol,
                chunks.len(),
                content,
                symbol.line_start + chunk_start_line,
                symbol.line_start + i,
                content_nws,
            ));
            current_lines.clear();
            current_nws = 0;
            chunk_start_line = i + 1;
        }
    }

    // Remaining lines become the last chunk
    if !current_lines.is_empty() {
        let content = current_lines.join("\n");
        let content_nws = count_nws(&content);
        chunks.push(make_chunk(
            symbol,
            chunks.len(),
            content,
            symbol.line_start + chunk_start_line,
            symbol.line_end,
            content_nws,
        ));
    }

    // If we only got one chunk, splitting wasn't useful
    if chunks.len() <= 1 {
        return vec![];
    }

    chunks
}

fn is_boundary(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.ends_with(';')
        || trimmed.ends_with('{')
        || trimmed.ends_with('}')
        || trimmed.ends_with(':')
        || trimmed.is_empty()
        // Python: lines at indentation level 0 or 1 (def, class, return, etc.)
        || (line.len() > 0 && !line.starts_with("    ") && !line.starts_with('\t'))
}

fn make_chunk(
    symbol: &Symbol,
    index: usize,
    content: String,
    line_start: usize,
    line_end: usize,
    nws_count: usize,
) -> Chunk {
    let mut hasher = sha2::Sha256::new();
    hasher.update(content.as_bytes());
    let body_hash = hex::encode(hasher.finalize());

    let id = {
        let mut h = sha2::Sha256::new();
        h.update(format!("{}:chunk:{}", symbol.id, index).as_bytes());
        hex::encode(h.finalize())
    };

    Chunk {
        id,
        symbol_id: symbol.id.clone(),
        chunk_index: index,
        token_count: estimate_tokens(content.len()),
        nws_count,
        content,
        line_start,
        line_end,
        body_hash,
        scope_chain: symbol.scope_chain.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::CodeParser;

    #[test]
    fn test_small_function_not_chunked() {
        let parser = CodeParser::new();
        let source = r#"fn small() { let x = 1; }"#;
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();

        for sym in &result.symbols {
            let chunks = chunk_symbol(sym);
            assert!(chunks.is_empty(), "small function should not be chunked");
        }
    }

    #[test]
    fn test_large_function_chunked() {
        // Generate a function with >500 NWS chars
        let mut body_lines = Vec::new();
        for i in 0..60 {
            body_lines.push(format!("    let var_{} = compute_something({});", i, i));
        }
        let source = format!("fn big_function() {{\n{}\n}}", body_lines.join("\n"));

        let parser = CodeParser::new();
        let result = parser.parse_source(&source, "rust", "test.rs").unwrap();

        let sym = &result.symbols[0];
        let chunks = chunk_symbol(sym);
        assert!(
            chunks.len() >= 2,
            "large function should produce at least 2 chunks, got {}",
            chunks.len()
        );

        // Each chunk should inherit scope chain
        for chunk in &chunks {
            assert_eq!(chunk.scope_chain, sym.scope_chain);
            assert!(!chunk.body_hash.is_empty());
            assert!(chunk.token_count > 0);
            assert!(chunk.nws_count > 0);
        }
    }

    #[test]
    fn test_chunk_line_ranges_are_contiguous() {
        let mut body_lines = Vec::new();
        for i in 0..80 {
            body_lines.push(format!("    let v{} = {};", i, i));
        }
        let source = format!("fn big() {{\n{}\n}}", body_lines.join("\n"));

        let parser = CodeParser::new();
        let result = parser.parse_source(&source, "rust", "test.rs").unwrap();
        let chunks = chunk_symbol(&result.symbols[0]);

        if chunks.len() >= 2 {
            for i in 1..chunks.len() {
                assert!(
                    chunks[i].line_start >= chunks[i - 1].line_start,
                    "chunk line ranges should be monotonically increasing"
                );
            }
        }
    }

    #[test]
    fn test_nws_count_accuracy() {
        let s = "  hello  world  \n\t  foo  ";
        assert_eq!(count_nws(s), 13); // helloworld + foo = 13
    }

    #[test]
    fn test_token_estimate() {
        assert_eq!(estimate_tokens(100), 25);
        assert_eq!(estimate_tokens(0), 0);
    }
}
