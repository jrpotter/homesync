use super::config::PathConfig;
use super::path;
use super::path::ResPathBuf;
use std::env::VarError;
use std::path::{Path, PathBuf};
use std::{error, fmt, fs, io, result};

// ========================================
// Error
// ========================================

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    IOError(io::Error),
    VarError(VarError),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IOError(err)
    }
}

impl From<VarError> for Error {
    fn from(err: VarError) -> Error {
        Error::VarError(err)
    }
}

impl From<path::Error> for Error {
    fn from(err: path::Error) -> Error {
        match err {
            path::Error::IOError(e) => Error::IOError(e),
            path::Error::VarError(e) => Error::VarError(e),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IOError(e) => write!(f, "{}", e),
            Error::VarError(e) => write!(f, "{}", e),
        }
    }
}

impl error::Error for Error {}

// ========================================
// Validation
// ========================================

fn validate_is_file(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        // TODO(jrpotter): Use `IsADirectory` when stable.
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("'{}' is not a file.", path.display()),
        ))?;
    }
    Ok(())
}

fn validate_is_dir(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_dir() {
        // TODO(jrpotter): Use `NotADirectory` when stable.
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("'{}' is not a directory.", path.display()),
        ))?;
    }
    Ok(())
}

pub fn validate_local(path: &Path) -> Result<()> {
    let resolved = path::resolve(path)?;
    validate_is_dir(resolved.as_ref())?;

    let mut local: PathBuf = resolved.into();
    local.push(".git");
    path::resolve(&local).map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Local directory '{}' is not a git repository.",
                path.display()
            ),
        )
    })?;
    validate_is_dir(local.as_ref())?;

    local.pop();
    local.push(".homesync");
    path::resolve(&local).map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Sentinel file '.homesync' missing from local repository '{}'.",
                path.display()
            ),
        )
    })?;
    validate_is_file(local.as_ref())?;

    // TODO(jrpotter): Verify git repository is pointing to remote.

    Ok(())
}

// ========================================
// Repository
// ========================================

fn _setup_repo(path: &Path) -> Result<()> {
    match path.parent() {
        Some(p) => fs::create_dir_all(p)?,
        None => (),
    };
    let mut repo_dir = path.to_path_buf();
    repo_dir.push(".homesync");
    match path::soft_resolve(&repo_dir) {
        // The path already exists. Verify we are working with a git respository
        // with sentinel value.
        Ok(Some(resolved)) => {
            validate_local(resolved.as_ref())?;
        }
        // Path does not exist yet. If a remote path exists, we should clone it.
        // Otherwise boot up a local repsoitory.
        Ok(None) => {}
        Err(e) => Err(e)?,
    }
    Ok(())
}

// ========================================
// Initialization
// ========================================

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
pub fn init(_path: &Path, _config: &PathConfig) -> Result<ResPathBuf> {
    // let repository = match Repository::clone(url, "/path/to/a/repo") {
    //     Ok(repo) => repo,
    //     Err(e) => panic!("failed to clone: {}", e),
    // };
    // Hard resolution should succeed now that the above directory was created.
    // Ok(path::resolve(&expanded)?);
    panic!("")
}
