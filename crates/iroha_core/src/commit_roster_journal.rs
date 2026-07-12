//! Durable commit-roster journal persisted alongside the block store.
//!
//! This journal keeps per-height commit certificates and validator set
//! checkpoints so block-sync consumers can rebuild validator rosters after a
//! restart without depending on in-memory status caches.

use std::{
    collections::{BTreeMap, btree_map::Entry},
    fs,
    io::{self, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::BlockHeader,
    consensus::{Qc, VALIDATOR_SET_HASH_VERSION_V1, ValidatorSetCheckpoint},
};
use iroha_logger::warn;
use norito::{
    codec::{Decode, Encode},
    decode_from_bytes, to_bytes,
};
use thiserror::Error;

use crate::sumeragi::{
    consensus::{NPOS_TAG, PERMISSIONED_TAG, Phase},
    stake_snapshot::CommitStakeSnapshot,
};

/// Persisted commit-roster journal payload.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
struct PersistedCommitRosters {
    /// Journal version for format control.
    version: u32,
    /// Shared stake snapshots referenced by stored commit roster entries.
    #[norito(default)]
    stake_snapshots: Vec<CommitStakeSnapshot>,
    /// Stored commit roster entries.
    entries: Vec<CommitRosterRecord>,
}

/// Persisted commit-roster entry.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
struct CommitRosterRecord {
    /// Block height certified by this entry.
    height: u64,
    /// Block hash certified by this entry.
    block_hash: HashOf<BlockHeader>,
    /// Commit certificate for the block.
    commit_qc: Qc,
    /// Validator set checkpoint for the block.
    validator_checkpoint: ValidatorSetCheckpoint,
    /// Optional index into the payload-level stake snapshot table.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    stake_snapshot_index: Option<u32>,
    /// Optional stake snapshot aligned to the validator set.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    stake_snapshot: Option<CommitStakeSnapshot>,
}

/// Errors returned when loading or persisting commit rosters.
#[derive(Debug, Error)]
pub enum CommitRosterJournalError {
    /// Failed to read the persisted journal.
    #[error("failed to read commit roster journal {path}: {source}")]
    Read {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to decode the persisted journal.
    #[error("failed to decode commit roster journal {path}: {source}")]
    Decode {
        /// Path that failed.
        path: PathBuf,
        /// Source decode error.
        #[source]
        source: norito::core::Error,
    },
    /// Failed to write the journal to disk.
    #[error("failed to persist commit roster journal {path}: {source}")]
    Write {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to encode the journal payload.
    #[error("failed to encode commit roster journal: {0}")]
    Encode(#[source] norito::core::Error),
    /// Persisted journal uses an unsupported version.
    #[error("unsupported commit roster journal version {version} at {path}")]
    UnsupportedVersion {
        /// Path for the journal.
        path: PathBuf,
        /// Unsupported version encountered.
        version: u32,
    },
    /// A decodable row does not describe one exact signed commit subject.
    #[error("invalid commit roster journal entry at height {height} in {path}: {reason}")]
    InvalidEntry {
        /// Path for the journal.
        path: PathBuf,
        /// Height carried by the invalid row.
        height: u64,
        /// Stable validation failure reason.
        reason: &'static str,
    },
}

/// Snapshot combining commit certificate and validator checkpoint for a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRosterSnapshot {
    /// Commit certificate for the block.
    pub commit_qc: Qc,
    /// Validator set checkpoint for the block.
    pub validator_checkpoint: ValidatorSetCheckpoint,
    /// Optional stake snapshot aligned to the validator set.
    pub stake_snapshot: Option<CommitStakeSnapshot>,
}

/// Journal that records commit rosters derived from committed blocks.
#[derive(Debug, Clone)]
pub struct CommitRosterJournal {
    entries: BTreeMap<(u64, HashOf<BlockHeader>), CommitRosterSnapshot>,
    path: PathBuf,
    retention: NonZeroUsize,
    dirty: bool,
}

impl CommitRosterJournal {
    /// Extra non-genesis row retained for an authenticated successor before Kura commits it.
    const AUTHENTICATED_PRE_KURA_SUCCESSOR_RESERVE: usize = 1;

    /// Filename used to persist commit roster journals next to the block store.
    pub const JOURNAL_FILE: &'static str = "commit-rosters.norito";
    const JOURNAL_VERSION: u32 = 2;

    /// Build the canonical journal path under the provided root.
    #[must_use]
    pub fn journal_path(root: &Path) -> PathBuf {
        if root.as_os_str().is_empty() {
            PathBuf::new()
        } else {
            root.join(Self::JOURNAL_FILE)
        }
    }

    /// Construct a fresh journal with no entries.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, retention: NonZeroUsize) -> Self {
        Self {
            entries: BTreeMap::new(),
            path: path.into(),
            retention,
            dirty: false,
        }
    }

    /// Load a journal from disk, accepting only exact duplicate rows for one block subject.
    ///
    /// Missing files are treated as empty journals. Unsupported versions surface an error.
    ///
    /// # Errors
    ///
    /// Returns [`CommitRosterJournalError::Read`], [`CommitRosterJournalError::Decode`],
    /// [`CommitRosterJournalError::UnsupportedVersion`], or
    /// [`CommitRosterJournalError::InvalidEntry`] when the durable payload cannot be accepted.
    pub fn load(
        path: impl Into<PathBuf>,
        retention: NonZeroUsize,
    ) -> Result<Self, CommitRosterJournalError> {
        let path = path.into();
        let mut journal = Self::new(path.clone(), retention);
        let tmp_path = path.with_extension("norito.tmp");
        if path.as_os_str().is_empty() {
            return Ok(journal);
        }

        let main = if path.exists() {
            Some(Self::load_persisted(&path))
        } else {
            None
        };
        let tmp = if tmp_path.exists() {
            Some(Self::load_persisted(&tmp_path))
        } else {
            None
        };

        let (persisted, read_path) = match (tmp, main) {
            (None, None) => return Ok(journal),
            (Some(Ok(persisted)), _) => (persisted, tmp_path.clone()),
            (Some(Err(tmp_err)), None) => return Err(tmp_err),
            (Some(Err(_)) | None, Some(Ok(persisted))) => (persisted, path.clone()),
            (None | Some(Err(_)), Some(Err(err))) => return Err(err),
        };

        let PersistedCommitRosters {
            version: _,
            stake_snapshots,
            entries,
        } = persisted;
        let persisted_entry_count = entries.len();

        let mut decoded_entries = BTreeMap::new();
        for entry in entries {
            let snapshot = Self::validate_record(&read_path, entry, &stake_snapshots)?;
            let key = (
                snapshot.commit_qc.height,
                snapshot.commit_qc.subject_block_hash,
            );
            match decoded_entries.entry(key) {
                Entry::Occupied(existing) if existing.get() != &snapshot => {
                    return Err(CommitRosterJournalError::InvalidEntry {
                        path: read_path,
                        height: key.0,
                        reason: "divergent duplicate rows for the same block subject",
                    });
                }
                Entry::Occupied(_) => {}
                Entry::Vacant(entry) => {
                    entry.insert(snapshot);
                }
            }
        }

        journal.entries = decoded_entries;

        if read_path != path {
            Self::promote_temp_journal(&read_path, &path);
        }

        // Duplicate rows and retention can make memory differ from disk. In that case the next
        // authenticated durability boundary also repairs the journal payload.
        journal.dirty = journal.entries.len() != persisted_entry_count;
        journal.enforce_retention();
        Ok(journal)
    }

    fn load_persisted(path: &Path) -> Result<PersistedCommitRosters, CommitRosterJournalError> {
        let bytes = fs::read(path).map_err(|source| CommitRosterJournalError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let persisted: PersistedCommitRosters =
            decode_from_bytes(&bytes).map_err(|source| CommitRosterJournalError::Decode {
                path: path.to_path_buf(),
                source,
            })?;
        if persisted.version != Self::JOURNAL_VERSION {
            return Err(CommitRosterJournalError::UnsupportedVersion {
                path: path.to_path_buf(),
                version: persisted.version,
            });
        }
        Ok(persisted)
    }

    fn validate_record(
        path: &Path,
        entry: CommitRosterRecord,
        stake_snapshots: &[CommitStakeSnapshot],
    ) -> Result<CommitRosterSnapshot, CommitRosterJournalError> {
        let entry_height = entry.height;
        let invalid = |reason| CommitRosterJournalError::InvalidEntry {
            path: path.to_path_buf(),
            height: entry_height,
            reason,
        };
        let qc = &entry.commit_qc;
        let checkpoint = &entry.validator_checkpoint;
        if entry.height == 0 {
            return Err(invalid("height is zero"));
        }
        if qc.phase != Phase::Commit {
            return Err(invalid("certificate phase is not Commit"));
        }
        if qc.highest_qc.is_some() {
            return Err(invalid("commit certificate carries a highest-QC reference"));
        }
        if !matches!(qc.mode_tag.as_str(), PERMISSIONED_TAG | NPOS_TAG) {
            return Err(invalid("certificate mode tag is unsupported"));
        }
        if entry.height != qc.height || entry.block_hash != qc.subject_block_hash {
            return Err(invalid("certificate subject does not match row key"));
        }
        if entry.height != checkpoint.height || entry.block_hash != checkpoint.block_hash {
            return Err(invalid("checkpoint subject does not match row key"));
        }
        if qc.validator_set.is_empty() {
            return Err(invalid("validator set is empty"));
        }
        if qc
            .validator_set
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != qc.validator_set.len()
        {
            return Err(invalid("validator set contains duplicate peers"));
        }
        if qc.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1
            || qc.validator_set_hash != HashOf::new(&qc.validator_set)
        {
            return Err(invalid("certificate validator-set commitment is invalid"));
        }
        if checkpoint.view != qc.view
            || checkpoint.validator_set_hash_version != qc.validator_set_hash_version
            || checkpoint.validator_set_hash != qc.validator_set_hash
            || checkpoint.validator_set != qc.validator_set
            || checkpoint.parent_state_root != qc.parent_state_root
            || checkpoint.post_state_root != qc.post_state_root
            || checkpoint.chain_order_hash != qc.chain_order_hash
            || checkpoint.rechain_seq != qc.rechain_seq
            || checkpoint.signers_bitmap != qc.aggregate.signers_bitmap
            || checkpoint.bls_aggregate_signature != qc.aggregate.bls_aggregate_signature
        {
            return Err(invalid(
                "checkpoint does not exactly match the signed certificate subject",
            ));
        }
        if checkpoint.expires_at_height.is_some() {
            return Err(invalid("canonical checkpoint carries an expiry"));
        }
        let expected_bitmap_len = qc.validator_set.len().div_ceil(8);
        if qc.aggregate.signers_bitmap.len() != expected_bitmap_len {
            return Err(invalid("signer bitmap length does not match validator set"));
        }
        if let Some(last) = qc.aggregate.signers_bitmap.last().copied() {
            let used_bits = qc.validator_set.len() % 8;
            if used_bits != 0 && last & !((1_u8 << used_bits) - 1) != 0 {
                return Err(invalid("signer bitmap sets bits outside validator set"));
            }
        }
        let zero_root = Hash::prehashed([0; Hash::LENGTH]);
        let genesis_stub = entry.height == 1
            && qc.view == 0
            && qc.epoch == 0
            && qc.rechain_seq == 0
            && qc.parent_state_root == zero_root
            && qc.post_state_root == zero_root
            && qc.aggregate.bls_aggregate_signature.is_empty()
            && qc.aggregate.signers_bitmap.iter().all(|byte| *byte == 0);
        if entry.height == 1 && !genesis_stub {
            return Err(invalid(
                "height-one certificate is not the canonical unsigned genesis stub",
            ));
        }
        if !genesis_stub && qc.aggregate.bls_aggregate_signature.is_empty() {
            return Err(invalid("non-genesis certificate signature is empty"));
        }
        let stake_snapshot = match (entry.stake_snapshot, entry.stake_snapshot_index) {
            (Some(_), Some(_)) => {
                return Err(invalid("stake snapshot is both inline and indexed"));
            }
            (Some(snapshot), None) => Some(snapshot),
            (None, Some(index)) => {
                let index = usize::try_from(index)
                    .map_err(|_| invalid("stake snapshot index exceeds usize"))?;
                Some(
                    stake_snapshots
                        .get(index)
                        .cloned()
                        .ok_or_else(|| invalid("stake snapshot index is out of bounds"))?,
                )
            }
            (None, None) => None,
        };
        if stake_snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.matches_roster(&qc.validator_set))
        {
            return Err(invalid("stake snapshot does not match validator set"));
        }
        if genesis_stub && stake_snapshot.is_some() {
            return Err(invalid(
                "canonical genesis certificate carries a stake snapshot",
            ));
        }
        match qc.mode_tag.as_str() {
            PERMISSIONED_TAG if stake_snapshot.is_some() => {
                return Err(invalid("permissioned certificate carries a stake snapshot"));
            }
            NPOS_TAG if entry.height > 1 && stake_snapshot.is_none() => {
                return Err(invalid(
                    "non-genesis NPoS certificate lacks a stake snapshot",
                ));
            }
            PERMISSIONED_TAG | NPOS_TAG => {}
            _ => unreachable!("unsupported mode rejected before snapshot decoding"),
        }
        Ok(CommitRosterSnapshot {
            commit_qc: entry.commit_qc,
            validator_checkpoint: entry.validator_checkpoint,
            stake_snapshot,
        })
    }

    fn promote_temp_journal(from: &Path, to: &Path) {
        if let Err(err) = fs::rename(from, to) {
            if to.exists() {
                if let Err(remove_err) = fs::remove_file(to) {
                    warn!(
                        ?remove_err,
                        path = %to.display(),
                        "failed to remove commit roster journal before promotion"
                    );
                    return;
                }
                if let Err(err) = fs::rename(from, to) {
                    warn!(
                        ?err,
                        from = %from.display(),
                        to = %to.display(),
                        "failed to promote commit roster journal temp file after removal"
                    );
                    return;
                }
            } else {
                warn!(
                    ?err,
                    from = %from.display(),
                    to = %to.display(),
                    "failed to promote commit roster journal temp file"
                );
                return;
            }
        }
        if let Some(parent) = to.parent() {
            if let Err(err) = sync_dir(parent) {
                warn!(
                    ?err,
                    path = %parent.display(),
                    "failed to sync commit roster journal parent after temp promotion"
                );
            }
        }
    }

    /// Insert an exact commit-roster tuple without replacing a prepared tuple for the same block.
    ///
    /// Returns `true` when the tuple was inserted or was an exact retry. Returns `false` when the
    /// journal already contains a different QC, checkpoint, or stake snapshot for the same
    /// `(height, block_hash)` key. The first accepted tuple remains immutable.
    pub fn upsert(
        &mut self,
        commit_qc: Qc,
        validator_checkpoint: ValidatorSetCheckpoint,
        stake_snapshot: Option<CommitStakeSnapshot>,
    ) -> bool {
        let key = (commit_qc.height, commit_qc.subject_block_hash);
        let snapshot = CommitRosterSnapshot {
            commit_qc,
            validator_checkpoint,
            stake_snapshot,
        };
        let accepted = match self.entries.entry(key) {
            Entry::Occupied(entry) => entry.get() == &snapshot,
            Entry::Vacant(entry) => {
                entry.insert(snapshot);
                self.dirty = true;
                true
            }
        };
        self.enforce_retention();
        accepted
    }

    /// Return whether the in-memory snapshot has changes not yet acknowledged by persistence.
    #[must_use]
    pub fn needs_persistence(&self) -> bool {
        self.dirty
    }

    /// Persist the journal to disk.
    ///
    /// # Errors
    ///
    /// Returns [`CommitRosterJournalError::Write`] when the journal cannot be written or
    /// [`CommitRosterJournalError::Encode`] when encoding fails.
    pub fn persist(&mut self) -> Result<(), CommitRosterJournalError> {
        self.persist_durable()
    }

    fn persist_durable(&mut self) -> Result<(), CommitRosterJournalError> {
        if self.path.as_os_str().is_empty() {
            self.dirty = false;
            return Ok(());
        }
        // Ensure persisted payload honours the configured retention window.
        self.enforce_retention();
        let mut stake_snapshots = Vec::new();
        let payload = PersistedCommitRosters {
            version: Self::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: self
                .entries
                .iter()
                .map(|((height, block_hash), snapshot)| {
                    let mut inline_stake_snapshot = None;
                    let stake_snapshot_index = snapshot.stake_snapshot.as_ref().and_then(|stake| {
                        let position = stake_snapshots
                            .iter()
                            .position(|existing| existing == stake)
                            .unwrap_or_else(|| {
                                stake_snapshots.push(stake.clone());
                                stake_snapshots.len() - 1
                            });
                        u32::try_from(position).ok().or_else(|| {
                            inline_stake_snapshot = Some(stake.clone());
                            None
                        })
                    });
                    CommitRosterRecord {
                        height: *height,
                        block_hash: *block_hash,
                        commit_qc: snapshot.commit_qc.clone(),
                        validator_checkpoint: snapshot.validator_checkpoint.clone(),
                        stake_snapshot_index,
                        stake_snapshot: inline_stake_snapshot,
                    }
                })
                .collect(),
        };
        let payload = PersistedCommitRosters {
            stake_snapshots,
            ..payload
        };
        let bytes = to_bytes(&payload).map_err(CommitRosterJournalError::Encode)?;
        if let Some(parent) = self.path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                return Err(CommitRosterJournalError::Write {
                    path: self.path.clone(),
                    source: err,
                });
            }
        }
        let tmp_path = self.path.with_extension("norito.tmp");
        {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(|source| CommitRosterJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                })?;
            file.write_all(&bytes)
                .and_then(|()| file.flush())
                .map_err(|source| CommitRosterJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                })?;
            file.sync_data()
                .map_err(|source| CommitRosterJournalError::Write {
                    path: tmp_path.clone(),
                    source,
                })?;
        }
        if let Err(source) = fs::rename(&tmp_path, &self.path) {
            if source.kind() == io::ErrorKind::AlreadyExists {
                fs::remove_file(&self.path).map_err(|source| CommitRosterJournalError::Write {
                    path: self.path.clone(),
                    source,
                })?;
                fs::rename(&tmp_path, &self.path).map_err(|source| {
                    CommitRosterJournalError::Write {
                        path: self.path.clone(),
                        source,
                    }
                })?;
            } else {
                return Err(CommitRosterJournalError::Write {
                    path: self.path.clone(),
                    source,
                });
            }
        }
        if let Some(parent) = self.path.parent() {
            sync_dir(parent).map_err(|source| CommitRosterJournalError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        self.dirty = false;
        Ok(())
    }

    /// Drop entries above `height` and persist the updated journal.
    ///
    /// # Errors
    ///
    /// Returns [`CommitRosterJournalError::Write`] or [`CommitRosterJournalError::Encode`] when
    /// persistence fails.
    pub fn truncate_to_height(&mut self, height: u64) -> Result<(), CommitRosterJournalError> {
        let before = self.entries.len();
        self.entries
            .retain(|(entry_height, _), _| *entry_height <= height);
        if self.entries.len() == before {
            return Ok(());
        }
        self.dirty = true;
        self.persist()
    }

    /// Retrieve the snapshot for `height`/`block_hash` if present.
    #[must_use]
    pub fn get(
        &self,
        height: u64,
        block_hash: HashOf<BlockHeader>,
    ) -> Option<CommitRosterSnapshot> {
        self.entries.get(&(height, block_hash)).cloned()
    }

    /// Re-open the durable journal and require an exact tuple match.
    ///
    /// An empty path is reserved for in-memory unit-test journals, where no durable artifact
    /// exists to re-open. Every production path is decoded from disk so stale in-memory state can
    /// never satisfy a pre-Kura recovery-fence readback after deletion or corruption.
    #[must_use]
    pub fn durable_entry_matches_exact(
        &self,
        commit_qc: &Qc,
        checkpoint: &ValidatorSetCheckpoint,
        stake_snapshot: Option<&CommitStakeSnapshot>,
    ) -> bool {
        let snapshot = if self.path.as_os_str().is_empty() {
            self.get(commit_qc.height, commit_qc.subject_block_hash)
        } else {
            let durable = match Self::load(self.path.clone(), self.retention) {
                Ok(journal) => journal,
                Err(err) => {
                    warn!(
                        ?err,
                        path = %self.path.display(),
                        height = commit_qc.height,
                        block = %commit_qc.subject_block_hash,
                        "commit roster durable exact readback failed"
                    );
                    return false;
                }
            };
            durable.get(commit_qc.height, commit_qc.subject_block_hash)
        };
        snapshot.is_some_and(|snapshot| {
            snapshot.commit_qc == *commit_qc
                && snapshot.validator_checkpoint == *checkpoint
                && snapshot.stake_snapshot.as_ref() == stake_snapshot
        })
    }

    /// Return all stored snapshots in height/hash order.
    #[must_use]
    pub fn snapshots(&self) -> Vec<CommitRosterSnapshot> {
        self.entries.values().cloned().collect()
    }

    /// Return whether the durable journal state contains any entry above `height`.
    #[must_use]
    pub(crate) fn has_entries_above(&self, height: u64) -> bool {
        self.entries
            .keys()
            .next_back()
            .is_some_and(|(entry_height, _)| *entry_height > height)
    }

    #[cfg(test)]
    pub(crate) fn empty_payload_bytes_for_version(version: u32) -> Vec<u8> {
        to_bytes(&PersistedCommitRosters {
            version,
            stake_snapshots: Vec::new(),
            entries: Vec::new(),
        })
        .expect("encode empty commit-roster test payload")
    }

    fn enforce_retention(&mut self) {
        // The configured window counts recent non-genesis *heights*, not rows. Every row at a
        // retained height must survive until restart authentication can detect independently valid
        // conflicting QCs. The canonical unsigned genesis stub is a permanent restart anchor and
        // one additional height is reserved for the authenticated pre-Kura successor.
        let retained_non_genesis_heights = self
            .retention
            .get()
            .saturating_add(Self::AUTHENTICATED_PRE_KURA_SUCCESSOR_RESERVE);
        let mut non_genesis_heights = self
            .entries
            .keys()
            .filter(|(height, _)| *height != 1)
            .map(|(height, _)| *height)
            .collect::<Vec<_>>();
        non_genesis_heights.dedup();
        let excess_heights = non_genesis_heights
            .len()
            .saturating_sub(retained_non_genesis_heights);
        if excess_heights == 0 {
            return;
        }
        let first_retained_height = non_genesis_heights[excess_heights];
        let before = self.entries.len();
        self.entries
            .retain(|(height, _), _| *height == 1 || *height >= first_retained_height);
        self.dirty |= self.entries.len() != before;
    }
}

fn sync_dir(path: &Path) -> std::io::Result<()> {
    let file = fs::File::open(path)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write, num::NonZeroU64, path::Path};

    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{
        block::BlockHeader, consensus::VALIDATOR_SET_HASH_VERSION_V1, peer::PeerId,
    };
    use iroha_primitives::numeric::Numeric;
    use tempfile::tempdir;

    use super::*;
    use crate::sumeragi::{
        consensus::{NPOS_TAG, PERMISSIONED_TAG, Phase, QcAggregate},
        stake_snapshot::CommitStakeSnapshotEntry,
    };

    fn sample_cert(view: u64) -> (Qc, ValidatorSetCheckpoint) {
        cert_with_height(2, view)
    }

    fn checked_random_bls_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("generate checked commit roster journal BLS fixture keypair")
    }

    fn cert_with_height(height: u64, view: u64) -> (Qc, ValidatorSetCheckpoint) {
        let kp = checked_random_bls_keypair();
        let peer = PeerId::new(kp.public_key().clone());
        cert_with_height_and_roster(height, view, vec![peer])
    }

    fn cert_with_height_and_roster(
        height: u64,
        view: u64,
        roster: Vec<PeerId>,
    ) -> (Qc, ValidatorSetCheckpoint) {
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero"),
            None,
            None,
            None,
            0,
            0,
        );
        let block_hash = header.hash();
        let signers_bitmap = vec![0b0000_0001];
        let bls_aggregate_signature = vec![0xAB; 96];
        let cert = Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height,
            view,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster.clone(),
            aggregate: QcAggregate {
                signers_bitmap: signers_bitmap.clone(),
                bls_aggregate_signature: bls_aggregate_signature.clone(),
            },
        };
        let checkpoint = ValidatorSetCheckpoint::new(
            height,
            view,
            block_hash,
            cert.parent_state_root,
            cert.post_state_root,
            roster,
            signers_bitmap,
            bls_aggregate_signature,
            VALIDATOR_SET_HASH_VERSION_V1,
            None,
        );
        (cert, checkpoint)
    }

    fn sample_stake_snapshot(roster: &[PeerId]) -> CommitStakeSnapshot {
        CommitStakeSnapshot {
            validator_set_hash: HashOf::new(&roster.to_vec()),
            entries: roster
                .iter()
                .map(|peer| CommitStakeSnapshotEntry {
                    peer_id: peer.clone(),
                    stake: Numeric::new(10, 0),
                })
                .collect(),
        }
    }

    fn retention(limit: usize) -> NonZeroUsize {
        NonZeroUsize::new(limit).expect("non-zero retention")
    }

    #[test]
    fn journal_roundtrips_entries() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert.clone(), checkpoint.clone(), None);
        journal.persist().expect("persist");

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(
            snapshots[0],
            CommitRosterSnapshot {
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot: None,
            }
        );
    }

    #[test]
    fn journal_durable_persist_clears_dirty_state() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert.clone(), checkpoint.clone(), None);
        assert!(journal.needs_persistence());
        journal.persist().expect("persist durable");
        assert!(!journal.needs_persistence());

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
        let snapshot = loaded
            .get(cert.height, cert.subject_block_hash)
            .expect("snapshot must be present");
        assert_eq!(snapshot.commit_qc, cert);
        assert_eq!(snapshot.validator_checkpoint, checkpoint);
    }

    #[test]
    fn journal_truncate_to_height_drops_future_entries() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert1, checkpoint1) = cert_with_height(1, 0);
        let (cert2, checkpoint2) = cert_with_height(2, 0);
        let mut journal = CommitRosterJournal::new(path, retention(4));
        journal.upsert(cert1.clone(), checkpoint1, None);
        journal.upsert(cert2.clone(), checkpoint2, None);
        assert!(journal.has_entries_above(1));

        journal.truncate_to_height(1).expect("truncate to height");
        assert!(!journal.has_entries_above(1));

        assert!(
            journal
                .get(cert1.height, cert1.subject_block_hash)
                .is_some()
        );
        assert!(
            journal
                .get(cert2.height, cert2.subject_block_hash)
                .is_none()
        );
    }

    #[test]
    fn journal_persist_overwrites_existing_file() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert1, checkpoint1) = cert_with_height(2, 1);
        let (cert2, checkpoint2) = cert_with_height(3, 1);

        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert1.clone(), checkpoint1, None);
        journal.persist().expect("persist first journal");

        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert2.clone(), checkpoint2.clone(), None);
        journal.persist().expect("persist second journal");

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load journal");
        assert!(
            loaded.get(cert1.height, cert1.subject_block_hash).is_none(),
            "old entry should be overwritten"
        );
        let snapshot = loaded
            .get(cert2.height, cert2.subject_block_hash)
            .expect("new entry should exist");
        assert_eq!(snapshot.commit_qc, cert2);
        assert_eq!(snapshot.validator_checkpoint, checkpoint2);
    }

    #[test]
    fn journal_prefers_temp_over_main() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let tmp_path = path.with_extension("norito.tmp");

        let (cert1, checkpoint1) = cert_with_height(2, 1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert1.clone(), checkpoint1.clone(), None);
        journal.persist().expect("persist main journal");

        let (cert2, checkpoint2) = cert_with_height(3, 1);
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![
                CommitRosterRecord {
                    height: cert1.height,
                    block_hash: cert1.subject_block_hash,
                    commit_qc: cert1.clone(),
                    validator_checkpoint: checkpoint1.clone(),
                    stake_snapshot_index: None,
                    stake_snapshot: None,
                },
                CommitRosterRecord {
                    height: cert2.height,
                    block_hash: cert2.subject_block_hash,
                    commit_qc: cert2.clone(),
                    validator_checkpoint: checkpoint2.clone(),
                    stake_snapshot_index: None,
                    stake_snapshot: None,
                },
            ],
        };
        let bytes = to_bytes(&payload).expect("encode temp journal");
        let mut file = File::create(&tmp_path).expect("create temp journal");
        file.write_all(&bytes).expect("write temp journal");
        file.flush().expect("flush temp journal");
        file.sync_data().expect("sync temp journal");

        let loaded = CommitRosterJournal::load(path.clone(), retention(4)).expect("load journal");
        assert!(loaded.get(cert2.height, cert2.subject_block_hash).is_some());
        assert!(loaded.get(cert1.height, cert1.subject_block_hash).is_some());
        assert!(!tmp_path.exists(), "temp journal should be promoted");
    }

    #[test]
    fn journal_preserves_prepared_tuple_against_higher_view_replacement() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (low_view_cert, low_view_checkpoint) = sample_cert(1);
        let (high_view_cert, high_view_checkpoint) = sample_cert(3);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        assert!(journal.upsert(low_view_cert.clone(), low_view_checkpoint.clone(), None));
        assert!(
            !journal.upsert(high_view_cert, high_view_checkpoint, None),
            "a divergent higher-view tuple must not replace prepared authority"
        );
        journal.persist().expect("persist");

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].commit_qc, low_view_cert);
        assert_eq!(snapshots[0].validator_checkpoint, low_view_checkpoint);
    }

    #[test]
    fn journal_exact_retry_is_idempotent() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path, retention(4));

        assert!(journal.upsert(cert.clone(), checkpoint.clone(), None));
        journal.persist().expect("persist prepared tuple");
        assert!(!journal.needs_persistence());

        assert!(
            journal.upsert(cert.clone(), checkpoint.clone(), None),
            "an exact retry must be accepted"
        );
        assert!(
            !journal.needs_persistence(),
            "upsert must not manufacture a logical change; the durability boundary still rewrites"
        );
        assert_eq!(
            journal.get(cert.height, cert.subject_block_hash),
            Some(CommitRosterSnapshot {
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot: None,
            })
        );
    }

    #[test]
    fn journal_exact_retry_repersists_deleted_durable_file() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));

        assert!(journal.upsert(cert.clone(), checkpoint.clone(), None));
        journal.persist().expect("persist prepared tuple");
        assert!(!journal.needs_persistence());
        std::fs::remove_file(&path).expect("delete durable journal");

        assert!(
            journal.upsert(cert.clone(), checkpoint.clone(), None),
            "the exact in-memory retry remains admissible"
        );
        assert!(
            !journal.needs_persistence(),
            "the adversary deletes disk state without changing the in-memory tuple"
        );
        journal
            .persist()
            .expect("an exact retry must rewrite and fsync the durable journal");

        assert!(journal.durable_entry_matches_exact(&cert, &checkpoint, None));
        assert!(
            path.exists(),
            "the exact retry must restore the deleted file"
        );
    }

    #[test]
    fn journal_durable_exact_readback_fails_closed_after_deletion_or_corruption() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        assert!(journal.upsert(cert.clone(), checkpoint.clone(), None));
        journal.persist().expect("persist prepared tuple");
        assert!(journal.durable_entry_matches_exact(&cert, &checkpoint, None));

        std::fs::remove_file(&path).expect("delete durable journal");
        assert!(
            !journal.durable_entry_matches_exact(&cert, &checkpoint, None),
            "stale memory must not hide deletion of the recovery fence"
        );

        std::fs::write(&path, b"corrupted commit roster journal")
            .expect("write corrupt durable journal");
        assert!(
            !journal.durable_entry_matches_exact(&cert, &checkpoint, None),
            "stale memory must not hide corruption of the recovery fence"
        );
    }

    #[test]
    fn journal_rejects_legacy_v1_payload() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let payload = PersistedCommitRosters {
            version: 1,
            stake_snapshots: Vec::new(),
            entries: vec![CommitRosterRecord {
                height: cert.height,
                block_hash: cert.subject_block_hash,
                commit_qc: cert.clone(),
                validator_checkpoint: checkpoint.clone(),
                stake_snapshot_index: None,
                stake_snapshot: None,
            }],
        };
        let bytes = norito::to_bytes(&payload).expect("encode payload");
        std::fs::write(&path, bytes).expect("write payload");

        let err = CommitRosterJournal::load(path, retention(4)).expect_err("reject v1 journal");
        assert!(
            matches!(
                err,
                CommitRosterJournalError::UnsupportedVersion { version: 1, .. }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn journal_load_rejects_divergent_duplicate_block_subject_rows() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (first_cert, first_checkpoint) = sample_cert(1);
        let (replacement_cert, replacement_checkpoint) = sample_cert(3);
        assert_eq!(
            first_cert.subject_block_hash, replacement_cert.subject_block_hash,
            "fixture must target one block subject"
        );
        let record =
            |commit_qc: Qc, validator_checkpoint: ValidatorSetCheckpoint| CommitRosterRecord {
                height: commit_qc.height,
                block_hash: commit_qc.subject_block_hash,
                commit_qc,
                validator_checkpoint,
                stake_snapshot_index: None,
                stake_snapshot: None,
            };
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![
                record(first_cert, first_checkpoint),
                record(replacement_cert, replacement_checkpoint),
            ],
        };
        std::fs::write(&path, to_bytes(&payload).expect("encode payload")).expect("write payload");

        let err = CommitRosterJournal::load(path, retention(4))
            .expect_err("divergent duplicate rows must fail closed");
        assert!(
            matches!(
                err,
                CommitRosterJournalError::InvalidEntry {
                    reason: "divergent duplicate rows for the same block subject",
                    ..
                }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn journal_load_accepts_exact_duplicate_rows_as_idempotent() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let record = CommitRosterRecord {
            height: cert.height,
            block_hash: cert.subject_block_hash,
            commit_qc: cert.clone(),
            validator_checkpoint: checkpoint.clone(),
            stake_snapshot_index: None,
            stake_snapshot: None,
        };
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![record.clone(), record],
        };
        std::fs::write(&path, to_bytes(&payload).expect("encode payload")).expect("write payload");

        let loaded = CommitRosterJournal::load(path, retention(4))
            .expect("exact duplicate rows are idempotent");
        assert_eq!(
            loaded.get(cert.height, cert.subject_block_hash),
            Some(CommitRosterSnapshot {
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot: None,
            })
        );
        assert!(
            loaded.needs_persistence(),
            "the next durability boundary should canonicalize duplicate rows"
        );
    }

    #[test]
    fn journal_rejects_checkpoint_that_differs_from_qc_subject() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, mut checkpoint) = sample_cert(1);
        checkpoint.post_state_root =
            iroha_crypto::Hash::prehashed([0xA5; iroha_crypto::Hash::LENGTH]);
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![CommitRosterRecord {
                height: cert.height,
                block_hash: cert.subject_block_hash,
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot_index: None,
                stake_snapshot: None,
            }],
        };
        std::fs::write(&path, to_bytes(&payload).expect("encode payload")).expect("write payload");

        let err = CommitRosterJournal::load(path, retention(4))
            .expect_err("reject mismatched signed subject");
        assert!(
            matches!(
                err,
                CommitRosterJournalError::InvalidEntry {
                    reason: "checkpoint does not exactly match the signed certificate subject",
                    ..
                }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn journal_rejects_row_key_that_differs_from_qc_subject() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![CommitRosterRecord {
                height: cert.height.saturating_add(1),
                block_hash: cert.subject_block_hash,
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot_index: None,
                stake_snapshot: None,
            }],
        };
        std::fs::write(&path, to_bytes(&payload).expect("encode payload")).expect("write payload");

        let err =
            CommitRosterJournal::load(path, retention(4)).expect_err("reject mismatched row key");
        assert!(
            matches!(
                err,
                CommitRosterJournalError::InvalidEntry {
                    reason: "certificate subject does not match row key",
                    ..
                }
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn journal_rejects_noncanonical_height_one_finality_metadata() {
        for case in ["nonzero_root", "nonzero_epoch", "nonzero_rechain", "signed"] {
            let dir = tempdir().expect("tempdir");
            let path = CommitRosterJournal::journal_path(dir.path());
            let (mut cert, mut checkpoint) = cert_with_height(1, 0);
            cert.aggregate.signers_bitmap.fill(0);
            cert.aggregate.bls_aggregate_signature.clear();
            checkpoint.signers_bitmap.fill(0);
            checkpoint.bls_aggregate_signature.clear();
            match case {
                "nonzero_root" => {
                    let root = Hash::prehashed([0xA5; Hash::LENGTH]);
                    cert.parent_state_root = root;
                    checkpoint.parent_state_root = root;
                }
                "nonzero_epoch" => cert.epoch = 1,
                "nonzero_rechain" => {
                    cert.rechain_seq = 1;
                    checkpoint.rechain_seq = 1;
                }
                "signed" => {
                    cert.aggregate.bls_aggregate_signature = vec![0xA5; 96];
                    checkpoint.bls_aggregate_signature = vec![0xA5; 96];
                }
                _ => unreachable!(),
            }
            let payload = PersistedCommitRosters {
                version: CommitRosterJournal::JOURNAL_VERSION,
                stake_snapshots: Vec::new(),
                entries: vec![CommitRosterRecord {
                    height: 1,
                    block_hash: cert.subject_block_hash,
                    commit_qc: cert,
                    validator_checkpoint: checkpoint,
                    stake_snapshot_index: None,
                    stake_snapshot: None,
                }],
            };
            std::fs::write(&path, to_bytes(&payload).expect("encode payload"))
                .expect("write payload");
            let err = CommitRosterJournal::load(path, retention(4))
                .expect_err("noncanonical height-one metadata must fail closed");
            assert!(
                matches!(
                    err,
                    CommitRosterJournalError::InvalidEntry {
                        reason: "height-one certificate is not the canonical unsigned genesis stub",
                        ..
                    }
                ),
                "unexpected {case} error: {err}"
            );
        }
    }

    #[test]
    fn journal_roundtrips_stake_snapshot() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (mut cert, checkpoint) = sample_cert(1);
        cert.mode_tag = NPOS_TAG.to_string();
        let stake_snapshot = sample_stake_snapshot(&cert.validator_set);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(
            cert.clone(),
            checkpoint.clone(),
            Some(stake_snapshot.clone()),
        );
        journal.persist().expect("persist");

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].stake_snapshot, Some(stake_snapshot));
    }

    #[test]
    fn journal_deduplicates_persisted_stake_snapshots() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let kp = checked_random_bls_keypair();
        let roster = vec![PeerId::new(kp.public_key().clone())];
        let stake_snapshot = sample_stake_snapshot(&roster);
        let (mut cert1, checkpoint1) = cert_with_height_and_roster(2, 0, roster.clone());
        let (mut cert2, checkpoint2) = cert_with_height_and_roster(3, 0, roster.clone());
        cert1.mode_tag = NPOS_TAG.to_string();
        cert2.mode_tag = NPOS_TAG.to_string();
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert1.clone(), checkpoint1, Some(stake_snapshot.clone()));
        journal.upsert(cert2.clone(), checkpoint2, Some(stake_snapshot.clone()));

        journal.persist().expect("persist");

        let bytes = std::fs::read(&path).expect("read journal");
        let payload: PersistedCommitRosters =
            decode_from_bytes(&bytes).expect("decode persisted journal");
        assert_eq!(payload.version, CommitRosterJournal::JOURNAL_VERSION);
        assert_eq!(payload.stake_snapshots, vec![stake_snapshot.clone()]);
        assert!(payload.entries.iter().all(|entry| {
            entry.stake_snapshot.is_none() && entry.stake_snapshot_index == Some(0)
        }));

        let loaded = CommitRosterJournal::load(path, retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 2);
        assert!(
            snapshots
                .iter()
                .all(|snapshot| snapshot.stake_snapshot == Some(stake_snapshot.clone()))
        );
    }

    #[test]
    fn journal_loads_from_temp_when_main_missing() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let tmp_path = path.with_extension("norito.tmp");
        let (cert, checkpoint) = sample_cert(1);
        let payload = PersistedCommitRosters {
            version: CommitRosterJournal::JOURNAL_VERSION,
            stake_snapshots: Vec::new(),
            entries: vec![CommitRosterRecord {
                height: cert.height,
                block_hash: cert.subject_block_hash,
                commit_qc: cert.clone(),
                validator_checkpoint: checkpoint.clone(),
                stake_snapshot_index: None,
                stake_snapshot: None,
            }],
        };
        let bytes = norito::to_bytes(&payload).expect("encode payload");
        std::fs::write(&tmp_path, bytes).expect("write temp payload");

        let loaded = CommitRosterJournal::load(path.clone(), retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(
            snapshots[0],
            CommitRosterSnapshot {
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot: None,
            }
        );
        assert!(path.exists(), "temp journal should be promoted");
        assert!(
            !tmp_path.exists(),
            "temp journal should be removed after promotion"
        );
    }

    #[test]
    fn journal_loads_from_temp_when_main_corrupted() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let tmp_path = path.with_extension("norito.tmp");
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert.clone(), checkpoint.clone(), None);
        journal.persist().expect("persist");

        std::fs::rename(&path, &tmp_path).expect("move journal to temp");
        std::fs::write(&path, b"corrupted").expect("write corrupted journal");

        let loaded = CommitRosterJournal::load(path.clone(), retention(4)).expect("load");
        let snapshots = loaded.snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(
            snapshots[0],
            CommitRosterSnapshot {
                commit_qc: cert,
                validator_checkpoint: checkpoint,
                stake_snapshot: None,
            }
        );
        assert!(path.exists(), "temp journal should be promoted");
        assert!(
            !tmp_path.exists(),
            "temp journal should be removed after promotion"
        );
    }

    #[test]
    fn journal_rejects_unsupported_version() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let payload = PersistedCommitRosters {
            version: 3,
            stake_snapshots: Vec::new(),
            entries: Vec::new(),
        };
        let bytes = norito::to_bytes(&payload).expect("encode payload");
        std::fs::write(&path, bytes).expect("write payload");

        let err = CommitRosterJournal::load(path, retention(4)).expect_err("unsupported version");
        assert!(matches!(
            err,
            CommitRosterJournalError::UnsupportedVersion { .. }
        ));
    }

    #[test]
    fn get_returns_matching_snapshot() {
        let (cert, checkpoint) = sample_cert(2);
        let mut journal = CommitRosterJournal::new(PathBuf::from("unused"), retention(4));
        journal.upsert(cert.clone(), checkpoint.clone(), None);

        let found = journal
            .get(cert.height, cert.subject_block_hash)
            .expect("snapshot must be present");
        assert_eq!(found.commit_qc, cert);
        assert_eq!(found.validator_checkpoint, checkpoint);

        assert!(
            journal
                .get(cert.height + 1, cert.subject_block_hash)
                .is_none(),
            "mismatched height should not return a snapshot"
        );
    }

    #[test]
    fn retention_drops_oldest_entries() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let mut journal = CommitRosterJournal::new(path.clone(), retention(2));
        for height in 1..=6 {
            let (mut cert, mut checkpoint) = cert_with_height(height, 0);
            if height == 1 {
                cert.aggregate.signers_bitmap.fill(0);
                cert.aggregate.bls_aggregate_signature.clear();
                checkpoint.signers_bitmap.fill(0);
                checkpoint.bls_aggregate_signature.clear();
            }
            journal.upsert(cert, checkpoint, None);
        }
        let snapshots = journal.snapshots();
        let heights: Vec<_> = snapshots
            .iter()
            .map(|snapshot| snapshot.commit_qc.height)
            .collect();
        assert_eq!(heights, vec![1, 4, 5, 6]);

        journal.persist().expect("persist");
        let reloaded = CommitRosterJournal::load(path, retention(2)).expect("load");
        let reloaded_heights: Vec<_> = reloaded
            .snapshots()
            .into_iter()
            .map(|snapshot| snapshot.commit_qc.height)
            .collect();
        assert_eq!(reloaded_heights, vec![1, 4, 5, 6]);
    }

    #[test]
    fn retention_one_keeps_all_tip_conflicts_and_prepared_successor_durable() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let mut journal = CommitRosterJournal::new(path.clone(), retention(1));

        let (mut genesis_qc, mut genesis_checkpoint) = cert_with_height(1, 0);
        genesis_qc.aggregate.signers_bitmap.fill(0);
        genesis_qc.aggregate.bls_aggregate_signature.clear();
        genesis_checkpoint.signers_bitmap.fill(0);
        genesis_checkpoint.bls_aggregate_signature.clear();
        let (tip_qc, tip_checkpoint) = cert_with_height(2, 0);
        let mut conflicting_tip_qc = tip_qc.clone();
        let conflicting_tip_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA5; Hash::LENGTH]));
        conflicting_tip_qc.subject_block_hash = conflicting_tip_hash;
        conflicting_tip_qc.parent_state_root = Hash::prehashed([0xB5; Hash::LENGTH]);
        let mut conflicting_tip_checkpoint = tip_checkpoint.clone();
        conflicting_tip_checkpoint.block_hash = conflicting_tip_hash;
        conflicting_tip_checkpoint.parent_state_root = conflicting_tip_qc.parent_state_root;
        let (prepared_successor_qc, prepared_successor_checkpoint) = cert_with_height(3, 0);

        assert!(journal.upsert(genesis_qc.clone(), genesis_checkpoint.clone(), None));
        assert!(journal.upsert(tip_qc.clone(), tip_checkpoint.clone(), None));
        assert!(journal.upsert(
            conflicting_tip_qc.clone(),
            conflicting_tip_checkpoint.clone(),
            None,
        ));
        assert!(journal.upsert(
            prepared_successor_qc.clone(),
            prepared_successor_checkpoint.clone(),
            None,
        ));
        journal
            .persist()
            .expect("persist genesis, committed tip, and prepared successor");

        assert!(journal.durable_entry_matches_exact(&genesis_qc, &genesis_checkpoint, None,));
        assert!(journal.durable_entry_matches_exact(&tip_qc, &tip_checkpoint, None));
        assert!(journal.durable_entry_matches_exact(
            &conflicting_tip_qc,
            &conflicting_tip_checkpoint,
            None,
        ));
        assert!(journal.durable_entry_matches_exact(
            &prepared_successor_qc,
            &prepared_successor_checkpoint,
            None,
        ));
        let reloaded = CommitRosterJournal::load(path, retention(1)).expect("reload journal");
        let heights = reloaded
            .snapshots()
            .into_iter()
            .map(|snapshot| snapshot.commit_qc.height)
            .collect::<Vec<_>>();
        assert_eq!(heights, vec![1, 2, 2, 3]);
    }

    #[test]
    fn journal_persist_removes_temp_file() {
        let dir = tempdir().expect("tempdir");
        let path = CommitRosterJournal::journal_path(dir.path());
        let (cert, checkpoint) = sample_cert(1);
        let mut journal = CommitRosterJournal::new(path.clone(), retention(4));
        journal.upsert(cert, checkpoint, None);
        journal.persist().expect("persist");

        let tmp_path = path.with_extension("norito.tmp");
        assert!(!tmp_path.exists(), "temp journal file should be removed");
    }

    #[test]
    fn journal_path_empty_root_is_empty() {
        let path = CommitRosterJournal::journal_path(Path::new(""));
        assert!(path.as_os_str().is_empty());
    }
}
