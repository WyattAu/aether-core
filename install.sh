#!/bin/bash
# Aether Installation Script

set -e

VERSION="${1:-latest}"
INSTALL_DIR="${2:-/usr/local/bin}"

echo "🛡️  Installing Aether..."

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    linux)  OS="unknown-linux-gnu" ;;
    darwin) OS="apple-darwin" ;;
    *)      echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
    x86_64|amd64)   ARCH="x86_64" ;;
    aarch64|arm64)  ARCH="aarch64" ;;
    *)              echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

TARGET="${ARCH}-${OS}"

# Get latest version if not specified
if [ "$VERSION" = "latest" ]; then
    VERSION=$(curl -s https://api.github.com/repos/aether-project/aether/releases/latest | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
fi

echo "   Version: $VERSION"
echo "   Target:  $TARGET"

# Download
URL="https://github.com/aether-project/aether/releases/download/${VERSION}/aether-${TARGET}.tar.gz"
echo "   URL: $URL"

curl -LO "$URL"
tar xzf "aether-${TARGET}.tar.gz"

# Install
sudo mv aether "$INSTALL_DIR/aether"
sudo chmod +x "$INSTALL_DIR/aether"

# Cleanup
rm "aether-${TARGET}.tar.gz"

echo ""
echo "✅ Aether installed to $INSTALL_DIR/aether"
echo ""
echo "Quick start:"
echo "  aether dev     # Start development environment"
echo "  aether deploy  # Deploy to cluster"
