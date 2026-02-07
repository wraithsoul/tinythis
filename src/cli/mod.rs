mod args;
mod cmd_setup;
mod cmd_uninstall;
mod cmd_update;
mod positional;

pub use args::Cli;

use crate::error::Result;

pub fn run(command: args::Command) -> Result<()> {
    match command {
        args::Command::Setup(args) => cmd_setup::run(args),
        args::Command::Update(args) => cmd_update::run(args),
        args::Command::Uninstall(args) => cmd_uninstall::run(args),
        args::Command::SelfReplace(args) => {
            crate::update::run_self_replace(crate::update::SelfReplaceArgs {
                pid: args.pid,
                src: args.src,
                dst: args.dst,
                relaunch: args.relaunch,
            })
        }
    }
}

pub fn run_positional(cli: &Cli) -> Result<()> {
    positional::run(cli)
}
