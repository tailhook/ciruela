use std::fmt;
use std::path::{Path, PathBuf};
use std::borrow::Borrow;
use std::sync::Arc;

use serde::de::{Deserialize, Deserializer, Error};


/// Virtual path for uploading images
///
/// Basically it's an `Arc<PathBuf>` so it clones cheaply, but also has
/// convenience methods for extracting features specific to our
/// virtual paths.
///
/// The anatomy of the virtual path:
///
/// ```ignore
/// /[key]/[suffix]
/// ```
///
/// Type asserts on the presence of the ``key`` and that the path is absolute.
/// Suffix might be of arbitrary length including zero.
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Clone)]
pub struct VPath(Arc<PathBuf>);

#[derive(Fail, Debug)]
#[fail(display="path is not a valid virtual path")]
pub struct PathError;

impl VPath {
    /// Returns a `key`, i.e. the first component
    pub fn key(&self) -> &str {
        self.0.iter().nth(1).and_then(|x| x.to_str())
        .expect("valid virtual path")
    }
    /// Returns a `key`, i.e. the first component
    pub fn key_vpath(&self) -> VPath {
        // TODO(tailhook) optimize it
        self.0.iter().nth(1).and_then(|x| x.to_str())
        .map(|x| VPath::from(format!("/{}", x)))
        .expect("valid virtual path")
    }
    /// Returns a level of a directory
    ///
    /// Level is number of path components not counting a key
    pub fn level(&self) -> usize {
        // don't count key and initial slash
        self.0.iter().count() - 2
    }
    /// Return parent path relative to a `key`
    ///
    /// # Panics
    ///
    /// Panics if the directory contains only a key (has single component)
    pub fn parent_rel(&self) -> &Path {
        assert!(self.level() > 0);
        self.0.strip_prefix("/").ok().and_then(|x| x.parent())
        .expect("valid virtual path")
    }
    /// Return virtual path of the directory
    ///
    /// # Panics
    ///
    /// Panics if the directory contains only a key (has single component)
    pub fn parent(&self) -> VPath {
        assert!(self.level() > 0);
        let parent = self.0.parent().expect("valid virtual path");
        VPath(Arc::new(parent.to_path_buf()))
    }
    /// The path relative to the key
    pub fn suffix(&self) -> &Path {
        let mut names = self.0.iter();
        names.next().expect("valid virtual path");  // skip slash
        names.next().expect("valid virtual path");  // skip key
        names.as_path()
    }
    /// The last component of the directory
    ///
    /// # Panics
    ///
    /// Panics if the path has only one component (the key)
    pub fn final_name(&self) -> &str {
        assert!(self.level() > 0);
        self.0.file_name().and_then(|x| x.to_str())
        .expect("valid virtual path")
    }
    /// Join path to the virtual path
    ///
    /// # Panics
    ///
    /// Panics if suffix is invalid: empty, root or has parent `..` components
    pub fn join<P: AsRef<Path>>(&self, path: P) -> VPath {
        use std::path::Component::Normal;
        let path = path.as_ref();
        assert!(path != Path::new(""));
        assert!(path.components().all(|x| matches!(x, Normal(_))));
        VPath(Arc::new(self.0.join(path)))
    }
    /// Create a virtual path from path
    ///
    /// # Panics
    ///
    /// Panics if path is not absolute or doesn't contain directory components
    pub fn from<T: Into<PathBuf>>(t: T) -> VPath {
        let buf = t.into();
        assert!(buf.is_absolute());
        assert!(buf != Path::new("/"));
        VPath(Arc::new(buf))
    }

    /// Create a virtual path from path
    ///
    pub fn try_from<T: Into<PathBuf>>(t: T) -> Result<VPath, PathError> {
        let buf = t.into();
        // TODO(tailhook) check all components, check `/` at the end
        if !buf.is_absolute() || buf == Path::new("/") {
            return Err(PathError);
        }
        Ok(VPath(Arc::new(buf)))
    }

    /// Check this directory belongs to the specified basedir
    ///
    /// Technically the same, but faster version of:
    /// ```ignore
    /// self.parent() == base_dir
    /// ```
    pub fn matches_basedir(&self, base_dir: &VPath) -> bool {
        assert!(self.level() > 0);
        let parent = self.0.parent().expect("valid virtual path");
        return parent == base_dir.0.as_path();
    }
}

impl Borrow<Path> for VPath {
    fn borrow(&self) -> &Path {
        (&*self.0).borrow()
    }
}

impl AsRef<Path> for VPath {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl<'de> Deserialize<'de> for VPath {
    fn deserialize<D>(deserializer: D) -> Result<VPath, D::Error>
        where D: Deserializer<'de>
    {
        let s = String::deserialize(deserializer)?;
        if !check_path(&s) {
            return Err(D::Error::custom("virtual path must be absolute \
                and must contain at least two components"));
        }
        Ok(VPath(Arc::new(s.into())))
    }
}

fn check_path(path: &str) -> bool {
    use std::path::Component::*;
    let path = Path::new(path);
    let mut piter = path.components();
    if piter.next() != Some(RootDir) {
        return false;
    }
    let mut num = 0;
    for cmp in piter {
        if matches!(cmp, Normal(_)) {
            num += 1;
        } else {
            return false;
        }
    }
    return num >= 1;
}

impl fmt::Display for VPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.display().fmt(f)
    }
}

impl fmt::Debug for VPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "v{:?}", self.0)
    }
}
