use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor};

use super::languages;

// ── Types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Trait,
    Interface,
    Module,
    Import,
    TypeAlias,
    Constant,
    Variable,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Interface => "interface",
            Self::Module => "module",
            Self::Import => "import",
            Self::TypeAlias => "type_alias",
            Self::Constant => "constant",
            Self::Variable => "variable",
        }
    }
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EdgeKind {
    Calls,
    Imports,
    Inherits,
    Implements,
    Uses,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Calls => "CALLS",
            Self::Imports => "IMPORTS",
            Self::Inherits => "INHERITS",
            Self::Implements => "IMPLEMENTS",
            Self::Uses => "USES",
        }
    }
}

impl std::fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: String,
    pub canonical_id: String,
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub line_start: usize,
    pub line_end: usize,
    pub signature: String,
    pub docstring: Option<String>,
    pub body: String,
    pub body_hash: String,
    pub full_hash: String,
    pub language: String,
    pub scope_chain: Vec<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from_id: String,
    pub to_id: String,
    pub kind: EdgeKind,
    pub file: String,
    pub line: Option<usize>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub symbols: Vec<Symbol>,
    pub edges: Vec<Edge>,
    pub language: String,
}

// ── Hashing ───────────────────────────────────────────────────────────────

fn sha256_hex(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

fn make_symbol_id(file: &str, name: &str, kind: &SymbolKind) -> String {
    sha256_hex(&format!("{}:{}:{}", file, name, kind.as_str()))
}

fn make_canonical_id(name: &str, kind: &SymbolKind, body_hash: &str) -> String {
    sha256_hex(&format!("{}:{}:{}", name, kind.as_str(), body_hash))
}

fn make_full_hash(signature: &str, body: &str, docstring: Option<&str>) -> String {
    sha256_hex(&format!(
        "{}:{}:{}",
        signature,
        body,
        docstring.unwrap_or("")
    ))
}

// ── Parser ────────────────────────────────────────────────────────────────

pub struct CodeParser;

impl Default for CodeParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_file(&self, path: &Path) -> Result<ParseResult> {
        let language_name = super::detect_language(path)
            .with_context(|| format!("unsupported file type: {}", path.display()))?;

        let source =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

        let file_str = path.to_string_lossy().to_string();

        // Markdown files use a separate parser (not tree-sitter)
        if language_name == "markdown" {
            return Ok(super::markdown::parse_markdown(&source, &file_str));
        }

        self.parse_source(&source, language_name, &file_str)
    }

    pub fn parse_source(
        &self,
        source: &str,
        language_name: &str,
        file_path: &str,
    ) -> Result<ParseResult> {
        let ts_language = languages::get_language(language_name)
            .with_context(|| format!("no tree-sitter grammar for {}", language_name))?;

        // tree-sitter Parser is internally mutable via &self methods in 0.24+
        // but we need to work around the API — create a new parser per call
        let mut parser = Parser::new();
        parser
            .set_language(&ts_language)
            .context("setting parser language")?;

        let tree = parser
            .parse(source, None)
            .context("tree-sitter parse failed")?;

        let query_source = languages::get_symbol_query(language_name)
            .with_context(|| format!("no query patterns for {}", language_name))?;

        let query = Query::new(&ts_language, query_source)
            .map_err(|e| anyhow::anyhow!("query compilation error: {:?}", e))?;

        let mut raw_symbols = Vec::new();
        let mut edges = Vec::new();

        // Execute query to find symbols
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            if let Some(sym) =
                self.extract_symbol_from_match(&query, m, source, file_path, language_name)
            {
                raw_symbols.push(sym);
            }
        }

        // Compute parent_id and scope_chain by line-range containment
        self.compute_scope_hierarchy(&mut raw_symbols);

        // Extract call edges within file
        self.extract_call_edges(source, &tree, &raw_symbols, file_path, &mut edges);

        // Extract inheritance/implementation edges
        self.extract_inheritance_edges(
            &query,
            &mut cursor,
            &tree,
            source,
            &raw_symbols,
            file_path,
            language_name,
            &mut edges,
        );

        Ok(ParseResult {
            symbols: raw_symbols,
            edges,
            language: language_name.to_string(),
        })
    }

    fn extract_symbol_from_match(
        &self,
        query: &Query,
        m: &tree_sitter::QueryMatch,
        source: &str,
        file_path: &str,
        language: &str,
    ) -> Option<Symbol> {
        let capture_names: Vec<_> = m
            .captures
            .iter()
            .map(|c| (query.capture_names()[c.index as usize], c.node))
            .collect();

        // Determine symbol kind from capture names
        let (kind, name_capture, body_capture, def_capture) =
            self.classify_captures(&capture_names)?;

        // Skip imports for symbol table (we track them as edges)
        if matches!(kind, SymbolKind::Import) {
            return None;
        }

        let name = name_capture.utf8_text(source.as_bytes()).ok()?.to_string();
        let def_node = def_capture;

        let line_start = def_node.start_position().row + 1;
        let line_end = def_node.end_position().row + 1;

        let body = if let Some(body_node) = body_capture {
            body_node.utf8_text(source.as_bytes()).ok()?.to_string()
        } else {
            def_node.utf8_text(source.as_bytes()).ok()?.to_string()
        };

        let signature = self.build_signature(&def_node, &body, source);
        let docstring = self.extract_docstring(&def_node, source, language);

        let body_hash = sha256_hex(&body);
        let full_hash = make_full_hash(&signature, &body, docstring.as_deref());

        Some(Symbol {
            id: make_symbol_id(file_path, &name, &kind),
            canonical_id: make_canonical_id(&name, &kind, &body_hash),
            name,
            kind,
            file: file_path.to_string(),
            line_start,
            line_end,
            signature,
            docstring,
            body_hash,
            full_hash,
            body,
            language: language.to_string(),
            scope_chain: vec![],
            parent_id: None,
        })
    }

    fn classify_captures<'a>(
        &self,
        captures: &[(&str, tree_sitter::Node<'a>)],
    ) -> Option<(
        SymbolKind,
        tree_sitter::Node<'a>,
        Option<tree_sitter::Node<'a>>,
        tree_sitter::Node<'a>,
    )> {
        // Find the .def capture (the whole match) and the .name capture
        let def = captures.iter().find(|(name, _)| name.ends_with(".def"));
        let (def_prefix, def_node) = def?;
        let prefix = def_prefix.strip_suffix(".def")?;

        let name_node = captures
            .iter()
            .find(|(name, _)| *name == format!("{}.name", prefix))
            .map(|(_, n)| *n)?;

        let body_node = captures
            .iter()
            .find(|(name, _)| *name == format!("{}.body", prefix))
            .map(|(_, n)| *n);

        let kind = match prefix {
            "fn" => SymbolKind::Function,
            "method" => SymbolKind::Method,
            "class" => SymbolKind::Class,
            "struct" => SymbolKind::Struct,
            "enum" => SymbolKind::Enum,
            "trait" => SymbolKind::Trait,
            "interface" => SymbolKind::Interface,
            "mod" | "package" => SymbolKind::Module,
            "import" => SymbolKind::Import,
            "type_alias" | "type" => SymbolKind::TypeAlias,
            "const" | "static" => SymbolKind::Constant,
            "var" => SymbolKind::Variable,
            "decorated" => {
                // Python decorated functions/classes — skip, the inner def/class will be caught
                return None;
            }
            "impl" => {
                // Rust impl blocks — we don't create a symbol for impl itself,
                // methods inside will be caught separately
                return None;
            }
            "export" => {
                // Export wrappers — skip, inner declaration caught separately
                return None;
            }
            _ => return None,
        };

        Some((kind, name_node, body_node, *def_node))
    }

    fn build_signature(&self, def_node: &tree_sitter::Node, body: &str, source: &str) -> String {
        let full_text = def_node
            .utf8_text(source.as_bytes())
            .unwrap_or_default()
            .to_string();

        // Signature = full text minus body
        if !body.is_empty()
            && let Some(idx) = full_text.find(body)
        {
            let sig = full_text[..idx].trim_end();
            if !sig.is_empty() {
                return sig.to_string();
            }
        }
        // Fallback: first line
        full_text.lines().next().unwrap_or_default().to_string()
    }

    fn extract_docstring(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        language: &str,
    ) -> Option<String> {
        match language {
            "rust" => {
                // Look for line_comment or block_comment siblings before this node
                let mut doc_lines = Vec::new();
                let mut sibling = node.prev_sibling();
                while let Some(s) = sibling {
                    let kind = s.kind();
                    if kind == "line_comment" || kind == "block_comment" {
                        let text = s.utf8_text(source.as_bytes()).ok()?;
                        if text.starts_with("///") || text.starts_with("//!") {
                            doc_lines.push(
                                text.trim_start_matches("///")
                                    .trim_start_matches("//!")
                                    .trim()
                                    .to_string(),
                            );
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                    sibling = s.prev_sibling();
                }
                doc_lines.reverse();
                if doc_lines.is_empty() {
                    None
                } else {
                    Some(doc_lines.join("\n"))
                }
            }
            "python" => {
                // First child of body that is expression_statement > string
                let body = node.child_by_field_name("body")?;
                let first_stmt = body.child(0)?;
                if first_stmt.kind() == "expression_statement" {
                    let expr = first_stmt.child(0)?;
                    if expr.kind() == "string" || expr.kind() == "concatenated_string" {
                        let text = expr.utf8_text(source.as_bytes()).ok()?;
                        // Strip triple quotes
                        let trimmed = text
                            .trim_start_matches("\"\"\"")
                            .trim_start_matches("'''")
                            .trim_end_matches("\"\"\"")
                            .trim_end_matches("'''")
                            .trim()
                            .to_string();
                        return Some(trimmed);
                    }
                }
                None
            }
            "typescript" | "javascript" => {
                // JSDoc comment before node
                if let Some(s) = node.prev_sibling() {
                    if s.kind() == "comment" {
                        let text = s.utf8_text(source.as_bytes()).ok()?;
                        if text.starts_with("/**") {
                            return Some(
                                text.trim_start_matches("/**")
                                    .trim_end_matches("*/")
                                    .lines()
                                    .map(|l| l.trim().trim_start_matches('*').trim())
                                    .collect::<Vec<_>>()
                                    .join("\n")
                                    .trim()
                                    .to_string(),
                            );
                        }
                    }
                }
                None
            }
            "go" => {
                // Go doc comments
                let mut doc_lines = Vec::new();
                let mut sibling = node.prev_sibling();
                while let Some(s) = sibling {
                    if s.kind() == "comment" {
                        let text = s.utf8_text(source.as_bytes()).ok()?;
                        if text.starts_with("//") {
                            doc_lines.push(text.trim_start_matches("//").trim().to_string());
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                    sibling = s.prev_sibling();
                }
                doc_lines.reverse();
                if doc_lines.is_empty() {
                    None
                } else {
                    Some(doc_lines.join("\n"))
                }
            }
            _ => None,
        }
    }

    fn compute_scope_hierarchy(&self, symbols: &mut [Symbol]) {
        // Sort by line range (outer first for containment check)
        let ids_and_ranges: Vec<(String, usize, usize, String)> = symbols
            .iter()
            .map(|s| (s.id.clone(), s.line_start, s.line_end, s.name.clone()))
            .collect();

        for sym in symbols.iter_mut() {
            // Find the innermost containing symbol (smallest range that fully contains this one)
            let mut best_parent: Option<(String, usize)> = None; // (id, range_size)

            for (pid, pstart, pend, _pname) in &ids_and_ranges {
                if *pid == sym.id {
                    continue;
                }
                if *pstart <= sym.line_start && *pend >= sym.line_end {
                    let range_size = pend - pstart;
                    if best_parent
                        .as_ref()
                        .is_none_or(|(_, best_size)| range_size < *best_size)
                    {
                        best_parent = Some((pid.clone(), range_size));
                    }
                }
            }

            sym.parent_id = best_parent.map(|(id, _)| id);
        }

        // Build scope chains by walking parent_id upward
        let parent_map: std::collections::HashMap<String, (Option<String>, String)> = symbols
            .iter()
            .map(|s| (s.id.clone(), (s.parent_id.clone(), s.name.clone())))
            .collect();

        for sym in symbols.iter_mut() {
            let mut chain = vec![sym.name.clone()];
            let mut current_id = sym.parent_id.clone();
            let mut visited = std::collections::HashSet::new();
            while let Some(ref pid) = current_id {
                if !visited.insert(pid.clone()) {
                    break; // cycle guard
                }
                if let Some((next_parent, name)) = parent_map.get(pid) {
                    chain.push(name.clone());
                    current_id = next_parent.clone();
                } else {
                    break;
                }
            }
            chain.reverse();
            sym.scope_chain = chain;
        }
    }

    fn extract_call_edges(
        &self,
        source: &str,
        tree: &tree_sitter::Tree,
        symbols: &[Symbol],
        file_path: &str,
        edges: &mut Vec<Edge>,
    ) {
        // Build a map of function names → symbol IDs in this file
        let name_to_id: std::collections::HashMap<&str, &str> = symbols
            .iter()
            .filter(|s| {
                matches!(
                    s.kind,
                    SymbolKind::Function | SymbolKind::Method | SymbolKind::Class
                )
            })
            .map(|s| (s.name.as_str(), s.id.as_str()))
            .collect();

        // For each function/method, walk its body looking for call_expression nodes
        for sym in symbols
            .iter()
            .filter(|s| matches!(s.kind, SymbolKind::Function | SymbolKind::Method))
        {
            // Find call expressions by walking the AST within the symbol's byte range
            let root = tree.root_node();
            self.find_calls_in_range(
                root,
                source,
                sym.line_start,
                sym.line_end,
                &sym.id,
                &name_to_id,
                file_path,
                edges,
            );
        }
    }

    fn find_calls_in_range(
        &self,
        node: tree_sitter::Node,
        source: &str,
        line_start: usize,
        line_end: usize,
        caller_id: &str,
        name_to_id: &std::collections::HashMap<&str, &str>,
        file_path: &str,
        edges: &mut Vec<Edge>,
    ) {
        let node_line = node.start_position().row + 1;
        let node_end_line = node.end_position().row + 1;

        // Only process nodes within the symbol's range
        if node_end_line < line_start || node_line > line_end {
            return;
        }

        if node.kind() == "call_expression" || node.kind() == "call" {
            // Extract the function name being called
            if let Some(func_node) = node.child_by_field_name("function") {
                let callee_name = match func_node.kind() {
                    "identifier" | "field_identifier" => {
                        func_node.utf8_text(source.as_bytes()).ok()
                    }
                    "member_expression" | "field_expression" | "selector_expression" => {
                        // Get the last identifier (method name)
                        let mut child = func_node.child(func_node.child_count().saturating_sub(1));
                        while let Some(c) = child {
                            if c.kind() == "identifier"
                                || c.kind() == "field_identifier"
                                || c.kind() == "property_identifier"
                            {
                                break;
                            }
                            child = c.prev_sibling();
                        }
                        child.and_then(|c| c.utf8_text(source.as_bytes()).ok())
                    }
                    _ => func_node.utf8_text(source.as_bytes()).ok(),
                };

                if let Some(callee) = callee_name
                    && let Some(callee_id) = name_to_id.get(callee)
                    && *callee_id != caller_id
                {
                    let edge = Edge {
                        from_id: caller_id.to_string(),
                        to_id: callee_id.to_string(),
                        kind: EdgeKind::Calls,
                        file: file_path.to_string(),
                        line: Some(node_line),
                        confidence: 1.0,
                    };
                    // Dedup edges
                    if !edges.iter().any(|e| {
                        e.from_id == edge.from_id
                            && e.to_id == edge.to_id
                            && e.kind.as_str() == edge.kind.as_str()
                    }) {
                        edges.push(edge);
                    }
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                self.find_calls_in_range(
                    cursor.node(),
                    source,
                    line_start,
                    line_end,
                    caller_id,
                    name_to_id,
                    file_path,
                    edges,
                );
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    fn extract_inheritance_edges(
        &self,
        _query: &Query,
        _cursor: &mut QueryCursor,
        tree: &tree_sitter::Tree,
        source: &str,
        symbols: &[Symbol],
        file_path: &str,
        language: &str,
        edges: &mut Vec<Edge>,
    ) {
        let name_to_id: std::collections::HashMap<&str, &str> = symbols
            .iter()
            .map(|s| (s.name.as_str(), s.id.as_str()))
            .collect();

        // Walk the tree looking for inheritance patterns
        self.walk_for_inheritance(
            tree.root_node(),
            source,
            &name_to_id,
            file_path,
            language,
            edges,
        );
    }

    fn walk_for_inheritance(
        &self,
        node: tree_sitter::Node,
        source: &str,
        name_to_id: &std::collections::HashMap<&str, &str>,
        file_path: &str,
        language: &str,
        edges: &mut Vec<Edge>,
    ) {
        match (language, node.kind()) {
            // Python: class Foo(Bar, Baz)
            ("python", "class_definition") => {
                let class_name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok());
                if let Some(bases) = node.child_by_field_name("superclasses")
                    && let Some(class_name) = class_name
                    && let Some(class_id) = name_to_id.get(class_name)
                {
                    let mut cursor = bases.walk();
                    if cursor.goto_first_child() {
                        loop {
                            let child = cursor.node();
                            if child.kind() == "identifier"
                                && let Ok(base_name) = child.utf8_text(source.as_bytes())
                                && let Some(base_id) = name_to_id.get(base_name)
                            {
                                edges.push(Edge {
                                    from_id: class_id.to_string(),
                                    to_id: base_id.to_string(),
                                    kind: EdgeKind::Inherits,
                                    file: file_path.to_string(),
                                    line: Some(node.start_position().row + 1),
                                    confidence: 1.0,
                                });
                            }
                            if !cursor.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
            }
            // Rust: impl Trait for Type
            ("rust", "impl_item") => {
                let trait_node = node.child_by_field_name("trait");
                let type_node = node.child_by_field_name("type");
                if let (Some(trait_n), Some(type_n)) = (trait_node, type_node) {
                    let trait_name = trait_n.utf8_text(source.as_bytes()).ok();
                    let type_name = type_n.utf8_text(source.as_bytes()).ok();
                    if let (Some(tn), Some(typ)) = (trait_name, type_name)
                        && let (Some(type_id), Some(trait_id)) =
                            (name_to_id.get(typ), name_to_id.get(tn))
                    {
                        edges.push(Edge {
                            from_id: type_id.to_string(),
                            to_id: trait_id.to_string(),
                            kind: EdgeKind::Implements,
                            file: file_path.to_string(),
                            line: Some(node.start_position().row + 1),
                            confidence: 1.0,
                        });
                    }
                }
            }
            _ => {}
        }

        // Recurse
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                self.walk_for_inheritance(
                    cursor.node(),
                    source,
                    name_to_id,
                    file_path,
                    language,
                    edges,
                );
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rust_function() {
        let parser = CodeParser::new();
        let source = r#"
/// Validates the JWT token
fn validate_token(token: &str) -> bool {
    token.len() > 0
}
"#;
        let result = parser
            .parse_source(source, "rust", "test.rs")
            .expect("parse failed");

        assert_eq!(result.symbols.len(), 1);
        let sym = &result.symbols[0];
        assert_eq!(sym.name, "validate_token");
        assert!(matches!(sym.kind, SymbolKind::Function));
        assert_eq!(sym.language, "rust");
        assert!(sym.docstring.as_deref() == Some("Validates the JWT token"));
    }

    #[test]
    fn test_parse_rust_struct_and_impl() {
        let parser = CodeParser::new();
        let source = r#"
struct AuthService {
    secret: String,
}

impl AuthService {
    fn validate(&self, token: &str) -> bool {
        token.len() > 0
    }
}
"#;
        let result = parser
            .parse_source(source, "rust", "auth.rs")
            .expect("parse failed");

        let struct_sym = result.symbols.iter().find(|s| s.name == "AuthService");
        assert!(struct_sym.is_some());
        assert!(matches!(struct_sym.unwrap().kind, SymbolKind::Struct));

        let method_sym = result.symbols.iter().find(|s| s.name == "validate");
        assert!(method_sym.is_some());
    }

    #[test]
    fn test_parse_python_class() {
        let parser = CodeParser::new();
        let source = r#"
class AuthService:
    """Handles authentication"""

    def validate_token(self, token: str) -> bool:
        """Check if token is valid"""
        return len(token) > 0
"#;
        let result = parser
            .parse_source(source, "python", "auth.py")
            .expect("parse failed");

        let class = result.symbols.iter().find(|s| s.name == "AuthService");
        assert!(class.is_some());
        assert!(matches!(class.unwrap().kind, SymbolKind::Class));

        let method = result.symbols.iter().find(|s| s.name == "validate_token");
        assert!(method.is_some());
    }

    #[test]
    fn test_scope_chain() {
        let parser = CodeParser::new();
        let source = r#"
class AuthService:
    def validate_token(self, token):
        return True
"#;
        let result = parser
            .parse_source(source, "python", "auth.py")
            .expect("parse failed");

        let method = result
            .symbols
            .iter()
            .find(|s| s.name == "validate_token")
            .expect("method not found");

        assert_eq!(method.scope_chain, vec!["AuthService", "validate_token"]);
        assert!(method.parent_id.is_some());
    }

    #[test]
    fn test_call_edges() {
        let parser = CodeParser::new();
        let source = r#"
fn helper() -> bool {
    true
}

fn main_func() {
    let result = helper();
}
"#;
        let result = parser
            .parse_source(source, "rust", "test.rs")
            .expect("parse failed");

        assert!(
            result
                .edges
                .iter()
                .any(|e| { e.kind.as_str() == "CALLS" && e.confidence == 1.0 }),
            "should find a CALLS edge"
        );
    }

    #[test]
    fn test_canonical_id_stability() {
        let parser = CodeParser::new();
        let source = r#"fn greet() { println!("hello"); }"#;

        let r1 = parser.parse_source(source, "rust", "a.rs").expect("parse");
        let r2 = parser.parse_source(source, "rust", "b.rs").expect("parse");

        // Same code in different files → different id, same canonical_id
        assert_ne!(r1.symbols[0].id, r2.symbols[0].id);
        assert_eq!(r1.symbols[0].canonical_id, r2.symbols[0].canonical_id);
    }
}
