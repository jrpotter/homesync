use super::config::PathConfig;
use git2::Repository;
use octocrab;

/// Sets up a local github repository all configuration files will be synced to.
/// We attempt to clone the remote repository in favor of building our own.
///
/// If a remote repository exists, we verify its managed by homesync (based on
/// the presence of a sentinel file `.homesync`). Otherwise we raise an error.
///
/// If there is no local repository but a remote is available, we clone it.
/// Otherwise we create a new, empty repository.
///
/// NOTE! This does not perform any syncing between local and remote. That
/// should be done as a specific command line request.
pub async fn init(config: &PathConfig) {
    // TODO(jrpotter): Fill this out.
}
