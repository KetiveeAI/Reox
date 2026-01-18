#!/bin/bash
# Build Reox compiler for Linux (static binary using musl)
# Output: target/x86_64-unknown-linux-musl/release/reoxc

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR/reox-lang"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}=== Reox Linux Build ===${NC}"

# Check if musl target is installed
if ! rustup target list --installed | grep -q "x86_64-unknown-linux-musl"; then
    echo -e "${YELLOW}Installing musl target...${NC}"
    rustup target add x86_64-unknown-linux-musl
fi

# Check for musl-gcc (required for static linking)
if ! command -v musl-gcc &> /dev/null; then
    echo -e "${RED}Error: musl-gcc not found${NC}"
    echo "Install with: sudo apt install musl-tools"
    exit 1
fi

# Build release binary
echo -e "${GREEN}Building static release binary...${NC}"
RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-musl

# Get version from Cargo.toml
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
BINARY="target/x86_64-unknown-linux-musl/release/reoxc"

if [ -f "$BINARY" ]; then
    echo -e "${GREEN}Build successful!${NC}"
    echo "Binary: $BINARY"
    echo "Size: $(du -h "$BINARY" | cut -f1)"
    
    # Verify it's statically linked
    if ldd "$BINARY" 2>&1 | grep -q "not a dynamic executable"; then
        echo -e "${GREEN}✓ Static binary confirmed${NC}"
    fi
    
    # Copy to output directory
    mkdir -p "$PROJECT_DIR/dist/linux-x64"
    cp "$BINARY" "$PROJECT_DIR/dist/linux-x64/reoxc"
    echo "Copied to: dist/linux-x64/reoxc"
else
    echo -e "${RED}Build failed${NC}"
    exit 1
fi

echo ""
echo "To test: ./dist/linux-x64/reoxc --version"
