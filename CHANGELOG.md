# Changelog

## v0.1.0 (2026-04-20)

Initial release.

### Features

- **28 MCP tools** across 7 architectural layers (structural, historical, interaction, knowledge, reasoning, temporal, agent protocol)
- **9 language parsers** via tree-sitter: Rust, Python, TypeScript, JavaScript, Go, Java, C, C++, Ruby
- **Markdown/docs parsing** — README and .md files indexed alongside code for unified search
- **Self-healing memory** — hash-anchored annotations with BFS staleness cascade through call graph
- **Hybrid search** — vector (fastembed AllMiniLM-L6-V2) + BM25 (SQLite FTS5) + graph proximity, fused via Reciprocal Rank Fusion
- **Progressive context loading** — 4 tiers (brief/standard/full/deep) with token-budget batch packing
- **Deep tier includes source code** — actual file content with line numbers and inline annotations
- **Constant/config value search** — `MAX_RETRIES=3` findable by searching "3" or "MAX_RETRIES"
- **Git integration** — in-process blame, ownership, symbol history via libgit2
- **Symbol evolution tracking** — created/modified/deleted/renamed/moved with branch context
- **Community detection** — Louvain-style modularity clustering
- **Clone detection** — exact (body hash) and near (embedding cosine) code clones
- **Contradiction detection** — flags conflicting annotations on the same symbol
- **File watcher** — 3-second debounce, incremental re-index with cascade + embedding recompute
- **Claude Code hooks** — PreToolUse/Read hook for context injection
- **CLI** — init, start, stop, status, search, mcp, install-hooks

### Benchmarks

- Incremental file update: 1.6 us
- BM25 search (1,400 symbols): 50 us
- Cascade propagation: 9.3 us
- Index 200 files: 56 ms (3,571 files/sec)
- Binary size: 16 MB
- EngramBench MRR: 1.000 (hybrid RRF)
- CodeSearchNet NDCG@10: 0.154 (general-purpose model)
- Cascade: 1,003 random DAGs tested, zero failures

### Test Suite

- 176 tests across 12 test suites
- Property-based testing with proptest (1,000+ random inputs)
- Snapshot testing with insta (9 languages)
- CLI integration testing with assert_cmd
- EngramBench search quality evaluation
- CodeSearchNet external benchmark
