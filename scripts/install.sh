#!/bin/sh
# CheckAI Installer — Linux, macOS & Windows
# Cross-platform polyglot: valid in sh/bash and PowerShell.
#
#   Linux / macOS:  curl -fsSL https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.sh | sh
#   Windows (PS):   irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.sh | iex
#
# The script automatically detects the operating system and CPU architecture.
# No manual version entry is required — the latest GitHub release is fetched.
#
# Polyglot boundary — in sh, the backticks run `# | Out-Null <#` as a
# command substitution, where `#` starts a shell comment so the rest of
# the line is ignored and the substitution expands to an empty string.
# In PowerShell, `# becomes a literal #, the output is piped to
# Out-Null, and <# starts a block comment that hides the shell section.
echo `# | Out-Null <#`

# ====================== POSIX Shell Section (Linux / macOS) ======================
set -e

REPO="JosunLP/checkai"
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="checkai"

echo ""
echo "====================================="
echo "       CheckAI Installer"
echo "====================================="
echo ""

# --- Detect OS ---
OS="$(uname -s)"
case "$OS" in
    Linux)  OS="linux"  ;;
    Darwin) OS="darwin" ;;
    *)
        echo "Error: Unsupported operating system for the POSIX shell installer: $OS"
        echo "This shell path supports Linux and macOS."
        echo "On Windows, run the installer in PowerShell instead:"
        echo "  irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.sh | iex"
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
echo "====================================="
echo "  CheckAI installed successfully!"
echo "====================================="
echo ""
echo "Version: ${VERSION}"
echo "Location: ${INSTALL_DIR}/${BINARY_NAME}"
echo ""
echo "Get started:"
echo "  checkai --help      Show help"
echo "  checkai serve       Start the API server"
echo "  checkai play        Play in the terminal"
echo ""

exit 0
#>

# ====================== PowerShell Section (Windows / Linux / macOS) ======================

$ErrorActionPreference = "Stop"

$repo = "JosunLP/checkai"

function Invoke-CheckAIDownload {
    param(
        [string]$Uri,
        [string]$OutFile
    )

    $requestParams = @{
        Uri = $Uri
        OutFile = $OutFile
    }

    if ($PSVersionTable.PSVersion.Major -lt 6) {
        $requestParams.UseBasicParsing = $true
    }

    Invoke-WebRequest @requestParams
}

function Assert-NativeCommandSucceeded {
    param(
        [string]$ErrorMessage
    )

    if ($LASTEXITCODE -ne 0) {
        Write-Error $ErrorMessage
        exit $LASTEXITCODE
    }
}

Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "       CheckAI Installer" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

# --- Detect OS ---
if ($IsLinux) {
    $os = "linux"
    $binaryName = "checkai"
    $installDir = "/usr/local/bin"
} elseif ($IsMacOS) {
    $os = "darwin"
    $binaryName = "checkai"
    $installDir = "/usr/local/bin"
} else {
    $os = "windows"
    $binaryName = "checkai.exe"
    $installDir = "$env:LOCALAPPDATA\checkai"
}

# --- Detect Architecture ---
$osArchitecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
$arch = if ($osArchitecture -eq [System.Runtime.InteropServices.Architecture]::X64) {
    "x86_64"
} elseif ($osArchitecture -eq [System.Runtime.InteropServices.Architecture]::Arm64) {
    "aarch64"
} else {
    Write-Error "Unsupported architecture: $osArchitecture. Supported architectures are X64 and Arm64."
    exit 1
}

if ($os -eq "windows") {
    $assetName = "checkai-windows-${arch}.exe"
} else {
    $assetName = "checkai-${os}-${arch}"
}

Write-Host "Detected platform: ${os}/${arch}"
Write-Host "Asset: ${assetName}"
Write-Host ""

# --- Get latest release info ---
Write-Host "Fetching latest release..."
$latestUrl = "https://api.github.com/repos/$repo/releases/latest"

try {
    $release = Invoke-RestMethod -Uri $latestUrl -Headers @{
        "User-Agent" = "checkai-installer"
    }
} catch {
    Write-Error "Failed to fetch latest release: $_"
    exit 1
}

$version = $release.tag_name
$asset = $release.assets | Where-Object { $_.name -eq $assetName }

if (-not $asset) {
    Write-Error "Could not find release asset '$assetName'."
    Write-Host "Available assets:"
    $release.assets | ForEach-Object { Write-Host "  - $($_.name)" }
    exit 1
}

Write-Host "Latest version: $version"
Write-Host ""

$downloadUrl = $asset.browser_download_url

if ($os -eq "windows") {
    # --- Windows: install to LOCALAPPDATA ---
    if (!(Test-Path $installDir)) {
        Write-Host "Creating install directory: $installDir"
        New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    }

    $targetPath = Join-Path $installDir $binaryName
    Write-Host "Downloading $assetName..."
    try {
        Invoke-CheckAIDownload -Uri $downloadUrl -OutFile $targetPath
    } catch {
        Write-Error "Failed to download: $_"
        exit 1
    }

    # --- Add to PATH ---
    $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($currentPath -notlike "*$installDir*") {
        Write-Host "Adding $installDir to user PATH..."
        [Environment]::SetEnvironmentVariable("PATH", "$currentPath;$installDir", "User")
        $env:PATH = "$env:PATH;$installDir"
        Write-Host "  PATH updated. You may need to restart your terminal." -ForegroundColor Yellow
    }
} else {
    # --- Linux / macOS: install to /usr/local/bin ---
    $tempFile = [System.IO.Path]::GetTempFileName()
    Write-Host "Downloading $assetName..."
    try {
        Invoke-CheckAIDownload -Uri $downloadUrl -OutFile $tempFile
    } catch {
        Write-Error "Failed to download: $_"
        exit 1
    }

    chmod +x $tempFile
    Assert-NativeCommandSucceeded "Failed to mark $tempFile as executable."

    $targetPath = Join-Path $installDir $binaryName
    if (Test-Path $installDir -PathType Container) {
        try {
            Move-Item -Force $tempFile $targetPath
        } catch {
            Write-Host "Requires elevated permissions. Using sudo..."
            sudo mv $tempFile $targetPath
            Assert-NativeCommandSucceeded "Failed to move $tempFile to $targetPath with sudo."
            sudo chmod +x $targetPath
            Assert-NativeCommandSucceeded "Failed to mark $targetPath as executable with sudo."
        }
    } else {
        Write-Host "Creating install directory with sudo: $installDir"
        sudo mkdir -p $installDir
        Assert-NativeCommandSucceeded "Failed to create install directory $installDir with sudo."
        sudo mv $tempFile $targetPath
        Assert-NativeCommandSucceeded "Failed to move $tempFile to $targetPath with sudo."
        sudo chmod +x $targetPath
        Assert-NativeCommandSucceeded "Failed to mark $targetPath as executable with sudo."
    }
}

Write-Host ""
Write-Host "=====================================" -ForegroundColor Green
Write-Host "  CheckAI installed successfully!" -ForegroundColor Green
Write-Host "=====================================" -ForegroundColor Green
Write-Host ""
Write-Host "Version:  $version"
Write-Host "Location: $targetPath"
Write-Host ""
Write-Host "Get started:"
Write-Host "  checkai --help      Show help"
Write-Host "  checkai serve       Start the API server"
Write-Host "  checkai play        Play in the terminal"
Write-Host ""
