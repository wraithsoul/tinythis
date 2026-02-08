use std::io::IsTerminal;

use crate::error::Result;

pub fn run(args: super::args::SetupArgs) -> Result<()> {
    let bins = crate::assets::ffmpeg::ensure_installed(args.force)?;
    println!("ffmpeg:  {}", bins.ffmpeg.display());

    let exe = crate::self_install::install_exe(args.force)?;
    println!("installed: {}", exe.installed_exe.display());

    if crate::self_install::user_path_contains(&exe.bin_dir)? {
        println!("path: already contains {}", exe.bin_dir.display());
        let _ = crate::prefs::set_path_opted_out(false);
        return Ok(());
    }

    let interactive = std::io::stdin().is_terminal();
    let opted_out = crate::prefs::path_opted_out()?;
    let should_add = if args.yes {
        true
    } else if opted_out {
        false
    } else if !interactive {
        true
    } else {
        crate::confirm::confirm("add tinythis to your PATH for quick use?")?
    };

    if should_add {
        let updated = crate::self_install::ensure_user_path_contains(&exe.bin_dir)?;
        let _ = crate::prefs::set_path_opted_out(false);
        if updated {
            println!("path: updated");
        } else {
            println!("path: already contains {}", exe.bin_dir.display());
        }
    } else {
        let _ = crate::prefs::set_path_opted_out(true);
        println!("path: skipped (run `tinythis setup path` to install later)");
    }
    Ok(())
}
