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
    pub ffprobe: PathBuf,
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
    let ffprobe = crate::paths::ffprobe_exe_path()?;

    if !force && ffmpeg.is_file() && ffprobe.is_file() {
        return Ok(FfmpegBinaries { ffmpeg, ffprobe });
    }

    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("tinythis/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    let mut zip_tmp = NamedTempFile::new_in(&install_dir)?;
    download_zip(&client, FFMPEG_ZIP_URL, zip_tmp.as_file_mut())?;
    zip_tmp.as_file_mut().flush()?;
    zip_tmp.as_file_mut().sync_all()?;

    extract_executables(zip_tmp.path(), &install_dir, &ffmpeg, &ffprobe)?;

    let mut missing = Vec::new();
    if !ffmpeg.is_file() {
        missing.push(ffmpeg.clone());
    }
    if !ffprobe.is_file() {
        missing.push(ffprobe.clone());
    }
    if !missing.is_empty() {
        return Err(TinythisError::InstallIncomplete { missing });
    }

    verify_installed(&ffmpeg, &ffprobe)?;
    Ok(FfmpegBinaries { ffmpeg, ffprobe })
}

pub fn uninstall_assets() -> Result<()> {
    if !cfg!(windows) {
        return Err(TinythisError::UnsupportedPlatform(std::env::consts::OS));
    }

    let install_dir = crate::paths::ffmpeg_dir()?;
    let lock_path = install_dir.join(".install.lock");
    let ffmpeg = crate::paths::ffmpeg_exe_path()?;
    let ffprobe = crate::paths::ffprobe_exe_path()?;

    remove_file_if_exists(&ffmpeg)?;
    remove_file_if_exists(&ffprobe)?;
    remove_file_if_exists(&lock_path)?;

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

fn verify_installed(ffmpeg: &Path, ffprobe: &Path) -> Result<()> {
    let args = [OsString::from("-version")];
    crate::process::run::run_capture(ffmpeg, &args)?;
    crate::process::run::run_capture(ffprobe, &args)?;
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

fn extract_executables(
    zip_path: &Path,
    install_dir: &Path,
    ffmpeg_dest: &Path,
    ffprobe_dest: &Path,
) -> Result<()> {
    let zip_file = File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(zip_file)?;

    let mut ffmpeg_found = false;
    let mut ffprobe_found = false;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if !entry.is_file() {
            continue;
        }

        let name = entry.name().replace('\\', "/");
        if ends_with_path_ci(&name, "bin/ffmpeg.exe") {
            write_zip_entry_to_path(&mut entry, install_dir, ffmpeg_dest)?;
            ffmpeg_found = true;
        } else if ends_with_path_ci(&name, "bin/ffprobe.exe") {
            write_zip_entry_to_path(&mut entry, install_dir, ffprobe_dest)?;
            ffprobe_found = true;
        }

        if ffmpeg_found && ffprobe_found {
            break;
        }
    }

    if !ffmpeg_found {
        return Err(TinythisError::AssetEntryMissing { name: "ffmpeg.exe" });
    }
    if !ffprobe_found {
        return Err(TinythisError::AssetEntryMissing {
            name: "ffprobe.exe",
        });
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
