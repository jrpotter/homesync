//! Utilities for resolving paths.

use serde::{
    de,
    de::{Unexpected, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{
    env,
    env::VarError,
    error,
    ffi::OsString,
    fmt,
    hash::{Hash, Hasher},
    io,
    path::{Component, Path, PathBuf},
    result, str,
};

// ========================================
// Error
// ========================================

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    IOError(io::Error),
    VarError(VarError),
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IOError(e) => write!(f, "{}", e),
            Error::VarError(e) => write!(f, "{}", e),
        }
    }
}

impl error::Error for Error {}

// ========================================
// Path
// ========================================

/// A "resolved" `PathBuf` that takes in the originally supplied (potentially
/// relative) path and annotates it with the absolute path. A `ResPathBuf`
/// instance cannot be made if the relative path supplied to it does not refer
/// to an actual file.
#[derive(Clone, Debug)]
pub struct ResPathBuf {
    inner: PathBuf,
    unresolved: PathBuf,
}

fn unresolved_error(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::Other,
        format!("Path '{}' should be fully resolved.", path.display()),
    )
}

impl ResPathBuf {
    pub fn new(inner: &Path, unresolved: &Path) -> Result<Self> {
        if !inner.is_absolute() {
            Err(unresolved_error(inner))?;
        }
        Ok(ResPathBuf {
            inner: inner.to_path_buf(),
            unresolved: unresolved.to_path_buf(),
        })
    }

    pub fn resolved(&self) -> &PathBuf {
        &self.inner
    }

    pub fn unresolved(&self) -> &PathBuf {
        &self.unresolved
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

impl AsRef<PathBuf> for ResPathBuf {
    fn as_ref(&self) -> &PathBuf {
        &self.inner
    }
}

impl AsRef<Path> for ResPathBuf {
    fn as_ref(&self) -> &Path {
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

// ========================================
// (De)serialization
// ========================================

impl Serialize for ResPathBuf {
    fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.inner.as_path().serialize(serializer)
    }
}

struct ResPathBufVisitor;

impl<'de> Visitor<'de> for ResPathBufVisitor {
    type Value = ResPathBuf;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("path string")
    }

    fn visit_str<E>(self, v: &str) -> result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        resolve(&PathBuf::from(&v))
            .map_err(|_| de::Error::custom(format!("Could not resolve path {}", v)))
    }

    fn visit_string<E>(self, v: String) -> result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        resolve(&PathBuf::from(&v))
            .map_err(|_| de::Error::custom(format!("Could not resolve path {}", v)))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        let value = str::from_utf8(v)
            .map(From::from)
            .map_err(|_| de::Error::invalid_value(Unexpected::Bytes(v), &self))?;
        self.visit_str(value)
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        let value = String::from_utf8(v)
            .map(From::from)
            .map_err(|e| de::Error::invalid_value(Unexpected::Bytes(&e.into_bytes()), &self))?;
        self.visit_string(value)
    }
}

impl<'de> Deserialize<'de> for ResPathBuf {
    fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(ResPathBufVisitor)
    }
}

// ========================================
// Resolution
// ========================================

/// Find environment variables within the argument and expand them if possible.
///
/// Returns an error if any found environment variables are not defined.
pub fn expand(path: &Path) -> Result<PathBuf> {
    let mut expanded = env::current_dir()?;
    for comp in path.components() {
        match comp {
            Component::Prefix(_) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "We do not support Windows.",
            ))?,
            Component::RootDir => {
                expanded.clear();
                expanded.push(Component::RootDir)
            }
            Component::CurDir => (),
            Component::ParentDir => {
                if !expanded.pop() {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Cannot take parent of root.",
                    ))?
                }
            }
            Component::Normal(c) => {
                let lossy = c.to_string_lossy();
                if lossy.starts_with("$") {
                    let evar = env::var(lossy.replacen("$", "", 1))?;
                    expanded.push(Component::Normal(&OsString::from(evar)));
                } else {
                    expanded.push(c);
                }
            }
        }
    }
    Ok(expanded)
}

/// Attempt to resolve the provided path, returning a fully resolved path
/// instance if successful.
pub fn resolve(path: &Path) -> Result<ResPathBuf> {
    let resolved = expand(&path)?;
    let resolved = resolved.canonicalize()?;
    Ok(ResPathBuf {
        inner: resolved,
        unresolved: path.to_path_buf(),
    })
}

/// Attempt to resolve the provided path, returning a fully resolved path
/// instance if successful.
///
/// If the provided file does not exist but could potentially exist in the
/// future (e.g. for paths with environment variables defined), this will
/// return a `None` instead of an error.
pub fn soft_resolve(path: &Path) -> Result<Option<ResPathBuf>> {
    match resolve(path) {
        Ok(resolved) => Ok(Some(resolved)),
        Err(Error::IOError(e)) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e @ Error::IOError(_)) => Err(e),
        // An ENV variable isn't defined yet, but we assume its possible it'll
        // be defined in the future. Don't report as an error.
        Err(Error::VarError(_)) => Ok(None),
    }
}

// ========================================
// Tests
// ========================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tempfile::NamedTempFile;

    #[test]
    fn respath_absolute() {
        let abs = Path::new("/home/jrpotter/example");
        let rel = Path::new("home/jrpotter/example");
        assert!(ResPathBuf::new(abs, rel).is_ok());
        assert!(ResPathBuf::new(rel, rel).is_err());
    }

    #[test]
    fn respath_equality() {
        let path = Path::new("/home/jrpotter/example");
        let res1 = ResPathBuf::new(path, Path::new("rel1")).unwrap();
        let res2 = ResPathBuf::new(path, Path::new("rel2")).unwrap();
        assert_eq!(res1, res2);
    }

    #[test]
    fn respath_hash() {
        let mut set = HashSet::new();
        let path = Path::new("/home/jrpotter/example");
        let res1 = ResPathBuf::new(path, Path::new("rel1")).unwrap();
        let res2 = ResPathBuf::new(path, Path::new("rel2")).unwrap();
        set.insert(res1);
        set.insert(res2);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn expand_root_dir() {
        let current = env::current_dir().unwrap();
        let expanded = expand(Path::new("")).unwrap();
        assert_eq!(current, expanded);
        let expanded = expand(Path::new("/")).unwrap();
        assert_eq!(Path::new("/"), expanded);
    }

    #[test]
    fn expand_component() {
        env::set_var("EXAMPLE", "example");
        let expanded = expand(Path::new("/a/b/$EXAMPLE/c")).unwrap();
        assert_eq!(Path::new("/a/b/example/c"), expanded);
        let expanded = expand(Path::new("/a/b/pre$EXAMPLE/c")).unwrap();
        assert_eq!(Path::new("/a/b/pre$EXAMPLE/c"), expanded);
    }

    #[test]
    fn resolve() {
        let path: PathBuf;
        {
            let temp = NamedTempFile::new().unwrap();
            path = temp.path().to_path_buf();
            assert!(super::resolve(&path).is_ok());
        }
        assert!(super::resolve(&path).is_err());
    }

    #[test]
    fn soft_resolve() {
        let path: PathBuf;
        {
            let temp = NamedTempFile::new().unwrap();
            path = temp.path().to_path_buf();
            assert!(super::soft_resolve(&path).unwrap().is_some());
        }
        assert!(super::soft_resolve(&path).unwrap().is_none());
    }
}
