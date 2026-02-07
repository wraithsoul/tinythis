use crate::error::Result;

pub fn run(args: super::args::UninstallArgs) -> Result<()> {
    let _ = args;
    let out = crate::self_install::uninstall()?;
    if out.path_was_updated {
        println!("path: updated");
    } else {
        println!("path: no change");
    }

    crate::assets::ffmpeg::uninstall_assets()?;
    println!("assets: removed");
    Ok(())
}
