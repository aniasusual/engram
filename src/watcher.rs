use anyhow::Result;
use notify::{EventKind, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::graph::Store;
use crate::parser::{self, CodeParser};

/// Debounce window for file events (3 seconds, like Graphify).
const DEBOUNCE_DURATION: Duration = Duration::from_secs(3);

/// Start watching a directory for changes and incrementally re-index.
/// On each change: re-parse → sync to DB → track evolution → run cascade → recompute embeddings.
pub async fn watch_and_sync(root: PathBuf, store: Arc<Store>) -> Result<()> {
    let parser = CodeParser::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<PathBuf>>(32);

    // Spawn the notify watcher in a blocking thread
    let root_for_watcher = root.clone();
    let _watcher_handle = tokio::task::spawn_blocking(move || {
        let tx = tx;
        let mut pending: Vec<PathBuf> = Vec::new();
        let mut last_event = std::time::Instant::now();

        let (notify_tx, notify_rx) = std::sync::mpsc::channel();
        let mut watcher =
            notify::recommended_watcher(notify_tx).expect("failed to create watcher");

        watcher
            .watch(&root_for_watcher, RecursiveMode::Recursive)
            .expect("failed to watch directory");

        loop {
            match notify_rx.recv_timeout(DEBOUNCE_DURATION) {
                Ok(Ok(event)) => {
                    if matches!(
                        event.kind,
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                    ) {
                        for path in event.paths {
                            if parser::detect_language(&path).is_some() {
                                pending.push(path);
                                last_event = std::time::Instant::now();
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!("Watch error: {}", e);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if !pending.is_empty() && last_event.elapsed() >= DEBOUNCE_DURATION {
                        let batch: Vec<PathBuf> = pending.drain(..).collect();
                        if tx.blocking_send(batch).is_err() {
                            break;
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    // Lazily-initialized embedding engine (only loaded if needed)
    let mut embedding_engine: Option<crate::embeddings::EmbeddingEngine> = None;

    // Process debounced events
    while let Some(changed_files) = rx.recv().await {
        let unique_files: std::collections::HashSet<PathBuf> =
            changed_files.into_iter().collect();

        let mut changed_file_strs: Vec<String> = Vec::new();
        let mut deleted_file_strs: Vec<String> = Vec::new();

        for file in &unique_files {
            let relative = file.strip_prefix(&root).unwrap_or(file);
            let rel_str = relative.to_string_lossy().to_string();

            if file.exists() {
                // ── Track evolution BEFORE sync (compare old vs new) ──
                if let Ok(new_result) = parser.parse_file(file) {
                    crate::temporal::track_evolution(&store, &rel_str, &new_result)
                        .unwrap_or_else(|e| tracing::warn!("Evolution tracking failed: {}", e));

                    // ── Sync to DB ──
                    match store.sync_file(relative, &new_result) {
                        Ok(true) => {
                            tracing::info!(
                                "Re-indexed {} ({} symbols)",
                                rel_str,
                                new_result.symbols.len()
                            );
                            changed_file_strs.push(rel_str.clone());

                            // ── Run cascade for changed symbols ──
                            match crate::memory::cascade::cascade_file(&store, &rel_str) {
                                Ok(results) => {
                                    for r in &results {
                                        if r.direct_stale_count > 0 || r.transitive_affected_count > 0 {
                                            tracing::info!(
                                                "Cascade: {} stale, {} transitive for {}",
                                                r.direct_stale_count,
                                                r.transitive_affected_count,
                                                rel_str,
                                            );
                                        }
                                    }
                                }
                                Err(e) => tracing::warn!("Cascade failed: {}", e),
                            }

                            // ── Recompute embeddings for changed symbols ──
                            let engine = embedding_engine.get_or_insert_with(|| {
                                crate::embeddings::EmbeddingEngine::new()
                                    .expect("failed to load embedding model")
                            });
                            for sym in &new_result.symbols {
                                let callers: Vec<String> = store
                                    .get_direct_callers(&sym.id)
                                    .unwrap_or_default()
                                    .iter()
                                    .filter_map(|id| {
                                        store.get_symbol(id).ok().flatten().map(|s| s.name)
                                    })
                                    .collect();
                                let text = crate::embeddings::EmbeddingEngine::build_embedding_text(
                                    &store.get_symbol(&sym.id).unwrap().unwrap(),
                                    &callers,
                                    &[],
                                );
                                if let Ok(emb) = engine.embed_one(&text) {
                                    let _ = store.save_embedding(&sym.id, &emb, &sym.body_hash);
                                }
                            }
                        }
                        Ok(false) => {} // No changes
                        Err(e) => tracing::warn!("Sync failed for {}: {}", rel_str, e),
                    }
                }
            } else {
                // File deleted
                tracing::info!("File deleted: {}", rel_str);
                deleted_file_strs.push(rel_str);
            }
        }

        // ── Garbage collect deleted files ──
        if !deleted_file_strs.is_empty() {
            let existing: Vec<String> = store.get_all_files().unwrap_or_default();
            let existing_refs: Vec<&str> = existing.iter().map(|s| s.as_str()).collect();
            match store.garbage_collect(&existing_refs) {
                Ok(removed) if removed > 0 => {
                    tracing::info!("Garbage collected {} deleted files", removed);
                }
                _ => {}
            }
        }

        // ── Detect renames (deleted + created in same batch) ──
        if !deleted_file_strs.is_empty() && !changed_file_strs.is_empty() {
            match crate::temporal::detect_renames(
                &store,
                &deleted_file_strs,
                &changed_file_strs,
                &parser,
                &root,
            ) {
                Ok(renames) => {
                    for (old, new) in &renames {
                        tracing::info!("Detected rename: {} → {}", old, new);
                    }
                }
                Err(e) => tracing::warn!("Rename detection failed: {}", e),
            }
        }

        // ── Scan for contradictions ──
        if !changed_file_strs.is_empty() {
            match crate::intelligence::reasoning::scan_all_contradictions(&store) {
                Ok(count) if count > 0 => {
                    tracing::info!("Detected {} contradictions", count);
                }
                _ => {}
            }
        }
    }

    Ok(())
}
