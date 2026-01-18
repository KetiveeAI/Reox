#!/bin/bash
# Update APT repository metadata
# Run this after adding new .deb packages to pool/main/

set -e

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DIST_DIR="$REPO_DIR/dists/stable"
POOL_DIR="$REPO_DIR/pool/main"

echo "=== Updating Reox APT Repository ==="
cd "$REPO_DIR"

# Check for dpkg-scanpackages
if ! command -v dpkg-scanpackages &> /dev/null; then
    echo "Error: dpkg-scanpackages not found"
    echo "Install with: sudo apt install dpkg-dev"
    exit 1
fi

# Generate Packages file
echo "Generating Packages..."
mkdir -p "$DIST_DIR/main/binary-amd64"

dpkg-scanpackages --arch amd64 pool/ > "$DIST_DIR/main/binary-amd64/Packages"
gzip -kf "$DIST_DIR/main/binary-amd64/Packages"

# Generate Release file
echo "Generating Release..."
cat > "$DIST_DIR/Release" << EOF
Origin: Reox
Label: Reox
Suite: stable
Codename: stable
Version: 1.0
Architectures: amd64
Components: main
Description: Reox Language APT Repository
Date: $(date -Ru)
EOF

# Add checksums to Release
echo "MD5Sum:" >> "$DIST_DIR/Release"
for f in main/binary-amd64/Packages main/binary-amd64/Packages.gz; do
    if [ -f "$DIST_DIR/$f" ]; then
        HASH=$(md5sum "$DIST_DIR/$f" | cut -d' ' -f1)
        SIZE=$(wc -c < "$DIST_DIR/$f")
        echo " $HASH $SIZE $f" >> "$DIST_DIR/Release"
    fi
done

echo "SHA256:" >> "$DIST_DIR/Release"
for f in main/binary-amd64/Packages main/binary-amd64/Packages.gz; do
    if [ -f "$DIST_DIR/$f" ]; then
        HASH=$(sha256sum "$DIST_DIR/$f" | cut -d' ' -f1)
        SIZE=$(wc -c < "$DIST_DIR/$f")
        echo " $HASH $SIZE $f" >> "$DIST_DIR/Release"
    fi
done

# Sign Release if GPG key available
if gpg --list-keys "reox@ketivee.com" &> /dev/null; then
    echo "Signing Release..."
    gpg --default-key "reox@ketivee.com" -abs -o "$DIST_DIR/Release.gpg" "$DIST_DIR/Release"
    gpg --default-key "reox@ketivee.com" --clearsign -o "$DIST_DIR/InRelease" "$DIST_DIR/Release"
    echo "Signed with GPG key"
else
    echo "Warning: GPG key not found, skipping signing"
    echo "Generate with: gpg --gen-key (use reox@ketivee.com)"
fi

echo ""
echo "=== Repository Updated ==="
echo "Packages in pool:"
ls -la "$POOL_DIR"/*.deb 2>/dev/null || echo "  (none yet)"
echo ""
echo "To add packages, copy .deb files to pool/main/ and run this script again."
