use std::io::Write;
use std::path::PathBuf;

use crate::error::Result;

#[derive(Debug, Copy, Clone)]
struct Options {
    gpu: bool,
    path_optout: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            gpu: false,
            path_optout: false,
        }
    }
}

fn options_file() -> Result<PathBuf> {
    Ok(crate::paths::app_root_dir()?.join("options.toml"))
}

fn legacy_path_optout_file() -> Result<PathBuf> {
    Ok(crate::paths::app_root_dir()?.join("path.optout"))
}

fn parse_options(content: &str) -> Options {
    let mut out = Options::default();

    for raw in content.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let Some((key_raw, val_raw)) = line.split_once('=') else {
            continue;
        };

        let key = key_raw.trim().trim_matches('"');
        let val = val_raw.trim();
        let parsed = match val {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        };
        let Some(v) = parsed else {
            continue;
        };

        match key {
            "gpu" => out.gpu = v,
            "path.optout" => out.path_optout = v,
            _ => {}
        }
    }

    out
}

fn read_options() -> Result<Options> {
    let path = options_file()?;
    let mut options = match std::fs::read_to_string(&path) {
        Ok(content) => parse_options(&content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Options::default(),
        Err(e) => return Err(e.into()),
    };

    let legacy = legacy_path_optout_file()?;
    if std::fs::metadata(&legacy)
        .map(|m| m.is_file())
        .unwrap_or(false)
    {
        options.path_optout = true;
        write_options(&options)?;
        let _ = std::fs::remove_file(&legacy);
    }

    Ok(options)
}

fn write_options(options: &Options) -> Result<()> {
    let path = options_file()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }

    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)?;

    writeln!(f, "gpu={}", options.gpu)?;
    writeln!(f, "path.optout={}", options.path_optout)?;
    f.flush()?;
    f.sync_all()?;
    Ok(())
}

pub fn gpu_enabled() -> Result<bool> {
    Ok(read_options()?.gpu)
}

pub fn set_gpu_enabled(enabled: bool) -> Result<()> {
    let mut options = read_options()?;
    options.gpu = enabled;
    write_options(&options)
}

pub fn path_opted_out() -> Result<bool> {
    Ok(read_options()?.path_optout)
}

pub fn set_path_opted_out(opt_out: bool) -> Result<()> {
    let mut options = read_options()?;
    options.path_optout = opt_out;
    write_options(&options)
}
