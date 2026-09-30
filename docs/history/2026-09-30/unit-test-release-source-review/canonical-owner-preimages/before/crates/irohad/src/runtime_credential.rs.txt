//! Bounded, zeroizing supervisor credential reads shared by daemon runtime providers.
use std::{
    fs::{self, OpenOptions},
    io::Read as _,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::{Component, Path},
};
use zeroize::{Zeroize as _, Zeroizing};

/// Read one bounded secret credential from an authenticated supervisor path.
///
/// The returned allocation is zeroized on every exit path. Callers must decode
/// it immediately into secret-owning types whose `Drop` implementation also
/// scrubs private fields.
pub(crate) fn load_bounded_runtime_credential_v1(
    path: &Path,
    minimum_bytes: usize,
    maximum_bytes: usize,
) -> Result<Zeroizing<Vec<u8>>, RuntimeCredentialErrorV1> {
    if minimum_bytes == 0 || minimum_bytes > maximum_bytes {
        return Err(RuntimeCredentialErrorV1::InvalidLength);
    }
    let expected_identity = validate_credential_path(path)?;
    let named_before =
        fs::symlink_metadata(path).map_err(|_| RuntimeCredentialErrorV1::Unavailable)?;
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(
        (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC)
            .bits()
            .try_into()
            .map_err(|_| RuntimeCredentialErrorV1::InvalidSource)?,
    );
    let descriptor = options
        .open(path)
        .map_err(|_| RuntimeCredentialErrorV1::Unavailable)?;
    let opened = descriptor
        .metadata()
        .map_err(|_| RuntimeCredentialErrorV1::Unavailable)?;
    if (opened.dev(), opened.ino()) != expected_identity
        || !same_credential_metadata_v1(&named_before, &opened)
    {
        return Err(RuntimeCredentialErrorV1::InvalidSource);
    }
    let declared_bytes =
        usize::try_from(opened.len()).map_err(|_| RuntimeCredentialErrorV1::InvalidLength)?;
    if declared_bytes < minimum_bytes || declared_bytes > maximum_bytes {
        return Err(RuntimeCredentialErrorV1::InvalidLength);
    }
    // Allocate the metadata-declared credential length exactly once. Using
    // `read_to_end` here would grow a large `Zeroizing<Vec<_>>` through
    // ordinary reallocations, leaving freed secret-bearing allocations
    // outside the final zeroizing owner.
    let mut allocation = Vec::new();
    allocation
        .try_reserve_exact(declared_bytes)
        .map_err(|_| RuntimeCredentialErrorV1::Unavailable)?;
    allocation.resize(declared_bytes, 0);
    let mut bytes = Zeroizing::new(allocation);
    let mut reader = &descriptor;
    reader
        .read_exact(&mut bytes)
        .map_err(|_| RuntimeCredentialErrorV1::Unavailable)?;
    let mut trailing = [0_u8; 1];
    let trailing_len = reader
        .read(&mut trailing)
        .map_err(|_| RuntimeCredentialErrorV1::Unavailable)?;
    trailing.zeroize();
    if trailing_len != 0 {
        return Err(RuntimeCredentialErrorV1::InvalidLength);
    }
    let opened_after = descriptor
        .metadata()
        .map_err(|_| RuntimeCredentialErrorV1::Unavailable)?;
    let named_after =
        fs::symlink_metadata(path).map_err(|_| RuntimeCredentialErrorV1::Unavailable)?;
    if (opened_after.dev(), opened_after.ino()) != expected_identity
        || !same_credential_metadata_v1(&opened, &opened_after)
        || !same_credential_metadata_v1(&opened_after, &named_after)
    {
        return Err(RuntimeCredentialErrorV1::InvalidSource);
    }
    Ok(bytes)
}

fn same_credential_metadata_v1(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.is_file()
        && right.is_file()
        && left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn validate_credential_path(path: &Path) -> Result<(u64, u64), RuntimeCredentialErrorV1> {
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
    {
        return Err(RuntimeCredentialErrorV1::InvalidSource);
    }
    let euid = rustix::process::geteuid().as_raw();
    let metadata = fs::symlink_metadata(path).map_err(|_| RuntimeCredentialErrorV1::Unavailable)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || (metadata.uid() != 0 && metadata.uid() != euid)
        || metadata.mode() & 0o7077 != 0
        || metadata.mode() & 0o400 == 0
        || metadata.nlink() != 1
    {
        return Err(RuntimeCredentialErrorV1::InvalidSource);
    }
    for ancestor in path
        .parent()
        .ok_or(RuntimeCredentialErrorV1::InvalidSource)?
        .ancestors()
    {
        let metadata =
            fs::symlink_metadata(ancestor).map_err(|_| RuntimeCredentialErrorV1::Unavailable)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || (metadata.uid() != 0 && metadata.uid() != euid)
            || metadata.mode() & 0o022 != 0
        {
            return Err(RuntimeCredentialErrorV1::InvalidSource);
        }
    }
    Ok((metadata.dev(), metadata.ino()))
}
/// Payload-free bounded runtime-credential source failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeCredentialErrorV1 {
    /// Descriptor or credential path/metadata is not trusted.
    InvalidSource,
    /// Credential length is outside the caller's admitted bounds.
    InvalidLength,
    /// Credential could not be read.
    Unavailable,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    fn fixture(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        // Keep the fixture below the checkout so writable global temporary
        // ancestors cannot weaken the production credential-path policy.
        let directory = tempfile::Builder::new()
            .prefix(".runtime-credential-test-")
            .tempdir_in(std::env::current_dir().expect("checkout directory"))
            .expect("private credential directory");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private directory mode");
        let path = fs::canonicalize(directory.path())
            .expect("canonical fixture directory")
            .join("runtime-secret");
        fs::write(&path, bytes).expect("write fixture credential");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400))
            .expect("private read-only credential");
        (directory, path)
    }

    #[test]
    fn bounded_runtime_credential_reads_exact_bytes_and_checks_limits() {
        let (_directory, path) = fixture(&[0xA7; 64]);
        assert_eq!(
            load_bounded_runtime_credential_v1(&path, 32, 64)
                .expect("admitted runtime credential")
                .as_slice(),
            &[0xA7; 64]
        );
        for (minimum, maximum) in [(0, 64), (65, 64), (1, 63), (65, 128)] {
            assert!(matches!(
                load_bounded_runtime_credential_v1(&path, minimum, maximum),
                Err(RuntimeCredentialErrorV1::InvalidLength)
            ));
        }
        let (_empty_directory, empty) = fixture(&[]);
        assert!(matches!(
            load_bounded_runtime_credential_v1(&empty, 1, 64),
            Err(RuntimeCredentialErrorV1::InvalidLength)
        ));
    }

    #[test]
    fn runtime_credential_rejects_links_public_modes_and_unsafe_ancestors() {
        let (directory, path) = fixture(&[0xB7; 32]);
        let symbolic = path.with_file_name("symbolic");
        symlink(&path, &symbolic).expect("credential symlink fixture");
        assert!(matches!(
            load_bounded_runtime_credential_v1(&symbolic, 32, 32),
            Err(RuntimeCredentialErrorV1::InvalidSource)
        ));
        let hard = path.with_file_name("hard");
        fs::hard_link(&path, &hard).expect("credential hardlink fixture");
        assert!(matches!(
            load_bounded_runtime_credential_v1(&path, 32, 32),
            Err(RuntimeCredentialErrorV1::InvalidSource)
        ));
        fs::remove_file(hard).expect("remove fixture hardlink");
        for mode in [0o644, 0o440, 0o004, 0o1400] {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).expect("unsafe mode");
            assert!(matches!(
                load_bounded_runtime_credential_v1(&path, 32, 32),
                Err(RuntimeCredentialErrorV1::InvalidSource)
            ));
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).expect("restore mode");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o770))
            .expect("writable parent fixture");
        assert!(matches!(
            load_bounded_runtime_credential_v1(&path, 32, 32),
            Err(RuntimeCredentialErrorV1::InvalidSource)
        ));
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("restore private parent");
        assert!(matches!(
            validate_credential_path(Path::new("runtime-secret")),
            Err(RuntimeCredentialErrorV1::InvalidSource)
        ));
        assert!(matches!(
            validate_credential_path(directory.path()),
            Err(RuntimeCredentialErrorV1::InvalidSource)
        ));
    }

    #[test]
    fn credential_metadata_comparison_detects_identity_mode_and_length_changes() {
        let (_directory, path) = fixture(&[0xC7; 32]);
        let before = fs::metadata(&path).expect("initial metadata");
        assert!(same_credential_metadata_v1(&before, &before));
        let (_other_directory, other) = fixture(&[0xC7; 32]);
        assert!(!same_credential_metadata_v1(
            &before,
            &fs::metadata(other).expect("other inode")
        ));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("mode change");
        let changed_mode = fs::metadata(&path).expect("changed mode metadata");
        assert!(!same_credential_metadata_v1(&before, &changed_mode));
        fs::write(&path, [0xC7; 33]).expect("length change");
        assert!(!same_credential_metadata_v1(
            &changed_mode,
            &fs::metadata(path).expect("changed length")
        ));
    }
}
