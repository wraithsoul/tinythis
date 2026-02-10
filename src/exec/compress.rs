use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{Result, TinythisError};
use crate::presets::Preset;

#[derive(Debug, Clone)]
pub struct SelectedFile {
    pub path: PathBuf,
    pub size_bytes: u64,
}

pub fn build_output_path(input: &Path, preset: Preset) -> Result<PathBuf> {
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    let stem = input.file_stem().ok_or_else(|| {
        TinythisError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "missing file stem",
        ))
    })?;

    let base = format!("{}.tinythis.{}", stem.to_string_lossy(), preset.as_str());
    let mut candidate = parent.join(format!("{base}.mp4"));
    if !candidate.exists() {
        return Ok(candidate);
    }

    for n in 2u32.. {
        candidate = parent.join(format!("{base}.{n}.mp4"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    unreachable!("the loop returns once it finds a free name")
}

pub fn build_ffmpeg_args(
    input: &Path,
    output: &Path,
    preset: Preset,
    use_gpu: bool,
) -> Vec<OsString> {
    let mut args = Vec::<OsString>::new();

    args.extend([
        OsString::from("-hide_banner"),
        OsString::from("-nostdin"),
        OsString::from("-nostats"),
        OsString::from("-y"),
        OsString::from("-i"),
        input.as_os_str().to_owned(),
        OsString::from("-map"),
        OsString::from("0:v:0"),
        OsString::from("-map"),
        OsString::from("0:a?"),
    ]);

    let codec = if use_gpu { "h264_nvenc" } else { "libx264" };
    args.extend(crate::presets::ffmpeg_video_args(preset, codec));

    args.extend([
        OsString::from("-pix_fmt"),
        OsString::from("yuv420p"),
        OsString::from("-movflags"),
        OsString::from("+faststart"),
    ]);

    args.extend([
        OsString::from("-c:a"),
        OsString::from("aac"),
        OsString::from("-b:a"),
        OsString::from(crate::presets::audio_bitrate(preset)),
    ]);

    args.push(output.as_os_str().to_owned());
    args
}

pub fn run_ffmpeg(
    ffmpeg: &Path,
    args: &[OsString],
    mut on_percent: impl FnMut(u8) + Send + 'static,
) -> Result<()> {
    let mut cmd = std::process::Command::new(ffmpeg);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| TinythisError::Io(std::io::Error::other("missing stdout")))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| TinythisError::Io(std::io::Error::other("missing stderr")))?;

    let total_us = Arc::new(AtomicU64::new(0));
    let stderr_tail = Arc::new(std::sync::Mutex::new(
        std::collections::VecDeque::<String>::new(),
    ));

    let total_us_stderr = Arc::clone(&total_us);
    let stderr_tail_stderr = Arc::clone(&stderr_tail);
    let stderr_thread = std::thread::spawn(move || {
        use std::io::BufRead;

        let reader = std::io::BufReader::new(stderr);
        for line in reader.lines().map_while(|r| r.ok()) {
            if total_us_stderr.load(Ordering::Relaxed) == 0
                && let Some(us) = parse_duration_us_from_stderr_line(&line)
            {
                total_us_stderr.store(us, Ordering::Relaxed);
            }

            let mut tail = stderr_tail_stderr.lock().unwrap();
            tail.push_back(line);
            while tail.len() > 30 {
                tail.pop_front();
            }
        }
    });

    let total_us_stdout = Arc::clone(&total_us);
    let stdout_thread = std::thread::spawn(move || {
        use std::io::BufRead;

        let reader = std::io::BufReader::new(stdout);
        let mut last_pct: Option<u8> = None;
        let mut seen_end = false;

        for line in reader.lines().map_while(|r| r.ok()) {
            let (key, val) = match line.split_once('=') {
                Some(kv) => kv,
                None => continue,
            };

            match key {
                "progress" => {
                    if val.trim() == "end" {
                        seen_end = true;
                        if last_pct != Some(100) {
                            on_percent(100);
                            last_pct = Some(100);
                        }
                    }
                }
                "out_time_us" => {
                    if let Ok(out_us) = val.trim().parse::<u64>()
                        && let Some(pct) = compute_percent(
                            out_us,
                            total_us_stdout.load(Ordering::Relaxed),
                            seen_end,
                        )
                        && last_pct != Some(pct)
                    {
                        on_percent(pct);
                        last_pct = Some(pct);
                    }
                }
                "out_time_ms" => {
                    if let Ok(out_us) = val.trim().parse::<u64>()
                        && let Some(pct) = compute_percent(
                            out_us,
                            total_us_stdout.load(Ordering::Relaxed),
                            seen_end,
                        )
                        && last_pct != Some(pct)
                    {
                        on_percent(pct);
                        last_pct = Some(pct);
                    }
                }
                _ => {}
            }
        }
    });

    let status = child.wait()?;
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    if status.success() {
        return Ok(());
    }

    let tail = stderr_tail.lock().unwrap();
    let stderr = tail.iter().cloned().collect::<Vec<_>>().join("\n");
    Err(TinythisError::ProcessFailed {
        program: ffmpeg.display().to_string(),
        code: status.code(),
        stderr,
    })
}

fn compute_percent(out_us: u64, total_us: u64, seen_end: bool) -> Option<u8> {
    if total_us == 0 {
        return None;
    }
    let raw = ((out_us as u128) * 100u128) / (total_us as u128);
    let mut pct = raw.min(100) as u8;
    if !seen_end && pct == 100 {
        pct = 99;
    }
    if !seen_end {
        pct = pct.min(99);
    }
    Some(pct)
}

fn parse_duration_us_from_stderr_line(line: &str) -> Option<u64> {
    // example: "  Duration: 00:00:08.05, start: 0.000000, bitrate: ..."
    let idx = line.find("Duration: ")?;
    let after = &line[idx + "Duration: ".len()..];
    let dur = after.split(',').next()?.trim();
    parse_hhmmss_to_us(dur)
}

fn parse_hhmmss_to_us(s: &str) -> Option<u64> {
    let mut parts = s.split(':');
    let h = parts.next()?.parse::<u64>().ok()?;
    let m = parts.next()?.parse::<u64>().ok()?;
    let sec_part = parts.next()?;

    let (sec_str, frac_str) = match sec_part.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (sec_part, None),
    };
    let sec = sec_str.parse::<u64>().ok()?;

    let mut us = (h * 3600 + m * 60 + sec) * 1_000_000;
    if let Some(frac) = frac_str {
        let mut frac_digits = frac.chars().take(6).collect::<String>();
        while frac_digits.len() < 6 {
            frac_digits.push('0');
        }
        if let Ok(f) = frac_digits.parse::<u64>() {
            us += f;
        }
    }

    Some(us)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_duration_us_from_stderr() {
        let line = "Duration: 00:00:08.05, start: 0.000000, bitrate: 123 kb/s";
        assert_eq!(parse_duration_us_from_stderr_line(line), Some(8_050_000));
    }

    #[test]
    fn percent_caps_at_99_until_end() {
        let total = 10_000_000u64;
        let out = 10_000_000u64;
        assert_eq!(compute_percent(out, total, false), Some(99));
        assert_eq!(compute_percent(out, total, true), Some(100));
    }
}
