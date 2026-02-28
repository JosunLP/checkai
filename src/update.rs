//! Update and version-check module for CheckAI.
//!
//! This module provides functionality to:
//! - Check for new releases on GitHub at startup
//! - Self-update the binary to the latest version
//!
//! The update mechanism works cross-platform (Linux, macOS, Windows)
//! and downloads pre-built binaries from GitHub Releases.

use semver::Version;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// GitHub repository identifier (owner/repo).
const GITHUB_REPO: &str = "JosunLP/checkai";

/// Current version of this binary, read from Cargo.toml at compile time.
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

// ---------------------------------------------------------------------------
// GitHub API types
// ---------------------------------------------------------------------------

/// Minimal representation of a GitHub release.
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GitHubAsset>,
}

/// A single asset (binary) attached to a GitHub release.
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Checks GitHub for a newer version and prints a notice if one is available.
///
/// This function is designed to be called at startup. It will:
/// - Time out after 5 seconds to avoid slowing down the application
/// - Silently ignore any errors (no internet, rate-limited, etc.)
pub async fn check_for_updates() {
    // Use a timeout so we never block startup for too long
    let result =
        tokio::time::timeout(std::time::Duration::from_secs(5), check_latest_version()).await;

    match result {
        Ok(Ok(Some(info))) => {
            let current = CURRENT_VERSION;
            let latest = &info.version;
            let url = &info.url;

            // Build the notice dynamically so column alignment is clean
            println!();
            println!("  ╔══════════════════════════════════════════════════════════╗");
            println!("  ║  {:<57}║", t!("update.new_version_title"));
            println!(
                "  ║  {:<57}║",
                t!("update.current_latest", current = current, latest = latest)
            );
            println!("  ║                                                          ║");
            println!("  ║  {:<57}║", t!("update.run_update_hint"));
            println!("  ║  {:<57}║", url);
            println!("  ╚══════════════════════════════════════════════════════════╝");
            println!();
        }
        Ok(Ok(None)) => {
            // Already up to date — nothing to print
        }
        Ok(Err(_)) | Err(_) => {
            // Network error or timeout — silently ignore
        }
    }
}

/// Downloads the latest release and replaces the current binary.
///
/// This is the implementation behind `checkai update`.
pub async fn perform_update() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", t!("update.checking"));

    let info = check_latest_version().await?;

    let info = match info {
        Some(info) => info,
        None => {
            println!("{}", t!("update.up_to_date", version = CURRENT_VERSION));
            return Ok(());
        }
    };

    println!(
        "{}",
        t!(
            "update.updating",
            current = CURRENT_VERSION,
            latest = &info.version
        )
    );

    // Determine which release asset to download for this platform
    let asset_name = get_asset_name()?;

    let asset = info
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| {
            t!(
                "update.no_asset",
                expected = &asset_name,
                available = info
                    .assets
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .to_string()
        })?;

    println!("{}", t!("update.downloading", name = &asset.name));

    let client = build_client()?;
    let response = client
        .get(&asset.browser_download_url)
        .send()
        .await?
        .error_for_status()?;

    let bytes = response.bytes().await?;

    println!("{}", t!("update.downloaded", bytes = bytes.len()));

    // Write the new binary and replace the current one
    replace_binary(&bytes)?;

    println!();
    println!("{}", t!("update.success", version = &info.version));
    println!("{}", t!("update.restart_hint"));

    Ok(())
}

/// Returns the current version string.
pub fn version() -> &'static str {
    CURRENT_VERSION
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Information about an available update.
struct UpdateInfo {
    version: String,
    url: String,
    assets: Vec<GitHubAsset>,
}

/// Queries the GitHub Releases API and returns update info if a newer
/// version is available, or `None` if we are already up to date.
async fn check_latest_version() -> Result<Option<UpdateInfo>, Box<dyn std::error::Error>> {
    let client = build_client()?;

    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );

    let release: GitHubRelease = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let latest_tag = release.tag_name.trim_start_matches('v');
    let latest = Version::parse(latest_tag)?;
    let current = Version::parse(CURRENT_VERSION)?;

    if latest > current {
        Ok(Some(UpdateInfo {
            version: latest.to_string(),
            url: release.html_url,
            assets: release.assets,
        }))
    } else {
        Ok(None)
    }
}

/// Creates a `reqwest::Client` with a proper User-Agent header
/// (required by the GitHub API).
fn build_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .user_agent(format!("checkai/{}", CURRENT_VERSION))
        .timeout(std::time::Duration::from_secs(30))
        .build()
}

/// Returns the expected release-asset filename for the current platform.
fn get_asset_name() -> Result<String, String> {
    let os = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        return Err(t!("update.unsupported_os").to_string());
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        return Err(t!("update.unsupported_arch").to_string());
    };

    let ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };

    Ok(format!("checkai-{}-{}{}", os, arch, ext))
}

/// Writes the downloaded bytes as the new binary, replacing the currently
/// running executable.
fn replace_binary(bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let current_exe = std::env::current_exe()?;

    // ── Unix ──────────────────────────────────────────────────────────────
    // On Unix we can write to a temp file and atomically rename it over the
    // running binary (Unix allows unlinking/renaming open files).
    #[cfg(unix)]
    {
        let temp_path = temp_binary_path(&current_exe);
        std::fs::write(&temp_path, bytes)?;

        // Make it executable
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o755))?;

        // Atomic rename
        std::fs::rename(&temp_path, &current_exe)?;
    }

    // ── Windows ───────────────────────────────────────────────────────────
    // On Windows the running executable is locked. The standard trick is:
    //   1. Rename the running exe to a .old backup
    //   2. Write the new exe in its place
    //   3. The .old file can be deleted on the next run
    #[cfg(windows)]
    {
        let old_path = current_exe.with_extension("old.exe");
        let temp_path = temp_binary_path(&current_exe);

        // Write new binary to temp
        std::fs::write(&temp_path, bytes)?;

        // Remove previous .old if it exists
        let _ = std::fs::remove_file(&old_path);

        // Rename running exe → .old
        std::fs::rename(&current_exe, &old_path)?;

        // Rename new → current
        std::fs::rename(&temp_path, &current_exe)?;

        log::info!(
            "Old binary backed up to {}. It will be cleaned up automatically.",
            old_path.display()
        );
    }

    Ok(())
}

/// Returns a temporary path next to the given binary for staging downloads.
fn temp_binary_path(current_exe: &Path) -> PathBuf {
    let mut temp = current_exe.to_path_buf();
    temp.set_extension("update-tmp");
    temp
}

/// Cleans up leftover `.old.exe` files from previous updates (Windows only).
/// Call this early at startup.
pub fn cleanup_old_binary() {
    #[cfg(windows)]
    {
        if let Ok(exe) = std::env::current_exe() {
            let old = exe.with_extension("old.exe");
            if old.exists() {
                let _ = std::fs::remove_file(&old);
            }
        }
    }
}
