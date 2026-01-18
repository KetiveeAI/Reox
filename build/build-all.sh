#!/bin/bash
# Master build script - builds all targets from Linux
# macOS build requires macOS host (or CI/CD)

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Reox Multi-Platform Build ==="
echo ""

# Build Linux
echo ">>> Building Linux..."
bash "$SCRIPT_DIR/build-linux.sh"
echo ""

# Build Windows
echo ">>> Building Windows..."
bash "$SCRIPT_DIR/build-windows.sh"
echo ""

# macOS can only be built on macOS
if [[ "$(uname)" == "Darwin" ]]; then
    echo ">>> Building macOS..."
    bash "$SCRIPT_DIR/build-macos.sh"
else
    echo ">>> Skipping macOS (requires macOS host)"
fi

echo ""
echo "=== Build Complete ==="
echo "Outputs in: $PROJECT_DIR/dist/"
ls -la "$PROJECT_DIR/dist/"
