# Graphify vs Engram: Complete Workflow Comparison & Gap Analysis

## How Graphify Actually Works (End-to-End)

### User Journey: Install to Value

```
Step 1: pip install graphifyy && graphify install     (30 seconds)
Step 2: Open Claude Code, type: /graphify .           (that's it)
Step 3: Wait 1-2 minutes                              (Claude runs the pipeline)
Step 4: See interactive graph, report, surprises       (value delivered)
Step 5: Ask questions: /graphify query "how does auth work?"
```

**Total time to first value: ~2 minutes. Total user effort: 2 commands.**

### What Happens When You Type `/graphify .`

Claude reads the 1,100-line skill.md and executes a 9-step pipeline:

```
Step 1: Check graphify installed (auto-install if missing)
Step 2: Scan directory → "52 files, 92K words (29 code, 14 docs, 5 PDFs, 4 images)"
Step 3A: AST extraction via tree-sitter (deterministic, free, instant)
Step 3B: Semantic extraction via parallel Claude subagents (LLM, cached per SHA256)
Step 4: Build NetworkX graph → Leiden community detection → analyze
Step 5: Claude reads node labels, writes community names ("Auth Module", "Data Pipeline")
Step 6: Generate HTML visualization + Obsidian vault
Step 7: Optionally generate wiki, SVG, Neo4j export
Step 8: Token benchmark (71.5x reduction)
Step 9: Present report, offer exploration
```

### What The User Sees After

```
Corpus: 52 files ~ 92,616 words
  code: 29 files (.py)  docs: 14 files (.md)  papers: 5 (.pdf)  images: 4

Graph: 847 nodes, 1,203 edges, 12 communities

God Nodes (most connected):
  1. Client (34 connections) — auth/client.py
  2. RequestBuilder (28 connections) — core/request.py
  3. ResponseParser (22 connections) — core/response.py

Surprising Connections:
  ⚡ CausalSelfAttention implements the algorithm from FlashAttention paper
     (INFERRED, cross-file-type: code→paper)
  ⚡ DigestAuth connects HTTP Transport to Auth Module
     (EXTRACTED, cross-community bridge)

Suggested Questions:
  ? Why does Client connect HTTP Transport to Auth Module?
  ? What is the relationship between Config and Environment?
  ? How does ResponseParser handle streaming vs batch?

The most interesting question: "Why does Client connect HTTP Transport
to Auth Module?" Want me to trace it?
```

### Post-Build Querying

```
/graphify query "how does authentication work?"
→ BFS traversal from auth-related nodes, answer using ONLY graph data

/graphify path "DigestAuth" "Response"
→ Shortest path: DigestAuth → AuthHandler → RequestBuilder → Response

/graphify explain "CausalSelfAttention"
→ 3-5 sentence explanation of all connections

/graphify . --update
→ Only re-extracts changed files (SHA256 cache), shows diff
```

### Continuous Sync

```
Git hook: Every commit → auto AST re-extraction → graph updated (no LLM needed)
Watcher: --watch mode → detects file changes → 3-second debounce → auto-rebuild
Feedback loop: Query results saved as markdown → extracted as nodes on next --update
```

---

## How Engram Currently Works (End-to-End)

### User Journey: Install to Value

```
Step 1: cargo build --release                         (1-2 minutes)
Step 2: engram init                                   (instant)
Step 3: engram start                                  (indexes + embeds, 5-30 seconds)
Step 4: Open second terminal                          (start blocks for watching)
Step 5: engram search "authentication"                (get results)
   OR: Configure MCP server in claude.json             (complex)
   OR: engram mcp                                      (blocks another terminal)
```

**Total time to first value: ~3 minutes. But requires 3-5 commands and understanding of the architecture.**

### What Claude Can Do With Engram MCP

Claude calls individual tools and gets back structured data:

```
get_symbol("validate_token")
→ function validate_token (auth.rs:15-20) signature: pub fn validate_token...

find_callers("validate_token", 3)
→ [depth 1] function handle_request (handler.rs:6)

search_semantic("authentication")
→ [0.016] struct AuthService (auth.rs:2)
→ [0.016] function validate_token (auth.rs:15)

get_risk_score("validate_token")
→ risk score: 24.0

get_codebase_report()
→ Symbols: 891 | Edges: 234 | Files: 142
→ Languages: rust — 891 symbols
→ Most-called: validate_token (12 callers)
```

### What's Missing In The User Experience

Claude gets raw data. It does NOT get:
- Guidance on what to do with it
- A narrative about the codebase
- Surprising connections it didn't ask about
- Suggested questions
- An offer to explore further
- Community labels written from understanding
- A visual graph

---

## Gap Analysis: What Graphify Has That We Don't

### Critical Gaps (must fix)

| # | Gap | Graphify | Engram | Impact |
|---|-----|----------|--------|--------|
| 1 | **Single-command experience** | `/graphify .` does everything | Need `init` + `start` + second terminal + MCP config | Adoption killer |
| 2 | **Skill orchestration** | 1,100-line runbook guides Claude through workflows | Claude guesses how to use 28 tools | Quality of agent output |
| 3 | **Narrative output** | "God Nodes", "Surprising Connections", "Suggested Questions" | Raw lists and numbers | User delight |
| 4 | **Visual graph** | Interactive HTML with vis.js, click nodes, color-coded communities | Nothing visual | Shareability, "aha moment" |
| 5 | **Query saves as memory** | Q&A results saved to markdown, extracted on next --update | Annotations are manual only | Memory grows from use |
| 6 | **Token reduction benchmark** | "71.5x fewer tokens per query" — concrete, verifiable | We have benchmarks but don't surface them to users | Marketing proof |

### Moderate Gaps

| # | Gap | Graphify | Engram | Impact |
|---|-----|----------|--------|--------|
| 7 | **Multimodal** | Code + docs + PDFs + images + tweets | Code + markdown | Breadth of input |
| 8 | **Output formats** | HTML, Obsidian, Wiki, SVG, GraphML, Neo4j | SQLite DB + CLI text | User choice |
| 9 | **Community labeling** | Claude reads nodes, writes human names like "Auth Module" | Louvain clustering without readable labels | Understanding |
| 10 | **Parallel subagents** | Skill dispatches parallel extraction agents | Single-threaded processing | Speed at scale |
| 11 | **Feedback loop** | Queries become graph nodes | No automatic learning from queries | Memory richness |

### Gaps Where Engram Is AHEAD

| # | Advantage | Engram | Graphify | Impact |
|---|-----------|--------|----------|--------|
| 1 | **Self-healing memory** | Hash-anchored annotations + BFS cascade | No memory at all | Knowledge stays correct |
| 2 | **Semantic search** | Vector + BM25 + graph via RRF fusion | Keyword substring matching only | Finding things by meaning |
| 3 | **Temporal awareness** | Symbol evolution + branch context + rename detection | Snapshot only, no history | Understanding change |
| 4 | **Risk quantification** | complexity × callers × stale_annotations | God nodes only (degree count) | Prioritizing attention |
| 5 | **Exploration guidance** | Attention tracking, blind spots, suggest_next | None | Systematic exploration |
| 6 | **Cross-agent support** | 28 MCP tools work with Cursor, Codex, Gemini | Claude-only (skill + 7 MCP tools) | Reach |
| 7 | **Directed call graph** | Directed edges with BFS traversal | Undirected NetworkX graph | Accuracy |
| 8 | **Continuous cascade** | File change → auto cascade → stale annotations | Git hook rebuilds from scratch | Incremental correctness |
| 9 | **Progressive context** | 4 tiers (brief→deep) with source code + token budget | Fixed output format | Token efficiency |
| 10 | **Zero LLM dependency** | All core operations work without LLM API | Requires Claude for semantic extraction | Independence |

---

## The Complete Fix: What We Need To Build

### Priority 1: The Skill (biggest impact)

Build `skills/engram/SKILL.md` — ~800-1200 lines. This is the single most important improvement. It transforms Engram from "28 APIs an agent could theoretically use" to "an intelligent orchestration system that guides Claude through codebase understanding."

**The skill should define these commands:**

| Command | What it does |
|---------|-------------|
| `/engram` or `/engram .` | Full pipeline: init + index + embed + report + offer exploration |
| `/engram health` | Codebase health check: risks, stale annotations, blind spots |
| `/engram analyze <symbol>` | Deep dive: context, callers, deps, risk, evolution, clones |
| `/engram explore` | Guided exploration: suggest_next → investigate → annotate |
| `/engram remember <finding>` | Annotate symbols with findings |
| `/engram query "<question>"` | Semantic search with contextual follow-up |
| `/engram diff` | What changed since last session + cascade impact |

**The skill embeds:**
- Workflow orchestration (chain 5-8 MCP calls per command)
- Reasoning rules (how to interpret risk scores, stale annotations, contradictions)
- Conditional logic (if no embeddings: explain and offer to compute; if no git: skip history)
- Conversational flow (present findings, offer next steps, create exploration loops)
- Error recovery (MCP not running, DB empty, model not downloaded)

### Priority 2: `context-for-file` CLI command (enables passive hooks)

```bash
engram context-for-file --brief --file auth.rs
# Output: [engram] AuthService — 12 callers, risk=24.0, 1 stale annotation
```

This enables the PreToolUse hook that injects context before every file read. Zero-cost passive intelligence.

### Priority 3: Single-command experience

Merge `init` + `start` + `mcp` into one flow:
```bash
engram start --mcp
# Initializes if needed
# Indexes + embeds
# Starts MCP server on stdio
# Watches for changes in background
# All in one process
```

Or even better — the skill handles it:
```
User types: /engram .
Skill checks: Is engram initialized? No → run engram init
Skill checks: Is DB populated? No → run engram start (one-shot index)
Skill checks: Are embeddings computed? No → compute them
Then proceed with the report workflow
```

### Priority 4: Narrative Output

When the skill runs `/engram`, the output should look like this (not raw API data):

```
📊 Project: engram (Rust)
   1,890 symbols | 802 edges | 255 files | 9 languages

🔥 Hot Spots (highest risk):
   1. Store (store.rs) — 48 methods, 14 callers, risk=168
   2. EngramMcp (mcp/mod.rs) — 28 tools, risk=84
   3. CodeParser (treesitter.rs) — 6 callers, risk=42

⚠️  Stale Annotations:
   validate_token: "assumes JWT format" — CODE CHANGED since annotation

🔍 Blind Spots (high-caller symbols never explored):
   run_cascade, detect_renames, snapshot_branch

💡 Suggested:
   "Store has 48 methods — want me to check which ones overlap or could be split?"
```

### Priority 5: Visual Graph (future)

Generate an interactive HTML visualization of the call graph. This is what makes Graphify shareable. Lower priority because it's pure frontend work, but high impact for virality.

---

## Implementation Order

| Phase | What | Files | Effort |
|-------|------|-------|--------|
| **1** | `context-for-file` CLI command | `src/cli/mod.rs` | Small |
| **2** | SKILL.md — complete skill file | `skills/engram/SKILL.md` | Large (800+ lines) |
| **3** | `install-skill` CLI command | `src/cli/mod.rs` | Small |
| **4** | Single-command `start --mcp` | `src/cli/mod.rs`, `src/watcher.rs` | Medium |
| **5** | Auto-annotate from queries (feedback loop) | `src/graph/store.rs` | Small |
| **6** | Community labeling (human-readable names) | `src/intelligence/community.rs` | Medium |
| **7** | HTML graph visualization | New module | Large |

Phases 1-3 are the critical path. They transform the product from "powerful but hard to use" to "delightful from first command."
