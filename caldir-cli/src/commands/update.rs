use anyhow::{Result, bail};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::path::{Path, PathBuf};

const REPO: &str = "t4t5/caldir";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn run() -> Result<()> {
    let install_dir = get_install_dir()?;

    remove_stale_backups(&install_dir);

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

    let archive_name = archive_name()?;

    let download_url = latest
        .assets
        .iter()
        .find(|a| a.name == archive_name)
        .map(|a| &a.browser_download_url)
        .ok_or_else(|| anyhow::anyhow!("No release found for platform: {}", archive_name))?;

    let spinner = crate::utils::tui::create_spinner("Downloading...".to_string());

    let client = http_client()?;
    let response = client.get(download_url).send().await?;
    if !response.status().is_success() {
        bail!("Download failed (HTTP {})", response.status());
    }
    let bytes = response.bytes().await?;

    spinner.finish_and_clear();

    let tmp_dir = tempfile::tempdir()?;
    extract_archive(&archive_name, &bytes, tmp_dir.path())?;

    // Discover binaries from the archive — the release is the source of truth
    // for what ships. Only update binaries that are also installed locally,
    // so users keep whichever providers they originally installed.
    let mut to_update: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(tmp_dir.path())? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(|s| s.to_string()) else {
            continue;
        };
        if install_dir.join(&name).exists() {
            to_update.push(name);
        }
    }
    to_update.sort();

    println!(
        "  {} {} → {}",
        "caldir".bold(),
        format!("v{}", CURRENT_VERSION).dimmed(),
        format!("v{}", latest_version).green(),
    );
    for bin in &to_update {
        if bin != "caldir" && bin != "caldir.exe" {
            println!(
                "  {} {}",
                bin.bold(),
                format!("v{}", latest_version).green(),
            );
        }
    }
    println!();

    for bin in &to_update {
        replace_binary(&tmp_dir.path().join(bin), &install_dir.join(bin))?;
    }

    println!("{}", format!("Updated to v{}!", latest_version).green());

    Ok(())
}

fn get_install_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let exe = exe.canonicalize()?;
    exe.parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| anyhow::anyhow!("Could not determine install directory"))
}

fn archive_name() -> Result<String> {
    let (os, ext) = match std::env::consts::OS {
        "macos" => ("apple-darwin", "tar.gz"),
        "linux" => ("unknown-linux-musl", "tar.gz"),
        "windows" => ("pc-windows-msvc", "zip"),
        os => bail!("Unsupported OS: {}", os),
    };
    Ok(format!("caldir-{}-{}.{}", std::env::consts::ARCH, os, ext))
}

fn extract_archive(name: &str, bytes: &[u8], dest: &Path) -> Result<()> {
    if name.ends_with(".zip") {
        zip::ZipArchive::new(std::io::Cursor::new(bytes))?.extract(dest)?;
    } else {
        let decoder = flate2::read::GzDecoder::new(bytes);
        tar::Archive::new(decoder).unpack(dest)?;
    }
    Ok(())
}

fn replace_binary(src: &Path, dst: &Path) -> Result<()> {
    // Unlink first: Linux can't overwrite a running executable (ETXTBSY) but can
    // unlink it; Windows can't delete one but can rename it aside.
    if let Err(e) = std::fs::remove_file(dst) {
        let mut backup = dst.as_os_str().to_owned();
        backup.push(".old");
        if !(cfg!(windows) && std::fs::rename(dst, &backup).is_ok()) {
            let hint = if cfg!(windows) {
                ""
            } else {
                " Try:\n  sudo caldir update\n"
            };
            bail!(
                "Failed to update {} (permission denied?).{}\nError: {}",
                dst.display(),
                hint,
                e
            );
        }
    }
    std::fs::copy(src, dst)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dst, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

// Delete `caldir*.old` backups left by a previous Windows update.
fn remove_stale_backups(install_dir: &Path) {
    if !cfg!(windows) {
        return;
    }
    let Ok(entries) = std::fs::read_dir(install_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("caldir") && name.ends_with(".old") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(format!("caldir-cli/{}", CURRENT_VERSION))
        .build()?)
}

async fn fetch_latest_release() -> Result<GitHubRelease> {
    let client = http_client()?;
    let url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
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
