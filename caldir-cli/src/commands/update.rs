use anyhow::{bail, Result};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::path::PathBuf;

const REPO: &str = "t4t5/caldir";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// All binaries included in a caldir release.
const RELEASE_BINARIES: &[&str] = &[
    "caldir",
    "caldir-provider-google",
    "caldir-provider-icloud",
];

pub async fn run() -> Result<()> {
    let spinner = crate::utils::tui::create_spinner("Checking for updates...".to_string());

    let latest = fetch_latest_release().await?;
    spinner.finish_and_clear();

    let latest_version = latest.tag_name.trim_start_matches('v');

    if latest_version == CURRENT_VERSION {
        println!(
            "Already up to date ({}).",
            format!("v{}", CURRENT_VERSION).dimmed()
        );
        return Ok(());
    }

    let install_dir = get_install_dir()?;

    // Find which release binaries are currently installed
    let installed: Vec<&str> = RELEASE_BINARIES
        .iter()
        .filter(|bin| install_dir.join(bin).exists())
        .copied()
        .collect();

    println!(
        "  {} {} → {}",
        "caldir".bold(),
        format!("v{}", CURRENT_VERSION).dimmed(),
        format!("v{}", latest_version).green(),
    );

    for bin in &installed {
        if *bin != "caldir" {
            println!(
                "  {} {}",
                bin.bold(),
                format!("v{}", latest_version).green(),
            );
        }
    }
    println!();

    let target = detect_target()?;
    let tarball_name = format!("caldir-{}.tar.gz", target);

    let download_url = latest
        .assets
        .iter()
        .find(|a| a.name == tarball_name)
        .map(|a| &a.browser_download_url)
        .ok_or_else(|| anyhow::anyhow!("No release found for platform: {}", target))?;

    let spinner = crate::utils::tui::create_spinner("Downloading...".to_string());

    let client = http_client()?;
    let response = client.get(download_url).send().await?;
    if !response.status().is_success() {
        bail!("Download failed (HTTP {})", response.status());
    }
    let bytes = response.bytes().await?;

    spinner.finish_and_clear();

    // Extract tarball to a temp directory
    let tmp_dir = tempfile::tempdir()?;
    let decoder = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(tmp_dir.path())?;

    // Replace each installed binary
    for bin in &installed {
        let src = tmp_dir.path().join(bin);
        let dst = install_dir.join(bin);

        if src.exists() {
            // Remove first to avoid ETXTBSY on Linux (can't write to a running executable,
            // but unlinking is fine — the kernel keeps the old inode mapped until the process exits)
            if dst.exists() {
                std::fs::remove_file(&dst).map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to update {} (permission denied?). Try:\n  sudo caldir update\n\nError: {}",
                        dst.display(),
                        e
                    )
                })?;
            }
            std::fs::copy(&src, &dst)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755))?;
            }
        }
    }

    println!(
        "{}",
        format!("Updated to v{}!", latest_version).green()
    );

    Ok(())
}

fn get_install_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let exe = exe.canonicalize()?;
    exe.parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| anyhow::anyhow!("Could not determine install directory"))
}

fn detect_target() -> Result<String> {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;

    let target_os = match os {
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-musl",
        _ => bail!("Unsupported OS: {}", os),
    };

    Ok(format!("{}-{}", arch, target_os))
}

fn http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(format!("caldir-cli/{}", CURRENT_VERSION))
        .build()?)
}

async fn fetch_latest_release() -> Result<GitHubRelease> {
    let client = http_client()?;
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        REPO
    );
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        bail!(
            "Failed to check for updates (HTTP {}). GitHub API may be rate-limited.",
            response.status()
        );
    }

    let body = response.bytes().await?;
    let release: GitHubRelease = serde_json::from_slice(&body)?;
    Ok(release)
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}
