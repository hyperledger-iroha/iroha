//! Shared filesystem-path validation for MOCHI-owned local files.

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Component, Path, PathBuf},
};

#[cfg(any(target_os = "linux", target_os = "android"))]
const O_NOFOLLOW_NONBLOCK: i32 = 0x0002_0000 | 0x0000_0800;
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const O_NOFOLLOW_NONBLOCK: i32 = 0x0000_0100 | 0x0000_0004;
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!("Mochi file custody requires defined no-follow/nonblocking open flags");

#[derive(Clone, Copy)]
enum MissingDirectoryPolicy {
    ReturnMissing,
    Create,
    CreateOwnerOnly,
}

/// Validate an existing directory path, rejecting link components observed while walking it.
pub(crate) fn validate_directory_path(path: &Path, subject: &str) -> io::Result<Option<PathBuf>> {
    walk_directory_path(path, subject, MissingDirectoryPolicy::ReturnMissing)
}

/// Create a directory path, rejecting link components observed while walking it.
pub(crate) fn ensure_directory_path(path: &Path, subject: &str) -> io::Result<PathBuf> {
    require_directory_path(path, subject, MissingDirectoryPolicy::Create)
}

/// Create an owner-only directory path, rejecting observed link components.
pub(crate) fn ensure_owner_only_directory_path(path: &Path, subject: &str) -> io::Result<PathBuf> {
    require_directory_path(path, subject, MissingDirectoryPolicy::CreateOwnerOnly)
}

/// Open an existing Unix path without following its final link or blocking on a FIFO.
///
/// Callers must still inspect the returned descriptor and compare it with any pathname metadata
/// used for admission. The nonblocking flag has no effect on regular files.
#[cfg(unix)]
pub(crate) fn open_existing_file_no_follow_nonblocking(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW_NONBLOCK)
        .open(path)
}

/// Open an existing path on platforms without Unix FIFO/link flags.
#[cfg(not(unix))]
pub(crate) fn open_existing_file_no_follow_nonblocking(path: &Path) -> io::Result<File> {
    OpenOptions::new().read(true).open(path)
}

fn require_directory_path(
    path: &Path,
    subject: &str,
    policy: MissingDirectoryPolicy,
) -> io::Result<PathBuf> {
    walk_directory_path(path, subject, policy)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{subject} disappeared during creation"),
        )
    })
}

fn walk_directory_path(
    path: &Path,
    subject: &str,
    policy: MissingDirectoryPolicy,
) -> io::Result<Option<PathBuf>> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                current.push(component.as_os_str());
            }
            Component::CurDir => continue,
            Component::ParentDir => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{subject} must not contain `..`"),
                ));
            }
        }
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                if let Some(target) = resolve_system_directory_link(&current, &metadata)? {
                    current = target;
                    continue;
                }
                return Err(not_real_directory(subject, &current));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(not_real_directory(subject, &current));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => match policy {
                MissingDirectoryPolicy::ReturnMissing => return Ok(None),
                MissingDirectoryPolicy::Create | MissingDirectoryPolicy::CreateOwnerOnly => {
                    fs::create_dir(&current)?;
                    #[cfg(unix)]
                    if matches!(policy, MissingDirectoryPolicy::CreateOwnerOnly) {
                        fs::set_permissions(&current, fs::Permissions::from_mode(0o700))?;
                    }
                }
            },
            Err(error) => return Err(error),
        }
    }
    Ok(Some(current))
}

fn not_real_directory(subject: &str, path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "{subject} component `{}` must be a real directory",
            path.display()
        ),
    )
}

/// Resolve the one root-managed system-directory link MOCHI needs on macOS.
///
/// macOS exposes `/var` as `/private/var`, which is also where temporary
/// directories normally live. All other links fail closed, including
/// root-owned links whose targets could contain a user-controlled link.
fn resolve_system_directory_link(
    path: &Path,
    metadata: &fs::Metadata,
) -> io::Result<Option<PathBuf>> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::MetadataExt as _;

        if path != Path::new("/var") || metadata.uid() != 0 {
            return Ok(None);
        }
        let target = fs::read_link(path)?;
        if target != Path::new("private/var") {
            return Ok(None);
        }
        for component in [Path::new("/private"), Path::new("/private/var")] {
            let metadata = fs::symlink_metadata(component)?;
            if metadata.file_type().is_symlink()
                || !metadata.is_dir()
                || metadata.uid() != 0
                || metadata.mode() & 0o022 != 0
            {
                return Ok(None);
            }
        }
        Ok(Some(PathBuf::from("/private/var")))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (path, metadata);
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_walkers_create_and_validate_components() {
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("created").join("nested");
        let ensured = ensure_directory_path(&path, "test directory").expect("create directory");
        let resolved = fs::canonicalize(&path).expect("canonical created directory");
        assert_eq!(ensured, resolved);
        assert_eq!(
            validate_directory_path(&path, "test directory").expect("validate directory"),
            Some(resolved)
        );
        assert_eq!(
            validate_directory_path(&root.path().join("missing"), "test directory")
                .expect("inspect missing directory"),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn owner_only_directory_walker_sets_private_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("private").join("nested");
        ensure_owner_only_directory_path(&path, "private test directory")
            .expect("create private directory");
        for component in [root.path().join("private"), path] {
            let mode = fs::metadata(component)
                .expect("private directory metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o700);
        }
    }

    #[test]
    fn ordinary_directories_are_not_resolved_as_system_links() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let metadata = fs::symlink_metadata(directory.path()).expect("directory metadata");
        assert_eq!(
            resolve_system_directory_link(directory.path(), &metadata)
                .expect("inspect ordinary directory"),
            None
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn only_the_standard_var_link_is_accepted() {
        let var_metadata = fs::symlink_metadata("/var").expect("/var metadata");
        assert_eq!(
            resolve_system_directory_link(Path::new("/var"), &var_metadata).expect("resolve /var"),
            Some(PathBuf::from("/private/var"))
        );

        let etc_metadata = fs::symlink_metadata("/etc").expect("/etc metadata");
        assert_eq!(
            resolve_system_directory_link(Path::new("/etc"), &etc_metadata).expect("inspect /etc"),
            None
        );
    }
}
