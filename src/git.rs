use super::{config::PathConfig, path};
use git2::{IndexAddOption, ObjectType, Remote, Repository, Signature, StashFlags};
use path::ResPathBuf;
use simplelog::{info, paris};
use std::{
    collections::HashSet,
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

// ========================================
// Syncing
// ========================================

fn get_remote<'repo>(pc: &PathConfig, repo: &'repo Repository) -> Result<Remote<'repo>> {
    // Sets a new remote if it does not yet exist.
    repo.remote_set_url(&pc.config.remote.name, &pc.config.remote.url.to_string())?;
    // We could go with "*" instead of referencing the one branch, but let's be
    // specific for the time being.
    // https://git-scm.com/book/en/v2/Git-Internals-The-Refspec
    repo.remote_add_fetch(
        &pc.config.remote.name,
        &format!(
            "+refs/heads/{branch}:refs/remotes/origin/{branch}",
            branch = pc.config.remote.branch
        ),
    )?;
    Ok(repo.find_remote(&pc.config.remote.name)?)
}

pub fn push(pc: &PathConfig, repo: &mut Repository) -> Result<()> {
    repo.workdir().ok_or(Error::NotWorkingRepo)?;
    // Switch to the new branch we want to work on. If the branch does not
    // exist, `set_head` will point to an unborn branch.
    // https://git-scm.com/docs/git-check-ref-format.
    repo.set_head(&format!("refs/heads/{}", pc.config.remote.branch))?;
    // Establish our remote. If the remote already exists, re-configure it
    // blindly to point to the appropriate url. Our results should now exist
    // in a branch called `remotes/origin/<branch>`.
    // https://git-scm.com/book/it/v2/Git-Basics-Working-with-Remotes
    // TODO(jrpotter): Rebase against the remote.
    let mut remote = get_remote(&pc, &repo)?;
    remote.fetch(&[&pc.config.remote.branch], None, None)?;
    // Find the latest commit on our current branch. This could be empty if just
    // having initialized the repository.
    let parent_commit = match repo.head() {
        Ok(head) => {
            let obj = head
                .resolve()?
                .peel(ObjectType::Commit)?
                .into_commit()
                .map_err(|_| git2::Error::from_str("Couldn't find commit"))?;
            vec![obj]
        }
        // An unborn branch error is fired when first initializing the
        // repository. Our first commit will create the branch.
        Err(e) => match e.code() {
            git2::ErrorCode::UnbornBranch => vec![],
            _ => Err(e)?,
        },
    };
    // The index corresponds to our staging area. We add all files and write out
    // to a tree. The resulting tree can be found using `git ls-tree <oid>`.
    // https://git-scm.com/book/en/v2/Git-Internals-Git-Objects
    let mut index = repo.index()?;
    index.add_all(["."].iter(), IndexAddOption::DEFAULT, None)?;
    let index_oid = index.write_tree()?;
    let index_tree = repo.find_tree(index_oid)?;
    // Stash any of our changes. We will first fetch from the remote and then
    // apply our changes on top of it.
    // TODO(jrpotter): Add user and email to config. Remove init comamnd.
    // TODO(jrpotter): Cannot stash changes with no initial commit.
    let signature = Signature::now("homesync", "robot@homesync.org")?;
    let commit_oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        // TODO(jrpotter): See how many previous pushes were made.
        "homesync push",
        &index_tree,
        // iter/collect to collect an array of references.
        &parent_commit.iter().collect::<Vec<_>>()[..],
    )?;
    let _commit = repo.find_commit(commit_oid)?;
    Ok(())
}
