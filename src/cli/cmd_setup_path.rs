use crate::error::Result;

pub fn run(_args: super::args::SetupPathArgs) -> Result<()> {
    let exe = crate::self_install::install_exe(false)?;
    let updated = crate::self_install::ensure_user_path_contains(&exe.bin_dir)?;
    let _ = crate::prefs::set_path_opted_out(false);

    println!("installed: {}", exe.installed_exe.display());
    if updated {
        println!("path: updated");
    } else {
        println!("path: already contains {}", exe.bin_dir.display());
    }
    Ok(())
}
