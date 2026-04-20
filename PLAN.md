# Engram: The Definitive Memory System for Coding Agents

> **engram** (/ˈɛnɡræm/) — a hypothetical permanent change in the brain accounting for the existence of memory; a memory trace.

## Mission

Build the best-in-market memory system specifically for coding agents — not general-purpose memory, not conversation recall, but deep, persistent, self-healing code intelligence that makes every agent session smarter than the last.

---

## Part 1: Competitive Landscape (Deep Research)

We researched every major player at the source-code level — not docs, not READMEs, but actual implementations. Here's what exists, what actually works, and where the real gaps are.

### 1.1 Graphify (github.com/safishamsi/graphify)

**What it is**: A Claude Code skill + Python library that turns folders of mixed content (code, docs, PDFs, images, screenshots) into a queryable knowledge graph. ~30K stars, 16 days old, MIT licensed. Solo developer (164/169 commits by one author). Default branch is `v4`, package name is `graphifyy`.

**Critical architectural insight**: Graphify is primarily a **skill file** (system prompt) for AI coding assistants, backed by a deterministic Python library. The AI assistant IS the orchestrator. The Python library handles only AST extraction — all "semantic extraction" (INFERRED/AMBIGUOUS edges) is performed by the LLM reading the ~50KB skill.md file. This means the "71.5x token reduction" claim depends entirely on the quality of the LLM's extraction pass, not the library itself.

**Memory model**: Pure graph (NetworkX JSON). Nodes = named entities (functions, classes, concepts). Edges = typed relationships (`calls`, `imports`, `implements`, `references`, `conceptually_related_to`). Every edge tagged as `EXTRACTED`, `INFERRED`, or `AMBIGUOUS`.

**How it works**:
```
detect() → extract() → build_graph() → cluster() → analyze() → report() → export()
```

**Code structure** (~14 core Python modules, ~11 skill markdown files):

| Module | Size | Purpose |
|--------|------|---------|
| `extract.py` | 142KB | Tree-sitter AST extraction (the bulk of codebase) |
| `__main__.py` | 58KB | CLI entry point, argument parsing, skill installation |
| `export.py` | 40KB | 7 export formats (JSON, HTML, SVG, GraphML, Cypher, Obsidian, Canvas) |
| `detect.py` | 19KB | File discovery, .graphifyignore, sensitive file filtering |
| `serve.py` | 16KB | MCP stdio server with **7 tools** (not 3 as previously estimated) |
| `analyze.py` | 21KB | God nodes, surprising connections, graph diffing |
| `build.py` | 5.3KB | NetworkX graph assembly |
| `cluster.py` | 5.4KB | Leiden/Louvain community detection |
| `cache.py` | 5.5KB | SHA256-based semantic extraction cache |
| `watch.py` | 8.5KB | Filesystem watcher with auto-rebuild |

**Tree-sitter integration details**:
- `LanguageConfig` dataclass per language: `class_types`, `function_types`, `import_types`, `call_types` as frozensets
- `_extract_generic()` (~500+ lines): Two-pass — first pass walks AST collecting classes/functions/imports, second pass (`walk_calls()`) resolves call expressions within same file
- **20+ languages** via tree-sitter packages (Python, JS/TS, Go, Rust, Java, C, C++, Ruby, C#, Kotlin, Scala, PHP, Swift, Lua, Zig, PowerShell, Elixir, Objective-C, Julia, Verilog, Vue/Svelte)
- Cross-file call resolution: `raw_calls` accumulated but actual resolution happens in the LLM orchestration layer, not the Python library
- Node ID: `_make_id()` normalizes to lowercase alphanumeric with underscores

**MCP server** (`serve.py`) — **7 tools** (not 3):

| Tool | Purpose |
|------|---------|
| `query_graph` | BFS/DFS traversal from keyword-matched nodes, with depth and token budget |
| `get_node` | Lookup node by label/ID |
| `get_neighbors` | List direct neighbors with optional relation filter |
| `get_community` | List all nodes in a community |
| `god_nodes` | Return top-N most connected nodes |
| `graph_stats` | Summary: node/edge/community counts, confidence breakdown |
| `shortest_path` | Find shortest path between two concepts |

**Query is purely keyword substring matching** — no embeddings, no vector search. An open issue (#424) proposes sentence-transformer embeddings but it hasn't been merged.

**Leiden community detection** (`cluster.py`):
- Tries `graspologic.partition.leiden()` first, falls back to `networkx.community.louvain_communities()`
- Oversized community splitting: communities >25% of graph (min 10 nodes) get re-split
- `graspologic` only works on Python < 3.13 due to upstream constraints
- Cohesion score = intra-community edge density (actual/possible ratio)

**Incremental updates**:
- SHA256 content hash cache (`cache.py`): file content + relative path hashed, cached as JSON
- `detect_incremental()` compares mtimes against stored manifest
- Git hooks: post-commit auto-rebuild, post-checkout conditional rebuild
- Filesystem watcher with 3-second debounce, code-only changes bypass LLM

**Strengths**:
- Multimodal (images via Claude vision)
- 20+ language support via tree-sitter (far more than the 13 advertised)
- 7 export formats (Obsidian, HTML/vis.js, SVG, GraphML/Gephi, Neo4j Cypher, MCP, wiki)
- SHA256 content-hash cache for incremental updates
- Filesystem watcher + git hooks for auto-rebuild
- Zero infrastructure (no server, no database, pure local JSON)
- Clean separation of concerns, good security practices (SSRF protection, path traversal guards)

**Weaknesses (confirmed by code review)**:
1. **NO semantic/vector search** — query is keyword substring matching on node labels. This is the biggest gap.
2. **Undirected graph** — uses `nx.Graph` not `nx.DiGraph`. Stores direction hints as `_src`/`_tgt` attributes (a workaround, not a solution).
3. **No cross-file entity resolution** — `_make_id()` includes file stem, so `class Config` in two files = two separate nodes. Node ID collisions when files in different directories share stems (confirmed bug #438).
4. **No temporal dimension** — graph is a snapshot. Git hook rebuilds from scratch.
5. **No garbage collection** — deleted files leave stale nodes/edges.
6. **Scale ceiling** — warns at >200 files or >2M words. HTML viz caps at 5K nodes.
7. **Cross-file call resolution incomplete** — `raw_calls` collected but resolution relies on LLM orchestration.
8. **No type resolution** — tree-sitter gives syntax, not semantics. Method calls on objects are best-effort string matching.
9. **LLM hallucination risk** — false INFERRED edges from common method names (confirmed bug #437).
10. **Sensitive file false positives** — substring matching flags directories named "secret" or "token" (#436).
11. **Absolute paths leak into artifacts** (#433).
12. **MCP server loads graph once at startup** — no hot-reload after updates.
13. **Solo developer** — 30K stars in 16 days but sustainability is a question.

---

### 1.2 MemPalace (github.com/mempalace/mempalace)

**What it is**: A local-first AI memory system giving coding agents persistent, cross-session memory using a spatial metaphor. ~48K stars, 14 days old, MIT licensed. Python. Default branch is `develop`.

**Core philosophy**: "Memory is identity." Never summarize, never paraphrase. Store your exact words, retrieve your exact words.

**Memory model**: Spatial metaphor (method of loci) + Zettelkasten + hybrid retrieval:

| Metaphor | Implementation | Purpose |
|----------|---------------|---------|
| Wing | Top-level metadata tag on drawers | A person, project, or domain |
| Room | Secondary metadata tag | A topic within a wing (detected via folder path > filename > content keywords > fallback "general") |
| Hall | Tertiary metadata tag | Content type classification (emotional, technical, family) via keyword scoring |
| Drawer | A ChromaDB document (800-char chunk) | Verbatim text with metadata — ground truth |
| Closet | Separate ChromaDB collection (`mempalace_closets`) | Compact pointer lines: `topic\|entities\|→drawer_ids`. Searchable index layer, ~1500 chars each |
| Tunnel | Cross-wing links in `~/.mempalace/tunnels.json` | Bidirectional connections between rooms across wings. Symmetric IDs ensure A-B == B-A |

**Retrieval stack — what the README says vs what the code does**:

The README claims a 4-layer fused retrieval stack (ChromaDB + BM25 + Palace graph + Temporal KG). **The reality is different**:
- ChromaDB + BM25 are tightly integrated in `searcher.py` (60% vector + 40% BM25 after min-max normalization)
- Palace graph provides navigational traversal (tunnels, room connections) but is **NOT a retrieval signal in the search scoring pipeline**
- Temporal KG is a separate SQLite database with its own query tools, also **NOT fused into search scoring**
- They are parallel systems, not a unified stack

**Actual hybrid search pipeline** (`searcher.py`):
1. Over-fetch: query drawers with `n_results * 3`
2. Closet boost: independently query closets collection; apply rank-based boosts `[0.40, 0.25, 0.15, 0.08, 0.04]` to matching drawers (only if closet distance <= 1.5)
3. BM25 re-ranking: Okapi BM25 with Lucene-smoothed IDF, k1=1.5, b=0.75 computed over candidate set
4. Hybrid scoring: 60% vector similarity + 40% BM25 (min-max normalized)
5. Keyword-best chunk expansion: for closet-matched hits, find chunk with highest query-term frequency, return with ±1 neighbors

**4-Layer Wake-Up System**:

| Layer | What | Cost |
|-------|------|------|
| L0 — Identity | Reads `~/.mempalace/identity.txt` (user-authored) | ~100 tokens |
| L1 — Essential Story | Auto-generated from highest-scoring drawers, hard-capped at 3200 chars | ~500-800 tokens |
| L2 — On-Demand | Wing/room-filtered retrieval, activated when conversation narrows | ~200-500 each |
| L3 — Deep Search | Full semantic search via ChromaDB + hybrid ranking | Unlimited |

**29 MCP tools** (actual breakdown):

| Category | Count | Tools |
|----------|-------|-------|
| Read | 6 | `status`, `list_wings`, `list_rooms`, `get_taxonomy`, `search`, `check_duplicate` |
| Write | 5 | `add_drawer`, `delete_drawer`, `get_drawer`, `list_drawers`, `update_drawer` |
| Knowledge Graph | 6 | `kg_query`, `kg_add`, `kg_invalidate`, `kg_timeline`, `kg_stats` |
| Graph Navigation | 6 | `traverse`, `find_tunnels`, `graph_stats`, `create_tunnel`, `list_tunnels`, `follow_tunnels` |
| Agent Diary | 3 | `diary_write`, `diary_read`, `get_aaak_spec` |
| Maintenance | 3 | `hook_settings`, `memories_filed_away`, `reconnect` |

**AAAK compression format** — a lossy summarization format (not compression). Original text cannot be reconstructed:
```
FILE_NUM|PRIMARY_ENTITY|DATE|TITLE          (Header)
ZID:ENTITIES|topic_keywords|"key_quote"|WEIGHT|EMOTIONS|FLAGS   (Zettel)
T:ZID<->ZID|label                           (Tunnel)
ARC:emotion->emotion->emotion               (Arc)
```
- 24 emotion codes (vul, joy, fear, trust, grief, etc.) — designed for personal journals, NOT code
- 6 importance flags: ORIGIN, CORE, SENSITIVE, PIVOT, GENESIS, DECISION

**Claude Code hooks**:
- `mempal_save_hook.sh` (PostResponse): Every N messages (default 15), blocks AI and forces diary/palace saves
- `mempal_precompact_hook.sh` (PreCompact): Always blocks, forces final save before context window compaction
- State tracked in `~/.mempalace/hook_state`

**Benchmarks — verified assessment**:

| Benchmark | Raw Score | Enhanced Score | Credibility |
|-----------|-----------|----------------|-------------|
| LongMemEval R@5 (no LLM) | 96.6% | 100% (Haiku rerank) | Strong — 50/450 train/test split, clean held-out score is 98.4% |
| LoCoMo R@10 (hybrid v5) | 84.8% → 88.9% | 100% (Sonnet rerank, top-50) | The 100% used top-k=50 exceeding actual session counts — upper bound, not practical |
| ConvoMem | 92.9% avg recall | — | Compared against Mem0's 30-45%. 50 items per category |
| MemBench (ACL 2025) | 80.3% R@5 | — | 8,500 items. Less impressive |

**The 96.6% baseline is defensible** — requires only Python + ChromaDB, no API keys. All results include raw JSONL files with every question and retrieval candidate for full auditability. They are transparent about where they taught to the test.

**Strengths**:
- Verbatim storage (never loses information)
- Zero API dependency with 96.6% recall
- 29 MCP tools with write-ahead log
- Temporal KG with point-in-time queries
- 13 language entity detection (i18n)
- Agent diaries (each agent gets its own wing)
- Claude Code hooks (save on stop + save before context compression)
- Security hardening (path traversal prevention, query sanitization)

**Weaknesses (confirmed by code review)**:
1. **ZERO code structure awareness** — treats `.rs`, `.py`, `.go` as plain text. Functions split arbitrarily at 800-char boundaries with 100-char overlap. No AST, no symbols, no call graphs.
2. **AAAK format is emotion-oriented** — 24 emotion codes designed for personal journals, not code comprehension.
3. **Closets only for project files** — conversation-mined wings lack the index layer.
4. **First 5K chars only** for closet extraction — back-of-file content invisible.
5. **Substring room routing** — `detect_room()` uses substring matching: "views" matches "interviews" (confirmed bugs #1002/#1004). Systemic misrouting in monorepos.
6. **SIGSEGV on parallel ChromaDB inserts** (#991) — HNSW multi-threaded inserts cause Rust compactor segfaults. Fix proposed but not shipped.
7. **HNSW/SQLite drift** (#1000) — Segments drift causing SIGSEGV on read. Quarantine mechanism proposed but not implemented.
8. **SQLite variable limit crash** (#1015/#1016) — `mempalace status` crashes on palaces with >32K drawers.
9. **None metadata crashes** (#1019/#1020) — ~9 unguarded sites cause `AttributeError` cascades.
10. **No incremental embeddings** — re-mining deletes + recreates all drawers per file.
11. **Palace graph is decorative for search** — graph traversal and tunnel system exist but do NOT feed into search ranking.
12. **KG is disconnected from search** — temporal KG has its own query tools but does not influence drawer retrieval scoring.
13. **Negative similarity scores** (#988) — distance > 1.0 produces negative values with no clamping.
14. **BM25 is not a pre-built index** — computed over candidate set per query (fine for small sets, won't scale).
15. **32K drawer limit** suggests system hasn't been tested at scale.

---

### 1.3 code-chunk (github.com/supermemoryai/code-chunk)

**What it is**: An AST-based code chunking library for RAG pipelines. TypeScript (Bun monorepo), 178 stars, 4 months old, MIT licensed. A focused primitive, not a platform. Version 0.1.14, only 30 commits.

**How it chunks** — 5-stage pipeline:
```
Parse (tree-sitter WASM) → Extract Entities → Build Scope Tree → Greedy Window Chunking → Enrich Context
```

**Stage 1 — Parse**: Two parser backends:
- **Native** (`parser/index.ts`): `web-tree-sitter` (WASM) for any JS runtime. Grammar objects cached in `Map<Language, TSLanguage>`. TypeScript always uses TSX grammar (so JSX syntax works in `.ts` files).
- **WASM/Cloudflare Workers** (`parser/wasm.ts`): Accepts user-provided WASM binaries as `ArrayBuffer`/`Uint8Array`/`Response`/`WebAssembly.Module`/URL. Exists because Workers can't use `require.resolve()`.

**Stage 2 — Extract**: Two strategies:
- Primary: Tree-sitter S-expression queries (adapted from **Zed editor's** `outline.scm` — battle-tested, embedded as string constants in `extract/queries.ts`)
- Fallback: Node-type DFS matching (explicit stack, not recursion) against hardcoded `ENTITY_NODE_TYPES` map
- Per entity: name, type, signature, docstring, byte/line ranges, parent name, raw AST node reference

**Stage 3 — Scope Tree** (`scope/tree.ts`):
- Entities (excluding imports/exports) sorted by byte offset
- For each entity, DFS through existing roots to find deepest containing node (`outer.start <= inner.start && inner.end <= outer.end`)
- Result: `ScopeTree` with root nodes, flat `imports`/`exports` arrays, `allEntities`
- `getAncestorChain` walks parent pointers for scope chain
- Weakness: O(n*m) where n = entities, m = tree depth (acceptable since code rarely nests >5-6 levels)

**Stage 4 — Greedy Window Chunking** (`chunking/index.ts`):
- **NWS cumulative sum** (`Uint32Array` of length `code.length + 1`): `cumsum[i]` = count of non-whitespace chars in `code[0..i-1]`. Built in O(n) pass. Any `charCodeAt() <= 32` = whitespace.
- **O(1) range queries**: `getNwsCountForNode(node, cumsum) = cumsum[node.endIndex] - cumsum[node.startIndex]`
- Three cases per node:
  - **Fits**: add to current window
  - **Oversized** (nodeSize > maxSize): recursively descend into children. Leaf nodes get line-boundary splitting via `splitOversizedLeafByLines`
  - **Doesn't fit**: yield current window, start new one
- `mergeAdjacentWindows` second pass: combines consecutive small windows
- `rebuildText` converts windows back to source via byte slicing

**IMPORTANT**: `maxChunkSize` (default 1500) counts **non-whitespace characters**, NOT bytes. This is documented as "bytes" but is actually NWS chars. A chunk can be much larger in actual bytes due to indentation. No tokenizer integration exists.

**Stage 5 — Context Enrichment** (`context/format.ts`):
```
# <filepath last 3 segments>
# Scope: <ancestor1 > ancestor2 > ... > current>
# Defines: <signature1>, <signature2>, ...
# Uses: <import1>, <import2>, ...              (up to 10)
# After: <sibling1>, <sibling2>                (up to 3)
# Before: <sibling3>, <sibling4>               (up to 3)

# ...                                          (overlap marker)
<last N lines of previous chunk>               (overlap text, default 10 lines)
# ---
<actual chunk code>
```

- **Scope**: `findScopeAtOffset` at byte range start only — cross-scope chunks get wrong scope
- **Entities**: filtered by overlap with chunk byte range, each gets `isPartial` flag
- **Siblings**: ±`maxSiblings` (default 3) at same scope level, sorted by distance
- **Imports**: ALL file imports by default. If `filterImports=true`, regex identifier extraction from signatures
- **Overlap**: only in `contextualizedText`, not `text`. Raw `text` has no boundary context

**Effect-TS usage**: Hard production dependency (~300KB). Used for error handling + concurrency, but many usages are thin wrappers around synchronous operations (`Effect.sync` wrapping pure functions). Primary value: batch concurrency model and error channel typing.

**Language support** (6 languages, quality assessment):
- **TypeScript/JavaScript**: Best supported. Rich Zed queries. JSX works via TSX grammar. JSDoc extraction works.
- **Python**: Good. Handles docstrings, decorators, async functions. Note: methods typed as `function`, not `method`.
- **Rust**: Good. Structs, enums, traits, impl blocks, doc comments. Note: `impl` blocks typed as `class` (semantically wrong).
- **Go**: Good. Functions, methods with receivers, interfaces, struct fields.
- **Java**: Good. Records (14+), enums with constants, annotations, inner classes. Javadoc works.
- **Missing**: C, C++, C#, Ruby, PHP, Swift, Kotlin, Scala, Haskell, Lua, Shell, SQL, markup/config languages.

**Strengths**:
- NWS-based sizing (indentation doesn't waste budget)
- Recursive oversized handling (AST-aware splitting at every level)
- Scope-aware context headers (dramatically improves embedding quality)
- Partial entity detection with `isPartial` flag
- Dual backend (native + WASM) for server and edge deployment
- Tree-sitter queries from Zed editor are battle-tested
- Streaming API with controlled concurrency

**Weaknesses (confirmed by code review)**:
1. **Only 6 languages** — no C/C++, C#, Ruby, PHP, Kotlin, Swift.
2. **NWS vs bytes confusion** — `maxChunkSize` documented as bytes, actually measures NWS chars.
3. **No token-based sizing** — critical gap for LLM use cases. NWS chars ≠ tokens.
4. **Scope detection uses only start byte** — chunks spanning scope boundaries get wrong scope.
5. **No cross-file awareness** — each file chunked independently. Import resolution just extracts source path string.
6. **No graph** — flat chunks, can't traverse relationships.
7. **No incremental updates** — 1 line change = re-chunk entire file.
8. **Import filtering is naive** — regex identifier extraction produces false positives/negatives.
9. **Python methods not typed as `method`** — tree-sitter Python grammar uses `function_definition` for both.
10. **Rust `impl_item` typed as `class`** — semantically incorrect.
11. **Line splitting is O(n)** — `splitOversizedLeafByLines` uses `code.slice(0, offset).split('\n').length` repeatedly.
12. **Effect overhead** — ~300KB dependency for mostly synchronous wrapper usage.
13. **Bun-centric** — primary target, Node.js via dist build only.
14. **No CLI** — library only, must be used programmatically.
15. **Very young** — v0.1.14, 30 commits, 2 issues. Limited community testing.

---

### 1.4 Mem0 (github.com/mem0ai/mem0)

**What it is**: The most established AI memory system. 53K+ stars, 6K+ forks, 307 contributors, 2,135 commits, Apache 2.0. Originally "embedchain" (~2023). Python 58.7%, TypeScript 31.0%. Published paper: arXiv:2504.19413 (2025). Has both open-source SDK and commercial platform (mem0.ai).

**Critical architectural insight**: Mem0's entire intelligence is in the LLM prompt. The code is plumbing — embed, store, retrieve, call LLM. A ~940-line `ADDITIVE_EXTRACTION_PROMPT` drives fact extraction. There is no algorithmic memory management; the LLM decides what to extract, what to keep, and what to link.

**Core pipeline** — V3 Phased Batch Pipeline (8 phases):
```
Phase 0: Context Gathering (last 10 messages from SQLite session)
Phase 1: Existing Memory Retrieval (embed messages → vector search → top-10 similar)
Phase 2: LLM Extraction (single call with ~940-line prompt → JSON array of ADD operations)
Phase 3: Batch Embedding (embed all extracted facts)
Phase 4-5: Dedup (MD5 hash check against existing + within-batch)
Phase 6: Batch Persist (insert vectors + payloads into vector store)
Phase 7: Entity Linking (spaCy NLP → embed entities → match at ≥0.95 threshold)
Phase 8: Save Messages (raw messages to SQLite)
```

**Memory model**: Pure vector with entity overlay — NOT a graph despite marketing.

Each memory:
```python
MemoryItem(
    id: str,              # UUID
    memory: str,          # Extracted fact (15-80 words)
    hash: str,            # MD5 of text for dedup
    metadata: dict,       # user_id, agent_id, run_id
    score: float,         # Similarity score (on retrieval)
    created_at: str,
    updated_at: str,
)
```

Entity store is a separate vector collection with `linked_memory_ids` arrays in payloads — NOT a graph database, just flat vector index with ID references.

**Retrieval** — 9-step hybrid search:
1. Preprocess: lemmatize query (spaCy), extract entities
2. Embed query
3. Semantic search: vector similarity, over-fetches at 4x `top_k` (min 60)
4. Keyword search: BM25 via vector store native full-text search
5. BM25 scoring: sigmoid normalization with query-length-adaptive parameters
6. Entity boosts: search entity store, add spread-attenuated boosts to linked memories
7. Candidate building from semantic results
8. Score and rank: `combined = (semantic + bm25 + entity_boost) / max_possible`
9. Format and return

Scoring normalizes to [0, 1] where `max_possible` = 1.0 (semantic) + 1.0 (BM25) + 0.5 (entity). Default threshold: 0.1.

**Graph memory — the truth**: Graph memory has been **largely removed from the open-source version**. No `graph_store` directory exists. Only remnants in test mocks, an example using Neptune Analytics, and exception classes mentioning `kuzu`. The `MemoryConfig` class has no `graph_store` field. V2's ADD/UPDATE/DELETE operations replaced by V3's ADD-only with memory linking. Graph search exists in the **commercial platform** only, not the OSS SDK.

**Massive ecosystem** (the real strength):
- **34 vector stores**: Qdrant (default), Chroma, Pinecone, Milvus, Weaviate, FAISS, PgVector, Redis, Elasticsearch, OpenSearch, MongoDB, Supabase, Azure AI Search, Cassandra, Turbopuffer, S3 Vectors, Neptune, Valkey, Databricks, Baidu, Langchain, Azure MySQL, Upstash, Vertex AI
- **17 LLM providers**: OpenAI (default), Anthropic, Ollama, Groq, Together, LiteLLM, Azure, Gemini, DeepSeek, AWS Bedrock, MiniMax, xAI, Sarvam, LM Studio, vLLM, Langchain
- **11 embedding providers**: OpenAI (default), Ollama, HuggingFace, Azure, Gemini, VertexAI, Together, LM Studio, Langchain, AWS Bedrock, FastEmbed
- **Rerankers**: Cohere, HuggingFace, LLM-based, Sentence Transformer, Zero Entropy

**MCP Server**: Ships via `openmemory` sub-project. Provides `add_memories` and `search_memories` as MCP tools via FastMCP. SSE and Streamable HTTP transports. Only 2 tools (vs our 28).

**Pricing**:
- Open source: Apache 2.0, self-hosted with your own LLM keys
- Hobby (free): 10K add/month, 1K search/month
- Starter ($19/mo): 50K add, 5K search
- Pro ($249/mo): 500K add, 50K search
- Enterprise (custom): Unlimited, on-prem, SSO, SLA, SOC 2 & HIPAA

**Benchmarks** (from paper arXiv:2504.19413):

| Benchmark | Score | Note |
|-----------|-------|------|
| LoCoMo | 91.6 | +20 over previous version |
| LongMemEval | 93.4 | +26 points |
| vs OpenAI Memory | +26% relative | LLM-as-a-Judge metric |
| Latency | 91% lower P95 | vs full-context approach |
| Token savings | 90%+ | vs full-context |

These are **end-to-end QA accuracy metrics** (BLEU, F1, LLM-as-a-Judge), NOT retrieval recall. Direct comparison to MemPalace's retrieval recall numbers is apples-to-oranges.

**Strengths**:
- Massive ecosystem (34 vector stores, 17 LLMs, 11 embedding providers)
- 53K stars, 307 contributors — most established player
- Excellent SDK ergonomics (Python + TypeScript)
- Entity linking with spread-attenuated boosts (interesting retrieval signal)
- Commercial platform with graph memory, webhooks, exports
- Published paper with benchmark numbers
- Strong brand recognition

**Weaknesses (confirmed by code review — devastating for code use cases)**:
1. **ZERO code awareness** — no AST parsing, no language detection, no code structure understanding, no file/function/class entity types. Extraction prompts target personal facts, preferences, relationships. The word "code" appears only as "stat blocks, code, or any structured information."
2. **97.8% junk rate in production** (Issue #4573) — documented across 10,134 entries in 32 days. Only 38/10,134 entries were clean enough to retain. Switching LLMs (gemma2 to Claude Sonnet) barely helped. Root cause: no quality gate before storage, no feedback loop prevention.
3. **Feedback loop vulnerability** — recalled memories get re-extracted as new entries. A single hallucination ("User prefers Vim") became 808 entries through recall-extract-store cycles.
4. **No REJECT action** — pipeline only supports ADD. V3 is ADD-only, even more permissive than V2's ADD/UPDATE/DELETE/NONE. No "this is noise, do not store" operation.
5. **No staleness detection** — memories have timestamps but no TTL, no decay, no confidence scoring, no automatic invalidation. System never questions whether old memories are still valid.
6. **Graph memory removed from OSS** — the competitive differentiator is platform-only. OSS has entity-linking overlay, not a real knowledge graph.
7. **Heavy LLM dependency for writes** — every `add()` requires LLM round-trip for extraction. Adds latency, cost, and non-determinism. Quality depends entirely on LLM interpretation of a 940-line prompt.
8. **Memory entries too small** — extraction targets 15-80 words per fact (Issue #2762). Large documents get reduced to single sentences.
9. **No identity-aware extraction** — doesn't reliably distinguish AI agents from humans, leading to hallucinated user profiles.
10. **PostHog telemetry always on** — not optional, always included as core dependency.
11. **MCP server has only 2 tools** — `add_memories` and `search_memories`. No structural queries, no graph traversal, no context loading.
12. **MemPalace benchmark comparison is misleading** — MemPalace's 92.9% recall vs Mem0's "30-45%" compares retrieval recall to QA accuracy (different metrics). Both communities have flagged this.

---

### 1.5 Competitive Matrix (Updated with confirmed data)

| Capability | Graphify | MemPalace | code-chunk | Mem0 | **Engram** |
|-----------|----------|-----------|------------|------|------------|
| Code AST understanding | tree-sitter (20+ langs) | None | tree-sitter (6 langs) | None | **tree-sitter (9+ langs)** |
| Directed call graph | No (undirected nx.Graph) | No | No | No | **Yes (directed edges table)** |
| Cross-file entity resolution | No (file-scoped IDs) | No | No (file-local only) | No | **Yes (canonical_id)** |
| Vector semantic search | No (keyword substring only) | ChromaDB (cosine HNSW) | Produces embeddings | Yes (34 vector stores) | **fastembed (local ONNX)** |
| BM25 keyword search | No | Yes (per-query, not pre-built) | No | Yes (vector store native) | **Yes (SQLite FTS5, pre-built)** |
| Graph-proximity search | BFS/DFS only | Decorative (not fused into search) | No | Entity boost (not graph) | **Yes (weighted BFS, fused via RRF)** |
| Hybrid fusion | No | Partial (vector+BM25, graph disconnected) | No | Yes (semantic+BM25+entity boost) | **RRF (vector+BM25+graph)** |
| Temporal tracking | No (snapshot) | valid_from/valid_to KG (separate) | No | Timestamps only (no decay) | **Symbol evolution + branch context** |
| Self-healing memory | No memory | Append-only (no staleness) | No memory | No (97.8% junk rate reported) | **Hash-anchored cascade** |
| Token-budget retrieval | No | 4-layer wake-up (conversation) | No | No | **Budget-aware batch packing (code)** |
| Scope-enriched context | No | No | Yes (contextualizedText) | No | **Yes (embeddings + retrieval)** |
| LLM dependency (core) | Yes (non-code extraction) | No | No | **Yes (every add() call)** | **No** |
| Memory quality control | Schema validation only | Verbatim (no extraction) | N/A | No quality gate (feedback loops) | **Hash-anchored + cascade** |
| Infrastructure | Python + NetworkX | Python + ChromaDB | TS + WASM + Effect | Python + LLM + vector DB | **Single Rust binary** |
| MCP tools | 7 | 29 | 0 | 2 | **28** |
| Maturity | Solo dev, 16 days | 14 days, SIGSEGV bugs | 30 commits | **53K stars, 307 contributors** | **—** |

---

## Part 2: What We're Building

### 2.1 One-Line Pitch

**Engram is a persistent codebase intelligence daemon that gives coding agents structural understanding, semantic search, temporal awareness, and self-healing memory — in a single local binary with zero dependencies.**

### 2.2 Why This Wins

The market has:
- **Structural tools** (Graphify) that can't search semantically — keyword substring matching only
- **Memory tools** (MemPalace) that can't understand code — treats all files as plain text, splits at 800-char boundaries
- **General-purpose memory** (Mem0) that has zero code awareness, a 97.8% junk rate in production, and requires an LLM for every write
- **Chunking tools** (code-chunk) that are just primitives — no memory, no persistence, no agent integration

Nobody combines all four pillars:
1. **Deep code structure** — directed AST graphs, cross-file entity resolution, scope trees
2. **Hybrid retrieval** — vector + BM25 + graph proximity, truly fused with RRF (not parallel disconnected systems like MemPalace)
3. **Temporal awareness** — how code evolved, branch-aware memory, decision timeline
4. **Self-healing memory** — hash-anchored annotations with staleness cascade through the call graph

Plus the distribution advantage: **single Rust binary, zero runtime dependencies, sub-50ms incremental updates**. No Python, no JVM, no Docker, no API keys for the core path.

### 2.3 What We Take From Each Competitor

**From Graphify**:
- Leiden community detection for topology-based module clustering
- Confidence provenance on edges (EXTRACTED/INFERRED/AMBIGUOUS)
- Wiki-style export for agent-readable navigation
- Multimodal: parse Markdown docs and READMEs, not just code
- `_make_id()` normalization approach (but with cross-file dedup)
- Filesystem watcher with debounce + git hook integration

**From MemPalace**:
- L0 Identity layer (~100 tokens, injected at session start)
- BM25 keyword search fused with vector search (but as pre-built FTS5 index, not per-query BM25)
- Token-budget-aware retrieval (agents specify affordance)
- Write-ahead log / append-only for crash safety and audit
- Performance budgets (<500ms hooks, <100ms startup)
- Closet-style boosting: index layer boosts specific results in retrieval
- Neighbor expansion (±1 sibling chunks when hit clips mid-thought)

**From Mem0**:
- Entity linking with spread-attenuated boosts as a retrieval signal (but integrated into RRF, not as a separate additive score)
- MD5/hash-based dedup before storage (we use SHA256, same principle)
- Multi-provider architecture lesson: design embedding layer behind a trait so models are swappable (Mem0 supports 11 embedding providers — we start with fastembed but keep the door open)
- **Anti-patterns to avoid**: no quality gate → 97.8% junk; feedback loops where recalled memories get re-extracted; ADD-only pipeline with no REJECT; 940-line LLM prompts as the entire intelligence layer; PostHog telemetry as a core dependency

**From code-chunk**:
- Scope chains on every symbol (`["auth", "AuthService", "validate"]`)
- Sub-chunking large functions at statement boundaries (recursive descent, not arbitrary char splits)
- NWS-aware sizing for fair chunk measurement (but also provide token estimates)
- Contextualized embedding text format (scope + neighbors + imports)
- Partial entity detection for split symbols with `isPartial` flag
- Tree-sitter query patterns from Zed editor's `outline.scm`
- Greedy window chunking with merge pass to reduce fragmentation

---

## Part 3: Architecture

### 3.1 Seven Layers

```
┌─────────────────────────────────────────────────────────────┐
│  L7  AGENT PROTOCOL                                         │
│      MCP tools, Claude Code hooks, progressive loading,     │
│      token-budget negotiation                                │
├─────────────────────────────────────────────────────────────┤
│  L6  TEMPORAL                                                │
│      Symbol evolution, decision timeline, branch-aware       │
│      memory, rename tracking                                 │
├─────────────────────────────────────────────────────────────┤
│  L5  REASONING                                               │
│      Insight generators, contradiction detection,            │
│      auto-summaries, exploration suggestions                 │
├─────────────────────────────────────────────────────────────┤
│  L4  KNOWLEDGE                                               │
│      Annotations, decisions, patterns — all hash-anchored    │
│      with BFS staleness cascade through call graph           │
├─────────────────────────────────────────────────────────────┤
│  L3  INTERACTION                                             │
│      Session logs, attention map, query patterns,            │
│      agent diaries                                           │
├─────────────────────────────────────────────────────────────┤
│  L2  HISTORICAL                                              │
│      Git blame, ownership, commit history,                   │
│      contributor patterns                                    │
├─────────────────────────────────────────────────────────────┤
│  L1  STRUCTURAL                                              │
│      Symbols, directed edges, embeddings, scope trees,       │
│      canonical IDs, sub-chunks                               │
├─────────────────────────────────────────────────────────────┤
│  L0  INGEST                                                  │
│      File watcher (<50ms), tree-sitter parser,               │
│      incremental sync, content hashing                       │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Tech Stack

| Component | Choice | Justification |
|-----------|--------|---------------|
| **Language** | Rust | Single binary, no runtime deps, memory safe for long-running daemon, sub-50ms proven |
| **Storage** | SQLite (WAL mode) | One file, zero ops, FTS5 built-in, recursive CTEs for BFS, handles 100K+ symbols |
| **Vector search** | fastembed (AllMiniLM-L6-V2, ONNX) | Local inference, no API key, 384 dims, in-process. Model already cached locally |
| **BM25** | SQLite FTS5 | Pre-built index (unlike MemPalace's per-query BM25). Zero additional deps |
| **AST parsing** | tree-sitter | Battle-tested, incremental, multi-language. Zed editor query patterns for extraction |
| **Git** | libgit2 (git2 crate) | In-process, no shell-out, blame/log/diff |
| **MCP** | rmcp crate | Rust-native MCP framework, stdio transport |
| **Async** | tokio | Full-featured async runtime |

**Why NOT other choices**:
- **Python**: Requires interpreter, slower, both competitors chose it and both have SIGSEGV/crash bugs
- **TypeScript**: Requires Node/Bun runtime, code-chunk already occupies this space, Effect-TS adds ~300KB overhead
- **ChromaDB/external vector DB**: Operational complexity for zero benefit at our scale. MemPalace's 441GB HNSW bloat and SIGSEGV on parallel inserts prove the risk. In-memory brute-force dot product is <10ms at 100K × 384 dims (~150MB RAM)
- **Neo4j/graph DB**: SQLite recursive CTEs handle BFS in single-digit ms. External DB adds a process to manage
- **NetworkX (via PyO3 bindings)**: Graphify's undirected graph limitation stems from choosing NetworkX

**Embedding model considerations**:
- Start with AllMiniLM-L6-V2 (384 dims, fast, proven)
- Architecture should be pluggable for future swap to code-specific models (CodeBERT, StarCoder embeddings)
- Keep embedding layer behind a trait for easy model substitution

### 3.3 Data Model

#### Core Tables (Layer 0-1: Structural)

```sql
-- Symbols: every function, class, struct, trait, interface, enum, etc.
CREATE TABLE symbols (
    id              TEXT PRIMARY KEY,           -- sha256(file:name:kind)
    canonical_id    TEXT,                       -- sha256(name:kind:body_hash) for cross-file dedup
    name            TEXT NOT NULL,
    kind            TEXT NOT NULL,              -- function, class, struct, trait, interface, enum, method
    file            TEXT NOT NULL,
    line_start      INTEGER NOT NULL,
    line_end        INTEGER NOT NULL,
    signature       TEXT,                       -- full function/class signature
    docstring       TEXT,
    body_hash       TEXT NOT NULL,              -- sha256(body) for change detection
    full_hash       TEXT NOT NULL,              -- sha256(signature + body + docstring)
    complexity      INTEGER DEFAULT 0,         -- cyclomatic estimate
    language        TEXT NOT NULL,
    scope_chain     TEXT,                       -- JSON: ["module", "ClassName", "method_name"]
    parent_id       TEXT,                       -- symbol_id of containing class/module
    updated_at      INTEGER NOT NULL
);

-- Directed edges: calls, imports, inherits, implements, uses
CREATE TABLE edges (
    from_id         TEXT NOT NULL,
    to_id           TEXT NOT NULL,
    kind            TEXT NOT NULL,              -- CALLS, IMPORTS, INHERITS, IMPLEMENTS, USES
    file            TEXT,
    line            INTEGER,
    confidence      REAL DEFAULT 1.0,          -- 1.0 = EXTRACTED, 0.7 = INFERRED, 0.3 = AMBIGUOUS
    PRIMARY KEY (from_id, to_id, kind)
);

-- Sub-chunks for large symbols (>500 tokens)
CREATE TABLE chunks (
    id              TEXT PRIMARY KEY,
    symbol_id       TEXT NOT NULL,
    chunk_index     INTEGER NOT NULL,
    content         TEXT NOT NULL,
    line_start      INTEGER NOT NULL,
    line_end        INTEGER NOT NULL,
    token_count     INTEGER NOT NULL,
    nws_count       INTEGER NOT NULL,          -- non-whitespace char count (learned from code-chunk)
    body_hash       TEXT NOT NULL,
    FOREIGN KEY (symbol_id) REFERENCES symbols(id)
);

-- File tracking for incremental sync
CREATE TABLE file_hashes (
    file            TEXT PRIMARY KEY,
    content_hash    TEXT NOT NULL,
    updated_at      INTEGER NOT NULL
);

-- Vector embeddings (in-memory index, persisted to disk)
CREATE TABLE embeddings (
    symbol_id       TEXT PRIMARY KEY,
    embedding       BLOB NOT NULL,             -- f32 × 384 dimensions
    body_hash       TEXT NOT NULL,             -- recompute when stale
    FOREIGN KEY (symbol_id) REFERENCES symbols(id)
);

-- BM25 full-text search (pre-built index, unlike MemPalace's per-query approach)
CREATE VIRTUAL TABLE fts_symbols USING fts5(
    symbol_id, name, signature, docstring, scope_chain, file
);
```

#### Historical Tables (Layer 2)

```sql
CREATE TABLE git_commits (
    hash            TEXT PRIMARY KEY,
    author          TEXT NOT NULL,
    email           TEXT,
    timestamp       INTEGER NOT NULL,
    message         TEXT
);

CREATE TABLE symbol_commits (
    symbol_id       TEXT NOT NULL,
    commit_hash     TEXT NOT NULL,
    change_type     TEXT,                      -- added, modified, deleted
    PRIMARY KEY (symbol_id, commit_hash)
);

CREATE TABLE file_ownership (
    file            TEXT NOT NULL,
    author          TEXT NOT NULL,
    email           TEXT,
    commits         INTEGER DEFAULT 1,
    last_touched    INTEGER NOT NULL,
    PRIMARY KEY (file, author)
);
```

#### Interaction Tables (Layer 3)

```sql
CREATE TABLE cortex_sessions (
    id              TEXT PRIMARY KEY,
    started_at      INTEGER NOT NULL,
    ended_at        INTEGER,
    source          TEXT,                      -- "claude-code", "cursor", "codex"
    goal            TEXT,
    summary         TEXT
);

CREATE TABLE interactions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id      TEXT,
    timestamp       INTEGER NOT NULL,
    tool_name       TEXT NOT NULL,
    query_text      TEXT,
    result_symbols  TEXT,                      -- JSON array of symbol_ids
    duration_ms     INTEGER
);

CREATE TABLE attention_map (
    symbol_id       TEXT PRIMARY KEY,
    view_count      INTEGER DEFAULT 0,
    query_count     INTEGER DEFAULT 0,
    annotate_count  INTEGER DEFAULT 0,
    last_accessed   INTEGER,
    importance_score REAL DEFAULT 0.0
);
```

#### Knowledge Tables (Layer 4)

```sql
CREATE TABLE annotations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol_id       TEXT NOT NULL,
    annotation_type TEXT NOT NULL,             -- explanation, warning, todo, context, assumption
    content         TEXT NOT NULL,
    author          TEXT DEFAULT 'agent',
    confidence      REAL DEFAULT 0.8,
    full_hash_at    TEXT NOT NULL,             -- symbol's full_hash when annotation was written
    status          TEXT DEFAULT 'active',     -- active, stale, superseded, archived
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE TABLE decisions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol_ids      TEXT NOT NULL,             -- JSON array
    description     TEXT NOT NULL,
    rationale       TEXT,
    alternatives    TEXT,                      -- JSON: what was considered and rejected
    confidence      REAL DEFAULT 0.8,
    created_at      INTEGER NOT NULL
);

CREATE TABLE patterns (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL,
    trigger_symbol_ids TEXT,                   -- JSON array
    evidence        TEXT,
    confidence      REAL DEFAULT 0.7,
    created_at      INTEGER NOT NULL
);

CREATE TABLE topics (
    topic           TEXT NOT NULL,
    symbol_id       TEXT NOT NULL,
    PRIMARY KEY (topic, symbol_id)
);
```

#### Reasoning Tables (Layer 5)

```sql
CREATE TABLE insights (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    insight_type    TEXT NOT NULL,             -- god_node, dead_code, complexity_spike, missing_docs, circular_dep
    content         TEXT NOT NULL,
    symbol_ids      TEXT,                      -- JSON array
    confidence      REAL DEFAULT 0.7,
    status          TEXT DEFAULT 'active',     -- active, resolved, dismissed
    created_at      INTEGER NOT NULL
);

CREATE TABLE cascade_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger_symbol  TEXT NOT NULL,
    affected_symbol TEXT NOT NULL,
    annotation_id   INTEGER,
    old_confidence  REAL,
    new_confidence  REAL,
    reason          TEXT,
    timestamp       INTEGER NOT NULL
);
```

#### Temporal Tables (Layer 6)

```sql
CREATE TABLE symbol_evolution (
    id              TEXT PRIMARY KEY,
    symbol_id       TEXT NOT NULL,
    commit_hash     TEXT,
    timestamp       INTEGER NOT NULL,
    change_type     TEXT NOT NULL,             -- created, modified, renamed, deleted, moved
    old_full_hash   TEXT,
    new_full_hash   TEXT,
    old_file        TEXT,
    new_file        TEXT,
    diff_summary    TEXT,
    FOREIGN KEY (symbol_id) REFERENCES symbols(id)
);

CREATE TABLE branch_context (
    branch_name     TEXT PRIMARY KEY,
    last_seen_at    INTEGER NOT NULL,
    symbol_snapshot TEXT NOT NULL              -- JSON: {symbol_id: full_hash} at branch switch
);
```

**Total: 22 tables across 7 layers.**

### 3.4 Hybrid Search: Reciprocal Rank Fusion

Three retrieval signals, truly fused (unlike MemPalace where graph/KG are disconnected from search):

```
Signal 1: Vector similarity (fastembed cosine over scope-enriched text)
Signal 2: BM25 keyword match (SQLite FTS5, pre-built index)
Signal 3: Graph proximity (weighted BFS distance from attention-hot symbols)

RRF fusion:
  final_score(d) = Σ 1/(k + rank_i(d))    for each signal i
  where k = 60 (standard RRF constant, tunable — test k=30 for code)
```

**Why RRF over weighted average**: RRF is rank-based, not score-based. It handles the incomparable scales of cosine similarity (0-1), BM25 (0-∞), and graph distance (0-N) without normalization. MemPalace uses min-max normalization for their vector+BM25 fusion (60/40 split) which requires the full result set to compute bounds.

**RRF k-parameter tuning**: Standard k=60 assumes large result sets. For code search with smaller candidate pools, k=30 may work better. We will test k ∈ {10, 30, 60, 100} via ablation on CodeSearchNet queries and our own EngramBench.

**Scope-enriched embedding text** (adapted from code-chunk's `contextualizedText` format):
```
function validate_token in AuthService in module auth
pub fn validate_token(token: &str) -> Result<Claims>
/// Validates the JWT token and extracts claims
called by: handle_request, middleware_auth
imports: jsonwebtoken, Claims
```

This gives embeddings structural awareness without a graph lookup at query time. code-chunk proved this format dramatically improves embedding quality for disambiguation queries.

### 3.5 Self-Healing Memory: The Cascade

This is our single biggest differentiator. No competitor does this.

**How it works**:
1. Agent writes annotation: "validate_token assumes tokens are always JWT format"
2. Annotation is anchored to `full_hash` of validate_token at write time
3. Developer modifies validate_token → `full_hash` changes → annotation marked `stale`
4. **Cascade**: BFS traversal through the call graph. Every caller of validate_token gets a confidence reduction on their annotations. The reduction decays with graph distance:
   ```
   confidence_reduction = base_reduction × decay^distance
   ```
5. Cascade logged in `cascade_log` for full audit trail
6. **Revert reactivation**: If code reverts to original `full_hash`, annotation status returns to `active`

**Why this matters**: Without this, agent memory silently becomes wrong. Graphify has no memory at all. MemPalace is append-only — it never detects that a stored fact became stale. Its temporal KG has `valid_from`/`valid_to` but these are set manually, not triggered by code changes. Engram's memory self-corrects automatically.

### 3.6 Progressive Context Loading

Inspired by MemPalace's 4-layer wake-up, adapted for code (MemPalace's version is conversation-oriented with emotion codes):

| Tier | Name | Tokens | What's Included |
|------|------|--------|-----------------|
| L0 | Project Identity | ~100 | Project name, language breakdown, module map, top 3 risks, recent activity |
| L1 | Symbol Brief | ~80 | Name, kind, file, signature, risk score, active warnings |
| L2 | Symbol Standard | ~500 | + top 5 callers, top 5 deps, recent annotations, git blame summary |
| L3 | Symbol Full | ~2500 | Everything: all callers, all deps, full history, all annotations with confidence |
| L4 | Symbol Deep | ~5000 | + actual source code with inline Engram annotations |

**Token-budget batch mode**: `get_context_batch(symbols=["A","B","C"], budget=3000)`
- Start with L1 for all symbols
- Greedily upgrade highest-attention symbol to next tier
- Repeat until budget exhausted
- Returns maximally informative output for the given budget
- **Metric**: useful_tokens / budget_tokens ratio. Target: >0.7

---

## Part 4: MCP Tools (28 total)

### Structural (Layer 1)
| Tool | Purpose |
|------|---------|
| `get_symbol` | Symbol lookup with fuzzy matching |
| `find_callers` | Reverse call graph (BFS, configurable depth) |
| `find_dependencies` | Forward dependency traversal |
| `search_semantic` | Hybrid RRF search (vector + BM25 + graph) |
| `get_file_summary` | File overview: symbols, complexity, imports |
| `find_similar` | Structurally + semantically similar code |

### Historical (Layer 2)
| Tool | Purpose |
|------|---------|
| `get_symbol_history` | Git commit history for a symbol |
| `get_ownership` | File ownership by contributor |
| `get_blame` | Line-by-line git context |
| `get_evolution` | Symbol change timeline with annotation history |

### Interaction (Layer 3)
| Tool | Purpose |
|------|---------|
| `get_exploration_map` | Coverage analysis + blind spots |
| `suggest_next` | Attention-based exploration suggestions |

### Knowledge (Layer 4)
| Tool | Purpose |
|------|---------|
| `annotate_symbol` | Write persistent knowledge (hash-anchored) |
| `verify_annotation` | Confirm annotation correctness |
| `downvote_annotation` | Reduce annotation confidence |
| `record_decision` | Log architectural decision |
| `record_pattern` | Record recurring pattern |

### Reasoning (Layer 5)
| Tool | Purpose |
|------|---------|
| `get_insights` | Auto-generated warnings (god nodes, risk, gaps) |
| `resolve_insight` | Dismiss or mark insight resolved |
| `get_risk_score` | Risk scoring (complexity × callers × staleness) |
| `get_codebase_report` | Architecture summary |
| `get_topic_summary` | Topic-clustered symbols & annotations |

### Temporal (Layer 6)
| Tool | Purpose |
|------|---------|
| `get_evolution` | How has this symbol changed? Commit diffs + annotation timeline |

### Agent Protocol (Layer 7)
| Tool | Purpose |
|------|---------|
| `get_project_identity` | L0 wake-up: ~100 token project summary |
| `get_context` | All-in-one (brief/standard/full/deep) |
| `get_context_batch` | Multi-symbol within token budget |
| `get_module_context` | Directory-level: symbols, health, annotations |
| `get_hot_path` | Execution trace analysis |
| `get_trace` | Runtime trace data |

---

## Part 5: Agent Integration

### 5.1 MCP Server

```bash
engram mcp --root /path/to/project
```

Stdio transport. Compatible with Claude Code, Cursor, Codex, Gemini CLI, any MCP client.

### 5.2 Claude Code Hooks

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Read",
        "hooks": [{
          "type": "command",
          "command": "engram context-for-file --brief --file $TOOL_INPUT_FILE"
        }]
      }
    ]
  }
}
```

**PreToolUse/Read hook**: Before Claude reads a file, inject a brief context header ("AuthService — god node, 34 callers, stale annotation on validate_token"). Agent gets Engram intelligence without explicitly asking.

**Session start**: Auto-call `get_project_identity` so every session begins with project awareness.

**Post-edit**: File watcher detects change → cascade fires → stale annotations flagged → next query reflects updated state.

### 5.3 CLI

```bash
engram init                    # Initialize in current repo
engram start                   # Start daemon (background)
engram stop                    # Stop daemon
engram status                  # Show indexing state
engram search "authentication" # CLI search
engram install-hooks           # Configure Claude Code hooks
engram mcp                     # Start MCP server (stdio)
```

---

## Part 6: Testing & Benchmarking Strategy

### 6.1 Why Testing Is Critical

Both top competitors have serious stability issues discovered through code review:
- **MemPalace**: SIGSEGV on parallel ChromaDB inserts (#991), SQLite variable limit crashes at 32K+ drawers (#1015), HNSW/SQLite drift causing SIGSEGV on read (#1000)
- **Graphify**: Node ID collisions across directories (#438), absolute paths leaking into artifacts (#433), MCP server with no hot-reload

We must be more stable than both. Testing is baked in from day 1, not bolted on later.

### 6.2 Existing Benchmarks Assessment

| Benchmark | What It Tests | Applicable to Engram? |
|-----------|--------------|----------------------|
| **LongMemEval** | 500 questions across 5 long-term memory abilities (extraction, multi-session reasoning, knowledge updates, temporal reasoning, abstention) | Yes — adaptable. Knowledge updates maps to staleness cascade. Temporal reasoning maps to symbol evolution. MemPalace's 96.6% R@5 is the number to beat. |
| **LoCoMo** | 1,986 questions on session-based conversational memory at R@10 | Partially — session-based retrieval structure is analogous to cross-session code intelligence |
| **ConvoMem** | 250 items across 5 memory categories | No — purely conversational, not worth adapting |
| **MemBench (ACL 2025)** | 8,500 items, R@5 | No — conversation-focused |
| **CodeSearchNet** | 2M (comment, code) pairs, 6 languages, 99 NL queries with ~4K expert relevance annotations. NDCG metric | **Yes — highly relevant**. Index corpus, run 99 queries, compare NDCG. Ablation: vector-only vs BM25-only vs graph-only vs RRF |
| **CoIR** | 10 code datasets, 8 retrieval tasks, 7 domains, ~2M docs. MTEB-compatible (MRR, NDCG@k, Recall@k) | **Yes — even more relevant**. Code-to-code, cross-language, StackOverflow Q&A tasks |

**Critical gap**: No existing benchmark tests code structure retrieval + annotation staleness + temporal awareness + hybrid search over code graphs. We must build our own.

### 6.3 EngramBench: Our Custom Benchmark

**Build our own code memory benchmark** — this becomes both our testing infrastructure and our competitive moat.

**Design**:
1. Select 3-5 open-source Rust repos of varying sizes:
   - Small: `ripgrep` (~50 files) — well-structured, single-purpose
   - Medium: `axum` (~200 files) — web framework with clear module boundaries
   - Large: `tokio` (~500 files) — complex, multi-crate workspace
2. For each repo, manually create 20-30 queries with ground-truth relevant symbols:

| Query Type | Example | What It Tests |
|------------|---------|---------------|
| Name query | "find the `Router` struct" | Exact/fuzzy symbol lookup |
| Semantic query | "error handling middleware" | Vector search quality |
| Structural query | "what calls the database connection pool" | Graph traversal + search fusion |
| Scope-disambiguated | "the `validate` method inside `AuthService`" | Scope-enriched embedding quality |
| Import-aware | "function that uses jsonwebtoken" | Import context in embeddings |
| Temporal query | "what changed in the auth module recently" | Symbol evolution retrieval |
| Annotation query | "symbols with stale annotations" | Self-healing memory correctness |
| Cross-file | "all implementations of the `Handler` trait" | Cross-file entity resolution |

3. **Metrics computed per query set**:
   - **MRR** (Mean Reciprocal Rank) — where does the first relevant result appear?
   - **NDCG@k** (k=5, 10, 20) — accounts for ranking quality with graded relevance
   - **Precision@5** — of the top 5 results, how many are relevant?
   - **Recall@10** — of all relevant results, how many are in the top 10?

4. **Ablation study**: Run each signal independently and the fused result:
   - Vector-only → MRR, NDCG@10
   - BM25-only → MRR, NDCG@10
   - Graph-only → MRR, NDCG@10
   - RRF fusion → MRR, NDCG@10
   - Per-query analysis: which signal contributed the winning result

5. **RRF k-parameter sweep**: Test k ∈ {10, 30, 60, 100}. Publish results.

6. **Regression gate**: Every PR must not regress any metric by more than 2%.

### 6.4 Rust Testing Stack

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
proptest = "1.4"
insta = { version = "1.38", features = ["yaml"] }
rstest = "0.19"
tempfile = "3"
assert_cmd = "2"
predicates = "3"
```

| Tool | Purpose | Use in Engram |
|------|---------|---------------|
| **`cargo test`** | Built-in unit + integration tests | Core test runner |
| **`criterion`** | Statistical micro-benchmarking with regression detection | Indexing speed, search latency, cascade throughput |
| **`proptest`** | Property-based testing with automatic shrinking | Graph invariants, cascade correctness, DAG edge-cases |
| **`insta`** | Snapshot testing (`cargo insta review` to approve changes) | AST parsing output, search result formatting, MCP response shapes |
| **`rstest`** | Parameterized test fixtures | Test across multiple languages, repo sizes |
| **`tempfile`** | Temporary directories for test repos | Isolation for indexing tests |
| **`assert_cmd`** | CLI integration testing | Test `engram init`, `engram search`, etc. |

### 6.5 Test Categories

#### A. Unit Tests (per module)

Every module gets unit tests. Key areas:

**Parser tests** (snapshot with `insta`):
```rust
#[test]
fn test_parse_rust_function() {
    let symbols = parse_file("tests/fixtures/sample.rs");
    insta::assert_yaml_snapshot!(symbols);
}
// Repeat for each supported language
```

**Edge extraction tests**:
```rust
#[test]
fn test_extract_call_edges() {
    let (symbols, edges) = parse_file_with_edges("tests/fixtures/calls.rs");
    assert!(edges.iter().any(|e| e.from_name == "handler" && e.to_name == "validate_token"));
    assert!(edges.iter().all(|e| e.kind == EdgeKind::Calls));
}
```

**Scope chain tests**:
```rust
#[test]
fn test_scope_chain_nested_method() {
    let symbols = parse_file("tests/fixtures/nested.rs");
    let method = symbols.iter().find(|s| s.name == "inner_method").unwrap();
    assert_eq!(method.scope_chain, vec!["mod_name", "OuterClass", "InnerClass", "inner_method"]);
}
```

#### B. Property-Based Tests (cascade correctness)

Using `proptest`, generate random DAGs and verify cascade invariants:

```rust
proptest! {
    #[test]
    fn cascade_terminates(dag in arb_dag(10..1000, 0.1..0.3)) {
        let result = run_cascade(&dag, random_mutation(&dag));
        // Cascade must always terminate
        prop_assert!(result.is_ok());
    }

    #[test]
    fn cascade_no_negative_confidence(dag in arb_dag(10..100, 0.1..0.5)) {
        let result = run_cascade(&dag, random_mutation(&dag)).unwrap();
        for annotation in result.annotations {
            prop_assert!(annotation.confidence >= 0.0);
            prop_assert!(annotation.confidence <= 1.0);
        }
    }

    #[test]
    fn cascade_no_duplicate_visits(dag in arb_dag(10..100, 0.1..0.5)) {
        let log = run_cascade(&dag, random_mutation(&dag)).unwrap().log;
        let affected: Vec<_> = log.iter().map(|e| &e.affected_symbol).collect();
        let unique: HashSet<_> = affected.iter().collect();
        prop_assert_eq!(affected.len(), unique.len());
    }
}
```

**Specific cascade correctness tests**:

```
Test: single_hop_cascade
  Setup: A calls B. Annotate B. Change B's body.
  Assert: B's annotation marked stale. A's annotation confidence reduced by base_reduction * decay^1.

Test: multi_hop_cascade
  Setup: A calls B calls C. Annotate C. Change C.
  Assert: C stale. B reduced by decay^1. A reduced by decay^2.

Test: diamond_cascade
  Setup: A calls B, A calls C, B calls D, C calls D. Annotate D. Change D.
  Assert: D stale. B and C both reduced. A reduced once (not double-counted via shortest path).

Test: cascade_does_not_cross_unchanged
  Setup: A calls B calls C. Annotate A (about A, not C). Change C.
  Assert: C's annotations stale. B reduced. A's annotation about A is NOT affected
          (cascade only affects annotations whose content is about the changed symbol or its callers).

Test: revert_reactivates
  Setup: Annotate B. Change B (annotation becomes stale). Revert B to original hash.
  Assert: Annotation reactivated (full_hash matches full_hash_at again).

Test: cascade_log_audit_trail
  Setup: Run any cascade.
  Assert: cascade_log contains every affected symbol with old/new confidence and reason.
```

#### C. Incremental Indexing Tests

```
Test: full_index_equals_incremental
  Setup: Index repo from scratch → snapshot all tables.
  Then: Delete DB. Index file-by-file incrementally.
  Assert: Both DBs produce identical symbol tables, edges, embeddings.

Test: edit_only_reindexes_changed
  Setup: Index repo. Record symbol count.
  Edit one file (add a function).
  Assert: Only that file's symbols re-parsed. Total symbols = old + 1.

Test: delete_removes_symbols
  Setup: Index repo. Delete a file.
  Assert: All symbols from that file removed. Edges referencing them removed.
  Annotations on those symbols marked with appropriate status.

Test: rename_preserves_annotations
  Setup: Annotate function F in file A. Rename file A to B.
  Assert: Annotation survives, attached to same canonical_id.

Test: idempotency
  Setup: Index repo. Run indexing again on same unchanged repo.
  Assert: Zero symbols updated, zero embeddings recomputed, zero file_hashes changed.
```

#### D. Search Quality Tests

**Scope-enriched embeddings A/B evaluation**:
1. Index the same codebase twice: plain embeddings (`name + body`) vs scope-enriched (`scope_chain + signature + docstring + callers + imports`)
2. Run identical query set against both
3. Compare NDCG@10 and MRR
4. **Hypothesis**: scope-enriched should win significantly on disambiguation queries

**Specific search tests**:
```
Test: ambiguous_names
  Setup: Two functions named `process()` in different classes.
  Query: "process data in the pipeline module"
  Assert: scope-enriched ranks the pipeline one higher than plain embeddings.

Test: import_aware
  Setup: Function that uses jsonwebtoken crate.
  Query: "function that uses jsonwebtoken"
  Assert: scope-enriched embeddings (which include import context) rank it in top 3.

Test: rrf_beats_single_signal
  Setup: Index EngramBench repos.
  Assert: RRF MRR > max(vector_MRR, bm25_MRR, graph_MRR) on at least 60% of queries.
```

#### E. MCP Tool Integration Tests

In-process tests that spin up MCP server on test transport:

```rust
#[tokio::test]
async fn test_get_symbol_fuzzy_match() {
    let server = start_test_server(test_repo_path()).await;
    let result = server.call_tool("get_symbol", json!({"name": "validte_tokn"})).await;
    assert_eq!(result.symbols[0].name, "validate_token");
}

#[tokio::test]
async fn test_search_semantic_respects_budget() {
    let server = start_test_server(test_repo_path()).await;
    let result = server.call_tool("get_context_batch", json!({
        "symbols": ["A", "B", "C"],
        "budget": 500
    })).await;
    assert!(token_count(&result) <= 500);
}

#[tokio::test]
async fn test_find_callers_transitive() {
    let server = start_test_server(test_repo_path()).await;
    let result = server.call_tool("find_callers", json!({"symbol": "db_query", "depth": 3})).await;
    // Should find transitive callers up to depth 3
    assert!(result.callers.len() >= 3);
}
```

#### F. CLI Integration Tests (with `assert_cmd`)

```rust
#[test]
fn test_engram_init_creates_db() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("engram").unwrap()
        .args(["init", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success();
    assert!(dir.path().join(".engram").join("engram.db").exists());
}

#[test]
fn test_engram_search_returns_results() {
    let dir = setup_indexed_test_repo();
    Command::cargo_bin("engram").unwrap()
        .args(["search", "validate", "--root", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::str::contains("validate_token"));
}
```

### 6.6 Performance Benchmarks (criterion)

```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_incremental_update(c: &mut Criterion) {
    let repo = setup_large_test_repo(1000); // 1000 files
    let mut engine = index_repo(&repo);

    c.bench_function("incremental_single_file_change", |b| {
        b.iter(|| {
            modify_one_file(&repo);
            engine.sync();
        })
    });
}

fn bench_hybrid_search(c: &mut Criterion) {
    let engine = setup_indexed_engine(10_000); // 10K symbols

    c.bench_function("search_rrf_10k_symbols", |b| {
        b.iter(|| engine.search_hybrid("authentication handler", 10))
    });
}

fn bench_cascade_propagation(c: &mut Criterion) {
    let engine = setup_annotated_engine(1000, 5); // 1000 nodes, depth 5

    c.bench_function("cascade_depth_5", |b| {
        b.iter(|| engine.run_cascade("leaf_node"))
    });
}

fn bench_embedding_throughput(c: &mut Criterion) {
    let symbols = generate_symbols(100);

    c.bench_function("embed_100_symbols", |b| {
        b.iter(|| embed_batch(&symbols))
    });
}

criterion_group!(benches,
    bench_incremental_update,
    bench_hybrid_search,
    bench_cascade_propagation,
    bench_embedding_throughput
);
criterion_main!(benches);
```

### 6.7 Performance Budgets (CI gates)

| Metric | Target | Gate |
|--------|--------|------|
| Incremental update (single file change) | <50ms P99 | Fail CI if >50ms |
| Hybrid search (10K symbols) | <100ms P99 | Fail CI if >100ms |
| Cascade propagation (depth 5) | <200ms P99 | Warn if >200ms |
| Embedding throughput | >50 symbols/sec | Warn if below |
| Startup / L0 injection | <100ms | Fail CI if >100ms |
| Binary size (release) | <50MB | Warn if >50MB |
| RSS at 100K symbols | <500MB | Warn if above |

### 6.8 Key Metrics (prioritized by impact)

#### Tier 1: Must-Have (Gate CI)

| Metric | Target | Why |
|--------|--------|-----|
| **Search MRR** | >0.85 on EngramBench | If the agent's first query doesn't find the right symbol, everything downstream fails |
| **Precision@5** | >0.80 on EngramBench | Agent's context window is precious. 3 irrelevant results in top 5 wastes tokens |
| **Incremental update P99** | <50ms | If indexing blocks the agent's workflow, adoption dies |
| **Cascade correctness** | 100% direct, >95% transitive | Zero false negatives. If a symbol changed and has dependents with annotations, the cascade must fire |
| **All tests pass** | `cargo test` green | Basic hygiene |

#### Tier 2: Important (Track, improve iteratively)

| Metric | Target | Why |
|--------|--------|-----|
| **Recall@10** | >0.90 on EngramBench | Did we find all relevant symbols? Matters for "find all callers" |
| **NDCG@10** | >0.75 on EngramBench | Ranking quality — most relevant result should be ranked highest |
| **Token efficiency** | useful_tokens/budget > 0.7 | For `get_context_batch`, higher = less waste |
| **Staleness detection recall** | 100% direct, >95% transitive | When code changes, what percentage of affected annotations are correctly flagged? |

#### Tier 3: Competitive positioning

| Metric | Target | Why |
|--------|--------|-----|
| **CodeSearchNet NDCG** | Beat neural BoW baseline | Credible published number for marketing |
| **Indexing throughput** | >500 files/sec | First-run experience |
| **Binary size** | <50MB | Single-binary advantage meaningless at 500MB |
| **RSS at 100K symbols** | <500MB | ~150MB for vectors + DB + overhead |

### 6.9 Test Fixture Repos

In `tests/fixtures/`, create small deterministic repos:

```
tests/fixtures/
├── simple/           # 5 files, known call graph A->B->C, known annotations
│   ├── main.rs
│   ├── auth.rs       # AuthService with validate_token
│   ├── db.rs         # Database layer
│   ├── handler.rs    # Request handlers calling auth + db
│   └── utils.rs      # Shared utilities
├── diamond/          # Diamond dependency pattern for cascade testing
│   ├── main.rs       # calls B and C
│   ├── b.rs          # calls D
│   ├── c.rs          # calls D
│   └── d.rs          # leaf node
├── multilang/        # One file per supported language for parser snapshot tests
│   ├── sample.rs
│   ├── sample.py
│   ├── sample.ts
│   ├── sample.js
│   ├── sample.go
│   ├── sample.java
│   ├── sample.c
│   ├── sample.cpp
│   └── sample.rb
├── scope_test/       # Deeply nested structures for scope chain testing
│   ├── nested.rs     # mod > struct > impl > fn > closure
│   └── nested.py     # module > class > class > method
├── large/            # Generated 1000-file repo for performance benchmarks
│   └── generate.sh   # Script to create deterministic large repo
└── cross_file/       # Cross-file entity resolution testing
    ├── trait_def.rs   # defines trait Handler
    ├── impl_a.rs      # implements Handler
    └── impl_b.rs      # implements Handler (same canonical_id test)
```

### 6.10 CI Pipeline

```yaml
# .github/workflows/ci.yml
jobs:
  test:
    steps:
      - cargo test                          # All unit + integration tests
      - cargo insta test                    # Snapshot tests
      - cargo test --features proptest      # Property-based tests

  bench:
    steps:
      - cargo bench                         # criterion benchmarks
      - # Compare against baseline, alert if regression >5%

  quality:
    steps:
      - cargo run -- bench engrambench      # Run EngramBench golden test set
      - # Assert MRR >0.85, Precision@5 >0.80
      - # Assert no metric regressed >2% vs main branch

  build:
    steps:
      - cargo build --release
      - # Check binary size <50MB
      - # Run CLI integration tests with assert_cmd
```

---

## Part 7: Implementation Phases (Revised)

Strategy: **Build a vertical slice first** — get L0 (ingest) → L1 (structural) → hybrid search → core MCP tools working end-to-end before widening. This lets us validate the architecture and start benchmarking early.

### Phase 1: Vertical Slice — Ingest + Structure + Search (Weeks 1-4)

**Goal**: A working end-to-end system that can index a Rust repo, build a directed call graph, and answer queries via hybrid search + MCP. All infrastructure (test fixtures, benchmarks, CI) established.

**1A. Project scaffold + test infrastructure** (Week 1):
- `Cargo.toml` with all dependencies
- Project structure: `src/{cli, parser, graph, embeddings, mcp, memory, temporal, intelligence}`
- Test fixtures: `tests/fixtures/{simple, diamond, multilang, scope_test}`
- `criterion` benchmark scaffold
- `insta` snapshot test scaffold
- CI pipeline (GitHub Actions)

**1B. Tree-sitter parsing + scope trees** (Week 1-2):
- Parse Rust, Python, TypeScript, JavaScript, Go (5 languages)
- Extract symbols: functions, classes, structs, traits, interfaces, enums, methods
- Extract edges: CALLS (within-file), IMPORTS, INHERITS, IMPLEMENTS
- Compute `parent_id` by line-range containment
- Build `scope_chain` by walking parent_id chain
- Compute `canonical_id = sha256(name + kind + body_hash)` for cross-file dedup
- **Snapshot tests for every language's extraction output**

**1C. SQLite storage + incremental sync** (Week 2):
- Schema creation: `symbols`, `edges`, `chunks`, `file_hashes`, `embeddings`, `fts_symbols` (6 core tables)
- WAL mode, write-ahead journal
- Symbol upsert with `body_hash` change detection
- File hash tracking for incremental sync
- FTS5 population during symbol upsert
- **Idempotency test**: index twice, assert zero changes second time

**1D. Sub-symbol chunking** (Week 2):
- Functions >500 NWS chars → split at statement boundaries (learned from code-chunk)
- Each chunk inherits parent's scope_chain
- Store in `chunks` table with own embedding
- NWS cumulative sum approach (adapted from code-chunk, but in Rust)
- Token estimate alongside NWS count

**1E. Embeddings + BM25 + RRF** (Week 3):
- fastembed integration (AllMiniLM-L6-V2, ONNX)
- Scope-enriched embedding text: `scope_chain + signature + docstring + callers + imports`
- BM25 via FTS5: `search_bm25(query) → Vec<(symbol_id, score)>`
- Vector search: in-memory brute-force cosine over embeddings table
- Graph proximity: weighted BFS from attention-hot symbols
- **RRF fusion**: combine all three signals
- **Ablation test**: vector-only vs BM25-only vs RRF on test fixture queries

**1F. Core MCP tools + CLI** (Week 3-4):
- MCP server via `rmcp` (stdio transport)
- 10 core tools first:
  - `get_symbol`, `find_callers`, `find_dependencies`, `search_semantic`
  - `get_file_summary`, `get_project_identity`, `get_context`
  - `annotate_symbol`, `verify_annotation`, `downvote_annotation`
- CLI: `engram init`, `engram start`, `engram search`, `engram mcp`
- **MCP integration tests for every implemented tool**

**1G. File watcher** (Week 4):
- `notify` crate for filesystem events
- Debounce (3-second window, like Graphify)
- On change: re-parse changed file, update symbols/edges, recompute affected embeddings
- **Benchmark: assert <50ms for single-file incremental update**

**Deliverable**: A working system that can `engram init && engram mcp` on any Rust/Python/TS/JS/Go repo. 10 MCP tools. Hybrid search. Test infrastructure. Benchmarks running.

### Phase 2: Self-Healing Memory + Knowledge Layer (Weeks 5-7)

**Goal**: The cascade — our primary differentiator. Plus the remaining knowledge/reasoning tools.

**2A. Self-healing cascade** (`src/memory/cascade.rs`) (Week 5):
- On symbol change: compare old vs new `full_hash`
- Direct staleness: annotation's `full_hash_at` != symbol's `full_hash` → mark stale
- Transitive cascade: BFS through reverse call graph, decay confidence with distance
- Revert reactivation: `full_hash` reverts to `full_hash_at` → annotation reactivated
- Run as tokio background task (off the hot path)
- Cascade log with full audit trail
- **Property-based tests with proptest**: random DAGs, random mutations, verify invariants
- **Specific cascade correctness tests**: single-hop, multi-hop, diamond, revert

**2B. Remaining knowledge tools** (Week 6):
- `record_decision`, `record_pattern`, `get_insights`, `resolve_insight`
- `get_risk_score` (complexity × callers × staleness)
- `get_codebase_report` (architecture summary)
- `get_topic_summary` (topic-clustered symbols)
- Schema: `annotations`, `decisions`, `patterns`, `topics`, `insights`, `cascade_log` tables

**2C. Progressive context loading** (Week 6):
- L0-L4 tier system implementation
- `get_context(symbol, tier)` — returns appropriate detail level
- `get_context_batch(symbols, budget)` — greedy token-budget packing
- Token estimation per tier
- **Test: assert token counts match tier specifications**

**2D. Interaction tracking** (Week 7):
- `cortex_sessions`, `interactions`, `attention_map` tables
- Track which symbols agents query, view, annotate
- `get_exploration_map` — coverage analysis
- `suggest_next` — attention-based exploration suggestions

**Deliverable**: 22+ MCP tools. Self-healing memory with full cascade. Progressive context loading. All knowledge/reasoning tools.

### Phase 3: Historical + Temporal Layers (Weeks 8-10)

**Goal**: Git integration, temporal awareness, language expansion.

**3A. Git integration** (`src/git/mod.rs`) (Week 8):
- libgit2 via `git2` crate: in-process blame, log, diff
- `git_commits`, `symbol_commits`, `file_ownership` tables
- `get_symbol_history`, `get_ownership`, `get_blame` tools
- Map symbols to commits by line-range overlap with blame data

**3B. Symbol evolution** (`src/temporal/mod.rs`) (Week 9):
- On each sync: compare old vs new `full_hash` → log in `symbol_evolution`
- Change types: created, modified, renamed, deleted, moved
- `get_evolution` tool: symbol change timeline with annotation history
- Branch detection: check current branch on watcher cycle
- Branch context: snapshot `{symbol_id: full_hash}` on branch switch

**3C. File rename detection** (Week 9):
- Deleted file + new file in same batch → compare `body_hash`es of symbols
- Match → migrate annotations, preserve `canonical_id`, log in `symbol_evolution`
- Cross-file entity resolution: same `canonical_id` links symbols across renames

**3D. Language expansion** (Week 10):
- Add Java, C, C++, Ruby → 9 languages total
- Each needs: tree-sitter grammar crate + query patterns (adapted from Zed editor where available)
- **Snapshot tests for each new language**
- Test all 9 languages on `tests/fixtures/multilang/`

**Deliverable**: Full git integration. Temporal tracking. Branch awareness. Rename handling. 9 languages. 28 MCP tools complete.

### Phase 4: Intelligence + Polish + Benchmarks (Weeks 11-14)

**Goal**: Make it smart, fast, and distributable. Publish benchmark numbers.

**4A. Leiden community detection** (`src/intelligence/community.rs`) (Week 11):
- Topology-based module clustering (replace file-path heuristic)
- Port Leiden algorithm to Rust or use `petgraph` + community detection crate
- Community labels, cohesion scores
- `get_module_context` enhanced with community data

**4B. Code clone detection** (`src/intelligence/similarity.rs`) (Week 11):
- `body_hash` exact matches for clones
- Embedding cosine similarity for near-clones
- `find_similar` tool enhanced

**4C. Contradiction detection** (`src/memory/reasoning.rs`) (Week 12):
- Flag when two annotations on same symbol assert conflicting facts
- Surface as insights

**4D. EngramBench + CodeSearchNet evaluation** (Week 12-13):
- Run full EngramBench suite on 3 repos
- Publish MRR, NDCG@10, Precision@5, Recall@10 numbers
- Run CodeSearchNet 99-query evaluation
- RRF ablation study: publish which k-value and signal weights work best
- Scope-enriched vs plain embedding A/B comparison

**4E. Distribution** (Week 13-14):
- `brew install engram` (Homebrew formula)
- `engram install-hooks` (auto-configure Claude Code)
- Binary size optimization (strip, LTO, UPX)
- Documentation + README
- GitHub releases with pre-built binaries (macOS arm64/x86_64, Linux x86_64)

**Deliverable**: Intelligent clustering, clone detection, published benchmark numbers, easy installation.

### Future Scope (post-v1)
- Cloud sync (encrypted annotation deltas for teams)
- LSP adapter (hover/definition/reference powered by Engram)
- Runtime tracers (Python sidecar, Node.js, Go)
- LLM-powered optional features (auto-summarization, contradiction resolution)
- Web dashboard (3D call graph visualization)
- WASM build for browser demos
- CoIR benchmark evaluation (broader code retrieval tasks)
- Code-specific embedding model (fine-tuned or CodeBERT swap)

---

## Part 8: Verification Plan (Per Phase)

### After Phase 1:
1. `cargo test` — all unit + snapshot + integration tests pass
2. `cargo build --release` — single binary compiles, <50MB
3. `engram init && engram mcp` on test fixture repos → all 10 MCP tools respond correctly
4. `criterion` benchmarks: incremental update <50ms P99, search <100ms P99
5. Snapshot tests pass for all 5 languages (Rust, Python, TS, JS, Go)
6. RRF ablation shows fusion beats any single signal on test queries

### After Phase 2:
7. Cascade tests: edit file → annotations marked stale → cascade_log populated
8. Property-based tests: 1000+ random DAGs, all cascade invariants hold
9. Revert test: change code → stale → revert → annotation reactivated
10. Token-budget test: `get_context_batch` respects budget within 5% margin
11. All 22+ MCP tools respond correctly

### After Phase 3:
12. Git integration: `get_blame`, `get_ownership` return correct data
13. Symbol evolution: change timeline matches actual git history
14. Rename detection: annotations survive file renames
15. All 9 languages parse correctly (snapshot tests)
16. All 28 MCP tools respond correctly

### After Phase 4:
17. EngramBench: MRR >0.85, Precision@5 >0.80, NDCG@10 >0.75
18. CodeSearchNet: NDCG beats neural bag-of-words baseline
19. RRF k-parameter: optimal k identified and documented
20. Scope-enriched embeddings: measurable improvement over plain on disambiguation queries
21. Binary size <50MB, RSS <500MB at 100K symbols
22. `brew install engram` works
23. `engram install-hooks` correctly configures Claude Code

---

## Part 9: Why We Win

```
Graphify:  "Here's a graph of your code"        → but keyword substring matching is your only search
MemPalace: "I remember what you said"            → but you split my code at arbitrary 800-char boundaries
Mem0:      "I remember everything"               → 97.8% of what you remember is junk, and you can't read code
code-chunk: "Here are nice chunks for embedding" → but that's all you do, and only for 6 languages

Engram:    "I understand your code structure, search it three ways
            fused together, remember what matters, detect when my
            memory is wrong, and give you exactly the context you
            need within your token budget — in a single binary
            with zero dependencies, backed by published benchmarks."
```

The moat is the combination. Any one feature can be replicated. The integration of directed code graphs + truly fused hybrid search + self-healing memory + token-aware progressive loading + temporal evolution — all in a single Rust binary that installs in one command and publishes its benchmark numbers — is the product.

**Mem0 has 53K stars and 307 contributors but zero code intelligence. MemPalace has 48K stars but treats code as plain text. Graphify has 30K stars but can't search semantically. We don't just claim to be better — we prove it with EngramBench and CodeSearchNet.**
