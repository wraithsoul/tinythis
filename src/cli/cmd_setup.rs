use crate::error::Result;

pub fn run(args: super::args::SetupArgs) -> Result<()> {
    let bins = crate::assets::ffmpeg::ensure_installed(args.force)?;
    println!("ffmpeg:  {}", bins.ffmpeg.display());
    println!("ffprobe: {}", bins.ffprobe.display());

    let out = crate::self_install::install(args.force)?;
    println!("installed: {}", out.installed_exe.display());
    if out.path_was_updated {
        println!("path: updated");
    } else {
        println!("path: already contains {}", out.bin_dir.display());
    }
    Ok(())
}
