use anyhow::{Context, Result};
use reqwest::blocking::Client;
use semver::Version;
use serde::Deserialize;
use std::time::Duration;

const DEFAULT_LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/KusStar/kpdf/releases/latest";
const HTTP_ACCEPT_HEADER: &str = "application/vnd.github+json";
const HTTP_USER_AGENT: &str = concat!("kPDF-Updater/", env!("CARGO_PKG_VERSION"));
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub latest_version: String,
    pub download_url: String,
}

#[derive(Debug, Clone)]
pub enum UpdateCheck {
    UpToDate { latest_version: String },
    UpdateAvailable(UpdateInfo),
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub fn check_for_updates(current_version: &str) -> Result<UpdateCheck> {
    let release = fetch_latest_release()?;
    let latest_version = normalize_version_label(&release.tag_name);
    let latest_semver = parse_semver(&latest_version)?;
    let current_semver = parse_semver(&normalize_version_label(current_version))?;

    if latest_semver <= current_semver {
        return Ok(UpdateCheck::UpToDate { latest_version });
    }

    let download_url = select_asset_for_current_platform(&release.assets)
        .map(|asset| asset.browser_download_url.clone())
        .unwrap_or_else(|| release.html_url.clone());

    Ok(UpdateCheck::UpdateAvailable(UpdateInfo {
        latest_version,
        download_url,
    }))
}

fn fetch_latest_release() -> Result<GithubRelease> {
    let client = Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .context("failed to create update http client")?;

    let endpoint = std::env::var("KPDF_UPDATER_LATEST_URL")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_LATEST_RELEASE_API_URL.to_string());

    client
        .get(&endpoint)
        .header(reqwest::header::ACCEPT, HTTP_ACCEPT_HEADER)
        .header(reqwest::header::USER_AGENT, HTTP_USER_AGENT)
        .send()
        .context("failed to request latest release")?
        .error_for_status()
        .context("latest release request failed")?
        .json::<GithubRelease>()
        .context("failed to parse latest release response")
}

fn parse_semver(raw: &str) -> Result<Version> {
    Version::parse(raw).with_context(|| format!("invalid version: {raw}"))
}

fn normalize_version_label(raw: &str) -> String {
    let trimmed = raw.trim();
    let no_ref = trimmed.strip_prefix("refs/tags/").unwrap_or(trimmed).trim();
    no_ref
        .trim_start_matches(|ch| ch == 'v' || ch == 'V')
        .to_string()
}

fn select_asset_for_current_platform(assets: &[GithubAsset]) -> Option<&GithubAsset> {
    #[cfg(target_os = "macos")]
    {
        return select_macos_asset(assets);
    }

    #[cfg(target_os = "windows")]
    {
        return select_windows_asset(assets);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return select_linux_asset(assets);
    }

    #[allow(unreachable_code)]
    None
}

#[cfg(any(test, target_os = "macos"))]
fn select_macos_asset(assets: &[GithubAsset]) -> Option<&GithubAsset> {
    assets
        .iter()
        .find(|asset| is_macos_dmg(&asset.name))
        .or_else(|| assets.iter().find(|asset| is_macos_app_zip(&asset.name)))
}

#[cfg(any(test, target_os = "windows"))]
fn select_windows_asset(assets: &[GithubAsset]) -> Option<&GithubAsset> {
    assets
        .iter()
        .find(|asset| is_windows_installer(&asset.name))
}

#[cfg(any(test, all(unix, not(target_os = "macos"))))]
fn select_linux_asset(assets: &[GithubAsset]) -> Option<&GithubAsset> {
    assets.iter().find(|asset| is_linux_installer(&asset.name))
}

#[cfg(any(test, target_os = "macos"))]
fn is_macos_dmg(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    (name.starts_with("macos-") || name.contains("macos")) && name.ends_with(".dmg")
}

#[cfg(any(test, target_os = "macos"))]
fn is_macos_app_zip(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    (name.starts_with("macos-") || name.contains("macos")) && name.ends_with(".app.zip")
}

#[cfg(any(test, target_os = "windows"))]
fn is_windows_installer(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    (name.starts_with("windows-") || name.contains("windows")) && name.ends_with(".exe")
}

#[cfg(any(test, all(unix, not(target_os = "macos"))))]
fn is_linux_installer(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(".appimage")
        || name.ends_with(".deb")
        || name.ends_with(".rpm")
        || name.ends_with(".tar.gz")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_version() {
        assert_eq!(normalize_version_label("v0.3.0"), "0.3.0");
        assert_eq!(normalize_version_label("V1.2.3"), "1.2.3");
        assert_eq!(normalize_version_label("refs/tags/v2.0.1"), "2.0.1");
        assert_eq!(normalize_version_label("0.4.0"), "0.4.0");
    }

    #[test]
    fn select_assets_by_platform_patterns() {
        let assets = vec![
            GithubAsset {
                name: "macos-kPDF_0.3.1_aarch64.dmg".into(),
                browser_download_url: "https://example.com/macos.dmg".into(),
            },
            GithubAsset {
                name: "windows-kpdf_0.3.1_x64-setup.exe".into(),
                browser_download_url: "https://example.com/windows.exe".into(),
            },
            GithubAsset {
                name: "kpdf_0.3.1_amd64.deb".into(),
                browser_download_url: "https://example.com/linux.deb".into(),
            },
        ];

        assert!(select_macos_asset(&assets).is_some());
        assert!(select_windows_asset(&assets).is_some());
        assert!(select_linux_asset(&assets).is_some());
    }
}
