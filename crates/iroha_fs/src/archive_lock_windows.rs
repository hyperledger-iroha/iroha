//! Exclusive private originals for serialized archive writers.

use super::*;

impl OwnerDirectory {
    /// Open or create a protected private archive lock with no sharing permissions.
    ///
    /// The original excludes other opens requesting data read, write or deletion. Metadata
    /// observations remain possible. This does not truncate or repair an existing file.
    /// The caller retains this original for its complete serialized archive lifetime.
    ///
    /// # Errors
    /// Refuses unsafe names, nonprivate custody, links, another writer and native errors.
    pub fn open_exclusive_private_lock(&self, name: impl AsRef<OsStr>) -> io::Result<File> {
        self.inner
            .open_exclusive_private_lock(checked_name(name.as_ref())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_archive_lock_reopens_only_after_original_closes() {
        let temporary = tempfile::tempdir().unwrap();
        let owner = OwnerDirectory::open_or_create(temporary.path().join("archive")).unwrap();
        let original = owner.open_exclusive_private_lock(".writer.lock").unwrap();
        let identity = FileMetadata::of(&original).unwrap().identity();
        assert!(owner.open_exclusive_private_lock(".writer.lock").is_err());
        assert!(
            std::fs::OpenOptions::new()
                .read(true)
                .open(owner.path().join(".writer.lock"))
                .is_err()
        );
        assert!(std::fs::remove_file(owner.path().join(".writer.lock")).is_err());
        assert_eq!(
            FileMetadata::symlink_metadata(owner.path().join(".writer.lock"))
                .unwrap()
                .identity(),
            identity
        );
        drop(original);
        let reopened = owner.open_exclusive_private_lock(".writer.lock").unwrap();
        assert_eq!(FileMetadata::of(&reopened).unwrap().identity(), identity);
    }

    #[test]
    fn exclusive_archive_lock_refuses_links_and_nonprivate_originals() {
        let temporary = tempfile::tempdir().unwrap();
        let owner = OwnerDirectory::open_or_create(temporary.path().join("archive")).unwrap();
        let file = owner.open_exclusive_private_lock("original").unwrap();
        drop(file);
        std::fs::hard_link(owner.path().join("original"), owner.path().join("alias")).unwrap();
        assert!(owner.open_exclusive_private_lock("original").is_err());
        std::fs::write(owner.path().join("public"), b"unchanged").unwrap();
        assert!(owner.open_exclusive_private_lock("public").is_err());
        assert_eq!(
            std::fs::read(owner.path().join("public")).unwrap(),
            b"unchanged"
        );
    }
}
