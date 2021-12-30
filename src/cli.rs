use super::config;
use super::config::PathConfig;
use ansi_term::Colour::Green as Success;
use ansi_term::Colour::Yellow as Warning;
use std::io;
use std::io::Write;

// TODO(jrpotter): Use curses to make this module behave nicer.

pub fn write_config(mut pending: PathConfig) -> config::Result<()> {
    println!(
        "Generating config at {}...\n",
        Success.paint(pending.0.display().to_string())
    );

    print!(
        "Git repository owner <{}> (enter to continue): ",
        Warning.paint(pending.1.remote.owner.trim())
    );
    io::stdout().flush()?;
    let mut owner = String::new();
    io::stdin().read_line(&mut owner)?;
    let owner = owner.trim().to_owned();
    if !owner.is_empty() {
        pending.1.remote.owner = owner;
    }

    print!(
        "Git repository name <{}> (enter to continue): ",
        Warning.paint(pending.1.remote.name.trim())
    );
    io::stdout().flush()?;
    let mut name = String::new();
    io::stdin().read_line(&mut name)?;
    let name = name.trim().to_owned();
    if !name.is_empty() {
        pending.1.remote.name = name;
    }

    pending.write()?;
    println!("\nFinished writing configuration file.");
    Ok(())
}

pub fn list_packages(config: PathConfig) {
    println!(
        "Listing packages in {}...\n",
        Success.paint(config.0.display().to_string())
    );
    // TODO(jrpotter): Alphabetize the output list.
    for (k, _) in config.1.packages {
        println!("• {}", k);
    }
}
