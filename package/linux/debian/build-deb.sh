#!/bin/bash
# Build Debian package for reoxc
# Requires: dpkg-deb, binary in dist/linux-x64/reoxc

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
VERSION="0.5.0-beta"
ARCH="amd64"
PKG_NAME="reoxc_${VERSION}_${ARCH}"

echo "=== Building Debian Package ==="

# Check for binary
BINARY="$PROJECT_DIR/dist/linux-x64/reoxc"
if [ ! -f "$BINARY" ]; then
    echo "Error: Linux binary not found at $BINARY"
    echo "Run build/build-linux.sh first"
    exit 1
fi

# Create package structure
PKG_DIR="$SCRIPT_DIR/$PKG_NAME"
rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/usr/share/doc/reoxc"
mkdir -p "$PKG_DIR/usr/share/reox/runtime/include"
mkdir -p "$PKG_DIR/usr/share/reox/runtime/lib"

# Copy binary
cp "$BINARY" "$PKG_DIR/usr/bin/reoxc"
chmod 755 "$PKG_DIR/usr/bin/reoxc"

# Copy control files
cp "$SCRIPT_DIR/control" "$PKG_DIR/DEBIAN/"
cp "$SCRIPT_DIR/postinst" "$PKG_DIR/DEBIAN/"
chmod 755 "$PKG_DIR/DEBIAN/postinst"

# Copy documentation
cp "$PROJECT_DIR/LICENSE" "$PKG_DIR/usr/share/doc/reoxc/copyright"
cp "$PROJECT_DIR/README.md" "$PKG_DIR/usr/share/doc/reoxc/"

# Copy runtime headers (if available)
if [ -d "$PROJECT_DIR/reox-lang/runtime" ]; then
    cp "$PROJECT_DIR/reox-lang/runtime/"*.h "$PKG_DIR/usr/share/reox/runtime/include/" 2>/dev/null || true
fi

# Build package
dpkg-deb --build "$PKG_DIR"

# Move to dist
mkdir -p "$PROJECT_DIR/dist/packages"
mv "$SCRIPT_DIR/$PKG_NAME.deb" "$PROJECT_DIR/dist/packages/"

echo ""
echo "Package built: dist/packages/$PKG_NAME.deb"
echo ""
echo "To install: sudo dpkg -i dist/packages/$PKG_NAME.deb"

# Cleanup
rm -rf "$PKG_DIR"
