use super::path;
use super::path::{NormalPathBuf, Normalize};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::{error, fmt, fs, io};

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
pub struct PathConfig(pub NormalPathBuf, pub Config);

impl PathConfig {
    pub fn new(path: &NormalPathBuf, config: Option<Config>) -> Self {
        PathConfig(
            path.clone(),
            config.unwrap_or(Config {
                remote: Remote {
                    owner: "example-user".to_owned(),
                    name: "home-config".to_owned(),
                },
                packages: HashMap::new(),
            }),
        )
    }

    // TODO(jrpotter): Create backup file before overwriting.
    pub fn write(&self) -> Result<()> {
        let mut file = fs::File::create(&self.0)?;
        let serialized = serde_yaml::to_string(&self.1)?;
        file.write_all(serialized.as_bytes())?;
        Ok(())
    }
}

// ========================================
// Loading
// ========================================

pub const DEFAULT_PATHS: &[&str] = &[
    "$HOME/.homesync.yml",
    "$HOME/.config/homesync/homesync.yml",
    "$XDG_CONFIG_HOME/homesync.yml",
    "$XDG_CONFIG_HOME/homesync/homesync.yml",
];

pub fn default_paths() -> Vec<PathBuf> {
    DEFAULT_PATHS.iter().map(|s| PathBuf::from(s)).collect()
}

pub fn load(candidates: &Vec<NormalPathBuf>) -> Result<PathConfig> {
    // When trying our paths, the only acceptable error is a `NotFound` file.
    // Anything else should be surfaced to the end user.
    for candidate in candidates {
        match fs::read_to_string(candidate) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(Error::FileError(err)),
            Ok(contents) => {
                let config = Config::new(&contents)?;
                return Ok(PathConfig::new(candidate, Some(config)));
            }
        }
    }
    Err(Error::MissingConfig)
}
