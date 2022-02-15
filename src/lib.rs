//! [homesync](https://github.com/jrpotter/homesync) is a project for collecting
//! various files strewn across your computer and consolidating them
//! automatically into a local git repository. It has the means of pushing those
//! changes to a remote git repository for syncing to other machines. Homesync
//! can pull in these changes on a different machine and *apply* the files,
//! putting them all in the correct spot.
//!
//! Throughout this documentation the "local repository" always refers to the
//! git repository homesync is managing (and potentially created on start). The
//! remote repository refers to the git repo hosted at the URL specified in the
//! homesync config.
//!
//! Thank you for your interest in contributing!

pub mod config;
pub mod copy;
pub mod daemon;
pub mod git;
pub mod path;

use config::PathConfig;
use std::error::Error;

type Result = std::result::Result<(), Box<dyn Error>>;

/// Refer to [copy::apply](copy/fn.apply.html).
pub fn run_apply(config: PathConfig, package: Option<&str>) -> Result {
    copy::apply(&config, package)?;
    Ok(())
}

/// Refer to [daemon::launch](daemon/fn.launch.html).
pub fn run_daemon(config: PathConfig, freq_secs: u64) -> Result {
    daemon::launch(config, freq_secs)?;
    Ok(())
}

/// Refer to [config::list_packages](config/fn.list_packages.html).
pub fn run_list(config: PathConfig) -> Result {
    config::list_packages(config);
    Ok(())
}

/// Refer to [git::push](git/fn.run_push.html).
pub fn run_push(config: PathConfig) -> Result {
    let mut repo = git::init(&config)?;
    git::push(&config, &mut repo)?;
    Ok(())
}

/// Refer to [git::pull](git/fn.run_pull.html).
pub fn run_pull(config: PathConfig) -> Result {
    let mut repo = git::init(&config)?;
    git::pull(&config, &mut repo)?;
    Ok(())
}

/// Refer to [copy::stage](copy/fn.stage.html).
pub fn run_stage(config: PathConfig) -> Result {
    copy::stage(&config)?;
    Ok(())
}
