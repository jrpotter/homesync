mod homesync;

use homesync::config;
use std::error::Error;

pub fn run_configure(_matches: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    // Check if we already have a local config somewhere. If so, reprompt the
    // same configuration options and override the values present in the current
    // YAML file.
    let _config = match config::find_config() {
        Ok(conf) => Ok(conf),
        Err(config::Error::MissingConfig) => Ok(config::generate_config()),
        Err(config::Error::WithFile(e)) => Err(e),
    };
    Ok(())
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
