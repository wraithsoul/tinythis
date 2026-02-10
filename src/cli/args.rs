use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

fn parse_supported_input(s: &str) -> std::result::Result<PathBuf, String> {
    let path = PathBuf::from(s);
    if crate::exec::input::is_supported_video(&path) {
        Ok(path)
    } else {
        Err("unsupported input extension".to_string())
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "tinythis",
    version,
    about = "tinythis! - a lightweight ffmpeg wrapper",
    subcommand_precedence_over_arg = true
)]
pub struct Cli {
    /// input files to compress (when no subcommand is used)
    #[arg(value_name = "INPUT", value_parser = parse_supported_input)]
    pub inputs: Vec<PathBuf>,

    /// use gpu encoder for cli compression, overriding options.toml
    #[arg(long, global = true, conflicts_with = "cpu")]
    pub gpu: bool,

    /// force cpu encoder for cli compression, overriding options.toml
    #[arg(long, global = true, conflicts_with = "gpu")]
    pub cpu: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// compress using the balanced preset
    Balanced(CompressArgs),

    /// compress using the quality preset
    Quality(CompressArgs),

    /// compress using the speed preset
    Speed(CompressArgs),

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
pub struct CompressArgs {
    /// input files to compress
    #[arg(value_name = "INPUT", required = true, value_parser = parse_supported_input)]
    pub inputs: Vec<PathBuf>,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_no_args() {
        let cli = Cli::try_parse_from(["tinythis"]).unwrap();
        assert!(cli.inputs.is_empty());
        assert!(cli.command.is_none());
    }

    #[test]
    fn parses_positional_inputs_default() {
        let cli = Cli::try_parse_from(["tinythis", "a.mp4", "b.mov"]).unwrap();
        assert_eq!(
            cli.inputs,
            vec![PathBuf::from("a.mp4"), PathBuf::from("b.mov")]
        );
        assert!(cli.command.is_none());
    }

    #[test]
    fn parses_preset_subcommand() {
        let cli = Cli::try_parse_from(["tinythis", "balanced", "a.mp4", "b.mov"]).unwrap();
        match cli.command {
            Some(Command::Balanced(args)) => {
                assert_eq!(
                    args.inputs,
                    vec![PathBuf::from("a.mp4"), PathBuf::from("b.mov")]
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn legacy_mode_flag_still_parses() {
        assert!(Cli::try_parse_from(["tinythis", "--mode", "quality", "a.mp4"]).is_err());
    }

    #[test]
    fn rejects_unexpected_args_for_setup() {
        assert!(Cli::try_parse_from(["tinythis", "setup", "a.mp4"]).is_err());
    }

    #[test]
    fn rejects_mode_flag_with_preset_subcommand() {
        assert!(
            Cli::try_parse_from(["tinythis", "balanced", "--mode", "quality", "a.mp4"]).is_err()
        );
    }

    #[test]
    fn requires_inputs_for_preset_subcommands() {
        assert!(Cli::try_parse_from(["tinythis", "speed"]).is_err());
    }

    #[test]
    fn parses_positional_inputs_with_subcommand_but_runtime_should_reject() {
        let cli = Cli::try_parse_from(["tinythis", "a.mp4", "setup"]).unwrap();
        assert_eq!(cli.inputs, vec![PathBuf::from("a.mp4")]);
        assert!(matches!(cli.command, Some(Command::Setup(_))));
    }

    #[test]
    fn parses_gpu_override_flags() {
        let cli = Cli::try_parse_from(["tinythis", "--gpu", "balanced", "a.mp4"]).unwrap();
        assert!(cli.gpu);
        assert!(!cli.cpu);
        assert!(cli.inputs.is_empty());
        assert!(matches!(cli.command, Some(Command::Balanced(_))));

        let cli = Cli::try_parse_from(["tinythis", "--cpu", "a.mp4"]).unwrap();
        assert!(!cli.gpu);
        assert!(cli.cpu);
        assert!(cli.command.is_none());
        assert_eq!(cli.inputs, vec![PathBuf::from("a.mp4")]);

        assert!(Cli::try_parse_from(["tinythis", "--gpu", "--cpu", "a.mp4"]).is_err());
    }
}
