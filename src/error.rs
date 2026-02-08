use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum TinythisError {
    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(&'static str),

    #[error("missing required environment variable: {0}")]
    MissingEnv(&'static str),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),

    #[error(transparent)]
    SelfUpdate(#[from] self_update::errors::Error),

    #[error("expected asset entry not found in zip: {name}")]
    AssetEntryMissing { name: &'static str },

    #[error("ffmpeg install incomplete; missing: {missing:?}")]
    InstallIncomplete { missing: Vec<PathBuf> },

    #[error("process failed: {program} (exit code: {code:?})\n{stderr}")]
    ProcessFailed {
        program: String,
        code: Option<i32>,
        stderr: String,
    },

    #[error("windows registry error in {api}: {code}")]
    Registry { api: &'static str, code: u32 },

    #[error("{0}")]
    InvalidArgs(String),
}

pub type Result<T> = std::result::Result<T, TinythisError>;
