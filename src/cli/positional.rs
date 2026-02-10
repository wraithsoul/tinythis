use std::ffi::OsString;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use crate::error::{Result, TinythisError};
use crate::presets::Preset;

pub fn run(preset: Preset, inputs: &[PathBuf], use_gpu: bool) -> Result<()> {
    let (bins, source) = match crate::assets::ffmpeg::resolve_ffmpeg()? {
        Some((bins, source)) => (bins, source),
        None => {
            let err = || {
                TinythisError::InvalidArgs(
                    "ffmpeg not available; run `tinythis setup` or place ffmpeg.exe next to tinythis.exe"
                        .to_string(),
                )
            };

            if !std::io::stdin().is_terminal() {
                return Err(err());
            }

            if !crate::confirm::confirm(
                "ffmpeg not available. run `tinythis setup` now?",
            )? {
                return Err(err());
            }

            super::cmd_setup::run(super::args::SetupArgs {
                force: false,
                yes: false,
            })?;

            crate::assets::ffmpeg::resolve_ffmpeg()?.ok_or_else(err)?
        }
    };
    if source == crate::assets::ffmpeg::FfmpegSource::NearExe {
        println!("local mode: using ffmpeg next to tinythis.exe");
    }

    for (i, input) in inputs.iter().enumerate() {
        let out_path = crate::exec::compress::build_output_path(input, preset)?;
        let mut args = crate::exec::compress::build_ffmpeg_args(input, &out_path, preset, use_gpu);
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
