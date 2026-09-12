//! Snapshot installation transactions, durable journals, and startup recovery.
//!
//! The supervisor retains generation validation and peer lifecycle coordination.
//! This owner stages mutable storage and logs, rolls back failures, and preserves
//! uncertain commit state for deterministic recovery under supervisor ownership.

use super::{
    PeerHandle, Result, SupervisorError, copy_dir_recursive, copy_snapshot_file, hash_directory,
    normalized_relative_path, read_snapshot_file_bounded, sync_managed_directory,
};
use iroha_crypto::Hash;
use norito::json::{self, Map, Value};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;
use std::{
    collections::{HashMap, HashSet},
    ffi::{OsStr, OsString},
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub(super) const SNAPSHOT_RESTORE_JOURNAL_FILE_NAME: &str = ".snapshot-restore-v1.json";
pub(super) const SNAPSHOT_RESTORE_COMMIT_FILE_NAME: &str = ".snapshot-restore-v1.committed";
const SNAPSHOT_RESTORE_JOURNAL_MAX_BYTES_V1: usize = 256 * 1024;

pub(super) struct StagedPeerRestore {
    pub(super) alias: String,
    pub(super) live_storage: PathBuf,
    pub(super) staged_storage: PathBuf,
    pub(super) backup_storage: PathBuf,
    pub(super) live_log: PathBuf,
    pub(super) staged_log: Option<PathBuf>,
    pub(super) backup_log: PathBuf,
    pub(super) original_log_present: bool,
    pub(super) log_touched: bool,
    pub(super) storage_backed_up: bool,
    pub(super) storage_installed: bool,
    pub(super) log_backed_up: bool,
    pub(super) log_installed: bool,
}
pub(super) struct SnapshotRestoreTransaction {
    pub(super) network_root: PathBuf,
    pub(super) journal_path: PathBuf,
    pub(super) commit_marker_path: PathBuf,
    pub(super) peers: Vec<StagedPeerRestore>,
    pub(super) committed: bool,
    pub(super) preserve_backups: bool,
}
pub(super) enum SnapshotRestoreApplyFailure {
    RolledBack(SupervisorError),
    RollbackFailed {
        primary: SupervisorError,
        rollback: io::Error,
    },
}
#[derive(Debug, thiserror::Error)]
pub(super) enum SnapshotRestoreCommitFailure {
    #[error("snapshot restore commit marker was not published: {source}")]
    NotPublished {
        #[source]
        source: io::Error,
    },
    #[error(
        "snapshot restore commit marker publication at `{path}` is uncertain: {source}; marker cleanup could not be made durable: {cleanup}"
    )]
    PublicationUncertain {
        path: PathBuf,
        #[source]
        source: io::Error,
        cleanup: io::Error,
    },
}
fn restore_relative_path(network_root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(network_root).map_err(|_| {
        SupervisorError::Config(format!(
            "snapshot restore path `{}` escapes network root `{}`",
            path.display(),
            network_root.display()
        ))
    })?;
    if relative.as_os_str().is_empty()
        || !relative
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(SupervisorError::Config(format!(
            "snapshot restore path `{}` is not a canonical relative path",
            path.display()
        )));
    }
    normalized_relative_path(network_root, path).map_err(Into::into)
}
pub(super) fn write_pending_restore_journal(
    transaction: &SnapshotRestoreTransaction,
) -> Result<()> {
    for path in [&transaction.journal_path, &transaction.commit_marker_path] {
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(SupervisorError::Config(format!(
                    "unfinished snapshot restore marker `{}` already exists",
                    path.display()
                )));
            }
            Err(error) => return Err(error.into()),
        }
    }
    let mut peers = Vec::with_capacity(transaction.peers.len());
    for peer in &transaction.peers {
        let mut object = Map::new();
        object.insert("alias".to_owned(), Value::String(peer.alias.clone()));
        for (field, path) in [
            ("live_storage", &peer.live_storage),
            ("staged_storage", &peer.staged_storage),
            ("backup_storage", &peer.backup_storage),
            ("live_log", &peer.live_log),
            ("backup_log", &peer.backup_log),
        ] {
            object.insert(
                field.to_owned(),
                Value::String(restore_relative_path(&transaction.network_root, path)?),
            );
        }
        object.insert(
            "staged_log".to_owned(),
            match peer.staged_log.as_ref() {
                Some(path) => {
                    Value::String(restore_relative_path(&transaction.network_root, path)?)
                }
                None => Value::Null,
            },
        );
        object.insert(
            "original_log_present".to_owned(),
            Value::Bool(peer.original_log_present),
        );
        peers.push(Value::Object(object));
    }
    let mut journal = Map::new();
    journal.insert("version".to_owned(), Value::Number(1_u64.into()));
    journal.insert("peers".to_owned(), Value::Array(peers));
    let encoded = json::to_json_bounded(
        &Value::Object(journal),
        SNAPSHOT_RESTORE_JOURNAL_MAX_BYTES_V1,
    )
    .map_err(|error| {
        SupervisorError::Config(format!(
            "snapshot restore journal exceeds its first-release byte budget: {error}"
        ))
    })?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&transaction.journal_path)?;
    let write = (|| -> io::Result<()> {
        file.write_all(encoded.as_bytes())?;
        file.sync_all()?;
        sync_managed_directory(&transaction.network_root)
    })();
    if write.is_err() {
        let _ = fs::remove_file(&transaction.journal_path);
        let _ = sync_managed_directory(&transaction.network_root);
    }
    write.map_err(Into::into)
}
pub(super) fn write_restore_commit_marker(
    path: &Path,
    network_root: &Path,
) -> std::result::Result<(), SnapshotRestoreCommitFailure> {
    write_restore_commit_marker_with(
        path,
        network_root,
        |marker| fs::remove_file(marker),
        sync_managed_directory,
    )
}
pub(super) fn write_restore_commit_marker_with(
    path: &Path,
    network_root: &Path,
    mut remove_marker: impl FnMut(&Path) -> io::Result<()>,
    mut sync_directory: impl FnMut(&Path) -> io::Result<()>,
) -> std::result::Result<(), SnapshotRestoreCommitFailure> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut marker = match options.open(path) {
        Ok(marker) => marker,
        Err(source) => {
            return match fs::symlink_metadata(path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    Err(SnapshotRestoreCommitFailure::NotPublished { source })
                }
                Ok(_) => Err(SnapshotRestoreCommitFailure::PublicationUncertain {
                    path: path.to_path_buf(),
                    source,
                    cleanup: io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "a filesystem entry occupies the commit marker path after create_new failed",
                    ),
                }),
                Err(cleanup) => Err(SnapshotRestoreCommitFailure::PublicationUncertain {
                    path: path.to_path_buf(),
                    source,
                    cleanup,
                }),
            };
        }
    };
    let write = (|| {
        marker.write_all(b"committed\n")?;
        marker.sync_all()?;
        sync_directory(network_root)
    })();
    drop(marker);
    let Err(source) = write else {
        return Ok(());
    };
    let cleanup = match remove_marker(path) {
        Ok(()) => sync_directory(network_root),
        Err(error) if error.kind() == io::ErrorKind::NotFound => sync_directory(network_root),
        Err(error) => Err(error),
    };
    match cleanup {
        Ok(()) => Err(SnapshotRestoreCommitFailure::NotPublished { source }),
        Err(cleanup) => Err(SnapshotRestoreCommitFailure::PublicationUncertain {
            path: path.to_path_buf(),
            source,
            cleanup,
        }),
    }
}
fn exact_restore_journal_object<'a>(
    value: &'a Value,
    label: &str,
    expected_fields: &[&str],
) -> Result<&'a Map> {
    let object = value
        .as_object()
        .ok_or_else(|| SupervisorError::Config(format!("{label} must be a JSON object")))?;
    if object.len() != expected_fields.len()
        || !expected_fields
            .iter()
            .all(|field| object.contains_key(*field))
    {
        return Err(SupervisorError::Config(format!(
            "{label} must contain exactly: {}",
            expected_fields.join(", ")
        )));
    }
    Ok(object)
}
fn decode_restore_journal_path(network_root: &Path, value: &Value, label: &str) -> Result<PathBuf> {
    let raw = value
        .as_str()
        .ok_or_else(|| SupervisorError::Config(format!("{label} must be a string")))?;
    let relative = Path::new(raw);
    if raw.is_empty()
        || relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(SupervisorError::Config(format!(
            "{label} must be a canonical relative path"
        )));
    }
    Ok(network_root.join(relative))
}
fn restore_sibling_matches(live: &Path, candidate: &Path, role: &str) -> bool {
    if candidate.parent() != live.parent() {
        return false;
    }
    let Some(live_name) = live.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    let Some(candidate_name) = candidate.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    candidate_name.starts_with(&format!(".{live_name}.mochi-restore-{role}."))
}
fn is_canonical_restore_generation_id(value: &OsStr) -> bool {
    value.to_str().is_some_and(|value| {
        value.len() == 32
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
fn require_restore_directory(path: &Path, label: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(SupervisorError::Config(format!(
                "{label} `{}` must be a real directory",
                path.display()
            )))
        }
        Ok(_) => Ok(()),
        Err(error) => Err(error.into()),
    }
}
fn validate_restore_peer_ancestors(network_root: &Path, alias: &str) -> Result<()> {
    let peers = network_root.join("peers");
    let peer = peers.join(alias);
    let storage_generations = peer.join("storage-generations");
    let logs = network_root.join("logs");
    for (path, label) in [
        (&peers, "snapshot restore peers ancestor"),
        (&peer, "snapshot restore peer ancestor"),
        (
            &storage_generations,
            "snapshot restore storage-generations ancestor",
        ),
        (&logs, "snapshot restore logs ancestor"),
    ] {
        require_restore_directory(path, label)?;
    }
    Ok(())
}
fn validate_recovered_restore_peer(network_root: &Path, peer: &StagedPeerRestore) -> Result<()> {
    if peer.alias.is_empty()
        || Path::new(&peer.alias).components().count() != 1
        || !matches!(
            Path::new(&peer.alias).components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        return Err(SupervisorError::Config(
            "snapshot restore journal contains an invalid peer alias".to_owned(),
        ));
    }
    validate_restore_peer_ancestors(network_root, &peer.alias)?;
    let storage_relative = peer.live_storage.strip_prefix(network_root).map_err(|_| {
        SupervisorError::Config("snapshot restore storage escapes the network root".to_owned())
    })?;
    let storage_components = storage_relative.components().collect::<Vec<_>>();
    if storage_components.len() != 4
        || storage_components[0].as_os_str() != "peers"
        || storage_components[1].as_os_str() != peer.alias.as_str()
        || storage_components[2].as_os_str() != "storage-generations"
        || !is_canonical_restore_generation_id(storage_components[3].as_os_str())
        || !restore_sibling_matches(&peer.live_storage, &peer.staged_storage, "staged-storage")
        || !restore_sibling_matches(&peer.live_storage, &peer.backup_storage, "backup-storage")
    {
        return Err(SupervisorError::Config(format!(
            "snapshot restore journal contains invalid storage paths for peer `{}`",
            peer.alias
        )));
    }
    let log_relative = peer.live_log.strip_prefix(network_root).map_err(|_| {
        SupervisorError::Config("snapshot restore log escapes the network root".to_owned())
    })?;
    let log_components = log_relative.components().collect::<Vec<_>>();
    let expected_log = format!("{}.log", peer.alias);
    if log_components.len() != 2
        || log_components[0].as_os_str() != "logs"
        || log_components[1].as_os_str() != expected_log.as_str()
        || !restore_sibling_matches(&peer.live_log, &peer.backup_log, "backup-log")
        || peer
            .staged_log
            .as_ref()
            .is_some_and(|path| !restore_sibling_matches(&peer.live_log, path, "staged-log"))
    {
        return Err(SupervisorError::Config(format!(
            "snapshot restore journal contains invalid log paths for peer `{}`",
            peer.alias
        )));
    }
    Ok(())
}
fn read_restore_journal(
    network_root: &Path,
    journal_path: &Path,
) -> Result<Vec<StagedPeerRestore>> {
    let bytes = read_snapshot_file_bounded(journal_path, SNAPSHOT_RESTORE_JOURNAL_MAX_BYTES_V1)?;
    let value: Value = json::from_slice(&bytes).map_err(|error| {
        SupervisorError::Config(format!(
            "snapshot restore journal `{}` is invalid JSON: {error}",
            journal_path.display()
        ))
    })?;
    let root =
        exact_restore_journal_object(&value, "snapshot restore journal", &["peers", "version"])?;
    if !matches!(
        root.get("version"),
        Some(Value::Number(number)) if number.as_u64() == Some(1)
    ) {
        return Err(SupervisorError::Config(
            "snapshot restore journal version must be 1".to_owned(),
        ));
    }
    let values = root.get("peers").and_then(Value::as_array).ok_or_else(|| {
        SupervisorError::Config("snapshot restore journal peers must be an array".to_owned())
    })?;
    if values.is_empty() || values.len() > 7 {
        return Err(SupervisorError::Config(
            "snapshot restore journal must contain between 1 and 7 peers".to_owned(),
        ));
    }
    let mut aliases = HashSet::with_capacity(values.len());
    let mut peers = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let label = format!("snapshot restore journal peer[{index}]");
        let object = exact_restore_journal_object(
            value,
            &label,
            &[
                "alias",
                "backup_log",
                "backup_storage",
                "live_log",
                "live_storage",
                "original_log_present",
                "staged_log",
                "staged_storage",
            ],
        )?;
        let alias = object
            .get("alias")
            .and_then(Value::as_str)
            .ok_or_else(|| SupervisorError::Config(format!("{label}.alias must be a string")))?
            .to_owned();
        if !aliases.insert(alias.clone()) {
            return Err(SupervisorError::Config(format!(
                "snapshot restore journal repeats peer alias `{alias}`"
            )));
        }
        let staged_log = match object.get("staged_log") {
            Some(Value::Null) => None,
            Some(value) => Some(decode_restore_journal_path(
                network_root,
                value,
                &format!("{label}.staged_log"),
            )?),
            None => unreachable!("exact field check requires staged_log"),
        };
        let original_log_present = object
            .get("original_log_present")
            .and_then(Value::as_bool)
            .ok_or_else(|| {
                SupervisorError::Config(format!("{label}.original_log_present must be a boolean"))
            })?;
        let mut peer = StagedPeerRestore {
            alias,
            live_storage: decode_restore_journal_path(
                network_root,
                &object["live_storage"],
                &format!("{label}.live_storage"),
            )?,
            staged_storage: decode_restore_journal_path(
                network_root,
                &object["staged_storage"],
                &format!("{label}.staged_storage"),
            )?,
            backup_storage: decode_restore_journal_path(
                network_root,
                &object["backup_storage"],
                &format!("{label}.backup_storage"),
            )?,
            live_log: decode_restore_journal_path(
                network_root,
                &object["live_log"],
                &format!("{label}.live_log"),
            )?,
            staged_log,
            backup_log: decode_restore_journal_path(
                network_root,
                &object["backup_log"],
                &format!("{label}.backup_log"),
            )?,
            original_log_present,
            log_touched: false,
            storage_backed_up: false,
            storage_installed: false,
            log_backed_up: false,
            log_installed: false,
        };
        validate_recovered_restore_peer(network_root, &peer)?;
        peer.log_touched = false;
        peers.push(peer);
    }
    Ok(peers)
}
fn restore_directory_exists(path: &Path, label: &str) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(SupervisorError::Config(format!(
                "{label} `{}` must be a real directory",
                path.display()
            )))
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}
fn restore_file_exists(path: &Path, label: &str) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(SupervisorError::Config(format!(
                "{label} `{}` must be a regular non-symlink file",
                path.display()
            )))
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}
fn rename_restore_path(from: &Path, to: &Path) -> Result<()> {
    fs::rename(from, to)?;
    let parent = from
        .parent()
        .ok_or_else(|| SupervisorError::Config("snapshot restore path has no parent".to_owned()))?;
    sync_managed_directory(parent)?;
    Ok(())
}
fn remove_restore_directory(path: &Path, label: &str) -> Result<()> {
    if restore_directory_exists(path, label)? {
        fs::remove_dir_all(path)?;
        sync_managed_directory(path.parent().ok_or_else(|| {
            SupervisorError::Config("snapshot restore directory has no parent".to_owned())
        })?)?;
    }
    Ok(())
}
fn remove_restore_file(path: &Path, label: &str) -> Result<()> {
    if restore_file_exists(path, label)? {
        fs::remove_file(path)?;
        sync_managed_directory(path.parent().ok_or_else(|| {
            SupervisorError::Config("snapshot restore file has no parent".to_owned())
        })?)?;
    }
    Ok(())
}
fn recover_pending_restore_peer(peer: &StagedPeerRestore) -> Result<()> {
    let live_storage = restore_directory_exists(&peer.live_storage, "live restore storage")?;
    let staged_storage = restore_directory_exists(&peer.staged_storage, "staged restore storage")?;
    let backup_storage = restore_directory_exists(&peer.backup_storage, "backup restore storage")?;
    if backup_storage {
        if live_storage {
            if staged_storage {
                return Err(SupervisorError::Config(format!(
                    "snapshot restore storage for peer `{}` has ambiguous live, staged, and backup directories",
                    peer.alias
                )));
            }
            rename_restore_path(&peer.live_storage, &peer.staged_storage)?;
        }
        rename_restore_path(&peer.backup_storage, &peer.live_storage)?;
        remove_restore_directory(&peer.staged_storage, "rolled-back staged storage")?;
    } else {
        if !live_storage {
            return Err(SupervisorError::Config(format!(
                "snapshot restore journal for peer `{}` has neither live nor backup storage",
                peer.alias
            )));
        }
        remove_restore_directory(&peer.staged_storage, "unused staged storage")?;
    }

    let live_log = restore_file_exists(&peer.live_log, "live restore log")?;
    let backup_log = restore_file_exists(&peer.backup_log, "backup restore log")?;
    if backup_log {
        if live_log {
            remove_restore_file(&peer.live_log, "uncommitted restored log")?;
        }
        rename_restore_path(&peer.backup_log, &peer.live_log)?;
    } else if peer.original_log_present {
        if !live_log {
            return Err(SupervisorError::Config(format!(
                "snapshot restore journal for peer `{}` lost its original log",
                peer.alias
            )));
        }
    } else if live_log {
        remove_restore_file(&peer.live_log, "uncommitted restored log")?;
    }
    if let Some(staged_log) = peer.staged_log.as_ref() {
        remove_restore_file(staged_log, "unused staged restore log")?;
    }
    Ok(())
}
fn recover_committed_restore_peer(peer: &StagedPeerRestore) -> Result<()> {
    if !restore_directory_exists(&peer.live_storage, "committed live restore storage")? {
        return Err(SupervisorError::Config(format!(
            "committed snapshot restore for peer `{}` is missing live storage",
            peer.alias
        )));
    }
    remove_restore_directory(&peer.staged_storage, "committed staged storage")?;
    remove_restore_directory(&peer.backup_storage, "committed backup storage")?;
    if restore_file_exists(&peer.live_log, "committed live restore log")? {
        // Validation is the only required action for the authoritative live log.
    }
    if let Some(staged_log) = peer.staged_log.as_ref() {
        remove_restore_file(staged_log, "committed staged restore log")?;
    }
    remove_restore_file(&peer.backup_log, "committed backup restore log")?;
    Ok(())
}
fn validate_restore_commit_marker(path: &Path) -> Result<()> {
    let bytes = read_snapshot_file_bounded(path, b"committed\n".len())?;
    if bytes != b"committed\n" {
        return Err(SupervisorError::Config(format!(
            "snapshot restore commit marker `{}` is invalid",
            path.display()
        )));
    }
    Ok(())
}
pub(super) fn recover_snapshot_restore_if_needed(network_root: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(network_root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(SupervisorError::Config(format!(
            "snapshot restore network root `{}` must be a real directory",
            network_root.display()
        )));
    }
    let network_root = fs::canonicalize(network_root)?;
    let journal_path = network_root.join(SNAPSHOT_RESTORE_JOURNAL_FILE_NAME);
    let commit_marker_path = network_root.join(SNAPSHOT_RESTORE_COMMIT_FILE_NAME);
    let journal_exists = restore_file_exists(&journal_path, "snapshot restore journal")?;
    let commit_exists = restore_file_exists(&commit_marker_path, "snapshot restore commit marker")?;
    if !journal_exists {
        if commit_exists {
            validate_restore_commit_marker(&commit_marker_path)?;
            fs::remove_file(&commit_marker_path)?;
            sync_managed_directory(&network_root)?;
        }
        return Ok(());
    }
    let peers = read_restore_journal(&network_root, &journal_path)?;
    if commit_exists {
        validate_restore_commit_marker(&commit_marker_path)?;
        for peer in &peers {
            recover_committed_restore_peer(peer)?;
        }
    } else {
        for peer in peers.iter().rev() {
            recover_pending_restore_peer(peer)?;
        }
    }
    fs::remove_file(&journal_path)?;
    sync_managed_directory(&network_root)?;
    if commit_exists {
        fs::remove_file(&commit_marker_path)?;
        sync_managed_directory(&network_root)?;
    }
    Ok(())
}
impl SnapshotRestoreTransaction {
    pub(super) fn stage(
        network_root: &Path,
        peers_root: &Path,
        peers: &[PeerHandle],
        expected_hashes: &HashMap<String, Hash>,
    ) -> Result<Self> {
        let network_root = fs::canonicalize(network_root)?;
        let mut transaction = Self {
            network_root: network_root.clone(),
            journal_path: network_root.join(SNAPSHOT_RESTORE_JOURNAL_FILE_NAME),
            commit_marker_path: network_root.join(SNAPSHOT_RESTORE_COMMIT_FILE_NAME),
            peers: Vec::with_capacity(peers.len()),
            committed: false,
            preserve_backups: false,
        };
        for peer in peers {
            let live_storage = peer.storage_dir().to_path_buf();
            let live_metadata = fs::symlink_metadata(&live_storage)?;
            if live_metadata.file_type().is_symlink() || !live_metadata.is_dir() {
                return Err(SupervisorError::Config(format!(
                    "managed storage for peer `{}` must be a real directory",
                    peer.alias()
                )));
            }
            let staged_storage = unused_restore_sibling(&live_storage, "staged-storage")?;
            let backup_storage = unused_restore_sibling(&live_storage, "backup-storage")?;
            let live_log_parent = peer.log_path().parent().ok_or_else(|| {
                SupervisorError::Config(format!(
                    "managed log `{}` has no parent directory",
                    peer.log_path().display()
                ))
            })?;
            let live_log = fs::canonicalize(live_log_parent)?.join(
                peer.log_path().file_name().ok_or_else(|| {
                    SupervisorError::Config(format!(
                        "managed log `{}` has no file name",
                        peer.log_path().display()
                    ))
                })?,
            );
            let backup_log = unused_restore_sibling(&live_log, "backup-log")?;
            let snapshot_log = peers_root.join(peer.alias()).join("latest.log");
            let staged_log = match fs::symlink_metadata(&snapshot_log) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                    return Err(SupervisorError::Config(format!(
                        "snapshot log `{}` must be a regular non-symlink file",
                        snapshot_log.display()
                    )));
                }
                Ok(_) => Some(unused_restore_sibling(&live_log, "staged-log")?),
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) => return Err(error.into()),
            };
            let original_log_present = match fs::symlink_metadata(&live_log) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                    return Err(SupervisorError::Config(format!(
                        "managed log for peer `{}` must be a regular non-symlink file",
                        peer.alias()
                    )));
                }
                Ok(_) => true,
                Err(error) if error.kind() == io::ErrorKind::NotFound => false,
                Err(error) => return Err(error.into()),
            };
            transaction.peers.push(StagedPeerRestore {
                alias: peer.alias().to_owned(),
                live_storage,
                staged_storage,
                backup_storage,
                live_log,
                staged_log,
                backup_log,
                original_log_present,
                log_touched: false,
                storage_backed_up: false,
                storage_installed: false,
                log_backed_up: false,
                log_installed: false,
            });
        }
        write_pending_restore_journal(&transaction)?;
        for staged in &transaction.peers {
            copy_dir_recursive(
                &peers_root.join(&staged.alias).join("storage"),
                &staged.staged_storage,
            )?;
            let staged_hash = hash_directory(&staged.staged_storage)?;
            let expected_hash = expected_hashes.get(&staged.alias).ok_or_else(|| {
                SupervisorError::Config(format!(
                    "snapshot metadata missing storage hash for peer `{}`",
                    staged.alias
                ))
            })?;
            if &staged_hash != expected_hash {
                return Err(SupervisorError::Config(format!(
                    "staged snapshot storage for peer `{}` failed integrity check: expected {expected_hash} but found {staged_hash}",
                    staged.alias
                )));
            }
            if let Some(staged_log) = staged.staged_log.as_ref() {
                if let Some(parent) = staged_log.parent() {
                    fs::create_dir_all(parent)?;
                }
                copy_snapshot_file(
                    &peers_root.join(&staged.alias).join("latest.log"),
                    staged_log,
                )?;
            }
        }
        Ok(transaction)
    }
    pub(super) fn apply<F>(
        &mut self,
        mut after_peer: F,
    ) -> std::result::Result<(), SnapshotRestoreApplyFailure>
    where
        F: FnMut(usize) -> Result<()>,
    {
        for index in 0..self.peers.len() {
            let result = apply_staged_peer_restore(&mut self.peers[index]);
            if let Err(primary) = result {
                return Err(self.rollback_after(primary));
            }
            if let Err(primary) = after_peer(index + 1) {
                return Err(self.rollback_after(primary));
            }
        }
        Ok(())
    }
    fn rollback_after(&mut self, primary: SupervisorError) -> SnapshotRestoreApplyFailure {
        match self.rollback() {
            Ok(()) => SnapshotRestoreApplyFailure::RolledBack(primary),
            Err(rollback) => SnapshotRestoreApplyFailure::RollbackFailed { primary, rollback },
        }
    }
    pub(super) fn rollback(&mut self) -> io::Result<()> {
        let mut first_error = None;
        for peer in self.peers.iter_mut().rev() {
            capture_cleanup_error(&mut first_error, rollback_staged_peer_log(peer));
            if peer.storage_installed {
                let result = fs::rename(&peer.live_storage, &peer.staged_storage);
                if result.is_ok() {
                    if let Some(parent) = peer.live_storage.parent() {
                        capture_cleanup_error(&mut first_error, sync_managed_directory(parent));
                    }
                    peer.storage_installed = false;
                }
                capture_cleanup_error(&mut first_error, result);
            }
            if peer.storage_backed_up {
                let result = fs::rename(&peer.backup_storage, &peer.live_storage);
                if result.is_ok() {
                    if let Some(parent) = peer.live_storage.parent() {
                        capture_cleanup_error(&mut first_error, sync_managed_directory(parent));
                    }
                    peer.storage_backed_up = false;
                }
                capture_cleanup_error(&mut first_error, result);
            }
            for parent in [peer.live_storage.parent(), peer.live_log.parent()]
                .into_iter()
                .flatten()
            {
                capture_cleanup_error(&mut first_error, sync_managed_directory(parent));
            }
        }
        if first_error.is_none() {
            self.cleanup_staged_best_effort();
            match self.recovery_artifacts_remain() {
                Ok(false) => self.remove_pending_journal_best_effort(),
                Ok(true) => {}
                Err(error) => first_error = Some(error),
            }
        }
        first_error.map_or(Ok(()), Err)
    }
    pub(super) fn commit(&mut self) -> std::result::Result<(), SnapshotRestoreCommitFailure> {
        write_restore_commit_marker(&self.commit_marker_path, &self.network_root)?;
        self.committed = true;
        self.cleanup_committed_artifacts_best_effort();
        if matches!(self.recovery_artifacts_remain(), Ok(false)) {
            self.remove_committed_journal_best_effort();
        }
        Ok(())
    }
    pub(super) fn preserve_installed_state(&mut self) {
        self.committed = true;
        self.preserve_backups = true;
    }
    fn cleanup_staged_best_effort(&self) {
        for peer in &self.peers {
            if peer.staged_storage.exists() {
                let _ = fs::remove_dir_all(&peer.staged_storage);
            }
            if let Some(staged_log) = peer.staged_log.as_ref()
                && staged_log.exists()
            {
                let _ = fs::remove_file(staged_log);
            }
        }
    }
    fn cleanup_committed_artifacts_best_effort(&self) {
        self.cleanup_staged_best_effort();
        for peer in &self.peers {
            if peer.backup_storage.exists() {
                let _ = fs::remove_dir_all(&peer.backup_storage);
            }
            if peer.backup_log.exists() {
                let _ = fs::remove_file(&peer.backup_log);
            }
        }
        for parent in self.peers.iter().flat_map(|peer| {
            [peer.live_storage.parent(), peer.live_log.parent()]
                .into_iter()
                .flatten()
        }) {
            let _ = sync_managed_directory(parent);
        }
    }
    fn recovery_artifacts_remain(&self) -> io::Result<bool> {
        for path in self.peers.iter().flat_map(|peer| {
            [
                Some(peer.staged_storage.as_path()),
                Some(peer.backup_storage.as_path()),
                peer.staged_log.as_deref(),
                Some(peer.backup_log.as_path()),
            ]
            .into_iter()
            .flatten()
        }) {
            match fs::symlink_metadata(path) {
                Ok(_) => return Ok(true),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(false)
    }
    fn remove_pending_journal_best_effort(&self) {
        if self.journal_path.exists() {
            let _ = fs::remove_file(&self.journal_path);
            let _ = sync_managed_directory(&self.network_root);
        }
    }
    fn remove_committed_journal_best_effort(&self) {
        self.remove_committed_journal_with(|path| {
            remove_restore_control_file_and_sync(path, &self.network_root)
        });
    }
    pub(super) fn remove_committed_journal_with(
        &self,
        mut remove_and_sync: impl FnMut(&Path) -> io::Result<()>,
    ) {
        if remove_and_sync(&self.journal_path).is_err() {
            return;
        }
        let _ = remove_and_sync(&self.commit_marker_path);
    }
}
impl Drop for SnapshotRestoreTransaction {
    fn drop(&mut self) {
        if self.committed {
            if !self.preserve_backups {
                self.cleanup_committed_artifacts_best_effort();
                if matches!(self.recovery_artifacts_remain(), Ok(false)) {
                    self.remove_committed_journal_best_effort();
                }
            }
        } else {
            let _ = self.rollback();
        }
        self.cleanup_staged_best_effort();
    }
}
fn remove_restore_control_file_and_sync(path: &Path, network_root: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "snapshot restore control file `{}` must be a regular non-symlink file",
                    path.display()
                ),
            ));
        }
        Ok(_) => fs::remove_file(path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    sync_managed_directory(network_root)
}
fn apply_staged_peer_restore(peer: &mut StagedPeerRestore) -> Result<()> {
    fs::rename(&peer.live_storage, &peer.backup_storage)?;
    peer.storage_backed_up = true;
    sync_managed_directory(
        peer.live_storage
            .parent()
            .expect("managed storage always has a parent"),
    )?;
    fs::rename(&peer.staged_storage, &peer.live_storage)?;
    peer.storage_installed = true;
    sync_managed_directory(
        peer.live_storage
            .parent()
            .expect("managed storage always has a parent"),
    )?;
    peer.log_touched = true;
    match fs::symlink_metadata(&peer.live_log) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(SupervisorError::Config(format!(
                "managed log for peer `{}` must be a regular non-symlink file",
                peer.alias
            )));
        }
        Ok(_) => {
            fs::rename(&peer.live_log, &peer.backup_log)?;
            peer.log_backed_up = true;
            sync_managed_directory(
                peer.live_log
                    .parent()
                    .expect("managed log always has a parent"),
            )?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    if let Some(staged_log) = peer.staged_log.as_ref() {
        fs::rename(staged_log, &peer.live_log)?;
        peer.log_installed = true;
        sync_managed_directory(
            peer.live_log
                .parent()
                .expect("managed log always has a parent"),
        )?;
    }
    Ok(())
}
fn rollback_staged_peer_log(peer: &mut StagedPeerRestore) -> io::Result<()> {
    if !peer.log_touched {
        return Ok(());
    }
    if peer.log_installed {
        let staged_log = peer
            .staged_log
            .as_ref()
            .expect("installed restore log always has a staged path");
        fs::rename(&peer.live_log, staged_log)?;
        sync_managed_directory(
            peer.live_log
                .parent()
                .expect("managed log always has a parent"),
        )?;
        peer.log_installed = false;
    } else if peer.staged_log.is_none() {
        remove_restore_log_if_present(&peer.live_log)?;
        sync_managed_directory(
            peer.live_log
                .parent()
                .expect("managed log always has a parent"),
        )?;
    }
    if peer.log_backed_up {
        fs::rename(&peer.backup_log, &peer.live_log)?;
        sync_managed_directory(
            peer.live_log
                .parent()
                .expect("managed log always has a parent"),
        )?;
        peer.log_backed_up = false;
    }
    peer.log_touched = false;
    Ok(())
}
fn capture_cleanup_error(first_error: &mut Option<io::Error>, result: io::Result<()>) {
    if let Err(error) = result
        && first_error.is_none()
    {
        *first_error = Some(error);
    }
}
fn remove_restore_log_if_present(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "restore-created log `{}` must be a regular non-symlink file",
                    path.display()
                ),
            ))
        }
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
fn unused_restore_sibling(path: &Path, role: &str) -> io::Result<PathBuf> {
    static NEXT_RESTORE_ID: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "managed restore path has no parent directory",
        )
    })?;
    let file_name = path.file_name().unwrap_or_else(|| OsStr::new("managed"));
    for _ in 0..64 {
        let id = NEXT_RESTORE_ID.fetch_add(1, Ordering::Relaxed);
        let mut candidate_name = OsString::from(".");
        candidate_name.push(file_name);
        candidate_name.push(format!(".mochi-restore-{role}.{}.{id}", std::process::id()));
        let candidate = parent.join(candidate_name);
        match fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(candidate),
            Ok(_) => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique snapshot restore path",
    ))
}
