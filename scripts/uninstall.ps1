# CheckAI Uninstaller — Windows (PowerShell)
# Usage: irm https://raw.githubusercontent.com/JosunLP/checkai/main/scripts/uninstall.ps1 | iex
$ErrorActionPreference = "Stop"

$binaryName = "checkai.exe"
$installDir = "$env:LOCALAPPDATA\checkai"
$binaryPath = Join-Path $installDir $binaryName

Write-Host ""
Write-Host "+===========================================+" -ForegroundColor Cyan
Write-Host "|       CheckAI Uninstaller (Windows)       |" -ForegroundColor Cyan
Write-Host "+===========================================+" -ForegroundColor Cyan
Write-Host ""

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

# --- Clean up data directory (optional) ---
$dataDir = "$env:LOCALAPPDATA\checkai-data"
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
Write-Host "+===========================================+" -ForegroundColor Green
Write-Host "|   CheckAI uninstalled successfully.       |" -ForegroundColor Green
Write-Host "+===========================================+" -ForegroundColor Green
Write-Host ""
