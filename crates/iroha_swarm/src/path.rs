//! Path utils.
use std::{
    ffi::OsStr,
    io,
    path::{Component, Path, PathBuf},
};
/// Absolute filesystem path.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AbsolutePath(std::path::PathBuf);
/// Relative filesystem path.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RelativePath(std::path::PathBuf);
#[derive(displaydoc::Display, Debug)]
pub enum Error {
    /// Failed to construct an absolute path: {0}
    AbsolutePath(std::io::Error),
    /// Failed to construct a relative path.
    RelativePath,
}
impl std::error::Error for Error {}
impl AbsolutePath {
    pub fn new(path: &std::path::Path) -> Result<Self, Error> {
        Ok(Self(if path.is_absolute() {
            path.to_path_buf()
        } else {
            absolutize(path).map_err(Error::AbsolutePath)?
        }))
    }
    #[cfg(test)]
    fn with_virtual_root(
        path: &std::path::Path,
        virtual_root: &std::path::Path,
    ) -> Result<Self, Error> {
        Ok(Self(
            absolutize_virtually(path, virtual_root).map_err(Error::AbsolutePath)?,
        ))
    }
    pub fn relative_to(&self, to: &Self) -> Result<RelativePath, Error> {
        let path = diff_paths(&self.0, &to.0).ok_or(Error::RelativePath)?;
        Ok(RelativePath(if path.starts_with("..") {
            path
        } else {
            std::path::Path::new("./").join(path)
        }))
    }
    pub fn parent(&self) -> Option<Self> {
        Some(Self(self.0.parent()?.to_path_buf()))
    }
}
impl AsRef<std::path::Path> for AbsolutePath {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}
impl AsRef<std::path::Path> for RelativePath {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}
/// `path` made absolute against the process working directory, resolved lexically.
///
/// See [`resolve_dots`] for the rules; an absolute `path` never reads the working directory.
fn absolutize(path: &Path) -> io::Result<PathBuf> {
    let cwd = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir()?
    };
    Ok(resolve_dots(path, &cwd))
}
/// `path` placed under `virtual_root`, resolved lexically.
///
/// Only a `path` that starts with `.` or `..` continues from the process working directory.
/// A relative result is appended to the absolutized `virtual_root`; an absolute result outside
/// `virtual_root` is rejected with [`io::ErrorKind::InvalidInput`].
#[cfg(test)]
fn absolutize_virtually(path: &Path, virtual_root: &Path) -> io::Result<PathBuf> {
    let virtual_root = absolutize(virtual_root)?;
    let cwd = match path.components().next() {
        Some(Component::CurDir | Component::ParentDir) => std::env::current_dir()?,
        _ => PathBuf::new(),
    };
    let path = resolve_dots(path, &cwd);
    if !path.is_absolute() {
        let mut joined = virtual_root;
        joined.push(path);
        Ok(joined)
    } else if path.starts_with(&virtual_root) {
        Ok(path)
    } else {
        Err(io::Error::from(io::ErrorKind::InvalidInput))
    }
}
/// Lexically resolve the `.` and `..` components of `path` without touching the filesystem.
///
/// A relative `path` continues from `cwd`, or from the parent of `cwd` when it starts with
/// `..`; an empty `path` resolves to `cwd`. A later `..` removes the preceding component but
/// never climbs above the root. Components taken from `cwd` are kept verbatim.
fn resolve_dots<'a>(path: &'a Path, cwd: &'a Path) -> PathBuf {
    let mut components = path.components();
    let Some(first) = components.next() else {
        return cwd.to_path_buf();
    };
    let mut root = PathBuf::new();
    let mut names = Vec::new();
    match first {
        Component::Prefix(_) => root.push(first),
        Component::RootDir => {
            // A root without a drive prefix (Windows `\dir`) stays on the drive of `cwd`.
            if let Some(prefix @ Component::Prefix(_)) = cwd.components().next() {
                root.push(prefix);
            }
            root.push(first);
        }
        Component::CurDir => split_root(cwd, &mut root, &mut names),
        Component::ParentDir => {
            split_root(cwd, &mut root, &mut names);
            names.pop();
        }
        Component::Normal(name) => {
            split_root(cwd, &mut root, &mut names);
            names.push(name);
        }
    }
    for component in components {
        match component {
            Component::Prefix(_) | Component::RootDir => root.push(component),
            Component::CurDir => {}
            Component::ParentDir => {
                names.pop();
            }
            Component::Normal(name) => names.push(name),
        }
    }
    root.extend(names);
    root
}
/// Append the prefix and root of `path` to `root` and its other components, verbatim, to
/// `names`.
fn split_root<'a>(path: &'a Path, root: &mut PathBuf, names: &mut Vec<&'a OsStr>) {
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => root.push(component),
            Component::CurDir | Component::ParentDir | Component::Normal(_) => {
                names.push(component.as_os_str());
            }
        }
    }
}
/// Relative path that leads from the directory `base` to `path`, computed lexically.
///
/// When only `path` is absolute it is returned unchanged; when only `base` is absolute, or
/// `base` needs a `..` resolved that `path` does not share, there is no answer.
fn diff_paths(path: &Path, base: &Path) -> Option<PathBuf> {
    if path.is_absolute() != base.is_absolute() {
        return path.is_absolute().then(|| path.to_path_buf());
    }
    let mut path_components = path.components();
    let mut base_components = base.components();
    let mut relative = Vec::new();
    loop {
        match (path_components.next(), base_components.next()) {
            (None, None) => break,
            (Some(component), None) => {
                relative.push(component);
                relative.extend(path_components.by_ref());
                break;
            }
            (None, Some(_)) => relative.push(Component::ParentDir),
            (Some(component), Some(base_component))
                if relative.is_empty() && component == base_component => {}
            (Some(component), Some(Component::CurDir)) => relative.push(component),
            (Some(_), Some(Component::ParentDir)) => return None,
            (Some(component), Some(_)) => {
                relative.push(Component::ParentDir);
                relative.extend(base_components.by_ref().map(|_| Component::ParentDir));
                relative.push(component);
                relative.extend(path_components.by_ref());
                break;
            }
        }
    }
    Some(relative.into_iter().collect())
}
#[cfg(test)]
mod tests {
    use super::{
        AbsolutePath, Error, RelativePath, absolutize, diff_paths, resolve_dots, split_root,
    };
    use std::path::{Path, PathBuf};
    #[test]
    fn relative_inner_path_starts_with_dot() {
        let root = "/".as_ref();
        let a = AbsolutePath::with_virtual_root("./a/b/c".as_ref(), root).unwrap();
        let b = AbsolutePath::with_virtual_root("./".as_ref(), root).unwrap();
        assert_eq!(
            a.relative_to(&b).unwrap(),
            RelativePath(std::path::PathBuf::from("./a/b/c"))
        );
    }
    #[test]
    fn relative_outer_path_starts_with_dots() {
        let root = "/".as_ref();
        let a = AbsolutePath::with_virtual_root("./a/b/c".as_ref(), root).unwrap();
        let b = AbsolutePath::with_virtual_root("./cde".as_ref(), root).unwrap();
        assert_eq!(
            b.relative_to(&a).unwrap(),
            RelativePath(std::path::PathBuf::from("../../../cde"))
        );
    }
    #[cfg(unix)]
    #[test]
    fn virtual_root_keeps_paths_inside_the_root() {
        let root = Path::new("/root");
        let inside = AbsolutePath::with_virtual_root("inside/x".as_ref(), root).unwrap();
        assert_eq!(inside.as_ref().as_os_str(), "/root/inside/x");
        let absolute = AbsolutePath::with_virtual_root("/root/x".as_ref(), root).unwrap();
        assert_eq!(absolute.as_ref().as_os_str(), "/root/x");
        let Err(Error::AbsolutePath(error)) =
            AbsolutePath::with_virtual_root("/outside".as_ref(), root)
        else {
            panic!("a path outside the virtual root must be rejected");
        };
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
    #[test]
    fn absolutize_joins_relative_paths_to_the_working_directory() {
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(absolutize(Path::new("a/./b")).unwrap(), cwd.join("a/b"));
        let absolute = cwd.join("x/../y");
        assert_eq!(absolutize(&absolute).unwrap(), cwd.join("y"));
    }
    #[cfg(unix)]
    #[test]
    fn resolve_dots_matches_lexical_absolutization() {
        for (path, cwd, expected) in [
            ("/path/to/./123/../456", "", "/path/to/456"),
            ("./path/to", "/cwd/dir", "/cwd/dir/path/to"),
            ("../path", "/cwd/dir", "/cwd/path"),
            ("path/../../to", "/cwd/dir", "/cwd/to"),
            ("../../../..", "/cwd", "/"),
            ("", "/cwd/dir", "/cwd/dir"),
            ("a//b/./c/", "/", "/a/b/c"),
            ("../../x", "a", "x"),
            ("a/../..", "", ""),
            ("x", "/a/../b", "/a/../b/x"),
        ] {
            assert_eq!(
                resolve_dots(Path::new(path), Path::new(cwd)).as_os_str(),
                expected,
                "resolve_dots({path:?}, {cwd:?})"
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn split_root_keeps_non_root_components_verbatim() {
        let mut root = PathBuf::new();
        let mut names = Vec::new();
        split_root(Path::new("/a/./../b/"), &mut root, &mut names);
        assert_eq!(root.as_os_str(), "/");
        assert_eq!(names, ["a", "..", "b"]);
        let mut root = PathBuf::new();
        let mut names = Vec::new();
        split_root(Path::new("./a"), &mut root, &mut names);
        assert_eq!(root.as_os_str(), "");
        assert_eq!(names, [".", "a"]);
    }
    #[cfg(unix)]
    #[test]
    fn diff_paths_builds_relative_paths() {
        for (path, base, expected) in [
            ("/foo/bar", "/foo/bar/baz", Some("..")),
            ("/foo/bar/baz", "/foo/bar", Some("baz")),
            ("/foo/bar/quux", "/foo/bar/baz", Some("../quux")),
            ("/foo/bar", "baz", Some("/foo/bar")),
            ("baz", "/foo", None),
            ("/foo/bar", "/baz", Some("../foo/bar")),
            ("foo", "bar", Some("../foo")),
            ("a/b", "a/../c", None),
            ("a/b", "./a", Some("a/../b")),
            ("/a/b", "/a/b", Some("")),
        ] {
            assert_eq!(
                diff_paths(Path::new(path), Path::new(base)).as_deref(),
                expected.map(Path::new),
                "diff_paths({path:?}, {base:?})"
            );
            if let Some(expected) = expected {
                assert_eq!(
                    diff_paths(Path::new(path), Path::new(base))
                        .unwrap()
                        .as_os_str(),
                    expected
                );
            }
        }
    }
}
