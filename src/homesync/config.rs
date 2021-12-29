use std::env;
use std::error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use yaml_rust::{ScanError, Yaml, YamlLoader};

// ========================================
// Error
// ========================================

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy)]
pub enum Key {
    Packages,    // OPTIONAL
    Remote,      // REQUIRED
    RemoteName,  // REQUIRED
    RemoteOwner, // REQUIRED
}

impl Key {
    fn to_yaml(&self) -> Yaml {
        Yaml::String(self.to_string())
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Key::Packages => write!(f, "packages"),
            Key::Remote => write!(f, "remote"),
            Key::RemoteName => write!(f, "name"),
            Key::RemoteOwner => write!(f, "owner"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ErrorKind {
    // Indicates our top-level data structure isn't a dictionary.
    InvalidHash,
    // Indicates a required key was not found.
    MissingKey(Key),
    // Indicates multiple YAML documents were found within our file.
    MultipleDocuments,
    // Indicates no YAML documents were found within our file.
    NoDocument,
    // Indicates there was a scan error when parsing the YAML.
    ScanError(ScanError),
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorKind::InvalidHash => write!(f, "expected dictionary"),
            ErrorKind::MissingKey(k) => write!(f, "missing key '{}'", k),
            ErrorKind::MultipleDocuments => write!(f, "has multiple YAML documents"),
            ErrorKind::NoDocument => write!(f, "has no YAML document"),
            ErrorKind::ScanError(s) => s.fmt(f),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ErrorWithFile {
    path: PathBuf,
    kind: ErrorKind,
}

impl ErrorWithFile {
    fn new(path: &Path, kind: ErrorKind) -> Self {
        ErrorWithFile {
            path: path.to_path_buf(),
            kind,
        }
    }
}

impl fmt::Display for ErrorWithFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ErrorWithFile { path, kind } = self;
        write!(
            f,
            "File {} failed with error: {}",
            path.to_str().ok_or(fmt::Error)?,
            kind
        )
    }
}

#[derive(Debug, Clone)]
pub enum Error {
    // Indicates we could not find the configuration file at all.
    MissingConfig,
    // Indicates an error occurred when reading the configuration file.
    WithFile(ErrorWithFile),
}

impl Error {
    pub fn new(path: &Path, kind: ErrorKind) -> Self {
        Error::WithFile(ErrorWithFile::new(path, kind))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::MissingConfig => write!(
                f,
                "\
                Could not find a valid configuration file. Looked in \
                \n\n- `$HOME/.homesync.yml` \
                \n- `$HOME/.config/homesync/homesync.yml` \
                \n- `$XDG_CONFIG_HOME/homesync.yml` \
                \n- `$XDG_CONFIG_HOME/homesync/homesync.yml` \
                \nin order."
            ),
            Error::WithFile(e) => write!(f, "{}", e),
        }
    }
}

impl error::Error for Error {}

// ========================================
// Config
// ========================================

pub struct Remote {
    pub owner: String,
    pub name: String,
}

pub struct Package {
    pub name: String,
    pub configs: Vec<PathBuf>,
}

pub struct Config {
    pub path: PathBuf,
    pub remote: Remote,
    pub packages: Vec<Package>,
}

impl Config {
    pub fn new(path: &Path, contents: &str) -> Result<Self> {
        if let Yaml::Hash(pairs) = get_document(path, contents)? {
            let remote = pairs
                .get(&Key::Remote.to_yaml())
                .ok_or(Error::new(path, ErrorKind::MissingKey(Key::Remote)))?;
            let remote = parseRemote(path, remote)?;
            let packages = pairs.get(&Key::Packages.to_yaml()).unwrap_or(&Yaml::Null);
            let packages = parsePackages(path, packages)?;
            // We intentionally ignore any other keys we may encounter.
            Ok(Config {
                path: path.to_path_buf(),
                remote,
                packages,
            })
        } else {
            Err(Error::new(path, ErrorKind::InvalidHash))
        }
    }
}

fn get_document(path: &Path, contents: &str) -> Result<Yaml> {
    match YamlLoader::load_from_str(contents) {
        Ok(mut docs) => {
            if docs.len() > 1 {
                Err(Error::new(path, ErrorKind::MultipleDocuments))
            } else if docs.is_empty() {
                Err(Error::new(path, ErrorKind::NoDocument))
            } else {
                Ok(docs.swap_remove(0))
            }
        }
        Err(e) => Err(Error::new(path, ErrorKind::ScanError(e))),
    }
}

// ========================================
// Parsers
// ========================================

fn parseRemote(path: &Path, value: &Yaml) -> Result<Remote> {
    Ok(Remote {
        owner: String::new(),
        name: String::new(),
    })
}

fn parsePackages(path: &Path, value: &Yaml) -> Result<Vec<Package>> {
    Ok(Vec::new())
}

// ========================================
// Public
// ========================================

/// Attempt to read in the project config in the following priorities:
///
/// - `$HOME/.homesync.yml`
/// - `$HOME/.config/homesync/homesync.yml`
/// - `$XDG_CONFIG_HOME/homesync.yml`
/// - `$XDG_CONFIG_HOME/homesync/homesync.yml`
///
/// Returns an error if a file does not exist in any of these locations or a
/// found file contains invalid YAML.
pub fn find_config() -> Result<Config> {
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
    // When trying our paths, the only acceptable error is a `NotFound` file.
    // Anything else should be surfaced to the end user.
    for path in paths {
        if let Ok(Some(contents)) = read_optional_config(&path) {
            return Ok(Config::new(&path, &contents)?);
        }
    }
    Err(Error::MissingConfig)
}

fn read_optional_config(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Err(err) => match err.kind() {
            // Ignore `NotFound` since we want to try multiple paths.
            io::ErrorKind::NotFound => Ok(None),
            _ => Err(err),
        },
        Ok(contents) => Ok(Some(contents)),
    }
}

pub fn generate_config() -> Config {
    Config {
        path: PathBuf::from(""),
        remote: Remote {
            owner: "".to_owned(),
            name: "".to_owned(),
        },
        packages: Vec::new(),
    }
}
