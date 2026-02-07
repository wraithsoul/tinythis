use std::io::Write;

use crate::error::Result;

pub fn run(args: super::args::UpdateArgs) -> Result<()> {
    let update = crate::update::check_latest_release(crate::update::DEFAULT_REPO)?;
    let Some(update) = update else {
        println!("up to date");
        return Ok(());
    };

    println!(
        "update available: v{} -> v{} ({})",
        update.current, update.latest, update.tag
    );

    if !args.yes && !confirm_update()? {
        return Ok(());
    }

    crate::update::apply_update(&update, false)?;
    println!("updating...");
    std::process::exit(0);
}

fn confirm_update() -> Result<bool> {
    print!("update now? [y/N] ");
    std::io::stdout().flush()?;

    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    let s = s.trim().to_ascii_lowercase();
    Ok(s == "y" || s == "yes")
}
