use regex::Regex;
use serde::de;
use serde::de::{Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::env::VarError;
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};
use std::{env, error, fmt, fs, io, result, str};

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
// Validation
// ========================================

pub fn validate_is_file(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        // TODO(jrpotter): Use `IsADirectory` when stable.
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("'{}' is not a file.", path.display()),
        ))?;
    }
    Ok(())
}

pub fn validate_is_dir(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_dir() {
        // TODO(jrpotter): Use `NotADirectory` when stable.
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("'{}' is not a directory.", path.display()),
        ))?;
    }
    Ok(())
}

// ========================================
// Resolution
// ========================================

/// Find environment variables within the argument and expand them if possible.
///
/// Returns an error if any found environment variables are not defined.
pub fn expand_env(s: &str) -> Result<String> {
    let re = Regex::new(r"\$(?P<env>[[:alnum:]]+)").unwrap();
    let mut path = s.to_owned();
    for caps in re.captures_iter(s) {
        let evar = env::var(&caps["env"])?;
        path = path.replace(&format!("${}", &caps["env"]), &evar);
    }
    Ok(path)
}

/// Attempt to resolve the provided path, returning a fully resolved path
/// instance if successful.
pub fn resolve(path: &Path) -> Result<ResPathBuf> {
    let mut resolved = env::current_dir()?;
    for comp in path.components() {
        match comp {
            Component::Prefix(_) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "We do not currently support Windows.",
            ))?,
            Component::RootDir => {
                resolved.clear();
                resolved.push(Component::RootDir)
            }
            Component::CurDir => (),
            Component::ParentDir => {
                if !resolved.pop() {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Cannot take parent of root.",
                    ))?
                }
            }
            Component::Normal(c) => {
                let c: OsString = expand_env(&c.to_string_lossy())?.into();
                resolved.push(Component::Normal(&c));
            }
        }
    }
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
