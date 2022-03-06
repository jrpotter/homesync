//! The in-memory representation of the homesync config. Refer to
//! [homesync/template.yml](https://github.com/jrpotter/homesync/blob/main/rsrc/template.yml)
//! (copied below) for an example of what this config might look like.
//!
//! ```yaml
//! ---
//! user:
//!   name: name
//!   email: email@email.com
//! ssh:
//!   public: $HOME/.ssh/id_ed25519.pub
//!   private: $HOME/.ssh/id_ed25519
//! repos:
//!   local: $HOME/.homesync
//!   remote:
//!     name: origin
//!     branch: master
//!     url: "https://github.com/owner/repo.git"
//! unmanaged:
//!   - LICENSE
//!   - README.md
//! packages:
//!   homesync:
//!     - $HOME/.homesync.yml
//!     - $HOME/.config/homesync/homesync.yml
//!     - $XDG_CONFIG_HOME/homesync.yml
//!     - $XDG_CONFIG_HOME/homesync/homesync.yml
//! ```

use super::{path, path::ResPathBuf};
use paris::formatter::colorize_string;
use serde_derive::{Deserialize, Serialize};
use simplelog::{info, paris};
use std::{
    collections::{BTreeMap, HashSet},
    env::VarError,
    error, fmt, fs, io,
    io::Write,
    path::PathBuf,
};

// ========================================
// Error
// ========================================

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    IOError(io::Error),
    MissingConfig,
    SerdeError(serde_yaml::Error),
    VarError(VarError),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IOError(err)
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(err: serde_yaml::Error) -> Error {
        Error::SerdeError(err)
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

impl From<VarError> for Error {
    fn from(err: VarError) -> Error {
        Error::VarError(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IOError(e) => write!(f, "{}", e),
            Error::MissingConfig => write!(f, "Could not find configuration file"),
            Error::SerdeError(e) => write!(f, "{}", e),
            Error::VarError(e) => write!(f, "{}", e),
        }
    }
}

impl error::Error for Error {}

// ========================================
// Config
// ========================================

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SSH {
    pub public: Option<PathBuf>,
    pub private: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Remote {
    pub name: String,
    pub branch: String,
    pub url: String,
}

impl Remote {
    pub fn tracking_branch(&self) -> String {
        format!("{}/{}", &self.name, &self.branch)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Repos {
    pub local: PathBuf,
    pub remote: Remote,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub user: User,
    pub ssh: SSH,
    pub repos: Repos,
    pub unmanaged: Option<HashSet<PathBuf>>,
    pub packages: BTreeMap<String, Vec<PathBuf>>,
}

impl Config {
    pub fn new(contents: &str) -> Result<Self> {
        Ok(serde_yaml::from_str(&contents)?)
    }
}

#[derive(Debug)]
pub struct PathConfig {
    pub homesync_yml: ResPathBuf,
    pub config: Config,
}

impl PathConfig {
    pub fn new(path: &ResPathBuf, config: Config) -> Self {
        PathConfig {
            homesync_yml: path.clone(),
            config,
        }
    }

    // TODO(jrpotter): Create backup file before overwriting.
    pub fn write(&self) -> Result<()> {
        let mut file = fs::File::create(&self.homesync_yml)?;
        let serialized = serde_yaml::to_string(&self.config)?;
        file.write_all(serialized.as_bytes())?;
        Ok(())
    }
}

// ========================================
// Loading
// ========================================

/// The paths our homesync configuration may live in, ordered by priority.
pub const DEFAULT_PATHS: &[&str] = &[
    "$HOME/.homesync.yml",
    "$HOME/.config/homesync/homesync.yml",
    "$XDG_CONFIG_HOME/homesync.yml",
    "$XDG_CONFIG_HOME/homesync/homesync.yml",
];

/// The paths our homesync configuration may live in, ordered by priority.
pub fn default_paths() -> Vec<PathBuf> {
    DEFAULT_PATHS.iter().map(|s| PathBuf::from(s)).collect()
}

/// Reads in the homesync configuration file into a [PathConfig](struct.PathConfig.html)
/// instance.
pub fn load(candidates: &Vec<ResPathBuf>) -> Result<PathConfig> {
    // When trying our paths, the only acceptable error is a `NotFound` file.
    // Anything else should be surfaced to the end user.
    for candidate in candidates {
        match fs::read_to_string(candidate) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => Err(Error::IOError(err))?,
            Ok(contents) => {
                let config = Config::new(&contents)?;
                return Ok(PathConfig::new(candidate, config));
            }
        }
    }
    Err(Error::MissingConfig)
}

/// Reads in the homesync configuration file into a [PathConfig](struct.PathConfig.html)
/// instance.
///
/// Useful in cases where we notice the homesync config itself has changed while
/// homesync is running.
pub fn reload(pc: &PathConfig) -> Result<PathConfig> {
    info!(
        "<bold>Reloaded:</> Configuration <cyan>{}</>.",
        pc.config.repos.local.display()
    );
    load(&vec![pc.homesync_yml.clone()])
}

// ========================================
// Listing
// ========================================

/// Prints the list of packages found in a [PathConfig](struct.PathConfig.html).
pub fn list_packages(pc: PathConfig) {
    println!(
        "Listing packages in {}...\n",
        colorize_string(format!(
            "<cyan>{}</>",
            pc.homesync_yml.unresolved().display()
        )),
    );
    // Alphabetical ordered ensured by B-tree implementation.
    for (k, _) in pc.config.packages {
        println!("• {}", k);
    }
}
