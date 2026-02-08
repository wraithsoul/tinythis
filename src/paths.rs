use std::path::PathBuf;

use directories::BaseDirs;

use crate::error::{Result, TinythisError};

pub fn local_appdata_dir() -> Result<PathBuf> {
    if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(local_appdata));
    }

    let base = BaseDirs::new().ok_or(TinythisError::MissingEnv("LOCALAPPDATA"))?;
    Ok(base.data_local_dir().to_path_buf())
}

pub fn app_root_dir() -> Result<PathBuf> {
    Ok(local_appdata_dir()?.join("tinythis"))
}

pub fn ffmpeg_dir() -> Result<PathBuf> {
    Ok(app_root_dir()?.join("ffmpeg"))
}

pub fn ffmpeg_exe_path() -> Result<PathBuf> {
    Ok(ffmpeg_dir()?.join("ffmpeg.exe"))
}

pub fn tinythis_bin_dir() -> Result<PathBuf> {
    Ok(app_root_dir()?.join("bin"))
}

pub fn tinythis_installed_exe_path() -> Result<PathBuf> {
    Ok(tinythis_bin_dir()?.join("tinythis.exe"))
}
