pub mod config;
pub mod daemon;
pub mod git;
pub mod path;

use config::PathConfig;
use path::ResPathBuf;
use std::{error::Error, io};

type Result = std::result::Result<(), Box<dyn Error>>;

pub fn run_apply(config: PathConfig) -> Result {
    let repo = git::init(&config)?;
    git::apply(&config, &repo)?;
    Ok(())
}

pub fn run_daemon(config: PathConfig, freq_secs: u64) -> Result {
    daemon::launch(config, freq_secs)?;
    Ok(())
}

pub fn run_init(candidates: Vec<ResPathBuf>) -> Result {
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
        Ok(loaded) => config::write(&loaded.homesync_yml, Some(loaded.config))?,
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
    git::init(&config)?;
    println!("\nFinished initialization.");
    Ok(())
}

pub fn run_list(config: PathConfig) -> Result {
    config::list_packages(config);
    Ok(())
}
