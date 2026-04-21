use rmcp::{
    ServerHandler,
    model::{ServerCapabilities, ServerInfo},
    schemars, tool,
};
use std::path::PathBuf;
use std::sync::Arc;

use crate::graph::{Store, SymbolRow};

/// Engram MCP Server — exposes codebase intelligence tools.
#[derive(Debug, Clone)]
pub struct EngramMcp {
    pub store: Arc<Store>,
    root: PathBuf,
}

// ── Tool parameter types ──────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetSymbolParams {
    #[schemars(description = "Symbol name to look up (exact or fuzzy)")]
    pub name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct FindCallersParams {
    #[schemars(description = "Symbol name to find callers for")]
    pub symbol: String,
    #[schemars(description = "Maximum BFS depth (default 3)")]
    pub depth: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct FindDependenciesParams {
    #[schemars(description = "Symbol name to find dependencies for")]
    pub symbol: String,
    #[schemars(description = "Maximum BFS depth (default 3)")]
    pub depth: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchSemanticParams {
    #[schemars(description = "Natural language search query")]
    pub query: String,
    #[schemars(description = "Maximum number of results (default 10)")]
    pub top_k: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetFileSummaryParams {
    #[schemars(description = "Relative file path")]
    pub file: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AnnotateSymbolParams {
    #[schemars(description = "Symbol name to annotate")]
    pub symbol: String,
    #[schemars(description = "Annotation type: explanation, warning, todo, context, assumption")]
    pub annotation_type: String,
    #[schemars(description = "Annotation content")]
    pub content: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AnnotationIdParams {
    #[schemars(description = "Annotation ID")]
    pub annotation_id: i64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RecordDecisionParams {
    #[schemars(description = "Comma-separated symbol names related to this decision")]
    pub symbols: String,
    #[schemars(description = "Description of the decision")]
    pub description: String,
    #[schemars(description = "Rationale for the decision")]
    pub rationale: Option<String>,
    #[schemars(description = "Alternatives that were considered and rejected")]
    pub alternatives: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RecordPatternParams {
    #[schemars(description = "Pattern name")]
    pub name: String,
    #[schemars(description = "Pattern description")]
    pub description: String,
    #[schemars(description = "Comma-separated symbol names that exhibit this pattern")]
    pub symbols: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct InsightIdParams {
    #[schemars(description = "Insight ID")]
    pub insight_id: i64,
    #[schemars(description = "New status: resolved or dismissed")]
    pub status: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RiskScoreParams {
    #[schemars(description = "Symbol name to compute risk for")]
    pub symbol: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetContextParams {
    #[schemars(description = "Symbol name")]
    pub symbol: String,
    #[schemars(description = "Detail tier: brief, standard, full, or deep (default: standard)")]
    pub tier: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetContextBatchParams {
    #[schemars(description = "Comma-separated symbol names")]
    pub symbols: String,
    #[schemars(description = "Maximum token budget")]
    pub budget: usize,
}

// ── Tool implementations ──────────────────────────────────────────────────

#[tool(tool_box)]
impl EngramMcp {
    #[tool(
        description = "Look up a symbol by name. Returns symbol details including kind, file, signature, scope chain, and docstring."
    )]
    fn get_symbol(&self, #[tool(aggr)] params: GetSymbolParams) -> String {
        match self.store.find_symbol_by_name(&params.name) {
            Ok(symbols) if symbols.is_empty() => {
                // Try BM25 fuzzy search as fallback
                match self.store.search_bm25(&params.name, 5) {
                    Ok(results) if !results.is_empty() => {
                        let mut output =
                            format!("No exact match for '{}'. Similar symbols:\n", params.name);
                        for (id, _score) in &results {
                            if let Ok(Some(sym)) = self.store.get_symbol(id) {
                                output.push_str(&format!(
                                    "  {} {} ({}:{}) — {}\n",
                                    sym.kind, sym.name, sym.file, sym.line_start, sym.signature
                                ));
                            }
                        }
                        output
                    }
                    _ => format!("No symbol found matching '{}'", params.name),
                }
            }
            Ok(symbols) => {
                self.track(
                    &symbols.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
                    "view",
                );
                let mut output = String::new();
                for sym in &symbols {
                    let scope: Vec<String> =
                        serde_json::from_str(&sym.scope_chain).unwrap_or_default();
                    output.push_str(&format!(
                        "{} {} ({}:{}–{})\n  signature: {}\n  scope: {}\n  docstring: {}\n  hash: {}\n\n",
                        sym.kind, sym.name, sym.file, sym.line_start, sym.line_end,
                        sym.signature,
                        scope.join(" > "),
                        sym.docstring.as_deref().unwrap_or("(none)"),
                        &sym.full_hash[..12],
                    ));
                }
                output
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Find all callers of a symbol via reverse call graph traversal (BFS).")]
    fn find_callers(&self, #[tool(aggr)] params: FindCallersParams) -> String {
        let depth = params.depth.unwrap_or(3);
        let symbols = match self.store.find_symbol_by_name(&params.symbol) {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };
        if symbols.is_empty() {
            return format!("Symbol '{}' not found", params.symbol);
        }

        let mut output = String::new();
        for sym in &symbols {
            match self.store.find_callers(&sym.id, depth) {
                Ok(callers) if callers.is_empty() => {
                    output.push_str(&format!("No callers found for {}\n", sym.name));
                }
                Ok(callers) => {
                    output.push_str(&format!("Callers of {} (depth {}):\n", sym.name, depth));
                    for (caller_id, dist) in &callers {
                        if let Ok(Some(caller)) = self.store.get_symbol(caller_id) {
                            output.push_str(&format!(
                                "  [depth {}] {} {} ({}:{})\n",
                                dist, caller.kind, caller.name, caller.file, caller.line_start
                            ));
                        }
                    }
                }
                Err(e) => output.push_str(&format!("Error: {}\n", e)),
            }
        }
        output
    }

    #[tool(description = "Find all dependencies of a symbol via forward call graph traversal.")]
    fn find_dependencies(&self, #[tool(aggr)] params: FindDependenciesParams) -> String {
        let depth = params.depth.unwrap_or(3);
        let symbols = match self.store.find_symbol_by_name(&params.symbol) {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };
        if symbols.is_empty() {
            return format!("Symbol '{}' not found", params.symbol);
        }

        let mut output = String::new();
        for sym in &symbols {
            match self.store.find_dependencies(&sym.id, depth) {
                Ok(deps) if deps.is_empty() => {
                    output.push_str(&format!("No dependencies found for {}\n", sym.name));
                }
                Ok(deps) => {
                    output.push_str(&format!(
                        "Dependencies of {} (depth {}):\n",
                        sym.name, depth
                    ));
                    for (dep_id, dist) in &deps {
                        if let Ok(Some(dep)) = self.store.get_symbol(dep_id) {
                            output.push_str(&format!(
                                "  [depth {}] {} {} ({}:{})\n",
                                dist, dep.kind, dep.name, dep.file, dep.line_start
                            ));
                        }
                    }
                }
                Err(e) => output.push_str(&format!("Error: {}\n", e)),
            }
        }
        output
    }

    #[tool(
        description = "Search the codebase using hybrid RRF search (vector + BM25 + graph proximity). Falls back to BM25 if no embeddings are computed yet."
    )]
    fn search_semantic(&self, #[tool(aggr)] params: SearchSemanticParams) -> String {
        let top_k = params.top_k.unwrap_or(10);

        // Try hybrid search if embeddings exist
        let has_embeddings = self
            .store
            .get_all_embeddings()
            .map(|e| !e.is_empty())
            .unwrap_or(false);

        if has_embeddings && let Ok(engine) = crate::embeddings::EmbeddingEngine::new() {
            let mut index = crate::embeddings::VectorIndex::new();
            if index.load_from_store(&self.store).is_ok() && !index.is_empty() {
                match crate::embeddings::search_hybrid(
                    &params.query,
                    &self.store,
                    &engine,
                    &index,
                    top_k,
                ) {
                    Ok(results) if results.is_empty() => {
                        return format!("No results found for '{}'", params.query);
                    }
                    Ok(results) => {
                        self.track(
                            &results
                                .iter()
                                .map(|r| r.symbol_id.as_str())
                                .collect::<Vec<_>>(),
                            "query",
                        );
                        let mut output = format!(
                            "Hybrid search results for '{}' (RRF: vector + BM25 + graph):\n",
                            params.query
                        );
                        for result in &results {
                            if let Ok(Some(sym)) = self.store.get_symbol(&result.symbol_id) {
                                output.push_str(&format!(
                                    "  [{:.4}] {} {} ({}:{}) — {}\n",
                                    result.score,
                                    sym.kind,
                                    sym.name,
                                    sym.file,
                                    sym.line_start,
                                    sym.signature
                                ));
                            }
                        }
                        return output;
                    }
                    Err(_) => {} // Fall through to BM25
                }
            }
        }

        // Fallback: BM25 only
        match self.store.search_bm25(&params.query, top_k) {
            Ok(results) if results.is_empty() => {
                format!("No results found for '{}'", params.query)
            }
            Ok(results) => {
                let mut output = format!("Search results for '{}' (BM25):\n", params.query);
                for (id, score) in &results {
                    if let Ok(Some(sym)) = self.store.get_symbol(id) {
                        output.push_str(&format!(
                            "  [{:.3}] {} {} ({}:{}) — {}\n",
                            -score, sym.kind, sym.name, sym.file, sym.line_start, sym.signature
                        ));
                    }
                }
                output
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Get a summary of symbols in a file: functions, classes, complexity, and imports."
    )]
    fn get_file_summary(&self, #[tool(aggr)] params: GetFileSummaryParams) -> String {
        match self.store.get_file_symbols(&params.file) {
            Ok(symbols) if symbols.is_empty() => {
                format!("No symbols found in '{}'", params.file)
            }
            Ok(symbols) => {
                let mut output = format!("File: {} ({} symbols)\n", params.file, symbols.len());
                for sym in &symbols {
                    output.push_str(&format!(
                        "  {}:{} {} {} — {}\n",
                        sym.line_start, sym.line_end, sym.kind, sym.name, sym.signature
                    ));
                }
                output
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Get a ~100-token project identity summary: language breakdown, module map, symbol count, recent activity."
    )]
    fn get_project_identity(&self) -> String {
        let stats = match self.store.stats() {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };

        let lang_breakdown = self.store.language_breakdown().unwrap_or_default();

        let mut output = format!(
            "Project: {}\nSymbols: {} | Edges: {} | Files: {}\n",
            self.root.file_name().unwrap_or_default().to_string_lossy(),
            stats.symbol_count,
            stats.edge_count,
            stats.file_count,
        );

        if !lang_breakdown.is_empty() {
            output.push_str("Languages: ");
            let parts: Vec<String> = lang_breakdown
                .iter()
                .map(|(lang, count)| format!("{} ({})", lang, count))
                .collect();
            output.push_str(&parts.join(", "));
            output.push('\n');
        }

        output
    }

    #[tool(
        description = "Write a persistent annotation on a symbol. The annotation is hash-anchored — it will be marked stale if the symbol changes."
    )]
    fn annotate_symbol(&self, #[tool(aggr)] params: AnnotateSymbolParams) -> String {
        let symbols = match self.store.find_symbol_by_name(&params.symbol) {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };
        if symbols.is_empty() {
            return format!("Symbol '{}' not found", params.symbol);
        }

        let sym = &symbols[0];
        self.track(&[&sym.id], "annotate");
        match self.store.create_annotation(
            &sym.id,
            &params.annotation_type,
            &params.content,
            &sym.full_hash,
        ) {
            Ok(id) => format!(
                "Annotation #{} created on {} (anchored to hash {})",
                id,
                sym.name,
                &sym.full_hash[..12]
            ),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Confirm an annotation is still correct. Resets its confidence to 1.0.")]
    fn verify_annotation(&self, #[tool(aggr)] params: AnnotationIdParams) -> String {
        match self.store.verify_annotation(params.annotation_id) {
            Ok(()) => format!(
                "Annotation #{} verified (confidence → 1.0)",
                params.annotation_id
            ),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Reduce confidence of an annotation. Use when an annotation seems wrong or outdated."
    )]
    fn downvote_annotation(&self, #[tool(aggr)] params: AnnotationIdParams) -> String {
        match self.store.downvote_annotation(params.annotation_id) {
            Ok(()) => format!(
                "Annotation #{} downvoted (confidence reduced)",
                params.annotation_id
            ),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Record an architectural decision with rationale and rejected alternatives."
    )]
    fn record_decision(&self, #[tool(aggr)] params: RecordDecisionParams) -> String {
        let symbol_names: Vec<String> = params
            .symbols
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let mut symbol_ids = Vec::new();
        for name in &symbol_names {
            if let Ok(syms) = self.store.find_symbol_by_name(name) {
                for s in &syms {
                    symbol_ids.push(s.id.clone());
                }
            }
        }

        match self.store.record_decision(
            &symbol_ids,
            &params.description,
            params.rationale.as_deref(),
            params.alternatives.as_deref(),
        ) {
            Ok(id) => format!("Decision #{} recorded for symbols: {}", id, params.symbols),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Record a recurring pattern observed in the codebase.")]
    fn record_pattern(&self, #[tool(aggr)] params: RecordPatternParams) -> String {
        let symbol_ids: Vec<String> = params
            .symbols
            .as_deref()
            .unwrap_or("")
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .flat_map(|name| {
                self.store
                    .find_symbol_by_name(name.trim())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|s| s.id)
            })
            .collect();

        match self
            .store
            .record_pattern(&params.name, &params.description, &symbol_ids)
        {
            Ok(id) => format!("Pattern #{} '{}' recorded", id, params.name),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Get auto-generated insights: god nodes, complexity spikes, dead code, missing docs."
    )]
    fn get_insights(&self) -> String {
        // Generate fresh insights
        let _ = self.generate_insights();

        match self.store.get_insights() {
            Ok(insights) if insights.is_empty() => "No active insights.".to_string(),
            Ok(insights) => {
                let mut output = format!("{} active insights:\n", insights.len());
                for (id, itype, content, _sym_ids, confidence, _status) in &insights {
                    output.push_str(&format!(
                        "  #{} [{}] ({:.0}%) {}\n",
                        id,
                        itype,
                        confidence * 100.0,
                        content
                    ));
                }
                output
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Mark an insight as resolved or dismissed.")]
    fn resolve_insight(&self, #[tool(aggr)] params: InsightIdParams) -> String {
        let status = if params.status == "resolved" || params.status == "dismissed" {
            &params.status
        } else {
            return "Invalid status. Use 'resolved' or 'dismissed'.".to_string();
        };
        match self.store.resolve_insight(params.insight_id, status) {
            Ok(()) => format!("Insight #{} marked as {}", params.insight_id, status),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Get risk score for a symbol: (1+complexity) × (1+callers) × (1+stale_annotations)."
    )]
    fn get_risk_score(&self, #[tool(aggr)] params: RiskScoreParams) -> String {
        let symbols = match self.store.find_symbol_by_name(&params.symbol) {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };
        if symbols.is_empty() {
            return format!("Symbol '{}' not found", params.symbol);
        }

        let mut output = String::new();
        for sym in &symbols {
            match self.store.get_risk_score(&sym.id) {
                Ok(score) => {
                    output.push_str(&format!(
                        "{} {} ({}:{}) — risk score: {:.1}\n",
                        sym.kind, sym.name, sym.file, sym.line_start, score
                    ));
                }
                Err(e) => output.push_str(&format!("Error for {}: {}\n", sym.name, e)),
            }
        }
        output
    }

    #[tool(
        description = "Get a codebase architecture report: language breakdown, top symbols by callers, module structure."
    )]
    fn get_codebase_report(&self) -> String {
        let stats = match self.store.stats() {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };

        let mut output = format!(
            "=== Codebase Report ===\nSymbols: {} | Edges: {} | Files: {}\n\n",
            stats.symbol_count, stats.edge_count, stats.file_count
        );

        // Language breakdown
        if let Ok(langs) = self.store.language_breakdown() {
            output.push_str("Languages:\n");
            for (lang, count) in &langs {
                output.push_str(&format!("  {} — {} symbols\n", lang, count));
            }
            output.push('\n');
        }

        // Top symbols by caller count (god nodes)
        if let Ok(symbols) = self.store.get_all_symbols() {
            let mut sym_callers: Vec<(&SymbolRow, usize)> = symbols
                .iter()
                .filter_map(|s| {
                    self.store
                        .get_direct_callers(&s.id)
                        .ok()
                        .map(|callers| (s, callers.len()))
                })
                .filter(|(_, count)| *count > 0)
                .collect();
            sym_callers.sort_by(|a, b| b.1.cmp(&a.1));

            if !sym_callers.is_empty() {
                output.push_str("Most-called symbols (god nodes):\n");
                for (sym, count) in sym_callers.iter().take(10) {
                    output.push_str(&format!(
                        "  {} callers — {} {} ({})\n",
                        count, sym.kind, sym.name, sym.file
                    ));
                }
                output.push('\n');
            }
        }

        // Stale annotations
        if let Ok(symbols) = self.store.get_all_symbols() {
            let stale: Vec<_> = symbols
                .iter()
                .filter_map(|s| {
                    self.store
                        .get_annotations(&s.id)
                        .ok()
                        .map(|anns| (s, anns.iter().filter(|a| a.4 == "stale").count()))
                })
                .filter(|(_, count)| *count > 0)
                .collect();

            if !stale.is_empty() {
                output.push_str("Symbols with stale annotations:\n");
                for (sym, count) in &stale {
                    output.push_str(&format!(
                        "  {} stale — {} {} ({})\n",
                        count, sym.kind, sym.name, sym.file
                    ));
                }
            }
        }

        output
    }

    #[tool(
        description = "Get symbol context at a specific detail level. Tiers: brief (~80 tokens), standard (~500), full (~2500), deep (~5000)."
    )]
    fn get_context(&self, #[tool(aggr)] params: GetContextParams) -> String {
        let tier = params.tier.as_deref().unwrap_or("standard");
        let symbols = match self.store.find_symbol_by_name(&params.symbol) {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };
        if symbols.is_empty() {
            return format!("Symbol '{}' not found", params.symbol);
        }
        let sym = &symbols[0];
        self.format_context(sym, tier)
    }

    #[tool(
        description = "Get context for multiple symbols within a token budget. Greedy packing: starts brief, upgrades highest-attention first."
    )]
    fn get_context_batch(&self, #[tool(aggr)] params: GetContextBatchParams) -> String {
        let names: Vec<&str> = params.symbols.split(',').map(|s| s.trim()).collect();
        let mut all_syms = Vec::new();
        for name in &names {
            if let Ok(syms) = self.store.find_symbol_by_name(name)
                && let Some(sym) = syms.into_iter().next()
            {
                all_syms.push(sym);
            }
        }

        if all_syms.is_empty() {
            return "No symbols found.".to_string();
        }

        // Start with brief for all, then upgrade greedily
        let mut output = String::new();
        let mut budget_remaining = params.budget;

        // Brief pass (~80 tokens each)
        for sym in &all_syms {
            let brief = self.format_context(sym, "brief");
            let est_tokens = brief.len() / 4;
            if est_tokens <= budget_remaining {
                output.push_str(&brief);
                output.push_str("---\n");
                budget_remaining = budget_remaining.saturating_sub(est_tokens);
            }
        }

        // Upgrade pass: add standard details for symbols that fit
        for sym in &all_syms {
            if budget_remaining < 100 {
                break;
            }
            let standard = self.format_context(sym, "standard");
            let est_tokens = standard.len() / 4;
            if est_tokens <= budget_remaining {
                output.push_str(&format!("\n[Upgraded: {}]\n", sym.name));
                output.push_str(&standard);
                budget_remaining = budget_remaining.saturating_sub(est_tokens);
            }
        }

        output
    }

    #[tool(
        description = "Get exploration coverage: which symbols have been accessed and which are blind spots (high-caller symbols never explored)."
    )]
    fn get_exploration_map(&self) -> String {
        match self.store.get_exploration_map() {
            Ok((explored, blind_spots)) => {
                let mut output = String::new();
                if explored.is_empty() {
                    output.push_str("No symbols explored yet.\n");
                } else {
                    output.push_str(&format!("Explored ({} symbols):\n", explored.len()));
                    for (id, score) in explored.iter().take(15) {
                        if let Ok(Some(sym)) = self.store.get_symbol(id) {
                            output.push_str(&format!(
                                "  [{:.0}] {} {} ({})\n",
                                score, sym.kind, sym.name, sym.file
                            ));
                        }
                    }
                }
                if !blind_spots.is_empty() {
                    output.push_str(&format!(
                        "\nBlind spots ({} high-caller symbols never explored):\n",
                        blind_spots.len()
                    ));
                    for id in &blind_spots {
                        if let Ok(Some(sym)) = self.store.get_symbol(id) {
                            let callers = self
                                .store
                                .get_direct_callers(id)
                                .map(|c| c.len())
                                .unwrap_or(0);
                            output.push_str(&format!(
                                "  {} {} ({}) — {} callers\n",
                                sym.kind, sym.name, sym.file, callers
                            ));
                        }
                    }
                }
                output
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Suggest next symbols to explore based on call graph centrality and unexplored areas."
    )]
    fn suggest_next(&self) -> String {
        match self.store.suggest_next(10) {
            Ok(suggestions) if suggestions.is_empty() => {
                "All symbols explored! No suggestions.".to_string()
            }
            Ok(suggestions) => {
                let mut output = "Suggested next symbols to explore:\n".to_string();
                for (id, name) in &suggestions {
                    if let Ok(Some(sym)) = self.store.get_symbol(id) {
                        let callers = self
                            .store
                            .get_direct_callers(id)
                            .map(|c| c.len())
                            .unwrap_or(0);
                        output.push_str(&format!(
                            "  {} {} ({}) — {} callers\n",
                            sym.kind, name, sym.file, callers
                        ));
                    }
                }
                output
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Get git blame for a file: who wrote each line, when, and in which commit."
    )]
    fn get_blame(&self, #[tool(aggr)] params: GetFileSummaryParams) -> String {
        let repo = match crate::git::open_repo(&self.root) {
            Ok(r) => r,
            Err(e) => return format!("Git error: {}", e),
        };
        match crate::git::blame_file(&repo, &params.file) {
            Ok(lines) if lines.is_empty() => format!("No blame data for '{}'", params.file),
            Ok(lines) => {
                let mut output = format!("Blame for {}:\n", params.file);
                for line in lines.iter().take(50) {
                    output.push_str(&format!(
                        "  L{}: {} ({}) {}\n",
                        line.line,
                        line.author,
                        &line.commit_hash[..7.min(line.commit_hash.len())],
                        line.summary
                    ));
                }
                if lines.len() > 50 {
                    output.push_str(&format!("  ... and {} more lines\n", lines.len() - 50));
                }
                output
            }
            Err(e) => format!("Blame error: {}", e),
        }
    }

    #[tool(
        description = "Get file ownership: who has contributed the most to a file based on git blame."
    )]
    fn get_ownership(&self, #[tool(aggr)] params: GetFileSummaryParams) -> String {
        match self.store.get_file_ownership(&params.file) {
            Ok(owners) if owners.is_empty() => {
                // Try computing from blame directly
                let repo = match crate::git::open_repo(&self.root) {
                    Ok(r) => r,
                    Err(_) => return format!("No ownership data for '{}'", params.file),
                };
                match crate::git::blame_file(&repo, &params.file) {
                    Ok(lines) => {
                        let ownership = crate::git::compute_ownership(&lines);
                        let mut output = format!("Ownership of {} (from blame):\n", params.file);
                        for entry in &ownership {
                            output.push_str(&format!(
                                "  {} ({}) — {} lines\n",
                                entry.author, entry.email, entry.commits
                            ));
                        }
                        output
                    }
                    Err(e) => format!("Error: {}", e),
                }
            }
            Ok(owners) => {
                let mut output = format!("Ownership of {}:\n", params.file);
                for (author, email, commits, _ts) in &owners {
                    output.push_str(&format!(
                        "  {} ({}) — {} contributions\n",
                        author, email, commits
                    ));
                }
                output
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Get git commit history for a symbol's file. Shows recent changes relevant to the symbol."
    )]
    fn get_symbol_history(&self, #[tool(aggr)] params: GetSymbolParams) -> String {
        let symbols = match self.store.find_symbol_by_name(&params.name) {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };
        if symbols.is_empty() {
            return format!("Symbol '{}' not found", params.name);
        }
        let sym = &symbols[0];

        let repo = match crate::git::open_repo(&self.root) {
            Ok(r) => r,
            Err(e) => return format!("Git error: {}", e),
        };

        match crate::git::get_file_commits(&repo, &sym.file, 20) {
            Ok(commits) if commits.is_empty() => {
                format!("No git history found for {} ({})", sym.name, sym.file)
            }
            Ok(commits) => {
                let mut output = format!("Git history for {} ({}):\n", sym.name, sym.file);
                for c in &commits {
                    let date = chrono::DateTime::from_timestamp(c.timestamp, 0)
                        .map(|d| d.format("%Y-%m-%d").to_string())
                        .unwrap_or_default();
                    output.push_str(&format!(
                        "  {} {} — {} ({})\n",
                        &c.hash[..7],
                        date,
                        c.message,
                        c.author
                    ));
                }
                output
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Get symbol evolution timeline: how a symbol has changed over time, with annotation history."
    )]
    fn get_evolution(&self, #[tool(aggr)] params: GetSymbolParams) -> String {
        let symbols = match self.store.find_symbol_by_name(&params.name) {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };
        if symbols.is_empty() {
            return format!("Symbol '{}' not found", params.name);
        }
        let sym = &symbols[0];

        match self.store.get_symbol_evolution(&sym.id) {
            Ok(events) if events.is_empty() => {
                format!(
                    "No evolution history for {} (symbol may not have changed since tracking started)",
                    sym.name
                )
            }
            Ok(events) => {
                let mut output = format!("Evolution of {} {}:\n", sym.kind, sym.name);
                for (change_type, timestamp, summary, _old, _new) in &events {
                    let date = chrono::DateTime::from_timestamp(*timestamp, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_default();
                    output.push_str(&format!("  {} [{}] {}\n", date, change_type, summary));
                }

                // Also show annotations
                if let Ok(anns) = self.store.get_annotations(&sym.id)
                    && !anns.is_empty()
                {
                    output.push_str("\nAnnotations:\n");
                    for (id, atype, content, conf, status) in &anns {
                        output.push_str(&format!(
                            "  #{} [{}] ({:.0}%, {}) {}\n",
                            id,
                            atype,
                            conf * 100.0,
                            status,
                            content
                        ));
                    }
                }
                output
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Get module-level context: symbols, health, community membership, and annotations for a directory."
    )]
    fn get_module_context(&self, #[tool(aggr)] params: GetFileSummaryParams) -> String {
        // Get all symbols in files matching this path prefix
        let all_symbols = match self.store.get_all_symbols() {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };

        let matching: Vec<_> = all_symbols
            .iter()
            .filter(|s| s.file.starts_with(&params.file) || s.file.contains(&params.file))
            .collect();

        if matching.is_empty() {
            return format!("No symbols found in module '{}'", params.file);
        }

        let mut output = format!("Module '{}' ({} symbols):\n", params.file, matching.len());

        // Symbols grouped by file
        let mut by_file: std::collections::HashMap<&str, Vec<&SymbolRow>> =
            std::collections::HashMap::new();
        for sym in &matching {
            by_file.entry(&sym.file).or_default().push(sym);
        }

        for (file, syms) in &by_file {
            output.push_str(&format!("\n  {}:\n", file));
            for sym in syms {
                let risk = self.store.get_risk_score(&sym.id).unwrap_or(1.0);
                output.push_str(&format!(
                    "    {} {} (L{}) risk={:.0}\n",
                    sym.kind, sym.name, sym.line_start, risk
                ));
            }
        }

        // Community info if available
        if let Ok(communities) = crate::intelligence::community::detect_communities(&self.store) {
            let relevant: Vec<_> = communities
                .iter()
                .filter(|c| {
                    c.symbol_ids
                        .iter()
                        .any(|id| matching.iter().any(|s| s.id == *id))
                })
                .collect();
            if !relevant.is_empty() {
                output.push_str("\nCommunities:\n");
                for comm in &relevant {
                    output.push_str(&format!(
                        "  #{} '{}' ({} symbols, cohesion={:.2})\n",
                        comm.id,
                        comm.label,
                        comm.symbol_ids.len(),
                        comm.cohesion
                    ));
                }
            }
        }

        output
    }

    #[tool(
        description = "Find structurally and semantically similar code. Detects exact clones (same body) and near-clones (similar structure)."
    )]
    fn find_similar(&self, #[tool(aggr)] params: GetSymbolParams) -> String {
        let symbols = match self.store.find_symbol_by_name(&params.name) {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };
        if symbols.is_empty() {
            return format!("Symbol '{}' not found", params.name);
        }
        let sym = &symbols[0];

        match crate::intelligence::similarity::find_similar_to(&self.store, &sym.id, 10) {
            Ok(clones) if clones.is_empty() => {
                format!("No similar code found for {}", sym.name)
            }
            Ok(clones) => {
                let mut output = format!("Similar to {} {}:\n", sym.kind, sym.name);
                for clone in &clones {
                    if let Ok(Some(other)) = self.store.get_symbol(&clone.symbol_b_id) {
                        let kind = match &clone.clone_type {
                            crate::intelligence::similarity::CloneType::Exact => "EXACT",
                            crate::intelligence::similarity::CloneType::Near => "NEAR",
                        };
                        output.push_str(&format!(
                            "  [{} {:.0}%] {} {} ({}:{})\n",
                            kind,
                            clone.similarity * 100.0,
                            other.kind,
                            other.name,
                            other.file,
                            other.line_start
                        ));
                    }
                }
                output
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Analyze execution hot paths: which symbols are called most frequently and have the highest risk."
    )]
    fn get_hot_path(&self) -> String {
        let all_symbols = match self.store.get_all_symbols() {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };

        let mut scored: Vec<(String, String, String, f64)> = all_symbols
            .iter()
            .filter_map(|sym| {
                let callers = self.store.get_direct_callers(&sym.id).ok()?;
                let risk = self.store.get_risk_score(&sym.id).ok()?;
                if callers.is_empty() {
                    return None;
                }
                Some((
                    sym.name.clone(),
                    sym.kind.clone(),
                    sym.file.clone(),
                    risk * callers.len() as f64,
                ))
            })
            .collect();

        scored.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(15);

        if scored.is_empty() {
            return "No hot paths detected (no call edges found).".to_string();
        }

        let mut output = "Hot paths (highest risk × caller count):\n".to_string();
        for (name, kind, file, score) in &scored {
            output.push_str(&format!("  [{:.0}] {} {} ({})\n", score, kind, name, file));
        }
        output
    }

    #[tool(
        description = "Get runtime trace data. Currently shows static call graph analysis. Runtime tracers (Python/Node.js/Go) planned for future."
    )]
    fn get_trace(&self, #[tool(aggr)] params: GetSymbolParams) -> String {
        let symbols = match self.store.find_symbol_by_name(&params.name) {
            Ok(s) => s,
            Err(e) => return format!("Error: {}", e),
        };
        if symbols.is_empty() {
            return format!("Symbol '{}' not found", params.name);
        }
        let sym = &symbols[0];

        let mut output = format!("Trace for {} {} ({}):\n", sym.kind, sym.name, sym.file);

        // Static call chain: who calls this, and what does this call
        if let Ok(callers) = self.store.find_callers(&sym.id, 2) {
            output.push_str("  Called by:\n");
            for (id, depth) in &callers {
                if let Ok(Some(s)) = self.store.get_symbol(id) {
                    output.push_str(&format!(
                        "    [depth {}] {} {} ({})\n",
                        depth, s.kind, s.name, s.file
                    ));
                }
            }
        }
        if let Ok(deps) = self.store.find_dependencies(&sym.id, 2) {
            output.push_str("  Calls:\n");
            for (id, depth) in &deps {
                if let Ok(Some(s)) = self.store.get_symbol(id) {
                    output.push_str(&format!(
                        "    [depth {}] {} {} ({})\n",
                        depth, s.kind, s.name, s.file
                    ));
                }
            }
        }

        output
    }

    #[tool(description = "Get topic-clustered symbols: which symbols belong to which topics.")]
    fn get_topic_summary(&self) -> String {
        match self.store.get_all_topics() {
            Ok(topics) if topics.is_empty() => {
                "No topics defined yet. Use annotate_symbol to add context.".to_string()
            }
            Ok(topics) => {
                let mut output = format!("{} topics:\n", topics.len());
                for (topic, count) in &topics {
                    output.push_str(&format!("  {} ({} symbols)\n", topic, count));
                }
                output
            }
            Err(e) => format!("Error: {}", e),
        }
    }
}

#[tool(tool_box)]
impl ServerHandler for EngramMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Engram — codebase intelligence daemon. Provides deep structural knowledge: \
                 symbol lookup, call graph traversal, semantic search, file summaries, \
                 and persistent hash-anchored annotations."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

impl EngramMcp {
    pub fn new(store: Arc<Store>, root: PathBuf) -> Self {
        Self { store, root }
    }

    /// Record that symbols were accessed (for attention tracking / exploration map).
    fn track(&self, symbol_ids: &[&str], event: &str) {
        for id in symbol_ids {
            let _ = self.store.record_attention(id, event);
        }
    }

    fn generate_insights(&self) -> anyhow::Result<()> {
        let symbols = self.store.get_all_symbols()?;

        for sym in &symbols {
            // God node detection: >10 callers
            let callers = self.store.get_direct_callers(&sym.id)?;
            if callers.len() > 10 {
                let _ = self.store.create_insight(
                    "god_node",
                    &format!(
                        "{} {} has {} callers — high coupling risk",
                        sym.kind,
                        sym.name,
                        callers.len()
                    ),
                    std::slice::from_ref(&sym.id),
                );
            }
        }
        Ok(())
    }

    /// Public accessor for format_context (used by tests).
    pub fn format_context_pub(&self, sym: &crate::graph::SymbolRow, tier: &str) -> String {
        self.format_context(sym, tier)
    }

    fn format_context(&self, sym: &crate::graph::SymbolRow, tier: &str) -> String {
        let scope: Vec<String> = serde_json::from_str(&sym.scope_chain).unwrap_or_default();
        match tier {
            "brief" => {
                // ~80 tokens: name, kind, file, signature, risk, warnings
                let risk = self.store.get_risk_score(&sym.id).unwrap_or(1.0);
                let stale = self
                    .store
                    .get_annotations(&sym.id)
                    .map(|a| a.iter().filter(|x| x.4 == "stale").count())
                    .unwrap_or(0);
                let mut out = format!(
                    "{} {} ({}:{}) risk={:.0}",
                    sym.kind, sym.name, sym.file, sym.line_start, risk
                );
                if stale > 0 {
                    out.push_str(&format!(" [{} stale annotations]", stale));
                }
                out.push('\n');
                out
            }
            "standard" => {
                // ~500 tokens: + callers, deps, annotations, scope
                let mut out = format!(
                    "{} {} ({}:{}–{})\n  scope: {}\n  signature: {}\n",
                    sym.kind,
                    sym.name,
                    sym.file,
                    sym.line_start,
                    sym.line_end,
                    scope.join(" > "),
                    sym.signature,
                );
                if let Some(ref doc) = sym.docstring {
                    out.push_str(&format!("  docstring: {}\n", doc));
                }
                // Top 5 callers
                if let Ok(callers) = self.store.find_callers(&sym.id, 1)
                    && !callers.is_empty()
                {
                    out.push_str("  callers: ");
                    let names: Vec<String> = callers
                        .iter()
                        .take(5)
                        .filter_map(|(id, _)| {
                            self.store.get_symbol(id).ok().flatten().map(|s| s.name)
                        })
                        .collect();
                    out.push_str(&names.join(", "));
                    out.push('\n');
                }
                // Annotations
                if let Ok(anns) = self.store.get_annotations(&sym.id) {
                    for (id, atype, content, conf, status) in &anns {
                        out.push_str(&format!(
                            "  annotation #{} [{}] ({:.0}%, {}) {}\n",
                            id,
                            atype,
                            conf * 100.0,
                            status,
                            content
                        ));
                    }
                }
                out
            }
            "full" => {
                // Full: everything standard + all callers/deps
                let mut out = self.format_context(sym, "standard");
                if let Ok(callers) = self.store.find_callers(&sym.id, 3)
                    && !callers.is_empty()
                {
                    out.push_str("  full caller graph:\n");
                    for (id, depth) in &callers {
                        if let Ok(Some(s)) = self.store.get_symbol(id) {
                            out.push_str(&format!("    [depth {}] {} {}\n", depth, s.kind, s.name));
                        }
                    }
                }
                if let Ok(deps) = self.store.find_dependencies(&sym.id, 3)
                    && !deps.is_empty()
                {
                    out.push_str("  dependencies:\n");
                    for (id, depth) in &deps {
                        if let Ok(Some(s)) = self.store.get_symbol(id) {
                            out.push_str(&format!("    [depth {}] {} {}\n", depth, s.kind, s.name));
                        }
                    }
                }
                out
            }
            "deep" => {
                // Deep: everything from full + actual source code with inline annotations
                let mut out = self.format_context(sym, "full");

                // Read the actual source file and extract the symbol's lines
                let file_path = self.root.join(&sym.file);
                match std::fs::read_to_string(&file_path) {
                    Ok(content) => {
                        let lines: Vec<&str> = content.lines().collect();
                        let start = sym.line_start.saturating_sub(1); // 1-indexed to 0-indexed
                        let end = sym.line_end.min(lines.len());

                        if start < lines.len() {
                            out.push_str("\n  ── source code ──\n");
                            for (i, line) in lines[start..end].iter().enumerate() {
                                let line_num = start + i + 1;

                                // Check if any annotation targets this line range
                                out.push_str(&format!("  {:>4} | {}\n", line_num, line));
                            }

                            // Append inline annotations after the code
                            if let Ok(anns) = self.store.get_annotations(&sym.id) {
                                let active: Vec<_> = anns
                                    .iter()
                                    .filter(|(_, _, _, _, status)| {
                                        status == "active" || status == "stale"
                                    })
                                    .collect();
                                if !active.is_empty() {
                                    out.push_str("  ── annotations ──\n");
                                    for (id, atype, content, conf, status) in &active {
                                        let marker = if *status == "stale" {
                                            "STALE"
                                        } else {
                                            "active"
                                        };
                                        out.push_str(&format!(
                                            "  #{} [{}] ({:.0}%, {}) {}\n",
                                            id,
                                            atype,
                                            conf * 100.0,
                                            marker,
                                            content
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        out.push_str(&format!("\n  (source file not found: {})\n", sym.file));
                    }
                }
                out
            }
            _ => self.format_context(sym, "standard"),
        }
    }
}

/// Start the MCP server on stdio.
pub async fn run_mcp_server(store: Arc<Store>, root: PathBuf) -> anyhow::Result<()> {
    let server = EngramMcp::new(store, root);
    let transport = rmcp::transport::stdio();

    tracing::info!("Starting Engram MCP server (stdio)");
    let service = rmcp::serve_server(server, transport).await?;
    service.waiting().await?;
    Ok(())
}
