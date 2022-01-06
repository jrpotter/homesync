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
pub struct Remote {
    pub name: String,
    pub branch: String,
    pub url: Url,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub local: PathBuf,
    pub remote: Remote,
    pub packages: BTreeMap<String, Package>,
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

pub fn reload(pc: &PathConfig) -> Result<PathConfig> {
    info!(
        "<green>{}</> configuration reloaded.",
        pc.config.local.display()
    );
    load(&vec![pc.homesync_yml.clone()])
}

// ========================================
// Creation
// ========================================

fn prompt_default(prompt: &str, default: String) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(default)
    } else {
        Ok(trimmed.to_owned())
    }
}

fn prompt_local(path: Option<&Path>) -> Result<PathBuf> {
    let default = path.map_or("$HOME/.homesync".to_owned(), |p| p.display().to_string());
    let value = prompt_default(
        &format!(
            "Local git repository <{}> (enter to continue): ",
            colorize_string(format!("<yellow>{}</>", &default)),
        ),
        default,
    )?;
    Ok(PathBuf::from(value))
}

fn prompt_remote(remote: Option<&Remote>) -> Result<Remote> {
    let default_name = remote.map_or("origin".to_owned(), |r| r.name.to_owned());
    let remote_name = prompt_default(
        &format!(
            "Remote git name <{}> (enter to continue): ",
            colorize_string(format!("<yellow>{}</>", &default_name))
        ),
        default_name,
    )?;

    let default_branch = remote.map_or("origin".to_owned(), |r| r.branch.to_owned());
    let remote_branch = prompt_default(
        &format!(
            "Remote git branch <{}> (enter to continue): ",
            colorize_string(format!("<yellow>{}</>", &default_branch))
        ),
        default_branch,
    )?;

    let default_url = remote.map_or("https://github.com/owner/repo.git".to_owned(), |r| {
        r.url.to_string()
    });
    let remote_url = prompt_default(
        &format!(
            "Remote git url <{}> (enter to continue): ",
            colorize_string(format!("<yellow>{}</>", &default_url))
        ),
        default_url,
    )?;

    Ok(Remote {
        name: remote_name,
        branch: remote_branch,
        url: Url::parse(&remote_url)?,
    })
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
    let generated = PathConfig {
        homesync_yml: path.clone(),
        config: Config {
            local,
            remote,
            packages: loaded.map_or(BTreeMap::new(), |c| c.packages),
        },
    };
    generated.write()?;
    Ok(generated)
}

// ========================================
// Listing
// ========================================

pub fn list_packages(pc: PathConfig) {
    println!(
        "Listing packages in {}...\n",
        colorize_string(format!(
            "<green>{}</>",
            pc.homesync_yml.unresolved().display()
        )),
    );
    // Alphabetical ordered ensured by B-tree implementation.
    for (k, _) in pc.config.packages {
        println!("• {}", k);
    }
}
