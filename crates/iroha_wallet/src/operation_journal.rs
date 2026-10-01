//! Descriptor-anchored immutable public evidence shared by wallet operations.
//!
//! A journal is one owner-private directory holding, in order:
//! - `operation.json`: the exact prepared operation (for example a signed
//!   transaction), published atomically by [`Journal::create_prepared`];
//! - `submission.json`: a durable marker recorded by
//!   [`Journal::record_submission`] *before* the only dispatch, so a crash can
//!   never cause a blind resubmission;
//! - `applied.json`: the exact committed evidence, written once with
//!   [`Journal::write_applied_evidence`].
//!
//! Every record is immutable: rewriting different bytes fails. The directory is
//! pinned by descriptor, exclusively locked while open, and rejects links and
//! group/other permissions. The deployment engine's exact-wire canary writes
//! (`iroha_deploy` gate G6, P2) are meant to use the same journal.
use eyre::{Result, WrapErr as _, eyre};
use iroha_fs::{FileIdentity, OwnerDirectory, PrivateDirectory, PublishMode};
use norito::json::{self, JsonDeserialize, JsonSerialize};
use sha2::{Digest as _, Sha256};
use std::{
    fs::File,
    path::{Path, PathBuf},
};

const MAX_JOURNAL_BYTES: usize = 4 * 1024 * 1024;
const APPLIED_EVIDENCE: &str = "applied.json";

/// Retained private operation directory and its exclusive native process lock.
pub struct Journal {
    path: PathBuf,
    directory: PrivateDirectory,
    lock: File,
}

impl core::fmt::Debug for Journal {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Journal")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Journal {
    /// Canonical absolute path of the pinned journal directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reserve an unprepared journal and hold its exclusive lock.
    /// Callers using this reservation API must explicitly recover a provably unprepared journal
    /// after interruption. When the operation already exists, use [`Self::create_prepared`].
    ///
    /// # Errors
    /// Refuses missing or unsafe parents, existing directories, and native custody failures.
    pub fn create(path: &Path) -> Result<Self> {
        Self::acquire(path, true)
    }

    /// Atomically publish the complete original operation and lock, then acquire the journal.
    /// No partial journal can appear at `path` between directory creation and retaining the signed
    /// bytes. An uncertain publication is reconciled by reopening this same path, never preparing
    /// or submitting a replacement transaction. Existing evidence is never overwritten.
    ///
    /// # Errors
    /// Invalid/oversized operation, unsafe or missing parent, existing destination, competing owner,
    /// or native publication failure. Private staging siblings can remain after interruption.
    pub fn create_prepared<T: JsonSerialize>(path: &Path, operation: &T) -> Result<Self> {
        let bytes = json::to_vec(operation)?;
        if bytes.len() > MAX_JOURNAL_BYTES {
            eyre::bail!("operation exceeds the journal byte bound");
        }
        let absolute = if path.is_absolute() {
            path.to_owned()
        } else {
            std::env::current_dir()?.join(path)
        };
        let name = absolute
            .file_name()
            .ok_or_else(|| eyre!("journal must name an operation directory"))?;
        let parent = OwnerDirectory::open(
            absolute
                .parent()
                .ok_or_else(|| eyre!("journal has no parent"))?,
        )?;
        let directory =
            parent.publish_private_child(name, &[("lock", &[]), ("operation.json", &bytes)])?;
        Self::lock_directory(directory, false)
    }

    /// Reopen an existing journal and hold its exclusive lock.
    ///
    /// # Errors
    /// Refuses missing, unsafe or replaced directories and locks, and competing owners.
    pub fn open(path: &Path) -> Result<Self> {
        Self::acquire(path, false)
    }

    fn acquire(path: &Path, create: bool) -> Result<Self> {
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
        let parent = OwnerDirectory::open(&parent_path)?;
        let directory = if create {
            parent.create_private_child(name).wrap_err(
                "journal must be a fresh directory; existing evidence is never replaced",
            )?
        } else {
            PrivateDirectory::open(parent.path().join(name))?
        };
        Self::lock_directory(directory, create)
    }

    fn lock_directory(directory: PrivateDirectory, create: bool) -> Result<Self> {
        let lock = if create {
            directory.create_lock("lock")?
        } else {
            directory.open_existing_lock("lock")?
        };
        lock.try_lock()
            .wrap_err("another account operation holds this journal")?;
        directory.sync()?;
        let result = Self {
            path: directory.path().to_owned(),
            directory,
            lock,
        };
        result.revalidate()?;
        Ok(result)
    }

    /// Retain the exact canonical JSON of a prepared operation, once.
    ///
    /// # Errors
    /// Fails when an operation is already retained or the bytes exceed the bound.
    pub fn write_operation<T: JsonSerialize>(&self, operation: &T) -> Result<()> {
        self.install("operation.json", &json::to_vec(operation)?)
    }

    /// Read the retained operation, requiring its exact canonical encoding.
    ///
    /// # Errors
    /// Fails when no operation was prepared or the retained bytes are not the
    /// canonical encoding of `T`.
    pub fn read_operation<T: JsonDeserialize + JsonSerialize>(&self) -> Result<T> {
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
    /// Whether the durable pre-dispatch marker for exactly `operation` exists.
    ///
    /// # Errors
    /// Fails when a marker exists for different operation bytes.
    pub fn submission_recorded<T: JsonSerialize>(&self, operation: &T) -> Result<bool> {
        let bytes = Self::submission_bytes(operation)?;
        match self.read_optional("submission.json")? {
            Some(existing) if existing == bytes => Ok(true),
            Some(_) => eyre::bail!("submission marker differs from the exact retained operation"),
            None => Ok(false),
        }
    }
    /// Durably record the pre-dispatch marker for `operation`.
    ///
    /// Returns `true` only for the call that created the marker; that caller
    /// alone may dispatch. `false` means an earlier attempt exists and the
    /// operation must be reconciled, never resubmitted.
    ///
    /// # Errors
    /// Fails when a marker exists for different bytes or cannot be installed.
    pub fn record_submission<T: JsonSerialize>(&self, operation: &T) -> Result<bool> {
        if self.submission_recorded(operation)? {
            return Ok(false);
        }
        self.install("submission.json", &Self::submission_bytes(operation)?)?;
        Ok(true)
    }

    /// Retain the exact committed evidence (`applied.json`); an identical
    /// rewrite is accepted, different bytes are rejected.
    ///
    /// # Errors
    /// Fails when different evidence is retained or the bytes exceed the bound.
    pub fn write_applied_evidence<T: JsonSerialize>(&self, evidence: &T) -> Result<()> {
        let bytes = json::to_vec(evidence)?;
        match self.read_optional(APPLIED_EVIDENCE)? {
            Some(existing) if existing == bytes => Ok(()),
            Some(_) => eyre::bail!("retained wallet evidence differs from the exact operation"),
            None => self.install(APPLIED_EVIDENCE, &bytes),
        }
    }

    fn revalidate(&self) -> Result<()> {
        self.directory.revalidate()?;
        let current = self.directory.open_read("lock")?;
        if FileIdentity::of(&current)? != FileIdentity::of(&self.lock)? {
            eyre::bail!("journal lock no longer identifies the retained ownership file");
        }
        Ok(())
    }

    fn read_optional(&self, name: &str) -> Result<Option<Vec<u8>>> {
        self.revalidate()?;
        match self.directory.read(name, MAX_JOURNAL_BYTES) {
            Ok(bytes) => {
                self.revalidate()?;
                Ok(Some(bytes.to_vec()))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).wrap_err("cannot read private journal evidence"),
        }
    }

    fn read(&self, name: &str) -> Result<Vec<u8>> {
        self.read_optional(name)?.ok_or_else(|| {
            eyre!("journal has no completed preparation; no transaction can be submitted")
        })
    }

    fn install(&self, name: &str, bytes: &[u8]) -> Result<()> {
        if bytes.len() > MAX_JOURNAL_BYTES {
            eyre::bail!("operation evidence exceeds its bounded size");
        }
        self.revalidate()?;
        self.directory
            .write_atomic(name, bytes, PublishMode::CreateNew)
            .wrap_err("immutable journal evidence already exists or could not be installed")?;
        if self.read(name)? != bytes {
            eyre::bail!("saved operation evidence differs from its retained bytes");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn prepared_creation_atomically_retains_original_bytes_and_survives_reopen() {
        let root = tempfile::tempdir().unwrap();
        let parent = OwnerDirectory::open(root.path()).unwrap();
        let interrupted = parent.create_private_child("unpublished").unwrap();
        interrupted.create_lock("lock").unwrap().sync_all().unwrap();
        let path = root.path().join("operation");
        let operation = norito::json!({"signed": "original"});
        let journal = Journal::create_prepared(&path, &operation).unwrap();
        assert_eq!(
            journal.read_operation::<norito::json::Value>().unwrap(),
            operation
        );
        assert!(!journal.submission_recorded(&operation).unwrap());
        assert!(
            Journal::create_prepared(&path, &norito::json!({"signed": "replacement"})).is_err()
        );
        drop(journal);
        let journal = Journal::open(&path).unwrap();
        assert_eq!(
            journal.read_operation::<norito::json::Value>().unwrap(),
            operation
        );
        assert!(journal.record_submission(&operation).unwrap());
        assert!(!journal.record_submission(&operation).unwrap());
        let absent = root.path().join("too-large");
        assert!(Journal::create_prepared(&absent, &"x".repeat(MAX_JOURNAL_BYTES)).is_err());
        assert!(!absent.exists());
    }

    #[test]
    fn journal_open_never_recreates_a_missing_lock() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("operation");
        drop(Journal::create(&path).unwrap());
        fs::remove_file(path.join("lock")).unwrap();
        assert!(Journal::open(&path).is_err());
        assert!(!path.join("lock").exists());
    }

    #[test]
    #[cfg(unix)]
    fn retained_journal_refuses_a_replaced_lock_identity() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("operation");
        let journal = Journal::create(&path).unwrap();
        fs::rename(path.join("lock"), path.join("old-lock")).unwrap();
        let replacement = journal.directory.create_lock("lock").unwrap();
        assert!(
            journal
                .write_operation(&norito::json!({"wire": "no"}))
                .is_err()
        );
        assert!(!path.join("operation.json").exists());
        drop(replacement);
    }

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
    fn public_protocol_records_one_operation_marker_and_exact_evidence() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("canary");
        let journal = Journal::create(&path).unwrap();
        assert!(format!("{journal:?}").contains("canary"));
        let operation = norito::json!({"schema": "test.canary.v1", "wire": "00ff"});
        journal.write_operation(&operation).unwrap();
        assert!(
            journal.write_operation(&operation).is_err(),
            "operations are written once"
        );
        assert_eq!(
            journal.read_operation::<norito::json::Value>().unwrap(),
            operation
        );
        assert!(!journal.submission_recorded(&operation).unwrap());
        assert!(
            journal.record_submission(&operation).unwrap(),
            "first marker owns dispatch"
        );
        assert!(
            !journal.record_submission(&operation).unwrap(),
            "later calls never dispatch"
        );
        assert!(journal.submission_recorded(&operation).unwrap());
        let other = norito::json!({"schema": "test.canary.v1", "wire": "0100"});
        assert!(journal.submission_recorded(&other).is_err());
        let evidence = norito::json!({"height": 7});
        journal.write_applied_evidence(&evidence).unwrap();
        journal.write_applied_evidence(&evidence).unwrap();
        assert!(
            journal
                .write_applied_evidence(&norito::json!({"height": 8}))
                .is_err()
        );
        assert!(path.join(APPLIED_EVIDENCE).is_file());
        drop(journal);
        let reopened = Journal::open(&path).unwrap();
        assert_eq!(reopened.path(), path.canonicalize().unwrap());
        assert!(reopened.submission_recorded(&operation).unwrap());
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
    #[cfg(unix)]
    fn journal_rejects_a_special_file_substituted_for_its_lock() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("operation");
        drop(Journal::create(&path).unwrap());
        fs::remove_file(path.join("lock")).unwrap();
        let _socket = std::os::unix::net::UnixListener::bind(path.join("lock")).unwrap();
        assert!(Journal::open(&path).is_err());
    }
}
