# CheckAI Installer — Windows (PowerShell)
# Usage: irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/install.ps1 | iex
$ErrorActionPreference = "Stop"

$repo = "JosunLP/checkai"
$binaryName = "checkai.exe"
$installDir = "$env:LOCALAPPDATA\checkai"

Write-Host ""
Write-Host "+===========================================+" -ForegroundColor Cyan
Write-Host "|         CheckAI Installer (Windows)       |" -ForegroundColor Cyan
Write-Host "+===========================================+" -ForegroundColor Cyan
Write-Host ""

# --- Detect Architecture ---
$arch = if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq [System.Runtime.InteropServices.Architecture]::Arm64) {
    "aarch64"
} else {
    "x86_64"
}

$assetName = "checkai-windows-${arch}.exe"
Write-Host "Detected platform: windows/${arch}"
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

# --- Create install directory ---
if (!(Test-Path $installDir)) {
    Write-Host "Creating install directory: $installDir"
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}


# --- Download binary ---
$downloadUrl = $asset.browser_download_url
$targetPath = Join-Path $installDir $binaryName

Write-Host "Downloading $assetName..."
try {
    Invoke-WebRequest -Uri $downloadUrl -OutFile $targetPath -UseBasicParsing
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

Write-Host ""
Write-Host "+===========================================+" -ForegroundColor Green
Write-Host "|   CheckAI installed successfully!         |" -ForegroundColor Green
Write-Host "+===========================================+" -ForegroundColor Green
Write-Host ""
Write-Host "Version:  $version"
Write-Host "Location: $targetPath"
Write-Host ""
Write-Host "Get started:"
Write-Host "  checkai --help      Show help"
Write-Host "  checkai serve       Start the API server"
Write-Host "  checkai play        Play in the terminal"
Write-Host ""
