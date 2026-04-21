---
name: engram
trigger: /engram
description: Codebase intelligence — understand, search, annotate, and explore your code with self-healing memory
---

# Engram Skill

You have access to Engram, a codebase intelligence daemon with 28 MCP tools. This skill tells you how to use them effectively.

## Prerequisites

Before using any Engram tool, check that the MCP server is running. If any tool call fails with a connection error, tell the user:

> Engram MCP server is not running. Start it with:
> ```
> engram start --root /path/to/project &
> engram mcp --root /path/to/project
> ```
> Or add it to your Claude Code MCP config.

If the first tool call returns empty results (0 symbols), the project likely hasn't been indexed:

> This project hasn't been indexed yet. The user should run:
> ```
> engram init && engram start
> ```
> This will parse all code files, compute embeddings, and sync git data.

---

## Commands

### `/engram` or `/engram .` — Orient on the project

**When to use:** First time working on a project, or at the start of any session.

**Steps:**

1. Call `get_project_identity` to get the project summary.

2. Call `get_codebase_report` to get the full architecture overview: languages, god nodes (most-called symbols), stale annotations.

3. Call `get_insights` to check for auto-detected warnings: god nodes with >10 callers, contradicting annotations, high-risk symbols.

4. Call `get_exploration_map` to see what has been explored vs what are blind spots (high-caller symbols never looked at).

5. Present findings as a narrative:

```
📊 Project: [name] ([language])
   [N] symbols | [N] edges | [N] files

🔥 Hot Spots (highest risk):
   1. [symbol] ([file]) — [callers] callers, risk=[score]
   2. ...

⚠️  Warnings:
   - [stale annotations if any]
   - [god nodes if any]
   - [contradictions if any]

🔍 Blind Spots ([N] high-caller symbols never explored):
   [symbol1], [symbol2], [symbol3]

💡 Suggested next: "[most connected blind spot]" has [N] callers
   and no annotations yet. Want me to investigate?
```

6. Wait for user response. If they say yes, proceed with `/engram analyze [symbol]`.

---

### `/engram health` — Codebase health check

**When to use:** To assess overall project health, find trouble spots.

**Steps:**

1. Call `get_codebase_report` for overview.

2. Call `get_insights` for warnings.

3. For the top 5 god nodes from the report, call `get_risk_score` on each.

4. Call `get_exploration_map` to identify coverage gaps.

5. Present as a health dashboard:

```
=== Engram Health Report ===

Overall: [N] symbols across [N] files

Risk Heatmap (top 5):
  🔴 [symbol] — risk [score] ([callers] callers, [stale] stale annotations)
  🟡 [symbol] — risk [score]
  🟢 [symbol] — risk [score]

Annotation Coverage:
  [N] symbols annotated, [N] annotations active, [N] stale

Exploration: [N]% of high-caller symbols explored

Recommendations:
  1. Review stale annotations on [symbols]
  2. Investigate [god node] — [callers] callers but no annotations
  3. [blind spot] has never been explored despite [callers] callers
```

---

### `/engram analyze <symbol>` — Deep dive into a symbol

**When to use:** To thoroughly understand a specific function, class, or module.

**Steps:**

1. Call `get_symbol` with the name. If not found, the tool will try BM25 fuzzy search automatically.

2. Call `get_context` with tier "deep" — this includes actual source code, signature, scope chain, callers, annotations.

3. Call `find_callers` with depth 3 — who depends on this symbol transitively.

4. Call `find_dependencies` with depth 3 — what this symbol depends on.

5. Call `get_risk_score` — quantify the risk.

6. Call `get_evolution` — how has this symbol changed over time.

7. Call `find_similar` — are there any code clones.

8. Read the source code from the "deep" context. Form your own understanding.

9. Present as a unified dossier:

```
=== [kind] [name] ([file]:[line]) ===

📝 Signature: [signature]
📖 Docstring: [docstring]
🎯 Risk: [score] (complexity=[c], callers=[n], stale=[s])

Scope: [scope chain]

Source code:
  [numbered source lines from deep tier]

📞 Called by ([N]):
  [caller1] → [caller2] → [this] (depth [N])

📤 Calls ([N]):
  [this] → [dep1] → [dep2]

📝 Annotations:
  #[id] [[type]] ([confidence]%, [status]) [content]

📜 Evolution:
  [date] [change_type] [summary]

🔄 Similar code:
  [EXACT/NEAR] [similarity]% — [other symbol] ([file])

💡 Observations:
  [Your analysis based on reading the actual source code,
   the call graph, and the annotations. Point out anything
   surprising, risky, or noteworthy.]
```

10. Offer: "Want me to annotate my findings for future sessions?"

If user says yes, call `annotate_symbol` for each finding. Use type "explanation" for how things work, "warning" for risks, "context" for architectural decisions, "assumption" for things that could break.

---

### `/engram explore` — Guided exploration

**When to use:** When the user wants to systematically explore the codebase.

**Steps:**

1. Call `suggest_next` — get symbols ranked by call graph centrality that haven't been explored.

2. For each suggestion, call `get_context` with tier "brief" to get a quick summary.

3. Present ranked exploration targets:

```
=== Exploration Suggestions ===

These symbols are highly connected but haven't been explored yet:

1. [symbol] ([file]) — [N] callers, risk=[score]
   [brief description from context]

2. [symbol] ([file]) — [N] callers, risk=[score]
   [brief description]

3. [symbol] ([file]) — [N] callers, risk=[score]
   [brief description]

Which one should I investigate? (or say "all" for a summary of each)
```

4. When user picks one, run `/engram analyze [symbol]`.

5. After analysis, offer to continue: "That's done. Next suggestion is [symbol]. Continue?"

This creates an exploration loop that systematically covers the codebase.

---

### `/engram query "<question>"` — Semantic search

**When to use:** When the user asks a question about the codebase.

**Steps:**

1. Call `search_semantic` with the question. This uses hybrid RRF (vector + BM25 + graph proximity) if embeddings exist, BM25 fallback otherwise.

2. For the top 3 results, call `get_context` with tier "standard" to get meaningful detail.

3. Present results with context:

```
Search: "[question]"

Found [N] relevant symbols:

1. [kind] [name] ([file]:[line])
   [signature]
   [docstring]
   [relevant callers/annotations]

2. ...

3. ...
```

4. Answer the user's question using the search results. Cite specific symbols and files.

5. Offer: "Want me to dive deeper into any of these? Or annotate this finding?"

---

### `/engram remember <finding>` — Annotate findings

**When to use:** When the user or you discover something important about the codebase.

**Steps:**

1. Identify which symbols the finding relates to. If unclear, call `search_semantic` to find them.

2. Determine the annotation type:
   - `explanation` — how something works
   - `warning` — a risk or gotcha
   - `todo` — something that needs to be done
   - `context` — architectural decision or background
   - `assumption` — something that could break if changed

3. Call `annotate_symbol` for each relevant symbol.

4. Confirm:

```
✅ Annotated [N] symbols:
  [symbol1] — [type]: [content snippet]
  [symbol2] — [type]: [content snippet]

These annotations are hash-anchored to the current code.
If the code changes, they'll automatically be flagged as stale.
```

---

### `/engram diff` — What changed since last session

**When to use:** Returning to a project after time away.

**Steps:**

1. Call `get_insights` to check for new warnings.

2. Call `get_exploration_map` — check if new symbols appeared.

3. Search for stale annotations: call `get_codebase_report` and look for the "Stale Annotations" section.

4. Present:

```
=== Changes Since Last Session ===

⚠️  Stale Annotations ([N]):
  [symbol]: "[annotation content]" — code changed, may be outdated

🆕 New Warnings:
  [any new insights]

📊 Coverage: [explored]% of high-caller symbols explored

Want me to review the stale annotations?
```

---

## Reasoning Rules

Follow these rules when interpreting Engram data:

### Risk Interpretation
- Risk score = (1 + complexity) × (1 + callers) × (1 + stale_annotations)
- Risk > 20: high risk — mention prominently, explain why
- Risk > 50: critical — warn explicitly
- A symbol with many callers AND stale annotations is the worst case — changes are risky AND knowledge is outdated

### Stale Annotations
- NEVER present a stale annotation as current fact
- Always say: "There was an annotation here, but the code has changed since. It said: [content]. This may no longer be accurate."
- Offer to review and re-annotate

### God Nodes
- A symbol with >10 callers is a god node
- Say: "This symbol has [N] callers — any change here ripples across [N] other symbols."
- Check if it has annotations. If not, flag this as a blind spot.

### Contradictions
- If two active annotations on the same symbol conflict, call both out
- Say: "Conflict detected — annotation #X says [A], but #Y says [B]. One of these is wrong."
- Offer to investigate and resolve

### Search Results
- When results come from both code AND markdown docs, present docs first (orientation), then code (implementation)
- When results include the "deep" tier with source code, actually read the code — don't just parrot metadata
- Form your own understanding and compare with existing annotations

### Exploration
- Always prioritize symbols with high callers but no annotations — these are knowledge gaps
- After investigating a symbol, always offer to annotate findings
- The exploration map tracks what you've looked at — use it to avoid re-investigating

### Annotations
- When annotating, be specific: "validates JWT tokens by checking signature and expiration" NOT "does validation"
- Use the right type: explanation for how, warning for risk, context for why, assumption for what could break
- Every annotation is automatically hash-anchored — you don't need to track staleness manually

---

## Conversational Patterns

### After every investigation
Offer one of:
- "Want me to annotate these findings for future sessions?"
- "Want me to trace the callers of this function?"
- "Want me to check if this pattern appears elsewhere?"
- "Want me to investigate the next blind spot?"

### After finding something risky
- "This looks fragile. Want me to map all symbols that would break if this function's signature changed?"
- "There are [N] callers that depend on this behavior. Want me to list them?"

### When asked "what should I look at?"
Always answer from data, not intuition:
1. Call `suggest_next` for data-driven suggestions
2. Call `get_risk_score` on each to rank
3. Present with reasoning: "Based on call graph centrality and annotation coverage, I'd start with X — it has 12 callers and no annotations yet."

### When user makes a code change
After any file save detected by the watcher:
- The cascade runs automatically (stale annotations, confidence reduction)
- Next time you access that symbol, check for newly stale annotations
- If found: "Heads up — your change to X invalidated [N] annotations. Want me to review them?"

---

## MCP Tool Quick Reference

### Structural (find code)
| Tool | When to use |
|------|-------------|
| `get_symbol` | Look up a specific function/class by name |
| `find_callers` | "What calls this?" — reverse call graph |
| `find_dependencies` | "What does this call?" — forward call graph |
| `search_semantic` | Natural language search ("authentication handling") |
| `get_file_summary` | Overview of all symbols in a file |
| `find_similar` | Find exact/near code clones |

### Knowledge (remember things)
| Tool | When to use |
|------|-------------|
| `annotate_symbol` | Write a finding (hash-anchored, auto-detects staleness) |
| `verify_annotation` | Confirm an annotation is still correct |
| `downvote_annotation` | Mark an annotation as questionable |
| `record_decision` | Log an architectural decision with rationale |
| `record_pattern` | Record a recurring pattern |

### Analysis (understand risk)
| Tool | When to use |
|------|-------------|
| `get_risk_score` | Quantify risk: complexity × callers × stale annotations |
| `get_insights` | Auto-generated warnings (god nodes, contradictions) |
| `resolve_insight` | Dismiss or mark a warning as resolved |
| `get_codebase_report` | Full architecture summary |
| `get_topic_summary` | Symbols grouped by topic |

### Navigation (explore systematically)
| Tool | When to use |
|------|-------------|
| `get_exploration_map` | What's been explored vs blind spots |
| `suggest_next` | Data-driven next symbol to investigate |
| `get_hot_path` | Most-called symbols with highest risk |
| `get_trace` | Static call chain for a symbol |

### Context (get details)
| Tool | When to use |
|------|-------------|
| `get_project_identity` | ~100-token project summary |
| `get_context` | Symbol detail at 4 tiers: brief/standard/full/deep |
| `get_context_batch` | Multiple symbols within a token budget |
| `get_module_context` | Directory-level view with communities |

### History (understand change)
| Tool | When to use |
|------|-------------|
| `get_symbol_history` | Git commits that touched this symbol's file |
| `get_ownership` | Who owns this file (git blame) |
| `get_blame` | Line-by-line git blame |
| `get_evolution` | Symbol change timeline + annotation history |
