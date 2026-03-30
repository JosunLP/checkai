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
    if [ -r /dev/tty ] && [ -w /dev/tty ]; then
        if ! printf "%s" "$1" >/dev/tty; then
            echo "Failed to write to /dev/tty. Aborting." >&2
            return 2
        fi
        if ! read -r REPLY </dev/tty; then
            echo "Failed to read user input from /dev/tty. Aborting." >&2
            return 2
        fi
        case "$REPLY" in
            [yY]|[yY][eE][sS]) return 0 ;;
            *) return 1 ;;
        esac
    else
        echo "No readable and writable /dev/tty is available for confirmation prompts. Aborting." >&2
        echo "Re-run the uninstall command from a terminal session that can provide interactive input." >&2
        return 2
    fi
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

if prompt_yes_no "Do you want to uninstall CheckAI? [y/N] "; then
    PROMPT_STATUS=0
else
    PROMPT_STATUS=$?
fi

case "$PROMPT_STATUS" in
    1)
        echo "Aborted."
        exit 0
        ;;
    2)
        exit 1
        ;;
esac

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
        PROMPT_STATUS=0
    else
        PROMPT_STATUS=$?
    fi

    case "$PROMPT_STATUS" in
        0)
            rm -rf "$DATA_DIR"
            echo "Data directory removed."
            ;;
        1)
            echo "Data directory kept."
            ;;
        2)
            exit 1
            ;;
    esac
fi

echo ""
echo "╔═══════════════════════════════════════╗"
echo "║  CheckAI uninstalled successfully.    ║"
echo "╚═══════════════════════════════════════╝"
echo ""
