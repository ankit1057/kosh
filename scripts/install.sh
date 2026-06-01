#!/bin/bash
# Kosh Installer
# Token elimination infrastructure for AI agents
# https://github.com/ankit1057/kosh

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Installing Kosh — token elimination for AI agents...${NC}"

# 1. Dependency check
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Cargo not found. Install Rust first: https://rustup.rs/${NC}"
    exit 1
fi

# 2. Install kosh binary
echo -e "${BLUE}Building and installing kosh CLI...${NC}"
cargo install --git https://github.com/ankit1057/kosh --bin kosh --force

# 3. Initialize project config
echo -e "${BLUE}Initializing .kosh configuration...${NC}"
kosh config init

# 4. Add cargo bin to PATH in shell rc if needed
SHELL_RC=""
case "$SHELL" in
    */zsh)  SHELL_RC="$HOME/.zshrc" ;;
    */bash) SHELL_RC="$HOME/.bashrc" ;;
esac

if [ -n "$SHELL_RC" ]; then
    if ! grep -q 'cargo/bin' "$SHELL_RC"; then
        echo '' >> "$SHELL_RC"
        echo '# Kosh' >> "$SHELL_RC"
        echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> "$SHELL_RC"
        echo -e "${GREEN}Added cargo/bin to PATH in $SHELL_RC${NC}"
        echo -e "Run: ${BLUE}source $SHELL_RC${NC}"
    fi
fi

echo -e "\n${GREEN}Kosh installed.${NC}"
echo -e "  ${BLUE}kosh gain${NC}            — see token savings"
echo -e "  ${BLUE}kosh lease list${NC}      — list context leases"
echo -e "  ${BLUE}kosh packet list${NC}     — list context packets"
echo -e "  ${BLUE}kosh --help${NC}          — all commands"
echo -e "\nhttps://github.com/ankit1057/kosh"
