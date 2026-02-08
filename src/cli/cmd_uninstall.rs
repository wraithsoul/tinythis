use crate::error::Result;

pub fn run(args: super::args::UninstallArgs) -> Result<()> {
    let _ = args;

    let app_root_dir = crate::paths::app_root_dir()?;
    let bin_dir = crate::paths::tinythis_bin_dir()?;
    let current_exe = std::env::current_exe().ok();

    let out = crate::self_install::uninstall()?;
    if out.path_was_updated {
        println!("path: updated");
    } else {
        println!("path: no change");
    }

    crate::assets::ffmpeg::uninstall_assets()?;
    println!("assets: removed");

    let _ = crate::prefs::set_path_opted_out(false);

    if let Some(current_exe) = current_exe
        && path_is_within_dir(&bin_dir, &current_exe)
    {
        spawn_self_remove(&bin_dir, &app_root_dir)?;
        println!("bin: removing...");
        std::process::exit(0);
    }

    crate::self_install::remove_bin_dir(&bin_dir)?;
    println!("bin: removed");
    let _ = crate::self_install::remove_app_root_if_empty(&app_root_dir);
    Ok(())
}

fn spawn_self_remove(bin_dir: &std::path::Path, app_root_dir: &std::path::Path) -> Result<()> {
    use std::io::Write;

    let src = std::env::current_exe()?;
    let temp_dir = std::env::temp_dir();
    let mut tmp = tempfile::NamedTempFile::new_in(&temp_dir)?;
    {
        let mut input = std::fs::File::open(&src)?;
        std::io::copy(&mut input, tmp.as_file_mut())?;
        tmp.as_file_mut().flush()?;
        tmp.as_file_mut().sync_all()?;
    }

    let helper = temp_dir.join(format!("tinythis-self-remove-{}.exe", std::process::id()));
    persist_overwrite(tmp, &helper)?;

    let mut cmd = std::process::Command::new(&helper);
    cmd.arg("self-remove")
        .arg("--pid")
        .arg(std::process::id().to_string())
        .arg("--bin-dir")
        .arg(bin_dir)
        .arg("--app-root-dir")
        .arg(app_root_dir);
    cmd.spawn()?;
    Ok(())
}

fn persist_overwrite(tmp: tempfile::NamedTempFile, dest: &std::path::Path) -> Result<()> {
    match tmp.into_temp_path().persist(dest) {
        Ok(_) => Ok(()),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(dest)?;
            e.path.persist(dest).map(|_| ()).map_err(|e| e.error.into())
        }
        Err(e) => Err(e.error.into()),
    }
}

fn path_is_within_dir(dir: &std::path::Path, path: &std::path::Path) -> bool {
    let mut d = dir.to_string_lossy().replace('/', "\\");
    if !d.ends_with('\\') {
        d.push('\\');
    }
    let p = path.to_string_lossy().replace('/', "\\");
    p.to_ascii_lowercase().starts_with(&d.to_ascii_lowercase())
}
