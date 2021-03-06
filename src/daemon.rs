//! Utilites around launching a filewatcher daemon service.
//!
//! This is intended to be the primary use case of Homesync once stable. The
//! daemon is responsible for loading in the homesync config (reloading as it
//! changes) and monitoring any files/file paths specified within. On changes,
//! it will automatically stage the files to the local repository.

use super::{config, config::PathConfig, copy, path, path::ResPathBuf};
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use simplelog::{error, paris, trace, warn};
use std::{
    collections::HashSet,
    error::Error,
    path::PathBuf,
    sync::mpsc::{channel, Receiver, Sender, TryRecvError},
    thread,
    time::Duration,
};

// TODO(jrpotter): Add pid file to only allow one daemon at a time.

// ========================================
// Polling
// ========================================

enum PollEvent {
    Pending(PathBuf),
    Clear,
}

fn resolve_pending(tx: &Sender<DebouncedEvent>, pending: &HashSet<PathBuf>) -> Vec<PathBuf> {
    let mut to_remove = vec![];
    for path in pending {
        match path::soft_resolve(&path) {
            Ok(Some(resolved)) => {
                to_remove.push(path.clone());
                tx.send(DebouncedEvent::Create(resolved.into()))
                    .expect("File watcher channel closed.");
            }
            Ok(None) => (),
            Err(e) => {
                to_remove.push(path.clone());
                error!(
                    "Encountered unexpected error {} when processing path {}",
                    e,
                    path.display()
                )
            }
        }
    }
    to_remove
}

fn poll_pending(tx: Sender<DebouncedEvent>, rx: Receiver<PollEvent>, freq_secs: u64) {
    let mut pending = HashSet::new();
    loop {
        match rx.try_recv() {
            Ok(PollEvent::Pending(path)) => {
                pending.insert(path);
            }
            Ok(PollEvent::Clear) => pending.clear(),
            Err(TryRecvError::Empty) => {
                resolve_pending(&tx, &pending).iter().for_each(|r| {
                    pending.remove(r);
                });
                thread::sleep(Duration::from_secs(freq_secs));
            }
            Err(TryRecvError::Disconnected) => panic!("Polling channel closed."),
        }
    }
}

// ========================================
// File Watcher
// ========================================

struct WatchState<'a> {
    poll_tx: Sender<PollEvent>,
    watcher: &'a mut RecommendedWatcher,
    watching: HashSet<ResPathBuf>,
}

impl<'a> WatchState<'a> {
    pub fn new(
        poll_tx: Sender<PollEvent>,
        watcher: &'a mut RecommendedWatcher,
    ) -> notify::Result<Self> {
        Ok(WatchState {
            poll_tx,
            watcher,
            watching: HashSet::new(),
        })
    }

    fn send_poll(&self, event: PollEvent) {
        self.poll_tx.send(event).expect("Polling channel closed.");
    }

    fn watch(&mut self, path: ResPathBuf) {
        match self.watcher.watch(&path, RecursiveMode::NonRecursive) {
            Ok(()) => {
                self.watching.insert(path);
            }
            Err(e) => {
                error!(
                    "Encountered unexpected error {} when watching path {}",
                    e,
                    path.unresolved().display()
                );
            }
        }
    }

    /// Reads in the new path config, updating all watched and pending files
    /// according to the packages in the specified config.
    pub fn update(&mut self, pc: &PathConfig) {
        self.send_poll(PollEvent::Clear);
        for path in &self.watching {
            match self.watcher.unwatch(&path) {
                Ok(()) => (),
                Err(e) => {
                    error!(
                        "Encountered unexpected error {} when unwatching path {}",
                        e,
                        path.unresolved().display()
                    );
                }
            }
        }
        self.watching.clear();
        for (_, packages) in &pc.config.packages {
            for path in packages {
                match path::soft_resolve(&path) {
                    Ok(None) => self.send_poll(PollEvent::Pending(path.clone())),
                    Ok(Some(n)) => self.watch(n),
                    Err(_) => (),
                }
            }
        }
    }
}

// ========================================
// Daemon
// ========================================

/// Launches a daemon service that monitors changes to files specified in the
/// config and stages them for changes in the local repository.
///
/// Warning! This service is still under development.
pub fn launch(mut pc: PathConfig, freq_secs: u64) -> Result<(), Box<dyn Error>> {
    let (poll_tx, poll_rx) = channel();
    let (watch_tx, watch_rx) = channel();
    let watch_tx1 = watch_tx.clone();
    // `notify-rs` internally uses `fs::canonicalize` on each path we try to
    // watch, but this fails if no file exists at the given path. In these
    // cases, we rely on a basic polling strategy to check if the files ever
    // come into existence.
    thread::spawn(move || poll_pending(watch_tx, poll_rx, freq_secs));
    // Track our original config file separately from the other files that may
    // be defined in the config. We want to make sure we're always alerted on
    // changes to it for hot reloading purposes, and not worry that our wrapper
    // will ever clear it from its watch state.
    let mut watcher: RecommendedWatcher = Watcher::new(watch_tx1, Duration::from_secs(freq_secs))?;
    watcher.watch(&pc.homesync_yml, RecursiveMode::NonRecursive)?;
    let mut state = WatchState::new(poll_tx, &mut watcher)?;
    state.update(&pc);
    loop {
        copy::stage(&pc)?;
        // Received paths should always be fully resolved.
        match watch_rx.recv() {
            Ok(DebouncedEvent::NoticeWrite(p)) => {
                trace!("<bold>Noticed:</> Write at <cyan>{}</>", p.display());
            }
            Ok(DebouncedEvent::NoticeRemove(p)) => {
                trace!("<bold>Noticed:</> Removal of <cyan>{}</>", p.display());
            }
            Ok(DebouncedEvent::Create(p)) => {
                trace!("<bold>Created:</> <cyan>{}</>", p.display());
                if pc.homesync_yml == p {
                    pc = config::reload(&pc)?;
                    state.update(&pc);
                }
            }
            Ok(DebouncedEvent::Write(p)) => {
                trace!("<bold>Wrote:</> <cyan>{}</>", p.display());
                if pc.homesync_yml == p {
                    pc = config::reload(&pc)?;
                    state.update(&pc);
                }
            }
            // Do not try reloading our primary config in any of the following
            // cases since it may lead to undesired behavior. If our config has
            // e.g. been removed, let's just keep using what we have in memory
            // in the chance it may be added back.
            Ok(DebouncedEvent::Chmod(p)) => {
                trace!("<bold>Chmod:</> <cyan>{}</>", p.display());
            }
            Ok(DebouncedEvent::Remove(p)) => {
                if pc.homesync_yml == p {
                    warn!(
                        "<bold>Removed:</> Primary config <cyan>{}</>. Continuing to use last \
                        loaded state",
                        p.display()
                    );
                } else {
                    trace!("<bold>Removed:</> <cyan>{}</>", p.display());
                }
            }
            Ok(DebouncedEvent::Rename(src, dst)) => {
                if pc.homesync_yml == src && pc.homesync_yml != dst {
                    warn!(
                        "<bold>Renamed:</> Primary config <cyan>{}</>. Continuing from last \
                        loaded state",
                        src.display()
                    );
                } else {
                    trace!(
                        "<bold>Renamed:</> <cyan>{}</> to <cyan>{}</>.",
                        src.display(),
                        dst.display()
                    )
                }
            }
            Ok(DebouncedEvent::Rescan) => {
                trace!("Rescanning");
            }
            Ok(DebouncedEvent::Error(e, path)) => {
                warn!(
                    "<bold>Unexpected:</> Error {} at <cyan>{}</>",
                    e,
                    path.unwrap_or_else(|| PathBuf::from("N/A")).display()
                );
            }
            Err(e) => {
                error!("Watch error: {:?}", e);
            }
        }
    }
}
