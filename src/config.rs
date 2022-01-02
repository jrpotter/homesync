use super::{path, path::ResPathBuf};
use paris::formatter::colorize_string;
use serde_derive::{Deserialize, Serialize};
use simplelog::{info, paris};
use std::{
    collections::BTreeMap,
    env::VarError,
    error, fmt, fs, io,
    io::Write,
    path::{Path, PathBuf},
};
use url::{ParseError, Url};

// ========================================
// Error
// ========================================

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    IOError(io::Error),
    MissingConfig,
    ParseError(ParseError),
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

impl From<ParseError> for Error {
    fn from(err: ParseError) -> Error {
        Error::ParseError(err)
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
            Error::ParseError(e) => write!(f, "{}", e),
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
pub struct Package {
    pub configs: Vec<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub local: PathBuf,
    pub remote: Url,
    pub packages: BTreeMap<String, Package>,
}

impl Config {
    pub fn new(contents: &str) -> Result<Self> {
        Ok(serde_yaml::from_str(&contents)?)
    }
}

#[derive(Debug)]
pub struct PathConfig(pub ResPathBuf, pub Config);

impl PathConfig {
    pub fn new(path: &ResPathBuf, config: Config) -> Self {
        PathConfig(path.clone(), config)
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

pub fn reload(config: &PathConfig) -> Result<PathConfig> {
    info!(
        "<green>{}</> configuration reloaded.",
        config.1.local.display()
    );
    load(&vec![config.0.clone()])
}

// ========================================
// Creation
// ========================================

fn prompt_local(path: Option<&Path>) -> Result<PathBuf> {
    let default = path.map_or("$HOME/.homesync".to_owned(), |p| p.display().to_string());
    print!(
        "Local git repository <{}> (enter to continue): ",
        colorize_string(format!("<yellow>{}</>", &default)),
    );
    io::stdout().flush()?;
    let mut local = String::new();
    io::stdin().read_line(&mut local)?;
    // Defer validation this path until initialization of the repository.
    let local = local.trim();
    if local.is_empty() {
        Ok(PathBuf::from(default))
    } else {
        Ok(PathBuf::from(local))
    }
}

fn prompt_remote(url: Option<&Url>) -> Result<Url> {
    let default = url.map_or("https://github.com/owner/repo.git".to_owned(), |u| {
        u.to_string()
    });
    print!(
        "Remote git repository <{}> (enter to continue): ",
        colorize_string(format!("<yellow>{}</>", &default)),
    );
    io::stdout().flush()?;
    let mut remote = String::new();
    io::stdin().read_line(&mut remote)?;
    let remote = remote.trim();
    if remote.is_empty() {
        Ok(Url::parse(&default)?)
    } else {
        Ok(Url::parse(&remote)?)
    }
}

pub fn write(path: &ResPathBuf, loaded: Option<Config>) -> Result<PathConfig> {
    println!(
        "Generating config at {}...\n",
        colorize_string(format!("<green>{}</>", path.unresolved().display())),
    );
    let local = prompt_local(match &loaded {
        Some(c) => Some(c.local.as_ref()),
        None => None,
    })?;
    let remote = prompt_remote(match &loaded {
        Some(c) => Some(&c.remote),
        None => None,
    })?;
    let generated = PathConfig(
        path.clone(),
        Config {
            local,
            remote,
            packages: loaded.map_or(BTreeMap::new(), |c| c.packages),
        },
    );
    generated.write()?;
    Ok(generated)
}

// ========================================
// Listing
// ========================================

pub fn list_packages(config: PathConfig) {
    println!(
        "Listing packages in {}...\n",
        colorize_string(format!("<green>{}</>", config.0.unresolved().display())),
    );
    // Alphabetical ordered ensured by B-tree implementation.
    for (k, _) in config.1.packages {
        println!("• {}", k);
    }
}
