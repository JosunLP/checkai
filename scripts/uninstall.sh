#!/bin/sh
# CheckAI Uninstaller — Linux & macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/uninstall.sh | sh
set -e

INSTALL_DIR="/usr/local/bin"
BINARY_NAME="checkai"
BINARY_PATH="${INSTALL_DIR}/${BINARY_NAME}"

echo "╔═══════════════════════════════════════╗"
echo "║       CheckAI Uninstaller             ║"
echo "╚═══════════════════════════════════════╝"
echo ""

prompt_yes_no() {
    if [ -r /dev/tty ]; then
        printf "%s" "$1" >/dev/tty
        if ! read -r REPLY </dev/tty; then
            echo "No input received from the terminal. Aborting." >&2
            return 1
        fi
        case "$REPLY" in
            [yY]|[yY][eE][sS]) return 0 ;;
        esac
    fi
    echo "No terminal available for interactive prompts. Aborting." >&2
    echo "Run the uninstall script directly instead of piping it if you need to answer the prompts." >&2
    return 1
}

# --- Check if installed ---
if [ ! -f "$BINARY_PATH" ]; then
    echo "CheckAI is not installed at ${BINARY_PATH}."
    echo ""

    # Try to find it elsewhere
    FOUND="$(command -v "$BINARY_NAME" 2>/dev/null || true)"
    if [ -n "$FOUND" ]; then
        echo "Found checkai at: $FOUND"
        echo "Remove it manually with: rm $FOUND"
    fi
    exit 0
fi

# --- Confirm removal ---
echo "Found CheckAI at: ${BINARY_PATH}"

if ! prompt_yes_no "Do you want to uninstall CheckAI? [y/N] "; then
    echo "Aborted."
    exit 0
fi

# --- Remove binary ---
echo "Removing ${BINARY_PATH}..."

if [ -w "$INSTALL_DIR" ]; then
    rm -f "$BINARY_PATH"
else
    echo "Requires elevated permissions."
    sudo rm -f "$BINARY_PATH"
fi

# --- Clean up data directory (optional) ---
DATA_DIR="${HOME}/.local/share/checkai"
if [ -d "$DATA_DIR" ]; then
    if prompt_yes_no "Remove data directory (${DATA_DIR})? [y/N] "; then
        rm -rf "$DATA_DIR"
        echo "Data directory removed."
    else
        echo "Data directory kept."
    fi
fi

echo ""
echo "╔═══════════════════════════════════════╗"
echo "║  CheckAI uninstalled successfully.    ║"
echo "╚═══════════════════════════════════════╝"
echo ""
