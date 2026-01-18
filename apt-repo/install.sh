#!/bin/bash
# Reox Language Installer for Linux
# Usage: curl -fsSL https://reox.dev/install.sh | sh

set -e

echo "=== Reox Language Installer ==="
echo ""

# Detect distro
if [ -f /etc/debian_version ]; then
    DISTRO="debian"
elif [ -f /etc/fedora-release ]; then
    DISTRO="fedora"
elif [ -f /etc/arch-release ]; then
    DISTRO="arch"
else
    DISTRO="generic"
fi

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

install_debian() {
    echo -e "${GREEN}Installing via APT repository...${NC}"
    
    # Add GPG key
    echo "Adding GPG key..."
    curl -fsSL https://apt.reox.dev/reox.gpg | sudo gpg --dearmor -o /usr/share/keyrings/reox.gpg
    
    # Add repository
    echo "Adding repository..."
    echo "deb [signed-by=/usr/share/keyrings/reox.gpg] https://apt.reox.dev stable main" | \
        sudo tee /etc/apt/sources.list.d/reox.list > /dev/null
    
    # Update and install
    echo "Installing reoxc..."
    sudo apt update
    sudo apt install -y reoxc
    
    echo -e "${GREEN}✓ Installation complete!${NC}"
}

install_fedora() {
    echo -e "${YELLOW}Fedora: Installing binary directly...${NC}"
    install_binary
}

install_arch() {
    echo -e "${YELLOW}Arch: Installing binary directly...${NC}"
    echo "AUR package coming soon: yay -S reoxc"
    install_binary
}

install_binary() {
    VERSION="0.5.0-beta"
    BINARY_URL="https://github.com/KetiveeAI/reox/releases/download/v${VERSION}/reoxc-linux-x64"
    
    echo "Downloading reoxc ${VERSION}..."
    curl -fsSL "$BINARY_URL" -o /tmp/reoxc
    chmod +x /tmp/reoxc
    
    echo "Installing to /usr/local/bin..."
    sudo mv /tmp/reoxc /usr/local/bin/reoxc
    
    # Create symlink
    sudo ln -sf /usr/local/bin/reoxc /usr/local/bin/reox
    
    echo -e "${GREEN}✓ Installation complete!${NC}"
}

# Main
case $DISTRO in
    debian)
        install_debian
        ;;
    fedora)
        install_fedora
        ;;
    arch)
        install_arch
        ;;
    *)
        echo "Unknown distribution, installing binary..."
        install_binary
        ;;
esac

echo ""
echo "Verify installation:"
reoxc --version
echo ""
echo "Get started: https://reox.dev/docs/quickstart"
