mod args;
mod cmd_setup;
mod cmd_setup_path;
mod cmd_uninstall;
mod cmd_update;
mod positional;

pub use args::Cli;

use crate::error::Result;

pub fn run(command: args::Command) -> Result<()> {
    match command {
        args::Command::Setup(setup) => match setup.command {
            Some(args::SetupSubcommand::Path(args)) => cmd_setup_path::run(args),
            None => cmd_setup::run(setup.args),
        },
        args::Command::Update(args) => cmd_update::run(args),
        args::Command::Uninstall(args) => cmd_uninstall::run(args),
        args::Command::SelfRemove(args) => {
            crate::self_install::run_self_remove(crate::self_install::SelfRemoveArgs {
                pid: args.pid,
                bin_dir: args.bin_dir,
                app_root_dir: args.app_root_dir,
            })
        }
    }
}

pub fn run_positional(cli: &Cli) -> Result<()> {
    positional::run(cli)
}
