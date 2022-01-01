pub mod config;
pub mod daemon;
pub mod git;
pub mod path;

use config::PathConfig;
use path::ResPathBuf;
use std::{error::Error, io};

pub fn run_add(_config: PathConfig) -> Result<(), config::Error> {
    // TODO(jrpotter): Show $EDITOR that allows writing specific package.
    Ok(())
}

pub fn run_daemon(config: PathConfig, freq_secs: u64) -> Result<(), Box<dyn Error>> {
    daemon::launch(config, freq_secs)?;
    Ok(())
}

pub fn run_init(candidates: Vec<ResPathBuf>) -> Result<(), Box<dyn Error>> {
    debug_assert!(!candidates.is_empty(), "Empty candidates found in `init`.");
    if candidates.is_empty() {
        Err(config::Error::IOError(io::Error::new(
            io::ErrorKind::NotFound,
            "No suitable config file found.",
        )))?;
    }
    let config = match config::load(&candidates) {
        // Check if we already have a local config somewhere. If so, reprompt
        // the same configuration options and override the values present in the
        // current YAML file.
        Ok(loaded) => config::write(&loaded.0, Some(loaded.1))?,
        // Otherwise create a new config file at the given location. We always
        // assume we want to write to the first file in our priority list. If
        // not, the user should specify which config they want to write using
        // the `-c` flag.
        // TODO(jrpotter): Verify I have permission to write at specified path.
        // Make directories if necessary.
        Err(config::Error::MissingConfig) if !candidates.is_empty() => {
            config::write(&candidates[0], None)?
        }
        Err(e) => Err(e)?,
    };
    // Verify (or create) our local and remote git repositories. The internal
    // git library we chose to use employs async/await so let's wrap around a
    // channel.
    git::init(&config)?;
    println!("Finished initialization.");
    Ok(())
}

pub fn run_list(config: PathConfig) -> Result<(), config::Error> {
    config::list_packages(config);
    Ok(())
}

pub fn run_pull(_config: PathConfig) -> Result<(), Box<dyn Error>> {
    Ok(())
}

pub fn run_push(_config: PathConfig) -> Result<(), Box<dyn Error>> {
    Ok(())
}
