//! Small cross-platform primitives for crash-consistent Torii persistence.

use std::{fs, io, path::Path};

fn invalid_directory(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn metadata_is_direct_directory(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
            && metadata.volume_serial_number().is_some()
            && metadata.file_index().is_some();
    }
    #[cfg(not(windows))]
    true
}

#[cfg(unix)]
fn metadata_identifies_same_directory(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    metadata_is_direct_directory(left)
        && metadata_is_direct_directory(right)
        && left.dev() == right.dev()
        && left.ino() == right.ino()
}

#[cfg(windows)]
fn metadata_identifies_same_directory(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    metadata_is_direct_directory(left)
        && metadata_is_direct_directory(right)
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
}

#[cfg(not(any(unix, windows)))]
fn metadata_identifies_same_directory(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

/// Flush a direct directory and verify that its pathname continued to identify the opened handle.
///
/// This is the durability boundary for atomic rename and unlink operations. Unsupported targets
/// fail closed instead of claiming that a namespace mutation reached stable storage.
pub(crate) fn sync_direct_directory(path: &Path) -> io::Result<()> {
    let named_before = fs::symlink_metadata(path)?;
    if !metadata_is_direct_directory(&named_before) {
        return Err(invalid_directory(
            "durability path is not a direct directory",
        ));
    }

    let mut options = fs::OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;

        options.read(true).custom_flags(
            libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        );
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        // `File::sync_all` maps to `FlushFileBuffers`, which needs a write-capable handle.
        options
            .write(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = options;
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "directory synchronization is unsupported on this platform",
        ));
    }

    let directory = options.open(path)?;
    let opened_before = directory.metadata()?;
    if !metadata_identifies_same_directory(&named_before, &opened_before) {
        return Err(invalid_directory(
            "durability directory changed while it was opened",
        ));
    }
    directory.sync_all()?;
    let opened_after = directory.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    if !metadata_identifies_same_directory(&opened_before, &opened_after)
        || !metadata_identifies_same_directory(&opened_after, &named_after)
    {
        return Err(invalid_directory(
            "durability directory changed while it was synchronized",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(unix, windows))]
    #[test]
    fn direct_directory_sync_rejects_regular_files() {
        let directory = tempfile::tempdir().expect("temporary durability directory");
        sync_direct_directory(directory.path()).expect("sync direct directory");

        let regular_file = directory.path().join("regular-file");
        fs::write(&regular_file, b"not a directory").expect("write regular file");
        assert!(sync_direct_directory(&regular_file).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn direct_directory_sync_rejects_symlinks() {
        use std::os::unix::fs::symlink;

        let target = tempfile::tempdir().expect("temporary durability target");
        let holder = tempfile::tempdir().expect("temporary symlink holder");
        let link = holder.path().join("directory-link");
        symlink(target.path(), &link).expect("create directory symlink");

        assert!(sync_direct_directory(&link).is_err());
    }
}
