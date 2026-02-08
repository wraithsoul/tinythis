use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use tempfile::NamedTempFile;

use crate::error::{Result, TinythisError};

const FFMPEG_ZIP_URL: &str = "https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-gpl.zip";

#[derive(Debug, Clone)]
pub struct FfmpegBinaries {
    pub ffmpeg: PathBuf,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FfmpegSource {
    NearExe,
    Bundled,
}

pub fn find_installed() -> Result<Option<FfmpegBinaries>> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    let ffmpeg = crate::paths::ffmpeg_exe_path()?;
    if ffmpeg.is_file() {
        return Ok(Some(FfmpegBinaries { ffmpeg }));
    }
    Ok(None)
}

pub fn find_near_exe() -> Result<Option<FfmpegBinaries>> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    let exe = std::env::current_exe()?;
    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    Ok(find_near_dir(dir))
}

pub fn resolve_ffmpeg() -> Result<Option<(FfmpegBinaries, FfmpegSource)>> {
    if let Some(bins) = find_near_exe()? {
        return Ok(Some((bins, FfmpegSource::NearExe)));
    }
    if let Some(bins) = find_installed()? {
        return Ok(Some((bins, FfmpegSource::Bundled)));
    }
    Ok(None)
}

fn find_near_dir(dir: &Path) -> Option<FfmpegBinaries> {
    let ffmpeg = dir.join("ffmpeg.exe");
    if ffmpeg.is_file() {
        return Some(FfmpegBinaries { ffmpeg });
    }
    None
}

pub fn ensure_installed(force: bool) -> Result<FfmpegBinaries> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    let install_dir = crate::paths::ffmpeg_dir()?;
    std::fs::create_dir_all(&install_dir)?;

    let lock_path = install_dir.join(".install.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    lock_file.lock_exclusive()?;

    let ffmpeg = crate::paths::ffmpeg_exe_path()?;

    if !force && ffmpeg.is_file() {
        return Ok(FfmpegBinaries { ffmpeg });
    }

    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("tinythis/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    let mut zip_tmp = NamedTempFile::new_in(&install_dir)?;
    download_zip(&client, FFMPEG_ZIP_URL, zip_tmp.as_file_mut())?;
    zip_tmp.as_file_mut().flush()?;
    zip_tmp.as_file_mut().sync_all()?;

    extract_executables(zip_tmp.path(), &install_dir, &ffmpeg)?;

    let mut missing = Vec::new();
    if !ffmpeg.is_file() {
        missing.push(ffmpeg.clone());
    }
    if !missing.is_empty() {
        return Err(TinythisError::InstallIncomplete { missing });
    }

    verify_installed(&ffmpeg)?;
    Ok(FfmpegBinaries { ffmpeg })
}

pub fn uninstall_assets() -> Result<()> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    let install_dir = crate::paths::ffmpeg_dir()?;
    let lock_path = install_dir.join(".install.lock");
    let ffmpeg = crate::paths::ffmpeg_exe_path()?;

    {
        let lock_file = match OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
        {
            Ok(f) => Some(f),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        if let Some(lock_file) = lock_file.as_ref() {
            lock_file.lock_exclusive()?;
        }

        remove_file_if_exists(&ffmpeg)?;
    }

    match std::fs::remove_file(&lock_path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {}
        Err(e) => return Err(e.into()),
    }

    match std::fs::remove_dir(&install_dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) if e.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
        Err(e) => return Err(e.into()),
    }

    Ok(())
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn verify_installed(ffmpeg: &Path) -> Result<()> {
    let args = [OsString::from("-version")];
    crate::process::run::run_capture(ffmpeg, &args)?;
    Ok(())
}

fn download_zip(client: &reqwest::blocking::Client, url: &str, out: &mut File) -> Result<()> {
    let mut resp = client.get(url).send()?.error_for_status()?;

    let total = resp.content_length();
    let pb = match total {
        Some(len) => {
            let pb = ProgressBar::new(len);
            pb.set_draw_target(ProgressDrawTarget::stderr());
            pb.set_style(
                ProgressStyle::with_template(
                    "{spinner:.green} downloading ffmpeg {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
                )
                .unwrap(),
            );
            pb
        }
        None => {
            let pb = ProgressBar::new_spinner();
            pb.set_draw_target(ProgressDrawTarget::stderr());
            pb.set_style(
                ProgressStyle::with_template("{spinner:.green} downloading ffmpeg ({bytes} read)")
                    .unwrap(),
            );
            pb.enable_steady_tick(std::time::Duration::from_millis(120));
            pb
        }
    };

    let mut buf = [0u8; 64 * 1024];
    loop {
        let read = resp.read(&mut buf)?;
        if read == 0 {
            break;
        }
        out.write_all(&buf[..read])?;
        pb.inc(read as u64);
    }
    pb.finish_and_clear();
    Ok(())
}

fn extract_executables(zip_path: &Path, install_dir: &Path, ffmpeg_dest: &Path) -> Result<()> {
    let zip_file = File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(zip_file)?;

    let mut ffmpeg_found = false;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if !entry.is_file() {
            continue;
        }

        let name = entry.name().replace('\\', "/");
        if ends_with_path_ci(&name, "bin/ffmpeg.exe") {
            write_zip_entry_to_path(&mut entry, install_dir, ffmpeg_dest)?;
            ffmpeg_found = true;
        }

        if ffmpeg_found {
            break;
        }
    }

    if !ffmpeg_found {
        return Err(TinythisError::AssetEntryMissing { name: "ffmpeg.exe" });
    }

    Ok(())
}

fn ends_with_path_ci(path: &str, suffix: &str) -> bool {
    path.len() >= suffix.len() && path[path.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

fn write_zip_entry_to_path<R: Read>(mut entry: R, install_dir: &Path, dest: &Path) -> Result<()> {
    let mut tmp = NamedTempFile::new_in(install_dir)?;
    std::io::copy(&mut entry, tmp.as_file_mut())?;
    tmp.as_file_mut().flush()?;
    tmp.as_file_mut().sync_all()?;

    persist_overwrite(tmp, dest)?;
    Ok(())
}

fn persist_overwrite(tmp: NamedTempFile, dest: &Path) -> Result<()> {
    match tmp.into_temp_path().persist(dest) {
        Ok(_) => Ok(()),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(dest)?;
            e.path
                .persist(dest)
                .map(|_| ())
                .map_err(|e| TinythisError::Io(e.error))
        }
        Err(e) => Err(e.error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_near_dir_requires_ffmpeg() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_near_dir(dir.path()).is_none());

        std::fs::write(dir.path().join("ffmpeg.exe"), b"x").unwrap();
        let bins = find_near_dir(dir.path()).unwrap();
        assert!(bins.ffmpeg.ends_with("ffmpeg.exe"));
    }
}
