pub mod cli;
pub mod config;
pub mod daemon;

use config::PathConfig;
use std::error::Error;
use std::path::PathBuf;

pub fn run_add(_candidates: Vec<PathBuf>) -> Result<(), config::Error> {
    // TODO(jrpotter): Show $EDITOR that allows writing specific package.
    Ok(())
}

pub fn run_daemon(candidates: Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    let loaded = config::load(&candidates)?;
    daemon::launch(loaded)?;
    Ok(())
}

pub fn run_init(candidates: Vec<PathBuf>) -> Result<(), config::Error> {
    match config::load(&candidates) {
        // Check if we already have a local config somewhere. If so, reprompt
        // the same configuration options and override the values present in the
        // current YAML file.
        Ok(pending) => cli::write_config(pending),
        // Otherwise create a new config file at the given location. We always
        // assume we want to write to the first file in our priority list. If
        // not, the user should specify which config they want to write using
        // the `-c` flag.
        // TODO(jrpotter): Verify I have permission to write at specified path.
        // Make directories if necessary.
        Err(config::Error::MissingConfig) if !candidates.is_empty() => {
            let pending = PathConfig::new(&candidates[0], None);
            cli::write_config(pending)
        }
        Err(e) => Err(e),
    }
}

pub fn run_list(candidates: Vec<PathBuf>) -> Result<(), config::Error> {
    let loaded = config::load(&candidates)?;
    cli::list_packages(loaded);
    Ok(())
}

pub fn run_pull() -> Result<(), Box<dyn Error>> {
    Ok(())
}

pub fn run_push() -> Result<(), Box<dyn Error>> {
    Ok(())
}
