use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use crate::error::{Result, TinythisError};
use crate::presets::Preset;

pub fn run(preset: Preset, inputs: &[PathBuf]) -> Result<()> {
    let (bins, source) = crate::assets::ffmpeg::resolve_ffmpeg()?.ok_or_else(|| {
        TinythisError::InvalidArgs(
            "ffmpeg not available; run `tinythis setup` or place ffmpeg.exe next to tinythis.exe"
                .to_string(),
        )
    })?;
    if source == crate::assets::ffmpeg::FfmpegSource::NearExe {
        println!("local mode: using ffmpeg next to tinythis.exe");
    }

    for (i, input) in inputs.iter().enumerate() {
        let out_path = crate::exec::compress::build_output_path(input, preset)?;
        let mut args = crate::exec::compress::build_ffmpeg_args(
            input,
            &out_path,
            preset,
            crate::prefs::gpu_enabled()?,
        );
        args.extend([OsString::from("-progress"), OsString::from("pipe:1")]);

        println!(
            "compressing ({}/{}) [{}] {} -> {}",
            i + 1,
            inputs.len(),
            preset.as_str(),
            input.display(),
            out_path.display()
        );

        let mut last: Option<u8> = None;
        crate::exec::compress::run_ffmpeg(&bins.ffmpeg, &args, move |pct| {
            if last == Some(pct) {
                return;
            }
            last = Some(pct);
            let _ = write!(std::io::stdout(), "\r{pct:3}%");
            let _ = std::io::stdout().flush();
            if pct == 100 {
                let _ = writeln!(std::io::stdout());
            }
        })?;
    }

    Ok(())
}
