pub mod config;

use std::error::Error;
use std::path::PathBuf;

pub fn run_configure(
    paths: Vec<PathBuf>,
    _matches: &clap::ArgMatches,
) -> Result<(), Box<dyn Error>> {
    // Check if we already have a local config somewhere. If so, reprompt the
    // same configuration options and override the values present in the current
    // YAML file.
    match config::read_config(&paths) {
        Ok(_) => {
            print!("successfully read\n");
            Ok(())
        }
        Err(config::Error::MissingConfig) => {
            print!("missing config\n");
            Ok(())
        }
        Err(e) => Err(Box::new(e)),
    }
}

pub fn run_push(_matches: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    Ok(())
}

pub fn run_pull(_matches: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    Ok(())
}

pub fn run_add(_matches: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    Ok(())
}
