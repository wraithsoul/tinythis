use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::Result;

fn path_optout_file() -> Result<PathBuf> {
    Ok(crate::paths::app_root_dir()?.join("path.optout"))
}

pub fn path_opted_out() -> Result<bool> {
    let p = path_optout_file()?;
    match std::fs::metadata(p) {
        Ok(m) => Ok(m.is_file()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub fn set_path_opted_out(opt_out: bool) -> Result<()> {
    let p = path_optout_file()?;
    let dir = p.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;

    if opt_out {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&p)?;
        f.write_all(b"user declined PATH prompt\n")?;
        f.flush()?;
        f.sync_all()?;
        return Ok(());
    }

    match std::fs::remove_file(&p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
