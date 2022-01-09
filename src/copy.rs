use super::{config::PathConfig, path, path::ResPathBuf};
use git2::Repository;
use simplelog::{info, paris, warn};
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
    let workdir = get_workdir(pc)?;
    let repo_files = walk_repo(workdir.as_ref())?;
    let package_lookup = get_package_lookup(pc);

    for repo_file in &repo_files {
        let path = match package_lookup.get(repo_file.unresolved()) {
            Some(value) => value,
            None => continue,
        };
        if let Some(value) = path {
            fs::copy(repo_file.resolved(), value.resolved())?;
            info!(
                "<bold>Copied:</> <cyan>{}</> from local repository.",
                repo_file.unresolved().display(),
            );
        } else {
            let expanded = match path::expand(repo_file.unresolved()) {
                Ok(expanded) => expanded,
                Err(_) => continue,
            };
            if let Some(p) = expanded.parent() {
                fs::create_dir_all(p)?;
            }
            fs::copy(repo_file.resolved(), expanded)?;
            info!(
                "<bold>Copied:</> <cyan>{}</> from local repository.",
                repo_file.unresolved().display(),
            );
        }
    }

    Ok(())
}

fn apply_one(pc: &PathConfig, package: &str) -> Result<()> {
    let workdir = get_workdir(pc)?;

    if let Some(paths) = pc.config.packages.get(package) {
        for path in paths {
            let mut repo_file = workdir.resolved().to_path_buf();
            repo_file.push(path);
            if !repo_file.exists() {
                continue;
            }
            let expanded = match path::expand(path) {
                Ok(expanded) => expanded,
                Err(_) => continue,
            };
            if let Some(p) = expanded.parent() {
                fs::create_dir_all(p)?;
            }
            fs::copy(repo_file, expanded)?;
            info!(
                "<bold>Copied:</> <cyan>{}</> from local repository.",
                path.display()
            );
        }
    } else {
        warn!("Could not find package <cyan>{}</> in config.", package);
    }

    Ok(())
}

pub fn apply(pc: &PathConfig, package: Option<&str>) -> Result<()> {
    if let Some(package) = package {
        apply_one(pc, package)
    } else {
        apply_all(pc)
    }
}

// ========================================
// Staging
// ========================================

pub fn stage(pc: &PathConfig) -> Result<()> {
    let workdir = get_workdir(pc)?;
    let repo_files = walk_repo(workdir.as_ref())?;
    let package_lookup = get_package_lookup(pc);

    // Find all files in our repository that are no longer being referenced in
    // our primary config file. They should be removed from the repository.
    for repo_file in &repo_files {
        if !package_lookup.contains_key(repo_file.unresolved()) {
            fs::remove_file(repo_file.resolved())?;
        }
        if let Some(p) = repo_file.resolved().parent() {
            if p.read_dir()?.next().is_none() {
                fs::remove_dir(p)?;
            }
        }
    }

    // Find all resolvable files in our primary config and copy them into the
    // repository.
    for (key, value) in &package_lookup {
        if let Some(value) = value {
            let mut copy = workdir.resolved().to_path_buf();
            copy.push(key);
            if let Some(p) = copy.parent() {
                fs::create_dir_all(p)?;
            }
            fs::copy(value.resolved(), copy)?;
        }
    }

    info!(
        "<bold>Staged:</> View using `<italic>git -C <cyan>{}</> <italic>status</>`.",
        &pc.config.repos.local.display()
    );

    Ok(())
}

// ========================================
// Utility
// ========================================

fn get_workdir(pc: &PathConfig) -> Result<ResPathBuf> {
    let workdir = path::resolve(&pc.config.repos.local)?;
    if let Ok(repo) = Repository::open(workdir.resolved()) {
        if repo.workdir().is_some() {
            Ok(workdir)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Local repository is --bare.",
            ))?
        }
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Local repository not found.",
        ))?
    }
}

fn recursive_walk_repo(root: &Path, path: &Path) -> Result<Vec<ResPathBuf>> {
    let mut seen = Vec::new();
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let nested = entry?.path();
            if nested.is_dir() {
                if nested.ends_with(".git") {
                    continue;
                }
                let nested = recursive_walk_repo(root, &nested)?;
                seen.extend_from_slice(&nested);
            } else {
                let relative = nested
                    .strip_prefix(root)
                    .expect("Relative git file could not be stripped properly.");
                seen.push(ResPathBuf::new(&nested, relative)?);
            }
        }
    }
    Ok(seen)
}

fn walk_repo(root: &Path) -> Result<Vec<ResPathBuf>> {
    recursive_walk_repo(root, root)
}

fn get_package_lookup(pc: &PathConfig) -> HashMap<PathBuf, Option<ResPathBuf>> {
    let mut seen = HashMap::new();
    for (_, packages) in &pc.config.packages {
        for path in packages {
            if let Ok(resolved) = path::resolve(path) {
                seen.insert(path.to_path_buf(), Some(resolved));
            } else {
                seen.insert(path.to_path_buf(), None);
            }
        }
    }
    seen
}

// ========================================
// Tests
// ========================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config, path};
    use git2::Repository;
    use std::{env, fs::File, io::Write};
    use tempfile::TempDir;

    // Tests must be serial since we are updating our environment variables.
    use serial_test::serial;

    // Wrap functionality around this method to ensure the temporary directory
    // does not go out of scope before we are ready.
    fn build_home<T: Fn(&config::PathConfig, &Path)>(func: T) {
        let temp_dir = TempDir::new().unwrap();

        let mut home_dir = temp_dir.path().to_path_buf();
        home_dir.push("home/owner");
        fs::create_dir_all(&home_dir).unwrap();

        let mut homesync_yml = home_dir.to_path_buf();
        homesync_yml.push(".homesync.yml");
        File::create(&homesync_yml).unwrap();
        path::resolve(&homesync_yml).unwrap();

        let mut config_homesync_yml = home_dir.to_path_buf();
        config_homesync_yml.push(".config/homesync");
        fs::create_dir_all(&config_homesync_yml).unwrap();
        config_homesync_yml.push("homesync.yml");
        File::create(&config_homesync_yml).unwrap();

        env::set_var("HOME", &home_dir);
        env::set_var("XDG_CONFIG_HOME", "");

        let template = path::resolve(Path::new("examples/template.yml")).unwrap();
        let config = config::load(&vec![template]).unwrap();

        func(&config, &home_dir);
    }

    fn build_repo(pc: &PathConfig) -> PathBuf {
        let repo_dir = path::expand(&pc.config.repos.local).unwrap();
        Repository::init(&repo_dir).unwrap();

        let mut path = repo_dir.to_path_buf();
        path.push("b");
        fs::create_dir(&path).unwrap();
        path.pop();
        path.push("a");
        File::create(&path).unwrap();
        path.pop();
        path.push("b/c");
        File::create(&path).unwrap();

        repo_dir
    }

    #[test]
    #[serial]
    fn walk_repo() {
        build_home(|pc, _home_dir| {
            let repo_dir = build_repo(pc);
            let walked = super::walk_repo(&repo_dir).unwrap();
            let mut walked: Vec<PathBuf> = walked
                .iter()
                .map(|w| w.unresolved().to_path_buf())
                .collect();
            walked.sort();
            assert_eq!(walked, vec![PathBuf::from("a"), PathBuf::from("b/c")]);
        });
    }

    #[test]
    #[serial]
    fn package_lookup() {
        build_home(|pc, _home_dir| {
            let lookup = super::get_package_lookup(pc);
            assert_eq!(lookup.len(), 4);
            assert_eq!(
                lookup
                    .iter()
                    .filter(|(_, v)| v.is_some())
                    .collect::<HashMap<_, _>>()
                    .len(),
                2
            );
            assert_eq!(
                lookup
                    .iter()
                    .filter(|(_, v)| v.is_none())
                    .collect::<HashMap<_, _>>()
                    .len(),
                2
            );
        });
    }

    #[test]
    #[serial]
    fn apply_all() {
        build_home(|pc, home_dir| {
            let repo_dir = build_repo(pc);
            let targets = [".homesync.yml", ".config/homesync/homesync.yml"];

            for target in &targets {
                let mut repo_path = repo_dir.to_path_buf();
                repo_path.push(&format!("$HOME/{}", target));
                fs::create_dir_all(repo_path.parent().unwrap()).unwrap();
                let mut file = File::create(&repo_path).unwrap();
                file.write_all(b"Hello, world!").unwrap();
            }

            super::apply_all(pc).expect("Could not apply packages");

            for target in &targets {
                let mut home_path = home_dir.to_path_buf();
                home_path.push(target);
                let contents = fs::read_to_string(&home_path).unwrap();
                assert_eq!(contents, "Hello, world!");
            }
        });
    }

    #[test]
    #[serial]
    fn apply_one() {
        build_home(|pc, home_dir| {
            let repo_dir = build_repo(pc);
            let targets = [".homesync.yml", ".config/homesync/homesync.yml"];

            for target in &targets {
                let mut repo_path = repo_dir.to_path_buf();
                repo_path.push(&format!("$HOME/{}", target));
                fs::create_dir_all(repo_path.parent().unwrap()).unwrap();
                let mut file = File::create(&repo_path).unwrap();
                file.write_all(b"Hello, world!").unwrap();
            }

            super::apply_one(pc, "homesync").expect("Could not apply `homesync`");

            for target in &targets {
                let mut home_path = home_dir.to_path_buf();
                home_path.push(target);
                let contents = fs::read_to_string(&home_path).unwrap();
                assert_eq!(contents, "Hello, world!");
            }
        });
    }

    #[test]
    #[serial]
    fn stage() {
        build_home(|pc, _home_dir| {
            let repo_dir = build_repo(pc);
            super::stage(pc).expect("Could not stage files.");
            // Copied over the files in $HOME that exist, and deleted files that
            // were previously defined but not referenced in the config.
            let walked = super::walk_repo(&repo_dir).unwrap();
            let mut walked: Vec<PathBuf> = walked
                .iter()
                .map(|w| w.unresolved().to_path_buf())
                .collect();
            walked.sort();
            assert_eq!(
                walked,
                vec![
                    PathBuf::from("$HOME/.config/homesync/homesync.yml"),
                    PathBuf::from("$HOME/.homesync.yml"),
                ]
            );
        });
    }
}
