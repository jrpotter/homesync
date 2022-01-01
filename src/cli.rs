use super::config::PathConfig;
use super::{config, git, path};
use ansi_term::Colour::{Green, Yellow};
use std::env::VarError;
use std::io::Write;
use std::path::PathBuf;
use std::{error, fmt, io};
use url::{ParseError, Url};

// TODO(jrpotter): Use curses to make this module behave nicer.

// ========================================
// Error
// ========================================

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    ConfigError(config::Error),
    IOError(io::Error),
    ParseError(ParseError),
    VarError(VarError),
}

impl From<config::Error> for Error {
    fn from(err: config::Error) -> Error {
        Error::ConfigError(err)
    }
}

impl From<git::Error> for Error {
    fn from(err: git::Error) -> Error {
        match err {
            git::Error::IOError(e) => Error::IOError(e),
            git::Error::VarError(e) => Error::VarError(e),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IOError(err)
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
            Error::ConfigError(e) => write!(f, "{}", e),
            Error::IOError(e) => write!(f, "{}", e),
            Error::ParseError(e) => write!(f, "{}", e),
            Error::VarError(e) => write!(f, "{}", e),
        }
    }
}

impl error::Error for Error {}

// ========================================
// Prompts
// ========================================

fn prompt_local(config: &PathConfig) -> Result<PathBuf> {
    print!(
        "Local git repository <{}> (enter to continue): ",
        Yellow.paint(
            config
                .1
                .local
                .as_ref()
                .map_or("".to_owned(), |v| v.display().to_string())
        )
    );
    io::stdout().flush()?;
    let mut local = String::new();
    io::stdin().read_line(&mut local)?;
    Ok(PathBuf::from(path::expand_env(&local.trim())?))
}

fn prompt_remote(config: &PathConfig) -> Result<Url> {
    print!(
        "Remote git repository <{}> (enter to continue): ",
        Yellow.paint(config.1.remote.to_string())
    );
    io::stdout().flush()?;
    let mut remote = String::new();
    io::stdin().read_line(&mut remote)?;
    Ok(Url::parse(&remote)?)
}

// ========================================
// CLI
// ========================================

pub fn write_config(mut pending: PathConfig) -> Result<()> {
    println!(
        "Generating config at {}...\n",
        Green.paint(pending.0.unresolved().display().to_string())
    );
    let local = prompt_local(&pending)?;
    let remote = prompt_remote(&pending)?;
    // Try to initialize the local respository if we can.
    let resolved = git::init(&local, &pending)?;
    pending.1.local = Some(resolved);
    pending.1.remote = remote;
    pending.write()?;
    println!("\nFinished writing configuration file.");
    Ok(())
}

pub fn list_packages(config: PathConfig) {
    println!(
        "Listing packages in {}...\n",
        Green.paint(config.0.unresolved().display().to_string())
    );
    // Alphabetical ordered ensured by B-tree implementation.
    for (k, _) in config.1.packages {
        println!("• {}", k);
    }
}
