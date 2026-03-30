#!/bin/sh
# CheckAI Uninstaller — Linux, macOS & Windows
# Cross-platform polyglot: valid in sh/bash and PowerShell.
#
#   Linux / macOS:  curl -fsSL https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/uninstall.sh | sh
#   Windows (PS):   irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/uninstall.sh | iex
#
# The script automatically detects the operating system.
echo --% >/dev/null;: ' | out-null
<#'

# ====================== POSIX Shell Section (Linux / macOS) ======================
set -e

INSTALL_DIR="/usr/local/bin"
BINARY_NAME="checkai"
BINARY_PATH="${INSTALL_DIR}/${BINARY_NAME}"

echo ""
echo "====================================="
echo "       CheckAI Uninstaller"
echo "====================================="
echo ""

prompt_tty_unavailable() {
    echo "No readable and writable /dev/tty is available for confirmation prompts. Aborting." >&2
    echo "Re-run the uninstall command from a terminal session that can provide interactive input." >&2
    return 2
}

prompt_yes_no() {
    if ! printf "%s" "$1" >/dev/tty 2>/dev/null; then
        prompt_tty_unavailable
        return $?
    fi

    if ! read -r REPLY </dev/tty 2>/dev/null; then
        prompt_tty_unavailable
        return $?
    fi

    case "$REPLY" in
        [yY]|[yY][eE][sS]) return 0 ;;
        *) return 1 ;;
    esac
}

handle_prompt_result() {
    PROMPT_STATUS=0
    prompt_yes_no "$1" || PROMPT_STATUS=$?

    case "$PROMPT_STATUS" in
        0)
            return 0
            ;;
        1)
            return 1
            ;;
        2)
            return 2
            ;;
    esac
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

PROMPT_STATUS=0
handle_prompt_result "Do you want to uninstall CheckAI? [y/N] " || PROMPT_STATUS=$?

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
    PROMPT_STATUS=0
    handle_prompt_result "Remove data directory (${DATA_DIR})? [y/N] " || PROMPT_STATUS=$?

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
echo "====================================="
echo "  CheckAI uninstalled successfully."
echo "====================================="
echo ""

exit 0
: '<#'
#>

# ====================== PowerShell Section (Windows / Linux / macOS) ======================

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "       CheckAI Uninstaller" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

# --- Detect OS ---
if ($IsLinux) {
    $binaryName = "checkai"
    $installDir = "/usr/local/bin"
    $dataDir = "$env:HOME/.local/share/checkai"
} elseif ($IsMacOS) {
    $binaryName = "checkai"
    $installDir = "/usr/local/bin"
    $dataDir = "$env:HOME/.local/share/checkai"
} else {
    $binaryName = "checkai.exe"
    $installDir = "$env:LOCALAPPDATA\checkai"
    $dataDir = "$env:LOCALAPPDATA\checkai-data"
}

$binaryPath = Join-Path $installDir $binaryName

# --- Check if installed ---
if (!(Test-Path $binaryPath)) {
    Write-Host "CheckAI is not installed at $binaryPath."
    Write-Host ""

    # Try to find it elsewhere
    $found = Get-Command $binaryName -ErrorAction SilentlyContinue
    if ($found) {
        Write-Host "Found checkai at: $($found.Source)"
        Write-Host "Remove it manually."
    }
    exit 0
}

Write-Host "Found CheckAI at: $binaryPath"

# --- Confirm removal ---
$confirm = Read-Host "Do you want to uninstall CheckAI? [y/N]"
if ($confirm -notmatch "^[yY]") {
    Write-Host "Aborted."
    exit 0
}

# --- Remove binary ---
Write-Host "Removing $binaryPath..."

if ($IsLinux -or $IsMacOS) {
    try {
        Remove-Item -Path $binaryPath -Force
    } catch {
        Write-Host "Requires elevated permissions. Using sudo..."
        sudo rm -f $binaryPath
    }
} else {
    Remove-Item -Path $binaryPath -Force

    # Also remove old version file if it exists
    $oldBinary = Join-Path $installDir "checkai.old.exe"
    if (Test-Path $oldBinary) {
        Remove-Item -Path $oldBinary -Force
    }

    # --- Remove install directory if empty ---
    $remaining = Get-ChildItem -Path $installDir -ErrorAction SilentlyContinue
    if (-not $remaining) {
        Remove-Item -Path $installDir -Force
        Write-Host "Removed empty install directory."
    }

    # --- Remove from PATH ---
    $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($currentPath -like "*$installDir*") {
        Write-Host "Removing $installDir from user PATH..."
        $newPath = ($currentPath -split ";" | Where-Object { $_ -ne $installDir }) -join ";"
        [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
        Write-Host "  PATH updated. You may need to restart your terminal." -ForegroundColor Yellow
    }
}

# --- Clean up data directory (optional) ---
if (Test-Path $dataDir) {
    $confirmData = Read-Host "Remove data directory ($dataDir)? [y/N]"
    if ($confirmData -match "^[yY]") {
        Remove-Item -Path $dataDir -Recurse -Force
        Write-Host "Data directory removed."
    } else {
        Write-Host "Data directory kept."
    }
}

Write-Host ""
Write-Host "=====================================" -ForegroundColor Green
Write-Host "  CheckAI uninstalled successfully." -ForegroundColor Green
Write-Host "=====================================" -ForegroundColor Green
Write-Host ""
