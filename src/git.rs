use super::{config::PathConfig, path};
use git2::{
    Branch, BranchType, Commit, Cred, DiffOptions, Direction, FetchOptions, Index, IndexAddOption,
    ObjectType, Remote, RemoteCallbacks, Repository, Signature,
};
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
    InvalidBareRepo,
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
            Error::InvalidBareRepo => write!(
                f,
                "Local repository should be a working directory. Did you manually initialize with \
                `--bare`?"
            ),
            Error::VarError(e) => write!(f, "{}", e),
        }
    }
}

impl error::Error for Error {}

// ========================================
// Initialization
// ========================================

fn clone(pc: &PathConfig, expanded: &Path) -> Result<Repository> {
    let fetch_options = get_fetch_options(&pc)?;
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch_options);

    Ok(builder.clone(&pc.config.repos.remote.url, &expanded)?)
}

/// Sets up a local github repository all configuration files will be synced to.
/// If there does not exist a local repository at the requested location, we
/// attempt to make it via cloning or initializing.
///
/// TODO(jrpotter): Setup a sentinel file in the given repository. This is used
/// for both ensuring any remote repositories are already managed by homesync
/// and for storing any persisted configurations.
pub fn init(pc: &PathConfig) -> Result<Repository> {
    // Permit the use of environment variables within the local configuration
    // path (e.g. `$HOME`). Unlike with resolution, we want to fail if the
    // environment variable is not defined.
    let expanded = path::expand(&pc.config.repos.local)?;
    // Attempt to open the local path as a git repository if possible. The
    // `NotFound` error is thrown if:
    //
    // - the directory does not exist.
    // - the directory is not git-initialized (i.e. has a valid `.git`
    //   subfolder).
    // - the directory does not have appropriate permissions.
    // - the remote repository is not found
    match Repository::open(&expanded) {
        Ok(repo) => {
            info!(
                "Opened local repository <green>{}</>.",
                &pc.config.repos.local.display()
            );
            Ok(repo)
        }
        Err(e) if e.code() == git2::ErrorCode::NotFound => match clone(&pc, &expanded) {
            Ok(repo) => {
                info!(
                    "Cloned remote repository <green>{}</>.",
                    &pc.config.repos.remote.url
                );
                Ok(repo)
            }
            Err(Error::GitError(e))
                if e.code() == git2::ErrorCode::Eof && e.class() == git2::ErrorClass::Ssh =>
            {
                info!(
                    "Creating local repository at <green>{}</>.",
                    pc.config.repos.local.display()
                );
                Ok(Repository::init(&expanded)?)
            }
            Err(e) => Err(e)?,
        },
        Err(e) => Err(e)?,
    }
}

// ========================================
// Staging
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
    for (_, packages) in &pc.config.packages {
        for path in packages {
            if let Ok(resolved) = path::resolve(path) {
                seen.push(resolved);
            }
        }
    }
    seen
}

pub fn stage(pc: &PathConfig, repo: &Repository) -> Result<()> {
    let workdir = validate_repo(&repo)?;
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
            .strip_prefix(&workdir)
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

/// Adds all files to our index.
///
/// Checks explicitly if any changes have been detected in the newly constructed
/// index. If not, return `None`.
pub fn index_add(repo: &Repository) -> Result<Option<Index>> {
    let mut index = repo.index()?;
    index.add_all(["."].iter(), IndexAddOption::DEFAULT, None)?;
    let diff_stats = repo
        .diff_index_to_workdir(
            Some(&index),
            Some(
                DiffOptions::new()
                    .include_untracked(true)
                    .include_unreadable(true),
            ),
        )?
        .stats()?;
    if diff_stats.files_changed() == 0
        && diff_stats.insertions() == 0
        && diff_stats.deletions() == 0
    {
        Ok(None)
    } else {
        Ok(Some(index))
    }
}

/// Create or retrieve the remote specified within our configuration.
///
/// This method also configures the fetchspec for the remote, explicitly mapping
/// the remote branch against our local one.
///
/// https://git-scm.com/book/en/v2/Git-Internals-The-Refspec
fn get_remote<'repo>(pc: &PathConfig, repo: &'repo Repository) -> Result<Remote<'repo>> {
    repo.remote_set_url(&pc.config.repos.remote.name, &pc.config.repos.remote.url)?;
    repo.remote_add_fetch(
        &pc.config.repos.remote.name,
        // We could go with "*" instead of {branch} for all remote branches.
        &format!(
            "+refs/heads/{}:refs/remotes/{}",
            pc.config.repos.remote.branch,
            pc.config.repos.remote.tracking_branch(),
        ),
    )?;
    Ok(repo.find_remote(&pc.config.repos.remote.name)?)
}

pub fn push(pc: &PathConfig, repo: &mut Repository) -> Result<()> {
    // First pull to make sure there are no conflicts when we push our changes.
    // This will also perform validation and construct our local and remote
    // environment.
    let _local_branch = pull(&pc, &repo)?;

    // The index corresponds to our staging area. We add all files and write out
    // to a tree. The resulting tree can be found using `git ls-tree <oid>`.
    // https://git-scm.com/book/en/v2/Git-Internals-Git-Objects
    let mut index = match index_add(&repo)? {
        Some(index) => index,
        None => {
            info!("Nothing to push. Have you run `homesync stage`?");
            return Ok(());
        }
    };
    // Retrieve the latest commit before writing to the object database.
    let parent_commit = get_commit(&repo)?;
    let index_oid = index.write_tree()?;
    let index_tree = repo.find_tree(index_oid)?;
    info!("Writing index to tree `{}`.", index_oid);

    // Commit our changes and push them to our remote.
    let refspec = format!("refs/heads/{}", &pc.config.repos.remote.branch);
    let signature = get_signature(&pc)?;
    repo.commit(
        Some(&refspec),
        &signature,
        &signature,
        // TODO(jrpotter): Come up with a more useful message.
        "homesync push",
        &index_tree,
        &[&parent_commit],
    )?;

    let mut remote = get_remote(&pc, &repo)?;
    remote.connect(Direction::Push)?;
    remote.push(&[&format!("{r}:{r}", r = refspec)], None)?;
    info!(
        "Pushed changes to remote `{}`.",
        pc.config.repos.remote.tracking_branch(),
    );

    Ok(())
}

pub fn pull<'repo>(pc: &PathConfig, repo: &'repo Repository) -> Result<Branch<'repo>> {
    validate_repo(&repo)?;

    // TODO(jrpotter): Configure our remote to point to the same URL mentioned
    // in our config.
    // TODO(jrpotter): If changes are available, need to stage them and then
    // reapply.

    // Establish our remote. If the remote already exists, re-configure it
    // blindly to point to the appropriate url. Our results should now exist
    // in a branch called `remotes/origin/<branch>`.
    // https://git-scm.com/book/it/v2/Git-Basics-Working-with-Remotes
    let mut remote = get_remote(&pc, &repo)?;
    let mut fetch_options = get_fetch_options(&pc)?;
    remote.fetch(
        &[&pc.config.repos.remote.branch],
        Some(&mut fetch_options),
        None,
    )?;
    let remote_branch_name = pc.config.repos.remote.tracking_branch();
    let remote_branch = repo.find_branch(&remote_branch_name, BranchType::Remote)?;
    info!("Fetched remote branch `{}`.", &remote_branch_name);

    // There are two cases we need to consider:
    //
    // 1. Our local branch actually exists, in which case there are commits
    // available. These should be rebased relative to remote (our upstream).
    // 2. Our repository has been initialized in an empty state. The branch we
    // are interested in is unborn, so we can just copy the branch from remote.
    let remote_ref = repo.reference_to_annotated_commit(remote_branch.get())?;
    if let Ok(local_branch) = repo.find_branch(&pc.config.repos.remote.branch, BranchType::Local) {
        let local_ref = repo.reference_to_annotated_commit(local_branch.get())?;
        let signature = get_signature(&pc)?;
        repo.rebase(Some(&local_ref), Some(&remote_ref), None, None)?
            .finish(Some(&signature))?;
        info!("Rebased local branch onto `{}`.", remote_branch_name);
        Ok(local_branch)
    } else {
        let local_branch =
            repo.branch_from_annotated_commit(&pc.config.repos.remote.branch, &remote_ref, false)?;
        info!("Created new local branch from `{}`.", remote_branch_name);
        Ok(local_branch)
    }
}

// ========================================
// Utility
// ========================================

/// Verify the repository we are working in supports the operations we want to
/// apply to it.
fn validate_repo(repo: &Repository) -> Result<PathBuf> {
    Ok(repo.workdir().ok_or(Error::InvalidBareRepo)?.to_path_buf())
}

/// Return the latest commit off of HEAD.
fn get_commit(repo: &Repository) -> Result<Commit> {
    Ok(repo
        .head()?
        .resolve()?
        .peel(ObjectType::Commit)?
        .into_commit()
        .map_err(|_| git2::Error::from_str("Couldn't find commit"))?)
}

/// Construct callbacks to supply authentication information on fetch/clone/etc.
fn get_fetch_options(pc: &PathConfig) -> Result<FetchOptions> {
    let public_path = match &pc.config.ssh.public {
        Some(p) => Some(path::resolve(p)?),
        None => None,
    };
    let private_path = path::resolve(&pc.config.ssh.private)?;

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, username_from_url, _allowed_types| {
        Cred::ssh_key(
            username_from_url.unwrap(),
            public_path.as_ref().map(|p| p.resolved().as_ref()),
            private_path.as_ref(),
            None,
        )
    });

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);
    Ok(fetch_options)
}

/// Generate a new signature at the current time.
fn get_signature(pc: &PathConfig) -> Result<Signature> {
    Ok(Signature::now(&pc.config.user.name, &pc.config.user.email)?)
}
