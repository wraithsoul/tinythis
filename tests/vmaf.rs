#![cfg(windows)]

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Preset {
    Quality,
    Balanced,
    Speed,
}

impl Preset {
    fn as_str(self) -> &'static str {
        match self {
            Preset::Quality => "quality",
            Preset::Balanced => "balanced",
            Preset::Speed => "speed",
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Encoder {
    Cpu,
    Gpu,
}

impl Encoder {
    fn as_str(self) -> &'static str {
        match self {
            Encoder::Cpu => "cpu",
            Encoder::Gpu => "gpu",
        }
    }

    fn flag(self) -> &'static str {
        match self {
            Encoder::Cpu => "--cpu",
            Encoder::Gpu => "--gpu",
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct Case {
    preset: Preset,
    encoder: Encoder,
}

impl Case {
    fn name(self) -> String {
        format!("{}_{}", self.encoder.as_str(), self.preset.as_str())
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tests_dir() -> PathBuf {
    repo_root().join("tests")
}

fn local_appdata_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

fn try_find_ffmpeg_exe() -> Option<PathBuf> {
    let tests = tests_dir();
    let tests_ffmpeg = tests.join("ffmpeg.exe");
    if tests_ffmpeg.is_file() {
        return Some(tests_ffmpeg);
    }

    if let Some(p) = std::env::var_os("TINYTHIS_TEST_FFMPEG") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }

    if let Some(local) = local_appdata_dir() {
        let installed = local.join("tinythis").join("ffmpeg").join("ffmpeg.exe");
        if installed.is_file() {
            return Some(installed);
        }
    }

    None
}

fn try_find_ffprobe_exe() -> Option<PathBuf> {
    let tests = tests_dir();
    let tests_ffprobe = tests.join("ffprobe.exe");
    if tests_ffprobe.is_file() {
        return Some(tests_ffprobe);
    }

    if let Some(p) = std::env::var_os("TINYTHIS_TEST_FFPROBE") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }

    if let Some(local) = local_appdata_dir() {
        let installed = local.join("tinythis").join("ffmpeg").join("ffprobe.exe");
        if installed.is_file() {
            return Some(installed);
        }
    }

    None
}

fn is_supported_input_ext(ext: &OsStr) -> bool {
    matches!(
        ext.to_string_lossy().to_ascii_lowercase().as_str(),
        "mp4" | "mov" | "m4v" | "mkv" | "webm" | "avi"
    )
}

fn try_find_input_video_file() -> Option<PathBuf> {
    let dir = tests_dir();

    let preferred = ["input.mp4", "dummy.mp4", "test.mp4"];
    for name in preferred {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }

    let mut candidates: Vec<(u64, PathBuf)> = Vec::new();
    for e in fs::read_dir(&dir).expect("read tests/") {
        let e = e.expect("read dir entry");
        let path = e.path();
        if !path.is_file() {
            continue;
        }
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if file_name.eq_ignore_ascii_case("ffmpeg.exe")
            || file_name.eq_ignore_ascii_case("ffprobe.exe")
            || file_name.eq_ignore_ascii_case("ffplay.exe")
            || file_name.eq_ignore_ascii_case("tinythis.exe")
        {
            continue;
        }
        if file_name.contains(".tinythis.") {
            continue;
        }
        let ext = match path.extension() {
            Some(e) => e,
            None => continue,
        };
        if !is_supported_input_ext(ext) {
            continue;
        }
        let size = e.metadata().expect("metadata").len();
        candidates.push((size, path));
    }

    candidates.sort_by_key(|(size, _)| *size);
    candidates.into_iter().next().map(|(_, p)| p)
}

fn hardlink_or_copy(src: &Path, dst: &Path) {
    match fs::hard_link(src, dst) {
        Ok(()) => {}
        Err(_) => {
            fs::copy(src, dst).unwrap_or_else(|e| {
                panic!(
                    "copy file failed: {} -> {}: {e}",
                    src.display(),
                    dst.display()
                )
            });
        }
    }
}

fn read_vmaf_mean(json_bytes: &[u8]) -> f64 {
    let v: serde_json::Value =
        serde_json::from_slice(json_bytes).expect("vmaf json should be valid json");
    v.get("pooled_metrics")
        .and_then(|v| v.get("vmaf"))
        .and_then(|v| v.get("mean"))
        .and_then(|v| v.as_f64())
        .expect("vmaf json should contain pooled_metrics.vmaf.mean as number")
}

fn run_tinythis_case(tinythis: &Path, case_dir: &Path, case: Case, input_name: &str) -> PathBuf {
    let out = Command::new(tinythis)
        .current_dir(case_dir)
        .args([case.encoder.flag(), case.preset.as_str(), input_name])
        .output()
        .expect("run tinythis");

    if !out.status.success() {
        panic!(
            "tinythis failed for {} (status={:?}).\nstdout:\n{}\nstderr:\n{}",
            case.name(),
            out.status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let stem = Path::new(input_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("input has file stem");
    let expected = case_dir.join(format!("{stem}.tinythis.{}.mp4", case.preset.as_str()));
    if expected.is_file() {
        return expected;
    }

    let listing = fs::read_dir(case_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect::<Vec<_>>()
        .join(", ");

    panic!(
        "tinythis did not produce expected output for {}. expected={}, dir=[{}]",
        case.name(),
        expected.display(),
        listing
    );
}

fn probe_r_frame_rate(ffprobe: &Path, dir: &Path, input: &Path) -> String {
    let out = Command::new(ffprobe)
        .current_dir(dir)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=r_frame_rate",
            "-of",
            "json",
            input.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run ffprobe");

    if !out.status.success() {
        panic!(
            "ffprobe failed (status={:?}). stderr:\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("ffprobe json should be valid");
    v.get("streams")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|s| s.get("r_frame_rate"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| panic!("ffprobe json missing streams[0].r_frame_rate"))
}

fn run_vmaf(ffmpeg: &Path, case_dir: &Path, fps: &str, reference: &Path, distorted: &Path) -> f64 {
    let json = case_dir.join("vmaf.json");
    let filter_complex = format!(
        "[0:v]setpts=PTS-STARTPTS,fps={fps},format=yuv420p[ref];\
[1:v]setpts=PTS-STARTPTS,fps={fps},format=yuv420p[dist];\
[dist][ref]scale2ref[dist2][ref2];\
[dist2][ref2]libvmaf=n_subsample=5:log_fmt=json:log_path=vmaf.json"
    );

    let out = Command::new(ffmpeg)
        .current_dir(case_dir)
        .args([
            "-hide_banner",
            "-nostats",
            "-loglevel",
            "error",
            "-i",
            reference.to_string_lossy().as_ref(),
            "-i",
            distorted.to_string_lossy().as_ref(),
            "-filter_complex",
            &filter_complex,
            "-f",
            "null",
            "-",
        ])
        .output()
        .expect("run ffmpeg libvmaf");

    if !out.status.success() {
        panic!(
            "ffmpeg libvmaf failed (status={:?}). stderr:\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let bytes = fs::read(&json).expect("read vmaf.json");
    let mean = read_vmaf_mean(&bytes);
    let _ = fs::remove_file(&json);
    mean
}

#[test]
#[ignore = "requires ffmpeg/ffprobe and an input video; run with `cargo test --test vmaf -- --ignored --show-output`"]
fn vmaf_cpu_gpu_quality_balanced_speed() {
    let input_src = if let Some(p) = std::env::var_os("TINYTHIS_TEST_INPUT") {
        let p = PathBuf::from(p);
        if p.is_file() {
            Some(p)
        } else {
            println!(
                "skipping vmaf test: TINYTHIS_TEST_INPUT is set but not a file: {}",
                p.display()
            );
            return;
        }
    } else {
        try_find_input_video_file()
    };

    let Some(input_src) = input_src else {
        println!(
            "skipping vmaf test: no input video found in tests/ (expected one of: mp4/mov/m4v/mkv/webm/avi). You can also set TINYTHIS_TEST_INPUT to an input file path."
        );
        return;
    };

    let input_name = input_src.file_name().and_then(|s| s.to_str()).unwrap();

    let target = repo_root().join("target");
    fs::create_dir_all(&target).expect("create target/");
    let tmp = tempfile::Builder::new()
        .prefix("vmaf_work_")
        .tempdir_in(&target)
        .expect("tempdir");
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    let tinythis_src = PathBuf::from(env!("CARGO_BIN_EXE_tinythis"));
    let tinythis = bin_dir.join("tinythis.exe");
    fs::copy(&tinythis_src, &tinythis).expect("copy tinythis.exe");

    let Some(ffmpeg_src) = try_find_ffmpeg_exe() else {
        println!(
            "skipping vmaf test: ffmpeg.exe not found (put tests/ffmpeg.exe, set TINYTHIS_TEST_FFMPEG, or run `tinythis setup`)"
        );
        return;
    };
    let ffmpeg = bin_dir.join("ffmpeg.exe");
    hardlink_or_copy(&ffmpeg_src, &ffmpeg);

    let Some(ffprobe_src) = try_find_ffprobe_exe() else {
        println!(
            "skipping vmaf test: ffprobe.exe not found (put tests/ffprobe.exe, set TINYTHIS_TEST_FFPROBE, or run `tinythis setup`)"
        );
        return;
    };
    let ffprobe = bin_dir.join("ffprobe.exe");
    hardlink_or_copy(&ffprobe_src, &ffprobe);

    let fps = probe_r_frame_rate(&ffprobe, &tests_dir(), &input_src);

    let cases = [
        Case {
            encoder: Encoder::Cpu,
            preset: Preset::Quality,
        },
        Case {
            encoder: Encoder::Cpu,
            preset: Preset::Balanced,
        },
        Case {
            encoder: Encoder::Cpu,
            preset: Preset::Speed,
        },
        Case {
            encoder: Encoder::Gpu,
            preset: Preset::Quality,
        },
        Case {
            encoder: Encoder::Gpu,
            preset: Preset::Balanced,
        },
        Case {
            encoder: Encoder::Gpu,
            preset: Preset::Speed,
        },
    ];

    let mut results: Vec<(Case, f64)> = Vec::with_capacity(cases.len());

    println!("input: {}", input_src.display());
    println!("vmaf_fps: {}", fps);
    for case in cases {
        let case_dir = tmp.path().join(case.name());
        fs::create_dir_all(&case_dir).expect("create case dir");

        let input_dst = case_dir.join(input_name);
        hardlink_or_copy(&input_src, &input_dst);

        let out_path = run_tinythis_case(&tinythis, &case_dir, case, input_name);
        let mean = run_vmaf(
            &ffmpeg,
            &case_dir,
            &fps,
            Path::new(input_name),
            out_path.file_name().unwrap().as_ref(),
        );

        let out_size = fs::metadata(&out_path).expect("output metadata").len();
        println!(
            "{}: vmaf_mean={:.6}, output_bytes={}",
            case.name(),
            mean,
            out_size
        );

        results.push((case, mean));
    }

    let cpu_quality = results
        .iter()
        .find(|(c, _)| c.encoder == Encoder::Cpu && c.preset == Preset::Quality)
        .unwrap()
        .1;
    let cpu_balanced = results
        .iter()
        .find(|(c, _)| c.encoder == Encoder::Cpu && c.preset == Preset::Balanced)
        .unwrap()
        .1;
    let cpu_speed = results
        .iter()
        .find(|(c, _)| c.encoder == Encoder::Cpu && c.preset == Preset::Speed)
        .unwrap()
        .1;
    assert!(cpu_quality >= cpu_balanced, "cpu: quality < balanced");
    assert!(cpu_balanced >= cpu_speed, "cpu: balanced < speed");

    let gpu_quality = results
        .iter()
        .find(|(c, _)| c.encoder == Encoder::Gpu && c.preset == Preset::Quality)
        .unwrap()
        .1;
    let gpu_balanced = results
        .iter()
        .find(|(c, _)| c.encoder == Encoder::Gpu && c.preset == Preset::Balanced)
        .unwrap()
        .1;
    let gpu_speed = results
        .iter()
        .find(|(c, _)| c.encoder == Encoder::Gpu && c.preset == Preset::Speed)
        .unwrap()
        .1;
    assert!(gpu_quality >= gpu_balanced, "gpu: quality < balanced");
    assert!(gpu_balanced >= gpu_speed, "gpu: balanced < speed");
}
