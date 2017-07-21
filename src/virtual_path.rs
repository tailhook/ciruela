use std::path::{Path, PathBuf, Iter};
use std::borrow::Borrow;

use serde::de::{Deserialize, Deserializer, Error};


#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Debug, Clone)]
pub struct VPath(PathBuf);

impl VPath {
    pub fn key(&self) -> &str {
        self.0.iter().nth(1).and_then(|x| x.to_str())
        .expect("valid virtual path")
    }
    pub fn level(&self) -> usize {
        // don't count key and initial slash
        self.0.iter().count() - 2
    }
    pub fn parent_rel(&self) -> &Path {
        debug_assert!(self.level() > 0);
        self.0.strip_prefix("/").ok().and_then(|x| x.parent())
        .expect("valid virtual path")
    }
    pub fn parent(&self) -> VPath {
        debug_assert!(self.level() > 0);
        VPath(self.0.parent().expect("valid virtual path").to_path_buf())
    }
    pub fn suffix(&self) -> &Path {
        let mut names = self.0.iter();
        names.next().expect("valid virtual path");  // skip slash
        names.next().expect("valid virtual path");  // skip key
        names.as_path()
    }
    pub fn names(&self) -> Iter {
        debug_assert!(self.level() > 0);
        let mut names = self.0.iter();
        names.next().expect("valid virtual path");  // skip slash
        names.next().expect("valid virtual path");  // skip key
        return names;
    }
    pub fn final_name(&self) -> &str {
        debug_assert!(self.level() > 0);
        self.0.file_name().and_then(|x| x.to_str())
        .expect("valid virtual path")
    }
    pub fn join<P: AsRef<Path>>(&self, path: P) -> VPath {
        use std::path::Component::Normal;
        let path = path.as_ref();
        assert!(path != Path::new(""));
        assert!(path.components().all(|x| matches!(x, Normal(_))));
        VPath(self.0.join(path))
    }
}

impl Borrow<Path> for VPath {
    fn borrow(&self) -> &Path {
        self.0.borrow()
    }
}

impl AsRef<Path> for VPath {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl<T> From<T> for VPath
    where T: Into<PathBuf>
{
    fn from(t: T) -> VPath {
        let buf = t.into();
        assert!(buf.is_absolute());
        assert!(buf != Path::new("/"));
        VPath(buf)
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
        Ok(VPath(s.into()))
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
