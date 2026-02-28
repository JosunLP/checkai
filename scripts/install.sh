#!/bin/sh
# CheckAI Installer — Linux & macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.sh | sh
set -e

REPO="JosunLP/checkai"
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="checkai"

echo "╔═══════════════════════════════════════╗"
echo "║       CheckAI Installer               ║"
echo "╚═══════════════════════════════════════╝"
echo ""

# --- Detect OS ---
OS="$(uname -s)"
case "$OS" in
    Linux)  OS="linux"  ;;
    Darwin) OS="darwin" ;;
    *)
        echo "Error: Unsupported operating system: $OS"
        echo "This installer supports Linux and macOS."
        echo "For Windows, use install.ps1 instead."
        exit 1
        ;;
esac

# --- Detect Architecture ---
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64)   ARCH="x86_64"  ;;
    aarch64|arm64)   ARCH="aarch64" ;;
    *)
        echo "Error: Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

ASSET_NAME="${BINARY_NAME}-${OS}-${ARCH}"
echo "Detected platform: ${OS}/${ARCH}"
echo "Asset: ${ASSET_NAME}"
echo ""

# --- Check for required tools ---
if ! command -v curl >/dev/null 2>&1; then
    echo "Error: 'curl' is required but not found. Please install curl first."
    exit 1
fi

# --- Get latest release info ---
echo "Fetching latest release..."
LATEST_URL="https://api.github.com/repos/${REPO}/releases/latest"
RELEASE_JSON="$(curl -sL "$LATEST_URL")"

# Extract download URL for our asset
DOWNLOAD_URL="$(echo "$RELEASE_JSON" | grep "browser_download_url.*${ASSET_NAME}\"" | head -1 | cut -d '"' -f 4)"

if [ -z "$DOWNLOAD_URL" ]; then
    echo "Error: Could not find release asset '${ASSET_NAME}'."
    echo "Available assets:"
    echo "$RELEASE_JSON" | grep '"name"' | head -10
    exit 1
fi

# Extract version
VERSION="$(echo "$RELEASE_JSON" | grep '"tag_name"' | head -1 | cut -d '"' -f 4)"
echo "Latest version: ${VERSION}"
echo "Download URL: ${DOWNLOAD_URL}"
echo ""

# --- Download binary ---
TEMP_FILE="$(mktemp)"
echo "Downloading ${ASSET_NAME}..."
curl -fSL "$DOWNLOAD_URL" -o "$TEMP_FILE"
chmod +x "$TEMP_FILE"

# --- Install ---
echo "Installing to ${INSTALL_DIR}/${BINARY_NAME}..."

# Check if we need sudo
if [ -w "$INSTALL_DIR" ]; then
    mv "$TEMP_FILE" "${INSTALL_DIR}/${BINARY_NAME}"
else
    echo "Requires elevated permissions to install to ${INSTALL_DIR}."
    sudo mv "$TEMP_FILE" "${INSTALL_DIR}/${BINARY_NAME}"
    sudo chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
fi

echo ""
echo "╔═══════════════════════════════════════╗"
echo "║  CheckAI installed successfully!      ║"
echo "╚═══════════════════════════════════════╝"
echo ""
echo "Version: ${VERSION}"
echo "Location: ${INSTALL_DIR}/${BINARY_NAME}"
echo ""
echo "Get started:"
echo "  checkai --help      Show help"
echo "  checkai serve       Start the API server"
echo "  checkai play        Play in the terminal"
echo ""
