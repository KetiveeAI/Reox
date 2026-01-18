#!/bin/bash
# Build Reox compiler for macOS (universal binary: x86_64 + arm64)
# Must be run on macOS

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR/reox-lang"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}=== Reox macOS Build ===${NC}"

# Verify running on macOS
if [[ "$(uname)" != "Darwin" ]]; then
    echo -e "${RED}Error: This script must be run on macOS${NC}"
    exit 1
fi

# Ensure both targets are installed
echo -e "${YELLOW}Checking Rust targets...${NC}"
rustup target add x86_64-apple-darwin aarch64-apple-darwin 2>/dev/null || true

# Build for Intel
echo -e "${GREEN}Building for x86_64 (Intel)...${NC}"
cargo build --release --target x86_64-apple-darwin

# Build for Apple Silicon
echo -e "${GREEN}Building for aarch64 (Apple Silicon)...${NC}"
cargo build --release --target aarch64-apple-darwin

# Create universal binary
echo -e "${GREEN}Creating universal binary...${NC}"
mkdir -p "$PROJECT_DIR/dist/macos-universal"

lipo -create \
    target/x86_64-apple-darwin/release/reoxc \
    target/aarch64-apple-darwin/release/reoxc \
    -output "$PROJECT_DIR/dist/macos-universal/reoxc"

# Get version
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')

if [ -f "$PROJECT_DIR/dist/macos-universal/reoxc" ]; then
    echo -e "${GREEN}Build successful!${NC}"
    echo "Universal binary: dist/macos-universal/reoxc"
    echo "Size: $(du -h "$PROJECT_DIR/dist/macos-universal/reoxc" | cut -f1)"
    
    # Verify universal binary
    echo "Architectures:"
    lipo -info "$PROJECT_DIR/dist/macos-universal/reoxc"
else
    echo -e "${RED}Build failed${NC}"
    exit 1
fi

echo ""
echo "To test: ./dist/macos-universal/reoxc --version"
