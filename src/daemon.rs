use super::config;
use super::config::PathConfig;
use super::path;
use super::path::ResPathBuf;
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::error::Error;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

// TODO(jrpotter): Add logging.
// TODO(jrpotter): Add pid file to only allow one daemon at a time.
// TODO(jrpotter): Sync files to local git repository.

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
                eprintln!(
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
                eprintln!(
                    "Encountered unexpected error {} when watching path {}",
                    e,
                    path.unresolved().display()
                );
            }
        }
    }

    /// Reads in the new path config, updating all watched and pending files
    /// according to the packages in the specified config.
    pub fn update(&mut self, config: &PathConfig) {
        self.send_poll(PollEvent::Clear);
        for path in &self.watching {
            match self.watcher.unwatch(&path) {
                Ok(()) => (),
                Err(e) => {
                    eprintln!(
                        "Encountered unexpected error {} when unwatching path {}",
                        e,
                        path.unresolved().display()
                    );
                }
            }
        }
        self.watching.clear();
        for (_, package) in &config.1.packages {
            for path in &package.configs {
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

pub fn launch(mut config: PathConfig, freq_secs: u64) -> Result<(), Box<dyn Error>> {
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
    watcher.watch(&config.0, RecursiveMode::NonRecursive)?;
    let mut state = WatchState::new(poll_tx, &mut watcher)?;
    state.update(&config);
    loop {
        // Received paths should always be the fully resolved ones so safe to
        // compare against our current config path.
        match watch_rx.recv() {
            Ok(DebouncedEvent::NoticeWrite(_)) => {
                // Intentionally ignore in favor of stronger signals.
            }
            Ok(DebouncedEvent::NoticeRemove(_)) => {
                // Intentionally ignore in favor of stronger signals.
            }
            Ok(DebouncedEvent::Create(p)) => {
                if config.0 == p {
                    config = config::reload(&config)?;
                    state.update(&config);
                }
                println!("Create {}", p.display());
            }
            Ok(DebouncedEvent::Write(p)) => {
                if config.0 == p {
                    config = config::reload(&config)?;
                    state.update(&config);
                }
                println!("Write {}", p.display());
            }
            // Do not try reloading our primary config in any of the following
            // cases since it may lead to undesired behavior. If our config has
            // e.g. been removed, let's just keep using what we have in memory
            // in the chance it may be added back.
            Ok(DebouncedEvent::Chmod(p)) => {
                println!("Chmod {}", p.display());
            }
            Ok(DebouncedEvent::Remove(p)) => {
                println!("Remove {}", p.display());
            }
            Ok(DebouncedEvent::Rename(src, dst)) => {
                println!("Rename {} {}", src.display(), dst.display())
            }
            Ok(DebouncedEvent::Rescan) => {
                println!("Rescanning");
            }
            Ok(DebouncedEvent::Error(e, _maybe_path)) => {
                println!("Error {}", e);
            }
            Err(e) => {
                println!("watch error: {:?}", e);
            }
        }
    }
}
