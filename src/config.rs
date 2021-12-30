use ansi_term::Colour::Green;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{env, error, fmt, fs, io};

// ========================================
// Error
// ========================================

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    FileError(io::Error),
    MissingConfig,
    SerdeError(serde_yaml::Error),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::FileError(err)
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(err: serde_yaml::Error) -> Error {
        Error::SerdeError(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::FileError(e) => write!(f, "{}", e),
            Error::MissingConfig => write!(f, "Could not find configuration file"),
            Error::SerdeError(e) => write!(f, "{}", e),
        }
    }
}

impl error::Error for Error {}

// ========================================
// Config
// ========================================

#[derive(Debug, Deserialize, Serialize)]
pub struct Remote {
    pub owner: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Package {
    pub configs: Vec<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub remote: Remote,
    pub packages: HashMap<String, Package>,
}

impl Config {
    pub fn new(contents: &str) -> Result<Self> {
        Ok(serde_yaml::from_str(&contents)?)
    }
}

#[derive(Debug)]
pub struct PathConfig(pub PathBuf, pub Config);

impl PathConfig {
    pub fn new(path: &Path, config: Option<Config>) -> Self {
        PathConfig(
            path.to_path_buf(),
            config.unwrap_or(Config {
                remote: Remote {
                    owner: "example-user".to_owned(),
                    name: "home-config".to_owned(),
                },
                packages: HashMap::new(),
            }),
        )
    }

    pub fn write(&self) -> Result<()> {
        // TODO(jrpotter): Create backup file before overwriting.
        let mut file = fs::File::create(&self.0)?;
        let serialized = serde_yaml::to_string(&self.1)?;
        file.write_all(serialized.as_bytes())?;
        Ok(())
    }
}

// ========================================
// Loading
// ========================================

/// Returns the default configuration files `homesync` looks for.
///
/// - `$HOME/.homesync.yml`
/// - `$HOME/.config/homesync/homesync.yml`
/// - `$XDG_CONFIG_HOME/homesync.yml`
/// - `$XDG_CONFIG_HOME/homesync/homesync.yml`
///
/// Returned `PathBuf`s are looked for in the above order.
pub fn default_paths() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Ok(home) = env::var("HOME") {
        paths.extend_from_slice(&[
            [&home, ".homesync.yml"].iter().collect(),
            [&home, ".config", "homesync", "homesync.yml"]
                .iter()
                .collect(),
        ]);
    }
    if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME") {
        paths.extend_from_slice(&[
            [&xdg_config_home, "homesync.yml"].iter().collect(),
            [&xdg_config_home, "homesync", "homesync.yml"]
                .iter()
                .collect(),
        ]);
    }
    paths
}

pub fn load(candidates: &Vec<PathBuf>) -> Result<PathConfig> {
    // When trying our paths, the only acceptable error is a `NotFound` file.
    // Anything else should be surfaced to the end user.
    for path in candidates {
        match fs::read_to_string(path) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(Error::FileError(err)),
            Ok(contents) => {
                let config = Config::new(&contents)?;
                return Ok(PathConfig::new(&path, Some(config)));
            }
        }
    }
    Err(Error::MissingConfig)
}
