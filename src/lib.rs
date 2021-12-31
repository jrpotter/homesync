pub mod cli;
pub mod config;
pub mod daemon;
pub mod git;
pub mod path;

use config::PathConfig;
use path::ResPathBuf;
use std::error::Error;
use std::io;

pub fn run_add(_config: PathConfig) -> Result<(), config::Error> {
    // TODO(jrpotter): Show $EDITOR that allows writing specific package.
    Ok(())
}

pub fn run_daemon(config: PathConfig, freq_secs: u64) -> Result<(), Box<dyn Error>> {
    daemon::launch(config, freq_secs)?;
    Ok(())
}

pub fn run_init(candidates: Vec<ResPathBuf>) -> Result<(), config::Error> {
    debug_assert!(!candidates.is_empty(), "Empty candidates found in `init`.");
    if candidates.is_empty() {
        return Err(config::Error::FileError(io::Error::new(
            io::ErrorKind::NotFound,
            "No suitable config file found.",
        )));
    }
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

pub fn run_list(config: PathConfig) -> Result<(), config::Error> {
    cli::list_packages(config);
    Ok(())
}

pub fn run_pull(_config: PathConfig) -> Result<(), Box<dyn Error>> {
    Ok(())
}

pub fn run_push(_config: PathConfig) -> Result<(), Box<dyn Error>> {
    Ok(())
}
