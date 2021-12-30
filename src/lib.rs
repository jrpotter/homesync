pub mod config;

use ansi_term::Colour::Green;
use config::Config;
use std::error::Error;
use std::path::{Path, PathBuf};

pub fn run_add(paths: Vec<PathBuf>) -> Result<(), config::Error> {
    debug_assert!(!paths.is_empty(), "`run_init` paths empty");
    if paths.is_empty() {
        return Err(config::Error::MissingConfig);
    }
    // TODO(jrpotter): Show $EDITOR that allows writing specific package.
    Ok(())
}

pub fn run_init(paths: Vec<PathBuf>) -> Result<(), config::Error> {
    // TODO(jrpotter): Use a nonempty implementation instead of this.
    debug_assert!(!paths.is_empty(), "`run_init` paths empty");
    if paths.is_empty() {
        return Err(config::Error::MissingConfig);
    }
    // Check if we already have a local config somewhere. If so, reprompt the
    // same configuration options and override the values present in the current
    // YAML file.
    match config::load(&paths) {
        Ok((path, config)) => config::init(path, config),
        // TODO(jrpotter): Verify I have permission to write at specified path.
        // Make directories if necessary.
        Err(config::Error::MissingConfig) => config::init(&paths[0], Config::default()),
        Err(e) => Err(e),
    }
}

pub fn run_list(paths: Vec<PathBuf>) -> Result<(), config::Error> {
    debug_assert!(!paths.is_empty(), "`run_init` paths empty");
    if paths.is_empty() {
        return Err(config::Error::MissingConfig);
    }
    match config::load(&paths) {
        Ok((path, config)) => {
            // TODO(jrpotter): Should sort these entries.
            // Also clean up where I use the console writing or not.
            println!(
                "Listing packages at {}...\n",
                Green.paint(path.display().to_string())
            );
            for (k, _) in config.packages {
                println!("• {}", k);
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

pub fn run_pull(_: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    Ok(())
}

pub fn run_push(_: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    Ok(())
}
