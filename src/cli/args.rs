use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "tinythis",
    version,
    about = "tinythis! - a lightweight ffmpeg wrapper"
)]
pub struct Cli {
    /// input files to compress (when no subcommand is used)
    #[arg(value_name = "INPUT")]
    pub inputs: Vec<PathBuf>,

    /// compression mode for positional inputs (defaults to balanced)
    #[arg(long, value_enum)]
    pub mode: Option<ModeArg>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// download and install ffmpeg/ffprobe assets and add tinythis to your PATH
    Setup(SetupArgs),

    /// check GitHub Releases and update tinythis
    Update(UpdateArgs),

    /// remove ffmpeg assets and remove tinythis from your PATH
    Uninstall(UninstallArgs),

    #[command(hide = true)]
    SelfReplace(SelfReplaceArgs),
}

#[derive(Debug, Args)]
pub struct SetupArgs {
    /// re-download and re-install even if already installed
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// skip confirmation prompt
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct UninstallArgs {}

#[derive(Debug, Args)]
pub struct SelfReplaceArgs {
    /// parent pid to wait for
    #[arg(long)]
    pub pid: u32,

    /// path to the staged exe to install
    #[arg(long)]
    pub src: PathBuf,

    /// destination exe path to replace
    #[arg(long)]
    pub dst: PathBuf,

    /// relaunch tinythis after replacing
    #[arg(long)]
    pub relaunch: bool,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
pub enum ModeArg {
    Quality,
    Balanced,
    Speed,
}

impl ModeArg {
    pub fn to_preset(self) -> crate::presets::Preset {
        match self {
            ModeArg::Quality => crate::presets::Preset::Quality,
            ModeArg::Balanced => crate::presets::Preset::Balanced,
            ModeArg::Speed => crate::presets::Preset::Speed,
        }
    }
}
