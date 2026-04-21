#!/bin/bash
# Install Engram as an MCP server for Claude Code.
# Usage: ./scripts/install-mcp.sh [project-root]
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
    echo "Initializing Engram in $PROJECT_ROOT..."
    engram init --root "$PROJECT_ROOT"
fi

# Add to Claude Code MCP config
CLAUDE_CONFIG="$HOME/.claude.json"
if [ ! -f "$CLAUDE_CONFIG" ]; then
    echo '{"mcpServers":{}}' > "$CLAUDE_CONFIG"
fi

python3 -c "
import json
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
print(f'Added engram MCP server for $PROJECT_ROOT to $CLAUDE_CONFIG')
"

echo "Done! Restart Claude Code to activate."
