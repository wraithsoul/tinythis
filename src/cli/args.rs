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
    /// download and install ffmpeg assets and add tinythis to your PATH
    Setup(SetupCmd),

    /// check GitHub Releases and update tinythis
    Update(UpdateArgs),

    /// remove ffmpeg assets and remove tinythis from your PATH
    Uninstall(UninstallArgs),

    #[command(hide = true)]
    SelfRemove(SelfRemoveArgs),
}

#[derive(Debug, Args)]
pub struct SetupCmd {
    #[command(flatten)]
    pub args: SetupArgs,

    #[command(subcommand)]
    pub command: Option<SetupSubcommand>,
}

#[derive(Debug, Subcommand)]
pub enum SetupSubcommand {
    /// add tinythis to your user PATH
    Path(SetupPathArgs),
}

#[derive(Debug, Args)]
pub struct SetupPathArgs {}

#[derive(Debug, Args)]
pub struct SetupArgs {
    /// re-download and re-install even if already installed
    #[arg(long)]
    pub force: bool,

    /// skip the PATH prompt and add tinythis to your user PATH (when missing)
    #[arg(long)]
    pub yes: bool,
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
pub struct SelfRemoveArgs {
    /// parent pid to wait for
    #[arg(long)]
    pub pid: u32,

    /// bin directory to remove
    #[arg(long)]
    pub bin_dir: PathBuf,

    /// app root directory to remove if empty
    #[arg(long)]
    pub app_root_dir: PathBuf,
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
