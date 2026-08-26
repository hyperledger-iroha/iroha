//! Stable filesystem metadata snapshots for security-sensitive path checks.

#[cfg(not(windows))]
use std::{fs, io, path::Path};

#[cfg(not(windows))]
pub(crate) type SecureMetadata = fs::Metadata;

#[cfg(not(windows))]
pub(crate) fn from_file(file: &fs::File) -> io::Result<SecureMetadata> {
    file.metadata()
}

#[cfg(not(windows))]
pub(crate) fn from_path(path: &Path) -> io::Result<SecureMetadata> {
    fs::symlink_metadata(path)
}

#[cfg(windows)]
mod windows {
    use iroha_file_mmap::{WindowsFileInformation, windows_file_information};
    use std::{
        fs::{self, OpenOptions},
        io,
        ops::Deref,
        os::windows::fs::OpenOptionsExt as _,
        path::Path,
    };

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_SHARE_READ_WRITE_DELETE: u32 = 0x0000_0001 | 0x0000_0002 | 0x0000_0004;

    /// Standard metadata bound to stable Win32 identity, revision, and link fields.
    #[derive(Clone, Debug)]
    pub(crate) struct SecureMetadata {
        metadata: fs::Metadata,
        information: WindowsFileInformation,
    }

    impl SecureMetadata {
        pub(crate) const fn volume_serial_number(&self) -> Option<u32> {
            Some(self.information.volume_serial_number())
        }

        pub(crate) const fn file_index(&self) -> Option<u64> {
            Some(self.information.file_index())
        }

        pub(crate) const fn number_of_links(&self) -> Option<u32> {
            Some(self.information.number_of_links())
        }

        pub(crate) const fn file_attributes(&self) -> u32 {
            self.information.file_attributes()
        }

        pub(crate) const fn creation_time(&self) -> u64 {
            self.information.creation_time()
        }

        pub(crate) const fn last_write_time(&self) -> u64 {
            self.information.last_write_time()
        }

        pub(crate) const fn file_size(&self) -> u64 {
            self.information.file_size()
        }
    }

    impl Deref for SecureMetadata {
        type Target = fs::Metadata;

        fn deref(&self) -> &Self::Target {
            &self.metadata
        }
    }

    pub(crate) fn from_file(file: &fs::File) -> io::Result<SecureMetadata> {
        let information_before = windows_file_information(file)?;
        let metadata = file.metadata()?;
        let information_after = windows_file_information(file)?;
        if information_before != information_after {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "file metadata changed while it was sampled",
            ));
        }
        Ok(SecureMetadata {
            metadata,
            information: information_after,
        })
    }

    pub(crate) fn from_path(path: &Path) -> io::Result<SecureMetadata> {
        let file = OpenOptions::new()
            .access_mode(0)
            .share_mode(FILE_SHARE_READ_WRITE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        from_file(&file)
    }

    #[cfg(test)]
    mod tests {
        use std::io::Write as _;

        use super::*;

        #[test]
        fn stable_information_tracks_identity_and_hard_links() {
            let directory = tempfile::tempdir().expect("create temporary directory");
            assert!(
                from_path(directory.path())
                    .expect("sample temporary directory")
                    .is_dir()
            );
            let mut file = tempfile::NamedTempFile::new_in(directory.path())
                .expect("create writable named temporary file");
            file.write_all(b"metadata").expect("write test file");
            file.flush().expect("flush test file");

            let opened = from_file(file.as_file()).expect("sample open file");
            let named = from_path(file.path()).expect("sample named file");
            assert_eq!(opened.volume_serial_number(), named.volume_serial_number());
            assert_eq!(opened.file_index(), named.file_index());
            assert_eq!(opened.number_of_links(), Some(1));
            assert_eq!(opened.file_size(), 8);

            let link = directory.path().join("metadata.hard-link");
            fs::hard_link(file.path(), &link).expect("create hard link");
            assert_eq!(
                from_file(file.as_file())
                    .expect("resample linked file")
                    .number_of_links(),
                Some(2)
            );
        }
    }
}

#[cfg(windows)]
pub(crate) use windows::{SecureMetadata, from_file, from_path};
