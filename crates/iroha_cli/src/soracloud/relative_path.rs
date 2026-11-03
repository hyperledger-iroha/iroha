//! Lexical relative paths between Soracloud manifest files.
use std::path::{Component, Path, PathBuf};

/// Relative path that leads from the directory `base` to `path`, computed lexically.
///
/// When only `path` is absolute it is returned unchanged; when only `base` is absolute, or
/// `base` needs a `..` resolved that `path` does not share, there is no answer.
pub(super) fn diff_paths(path: &Path, base: &Path) -> Option<PathBuf> {
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
    use super::diff_paths;
    use std::path::Path;

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
            let actual = diff_paths(Path::new(path), Path::new(base));
            assert_eq!(
                actual.as_deref().map(Path::as_os_str),
                expected.map(|expected| Path::new(expected).as_os_str()),
                "diff_paths({path:?}, {base:?})"
            );
        }
    }

    #[test]
    fn manifest_relative_paths_walk_up_from_the_manifest_directory() {
        let base = Path::new("apps").join("demo");
        let target = Path::new("services").join("api.json");
        assert_eq!(
            diff_paths(&target, &base),
            Some(Path::new("..").join("..").join(&target))
        );
    }
}
