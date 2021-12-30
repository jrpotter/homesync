use super::config;
use super::config::PathConfig;
use super::path;
use super::path::{NormalPathBuf, Normalize};
use notify::{RecommendedWatcher, Watcher};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

// TODO(jrpotter): Add logging.
// TODO(jrpotter): Add pid file to only allow one daemon at a time.

// ========================================
// State
// ========================================

struct WatchState {
    // Paths that we were not able to watch properly but could potentially do so
    // in the future. These include paths that did not exist at the time of
    // canonicalization or did not have environment variables defined that may
    // be defined later on.
    pending: HashSet<PathBuf>,
    // Paths that we are currently watching.
    watching: HashSet<NormalPathBuf>,
    // Paths that are not valid and will never become valid. These may include
    // paths that include prefixes or refer to directories that could never be
    // reached (e.g. parent of root).
    invalid: HashSet<PathBuf>,
}

impl WatchState {
    pub fn new(config: &PathConfig) -> notify::Result<Self> {
        let mut pending: HashSet<PathBuf> = HashSet::new();
        let mut watching: HashSet<NormalPathBuf> = HashSet::new();
        watching.insert(config.0.clone());
        // We try and resolve our configuration again here. We want to
        // specifically track any new configs that may pop up with higher
        // priority.
        for path in config::default_paths() {
            match path::normalize(&path)? {
                // TODO(jrpotter): Check if the path can be canonicalized.
                Normalize::Done(p) => watching.insert(p),
                Normalize::Pending => pending.insert(path),
            };
        }
        Ok(WatchState {
            pending,
            watching,
            invalid: HashSet::new(),
        })
    }
}

// ========================================
// Daemon
// ========================================

fn reload_config(config: &PathConfig) -> notify::Result<WatchState> {
    let state = WatchState::new(config)?;
    Ok(state)
}

pub fn launch(config: PathConfig) -> notify::Result<()> {
    let (tx, rx) = channel();
    // Create a "debounced" watcher. Events will not trigger until after the
    // specified duration has passed with no additional changes.
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(2))?;
    // for (_, package) in &config.1.packages {
    //     for path in &package.configs {
    //         match normalize_path(&path) {
    //             // `notify-rs` is not able to handle files that do not exist and
    //             // are then created. This is handled internally by the library
    //             // via the `fs::canonicalize` which fails on missing paths. So
    //             // track which paths end up missing and apply polling on them.
    //             Ok(normalized) => match watcher.watch(&normalized, RecursiveMode::NonRecursive) {
    //                 Ok(_) => {
    //                     tracked_paths.insert(normalized);
    //                 }
    //                 Err(notify::Error::PathNotFound) => {
    //                     missing_paths.insert(normalized);
    //                 }
    //                 Err(e) => return Err(e),
    //             },
    //             // TODO(jrpotter): Retry even in cases where environment
    //             // variables are not defined.
    //             Err(e) => eprintln!("{}", e),
    //         };
    //     }
    // }
    // This is a simple loop, but you may want to use more complex logic here,
    // for example to handle I/O.
    loop {
        match rx.recv() {
            Ok(event) => println!("{:?}", event),
            Err(e) => println!("watch error: {:?}", e),
        }
    }
}
