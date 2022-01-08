pub mod config;
pub mod daemon;
pub mod git;
pub mod path;

use config::PathConfig;
use std::error::Error;

type Result = std::result::Result<(), Box<dyn Error>>;

pub fn run_daemon(config: PathConfig, freq_secs: u64) -> Result {
    let repo = git::init(&config)?;
    daemon::launch(config, repo, freq_secs)?;
    Ok(())
}

pub fn run_list(config: PathConfig) -> Result {
    config::list_packages(config);
    Ok(())
}

pub fn run_push(config: PathConfig) -> Result {
    let mut repo = git::init(&config)?;
    git::push(&config, &mut repo)?;
    Ok(())
}

pub fn run_pull(config: PathConfig) -> Result {
    let mut repo = git::init(&config)?;
    git::pull(&config, &mut repo)?;
    Ok(())
}

pub fn run_stage(config: PathConfig) -> Result {
    let repo = git::init(&config)?;
    git::stage(&config, &repo)?;
    Ok(())
}
