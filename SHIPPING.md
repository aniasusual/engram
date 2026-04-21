# Shipping Engram: Step-by-Step Launch Guide

## Pre-Launch Checklist

Before going public, verify everything works:

```bash
cd /Users/animeshdhillon/Desktop/myProjects/engram

# 1. All tests pass
cargo test
# Expected: 176 tests, 0 failures

# 2. Release build compiles with zero warnings
cargo build --release
# Expected: 16MB binary, 0 warnings

# 3. End-to-end smoke test
cd /tmp && mkdir smoke-test && cd smoke-test
engram init
echo 'fn hello() { world(); }' > test.rs
echo 'fn world() -> bool { true }' >> test.rs
echo '# README' > README.md
echo 'This project greets the world.' >> README.md
engram start &
sleep 5
engram search "hello"
engram status
kill %1
cd - && rm -rf /tmp/smoke-test
```

---

## Step 1: Create GitHub Repository

```bash
# Go to https://github.com/new
# Repository name: engram
# Description: Persistent codebase intelligence for coding agents
# Visibility: Public
# License: MIT
# Do NOT initialize with README (we have one)

# Then push:
cd /Users/animeshdhillon/Desktop/myProjects/engram
git add -A
git commit -m "Initial release: Engram v0.1.0

Persistent codebase intelligence daemon for coding agents.

- 28 MCP tools across 7 architectural layers
- 9 languages (Rust, Python, TypeScript, JavaScript, Go, Java, C, C++, Ruby)
- Self-healing memory with hash-anchored annotations and BFS cascade
- Hybrid search: vector (fastembed) + BM25 (FTS5) + graph proximity via RRF
- Markdown/docs parsing and unified code+docs search
- Git integration: blame, ownership, symbol history
- Community detection, clone detection, contradiction detection
- Progressive context loading (brief/standard/full/deep with source code)
- 16MB single binary, zero runtime dependencies
- 176 tests, 13 criterion benchmarks

MRR: 1.000 on EngramBench (hybrid RRF)
CodeSearchNet NDCG@10: 0.154 (general-purpose model)
Cascade: 1,003 random DAGs tested, zero failures"

git remote add origin git@github.com:aniasusual/engram.git
git branch -M main
git push -u origin main
```

---

## Step 2: GitHub Actions — Auto-Build Release Binaries

The CI workflow at `.github/workflows/ci.yml` already handles test + clippy + build.
Now add a release workflow that builds binaries for macOS (arm64 + x86) and Linux (x86):

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  build:
    name: Build ${{ matrix.target }}
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - target: aarch64-apple-darwin
            os: macos-latest
            name: engram-macos-arm64
          - target: x86_64-apple-darwin
            os: macos-latest
            name: engram-macos-x86_64
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
            name: engram-linux-x86_64

    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: Swatinem/rust-cache@v2

      - name: Build
        run: cargo build --release --target ${{ matrix.target }}

      - name: Package
        run: |
          cd target/${{ matrix.target }}/release
          tar czf ../../../${{ matrix.name }}.tar.gz engram
          cd ../../..

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.name }}
          path: ${{ matrix.name }}.tar.gz

  release:
    name: Create Release
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Download all artifacts
        uses: actions/download-artifact@v4

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            engram-macos-arm64/engram-macos-arm64.tar.gz
            engram-macos-x86_64/engram-macos-x86_64.tar.gz
            engram-linux-x86_64/engram-linux-x86_64.tar.gz
          body: |
            ## Engram ${{ github.ref_name }}

            Persistent codebase intelligence for coding agents.

            ### Install

            **macOS (Apple Silicon):**
            ```bash
            curl -sL https://github.com/aniasusual/engram/releases/latest/download/engram-macos-arm64.tar.gz | tar xz
            sudo mv engram /usr/local/bin/
            ```

            **macOS (Intel):**
            ```bash
            curl -sL https://github.com/aniasusual/engram/releases/latest/download/engram-macos-x86_64.tar.gz | tar xz
            sudo mv engram /usr/local/bin/
            ```

            **Linux (x86_64):**
            ```bash
            curl -sL https://github.com/aniasusual/engram/releases/latest/download/engram-linux-x86_64.tar.gz | tar xz
            sudo mv engram /usr/local/bin/
            ```

            **From source:**
            ```bash
            cargo install --git https://github.com/aniasusual/engram
            ```

            ### Quick Start
            ```bash
            cd your-project
            engram init
            engram start     # Index + embed + watch
            engram search "authentication"
            ```
```

Then tag and push to trigger the release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

This automatically builds binaries for 3 platforms and creates a GitHub Release page with download links.

---

## Step 3: Homebrew Tap

Create a separate repo for the Homebrew tap:

```bash
# Go to https://github.com/new
# Repository name: homebrew-engram
# Then:

mkdir -p /tmp/homebrew-engram/Formula
cp /Users/animeshdhillon/Desktop/myProjects/engram/Formula/engram.rb /tmp/homebrew-engram/Formula/

cd /tmp/homebrew-engram
git init
git add -A
git commit -m "Add engram formula"
git remote add origin git@github.com:animeshdhillon/homebrew-engram.git
git branch -M main
git push -u origin main
```

Update the formula to point to the release tarball (after Step 2 completes):

```ruby
class Engram < Formula
  desc "Persistent codebase intelligence daemon for coding agents"
  homepage "https://github.com/aniasusual/engram"
  url "https://github.com/aniasusual/engram/archive/refs/tags/v0.1.0.tar.gz"
  # sha256 "PUT_ACTUAL_SHA_HERE"  # Get this after the release is created
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "codebase intelligence", shell_output("#{bin}/engram --help")
  end
end
```

Users can then:
```bash
brew tap aniasusual/engram
brew install engram
```

---

## Step 4: cargo install Support

This already works. After pushing to GitHub, anyone with Rust can:

```bash
cargo install --git https://github.com/aniasusual/engram
```

To also publish to crates.io (optional, gives `cargo install engram`):

```bash
# 1. Create account at https://crates.io
# 2. Get API token from https://crates.io/settings/tokens
cargo login <your-token>

# 3. Verify the package
cargo publish --dry-run

# 4. Publish
cargo publish
```

Then users can:
```bash
cargo install engram
```

---

## Step 5: MCP Server Registration

For Claude Code users, they need to add Engram to their MCP config.

Create a one-liner install script at `scripts/install-mcp.sh`:

```bash
#!/bin/bash
# Install Engram as an MCP server for Claude Code
set -e

ENGRAM_BIN=$(which engram 2>/dev/null || echo "")
if [ -z "$ENGRAM_BIN" ]; then
    echo "Error: engram not found in PATH. Install it first:"
    echo "  cargo install --git https://github.com/aniasusual/engram"
    exit 1
fi

PROJECT_ROOT="${1:-.}"
PROJECT_ROOT=$(cd "$PROJECT_ROOT" && pwd)

# Initialize if needed
if [ ! -d "$PROJECT_ROOT/.engram" ]; then
    engram init --root "$PROJECT_ROOT"
fi

# Add to Claude Code MCP config
CLAUDE_CONFIG="$HOME/.claude.json"
if [ ! -f "$CLAUDE_CONFIG" ]; then
    echo '{"mcpServers":{}}' > "$CLAUDE_CONFIG"
fi

# Use python/node to merge JSON (portable)
python3 -c "
import json, sys
config_path = '$CLAUDE_CONFIG'
with open(config_path) as f:
    config = json.load(f)
config.setdefault('mcpServers', {})
config['mcpServers']['engram'] = {
    'command': '$ENGRAM_BIN',
    'args': ['mcp', '--root', '$PROJECT_ROOT']
}
with open(config_path, 'w') as f:
    json.dump(config, f, indent=2)
print(f'Added engram MCP server for {\"$PROJECT_ROOT\"} to {config_path}')
"

echo "Done! Restart Claude Code to activate."
```

---

## Step 6: First Release Announcement

### GitHub Release Notes (auto-generated in Step 2)

### README badges (add to top of README.md):

```markdown
[![CI](https://github.com/aniasusual/engram/actions/workflows/ci.yml/badge.svg)](https://github.com/aniasusual/engram/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
```

### Where to share:

1. **Reddit** — r/rust, r/programming, r/MachineLearning
   - Title: "Engram: Self-healing codebase memory for AI coding agents (single Rust binary, 28 MCP tools)"
   - Lead with the cascade demo and benchmark numbers

2. **Hacker News** — Show HN post
   - Title: "Show HN: Engram – Persistent codebase intelligence for coding agents"
   - Keep it factual, lead with what it does and benchmark numbers

3. **Twitter/X** — Thread format:
   - Tweet 1: What it is + the one-liner value prop
   - Tweet 2: Benchmark numbers (MRR 1.000, 1.6µs incremental, 16MB binary)
   - Tweet 3: Self-healing memory demo GIF
   - Tweet 4: "engram init && engram start" — that's it

4. **Claude Code Discord / Anthropic community** — Direct audience

5. **Dev.to / Hashnode** — Longer-form "How I built a self-healing memory system for coding agents"

---

## Step 7: Post-Launch (First Week)

```
Day 1:  Push to GitHub, create v0.1.0 release, share on Reddit + HN
Day 2:  Publish Homebrew tap, respond to issues/feedback
Day 3:  Publish crates.io, write Dev.to article
Day 4:  Record terminal demo GIF, share on Twitter
Day 5:  Monitor issues, fix any user-reported bugs
Day 6:  Collect feedback, plan v0.2.0
Day 7:  Write "Lessons from building Engram" blog post
```

---

## Quick Reference: All Install Methods

| Method | Command | Audience |
|--------|---------|----------|
| Pre-built binary | `curl -sL .../engram-macos-arm64.tar.gz \| tar xz` | Everyone |
| Homebrew | `brew tap aniasusual/engram && brew install engram` | macOS users |
| Cargo install | `cargo install --git https://github.com/aniasusual/engram` | Rust devs |
| Crates.io | `cargo install engram` | Rust devs |
| From source | `git clone ... && cargo build --release` | Contributors |
| Docker | Future — post v0.1.0 | Teams/CI |

---

## Files to Create Before Launch

- [x] README.md — done
- [x] PLAN.md — done
- [x] .github/workflows/ci.yml — done
- [ ] .github/workflows/release.yml — create from template above
- [ ] scripts/install-mcp.sh — create from template above
- [ ] LICENSE — create MIT license file
- [ ] CHANGELOG.md — create for v0.1.0
