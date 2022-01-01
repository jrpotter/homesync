use super::config::PathConfig;
use super::path::ResPathBuf;
use super::{config, path};
use ansi_term::Colour::{Green, Yellow};
use std::env::VarError;
use std::io::Write;
use std::path::PathBuf;
use std::{error, fmt, fs, io};
use url::{ParseError, Url};

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

// TODO(jrpotter): Use curses to make this module behave nicer.

fn prompt_local(config: &PathConfig) -> Result<ResPathBuf> {
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
    let expanded = PathBuf::from(path::expand_env(&local.trim())?);
    // We need to generate the directory beforehand to verify the path is
    // actually valid. Worst case this leaves empty directories scattered in
    // various locations after repeated initialization.
    fs::create_dir_all(&expanded)?;
    // Hard resolution should succeed now that the above directory was created.
    Ok(path::resolve(&expanded)?)
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
    pending.1.local = Some(prompt_local(&pending)?);
    pending.1.remote = prompt_remote(&pending)?;
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
