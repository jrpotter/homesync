use super::{config::PathConfig, path};
use git2::Repository;
use path::ResPathBuf;
use simplelog::{info, paris};
use std::{
    collections::HashSet,
    env::VarError,
    error, fmt, fs, io,
    path::{Path, PathBuf},
    result,
};

// All git error codes.
// TODO(jrpotter): Remove these once done needing to reference them.
// git2::ErrorCode::GenericError => panic!("generic"),
// git2::ErrorCode::NotFound => panic!("not_found"),
// git2::ErrorCode::Exists => panic!("exists"),
// git2::ErrorCode::Ambiguous => panic!("ambiguous"),
// git2::ErrorCode::BufSize => panic!("buf_size"),
// git2::ErrorCode::User => panic!("user"),
// git2::ErrorCode::BareRepo => panic!("bare_repo"),
// git2::ErrorCode::UnbornBranch => panic!("unborn_branch"),
// git2::ErrorCode::Unmerged => panic!("unmerged"),
// git2::ErrorCode::NotFastForward => panic!("not_fast_forward"),
// git2::ErrorCode::InvalidSpec => panic!("invalid_spec"),
// git2::ErrorCode::Conflict => panic!("conflict"),
// git2::ErrorCode::Locked => panic!("locked"),
// git2::ErrorCode::Modified => panic!("modified"),
// git2::ErrorCode::Auth => panic!("auth"),
// git2::ErrorCode::Certificate => panic!("certificate"),
// git2::ErrorCode::Applied => panic!("applied"),
// git2::ErrorCode::Peel => panic!("peel"),
// git2::ErrorCode::Eof => panic!("eof"),
// git2::ErrorCode::Invalid => panic!("invalid"),
// git2::ErrorCode::Uncommitted => panic!("uncommitted"),
// git2::ErrorCode::Directory => panic!("directory"),
// git2::ErrorCode::MergeConflict => panic!("merge_conflict"),
// git2::ErrorCode::HashsumMismatch => panic!("hashsum_mismatch"),
// git2::ErrorCode::IndexDirty => panic!("index_dirty"),
// git2::ErrorCode::ApplyFail => panic!("apply_fail"),

// ========================================
// Error
// ========================================

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    GitError(git2::Error),
    IOError(io::Error),
    NotHomesyncRepo,
    NotWorkingRepo,
    VarError(VarError),
}

impl From<git2::Error> for Error {
    fn from(err: git2::Error) -> Error {
        Error::GitError(err)
    }
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
            Error::GitError(e) => write!(f, "{}", e),
            Error::IOError(e) => write!(f, "{}", e),
            Error::NotHomesyncRepo => write!(
                f,
                "Local repository is not managed by `homesync`. Missing `.homesync` sentinel file."
            ),
            Error::NotWorkingRepo => write!(
                f,
                "Local repository should be a working directory. Did you manually initialize with `--bare`?"
            ),
            Error::VarError(e) => write!(f, "{}", e),
        }
    }
}

impl error::Error for Error {}

// ========================================
// Initialization
// ========================================

/// Sets up a local github repository all configuration files will be synced to.
/// If there does not exist a local repository at the requested location, we
/// attempt to make it.
///
/// NOTE! This does not perform any syncing between local and remote. In fact,
/// this method does not perform any validation on remote at all.
pub fn init(pc: &PathConfig) -> Result<Repository> {
    // Permit the use of environment variables within the local configuration
    // path (e.g. `$HOME`). Unlike with resolution, we want to fail if the
    // environment variable is not defined.
    let expanded = path::expand(&pc.config.local)?;
    // Attempt to open the local path as a git repository if possible. The
    // `NotFound` error is thrown if:
    //
    // - the directory does not exist.
    // - the directory is not git-initialized (i.e. has a valid `.git`
    //   subfolder).
    // - the directory does not have appropriate permissions.
    let local = match Repository::open(&expanded) {
        Ok(repo) => Some(repo),
        Err(e) => match e.code() {
            git2::ErrorCode::NotFound => None,
            _ => Err(e)?,
        },
    };
    // Setup a sentinel file in the given repository. This is used for both
    // ensuring any remote repositories are already managed by homesync and for
    // storing any persisted configurations.
    let mut sentinel = PathBuf::from(&expanded);
    sentinel.push(".homesync");
    match local {
        Some(repo) => {
            // Verify the given repository has a homesync sentinel file.
            match path::validate_is_file(&sentinel) {
                Ok(_) => (),
                Err(_) => Err(Error::NotHomesyncRepo)?,
            };
            Ok(repo)
        }
        // If no local repository exists, we choose to just always initialize a
        // new one instead of cloning from remote. Cloning has a separate set of
        // issues that we need to resolve anyways (e.g. setting remote, pulling,
        // managing possible merge conflicts, etc.).
        None => {
            info!(
                "Creating new homesync repository at <green>{}</>.",
                pc.config.local.display()
            );
            let repo = Repository::init(&expanded)?;
            fs::File::create(sentinel)?;
            Ok(repo)
        }
    }
}

// ========================================
// Application
// ========================================

fn find_repo_files(path: &Path) -> Result<Vec<ResPathBuf>> {
    let mut seen = Vec::new();
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let nested = entry?.path();
            if nested.is_dir() {
                if nested.ends_with(".git") {
                    continue;
                }
                let nested = find_repo_files(&nested)?;
                seen.extend_from_slice(&nested);
            } else if !nested.ends_with(".homesync") {
                seen.push(ResPathBuf::new(&nested)?);
            }
        }
    }
    Ok(seen)
}

fn find_package_files(pc: &PathConfig) -> Vec<ResPathBuf> {
    let mut seen = Vec::new();
    for (_, package) in &pc.config.packages {
        for path in &package.configs {
            if let Ok(resolved) = path::resolve(path) {
                seen.push(resolved);
            }
        }
    }
    seen
}

pub fn apply(pc: &PathConfig, repo: &Repository) -> Result<()> {
    let workdir = repo.workdir().ok_or(Error::NotWorkingRepo)?;
    let repo_files = find_repo_files(&workdir)?;
    let package_files = find_package_files(&pc);
    // Find all files in our repository that are no longer being referenced in
    // our primary config file. They should be removed from the repository.
    let lookup_files: HashSet<PathBuf> = package_files
        .iter()
        .map(|m| m.unresolved().to_path_buf())
        .collect();
    for repo_file in &repo_files {
        let relative = repo_file
            .resolved()
            .strip_prefix(workdir)
            .expect("Relative git file could not be stripped properly.")
            .to_path_buf();
        if !lookup_files.contains(&relative) {
            fs::remove_file(repo_file)?;
        }
        if let Some(p) = repo_file.resolved().parent() {
            if p.read_dir()?.next().is_none() {
                fs::remove_dir(p)?;
            }
        }
    }
    // Find all resolvable files in our primary config and copy them into the
    // repository.
    for package_file in &package_files {
        let mut copy = workdir.to_path_buf();
        copy.push(package_file.unresolved());
        if let Some(p) = copy.parent() {
            fs::create_dir_all(p)?;
        }
        fs::copy(package_file.resolved(), copy)?;
    }
    Ok(())
}
