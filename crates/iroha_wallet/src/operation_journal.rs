//! Descriptor-anchored immutable public evidence shared by wallet operations.
use eyre::{Result, WrapErr as _, eyre};
use norito::json::{self, JsonDeserialize, JsonSerialize};
use sha2::{Digest as _, Sha256};
use std::{
    fs::{self, File},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};

const MAX_JOURNAL_BYTES: usize = 4 * 1024 * 1024;

/// Retained private operation directory and its exclusive process lock.
pub(crate) struct Journal {
    path: PathBuf,
    #[cfg(unix)]
    directory: File,
    #[cfg(unix)]
    _lock: File,
}

impl Journal {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn create(path: &Path) -> Result<Self> {
        Self::acquire(path, true)
    }

    pub(crate) fn open(path: &Path) -> Result<Self> {
        Self::acquire(path, false)
    }

    #[cfg(unix)]
    fn acquire(path: &Path, create: bool) -> Result<Self> {
        use rustix::fs::{Mode, OFlags};
        let absolute = if path.is_absolute() {
            path.to_owned()
        } else {
            std::env::current_dir()?.join(path)
        };
        let name = absolute
            .file_name()
            .ok_or_else(|| eyre!("journal must name an operation directory"))?;
        let parent_path = absolute
            .parent()
            .ok_or_else(|| eyre!("journal has no parent directory"))?
            .canonicalize()
            .wrap_err("journal parent must already exist")?;
        let parent = File::from(rustix::fs::open(
            &parent_path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )?);
        if create {
            rustix::fs::mkdirat(&parent, name, Mode::from_raw_mode(0o700)).wrap_err(
                "journal must be a fresh directory; existing evidence is never replaced",
            )?;
            parent.sync_all()?;
        }
        let directory = File::from(
            rustix::fs::openat(
                &parent,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )
            .wrap_err("journal must be a real directory")?,
        );
        validate_private_metadata(&directory.metadata()?, true)?;
        let lock_flags = OFlags::RDWR
            | OFlags::CLOEXEC
            | OFlags::NOFOLLOW
            | OFlags::NONBLOCK
            | if create {
                OFlags::CREATE | OFlags::EXCL
            } else {
                OFlags::empty()
            };
        let lock = File::from(
            rustix::fs::openat(&directory, "lock", lock_flags, Mode::from_raw_mode(0o600))
                .wrap_err("cannot open the journal's private lock")?,
        );
        validate_private_metadata(&lock.metadata()?, false)?;
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive)
            .wrap_err("another account operation holds this journal")?;
        directory.sync_all()?;
        let result = Self {
            path: parent_path.join(name),
            directory,
            _lock: lock,
        };
        result.revalidate()?;
        Ok(result)
    }

    #[cfg(not(unix))]
    fn acquire(_: &Path, _: bool) -> Result<Self> {
        eyre::bail!(
            "durable account operation journals require Unix descriptor and file-lock support"
        )
    }

    pub(crate) fn write_operation<T: JsonSerialize>(&self, operation: &T) -> Result<()> {
        self.install("operation.json", &json::to_vec(operation)?)
    }

    pub(crate) fn read_operation<T: JsonDeserialize + JsonSerialize>(&self) -> Result<T> {
        let bytes = self.read("operation.json")?;
        let operation: T =
            json::from_slice(&bytes).wrap_err("invalid closed account-operation journal")?;
        if json::to_vec(&operation)? != bytes {
            eyre::bail!("operation journal must retain its exact canonical encoding");
        }
        Ok(operation)
    }

    fn submission_bytes<T: JsonSerialize>(operation: &T) -> Result<Vec<u8>> {
        Ok(json::to_vec(&norito::json!({
            "schema": "iroha.wallet.submission-intent.v1",
            "operation_sha256": (hex::encode(Sha256::digest(json::to_vec(operation)?)))
        }))?)
    }
    pub(crate) fn submission_recorded<T: JsonSerialize>(&self, operation: &T) -> Result<bool> {
        let bytes = Self::submission_bytes(operation)?;
        match self.read_optional("submission.json")? {
            Some(existing) if existing == bytes => Ok(true),
            Some(_) => eyre::bail!("submission marker differs from the exact retained operation"),
            None => Ok(false),
        }
    }
    pub(crate) fn record_submission<T: JsonSerialize>(&self, operation: &T) -> Result<bool> {
        if self.submission_recorded(operation)? {
            return Ok(false);
        }
        self.install("submission.json", &Self::submission_bytes(operation)?)?;
        Ok(true)
    }

    pub(crate) fn write_evidence_exact<T: JsonSerialize>(
        &self,
        name: &str,
        evidence: &T,
    ) -> Result<()> {
        if !matches!(name, "applied.json") {
            eyre::bail!("invalid wallet evidence record name");
        }
        let bytes = json::to_vec(evidence)?;
        match self.read_optional(name)? {
            Some(existing) if existing == bytes => Ok(()),
            Some(_) => eyre::bail!("retained wallet evidence differs from the exact operation"),
            None => self.install(name, &bytes),
        }
    }

    #[cfg(unix)]
    fn revalidate(&self) -> Result<()> {
        use std::os::unix::fs::MetadataExt as _;
        let current = fs::symlink_metadata(&self.path)
            .wrap_err("journal directory moved during the operation")?;
        let pinned = self.directory.metadata()?;
        validate_private_metadata(&current, true)?;
        if current.dev() != pinned.dev() || current.ino() != pinned.ino() {
            eyre::bail!("journal path no longer identifies the retained directory");
        }
        Ok(())
    }

    #[cfg(unix)]
    fn read_optional(&self, name: &str) -> Result<Option<Vec<u8>>> {
        use rustix::fs::{Mode, OFlags};
        self.revalidate()?;
        let descriptor = match rustix::fs::openat(
            &self.directory,
            name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => {
                return Err(eyre!(error)).wrap_err("cannot open private journal evidence");
            }
        };
        let mut file = File::from(descriptor);
        validate_private_metadata(&file.metadata()?, false)?;
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut file)
            .take((MAX_JOURNAL_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_JOURNAL_BYTES {
            eyre::bail!("operation journal exceeds its bounded size");
        }
        validate_private_metadata(&file.metadata()?, false)?;
        self.revalidate()?;
        Ok(Some(bytes))
    }

    fn read(&self, name: &str) -> Result<Vec<u8>> {
        self.read_optional(name)?.ok_or_else(|| {
            eyre!("journal has no completed preparation; no transaction can be submitted")
        })
    }

    #[cfg(unix)]
    fn install(&self, name: &str, bytes: &[u8]) -> Result<()> {
        use rustix::fs::{AtFlags, Mode, OFlags};
        if bytes.len() > MAX_JOURNAL_BYTES {
            eyre::bail!("operation evidence exceeds its bounded size");
        }
        self.revalidate()?;
        let temporary = format!(".pending-{}", hex::encode(rand::random::<[u8; 16]>()));
        let mut file = File::from(rustix::fs::openat(
            &self.directory,
            temporary.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::from_raw_mode(0o600),
        )?);
        let result: Result<()> = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            self.revalidate()?;
            rustix::fs::linkat(
                &self.directory,
                temporary.as_str(),
                &self.directory,
                name,
                AtFlags::empty(),
            )
            .wrap_err("immutable journal evidence already exists or could not be installed")?;
            Ok(())
        })();
        let removed = rustix::fs::unlinkat(&self.directory, temporary.as_str(), AtFlags::empty());
        self.directory.sync_all()?;
        result?;
        removed?;
        if self.read(name)? != bytes {
            eyre::bail!("saved operation evidence differs from its retained bytes");
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn read_optional(&self, _: &str) -> Result<Option<Vec<u8>>> {
        eyre::bail!("durable account journals require Unix support")
    }

    #[cfg(not(unix))]
    fn install(&self, _: &str, _: &[u8]) -> Result<()> {
        eyre::bail!("durable account journals require Unix support")
    }
}

#[cfg(unix)]
fn validate_private_metadata(metadata: &fs::Metadata, directory: bool) -> Result<()> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    if metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.permissions().mode() & 0o077 != 0
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file() || metadata.nlink() != 1
        }
    {
        eyre::bail!(
            "journal evidence must be owner-private, owned by the current user, and free of symlinks or hard links"
        );
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn journal_creation_is_exclusive_and_retained_writes_are_immutable() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("operation");
        let journal = Journal::create(&path).unwrap();
        assert!(Journal::create(&path).is_err());
        assert!(Journal::open(&path).is_err());
        journal.install("evidence.json", b"one").unwrap();
        assert!(journal.install("evidence.json", b"two").is_err());
        assert_eq!(journal.read("evidence.json").unwrap(), b"one");
        assert!(journal.read_operation::<norito::json::Value>().is_err());
        drop(journal);
        assert!(Journal::open(&path).is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn journal_rejects_links_broad_permissions_and_directory_replacement() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("operation");
        let journal = Journal::create(&path).unwrap();
        journal.install("evidence.json", b"one").unwrap();
        symlink(path.join("evidence.json"), path.join("alias.json")).unwrap();
        assert!(journal.read("alias.json").is_err());
        fs::hard_link(path.join("evidence.json"), path.join("hardlink.json")).unwrap();
        assert!(journal.read("evidence.json").is_err());
        fs::remove_file(path.join("hardlink.json")).unwrap();
        fs::set_permissions(
            path.join("evidence.json"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(journal.read("evidence.json").is_err());
        fs::rename(&path, root.path().join("moved")).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(journal.install("later.json", b"two").is_err());
        assert!(!path.join("later.json").exists());
    }

    #[test]
    fn journal_size_is_bounded_before_creation() {
        let root = tempfile::tempdir().unwrap();
        let journal = Journal::create(&root.path().join("operation")).unwrap();
        assert!(
            journal
                .install("large.json", &vec![0; MAX_JOURNAL_BYTES + 1])
                .is_err()
        );
        assert!(!journal.path().join("large.json").exists());
    }

    #[test]
    fn journal_rejects_a_special_file_substituted_for_its_lock() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("operation");
        drop(Journal::create(&path).unwrap());
        fs::remove_file(path.join("lock")).unwrap();
        let _socket = std::os::unix::net::UnixListener::bind(path.join("lock")).unwrap();
        assert!(Journal::open(&path).is_err());
    }
}
