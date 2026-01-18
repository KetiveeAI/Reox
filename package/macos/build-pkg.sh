#!/bin/bash
# Build macOS .pkg installer for reoxc
# Must be run on macOS

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
VERSION="0.5.0-beta"
PKG_NAME="reoxc-${VERSION}-macos"

echo "=== Building macOS Package ==="

# Verify macOS
if [[ "$(uname)" != "Darwin" ]]; then
    echo "Error: This script must be run on macOS"
    exit 1
fi

# Check for binary
BINARY="$PROJECT_DIR/dist/macos-universal/reoxc"
if [ ! -f "$BINARY" ]; then
    echo "Error: macOS binary not found at $BINARY"
    echo "Run build/build-macos.sh first"
    exit 1
fi

# Create package root
PKG_ROOT="$SCRIPT_DIR/pkg-root"
rm -rf "$PKG_ROOT"
mkdir -p "$PKG_ROOT/usr/local/bin"
mkdir -p "$PKG_ROOT/usr/local/include/reox"
mkdir -p "$PKG_ROOT/usr/local/lib/reox"

# Copy binary
cp "$BINARY" "$PKG_ROOT/usr/local/bin/reoxc"
chmod 755 "$PKG_ROOT/usr/local/bin/reoxc"

# Copy runtime headers
if [ -d "$PROJECT_DIR/reox-lang/runtime" ]; then
    cp "$PROJECT_DIR/reox-lang/runtime/"*.h "$PKG_ROOT/usr/local/include/reox/" 2>/dev/null || true
fi

# Create component plist
cat > "$SCRIPT_DIR/component.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<array>
    <dict>
        <key>BundleHasStrictIdentifier</key>
        <false/>
        <key>BundleIsRelocatable</key>
        <false/>
        <key>BundleOverwriteAction</key>
        <string>upgrade</string>
        <key>RootRelativeBundlePath</key>
        <string>usr/local/bin/reoxc</string>
    </dict>
</array>
</plist>
EOF

# Build package
pkgbuild --root "$PKG_ROOT" \
         --identifier "com.ketivee.reoxc" \
         --version "$VERSION" \
         --install-location "/" \
         --component-plist "$SCRIPT_DIR/component.plist" \
         "$SCRIPT_DIR/$PKG_NAME.pkg"

# Create distribution file for productbuild (optional, for signed installer)
cat > "$SCRIPT_DIR/distribution.xml" << EOF
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>REOX Compiler</title>
    <welcome file="welcome.html"/>
    <license file="license.txt"/>
    <pkg-ref id="com.ketivee.reoxc"/>
    <options customize="never" require-scripts="false"/>
    <choices-outline>
        <line choice="default">
            <line choice="com.ketivee.reoxc"/>
        </line>
    </choices-outline>
    <choice id="default"/>
    <choice id="com.ketivee.reoxc" visible="false">
        <pkg-ref id="com.ketivee.reoxc"/>
    </choice>
    <pkg-ref id="com.ketivee.reoxc" version="$VERSION" onConclusion="none">$PKG_NAME.pkg</pkg-ref>
</installer-gui-script>
EOF

# Move to dist
mkdir -p "$PROJECT_DIR/dist/packages"
mv "$SCRIPT_DIR/$PKG_NAME.pkg" "$PROJECT_DIR/dist/packages/"

echo ""
echo "Package built: dist/packages/$PKG_NAME.pkg"
echo ""
echo "To install: open dist/packages/$PKG_NAME.pkg"

# Cleanup
rm -rf "$PKG_ROOT"
rm -f "$SCRIPT_DIR/component.plist"
