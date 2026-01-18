#!/bin/bash
# Cross-compile Reox compiler for Windows from Linux
# Output: target/x86_64-pc-windows-gnu/release/reoxc.exe

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR/reox-lang"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}=== Reox Windows Cross-Compile ===${NC}"

# Check if Windows target is installed
if ! rustup target list --installed | grep -q "x86_64-pc-windows-gnu"; then
    echo -e "${YELLOW}Installing Windows target...${NC}"
    rustup target add x86_64-pc-windows-gnu
fi

# Check for mingw-w64 cross compiler
if ! command -v x86_64-w64-mingw32-gcc &> /dev/null; then
    echo -e "${RED}Error: mingw-w64 cross compiler not found${NC}"
    echo "Install with: sudo apt install mingw-w64"
    exit 1
fi

# Build release binary for Windows
echo -e "${GREEN}Building Windows release binary...${NC}"
cargo build --release --target x86_64-pc-windows-gnu

# Get version
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
BINARY="target/x86_64-pc-windows-gnu/release/reoxc.exe"

if [ -f "$BINARY" ]; then
    echo -e "${GREEN}Build successful!${NC}"
    echo "Binary: $BINARY"
    echo "Size: $(du -h "$BINARY" | cut -f1)"
    
    # Copy to output directory
    mkdir -p "$PROJECT_DIR/dist/windows-x64"
    cp "$BINARY" "$PROJECT_DIR/dist/windows-x64/reoxc.exe"
    echo "Copied to: dist/windows-x64/reoxc.exe"
else
    echo -e "${RED}Build failed${NC}"
    exit 1
fi

echo ""
echo "To test on Windows: dist\\windows-x64\\reoxc.exe --version"
