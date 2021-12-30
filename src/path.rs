use regex::Regex;
use std::env;
use std::ffi::{OsStr, OsString};
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Component, Path, PathBuf};

// ========================================
// Path
// ========================================

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalPathBuf(PathBuf);

impl NormalPathBuf {
    pub fn display(&self) -> std::path::Display {
        self.0.display()
    }
}

impl AsRef<Path> for NormalPathBuf {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<PathBuf> for NormalPathBuf {
    fn as_ref(&self) -> &PathBuf {
        &self.0
    }
}

impl Hash for NormalPathBuf {
    fn hash<H: Hasher>(&self, h: &mut H) {
        for component in self.0.components() {
            component.hash(h);
        }
    }
}

pub enum Normalize {
    Done(NormalPathBuf), // An instance of a fully resolved path.
    Pending,             // An instance of a path that cannot yet be normalized.
}

// Find environment variables found within the argument and expand them if
// possible.
//
// Returns `None` in the case an environment variable present within the
// argument is not defined.
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

// Normalizes the provided path, returning a new instance.
//
// There currently does not exist a method that yields some canonical path for
// files that do not exist (at least in the parts of the standard library I've
// looked in). We create a consistent view of every path so as to avoid watching
// the same path multiple times, which would duplicate messages on changes.
//
// Note this does not actually prevent the issue fully. We could have two paths
// that refer to the same real path - normalization would not catch this.
pub fn normalize(path: &Path) -> io::Result<Normalize> {
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
            Component::Normal(c) => match expand_env(c) {
                Some(c) => pb.push(Component::Normal(&c)),
                // The environment variable isn't defined yet but might be in
                // the future.
                None => return Ok(Normalize::Pending),
            },
        }
    }
    Ok(Normalize::Done(NormalPathBuf(pb)))
}
