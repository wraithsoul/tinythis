use std::ffi::OsString;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Preset {
    Quality,
    Balanced,
    Speed,
}

impl Preset {
    pub fn as_str(self) -> &'static str {
        match self {
            Preset::Quality => "quality",
            Preset::Balanced => "balanced",
            Preset::Speed => "speed",
        }
    }
}

pub fn ffmpeg_video_args(preset: Preset, use_gpu: bool) -> Vec<OsString> {
    if !use_gpu {
        return match preset {
            Preset::Quality => vec![
                OsString::from("-c:v"),
                OsString::from("libx264"),
                OsString::from("-preset"),
                OsString::from("slower"),
                OsString::from("-crf"),
                OsString::from("18"),
            ],
            Preset::Balanced => vec![
                OsString::from("-c:v"),
                OsString::from("libx264"),
                OsString::from("-preset"),
                OsString::from("medium"),
                OsString::from("-crf"),
                OsString::from("23"),
            ],
            Preset::Speed => vec![
                OsString::from("-c:v"),
                OsString::from("libx264"),
                OsString::from("-preset"),
                OsString::from("veryfast"),
                OsString::from("-crf"),
                OsString::from("28"),
            ],
        };
    }

    let (nvenc_preset, b_v, maxrate, bufsize, multipass, lookahead, bf) = match preset {
        Preset::Quality => ("p7", "13M", "19M", "38M", "fullres", "32", "3"),
        Preset::Balanced => ("p6", "8M", "12M", "24M", "fullres", "32", "3"),
        Preset::Speed => ("p4", "4M", "6M", "12M", "disabled", "16", "2"),
    };

    vec![
        OsString::from("-c:v"),
        OsString::from("h264_nvenc"),
        OsString::from("-profile:v"),
        OsString::from("high"),
        OsString::from("-preset"),
        OsString::from(nvenc_preset),
        OsString::from("-rc"),
        OsString::from("vbr"),
        OsString::from("-tune"),
        OsString::from("hq"),
        OsString::from("-multipass"),
        OsString::from(multipass),
        OsString::from("-b:v"),
        OsString::from(b_v),
        OsString::from("-maxrate"),
        OsString::from(maxrate),
        OsString::from("-bufsize"),
        OsString::from(bufsize),
        OsString::from("-spatial-aq"),
        OsString::from("1"),
        OsString::from("-temporal-aq"),
        OsString::from("1"),
        OsString::from("-aq-strength"),
        OsString::from("8"),
        OsString::from("-rc-lookahead"),
        OsString::from(lookahead),
        OsString::from("-bf"),
        OsString::from(bf),
        OsString::from("-b_ref_mode"),
        OsString::from("middle"),
    ]
}

pub fn audio_bitrate(preset: Preset) -> &'static str {
    match preset {
        Preset::Quality => "160k",
        Preset::Balanced => "128k",
        Preset::Speed => "96k",
    }
}
