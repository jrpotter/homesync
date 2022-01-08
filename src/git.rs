use super::{config::PathConfig, path};
use git2::{
    BranchType, Commit, Cred, DiffOptions, Direction, FetchOptions, Index, IndexAddOption,
    ObjectType, PushOptions, Remote, RemoteCallbacks, Repository, Signature, StashApplyOptions,
    StashFlags,
};
use simplelog::{info, paris, warn};
use std::{
    collections::HashSet,
    env::VarError,
    error, fmt, io,
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
    let fetch_options = get_fetch_options(pc)?;
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch_options);

    Ok(builder.clone(&pc.config.repos.remote.url, &expanded)?)
}

// TODO(jrpotter): Setup a sentinel file in the given repository. This is used
// for both ensuring any remote repositories are already managed by homesync and
// for storing any persisted configurations.

/// Sets up a local github repository all configuration files will be synced to.
/// If there does not exist a local repository at the requested location, we
/// attempt to make it via cloning or initializing.
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
                "<bold>Opened:</> Local repository <cyan>{}</>.",
                &pc.config.repos.local.display()
            );
            Ok(repo)
        }
        Err(e) if e.code() == git2::ErrorCode::NotFound => match clone(pc, &expanded) {
            Ok(repo) => {
                info!(
                    "<bold>Cloned:</> Remote repository <cyan>{}</>.",
                    &pc.config.repos.remote.url
                );
                Ok(repo)
            }
            Err(Error::GitError(e))
                if e.class() == git2::ErrorClass::Ssh && e.code() == git2::ErrorCode::Eof =>
            {
                let repo = Repository::init(&expanded)?;
                info!(
                    "<bold>Created:</> Local repository <cyan>{}</>.",
                    pc.config.repos.local.display()
                );
                Ok(repo)
            }
            Err(e) => Err(e)?,
        },
        Err(e) => Err(e)?,
    }
}

// ========================================
// Syncing
// ========================================

pub fn push(pc: &PathConfig, repo: &mut Repository) -> Result<()> {
    // First pull to make sure there are no conflicts when we push our changes.
    // This will also perform validation and construct our local and remote
    // environment.
    pull(pc, repo)?;

    let refspec = format!("refs/heads/{}", &pc.config.repos.remote.branch);
    repo.set_head(&refspec)?;

    // The index corresponds to our staging area. We add all files and write out
    // to a tree. The resulting tree can be found using `git ls-tree <oid>`.
    // https://git-scm.com/book/en/v2/Git-Internals-Git-Objects
    let mut index = match index_with_all(repo)? {
        Some(index) => index,
        None => {
            warn!("Nothing to push. Have you run `homesync stage`?");
            return Ok(());
        }
    };
    let index_oid = index.write_tree()?;
    // Want to also reflect this change on the working directory.
    index.write()?;
    let index_tree = repo.find_tree(index_oid)?;
    info!("<bold>Wrote:</> Index to tree <cyan>{}</>.", index_oid);

    // Commit our changes and push them to our remote.
    // TODO(jrpotter): Come up with a more useful message.
    let signature = now_signature(pc)?;
    let message = "Automated homesync commit.";
    let commit_oid = if let Some(commit) = get_commit_at_head(repo) {
        repo.commit(
            Some(&refspec),
            &signature,
            &signature,
            message,
            &index_tree,
            &[&commit],
        )?
    } else {
        repo.commit(
            Some(&refspec),
            &signature,
            &signature,
            message,
            &index_tree,
            &[],
        )?
    };
    info!("<bold>Commited:</> <cyan>{}</>.", commit_oid);

    let mut remote = find_remote(pc, repo)?;
    let call_options = get_remote_callbacks(pc)?;
    remote.connect_auth(Direction::Push, Some(call_options), None)?;

    let mut push_options = get_push_options(pc)?;
    remote.push(&[&format!("{r}:{r}", r = refspec)], Some(&mut push_options))?;
    info!(
        "<bold>Pushed:</> Changes to remote <cyan>{}</>.",
        pc.config.repos.remote.tracking_branch(),
    );

    Ok(())
}

fn local_from_remote(pc: &PathConfig, repo: &Repository) -> Result<()> {
    fetch_remote(pc, repo)?;

    let tracking_branch = pc.config.repos.remote.tracking_branch();
    let remote_branch = repo.find_branch(&tracking_branch, BranchType::Remote)?;
    let remote_ref = repo.reference_to_annotated_commit(remote_branch.get())?;

    // It should never be the case this function is called when the local branch
    // exists. Keep `force` to `false` to catch any misuse here.
    repo.branch_from_annotated_commit(&pc.config.repos.remote.branch, &remote_ref, false)?;
    info!(
        "<bold>Created</>: Local branch <cyan>{}</>.",
        &pc.config.repos.remote.branch
    );

    Ok(())
}

fn local_rebase_remote(pc: &PathConfig, repo: &Repository) -> Result<()> {
    fetch_remote(pc, repo)?;

    let tracking_branch = pc.config.repos.remote.tracking_branch();
    let remote_branch = repo.find_branch(&tracking_branch, BranchType::Remote)?;
    let remote_ref = repo.reference_to_annotated_commit(remote_branch.get())?;

    // Our remote branch after fetching should exist at the fetch. We could just
    // rebase onto the remote branch directly, but let's keep things local when
    // we can.
    let local_branch = repo.find_branch(&pc.config.repos.remote.branch, BranchType::Local)?;
    let local_ref = repo.reference_to_annotated_commit(local_branch.get())?;

    let signature = now_signature(pc)?;
    repo.rebase(Some(&local_ref), Some(&remote_ref), None, None)?
        .finish(Some(&signature))?;
    info!(
        "<bold>Rebased:</> Local branch onto <cyan>{}<cyan>.",
        &tracking_branch
    );

    Ok(())
}

pub fn pull(pc: &PathConfig, repo: &mut Repository) -> Result<()> {
    check_working_repo(repo)?;

    // If our local branch exists, it must also have a commit on it. Therefore
    // we can apply stashes. Stow away our changes, rebase on remote, and then
    // reapply those changes.
    if repo
        .find_branch(&pc.config.repos.remote.branch, BranchType::Local)
        .is_ok()
    {
        return Ok(with_stash(pc, repo, |pc, repo| {
            Ok(local_rebase_remote(pc, repo)?)
        })?);
    }

    // If our local branch does not exist yet, we are likely in an empty git
    // repository. In this case, we should just try to find the remote branch
    // and establish a copy locally of the same name.
    //
    // That said, there is a possibility our repository isn't empty. We also are
    // not necessarily able to stash changes and reapply them like we normally
    // would since its possible we do not have an initial commit yet. Generally
    // switching would be fine but its also possible the user has a file that
    // would be overwritten on change. For this reason, we just create an
    // initial commit for any existing files so the user can reference it later
    // if need be.
    if let Some(mut index) = index_with_all(repo)? {
        let index_oid = index.write_tree()?;
        let index_tree = repo.find_tree(index_oid)?;
        info!("<bold>Wrote:</> Index to tree <cyan>{}</>.", index_oid);

        let signature = now_signature(pc)?;
        let message = "Save potentially conflicting files here.";
        // If we are on a current branch, there should exist a commit we
        // can just push onto. Otherwise let's create a new branch with the
        // saved contents.
        if let Some(parent_commit) = get_commit_at_head(repo) {
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &index_tree,
                &[&parent_commit],
            )?;
            info!("<bold>Saved:</> Potentially conflicting files in new commit of <cyan>HEAD</>.");
        } else {
            let temp_branch = temporary_branch_name(pc, repo)?;
            let refspec = format!("refs/heads/{}", &temp_branch);
            repo.commit(
                Some(&refspec),
                &signature,
                &signature,
                message,
                &index_tree,
                &[],
            )?;
            info!(
                "<bold>Saved:</> Potentially conflicting files on branch <cyan>{}</>.",
                temp_branch
            );
        }
    }

    Ok(local_from_remote(pc, repo)?)
}

// ========================================
// Index
// ========================================

fn index_with_all(repo: &Repository) -> Result<Option<Index>> {
    let mut index = repo.index()?;
    index.add_all(["."].iter(), IndexAddOption::DEFAULT, None)?;
    let has_diff = if let Some(commit) = get_commit_at_head(repo) {
        let diff_stats = repo
            .diff_tree_to_workdir_with_index(
                Some(&repo.find_tree(commit.tree_id())?),
                Some(
                    DiffOptions::new()
                        .include_untracked(true)
                        .include_unreadable(true),
                ),
            )?
            .stats()?;
        diff_stats.files_changed() != 0
            || diff_stats.insertions() != 0
            || diff_stats.deletions() != 0
    } else {
        !index.is_empty()
    };
    if has_diff {
        Ok(Some(index))
    } else {
        Ok(None)
    }
}

fn with_stash<T>(pc: &PathConfig, repo: &mut Repository, function: T) -> Result<()>
where
    T: Fn(&PathConfig, &mut Repository) -> Result<()>,
{
    let signature = now_signature(pc)?;
    let stash_oid = match repo.stash_save(
        &signature,
        "Temporary stash during pull",
        Some(StashFlags::INCLUDE_UNTRACKED),
    ) {
        Ok(oid) => {
            info!("<bold>Stashed:</> Changes in <cyan>{}</>.", oid);
            Some(oid)
        }
        Err(e) if e.class() == git2::ErrorClass::Stash && e.code() == git2::ErrorCode::NotFound => {
            None
        }
        Err(e) => Err(e)?,
    };

    function(pc, repo)?;

    if let Some(oid) = stash_oid {
        // It is possible something else made changes to our stash while we were
        // rebasing. To be extra cautious, search for our specific stash
        // instance.
        let mut stash_index = None;
        repo.stash_foreach(|index, _message, each_oid| {
            if *each_oid == oid {
                stash_index = Some(index);
                false
            } else {
                true
            }
        })?;
        if let Some(index) = stash_index {
            let mut checkout = git2::build::CheckoutBuilder::new();
            checkout.use_ours(true);

            let mut apply_options = StashApplyOptions::new();
            apply_options.checkout_options(checkout);

            repo.stash_apply(index, Some(&mut apply_options))?;
            info!("<bold>Applied</> Stash <cyan>{}</>.", oid);
        } else {
            warn!("Could not find stash <cyan>{}<cyan>. Ignoring.", oid);
        }
    }

    Ok(())
}

// ========================================
// Remote
// ========================================

fn find_remote<'repo>(pc: &PathConfig, repo: &'repo Repository) -> Result<Remote<'repo>> {
    repo.remote_set_url(&pc.config.repos.remote.name, &pc.config.repos.remote.url)?;
    // If the remote already exists, this just updates the fetchspec. We could
    // go with "*" instead of {branch} for all remote branches, but choosing to
    // be precise..
    // https://git-scm.com/book/en/v2/Git-Internals-The-Refspec
    repo.remote_add_fetch(
        &pc.config.repos.remote.name,
        &format!(
            "+refs/heads/{}:refs/remotes/{}",
            pc.config.repos.remote.branch,
            pc.config.repos.remote.tracking_branch(),
        ),
    )?;
    Ok(repo.find_remote(&pc.config.repos.remote.name)?)
}

fn fetch_remote<'repo>(pc: &PathConfig, repo: &'repo Repository) -> Result<Remote<'repo>> {
    let mut remote = find_remote(pc, repo)?;
    let mut fetch_options = get_fetch_options(pc)?;
    remote.fetch(
        &[&pc.config.repos.remote.branch],
        Some(&mut fetch_options),
        None,
    )?;
    let tracking_branch = pc.config.repos.remote.tracking_branch();
    info!(
        "<bold>Fetched:</> Remote branch <cyan>{}<cyan>.",
        &tracking_branch
    );

    Ok(remote)
}

fn get_remote_callbacks(pc: &PathConfig) -> Result<RemoteCallbacks> {
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

    Ok(callbacks)
}

fn get_fetch_options(pc: &PathConfig) -> Result<FetchOptions> {
    let callbacks = get_remote_callbacks(pc)?;
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);
    Ok(fetch_options)
}

fn get_push_options(pc: &PathConfig) -> Result<PushOptions> {
    let callbacks = get_remote_callbacks(pc)?;
    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);
    Ok(push_options)
}

// ========================================
// Miscellaneous
// ========================================

fn check_working_repo(repo: &Repository) -> Result<PathBuf> {
    Ok(repo.workdir().ok_or(Error::InvalidBareRepo)?.to_path_buf())
}

fn get_commit_at_head(repo: &Repository) -> Option<Commit> {
    let peel = || -> Result<Commit> {
        Ok(repo
            .head()?
            .resolve()?
            .peel(ObjectType::Commit)?
            .into_commit()
            .map_err(|_| git2::Error::from_str("Couldn't find commit"))?)
    };
    peel().ok()
}

fn now_signature(pc: &PathConfig) -> Result<Signature> {
    Ok(Signature::now(&pc.config.user.name, &pc.config.user.email)?)
}

fn temporary_branch_name(pc: &PathConfig, repo: &Repository) -> Result<String> {
    let mut branch_names = HashSet::new();
    for b in repo.branches(Some(BranchType::Local))? {
        if let Ok((branch, _branch_type)) = b {
            if let Some(name) = branch.name()? {
                branch_names.insert(name.to_owned());
            }
        }
    }

    let mut count = 1;
    let mut temp_name = format!("{}-tmp", &pc.config.repos.remote.branch);
    while branch_names.contains(&temp_name) {
        temp_name = format!("{}-tmp-{}", &pc.config.repos.remote.branch, count);
        count += 1;
    }

    Ok(temp_name)
}
