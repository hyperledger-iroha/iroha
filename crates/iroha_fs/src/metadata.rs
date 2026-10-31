//! Policy-neutral descriptor metadata with stable native identities.

use crate::FileIdentity;
use std::{
    fs::{File, Metadata},
    io,
    ops::Deref,
};

/// One descriptor's ordinary metadata, complete native identity and hard-link count.
///
/// This inspection grants no ownership, pathname, reparse or publication authority. Callers
/// retain their own admission policy. Windows uses the full 64-bit volume and 128-bit file ID;
/// it never substitutes the shorter historical file index available through older APIs.
#[derive(Debug, Clone)]
pub struct FileMetadata {
    ordinary: Metadata,
    identity: FileIdentity,
    links: u64,
}

impl FileMetadata {
    /// Inspect one retained file or directory without resolving its pathname.
    ///
    /// # Errors
    /// Returns native inspection errors or `WouldBlock` if Windows metadata changes across
    /// the native and standard-library observations.
    pub fn of(file: &File) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let ordinary = file.metadata()?;
            let mut object = [0; 16];
            object[..8].copy_from_slice(&ordinary.ino().to_le_bytes());
            let identity = FileIdentity {
                volume: ordinary.dev(),
                object,
            };
            let links = ordinary.nlink();
            Ok(Self {
                ordinary,
                identity,
                links,
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt as _;
            let before = crate::platform::metadata_snapshot(file)?;
            let ordinary = file.metadata()?;
            let after = crate::platform::metadata_snapshot(file)?;
            if before != after
                || ordinary.len() != after.1
                || ordinary.file_attributes() != after.3
                || ordinary.creation_time() != after.4.cast_unsigned()
                || ordinary.last_write_time() != after.5.cast_unsigned()
            {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "native descriptor metadata changed during inspection",
                ));
            }
            Ok(Self {
                ordinary,
                identity: after.0,
                links: after.2,
            })
        }
    }

    /// Inspect one named entry without following its final symbolic link or reparse point.
    ///
    /// Parent components are ordinary pathname resolution; this observation supplies no retained
    /// ancestry authority. Callers must reject inadmissible entry kinds and bind their own handles.
    ///
    /// # Errors
    /// Returns native open/inspection errors or the same metadata race errors as [`Self::of`].
    pub fn symlink_metadata(path: impl AsRef<std::path::Path>) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let ordinary = std::fs::symlink_metadata(path)?;
            let mut object = [0; 16];
            object[..8].copy_from_slice(&ordinary.ino().to_le_bytes());
            let identity = FileIdentity {
                volume: ordinary.dev(),
                object,
            };
            let links = ordinary.nlink();
            Ok(Self {
                ordinary,
                identity,
                links,
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt as _;
            let mut options = std::fs::OpenOptions::new();
            // FILE_READ_ATTRIBUTES; share read/write/delete; OPEN_REPARSE_POINT and BACKUP_SEMANTICS.
            options
                .access_mode(0x80)
                .share_mode(0x7)
                .custom_flags(0x0220_0000);
            Self::of(&options.open(path)?)
        }
    }

    /// Return the complete native volume and object identity.
    #[must_use]
    pub const fn identity(&self) -> FileIdentity {
        self.identity
    }

    /// Return the native count of names linked to this object.
    #[must_use]
    pub const fn links(&self) -> u64 {
        self.links
    }

    /// Borrow the ordinary metadata observed inside the native snapshot.
    #[must_use]
    pub const fn metadata(&self) -> &Metadata {
        &self.ordinary
    }
}

impl Deref for FileMetadata {
    type Target = Metadata;
    fn deref(&self) -> &Self::Target {
        &self.ordinary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_metadata_preserves_identity_and_observes_new_hard_links() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source");
        std::fs::write(&source, b"metadata").unwrap();
        let file = File::open(&source).unwrap();
        let original = FileMetadata::of(&file).unwrap();
        assert_eq!(original.identity(), FileIdentity::of(&file).unwrap());
        let named = FileMetadata::symlink_metadata(&source).unwrap();
        assert_eq!(named.identity(), original.identity());
        assert_eq!(named.links(), original.links());
        assert_eq!(
            FileMetadata::symlink_metadata(temporary.path().join("missing"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(original.links(), 1);
        assert_eq!(original.len(), 8);
        assert!(original.metadata().is_file());
        std::fs::hard_link(&source, temporary.path().join("alias")).unwrap();
        let linked = FileMetadata::of(&file).unwrap();
        assert_eq!(linked.identity(), original.identity());
        assert_eq!(linked.links(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn named_metadata_retains_the_final_symlink_identity() {
        let temporary = tempfile::tempdir().unwrap();
        let target = temporary.path().join("target");
        let link = temporary.path().join("link");
        std::fs::write(&target, b"target").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let target = FileMetadata::symlink_metadata(target).unwrap();
        let link = FileMetadata::symlink_metadata(link).unwrap();
        assert!(link.file_type().is_symlink());
        assert_ne!(link.identity(), target.identity());
    }

    #[test]
    fn retained_metadata_inspects_directories_without_private_file_policy() {
        let temporary = tempfile::tempdir().unwrap();
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt as _;
            options.custom_flags(0x0200_0000);
        }
        let directory = options.open(temporary.path()).unwrap();
        let metadata = FileMetadata::of(&directory).unwrap();
        assert!(metadata.is_dir());
        assert_eq!(metadata.identity(), FileIdentity::of(&directory).unwrap());
    }
}
