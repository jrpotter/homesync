pub mod config;
pub mod copy;
pub mod daemon;
pub mod git;
pub mod path;

use config::PathConfig;
use std::error::Error;

type Result = std::result::Result<(), Box<dyn Error>>;

pub fn run_apply(config: PathConfig, file: Option<&str>) -> Result {
    copy::apply(&config, file)?;
    Ok(())
}

pub fn run_daemon(config: PathConfig, freq_secs: u64) -> Result {
    daemon::launch(config, freq_secs)?;
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
    copy::stage(&config)?;
    Ok(())
}
