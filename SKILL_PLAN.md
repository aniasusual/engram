# Engram Skill + MCP: The Complete Architecture

## The Problem

Engram currently has 28 MCP tools. They work. But Claude has to figure out on its own:
- Which tool to call and when
- How to chain tool outputs into a workflow
- What to do when results are surprising
- When to annotate, when to search, when to explore
- How to present findings conversationally

Graphify solved this by writing a 57KB skill file — a complete runbook that tells Claude exactly what to do at every step. Their MCP server (7 tools) is secondary, used only for post-build querying.

**We need a skill layer on top of our MCP layer.** Not replacing MCP — augmenting it.

## The Architecture

```
┌──────────────────────────────────────────────────────┐
│  SKILL LAYER (skill.md — loaded into Claude's context) │
│                                                        │
│  • Workflow orchestration (when to call what)           │
│  • Reasoning strategies (how to interpret results)     │
│  • Conditional logic (if X then do Y)                  │
│  • Conversational flow (guide the user)                │
│  • Error recovery (what to do when things fail)        │
│  • Session management (orient → explore → annotate)    │
├──────────────────────────────────────────────────────┤
│  MCP LAYER (28 tools — deterministic data access)      │
│                                                        │
│  • Structured queries (get_symbol, find_callers, etc.) │
│  • Search (BM25, vector, hybrid RRF)                   │
│  • Annotations (create, verify, downvote)              │
│  • Analysis (risk scores, insights, community)         │
│  • Context loading (brief/standard/full/deep)          │
├──────────────────────────────────────────────────────┤
│  ENGINE LAYER (engram binary — runs as daemon)         │
│                                                        │
│  • File watcher with cascade                           │
│  • SQLite storage (20 tables)                          │
│  • Tree-sitter parsing (9 languages)                   │
│  • fastembed embeddings                                │
│  • Git integration                                     │
└──────────────────────────────────────────────────────┘
```

**The skill tells Claude WHAT to do. MCP tools are HOW it does it. The engine is WHERE the data lives.**

## What The Skill Does (That MCP Can't)

### 1. Session Workflows

The skill defines three workflows Claude follows depending on context:

**Workflow A: First Session (Orient)**
```
Trigger: User starts working on a project Engram hasn't seen
Steps:
  1. Call get_project_identity → print summary
  2. Call get_codebase_report → identify god nodes, risks
  3. Call get_exploration_map → show blind spots
  4. Offer: "I found 5 high-risk symbols. Want me to analyze them?"
  5. If yes → call get_context(symbol, "standard") for each
  6. Call suggest_next → recommend exploration path
```

**Workflow B: Returning Session (Recall)**
```
Trigger: User returns to a project Engram has indexed before
Steps:
  1. Call get_project_identity → quick summary
  2. Call get_insights → check for new warnings since last session
  3. Check for stale annotations → alert user
  4. Offer: "Since last session: 3 symbols changed, 2 annotations went stale. Review?"
```

**Workflow C: Deep Dive (Investigate)**
```
Trigger: User asks about a specific area of the codebase
Steps:
  1. Call search_semantic(query) → find relevant symbols
  2. For top 3 results: call get_context(symbol, "standard")
  3. Call find_callers + find_dependencies → map the neighborhood
  4. Call get_risk_score → quantify risk
  5. Present: "Here's the call chain. The riskiest point is X because..."
  6. Offer: "Want me to annotate what I found for future sessions?"
```

### 2. Reasoning Strategies

The skill embeds rules Claude follows when interpreting results:

```
REASONING RULES:

- When get_risk_score returns > 10: flag as high-risk, explain why
  (risk = complexity × callers × stale annotations)

- When find_callers returns > 10 callers: this is a god node.
  Say: "This symbol has N callers — changes here ripple widely."

- When an annotation is stale: DO NOT present it as truth.
  Say: "There was an annotation here, but the code changed since.
  The annotation said: [content]. This may no longer be accurate."

- When search returns results from both code AND markdown:
  present docs first (orientation), then code (implementation).

- When get_context returns "deep" tier with source code:
  read the actual code. Don't just parrot the metadata.
  Form your own understanding and compare with annotations.

- When two annotations contradict: call both out explicitly.
  Say: "There's a conflict — annotation #X says A, but #Y says B."

- NEVER present a stale annotation as current fact.
- NEVER skip blind spots — if suggest_next recommends a symbol, there's a reason.
- ALWAYS offer to annotate findings at the end of an investigation.
```

### 3. Conditional Logic

```
IF the project has never been indexed:
  Run: engram init && engram start
  Wait for indexing to complete
  Then proceed with Workflow A

IF search_semantic returns 0 results:
  Try search with different terms (synonyms, related concepts)
  If still 0: say "No matches found. The codebase may not have this concept,
  or try different search terms."

IF get_insights returns contradictions:
  Present them prominently: "⚠ Conflicting annotations detected"
  Offer to resolve: "Want me to investigate and recommend which is correct?"

IF a file changes while working:
  The engine auto-cascades (stale annotations, confidence reduction)
  Next time a stale symbol is queried, the skill says:
  "Note: this symbol changed since it was last annotated.
  The annotation may be outdated."

IF get_exploration_map shows > 20 blind spots:
  Prioritize by caller count: "There are 20 unexplored symbols.
  The 3 most connected (likely most important) are: X, Y, Z."
```

### 4. Conversational Guidance

The skill makes Claude conversational, not just data-dumping:

```
AFTER every investigation, offer one of:
  - "Want me to annotate these findings for future sessions?"
  - "Want me to trace the callers of this function?"
  - "Want me to check if this pattern appears elsewhere?"

AFTER finding a risky area, offer:
  - "This looks fragile. Want me to map all the symbols that would break
    if this function's signature changed?"

WHEN the user asks "what should I look at?":
  1. Call suggest_next
  2. Call get_risk_score on each suggestion
  3. Rank by risk
  4. Present: "Based on call graph centrality and annotation coverage,
     I'd start with X — it has 12 callers and no annotations yet."

WHEN the user makes a code change:
  After the change is saved:
  1. The cascade runs automatically
  2. On next query, check for newly stale annotations
  3. If found: "Heads up — your change to X.rs invalidated 2 annotations
     on validate_token and handle_request. Want me to review them?"
```

### 5. Multi-Tool Orchestration

The skill chains multiple MCP calls in sequences that Claude wouldn't figure out alone:

```
COMMAND: /engram analyze <symbol>

  Step 1: get_symbol(name) → get basic info
  Step 2: get_context(symbol, "deep") → read actual source code
  Step 3: find_callers(symbol, depth=3) → who depends on this
  Step 4: find_dependencies(symbol, depth=3) → what this depends on
  Step 5: get_risk_score(symbol) → quantify risk
  Step 6: get_annotations(symbol) → existing knowledge
  Step 7: get_evolution(symbol) → how it changed over time
  Step 8: find_similar(symbol) → any code clones?

  Present as a unified "dossier" on the symbol.

COMMAND: /engram health

  Step 1: get_codebase_report → overview
  Step 2: get_insights → warnings
  Step 3: get_exploration_map → coverage
  Step 4: For top 5 god nodes: get_risk_score
  Step 5: Present health dashboard with risk heatmap

COMMAND: /engram remember <finding>

  Step 1: Identify which symbols the finding relates to
  Step 2: For each: annotate_symbol(symbol, type, content)
  Step 3: Confirm: "Annotated 3 symbols. These annotations are hash-anchored —
          if the code changes, they'll be automatically flagged as stale."
```

### 6. Error Recovery

```
IF engram MCP server is not running:
  Say: "Engram isn't running. Start it with: engram start --mcp"
  OR: offer to start it automatically

IF get_symbol returns "not found":
  Try BM25 fuzzy search as fallback
  If still nothing: "Symbol not found. It might be in a file that hasn't
  been indexed yet, or spelled differently."

IF embedding computation fails:
  Say: "Semantic search unavailable (embedding model not loaded).
  Falling back to keyword search, which works but won't find
  semantic matches like 'authentication' → 'validate_token'."

IF the database is empty:
  Detect this on first tool call
  Say: "Engram hasn't indexed this project yet. Run 'engram start'
  to index and I'll be able to help with codebase intelligence."
```

## How Skill + MCP + Hooks Work Together

### Layer 1: Hooks (Passive Intelligence)

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Read",
      "hooks": [{
        "type": "command",
        "command": "engram context-for-file --brief --file $TOOL_INPUT_FILE"
      }]
    }]
  }
}
```

**When**: Every time Claude reads a file (automatically, no user action needed).
**What**: Injects a 1-line context header before the file content.
**Example**: Before Claude reads `auth.rs`, it sees:
```
[engram] AuthService — 12 callers, risk=24.0, 1 stale annotation on validate_token
```

This is zero-cost intelligence injection. Claude gets awareness without asking.

### Layer 2: MCP (Active Data Access)

**When**: Claude needs specific data — a symbol lookup, a search, caller graph.
**What**: Structured tool calls with predictable responses.
**Example**: Claude calls `find_callers("validate_token", 3)` and gets a list.

This is the data backbone. Fast, deterministic, works with any MCP client.

### Layer 3: Skill (Intelligent Orchestration)

**When**: User invokes `/engram` or asks about codebase architecture.
**What**: Claude follows a workflow — chains MCP calls, reasons about results, guides the user.
**Example**: `/engram health` triggers a 5-step health check with risk analysis and recommendations.

This is the intelligence layer. It turns raw data into insights and conversations.

## The Skill File Structure

```markdown
---
name: engram
trigger: /engram
description: Codebase intelligence — understand, search, and annotate your code
---

# Engram Skill

## Prerequisites
[Check engram is installed and MCP server is running]

## Workflows
### /engram (default — orient)
[Workflow A: Orient on the project]

### /engram health
[Health check workflow]

### /engram analyze <symbol>
[Deep dive workflow]

### /engram remember <finding>
[Annotation workflow]

### /engram explore
[Guided exploration workflow]

## Reasoning Rules
[How to interpret results, handle edge cases]

## Conversational Patterns
[How to present findings, what to offer next]

## Error Recovery
[What to do when things fail]

## MCP Tool Reference
[Quick reference for all 28 tools — when to use each]
```

## What Changes In Our Codebase

### New Files
```
skills/
  engram/
    SKILL.md          — The skill file (~800-1200 lines)
scripts/
  install-skill.sh    — Copies skill to ~/.claude/skills/engram/
```

### Modified Files
```
src/cli/mod.rs        — Add `engram install-skill` command
src/cli/mod.rs        — Add `engram context-for-file` command (for hooks)
README.md             — Document skill + MCP + hooks architecture
```

### New CLI Commands
```bash
engram install-skill    # Copy SKILL.md to ~/.claude/skills/engram/
engram context-for-file --brief --file auth.rs  # One-line context for hooks
```

## Why This Beats Graphify

| Dimension | Graphify | Engram (Skill + MCP) |
|-----------|----------|---------------------|
| Build pipeline | Skill orchestrates one-time graph build | Engine runs continuously (daemon + watcher + cascade) |
| Post-build intelligence | MCP with 7 read-only query tools | MCP with 28 tools + skill orchestration |
| Memory | None (graph is static snapshot) | Self-healing annotations with cascade |
| Exploration guidance | None | suggest_next + exploration_map + blind spots |
| Risk quantification | god_nodes only | risk = complexity × callers × stale_annotations |
| Cross-session memory | Graph persists, but no annotations | Annotations persist, auto-degrade when code changes |
| Non-Claude agents | MCP only (7 tools) | MCP (28 tools) — works with Cursor, Codex, Gemini |
| Conversational depth | Skill guides exploration | Skill guides + hooks inject passive awareness |
| Languages | 20+ (tree-sitter) | 9 (tree-sitter) + markdown |
| Semantic search | None (keyword only) | Vector + BM25 + graph via RRF |
| Temporal awareness | None (snapshot) | Symbol evolution + branch context |

## Implementation Order

### Phase 1: `context-for-file` CLI command (enables hooks)
This is the missing piece for passive intelligence. Build it first because it works without a skill.

### Phase 2: The skill file (SKILL.md)
Write the complete skill with all 5 workflows, reasoning rules, and conversational patterns. This is the biggest piece — ~800 lines of careful instructions.

### Phase 3: `install-skill` command
Automates copying SKILL.md to the right location.

### Phase 4: Hook integration
Wire up PreToolUse hooks to inject context-for-file output before file reads.

### Phase 5: Test the complete stack
Test all three layers working together:
- Hooks inject passive context
- MCP tools respond to active queries
- Skill orchestrates multi-step workflows
