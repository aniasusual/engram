use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "engram",
    about = "Persistent codebase intelligence for coding agents"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize Engram in the current repository
    Init {
        /// Root directory of the project (defaults to current dir)
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Start the Engram daemon (background indexing + file watching)
    Start {
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Stop the Engram daemon
    Stop,
    /// Show indexing status
    Status {
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Search the codebase
    Search {
        /// Search query
        query: String,
        /// Maximum number of results
        #[arg(long, default_value = "10")]
        top_k: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Start MCP server (stdio transport)
    Mcp {
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Get brief context for a file (used by Claude Code hooks)
    ContextForFile {
        /// File path (relative to root)
        #[arg(long)]
        file: String,
        /// Brief mode (one line) or full mode
        #[arg(long, default_value = "true")]
        brief: bool,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Install Engram as a Claude Code skill
    InstallSkill {
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Install Claude Code hooks
    InstallHooks {
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Command::Init { root } => {
                let root = std::fs::canonicalize(&root)?;
                let engram_dir = root.join(".engram");
                std::fs::create_dir_all(&engram_dir)?;

                let db_path = engram_dir.join("engram.db");
                let store = crate::graph::Store::open(&db_path)?;
                store.initialize()?;

                tracing::info!("Initialized Engram at {}", engram_dir.display());
                println!("Engram initialized at {}", engram_dir.display());
                Ok(())
            }
            Command::Start { root } => {
                let root = std::fs::canonicalize(&root)?;
                let db_path = root.join(".engram/engram.db");
                if !db_path.exists() {
                    anyhow::bail!("Engram not initialized. Run `engram init` first.");
                }
                let store = std::sync::Arc::new(crate::graph::Store::open(&db_path)?);

                // Initial full index
                println!("Indexing {}...", root.display());
                let parser = crate::parser::CodeParser::new();
                let files = crate::parser::discover_files(&root)?;
                let mut total = 0usize;
                for file in &files {
                    let relative = file.strip_prefix(&root).unwrap_or(file);
                    let result = parser.parse_file(file)?;
                    store.sync_file(relative, &result)?;
                    total += result.symbols.len();
                }
                println!("Indexed {} files, {} symbols", files.len(), total);

                // Compute embeddings for symbols that need them
                println!("Computing embeddings...");
                match compute_embeddings(&store) {
                    Ok(count) => {
                        if count > 0 {
                            println!("Embedded {} symbols", count);
                        } else {
                            println!("All embeddings up to date");
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Embedding computation failed (non-fatal): {}", e);
                        println!(
                            "Skipping embeddings (model download may be required on first run)"
                        );
                    }
                }

                // Sync git data if in a git repo
                match crate::git::sync_git_data(&store, &root) {
                    Ok(()) => println!("Git data synced"),
                    Err(_) => {} // Not a git repo or no commits — fine
                }

                // Start file watcher
                println!("Watching for changes... (Ctrl+C to stop)");
                crate::watcher::watch_and_sync(root, store).await?;
                Ok(())
            }
            Command::Stop => {
                // Check for a PID file and signal the daemon
                let pid_file = std::path::PathBuf::from(".engram/engram.pid");
                if pid_file.exists() {
                    let pid_str = std::fs::read_to_string(&pid_file)?;
                    if let Ok(pid) = pid_str.trim().parse::<u32>() {
                        // Send SIGTERM on Unix
                        #[cfg(unix)]
                        {
                            unsafe {
                                libc::kill(pid as i32, libc::SIGTERM);
                            }
                        }
                        // Terminate process on Windows
                        #[cfg(windows)]
                        {
                            let _ = std::process::Command::new("taskkill")
                                .args(["/PID", &pid.to_string(), "/F"])
                                .output();
                        }
                        std::fs::remove_file(&pid_file)?;
                        println!("Stopped Engram daemon (PID {})", pid);
                    } else {
                        std::fs::remove_file(&pid_file)?;
                        println!("Removed stale PID file");
                    }
                } else {
                    println!("No running Engram daemon found.");
                }
                Ok(())
            }
            Command::Status { root } => {
                let root = std::fs::canonicalize(&root)?;
                let db_path = root.join(".engram/engram.db");
                if !db_path.exists() {
                    println!("Engram not initialized in this directory.");
                    return Ok(());
                }
                let store = crate::graph::Store::open(&db_path)?;
                let stats = store.stats()?;
                println!("Engram status:");
                println!("  Symbols: {}", stats.symbol_count);
                println!("  Edges:   {}", stats.edge_count);
                println!("  Files:   {}", stats.file_count);
                Ok(())
            }
            Command::Search { query, top_k, root } => {
                let root = std::fs::canonicalize(&root)?;
                let db_path = root.join(".engram/engram.db");
                if !db_path.exists() {
                    anyhow::bail!("Engram not initialized. Run `engram init` first.");
                }
                let store = crate::graph::Store::open(&db_path)?;

                // Try hybrid search (vector + BM25 + graph via RRF)
                let has_embeddings = store
                    .get_all_embeddings()
                    .map(|e| !e.is_empty())
                    .unwrap_or(false);

                if has_embeddings {
                    // Full hybrid RRF search
                    match crate::embeddings::EmbeddingEngine::new() {
                        Ok(engine) => {
                            let mut index = crate::embeddings::VectorIndex::new();
                            index.load_from_store(&store)?;

                            let results = crate::embeddings::search_hybrid(
                                &query, &store, &engine, &index, top_k,
                            )?;

                            if results.is_empty() {
                                println!("No results found for '{}'", query);
                            } else {
                                println!("Hybrid search (RRF: vector + BM25 + graph):");
                                for result in &results {
                                    if let Some(sym) = store.get_symbol(&result.symbol_id)? {
                                        println!(
                                            "  {:.4}  {} {} ({}:{})",
                                            result.score,
                                            sym.kind,
                                            sym.name,
                                            sym.file,
                                            sym.line_start
                                        );
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            // Fall back to BM25 if embedding model fails to load
                            run_bm25_search(&store, &query, top_k)?;
                        }
                    }
                } else {
                    // BM25 only (no embeddings computed yet)
                    run_bm25_search(&store, &query, top_k)?;
                }
                Ok(())
            }
            Command::Mcp { root } => {
                let root = std::fs::canonicalize(&root)?;
                let db_path = root.join(".engram/engram.db");
                if !db_path.exists() {
                    anyhow::bail!("Engram not initialized. Run `engram init` first.");
                }
                let store = std::sync::Arc::new(crate::graph::Store::open(&db_path)?);
                crate::mcp::run_mcp_server(store, root).await?;
                Ok(())
            }
            Command::ContextForFile {
                file,
                brief: _,
                root,
            } => {
                let root = std::fs::canonicalize(&root)?;
                let db_path = root.join(".engram/engram.db");
                if !db_path.exists() {
                    // Silent — hooks should not crash the agent
                    return Ok(());
                }
                let store = crate::graph::Store::open(&db_path)?;

                // Find symbols in this file (try exact match, then suffix match)
                let mut symbols = store.get_file_symbols(&file)?;
                if symbols.is_empty() {
                    // Try matching by filename suffix (handles absolute vs relative paths)
                    let all = store.get_all_symbols()?;
                    symbols = all
                        .into_iter()
                        .filter(|s| s.file.ends_with(&file) || file.ends_with(&s.file))
                        .collect();
                }
                if symbols.is_empty() {
                    return Ok(());
                }

                // Build a one-line summary per file
                let total_callers: usize = symbols
                    .iter()
                    .filter_map(|s| store.get_direct_callers(&s.id).ok())
                    .map(|c| c.len())
                    .sum();

                let max_risk: f64 = symbols
                    .iter()
                    .filter_map(|s| store.get_risk_score(&s.id).ok())
                    .fold(0.0f64, f64::max);

                let stale_count: usize = symbols
                    .iter()
                    .filter_map(|s| store.get_annotations(&s.id).ok())
                    .flat_map(|anns| anns.into_iter())
                    .filter(|(_, _, _, _, status)| status == "stale")
                    .count();

                let top_symbols: Vec<String> = symbols
                    .iter()
                    .take(3)
                    .map(|s| format!("{} {}", s.kind, s.name))
                    .collect();

                let mut line = format!(
                    "[engram] {} — {} symbols, {} callers, risk={:.0}",
                    file,
                    symbols.len(),
                    total_callers,
                    max_risk
                );

                if stale_count > 0 {
                    line.push_str(&format!(", {} stale annotations", stale_count));
                }

                line.push_str(&format!(" | top: {}", top_symbols.join(", ")));

                println!("{}", line);
                Ok(())
            }
            Command::InstallSkill { root: _ } => {

                // Find the skill file bundled with the binary
                let skill_content = include_str!("../../skills/engram/SKILL.md");

                // Install to ~/.claude/skills/engram/
                let home = std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .unwrap_or_else(|_| ".".to_string());
                let skill_dir = std::path::PathBuf::from(&home).join(".claude/skills/engram");
                std::fs::create_dir_all(&skill_dir)?;
                std::fs::write(skill_dir.join("SKILL.md"), skill_content)?;

                // Register in ~/.claude/CLAUDE.md
                let claude_md_path = std::path::PathBuf::from(&home).join(".claude/CLAUDE.md");
                let existing = std::fs::read_to_string(&claude_md_path).unwrap_or_default();
                if !existing.contains("/engram") {
                    let registration = "\n\n## Engram\nWhen the user types `/engram`, invoke the Skill tool with `skill: \"engram\"` before doing anything else.\n";
                    let mut content = existing;
                    content.push_str(registration);
                    std::fs::write(&claude_md_path, content)?;
                }

                println!("Engram skill installed at {}", skill_dir.display());
                println!("Registered in ~/.claude/CLAUDE.md");
                println!("Use /engram in Claude Code to get started.");
                Ok(())
            }
            Command::InstallHooks { root } => {
                let root = std::fs::canonicalize(&root)?;
                let claude_settings_dir = root.join(".claude");
                std::fs::create_dir_all(&claude_settings_dir)?;

                let settings_path = claude_settings_dir.join("settings.local.json");
                let engram_bin =
                    std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("engram"));

                let hooks_config = serde_json::json!({
                    "hooks": {
                        "PreToolUse": [
                            {
                                "matcher": "Read",
                                "hooks": [{
                                    "type": "command",
                                    "command": format!("{} search --root {} --top_k 3 \"$TOOL_INPUT_FILE\" 2>/dev/null || true",
                                        engram_bin.display(), root.display())
                                }]
                            }
                        ]
                    }
                });

                let json = serde_json::to_string_pretty(&hooks_config)?;
                std::fs::write(&settings_path, json)?;

                println!("Claude Code hooks installed at {}", settings_path.display());
                println!("PreToolUse/Read hook will inject Engram context before file reads.");
                Ok(())
            }
        }
    }
}

/// Compute embeddings for all symbols that need them.
fn compute_embeddings(store: &crate::graph::Store) -> Result<usize> {
    let stale = store.get_stale_embeddings()?;
    if stale.is_empty() {
        return Ok(0);
    }

    let engine = crate::embeddings::EmbeddingEngine::new()?;
    let mut count = 0usize;

    // Build embedding texts for all stale symbols
    let mut texts = Vec::new();
    let mut ids_and_hashes = Vec::new();

    for (symbol_id, body_hash) in &stale {
        if let Some(sym) = store.get_symbol(symbol_id)? {
            // Get callers and deps for scope-enriched text
            let callers: Vec<String> = store
                .get_direct_callers(symbol_id)?
                .iter()
                .filter_map(|id| store.get_symbol(id).ok().flatten().map(|s| s.name))
                .collect();
            let deps: Vec<String> = store
                .find_dependencies(symbol_id, 1)?
                .iter()
                .filter_map(|(id, _)| store.get_symbol(id).ok().flatten().map(|s| s.name))
                .collect();

            let text =
                crate::embeddings::EmbeddingEngine::build_embedding_text(&sym, &callers, &deps);
            texts.push(text);
            ids_and_hashes.push((symbol_id.clone(), body_hash.clone()));
        }
    }

    // Batch embed (256 at a time to avoid OOM)
    for chunk in texts.chunks(256) {
        let chunk_vec: Vec<String> = chunk.to_vec();
        let embeddings = engine.embed_batch(&chunk_vec)?;

        let offset = count;
        for (i, embedding) in embeddings.into_iter().enumerate() {
            let (ref sym_id, ref body_hash) = ids_and_hashes[offset + i];
            store.save_embedding(sym_id, &embedding, body_hash)?;
            count += 1;
        }
    }

    Ok(count)
}

/// Fallback BM25-only search.
fn run_bm25_search(store: &crate::graph::Store, query: &str, top_k: usize) -> Result<()> {
    let results = store.search_bm25(query, top_k)?;
    if results.is_empty() {
        println!("No results found for '{}'", query);
    } else {
        for (symbol_id, score) in &results {
            if let Some(sym) = store.get_symbol(symbol_id)? {
                println!(
                    "  {:.3}  {} {} ({}:{})",
                    score, sym.kind, sym.name, sym.file, sym.line_start
                );
            }
        }
    }
    Ok(())
}
