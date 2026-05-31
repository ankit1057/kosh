#!/bin/bash

# Kosh Installer & Hook Configurator
# "Token Economics for Agentic Development"

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Initializing Kosh - The Token Elimination Infrastructure...${NC}"

# 1. Dependency Check: Rust/Cargo
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Cargo not found. Please install Rust: https://rustup.rs/${NC}"
    exit 1
fi

# 2. Install Kosh
echo -e "${BLUE}Installing Kosh CLI (v0.1.0)...${NC}"
cargo install --git https://github.com/ankit1057/kosh kosh-cli --force

# 3. Initialize Configuration
echo -e "${BLUE}Initializing .kosh configuration...${NC}"
kosh config init

# 4. Configure Shell Hooks (for human/agent parity)
SHELL_RC=""
case "$SHELL" in
    */zsh)  SHELL_RC="$HOME/.zshrc" ;;
    */bash) SHELL_RC="$HOME/.bashrc" ;;
esac

if [ -n "$SHELL_RC" ]; then
    echo -e "${BLUE}Configuring shell hooks in $SHELL_RC...${NC}"
    if ! grep -q "kosh" "$SHELL_RC"; then
        echo "" >> "$SHELL_RC"
        echo "# Kosh - Token Economics Hook" >> "$SHELL_RC"
        echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> "$SHELL_RC"
        echo 'alias gs="kosh gs"' >> "$SHELL_RC"
        echo 'alias gd="kosh gd"' >> "$SHELL_RC"
        echo 'alias gl="kosh gl"' >> "$SHELL_RC"
        echo -e "${GREEN}Shell hooks added! Restart your terminal or run: source $SHELL_RC${NC}"
    fi
fi

# 5. Configure Agent Hooks (Claude, Gemini, etc.)

# Claude Desktop / Claude Code MCP Hook
CLAUDE_CONFIG="$HOME/Library/Application Support/Claude/claude_desktop_config.json"
if [ -f "$CLAUDE_CONFIG" ]; then
    echo -e "${BLUE}Detecting Claude Desktop... adding Kosh MCP hook.${NC}"
    # Simple check if kosh is already there
    if ! grep -q "kosh" "$CLAUDE_CONFIG"; then
        echo -e "${RED}Manual action required: Add Kosh to your mcpServers in Claude Desktop config.${NC}"
        echo -e "Path: $CLAUDE_CONFIG"
    fi
fi

# 6. Success
echo -e "\n${GREEN}Kosh is successfully installed and hooked!${NC}"
echo -e "Try it now: ${BLUE}kosh gain --history${NC}"
echo -e "Whitepaper: https://github.com/ankit1057/kosh/blob/main/docs/whitepaper.md"
