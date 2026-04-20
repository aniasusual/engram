use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use super::schema;
use crate::parser::ParseResult;

// ── Types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SymbolRow {
    pub id: String,
    pub canonical_id: String,
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line_start: usize,
    pub line_end: usize,
    pub signature: String,
    pub docstring: Option<String>,
    pub body_hash: String,
    pub full_hash: String,
    pub language: String,
    pub scope_chain: String,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoreStats {
    pub symbol_count: usize,
    pub edge_count: usize,
    pub file_count: usize,
}

// ── Store ─────────────────────────────────────────────────────────────────

pub struct Store {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").finish()
    }
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).context("opening database")?;

        // WAL mode for concurrent reads
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("opening in-memory database")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn initialize(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        schema::create_schema(&conn)?;
        schema::create_fts_table(&conn)?;
        Ok(())
    }

    // ── File sync ─────────────────────────────────────────────────────

    /// Sync a parsed file into the store. Returns true if changes were made.
    pub fn sync_file(&self, relative_path: &Path, result: &ParseResult) -> Result<bool> {
        let file_str = relative_path.to_string_lossy().to_string();

        // Compute content hash from all symbols
        let content_hash = self.compute_content_hash(result);

        let conn = self.conn.lock().unwrap();

        // Check if file has changed
        let existing_hash: Option<String> = conn
            .query_row(
                "SELECT content_hash FROM file_hashes WHERE file = ?1",
                params![file_str],
                |row| row.get(0),
            )
            .ok();

        if existing_hash.as_deref() == Some(&content_hash) {
            return Ok(false); // No changes
        }

        let now = chrono::Utc::now().timestamp();

        // Begin transaction
        let tx = conn.unchecked_transaction()?;

        // Remove old data for this file
        tx.execute(
            "DELETE FROM chunks WHERE symbol_id IN (SELECT id FROM symbols WHERE file = ?1)",
            params![file_str],
        )?;
        tx.execute(
            "DELETE FROM fts_symbols WHERE symbol_id IN (SELECT id FROM symbols WHERE file = ?1)",
            params![file_str],
        )?;
        tx.execute("DELETE FROM edges WHERE file = ?1", params![file_str])?;
        tx.execute("DELETE FROM symbols WHERE file = ?1", params![file_str])?;

        // Insert symbols
        for sym in &result.symbols {
            let scope_chain_json = serde_json::to_string(&sym.scope_chain)?;
            tx.execute(
                "INSERT OR REPLACE INTO symbols
                 (id, canonical_id, name, kind, file, line_start, line_end,
                  signature, docstring, body_hash, full_hash, language,
                  scope_chain, parent_id, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    sym.id,
                    sym.canonical_id,
                    sym.name,
                    sym.kind.as_str(),
                    sym.file,
                    sym.line_start,
                    sym.line_end,
                    sym.signature,
                    sym.docstring,
                    sym.body_hash,
                    sym.full_hash,
                    sym.language,
                    scope_chain_json,
                    sym.parent_id,
                    now,
                ],
            )?;

            // Populate FTS index.
            // For constants, variables, and short symbols, include the body in FTS
            // so agents can search for values like "0.3" or "https://api.example.com".
            let fts_docstring = {
                let doc = sym.docstring.as_deref().unwrap_or("");
                let is_short_body = sym.body.len() < 200;
                let is_value_type = matches!(
                    sym.kind,
                    crate::parser::SymbolKind::Constant
                    | crate::parser::SymbolKind::Variable
                    | crate::parser::SymbolKind::TypeAlias
                );
                if is_value_type || is_short_body {
                    format!("{} {}", doc, sym.body)
                } else {
                    doc.to_string()
                }
            };

            tx.execute(
                "INSERT INTO fts_symbols (symbol_id, name, signature, docstring, scope_chain, file)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    sym.id,
                    sym.name,
                    sym.signature,
                    fts_docstring,
                    scope_chain_json,
                    sym.file,
                ],
            )?;

            // Sub-symbol chunking for large functions
            let chunks = crate::parser::chunking::chunk_symbol(sym);
            for chunk in &chunks {
                tx.execute(
                    "INSERT OR REPLACE INTO chunks
                     (id, symbol_id, chunk_index, content, line_start, line_end,
                      token_count, nws_count, body_hash)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        chunk.id,
                        chunk.symbol_id,
                        chunk.chunk_index,
                        chunk.content,
                        chunk.line_start,
                        chunk.line_end,
                        chunk.token_count,
                        chunk.nws_count,
                        chunk.body_hash,
                    ],
                )?;
            }
        }

        // Insert edges
        for edge in &result.edges {
            tx.execute(
                "INSERT OR REPLACE INTO edges (from_id, to_id, kind, file, line, confidence)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    edge.from_id,
                    edge.to_id,
                    edge.kind.as_str(),
                    edge.file,
                    edge.line,
                    edge.confidence,
                ],
            )?;
        }

        // Update file hash
        tx.execute(
            "INSERT OR REPLACE INTO file_hashes (file, content_hash, updated_at)
             VALUES (?1, ?2, ?3)",
            params![file_str, content_hash, now],
        )?;

        tx.commit()?;
        Ok(true)
    }

    fn compute_content_hash(&self, result: &ParseResult) -> String {
        let mut hasher = Sha256::new();
        for sym in &result.symbols {
            hasher.update(sym.full_hash.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    // ── Queries ───────────────────────────────────────────────────────

    pub fn get_symbol(&self, id: &str) -> Result<Option<SymbolRow>> {
        let result = self.conn.lock().unwrap().query_row(
            "SELECT id, canonical_id, name, kind, file, line_start, line_end,
                    signature, docstring, body_hash, full_hash, language,
                    scope_chain, parent_id
             FROM symbols WHERE id = ?1",
            params![id],
            |row| {
                Ok(SymbolRow {
                    id: row.get(0)?,
                    canonical_id: row.get(1)?,
                    name: row.get(2)?,
                    kind: row.get(3)?,
                    file: row.get(4)?,
                    line_start: row.get(5)?,
                    line_end: row.get(6)?,
                    signature: row.get(7)?,
                    docstring: row.get(8)?,
                    body_hash: row.get(9)?,
                    full_hash: row.get(10)?,
                    language: row.get(11)?,
                    scope_chain: row.get(12)?,
                    parent_id: row.get(13)?,
                })
            },
        );

        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn find_symbol_by_name(&self, name: &str) -> Result<Vec<SymbolRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, canonical_id, name, kind, file, line_start, line_end,
                    signature, docstring, body_hash, full_hash, language,
                    scope_chain, parent_id
             FROM symbols WHERE name = ?1",
        )?;
        let rows = stmt.query_map(params![name], |row| {
            Ok(SymbolRow {
                id: row.get(0)?,
                canonical_id: row.get(1)?,
                name: row.get(2)?,
                kind: row.get(3)?,
                file: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
                signature: row.get(7)?,
                docstring: row.get(8)?,
                body_hash: row.get(9)?,
                full_hash: row.get(10)?,
                language: row.get(11)?,
                scope_chain: row.get(12)?,
                parent_id: row.get(13)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// BM25 full-text search via FTS5.
    pub fn search_bm25(&self, query: &str, top_k: usize) -> Result<Vec<(String, f64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT symbol_id, rank
             FROM fts_symbols
             WHERE fts_symbols MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![query, top_k], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Find all callers of a symbol (reverse call graph).
    pub fn find_callers(&self, symbol_id: &str, depth: usize) -> Result<Vec<(String, usize)>> {
        let conn = self.conn.lock().unwrap();
        let mut results = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut frontier = vec![(symbol_id.to_string(), 0usize)];

        while let Some((current_id, current_depth)) = frontier.pop() {
            if current_depth > depth || !visited.insert(current_id.clone()) {
                continue;
            }
            if current_depth > 0 {
                results.push((current_id.clone(), current_depth));
            }

            let mut stmt = conn.prepare(
                "SELECT from_id FROM edges WHERE to_id = ?1 AND kind = 'CALLS'",
            )?;
            let callers: Vec<String> = stmt
                .query_map(params![current_id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();

            for caller_id in callers {
                frontier.push((caller_id, current_depth + 1));
            }
        }
        Ok(results)
    }

    /// Find all dependencies of a symbol (forward call graph).
    pub fn find_dependencies(&self, symbol_id: &str, depth: usize) -> Result<Vec<(String, usize)>> {
        let conn = self.conn.lock().unwrap();
        let mut results = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut frontier = vec![(symbol_id.to_string(), 0usize)];

        while let Some((current_id, current_depth)) = frontier.pop() {
            if current_depth > depth || !visited.insert(current_id.clone()) {
                continue;
            }
            if current_depth > 0 {
                results.push((current_id.clone(), current_depth));
            }

            let mut stmt = conn.prepare(
                "SELECT to_id FROM edges WHERE from_id = ?1 AND kind = 'CALLS'",
            )?;
            let deps: Vec<String> = stmt
                .query_map(params![current_id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();

            for dep_id in deps {
                frontier.push((dep_id, current_depth + 1));
            }
        }
        Ok(results)
    }

    /// Get symbols in a file.
    pub fn get_file_symbols(&self, file: &str) -> Result<Vec<SymbolRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, canonical_id, name, kind, file, line_start, line_end,
                    signature, docstring, body_hash, full_hash, language,
                    scope_chain, parent_id
             FROM symbols WHERE file = ?1 ORDER BY line_start",
        )?;
        let rows = stmt.query_map(params![file], |row| {
            Ok(SymbolRow {
                id: row.get(0)?,
                canonical_id: row.get(1)?,
                name: row.get(2)?,
                kind: row.get(3)?,
                file: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
                signature: row.get(7)?,
                docstring: row.get(8)?,
                body_hash: row.get(9)?,
                full_hash: row.get(10)?,
                language: row.get(11)?,
                scope_chain: row.get(12)?,
                parent_id: row.get(13)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get store statistics.
    pub fn stats(&self) -> Result<StoreStats> {
        let conn = self.conn.lock().unwrap();
        let symbol_count: usize = conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?;
        let edge_count: usize = conn
            .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))?;
        let file_count: usize = conn
            .query_row("SELECT COUNT(*) FROM file_hashes", [], |row| row.get(0))?;
        Ok(StoreStats {
            symbol_count,
            edge_count,
            file_count,
        })
    }

    /// Remove all data for files that no longer exist.
    pub fn garbage_collect(&self, existing_files: &[&str]) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let mut removed = 0usize;
        let all_files: Vec<String> = {
            let mut stmt = conn.prepare("SELECT file FROM file_hashes")?;
            stmt.query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect()
        };

        for file in &all_files {
            if !existing_files.contains(&file.as_str()) {
                conn.execute(
                    "DELETE FROM fts_symbols WHERE symbol_id IN (SELECT id FROM symbols WHERE file = ?1)",
                    params![file],
                )?;
                conn.execute("DELETE FROM edges WHERE file = ?1", params![file])?;
                conn.execute("DELETE FROM symbols WHERE file = ?1", params![file])?;
                conn.execute("DELETE FROM file_hashes WHERE file = ?1", params![file])?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// Get language breakdown (language → symbol count).
    pub fn language_breakdown(&self) -> Result<Vec<(String, usize)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT language, COUNT(*) FROM symbols GROUP BY language ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Create an annotation on a symbol, anchored to its current full_hash.
    pub fn create_annotation(
        &self,
        symbol_id: &str,
        annotation_type: &str,
        content: &str,
        full_hash_at: &str,
    ) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO annotations (symbol_id, annotation_type, content, full_hash_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![symbol_id, annotation_type, content, full_hash_at, now, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Verify an annotation — set confidence to 1.0.
    pub fn verify_annotation(&self, annotation_id: i64) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.lock().unwrap().execute(
            "UPDATE annotations SET confidence = 1.0, status = 'active', updated_at = ?1 WHERE id = ?2",
            params![now, annotation_id],
        )?;
        Ok(())
    }

    /// Downvote an annotation — reduce confidence by 0.2 (min 0.0).
    pub fn downvote_annotation(&self, annotation_id: i64) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.lock().unwrap().execute(
            "UPDATE annotations SET confidence = MAX(0.0, confidence - 0.2), updated_at = ?1 WHERE id = ?2",
            params![now, annotation_id],
        )?;
        Ok(())
    }

    // ── Embedding persistence ──────────────────────────────────────────

    /// Save an embedding to the database.
    pub fn save_embedding(&self, symbol_id: &str, embedding: &[f32], body_hash: &str) -> Result<()> {
        let blob: Vec<u8> = embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO embeddings (symbol_id, embedding, body_hash)
             VALUES (?1, ?2, ?3)",
            params![symbol_id, blob, body_hash],
        )?;
        Ok(())
    }

    /// Load all embeddings from the database.
    pub fn get_all_embeddings(&self) -> Result<Vec<(String, Vec<f32>)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT symbol_id, embedding FROM embeddings",
        )?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            let embedding: Vec<f32> = blob
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            Ok((id, embedding))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get embeddings that need recomputing (body_hash mismatch).
    pub fn get_stale_embeddings(&self) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT s.id, s.body_hash FROM symbols s
             LEFT JOIN embeddings e ON s.id = e.symbol_id
             WHERE e.symbol_id IS NULL OR e.body_hash != s.body_hash",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get annotations for a symbol.
    pub fn get_annotations(&self, symbol_id: &str) -> Result<Vec<(i64, String, String, f64, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, annotation_type, content, confidence, status
             FROM annotations WHERE symbol_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![symbol_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // ── Cascade support ──────────────────────────────────────────────

    /// Get direct callers of a symbol (one hop only).
    pub fn get_direct_callers(&self, symbol_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT from_id FROM edges WHERE to_id = ?1 AND kind = 'CALLS'",
        )?;
        let rows = stmt.query_map(rusqlite::params![symbol_id], |row| row.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get the full_hash_at for an annotation.
    pub fn get_annotation_hash(&self, annotation_id: i64) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT full_hash_at FROM annotations WHERE id = ?1",
            rusqlite::params![annotation_id],
            |row| row.get(0),
        );
        match result {
            Ok(hash) => Ok(Some(hash)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Update annotation status and confidence.
    pub fn update_annotation_status(&self, annotation_id: i64, status: &str, confidence: f64) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.lock().unwrap().execute(
            "UPDATE annotations SET status = ?1, confidence = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![status, confidence, now, annotation_id],
        )?;
        Ok(())
    }

    /// Reduce annotation confidence to a specific value.
    pub fn reduce_annotation_confidence(&self, annotation_id: i64, new_confidence: f64) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.lock().unwrap().execute(
            "UPDATE annotations SET confidence = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![new_confidence, now, annotation_id],
        )?;
        Ok(())
    }

    /// Write a cascade log entry to the database.
    pub fn write_cascade_log(&self, entry: &crate::memory::cascade::CascadeEntry, timestamp: i64) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO cascade_log (trigger_symbol, affected_symbol, annotation_id, old_confidence, new_confidence, reason, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                entry.trigger_symbol,
                entry.affected_symbol,
                entry.annotation_id,
                entry.old_confidence,
                entry.new_confidence,
                entry.reason,
                timestamp,
            ],
        )?;
        Ok(())
    }

    /// Get cascade log entries for a trigger symbol.
    pub fn get_cascade_log(&self, trigger_symbol: &str) -> Result<Vec<(String, String, i64, f64, f64, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT trigger_symbol, affected_symbol, annotation_id, old_confidence, new_confidence, reason
             FROM cascade_log WHERE trigger_symbol = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(rusqlite::params![trigger_symbol], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // ── Knowledge layer ──────────────────────────────────────────────

    /// Record an architectural decision.
    pub fn record_decision(
        &self,
        symbol_ids: &[String],
        description: &str,
        rationale: Option<&str>,
        alternatives: Option<&str>,
    ) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();
        let ids_json = serde_json::to_string(symbol_ids)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO decisions (symbol_ids, description, rationale, alternatives, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![ids_json, description, rationale, alternatives, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Record a recurring pattern.
    pub fn record_pattern(
        &self,
        name: &str,
        description: &str,
        symbol_ids: &[String],
    ) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();
        let ids_json = serde_json::to_string(symbol_ids)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO patterns (name, description, trigger_symbol_ids, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![name, description, ids_json, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Create an insight (auto-generated warning).
    pub fn create_insight(
        &self,
        insight_type: &str,
        content: &str,
        symbol_ids: &[String],
    ) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();
        let ids_json = serde_json::to_string(symbol_ids)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO insights (insight_type, content, symbol_ids, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![insight_type, content, ids_json, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get active insights.
    pub fn get_insights(&self) -> Result<Vec<(i64, String, String, String, f64, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, insight_type, content, COALESCE(symbol_ids, '[]'), confidence, status
             FROM insights WHERE status = 'active' ORDER BY confidence DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Resolve or dismiss an insight.
    pub fn resolve_insight(&self, insight_id: i64, status: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE insights SET status = ?1 WHERE id = ?2",
            params![status, insight_id],
        )?;
        Ok(())
    }

    /// Compute risk score for a symbol: complexity × caller_count × staleness.
    pub fn get_risk_score(&self, symbol_id: &str) -> Result<f64> {
        let conn = self.conn.lock().unwrap();

        // Caller count
        let caller_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM edges WHERE to_id = ?1 AND kind = 'CALLS'",
            params![symbol_id],
            |row| row.get(0),
        ).unwrap_or(0);

        // Stale annotation count
        let stale_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM annotations WHERE symbol_id = ?1 AND status = 'stale'",
            params![symbol_id],
            |row| row.get(0),
        ).unwrap_or(0);

        // Complexity (stored in symbols table, default 0)
        let complexity: i64 = conn.query_row(
            "SELECT COALESCE(complexity, 0) FROM symbols WHERE id = ?1",
            params![symbol_id],
            |row| row.get(0),
        ).unwrap_or(0);

        // Risk = (1 + complexity) × (1 + callers) × (1 + stale_annotations)
        let risk = (1.0 + complexity as f64) * (1.0 + caller_count as f64) * (1.0 + stale_count as f64);
        Ok(risk)
    }

    /// Add a topic tag to a symbol.
    pub fn add_topic(&self, topic: &str, symbol_id: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT OR IGNORE INTO topics (topic, symbol_id) VALUES (?1, ?2)",
            params![topic, symbol_id],
        )?;
        Ok(())
    }

    /// Get symbols for a topic.
    pub fn get_topic_symbols(&self, topic: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT symbol_id FROM topics WHERE topic = ?1",
        )?;
        let rows = stmt.query_map(params![topic], |row| row.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get all topics with their symbol counts.
    pub fn get_all_topics(&self) -> Result<Vec<(String, usize)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT topic, COUNT(*) FROM topics GROUP BY topic ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get all symbols (for codebase report).
    pub fn get_all_symbols(&self) -> Result<Vec<SymbolRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, canonical_id, name, kind, file, line_start, line_end,
                    signature, docstring, body_hash, full_hash, language,
                    scope_chain, parent_id
             FROM symbols ORDER BY file, line_start",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SymbolRow {
                id: row.get(0)?,
                canonical_id: row.get(1)?,
                name: row.get(2)?,
                kind: row.get(3)?,
                file: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
                signature: row.get(7)?,
                docstring: row.get(8)?,
                body_hash: row.get(9)?,
                full_hash: row.get(10)?,
                language: row.get(11)?,
                scope_chain: row.get(12)?,
                parent_id: row.get(13)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // ── Interaction tracking ─────────────────────────────────────────

    /// Record that a symbol was accessed (viewed, queried, or annotated).
    pub fn record_attention(&self, symbol_id: &str, event: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let conn = self.conn.lock().unwrap();

        // Upsert attention_map
        conn.execute(
            "INSERT INTO attention_map (symbol_id, view_count, query_count, annotate_count, last_accessed, importance_score)
             VALUES (?1, 0, 0, 0, ?2, 0.0)
             ON CONFLICT(symbol_id) DO UPDATE SET last_accessed = ?2",
            params![symbol_id, now],
        )?;

        match event {
            "view" => conn.execute(
                "UPDATE attention_map SET view_count = view_count + 1 WHERE symbol_id = ?1",
                params![symbol_id],
            )?,
            "query" => conn.execute(
                "UPDATE attention_map SET query_count = query_count + 1 WHERE symbol_id = ?1",
                params![symbol_id],
            )?,
            "annotate" => conn.execute(
                "UPDATE attention_map SET annotate_count = annotate_count + 1 WHERE symbol_id = ?1",
                params![symbol_id],
            )?,
            _ => 0,
        };

        // Update importance score: views + 2*queries + 3*annotations
        conn.execute(
            "UPDATE attention_map SET importance_score = view_count + 2.0 * query_count + 3.0 * annotate_count
             WHERE symbol_id = ?1",
            params![symbol_id],
        )?;

        Ok(())
    }

    /// Get the exploration map: which symbols have been accessed and which are blind spots.
    pub fn get_exploration_map(&self) -> Result<(Vec<(String, f64)>, Vec<String>)> {
        let conn = self.conn.lock().unwrap();

        // Explored symbols (have attention)
        let mut stmt = conn.prepare(
            "SELECT a.symbol_id, a.importance_score FROM attention_map a
             ORDER BY a.importance_score DESC",
        )?;
        let explored: Vec<(String, f64)> = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        // Blind spots: symbols with callers > 3 but never accessed
        let mut stmt2 = conn.prepare(
            "SELECT s.id FROM symbols s
             LEFT JOIN attention_map a ON s.id = a.symbol_id
             WHERE a.symbol_id IS NULL
             AND (SELECT COUNT(*) FROM edges e WHERE e.to_id = s.id AND e.kind = 'CALLS') > 3
             ORDER BY (SELECT COUNT(*) FROM edges e WHERE e.to_id = s.id) DESC
             LIMIT 20",
        )?;
        let blind_spots: Vec<String> = stmt2
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        Ok((explored, blind_spots))
    }

    /// Get suggested next symbols to explore, based on attention patterns.
    pub fn suggest_next(&self, limit: usize) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();

        // Suggest: high-caller symbols that haven't been explored,
        // or symbols adjacent to recently-explored ones
        let mut stmt = conn.prepare(
            "SELECT s.id, s.name FROM symbols s
             LEFT JOIN attention_map a ON s.id = a.symbol_id
             WHERE a.symbol_id IS NULL
             ORDER BY (SELECT COUNT(*) FROM edges e WHERE e.to_id = s.id) DESC
             LIMIT ?1",
        )?;
        let suggestions: Vec<(String, String)> = stmt
            .query_map(params![limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(suggestions)
    }

    /// Get high-attention symbols sorted by importance.
    pub fn get_hot_symbols(&self, limit: usize) -> Result<Vec<(String, f64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT symbol_id, importance_score FROM attention_map
             WHERE importance_score > 0 ORDER BY importance_score DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // ── Git data persistence ─────────────────────────────────────────

    /// Upsert a git commit.
    pub fn upsert_git_commit(&self, hash: &str, author: &str, email: &str, timestamp: i64, message: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO git_commits (hash, author, email, timestamp, message)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![hash, author, email, timestamp, message],
        )?;
        Ok(())
    }

    /// Upsert file ownership.
    pub fn upsert_file_ownership(&self, file: &str, author: &str, email: &str, commits: usize, last_touched: i64) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO file_ownership (file, author, email, commits, last_touched)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![file, author, email, commits, last_touched],
        )?;
        Ok(())
    }

    /// Get all tracked file paths.
    pub fn get_all_files(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT file FROM file_hashes")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get file ownership data.
    pub fn get_file_ownership(&self, file: &str) -> Result<Vec<(String, String, usize, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT author, email, commits, last_touched FROM file_ownership
             WHERE file = ?1 ORDER BY commits DESC",
        )?;
        let rows = stmt.query_map(params![file], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, usize>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get commits for a file from git_commits.
    pub fn get_file_commit_history(&self, _file: &str) -> Result<Vec<(String, String, i64, String)>> {
        // Returns recent commits. Full file-level filtering would use symbol_commits.
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT gc.hash, gc.author, gc.timestamp, gc.message
             FROM git_commits gc
             ORDER BY gc.timestamp DESC
             LIMIT 50",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // ── Temporal data persistence ────────────────────────────────────

    /// Log a symbol evolution event.
    pub fn log_symbol_evolution(
        &self,
        symbol_id: &str,
        commit_hash: Option<&str>,
        change_type: &str,
        old_full_hash: Option<&str>,
        new_full_hash: Option<&str>,
        old_file: Option<&str>,
        new_file: Option<&str>,
        diff_summary: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let id = {
            let mut hasher = sha2::Sha256::new();
            hasher.update(format!("{}:{}:{}", symbol_id, change_type, now).as_bytes());
            hex::encode(hasher.finalize())
        };

        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO symbol_evolution
             (id, symbol_id, commit_hash, timestamp, change_type, old_full_hash, new_full_hash, old_file, new_file, diff_summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![id, symbol_id, commit_hash, now, change_type, old_full_hash, new_full_hash, old_file, new_file, diff_summary],
        )?;
        Ok(())
    }

    /// Get evolution history for a symbol.
    pub fn get_symbol_evolution(&self, symbol_id: &str) -> Result<Vec<(String, i64, String, Option<String>, Option<String>)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT change_type, timestamp, COALESCE(diff_summary, ''), old_full_hash, new_full_hash
             FROM symbol_evolution WHERE symbol_id = ?1 ORDER BY timestamp DESC",
        )?;
        let rows = stmt.query_map(params![symbol_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Save branch context snapshot.
    pub fn save_branch_context(&self, branch: &str, snapshot: &HashMap<String, String>) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let json = serde_json::to_string(snapshot)?;
        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO branch_context (branch_name, last_seen_at, symbol_snapshot)
             VALUES (?1, ?2, ?3)",
            params![branch, now, json],
        )?;
        Ok(())
    }

    /// Get previous full_hash for a symbol (before the last sync).
    pub fn get_previous_full_hash(&self, symbol_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT old_full_hash FROM symbol_evolution
             WHERE symbol_id = ?1 ORDER BY timestamp DESC LIMIT 1",
            params![symbol_id],
            |row| row.get(0),
        );
        match result {
            Ok(hash) => Ok(Some(hash)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::CodeParser;

    fn setup_store() -> Store {
        let store = Store::open_in_memory().unwrap();
        store.initialize().unwrap();
        store
    }

    #[test]
    fn test_sync_and_query() {
        let store = setup_store();
        let parser = CodeParser::new();

        let source = r#"
fn hello() {
    println!("world");
}

fn greet() {
    hello();
}
"#;
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        let changed = store.sync_file(Path::new("test.rs"), &result).unwrap();
        assert!(changed);

        let stats = store.stats().unwrap();
        assert_eq!(stats.symbol_count, 2);
        assert_eq!(stats.file_count, 1);

        // BM25 search
        let results = store.search_bm25("hello", 5).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_idempotent_sync() {
        let store = setup_store();
        let parser = CodeParser::new();
        let source = "fn foo() {}";
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();

        let changed1 = store.sync_file(Path::new("test.rs"), &result).unwrap();
        assert!(changed1);

        let changed2 = store.sync_file(Path::new("test.rs"), &result).unwrap();
        assert!(!changed2, "second sync of identical file should be no-op");
    }

    #[test]
    fn test_find_callers() {
        let store = setup_store();
        let parser = CodeParser::new();

        let source = r#"
fn leaf() -> bool { true }
fn middle() { leaf(); }
fn top() { middle(); }
"#;
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        // Find callers of leaf
        let leaf = store.find_symbol_by_name("leaf").unwrap();
        assert!(!leaf.is_empty());

        let callers = store.find_callers(&leaf[0].id, 3).unwrap();
        assert!(callers.len() >= 1, "leaf should have at least 1 caller");
    }

    #[test]
    fn test_garbage_collection() {
        let store = setup_store();
        let parser = CodeParser::new();

        let r1 = parser.parse_source("fn a() {}", "rust", "a.rs").unwrap();
        let r2 = parser.parse_source("fn b() {}", "rust", "b.rs").unwrap();
        store.sync_file(Path::new("a.rs"), &r1).unwrap();
        store.sync_file(Path::new("b.rs"), &r2).unwrap();

        assert_eq!(store.stats().unwrap().file_count, 2);

        // b.rs is "deleted"
        let removed = store.garbage_collect(&["a.rs"]).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(store.stats().unwrap().file_count, 1);
        assert_eq!(store.stats().unwrap().symbol_count, 1);
    }

    #[test]
    fn test_constant_values_searchable() {
        let store = setup_store();
        let parser = CodeParser::new();

        let source = r#"
const MAX_RETRIES: u32 = 3;
const API_URL: &str = "https://api.example.com";
fn process() {}
"#;
        let result = parser.parse_source(source, "rust", "config.rs").unwrap();
        store.sync_file(Path::new("config.rs"), &result).unwrap();

        // Search for the constant value
        let results = store.search_bm25("MAX_RETRIES", 5).unwrap();
        assert!(!results.is_empty(), "should find MAX_RETRIES by name");

        // Search for the URL value in the constant body
        let url_results = store.search_bm25("api example", 5).unwrap();
        assert!(!url_results.is_empty(), "should find API_URL by searching its value content");
    }

    #[test]
    fn test_markdown_searchable() {
        let store = setup_store();

        let md_content = "# Authentication\n\nThis module handles JWT token validation.\n\n## Endpoints\n\nPOST /api/auth accepts Bearer tokens.\n";
        let result = crate::parser::markdown::parse_markdown(md_content, "docs/auth.md");
        store.sync_file(Path::new("docs/auth.md"), &result).unwrap();

        // Search for content from the markdown
        let results = store.search_bm25("JWT token", 5).unwrap();
        assert!(!results.is_empty(), "should find markdown content via FTS");

        let results2 = store.search_bm25("Bearer", 5).unwrap();
        assert!(!results2.is_empty(), "should find 'Bearer' in markdown via FTS");
    }
}
