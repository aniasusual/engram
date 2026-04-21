# Engram

[![CI](https://github.com/animeshdhillon/engram/actions/workflows/ci.yml/badge.svg)](https://github.com/animeshdhillon/engram/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

> **engram** (/ˈɛnɡræm/) — a hypothetical permanent change in the brain accounting for the existence of memory; a memory trace.

**Persistent codebase intelligence for coding agents.** A single 16MB binary that gives AI agents structural understanding, semantic search, temporal awareness, and self-healing memory — with zero dependencies and no API keys.

```
engram init && engram start
```

---

## Why Engram

Coding agents forget everything between sessions. They re-read files, re-discover architecture, and re-learn decisions that were already made. Existing tools either understand code structure but can't search semantically (Graphify), remember conversations but treat code as plain text (MemPalace), or store memories with a 97.8% junk rate (Mem0).

Engram combines what nobody else does:

- **Deep code structure** — directed AST call graphs across 9 languages, scope trees, cross-file entity resolution
- **Hybrid search** — vector + BM25 + graph proximity, fused with Reciprocal Rank Fusion
- **Self-healing memory** — hash-anchored annotations that auto-detect staleness and cascade confidence reductions through the call graph
- **Token-budget context** — progressive loading from 80-token briefs to full source, greedy-packed to your budget

## Benchmarks

### Performance

| Operation | Time |
|-----------|------|
| Incremental file update | 1.6 us |
| BM25 search (1,400 symbols) | 50 us |
| Cascade propagation | 9.3 us |
| Index 200 files | 56 ms |
| Binary size | 16 MB |

### Search Quality (EngramBench)

| Metric | BM25-only | Hybrid RRF | Target |
|--------|-----------|------------|--------|
| MRR | 0.667 | **1.000** | >0.85 |
| Recall@10 | 0.583 | **1.000** | >0.90 |
| Semantic query hit rate | 33% | **100%** | — |

### CodeSearchNet (External Benchmark)

| Model | NDCG@10 |
|-------|---------|
| Engram (AllMiniLM-L6-V2) | 0.154 |
| Neural BoW (baseline) | 0.170 |
| CodeBERT | 0.450 |

Engram uses a general-purpose English embedding model. The `Embedder` trait is pluggable — swapping to a code-specific model (CodeBERT, UniXcoder) will improve this score. The architecture is ready; the model is the bottleneck.

### Cascade Correctness

1,003 randomized test cases (proptest) across random DAGs, single-node graphs, fully-connected graphs, and 100-node stress tests. Zero failures. Properties verified: termination, no negative confidence, no duplicate visits, confidence monotonically decreasing on transitive cascade.

---

## Quick Start

### Build from source

```bash
git clone https://github.com/yourusername/engram.git
cd engram
cargo build --release
# Binary at target/release/engram (16MB)
```

### Use it

```bash
cd your-project

# Initialize
engram init

# Index + watch for changes
engram start
# Output:
#   Indexing /path/to/your-project...
#   Indexed 142 files, 891 symbols
#   Computing embeddings...
#   Embedded 891 symbols
#   Watching for changes...

# Search (hybrid RRF when embeddings exist, BM25 fallback)
engram search "authentication validation"
# Output:
#   Hybrid search (RRF: vector + BM25 + graph):
#     0.0164  struct AuthService (auth.rs:2)
#     0.0161  function validate_token (auth.rs:15)
#     0.0159  function extract_user_id (auth.rs:23)

# Check status
engram status
# Output:
#   Symbols: 891
#   Edges:   234
#   Files:   142
```

### Connect to Claude Code

```bash
# Install hooks (injects context before file reads)
engram install-hooks

# Or start the MCP server directly
engram mcp
```

Add to your Claude Code MCP config (`~/.claude.json`):

```json
{
  "mcpServers": {
    "engram": {
      "command": "/path/to/engram",
      "args": ["mcp", "--root", "/path/to/your/project"]
    }
  }
}
```

---

## Languages

Rust, Python, TypeScript, JavaScript, Go, Java, C, C++, Ruby.

All parsed with tree-sitter. Symbols extracted: functions, methods, classes, structs, enums, traits, interfaces, modules, constants. Edges extracted: CALLS, IMPORTS, INHERITS, IMPLEMENTS.

---

## 28 MCP Tools

### Structural
| Tool | What it does |
|------|-------------|
| `get_symbol` | Look up a symbol by name (fuzzy matching) |
| `find_callers` | Reverse call graph traversal (BFS, configurable depth) |
| `find_dependencies` | Forward call graph traversal |
| `search_semantic` | Hybrid RRF search (vector + BM25 + graph) |
| `get_file_summary` | All symbols in a file with signatures |
| `find_similar` | Exact clones (body hash) and near-clones (embedding cosine) |

### Historical
| Tool | What it does |
|------|-------------|
| `get_symbol_history` | Git commits that touched a symbol's file |
| `get_ownership` | File ownership from git blame |
| `get_blame` | Line-by-line blame |
| `get_evolution` | Symbol change timeline + annotation history |

### Knowledge
| Tool | What it does |
|------|-------------|
| `annotate_symbol` | Write hash-anchored annotation (auto-detects staleness) |
| `verify_annotation` | Confirm annotation is still correct |
| `downvote_annotation` | Reduce annotation confidence |
| `record_decision` | Log architectural decision with rationale |
| `record_pattern` | Record recurring pattern |

### Reasoning
| Tool | What it does |
|------|-------------|
| `get_insights` | Auto-generated warnings (god nodes, contradictions) |
| `resolve_insight` | Mark insight as resolved/dismissed |
| `get_risk_score` | (1+complexity) x (1+callers) x (1+stale annotations) |
| `get_codebase_report` | Architecture summary: languages, god nodes, stale annotations |
| `get_topic_summary` | Topic-clustered symbols |

### Agent Protocol
| Tool | What it does |
|------|-------------|
| `get_project_identity` | ~100-token project summary (L0 wake-up) |
| `get_context` | Symbol context at 4 detail tiers (brief/standard/full/deep) |
| `get_context_batch` | Multi-symbol context packed to a token budget |
| `get_module_context` | Directory-level view with community detection |
| `get_hot_path` | Most-called symbols with highest risk |
| `get_trace` | Static call chain analysis for a symbol |
| `get_exploration_map` | Which symbols have been explored vs blind spots |
| `suggest_next` | Attention-based exploration suggestions |

---

## Self-Healing Memory

This is the core differentiator. No competitor does this.

1. Agent writes annotation: *"validate_token assumes tokens are always JWT format"*
2. Annotation is anchored to `full_hash` of validate_token at write time
3. Developer modifies validate_token — hash changes — annotation marked **stale**
4. **Cascade**: BFS through the call graph. Every caller of validate_token gets a confidence reduction on their annotations. Reduction decays exponentially with graph distance.
5. If code reverts to the original hash, annotation is **reactivated**
6. Every cascade step logged in `cascade_log` for full audit trail

```
confidence_reduction = base_reduction * decay^distance
```

Graphify has no memory. MemPalace is append-only (never detects staleness). Mem0 has a 97.8% junk rate with no quality gate. Engram's memory self-corrects.

---

## Architecture

```
L7  AGENT PROTOCOL    MCP tools, hooks, progressive loading, token-budget negotiation
L6  TEMPORAL          Symbol evolution, branch context, rename tracking
L5  REASONING         Insights, contradiction detection, community clustering
L4  KNOWLEDGE         Annotations, decisions, patterns (hash-anchored + cascade)
L3  INTERACTION       Session logs, attention map, exploration suggestions
L2  HISTORICAL        Git blame, ownership, commit history
L1  STRUCTURAL        Symbols, directed edges, embeddings, scope trees, sub-chunks
L0  INGEST            File watcher, tree-sitter parser, incremental sync, content hashing
```

**Tech stack**: Rust, SQLite (WAL mode, FTS5), fastembed (AllMiniLM-L6-V2, ONNX), tree-sitter, libgit2, rmcp. Single binary with zero runtime dependencies.

**Storage**: 20 SQLite tables. One `.engram/engram.db` file per project. No external database, no Docker, no API keys.

**Search**: Three signals fused via Reciprocal Rank Fusion (RRF):
1. Vector similarity (fastembed cosine, 384 dims)
2. BM25 keyword match (SQLite FTS5, pre-built index)
3. Graph proximity (BFS expansion from high-attention symbols)

---

## Tests

119 tests across 9 test suites:

- **43 unit tests** — parser, store, cascade, embeddings, community detection, clone detection, contradiction detection
- **6 multilang integration tests** — all 9 languages parse correctly
- **7 CLI integration tests** — init, status, search, install-hooks (assert_cmd)
- **6 snapshot tests** — AST extraction pinned with insta
- **5 EngramBench tests** — MRR, Precision, Recall, ablation study, per-type breakdown
- **8 proptest cascade tests** — 1,000+ random DAGs, 5 property invariants
- **3 criterion benchmarks** — parse, index, search, cascade (11 benchmark functions)
- **1 CodeSearchNet eval** — 99 queries, 5,000 functions, NDCG@10

```bash
cargo test              # Run all tests (119 pass)
cargo bench             # Run performance benchmarks
cargo test --test codesearchnet_eval -- --ignored --nocapture  # Run CodeSearchNet (slow)
```

---

## CLI Reference

```
engram init [--root .]           Initialize Engram in a project
engram start [--root .]          Index + compute embeddings + watch for changes
engram stop                      Stop the daemon
engram status [--root .]         Show symbol/edge/file counts
engram search QUERY [--top_k 10] Hybrid search (RRF with vector fallback to BM25)
engram mcp [--root .]            Start MCP server (stdio transport)
engram install-hooks [--root .]  Configure Claude Code PreToolUse hooks
```

---

## Project Structure

```
src/
  cli/          CLI commands (init, start, search, mcp, install-hooks)
  parser/       Tree-sitter parsing, scope trees, chunking (9 languages)
  graph/        SQLite store (20 tables), schema, FTS5
  embeddings/   fastembed, vector index, RRF fusion, Embedder trait
  memory/       Self-healing cascade, progressive context loading
  mcp/          28 MCP tools via rmcp
  git/          libgit2 blame, log, ownership
  temporal/     Symbol evolution, branch context, rename detection
  intelligence/ Community detection, clone detection, contradiction detection
  watcher.rs    File watcher with 3-second debounce
tests/
  fixtures/     simple (5 files), diamond, multilang (9 langs), scope_test, cross_file, large (200 files)
  snapshots/    insta snapshot files (6 languages)
benches/        criterion benchmarks (indexing + search)
```

---

## License

MIT
