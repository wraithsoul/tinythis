mod assets;
mod cli;
mod error;
mod exec;
mod paths;
mod presets;
mod process;
mod self_install;
mod tui;
mod update;

use clap::Parser;

fn main() -> std::process::ExitCode {
    if let Err(err) = real_main() {
        eprintln!("{err}");
        return std::process::ExitCode::FAILURE;
    }

    std::process::ExitCode::SUCCESS
}

fn real_main() -> crate::error::Result<()> {
    let cli = crate::cli::Cli::parse();

    if cli.command.is_some() && !cli.inputs.is_empty() {
        return Err(crate::error::TinythisError::InvalidArgs(
            "positional inputs cannot be combined with subcommands".to_string(),
        ));
    }

    if !cli.inputs.is_empty() {
        return crate::cli::run_positional(&cli);
    }

    match cli.command {
        Some(command) => crate::cli::run(command),
        None => {
            if cfg!(windows) {
                if let Err(e) = crate::assets::ffmpeg::ensure_installed(false) {
                    eprintln!("auto-setup failed (assets): {e}");
                }
                if let Err(e) = crate::self_install::install(false) {
                    eprintln!("auto-setup failed (path): {e}");
                }
            }
            crate::tui::run()
        }
    }
}
