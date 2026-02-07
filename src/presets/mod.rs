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

pub fn ffmpeg_video_args(preset: Preset) -> Vec<OsString> {
    match preset {
        Preset::Quality => vec![
            OsString::from("-c:v"),
            OsString::from("libx264"),
            OsString::from("-preset"),
            OsString::from("slow"),
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
    }
}

pub fn audio_bitrate(preset: Preset) -> &'static str {
    match preset {
        Preset::Quality => "160k",
        Preset::Balanced => "128k",
        Preset::Speed => "96k",
    }
}
