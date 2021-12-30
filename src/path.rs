use regex::Regex;
use std::env;
use std::ffi::{OsStr, OsString};
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Component, Path, PathBuf};

// ========================================
// Path
// ========================================

#[derive(Clone, Debug)]
pub struct ResPathBuf {
    inner: PathBuf,
    unresolved: PathBuf,
}

impl ResPathBuf {
    pub fn display(&self) -> std::path::Display {
        self.inner.display()
    }

    pub fn unresolved(&self) -> &PathBuf {
        return &self.unresolved;
    }
}

impl PartialEq<ResPathBuf> for ResPathBuf {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl PartialEq<PathBuf> for ResPathBuf {
    fn eq(&self, other: &PathBuf) -> bool {
        self.inner == *other
    }
}

impl Eq for ResPathBuf {}

impl From<ResPathBuf> for PathBuf {
    fn from(path: ResPathBuf) -> PathBuf {
        path.inner
    }
}

impl AsRef<Path> for ResPathBuf {
    fn as_ref(&self) -> &Path {
        &self.inner
    }
}

impl AsRef<PathBuf> for ResPathBuf {
    fn as_ref(&self) -> &PathBuf {
        &self.inner
    }
}

impl Hash for ResPathBuf {
    fn hash<H: Hasher>(&self, h: &mut H) {
        for component in self.inner.components() {
            component.hash(h);
        }
    }
}

/// Find environment variables found within the argument and expand them if
/// possible.
///
/// Returns `None` in the case an environment variable present within the
/// argument is not defined.
fn expand_env(s: &OsStr) -> Option<OsString> {
    let re = Regex::new(r"\$(?P<env>[[:alnum:]]+)").unwrap();
    let lossy = s.to_string_lossy();
    let mut path = lossy.clone().to_string();
    for caps in re.captures_iter(&lossy) {
        let evar = env::var(&caps["env"]).ok()?;
        path = path.replace(&format!("${}", &caps["env"]), &evar);
    }
    Some(path.into())
}

/// Attempt to resolve the provided path, returning a fully resolved path
/// instance.
///
/// If the provided file does not exist but could potentially exist in the
/// future (e.g. for paths with environment variables defined), this will
/// return a `None` instead of an error.
pub fn resolve(path: &Path) -> io::Result<Option<ResPathBuf>> {
    let mut expanded = env::current_dir()?;
    for comp in path.components() {
        match comp {
            Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "We do not currently support Windows.",
                ))
            }
            Component::RootDir => {
                expanded.clear();
                expanded.push(Component::RootDir)
            }
            Component::CurDir => (), // Make no changes.
            Component::ParentDir => {
                if !expanded.pop() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Cannot take parent of root.",
                    ));
                }
            }
            Component::Normal(c) => match expand_env(c) {
                Some(c) => expanded.push(Component::Normal(&c)),
                // The environment variable isn't defined yet but might be in
                // the future.
                None => return Ok(None),
            },
        }
    }
    match expanded.canonicalize() {
        Ok(resolved) => Ok(Some(ResPathBuf {
            inner: resolved,
            unresolved: path.to_path_buf(),
        })),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
