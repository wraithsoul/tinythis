mod assets;
mod cli;
mod confirm;
mod error;
mod exec;
mod options;
mod paths;
mod prefs;
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
    use std::io::IsTerminal;

    let cli = crate::cli::Cli::parse();

    if !cli.inputs.is_empty() {
        if cli.command.is_some() {
            return Err(crate::error::TinythisError::InvalidArgs(
                "cannot combine positional inputs with a subcommand".to_string(),
            ));
        }
        return crate::cli::run_positional(&cli);
    }

    match cli.command {
        Some(command) => crate::cli::run(cli.gpu, cli.cpu, command),
        None => {
            let mut initial_status: Option<String> = None;
            if cfg!(windows) {
                let interactive = std::io::stdin().is_terminal();

                if interactive {
                    let bin_dir = crate::paths::tinythis_bin_dir()?;
                    if !crate::self_install::user_path_contains(&bin_dir)? {
                        if crate::prefs::path_opted_out()? {
                            // user previously declined. `tinythis setup path` can override.
                        } else if crate::confirm::confirm(
                            "add tinythis to your PATH for quick use?",
                        )? {
                            let _ = crate::self_install::install(false)?;
                            let _ = crate::prefs::set_path_opted_out(false);
                        } else {
                            let _ = crate::prefs::set_path_opted_out(true);
                            initial_status = Some(
                                "path: skipped (run `tinythis setup path` to install later)"
                                    .to_string(),
                            );
                        }
                    }
                }
            }
            crate::tui::run(initial_status)
        }
    }
}
