use super::{config::PathConfig, path, path::ResPathBuf};
use simplelog::{info, paris};
use std::{
    collections::HashMap,
    env::VarError,
    error, fmt, fs, io,
    path::{Path, PathBuf},
    result,
};

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
// Application
// ========================================

fn apply_all(pc: &PathConfig) -> Result<()> {
    let workdir = path::resolve(&pc.config.repos.local)?;
    let repo_lookup = get_repo_lookup(workdir.as_ref(), workdir.as_ref())?;
    let package_lookup = get_package_lookup(pc);

    for (repo_unresolved, repo_resolved) in &repo_lookup {
        if let Some(package_resolved) = package_lookup.get(repo_unresolved) {
            fs::copy(repo_resolved, package_resolved)?;
            info!(
                "Copied `{}` from local repository.",
                repo_unresolved.display(),
            );
        }
    }

    Ok(())
}

fn apply_one(pc: &PathConfig, target: &Path) -> Result<()> {
    let workdir = path::resolve(&pc.config.repos.local)?;
    let repo_lookup = get_repo_lookup(workdir.as_ref(), workdir.as_ref())?;
    let package_lookup = get_package_lookup(pc);

    // The user must specify a path that matches the unresolved one.
    if let Some(repo_resolved) = repo_lookup.get(target) {
        if let Some(package_resolved) = package_lookup.get(target) {
            fs::copy(repo_resolved, package_resolved)?;
            info!("Copied `{}` from local repository.", target.display(),);
        }
    }

    Ok(())
}

pub fn apply(pc: &PathConfig, file: Option<&str>) -> Result<()> {
    if let Some(file) = file {
        apply_one(pc, Path::new(file))
    } else {
        apply_all(pc)
    }
}

// ========================================
// Staging
// ========================================

pub fn stage(pc: &PathConfig) -> Result<()> {
    let workdir = path::resolve(&pc.config.repos.local)?;
    let repo_lookup = get_repo_lookup(workdir.as_ref(), workdir.as_ref())?;
    let package_lookup = get_package_lookup(pc);

    // Find all files in our repository that are no longer being referenced in
    // our primary config file. They should be removed from the repository.
    for (repo_unresolved, repo_resolved) in &repo_lookup {
        if !package_lookup.contains_key(repo_unresolved) {
            fs::remove_file(repo_resolved)?;
        }
        if let Some(p) = repo_resolved.resolved().parent() {
            if p.read_dir()?.next().is_none() {
                fs::remove_dir(p)?;
            }
        }
    }

    // Find all resolvable files in our primary config and copy them into the
    // repository.
    for (package_unresolved, package_resolved) in &package_lookup {
        let mut copy = package_resolved.resolved().to_path_buf();
        copy.push(package_unresolved);
        if let Some(p) = copy.parent() {
            fs::create_dir_all(p)?;
        }
        fs::copy(package_resolved.resolved(), copy)?;
    }

    info!(
        "Staged files. Run <italic>git -C <green>{}</> <italic>status</> to see what changed.",
        &pc.config.repos.local.display()
    );

    Ok(())
}

// ========================================
// Utility
// ========================================

fn get_repo_lookup(root: &Path, path: &Path) -> Result<HashMap<PathBuf, ResPathBuf>> {
    let mut seen = HashMap::new();
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let nested = entry?.path();
            if nested.is_dir() {
                if nested.ends_with(".git") {
                    continue;
                }
                let nested = get_repo_lookup(root, &nested)?;
                seen.extend(nested);
            } else {
                let relative = nested
                    .strip_prefix(root)
                    .expect("Relative git file could not be stripped properly.")
                    .to_path_buf();
                seen.insert(relative, ResPathBuf::new(&nested)?);
            }
        }
    }
    Ok(seen)
}

fn get_package_lookup(pc: &PathConfig) -> HashMap<PathBuf, ResPathBuf> {
    let mut seen = HashMap::new();
    for (_, packages) in &pc.config.packages {
        for path in packages {
            if let Ok(resolved) = path::resolve(path) {
                seen.insert(path.to_path_buf(), resolved);
            }
        }
    }
    seen
}
