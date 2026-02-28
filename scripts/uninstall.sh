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

if [ -t 0 ]; then
    printf "Do you want to uninstall CheckAI? [y/N] "
    read -r CONFIRM
    case "$CONFIRM" in
        [yY]|[yY][eE][sS]) ;;
        *)
            echo "Aborted."
            exit 0
            ;;
    esac
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
    if [ -t 0 ]; then
        printf "Remove data directory (%s)? [y/N] " "$DATA_DIR"
        read -r CONFIRM_DATA
        case "$CONFIRM_DATA" in
            [yY]|[yY][eE][sS])
                rm -rf "$DATA_DIR"
                echo "Data directory removed."
                ;;
            *)
                echo "Data directory kept."
                ;;
        esac
    fi
fi

echo ""
echo "╔═══════════════════════════════════════╗"
echo "║  CheckAI uninstalled successfully.    ║"
echo "╚═══════════════════════════════════════╝"
echo ""
