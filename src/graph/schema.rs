use anyhow::Result;
use rusqlite::Connection;

/// Create all tables for the Engram database.
/// 22 tables across 7 layers as specified in the plan.
pub fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA_SQL)?;
    Ok(())
}

const SCHEMA_SQL: &str = r#"
-- ════════════════════════════════════════════════════════════════════════
-- Layer 0-1: Structural (Core Tables)
-- ════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS symbols (
    id              TEXT PRIMARY KEY,
    canonical_id    TEXT,
    name            TEXT NOT NULL,
    kind            TEXT NOT NULL,
    file            TEXT NOT NULL,
    line_start      INTEGER NOT NULL,
    line_end        INTEGER NOT NULL,
    signature       TEXT,
    docstring       TEXT,
    body_hash       TEXT NOT NULL,
    full_hash       TEXT NOT NULL,
    complexity      INTEGER DEFAULT 0,
    language        TEXT NOT NULL,
    scope_chain     TEXT,
    parent_id       TEXT,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_symbols_canonical ON symbols(canonical_id);

CREATE TABLE IF NOT EXISTS edges (
    from_id         TEXT NOT NULL,
    to_id           TEXT NOT NULL,
    kind            TEXT NOT NULL,
    file            TEXT,
    line            INTEGER,
    confidence      REAL DEFAULT 1.0,
    PRIMARY KEY (from_id, to_id, kind)
);

CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_id);
CREATE INDEX IF NOT EXISTS idx_edges_to ON edges(to_id);

CREATE TABLE IF NOT EXISTS chunks (
    id              TEXT PRIMARY KEY,
    symbol_id       TEXT NOT NULL,
    chunk_index     INTEGER NOT NULL,
    content         TEXT NOT NULL,
    line_start      INTEGER NOT NULL,
    line_end        INTEGER NOT NULL,
    token_count     INTEGER NOT NULL,
    nws_count       INTEGER NOT NULL,
    body_hash       TEXT NOT NULL,
    FOREIGN KEY (symbol_id) REFERENCES symbols(id)
);

CREATE INDEX IF NOT EXISTS idx_chunks_symbol ON chunks(symbol_id);

CREATE TABLE IF NOT EXISTS file_hashes (
    file            TEXT PRIMARY KEY,
    content_hash    TEXT NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS embeddings (
    symbol_id       TEXT PRIMARY KEY,
    embedding       BLOB NOT NULL,
    body_hash       TEXT NOT NULL,
    FOREIGN KEY (symbol_id) REFERENCES symbols(id)
);

-- ════════════════════════════════════════════════════════════════════════
-- Layer 2: Historical
-- ════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS git_commits (
    hash            TEXT PRIMARY KEY,
    author          TEXT NOT NULL,
    email           TEXT,
    timestamp       INTEGER NOT NULL,
    message         TEXT
);

CREATE TABLE IF NOT EXISTS symbol_commits (
    symbol_id       TEXT NOT NULL,
    commit_hash     TEXT NOT NULL,
    change_type     TEXT,
    PRIMARY KEY (symbol_id, commit_hash)
);

CREATE TABLE IF NOT EXISTS file_ownership (
    file            TEXT NOT NULL,
    author          TEXT NOT NULL,
    email           TEXT,
    commits         INTEGER DEFAULT 1,
    last_touched    INTEGER NOT NULL,
    PRIMARY KEY (file, author)
);

-- ════════════════════════════════════════════════════════════════════════
-- Layer 3: Interaction
-- ════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS cortex_sessions (
    id              TEXT PRIMARY KEY,
    started_at      INTEGER NOT NULL,
    ended_at        INTEGER,
    source          TEXT,
    goal            TEXT,
    summary         TEXT
);

CREATE TABLE IF NOT EXISTS interactions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id      TEXT,
    timestamp       INTEGER NOT NULL,
    tool_name       TEXT NOT NULL,
    query_text      TEXT,
    result_symbols  TEXT,
    duration_ms     INTEGER
);

CREATE TABLE IF NOT EXISTS attention_map (
    symbol_id       TEXT PRIMARY KEY,
    view_count      INTEGER DEFAULT 0,
    query_count     INTEGER DEFAULT 0,
    annotate_count  INTEGER DEFAULT 0,
    last_accessed   INTEGER,
    importance_score REAL DEFAULT 0.0
);

-- ════════════════════════════════════════════════════════════════════════
-- Layer 4: Knowledge
-- ════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS annotations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol_id       TEXT NOT NULL,
    annotation_type TEXT NOT NULL,
    content         TEXT NOT NULL,
    author          TEXT DEFAULT 'agent',
    confidence      REAL DEFAULT 0.8,
    full_hash_at    TEXT NOT NULL,
    status          TEXT DEFAULT 'active',
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_annotations_symbol ON annotations(symbol_id);
CREATE INDEX IF NOT EXISTS idx_annotations_status ON annotations(status);

CREATE TABLE IF NOT EXISTS decisions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol_ids      TEXT NOT NULL,
    description     TEXT NOT NULL,
    rationale       TEXT,
    alternatives    TEXT,
    confidence      REAL DEFAULT 0.8,
    created_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS patterns (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL,
    trigger_symbol_ids TEXT,
    evidence        TEXT,
    confidence      REAL DEFAULT 0.7,
    created_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS topics (
    topic           TEXT NOT NULL,
    symbol_id       TEXT NOT NULL,
    PRIMARY KEY (topic, symbol_id)
);

-- ════════════════════════════════════════════════════════════════════════
-- Layer 5: Reasoning
-- ════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS insights (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    insight_type    TEXT NOT NULL,
    content         TEXT NOT NULL,
    symbol_ids      TEXT,
    confidence      REAL DEFAULT 0.7,
    status          TEXT DEFAULT 'active',
    created_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS cascade_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger_symbol  TEXT NOT NULL,
    affected_symbol TEXT NOT NULL,
    annotation_id   INTEGER,
    old_confidence  REAL,
    new_confidence  REAL,
    reason          TEXT,
    timestamp       INTEGER NOT NULL
);

-- ════════════════════════════════════════════════════════════════════════
-- Layer 6: Temporal
-- ════════════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS symbol_evolution (
    id              TEXT PRIMARY KEY,
    symbol_id       TEXT NOT NULL,
    commit_hash     TEXT,
    timestamp       INTEGER NOT NULL,
    change_type     TEXT NOT NULL,
    old_full_hash   TEXT,
    new_full_hash   TEXT,
    old_file        TEXT,
    new_file        TEXT,
    diff_summary    TEXT,
    FOREIGN KEY (symbol_id) REFERENCES symbols(id)
);

CREATE INDEX IF NOT EXISTS idx_evolution_symbol ON symbol_evolution(symbol_id);

CREATE TABLE IF NOT EXISTS branch_context (
    branch_name     TEXT PRIMARY KEY,
    last_seen_at    INTEGER NOT NULL,
    symbol_snapshot TEXT NOT NULL
);
"#;

// FTS5 virtual table must be created separately (can't use IF NOT EXISTS in some builds)
pub fn create_fts_table(conn: &Connection) -> Result<()> {
    // Check if fts_symbols already exists
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='fts_symbols'",
        [],
        |row| row.get(0),
    )?;

    if !exists {
        conn.execute_batch(
            "CREATE VIRTUAL TABLE fts_symbols USING fts5(
                symbol_id,
                name,
                signature,
                docstring,
                scope_chain,
                file
            );",
        )?;
    }
    Ok(())
}
