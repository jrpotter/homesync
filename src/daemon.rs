use super::config::PathConfig;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use std::collections::HashSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;

// TODO(jrpotter): Add logging.
// TODO(jrpotter): Add pid file to only allow one daemon at a time.

// Find environment variables found within the argument and expand them if
// possible.
//
// Returns `None` in the case an environment variable present within the
// argument is not defined.
fn expand_str(s: &OsStr) -> Option<OsString> {
    let re = Regex::new(r"\$(?P<env>[[:alnum:]]+)").unwrap();
    let lossy = s.to_string_lossy();
    let mut path = lossy.clone().to_string();
    for caps in re.captures_iter(&lossy) {
        let evar = env::var(&caps["env"]).ok()?;
        path = path.replace(&format!("${}", &caps["env"]), &evar);
    }
    Some(path.into())
}

// Normalizes the provided path, returning a new instance.
//
// There current doesn't exist a method that yields some canonical path for
// files that do not exist (at least in the parts of the standard library I've
// looked in). We create a consistent view of every path so as to avoid
// watching the same path multiple times, which would duplicate messages on
// changes.
fn normalize_path(path: &Path) -> io::Result<PathBuf> {
    let mut pb = env::current_dir()?;
    for comp in path.components() {
        match comp {
            Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "We do not currently support Windows.",
                ))
            }
            Component::RootDir => {
                pb.clear();
                pb.push(Component::RootDir)
            }
            Component::CurDir => (), // Make no changes.
            Component::ParentDir => {
                if !pb.pop() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Cannot take parent of root.",
                    ));
                }
            }
            Component::Normal(c) => match expand_str(c) {
                Some(c) => pb.push(Component::Normal(&c)),
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Cannot find path {}", path.display().to_string()),
                    ))
                }
            },
        }
    }
    Ok(pb)
}

/// Launches the daemon instance.
///
/// This method also spawns an additional thread responsible for handling
/// polling of files that are specified in the config but do not exist.
pub fn launch(config: PathConfig) -> notify::Result<()> {
    let (tx, rx) = channel();
    // Create a "debounced" watcher. Events will not trigger until after the
    // specified duration has passed with no additional changes.
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(2))?;
    // Take in the `homesync` configuration and add watchers to all paths
    // specified in the configuration. `watch` appends the path onto a list so
    // avoid tracking the same path multiple times.
    let mut tracked_paths: HashSet<PathBuf> = HashSet::new();
    // TODO(jrpotter): Spawn thread responsible for polling for missing files.
    let mut missing_paths: HashSet<PathBuf> = HashSet::new();
    for (_, package) in &config.1.packages {
        for path in &package.configs {
            match normalize_path(&path) {
                // `notify-rs` is not able to handle files that do not exist and
                // are then created. This is handled internally by the library
                // via the `fs::canonicalize` which fails on missing paths. So
                // track which paths end up missing and apply polling on them.
                Ok(normalized) => match watcher.watch(&normalized, RecursiveMode::NonRecursive) {
                    Ok(_) => {
                        tracked_paths.insert(normalized);
                    }
                    Err(notify::Error::PathNotFound) => {
                        missing_paths.insert(normalized);
                    }
                    Err(e) => return Err(e),
                },
                // TODO(jrpotter): Retry even in cases where environment
                // variables are not defined.
                Err(e) => eprintln!("{}", e),
            };
        }
    }
    // This is a simple loop, but you may want to use more complex logic here,
    // for example to handle I/O.
    loop {
        match rx.recv() {
            Ok(event) => println!("{:?}", event),
            Err(e) => println!("watch error: {:?}", e),
        }
    }
}
