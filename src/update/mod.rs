use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::error::{Result, TinythisError};

pub const DEFAULT_REPO: &str = "wraithsoul/tinythis";
pub const EXE_ASSET_NAME: &str = "tinythis-windows-x86_64.exe";
pub const SHA256_ASSET_NAME: &str = "tinythis-windows-x86_64.exe.sha256";

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let s = s.strip_prefix('v').unwrap_or(s);
        let mut it = s.split('.');
        let major = it.next()?.parse::<u32>().ok()?;
        let minor = it.next()?.parse::<u32>().ok()?;
        let patch = it.next()?.parse::<u32>().ok()?;
        if it.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current: Version,
    pub latest: Version,
    pub tag: String,
    pub exe_url: String,
    pub sha256_url: String,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

pub fn check_latest_release(repo: &str) -> Result<Option<UpdateInfo>> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    let current = Version::parse(env!("CARGO_PKG_VERSION"))
        .ok_or_else(|| TinythisError::InvalidArgs("invalid CARGO_PKG_VERSION".to_string()))?;

    let client = github_client()?;
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let release: GhRelease = client.get(url).send()?.error_for_status()?.json()?;

    let latest = Version::parse(&release.tag_name).ok_or_else(|| {
        TinythisError::InvalidArgs(format!("invalid release tag: {}", release.tag_name))
    })?;

    if latest <= current {
        return Ok(None);
    }

    let exe = release
        .assets
        .iter()
        .find(|a| a.name == EXE_ASSET_NAME)
        .map(|a| a.browser_download_url.as_str())
        .ok_or_else(|| {
            TinythisError::InvalidArgs(format!("missing release asset: {EXE_ASSET_NAME}"))
        })?;
    let sha = release
        .assets
        .iter()
        .find(|a| a.name == SHA256_ASSET_NAME)
        .map(|a| a.browser_download_url.as_str())
        .ok_or_else(|| {
            TinythisError::InvalidArgs(format!("missing release asset: {SHA256_ASSET_NAME}"))
        })?;

    Ok(Some(UpdateInfo {
        current,
        latest,
        tag: release.tag_name,
        exe_url: exe.to_string(),
        sha256_url: sha.to_string(),
    }))
}

fn github_client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent(concat!("tinythis/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(10))
        .build()?)
}

fn download_client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent(concat!("tinythis/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(300))
        .build()?)
}

pub fn apply_update(update: &UpdateInfo, relaunch: bool) -> Result<()> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    let install = crate::self_install::install(false)?;
    let bin_dir = install.bin_dir;
    let installed_exe = install.installed_exe;

    let client = download_client()?;

    let update_exe = bin_dir.join("tinythis-update.exe");
    download_to_file(&client, &update.exe_url, &update_exe, "downloading update")?;

    let expected = download_expected_sha256(&client, &update.sha256_url)?;
    verify_sha256(&update_exe, &expected)?;

    let helper_exe = bin_dir.join("tinythis-self-replace.exe");
    prepare_helper_exe(&helper_exe)?;

    let mut cmd = std::process::Command::new(&helper_exe);
    cmd.arg("self-replace")
        .arg("--pid")
        .arg(std::process::id().to_string())
        .arg("--src")
        .arg(&update_exe)
        .arg("--dst")
        .arg(&installed_exe);
    if relaunch {
        cmd.arg("--relaunch");
    }

    cmd.spawn()?;
    Ok(())
}

fn prepare_helper_exe(helper_exe: &Path) -> Result<()> {
    let current = std::env::current_exe()?;
    let _ = std::fs::remove_file(helper_exe);
    std::fs::copy(current, helper_exe)?;
    Ok(())
}

fn download_expected_sha256(client: &Client, url: &str) -> Result<[u8; 32]> {
    let text = client.get(url).send()?.error_for_status()?.text()?;
    let hex = extract_sha256_hex(&text)
        .ok_or_else(|| TinythisError::InvalidArgs("invalid sha256 asset contents".to_string()))?;
    parse_sha256_hex(&hex)
        .ok_or_else(|| TinythisError::InvalidArgs("invalid sha256 hex in sha256 asset".to_string()))
}

fn extract_sha256_hex(s: &str) -> Option<String> {
    let mut hex = String::new();
    for ch in s.chars() {
        if ch.is_ascii_hexdigit() {
            hex.push(ch.to_ascii_lowercase());
            if hex.len() == 64 {
                return Some(hex);
            }
        } else if !hex.is_empty() {
            break;
        }
    }
    None
}

fn parse_sha256_hex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    let bytes = s.as_bytes();
    for (i, outb) in out.iter_mut().enumerate() {
        let a = hex_val(bytes[2 * i])?;
        let b = hex_val(bytes[2 * i + 1])?;
        *outb = (a << 4) | b;
    }
    Some(out)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn verify_sha256(path: &Path, expected: &[u8; 32]) -> Result<()> {
    let mut f = std::fs::File::open(path)?;
    let mut buf = Vec::<u8>::new();
    f.read_to_end(&mut buf)?;

    let digest = ring::digest::digest(&ring::digest::SHA256, &buf);
    let got = digest.as_ref();
    if got == expected {
        return Ok(());
    }

    Err(TinythisError::InvalidArgs(format!(
        "sha256 mismatch for {} (expected {}, got {})",
        path.display(),
        to_hex(expected),
        to_hex(got),
    )))
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn download_to_file(client: &Client, url: &str, path: &Path, label: &str) -> Result<()> {
    let mut resp = client.get(url).send()?.error_for_status()?;
    let total = resp.content_length();

    let pb = ProgressBar::new(total.unwrap_or(0));
    pb.set_draw_target(ProgressDrawTarget::stderr_with_hz(8));

    if let Some(total) = total {
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} {msg} {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
            )
            .unwrap(),
        );
        pb.set_length(total);
    } else {
        pb.set_style(
            ProgressStyle::with_template("{spinner:.green} {msg} ({bytes} read)").unwrap(),
        );
    }
    pb.set_message(label.to_string());

    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;

    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = resp.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        pb.inc(n as u64);
    }
    file.flush()?;
    file.sync_all()?;
    pb.finish_and_clear();
    Ok(())
}

#[derive(Debug, Clone)]
pub struct SelfReplaceArgs {
    pub pid: u32,
    pub src: PathBuf,
    pub dst: PathBuf,
    pub relaunch: bool,
}

pub fn run_self_replace(args: SelfReplaceArgs) -> Result<()> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    let _ = args.pid;

    let dst_dir = args
        .dst
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    std::fs::create_dir_all(&dst_dir)?;

    let staged = dst_dir.join("tinythis.exe.new");
    let _ = std::fs::remove_file(&staged);
    std::fs::copy(&args.src, &staged)?;

    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        match replace_exe(&staged, &args.dst) {
            Ok(()) => break,
            Err(e) if should_retry_replace(&e) && std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(e) => return Err(e),
        }
    }

    let _ = std::fs::remove_file(&args.src);
    let _ = std::fs::remove_file(&staged);

    if args.relaunch {
        std::process::Command::new(&args.dst).spawn()?;
    }

    Ok(())
}

fn should_retry_replace(e: &TinythisError) -> bool {
    match e {
        TinythisError::Io(io) => matches!(
            io.kind(),
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::AlreadyExists
        ),
        _ => false,
    }
}

fn replace_exe(staged: &Path, dst: &Path) -> Result<()> {
    match std::fs::remove_file(dst) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }

    match std::fs::rename(staged, dst) {
        Ok(()) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        assert_eq!(
            Version::parse("0.1.2"),
            Some(Version {
                major: 0,
                minor: 1,
                patch: 2
            })
        );
        assert_eq!(
            Version::parse("v1.2.3"),
            Some(Version {
                major: 1,
                minor: 2,
                patch: 3
            })
        );
        assert_eq!(Version::parse("1.2"), None);
        assert_eq!(Version::parse("1.2.3.4"), None);
    }

    #[test]
    fn extracts_sha256_hex_from_common_formats() {
        let raw = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(extract_sha256_hex(raw), Some(raw.to_string()));
        assert_eq!(
            extract_sha256_hex(&format!("{raw}  tinythis-windows-x86_64.exe\n")),
            Some(raw.to_string())
        );
    }

    #[test]
    fn parses_github_latest_release_shape() {
        let json = r#"{
  "tag_name": "v0.2.0",
  "assets": [
    {
      "name": "tinythis-windows-x86_64.exe",
      "browser_download_url": "https://example.invalid/tinythis.exe"
    },
    {
      "name": "tinythis-windows-x86_64.exe.sha256",
      "browser_download_url": "https://example.invalid/tinythis.exe.sha256"
    }
  ]
}"#;
        let release: GhRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v0.2.0");
        assert_eq!(release.assets.len(), 2);
        assert_eq!(release.assets[0].name, "tinythis-windows-x86_64.exe");
    }
}
