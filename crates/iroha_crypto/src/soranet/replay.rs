//! Durable, namespace-bound replay ledgers for `SoraNet` admission credentials.
//!
//! Replay records are security state rather than cache entries. Active records
//! are never evicted to make room, every accepted insertion is persisted before
//! it returns, and a process-lifetime sidecar lock prevents concurrent writers
//! from forking the ledger history. Snapshots use canonical, checksum-protected
//! Norito frames and persist a monotonic wall-clock high-water mark.

use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs,
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
};

use norito::{NoritoDeserialize, NoritoSerialize, decode_canonical, encode_canonical};
use thiserror::Error;

use super::replay_lock::ExclusiveLedgerLock;

const SNAPSHOT_VERSION_V1: u8 = 1;
const NAMESPACE_DOMAIN_V1: &[u8] = b"iroha.soranet.replay-ledger.namespace.v1";
const SNAPSHOT_BASE_LIMIT_BYTES: usize = 4 * 1024;
const SNAPSHOT_ENTRY_LIMIT_BYTES: usize = 128;

/// Resource bounds for a durable replay ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayLedgerLimits {
    /// Maximum number of unexpired records retained by the ledger.
    pub max_entries: usize,
    /// Maximum accepted credential lifetime in milliseconds.
    pub max_ttl_ms: u64,
}

impl ReplayLedgerLimits {
    /// Construct validated replay-ledger limits.
    ///
    /// # Errors
    ///
    /// Returns [`ReplayLedgerError`] when either bound is zero or when the
    /// capacity cannot be converted into a bounded snapshot size.
    pub fn new(max_entries: usize, max_ttl_ms: u64) -> Result<Self, ReplayLedgerError> {
        if max_entries == 0 {
            return Err(ReplayLedgerError::CapacityZero);
        }
        if max_ttl_ms == 0 {
            return Err(ReplayLedgerError::TtlZero);
        }
        max_entries
            .checked_mul(SNAPSHOT_ENTRY_LIMIT_BYTES)
            .and_then(|bytes| bytes.checked_add(SNAPSHOT_BASE_LIMIT_BYTES))
            .ok_or(ReplayLedgerError::CapacityOverflow)?;
        Ok(Self {
            max_entries,
            max_ttl_ms,
        })
    }

    fn max_snapshot_bytes(self) -> usize {
        self.max_entries
            .checked_mul(SNAPSHOT_ENTRY_LIMIT_BYTES)
            .and_then(|bytes| bytes.checked_add(SNAPSHOT_BASE_LIMIT_BYTES))
            .expect("validated replay-ledger capacity")
    }
}

/// Result of attempting to consume a replay-protected identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayInsertStatus {
    /// The identifier was recorded durably.
    Accepted,
    /// An unexpired record already exists for the identifier.
    Duplicate,
    /// The credential expired before it could be consumed.
    Expired,
    /// The credential lifetime exceeds the ledger policy.
    TtlExceeded,
    /// Every configured slot contains an active record.
    Capacity,
}

/// Errors surfaced by durable replay-ledger operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplayLedgerError {
    /// Ledger capacity cannot be zero.
    #[error("replay ledger capacity must be greater than zero")]
    CapacityZero,
    /// Credential lifetime bound cannot be zero.
    #[error("replay ledger max_ttl_ms must be greater than zero")]
    TtlZero,
    /// Capacity cannot be represented as a bounded snapshot size.
    #[error("replay ledger capacity is too large for this platform")]
    CapacityOverflow,
    /// The namespace must be explicit so ledgers cannot be substituted.
    #[error("replay ledger namespace must not be empty")]
    NamespaceEmpty,
    /// Namespace length cannot be encoded canonically on this platform.
    #[error("replay ledger namespace length exceeds u64")]
    NamespaceTooLong,
    /// Persistent ledger paths must identify a concrete file.
    #[error("replay ledger path must not be empty")]
    PathEmpty,
    /// Filesystem operation failed.
    #[error("replay ledger io error: {0}")]
    Io(String),
    /// Persisted bytes did not contain a valid, policy-compliant snapshot.
    #[error("replay ledger snapshot error: {0}")]
    Snapshot(String),
    /// Snapshot encoding failed.
    #[error("replay ledger encode error: {0}")]
    Encode(String),
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
#[norito(decode_from_slice)]
struct ReplayLedgerSnapshotV1 {
    version: u8,
    namespace_digest: [u8; 32],
    high_watermark_ms: u64,
    entries: Vec<ReplayLedgerSnapshotEntryV1>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct ReplayLedgerSnapshotEntryV1 {
    id: [u8; 32],
    expires_at_ms: u64,
}

/// Persistent set of consumed credential identifiers with exact millisecond expiry.
#[derive(Debug)]
pub struct PersistentReplayLedger {
    limits: ReplayLedgerLimits,
    namespace_digest: [u8; 32],
    high_watermark_ms: u64,
    records: HashMap<[u8; 32], u64>,
    path: Option<PathBuf>,
    _ledger_lock: Option<ExclusiveLedgerLock>,
}

impl PersistentReplayLedger {
    /// Create an in-memory ledger for tests or non-persistent tooling.
    ///
    /// # Errors
    ///
    /// Returns [`ReplayLedgerError`] if the namespace or limits are invalid.
    #[cfg(test)]
    fn in_memory(namespace: &[u8], limits: ReplayLedgerLimits) -> Result<Self, ReplayLedgerError> {
        Ok(Self {
            limits: ReplayLedgerLimits::new(limits.max_entries, limits.max_ttl_ms)?,
            namespace_digest: namespace_digest(namespace)?,
            high_watermark_ms: 0,
            records: HashMap::new(),
            path: None,
            _ledger_lock: None,
        })
    }

    /// Load or create a durable replay ledger at `path`.
    ///
    /// Missing ledgers are materialized immediately, proving that replay state
    /// is writable before the caller begins accepting credentials. Existing
    /// ledgers are bound to `namespace`; a file copied from another credential
    /// class is rejected.
    ///
    /// # Errors
    ///
    /// Returns [`ReplayLedgerError`] if the path, namespace, bounds, lock,
    /// persisted snapshot, or initial durability check is invalid.
    pub fn load(
        path: impl Into<PathBuf>,
        namespace: &[u8],
        limits: ReplayLedgerLimits,
        now_ms: u64,
    ) -> Result<Self, ReplayLedgerError> {
        let limits = ReplayLedgerLimits::new(limits.max_entries, limits.max_ttl_ms)?;
        let path = path.into();
        if path.as_os_str().is_empty() {
            return Err(ReplayLedgerError::PathEmpty);
        }
        let namespace_digest = namespace_digest(namespace)?;
        let ledger_lock = ExclusiveLedgerLock::acquire(&path)
            .map_err(|error| ReplayLedgerError::Io(error.to_string()))?;
        let mut ledger = Self {
            limits,
            namespace_digest,
            high_watermark_ms: now_ms,
            records: HashMap::new(),
            path: Some(path),
            _ledger_lock: Some(ledger_lock),
        };
        ledger.load_from_disk(now_ms)?;
        Ok(ledger)
    }

    /// Atomically consume `id` until `expires_at_ms`.
    ///
    /// Accepted records are flushed before this method returns. Active records
    /// are never evicted; capacity exhaustion therefore fails closed.
    ///
    /// # Errors
    ///
    /// Returns [`ReplayLedgerError`] when the durable snapshot cannot be
    /// updated. The in-memory record remains present after a persistence error,
    /// so the running process cannot accidentally accept a retry.
    pub fn insert(
        &mut self,
        id: [u8; 32],
        expires_at_ms: u64,
        now_ms: u64,
    ) -> Result<ReplayInsertStatus, ReplayLedgerError> {
        let effective_now_ms = self.observe_now(now_ms);
        if expires_at_ms <= effective_now_ms {
            return Ok(ReplayInsertStatus::Expired);
        }
        if expires_at_ms.saturating_sub(effective_now_ms) > self.limits.max_ttl_ms {
            return Ok(ReplayInsertStatus::TtlExceeded);
        }
        if self.contains_active(&id, effective_now_ms) {
            return Ok(ReplayInsertStatus::Duplicate);
        }
        if self.active_len(effective_now_ms) >= self.limits.max_entries {
            return Ok(ReplayInsertStatus::Capacity);
        }
        self.prune_expired_in_memory(effective_now_ms);
        let replaced = self.records.insert(id, expires_at_ms);
        debug_assert!(replaced.is_none());
        self.persist()?;
        Ok(ReplayInsertStatus::Accepted)
    }

    fn contains_active(&self, id: &[u8; 32], now_ms: u64) -> bool {
        let effective_now_ms = self.effective_now(now_ms);
        self.records
            .get(id)
            .is_some_and(|expires_at_ms| *expires_at_ms > effective_now_ms)
    }

    fn active_len(&self, now_ms: u64) -> usize {
        let effective_now_ms = self.effective_now(now_ms);
        self.records
            .values()
            .filter(|expires_at_ms| **expires_at_ms > effective_now_ms)
            .count()
    }

    /// Remove expired records and persist the compacted snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`ReplayLedgerError`] when a changed snapshot cannot be made durable.
    pub fn purge_expired(&mut self, now_ms: u64) -> Result<usize, ReplayLedgerError> {
        let previous_high_watermark_ms = self.high_watermark_ms;
        let effective_now_ms = self.observe_now(now_ms);
        let before = self.records.len();
        self.prune_expired_in_memory(effective_now_ms);
        let removed = before.saturating_sub(self.records.len());
        if removed > 0 || self.high_watermark_ms != previous_high_watermark_ms {
            self.persist()?;
        }
        Ok(removed)
    }

    fn load_from_disk(&mut self, now_ms: u64) -> Result<(), ReplayLedgerError> {
        let path = self.path.as_ref().expect("persistent ledger has a path");
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| io_error(&error))?;
        }
        let mut file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return self.persist(),
            Err(error) => return Err(io_error(&error)),
        };
        let metadata = file.metadata().map_err(|error| io_error(&error))?;
        if !metadata.is_file() {
            return Err(ReplayLedgerError::Snapshot(
                "snapshot path is not a regular file".to_owned(),
            ));
        }
        let max_snapshot_bytes = self.limits.max_snapshot_bytes();
        let max_snapshot_bytes_u64 =
            u64::try_from(max_snapshot_bytes).map_err(|_| ReplayLedgerError::CapacityOverflow)?;
        if metadata.len() > max_snapshot_bytes_u64 {
            let snapshot_bytes = metadata.len();
            return Err(ReplayLedgerError::Snapshot(format!(
                "snapshot is {snapshot_bytes} bytes; limit is {max_snapshot_bytes} bytes"
            )));
        }
        let read_limit = max_snapshot_bytes_u64
            .checked_add(1)
            .ok_or(ReplayLedgerError::CapacityOverflow)?;
        let mut bytes =
            Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(max_snapshot_bytes));
        (&mut file)
            .take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(|error| io_error(&error))?;
        if bytes.is_empty() {
            return Err(ReplayLedgerError::Snapshot("snapshot is empty".to_owned()));
        }
        if bytes.len() > max_snapshot_bytes {
            let snapshot_bytes = bytes.len();
            return Err(ReplayLedgerError::Snapshot(format!(
                "snapshot is {snapshot_bytes} bytes; limit is {max_snapshot_bytes} bytes"
            )));
        }
        let snapshot: ReplayLedgerSnapshotV1 = decode_canonical(&bytes)
            .map_err(|error| ReplayLedgerError::Snapshot(error.to_string()))?;
        if snapshot.version != SNAPSHOT_VERSION_V1 {
            let version = snapshot.version;
            return Err(ReplayLedgerError::Snapshot(format!(
                "unsupported snapshot version {version}"
            )));
        }
        if snapshot.namespace_digest != self.namespace_digest {
            return Err(ReplayLedgerError::Snapshot(
                "namespace digest does not match configured credential class".to_owned(),
            ));
        }
        if snapshot
            .entries
            .windows(2)
            .any(|pair| replay_entry_order(&pair[0], &pair[1]) != std::cmp::Ordering::Less)
        {
            return Err(ReplayLedgerError::Snapshot(
                "snapshot entries are not in strict canonical order".to_owned(),
            ));
        }
        self.high_watermark_ms = self.high_watermark_ms.max(snapshot.high_watermark_ms);
        let effective_now_ms = self.observe_now(now_ms);
        let mut seen = HashSet::with_capacity(snapshot.entries.len());
        for entry in snapshot.entries {
            if !seen.insert(entry.id) {
                return Err(ReplayLedgerError::Snapshot(
                    "snapshot contains a duplicate identifier".to_owned(),
                ));
            }
            if entry.expires_at_ms <= effective_now_ms {
                continue;
            }
            if entry.expires_at_ms.saturating_sub(effective_now_ms) > self.limits.max_ttl_ms {
                let max_ttl_ms = self.limits.max_ttl_ms;
                return Err(ReplayLedgerError::Snapshot(format!(
                    "active record expiry exceeds max_ttl_ms {max_ttl_ms}"
                )));
            }
            let replaced = self.records.insert(entry.id, entry.expires_at_ms);
            debug_assert!(replaced.is_none());
            if self.records.len() > self.limits.max_entries {
                return Err(ReplayLedgerError::Snapshot(
                    "snapshot exceeds configured capacity".to_owned(),
                ));
            }
        }
        self.persist()
    }

    fn prune_expired_in_memory(&mut self, now_ms: u64) {
        self.records
            .retain(|_, expires_at_ms| *expires_at_ms > now_ms);
    }

    fn effective_now(&self, now_ms: u64) -> u64 {
        self.high_watermark_ms.max(now_ms)
    }

    fn observe_now(&mut self, now_ms: u64) -> u64 {
        self.high_watermark_ms = self.high_watermark_ms.max(now_ms);
        self.high_watermark_ms
    }

    fn persist(&self) -> Result<(), ReplayLedgerError> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| io_error(&error))?;
        }
        let mut entries: Vec<_> = self
            .records
            .iter()
            .map(|(id, expires_at_ms)| ReplayLedgerSnapshotEntryV1 {
                id: *id,
                expires_at_ms: *expires_at_ms,
            })
            .collect();
        entries.sort_by(replay_entry_order);
        let snapshot = ReplayLedgerSnapshotV1 {
            version: SNAPSHOT_VERSION_V1,
            namespace_digest: self.namespace_digest,
            high_watermark_ms: self.high_watermark_ms,
            entries,
        };
        let encoded = encode_canonical(&snapshot)
            .map_err(|error| ReplayLedgerError::Encode(error.to_string()))?;
        if encoded.len() > self.limits.max_snapshot_bytes() {
            let encoded_bytes = encoded.len();
            let max_snapshot_bytes = self.limits.max_snapshot_bytes();
            return Err(ReplayLedgerError::Snapshot(format!(
                "encoded snapshot is {encoded_bytes} bytes; limit is {max_snapshot_bytes} bytes"
            )));
        }
        let temporary_path = temporary_path(path);
        let mut temporary = fs::File::create(&temporary_path).map_err(|error| io_error(&error))?;
        temporary
            .write_all(&encoded)
            .and_then(|()| temporary.sync_all())
            .map_err(|error| io_error(&error))?;
        fs::rename(&temporary_path, path).map_err(|error| io_error(&error))?;
        #[cfg(unix)]
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| io_error(&error))?;
        }
        Ok(())
    }
}

fn namespace_digest(namespace: &[u8]) -> Result<[u8; 32], ReplayLedgerError> {
    if namespace.is_empty() {
        return Err(ReplayLedgerError::NamespaceEmpty);
    }
    let namespace_len =
        u64::try_from(namespace.len()).map_err(|_| ReplayLedgerError::NamespaceTooLong)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(NAMESPACE_DOMAIN_V1);
    hasher.update(&namespace_len.to_be_bytes());
    hasher.update(namespace);
    Ok(*hasher.finalize().as_bytes())
}

fn replay_entry_order(
    left: &ReplayLedgerSnapshotEntryV1,
    right: &ReplayLedgerSnapshotEntryV1,
) -> std::cmp::Ordering {
    left.expires_at_ms
        .cmp(&right.expires_at_ms)
        .then_with(|| left.id.cmp(&right.id))
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut temporary = OsString::from(path.as_os_str());
    temporary.push(".tmp");
    PathBuf::from(temporary)
}

fn io_error(error: &io::Error) -> ReplayLedgerError {
    ReplayLedgerError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    const NAMESPACE: &[u8] = b"test.soranet.replay-ledger.v1";

    fn limits(capacity: usize) -> ReplayLedgerLimits {
        ReplayLedgerLimits::new(capacity, 1_000).expect("valid limits")
    }

    #[test]
    fn limits_and_namespace_reject_zero_or_overflowing_bounds() {
        assert_eq!(
            ReplayLedgerLimits::new(0, 1).expect_err("zero capacity"),
            ReplayLedgerError::CapacityZero
        );
        assert_eq!(
            ReplayLedgerLimits::new(1, 0).expect_err("zero ttl"),
            ReplayLedgerError::TtlZero
        );
        assert_eq!(
            ReplayLedgerLimits::new(usize::MAX, 1).expect_err("overflowing capacity"),
            ReplayLedgerError::CapacityOverflow
        );
        assert_eq!(
            PersistentReplayLedger::in_memory(b"", limits(1)).expect_err("empty namespace"),
            ReplayLedgerError::NamespaceEmpty
        );
        assert_eq!(
            PersistentReplayLedger::load(PathBuf::new(), NAMESPACE, limits(1), 0)
                .expect_err("empty path"),
            ReplayLedgerError::PathEmpty
        );
    }

    #[test]
    fn insert_enforces_exact_expiry_ttl_duplicates_and_capacity() {
        let mut ledger = PersistentReplayLedger::in_memory(NAMESPACE, limits(1)).expect("ledger");
        let first = [0x11; 32];
        let second = [0x22; 32];

        assert_eq!(
            ledger.insert(first, 100, 100).expect("expired status"),
            ReplayInsertStatus::Expired
        );
        assert_eq!(
            ledger.insert(first, 1_101, 100).expect("ttl status"),
            ReplayInsertStatus::TtlExceeded
        );
        assert_eq!(
            ledger.insert(first, 1_100, 100).expect("accepted"),
            ReplayInsertStatus::Accepted
        );
        assert!(ledger.contains_active(&first, 1_099));
        assert!(!ledger.contains_active(&first, 1_100));
        assert_eq!(ledger.active_len(1_099), 1);
        assert_eq!(ledger.active_len(1_100), 0);
        assert_eq!(
            ledger.insert(first, 1_100, 101).expect("duplicate"),
            ReplayInsertStatus::Duplicate
        );
        assert_eq!(
            ledger.insert(second, 1_100, 101).expect("capacity"),
            ReplayInsertStatus::Capacity
        );
        assert_eq!(ledger.purge_expired(1_100).expect("purge"), 1);
        assert_eq!(
            ledger.insert(second, 2_100, 1_100).expect("reclaimed slot"),
            ReplayInsertStatus::Accepted
        );
    }

    #[test]
    fn durable_ledger_rejects_replay_after_restart() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("helper-replays.norito");
        let id = [0x33; 32];
        {
            let mut ledger =
                PersistentReplayLedger::load(&path, NAMESPACE, limits(4), 10).expect("load");
            assert_eq!(
                ledger.insert(id, 500, 10).expect("persist consumption"),
                ReplayInsertStatus::Accepted
            );
        }

        let mut reloaded =
            PersistentReplayLedger::load(&path, NAMESPACE, limits(4), 20).expect("reload");
        assert_eq!(
            reloaded.insert(id, 500, 20).expect("replay status"),
            ReplayInsertStatus::Duplicate
        );
    }

    #[test]
    fn durable_high_watermark_prevents_clock_rollback_replay() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("clock-rollback.norito");
        let id = [0x44; 32];
        {
            let mut ledger =
                PersistentReplayLedger::load(&path, NAMESPACE, limits(4), 100).expect("load");
            assert_eq!(
                ledger.insert(id, 500, 100).expect("consume ticket"),
                ReplayInsertStatus::Accepted
            );
            assert_eq!(ledger.purge_expired(500).expect("prune expired"), 1);
            assert_eq!(
                ledger.insert(id, 500, 400).expect("rollback status"),
                ReplayInsertStatus::Expired,
                "a regressed wall clock must not reopen the consumed ticket"
            );
        }

        let mut reloaded =
            PersistentReplayLedger::load(&path, NAMESPACE, limits(4), 400).expect("reload");
        assert_eq!(
            reloaded
                .insert(id, 500, 400)
                .expect("restart rollback status"),
            ReplayInsertStatus::Expired,
            "the persisted high-water mark must survive restart"
        );
    }

    #[test]
    fn durable_ledger_fails_closed_on_corruption_or_namespace_substitution() {
        let directory = tempdir().expect("temporary directory");
        let corrupt_path = directory.path().join("corrupt.norito");
        fs::write(&corrupt_path, [0xFF, 0x00, 0xAA]).expect("write corruption");
        assert!(matches!(
            PersistentReplayLedger::load(&corrupt_path, NAMESPACE, limits(4), 10),
            Err(ReplayLedgerError::Snapshot(_))
        ));

        let noncanonical_order_path = directory.path().join("noncanonical-order.norito");
        let noncanonical_order_snapshot = ReplayLedgerSnapshotV1 {
            version: SNAPSHOT_VERSION_V1,
            namespace_digest: namespace_digest(NAMESPACE).expect("namespace digest"),
            high_watermark_ms: 10,
            entries: vec![
                ReplayLedgerSnapshotEntryV1 {
                    id: [0x02; 32],
                    expires_at_ms: 30,
                },
                ReplayLedgerSnapshotEntryV1 {
                    id: [0x01; 32],
                    expires_at_ms: 20,
                },
            ],
        };
        fs::write(
            &noncanonical_order_path,
            encode_canonical(&noncanonical_order_snapshot).expect("encode unsorted snapshot"),
        )
        .expect("write unsorted snapshot");
        assert!(matches!(
            PersistentReplayLedger::load(&noncanonical_order_path, NAMESPACE, limits(4), 10),
            Err(ReplayLedgerError::Snapshot(message)) if message.contains("canonical order")
        ));

        let substituted_path = directory.path().join("substituted.norito");
        drop(
            PersistentReplayLedger::load(&substituted_path, NAMESPACE, limits(4), 10)
                .expect("create ledger"),
        );
        assert!(matches!(
            PersistentReplayLedger::load(
                &substituted_path,
                b"different.credential.class",
                limits(4),
                10,
            ),
            Err(ReplayLedgerError::Snapshot(message)) if message.contains("namespace")
        ));
    }

    #[test]
    fn durable_ledger_rejects_over_ttl_or_over_capacity_snapshots() {
        let directory = tempdir().expect("temporary directory");
        let namespace_digest = namespace_digest(NAMESPACE).expect("namespace digest");

        let over_ttl_path = directory.path().join("over-ttl.norito");
        let over_ttl = ReplayLedgerSnapshotV1 {
            version: SNAPSHOT_VERSION_V1,
            namespace_digest,
            high_watermark_ms: 10,
            entries: vec![ReplayLedgerSnapshotEntryV1 {
                id: [0x01; 32],
                expires_at_ms: 1_011,
            }],
        };
        fs::write(
            &over_ttl_path,
            encode_canonical(&over_ttl).expect("encode over-TTL snapshot"),
        )
        .expect("write over-TTL snapshot");
        assert!(matches!(
            PersistentReplayLedger::load(&over_ttl_path, NAMESPACE, limits(4), 10),
            Err(ReplayLedgerError::Snapshot(message)) if message.contains("max_ttl_ms")
        ));

        let over_capacity_path = directory.path().join("over-capacity.norito");
        let over_capacity = ReplayLedgerSnapshotV1 {
            version: SNAPSHOT_VERSION_V1,
            namespace_digest,
            high_watermark_ms: 10,
            entries: vec![
                ReplayLedgerSnapshotEntryV1 {
                    id: [0x01; 32],
                    expires_at_ms: 20,
                },
                ReplayLedgerSnapshotEntryV1 {
                    id: [0x02; 32],
                    expires_at_ms: 30,
                },
            ],
        };
        fs::write(
            &over_capacity_path,
            encode_canonical(&over_capacity).expect("encode over-capacity snapshot"),
        )
        .expect("write over-capacity snapshot");
        assert!(matches!(
            PersistentReplayLedger::load(&over_capacity_path, NAMESPACE, limits(1), 10),
            Err(ReplayLedgerError::Snapshot(message)) if message.contains("capacity")
        ));
    }

    #[test]
    fn durable_ledger_excludes_concurrent_owners() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("locked.norito");
        let first =
            PersistentReplayLedger::load(&path, NAMESPACE, limits(4), 10).expect("first owner");
        let error = PersistentReplayLedger::load(&path, NAMESPACE, limits(4), 10)
            .expect_err("second owner must fail");
        assert!(matches!(error, ReplayLedgerError::Io(_)));
        drop(first);
        PersistentReplayLedger::load(&path, NAMESPACE, limits(4), 10)
            .expect("lock released on drop");
    }
}
