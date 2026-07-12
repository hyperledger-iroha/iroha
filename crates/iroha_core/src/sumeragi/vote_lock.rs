//! Durable local Prepare/Commit vote history for restart-safe consensus locking.
//!
//! A validator must recover its own signed vote history before it can safely vote after a
//! restart. The journal is deliberately local: remote votes remain reconstructible from peers,
//! while a lost local signature can let the validator sign a conflicting subject in a later view.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey, Signature};
use iroha_data_model::{ChainId, block::BlockHeader, consensus::QcVote};
use norito::{
    codec::{Decode, Encode},
    decode_from_bytes, to_bytes,
};
use thiserror::Error;

use super::consensus::{NPOS_TAG, PERMISSIONED_TAG, Phase, vote_preimage};

// Version 2 strengthens a local Commit vote into a height-wide durable lock. Version 1 only
// rejected double-signing inside one exact view, so loading it would silently lose the invariant
// this journal exists to provide after restart. This is a first-release format: reject rather
// than migrate an unsafe payload.
const JOURNAL_VERSION: u32 = 2;
const MAX_UNCOMMITTED_VOTES: usize = 65_536;

/// Errors returned while loading or durably updating local vote locks.
#[derive(Debug, Error)]
pub(super) enum LocalVoteLockError {
    /// The journal could not be read.
    #[error("failed to read local vote-lock journal {path}: {source}")]
    Read {
        /// Path that failed.
        path: PathBuf,
        /// Source I/O error.
        #[source]
        source: io::Error,
    },
    /// The journal payload could not be decoded.
    #[error("failed to decode local vote-lock journal {path}: {source}")]
    Decode {
        /// Path that failed.
        path: PathBuf,
        /// Source codec error.
        #[source]
        source: norito::core::Error,
    },
    /// The journal could not be encoded.
    #[error("failed to encode local vote-lock journal: {0}")]
    Encode(#[source] norito::core::Error),
    /// The journal could not be durably written.
    #[error("failed to persist local vote-lock journal {path}: {source}")]
    Write {
        /// Path that failed.
        path: PathBuf,
        /// Source I/O error.
        #[source]
        source: io::Error,
    },
    /// The journal belongs to a different chain or validator.
    #[error("local vote-lock journal identity does not match this validator and chain")]
    IdentityMismatch,
    /// The journal format is newer or otherwise unsupported.
    #[error("unsupported local vote-lock journal version {0}")]
    UnsupportedVersion(u32),
    /// A persisted entry is not an authenticated local Prepare/Commit vote.
    #[error("invalid persisted local vote lock at height {height}, view {view}: {reason}")]
    InvalidVote {
        /// Vote height.
        height: u64,
        /// Vote view.
        view: u64,
        /// Validation failure reason.
        reason: &'static str,
    },
    /// The validator attempted to sign two different subjects in the same exact vote slot.
    #[error("conflicting local vote lock at height {height}, view {view}")]
    ConflictingSlot {
        /// Conflicting vote height.
        height: u64,
        /// Conflicting vote view.
        view: u64,
    },
    /// A local Commit lock already binds this height to another finality subject.
    #[error(
        "conflicting local Commit lock at height {height}: locked view {locked_view}, attempted view {attempted_view}"
    )]
    ConflictingCommitLock {
        /// Locked vote height.
        height: u64,
        /// View of the first durable Commit lock.
        locked_view: u64,
        /// View of the conflicting signing attempt.
        attempted_view: u64,
    },
    /// The validator attempted to create a new vote lock at an already committed height.
    #[error("local vote lock height {height} is not above committed height {committed_height}")]
    AlreadyCommitted {
        /// Vote height.
        height: u64,
        /// Locally committed height observed before signing.
        committed_height: u64,
    },
    /// Uncommitted vote history exceeded the fail-closed journal bound.
    #[error("local vote-lock journal reached its {0}-entry safety bound")]
    CapacityExceeded(usize),
    /// A recovered safety-halt marker is malformed.
    #[error("invalid persisted consensus safety-halt marker: {0}")]
    InvalidSafetyHalt(&'static str),
}

impl LocalVoteLockError {
    /// Stable operator-facing reason used when a local signing safety check fails closed.
    pub(super) const fn safety_halt_reason(&self) -> &'static str {
        match self {
            Self::ConflictingSlot { .. } => "local_vote_lock_conflict",
            Self::ConflictingCommitLock { .. } => "local_commit_vote_lock_conflict",
            Self::AlreadyCommitted { .. } => "local_vote_at_committed_height",
            Self::CapacityExceeded(_) => "local_vote_lock_capacity_exceeded",
            Self::InvalidVote { .. } | Self::InvalidSafetyHalt(_) => {
                "local_vote_lock_validation_failed"
            }
            Self::Read { .. }
            | Self::Decode { .. }
            | Self::Encode(_)
            | Self::Write { .. }
            | Self::IdentityMismatch
            | Self::UnsupportedVersion(_) => "local_vote_lock_persistence_failed",
        }
    }
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
struct PersistedVoteLock {
    mode_tag: String,
    vote: QcVote,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
struct PersistedLocalVoteLocks {
    version: u32,
    chain_id: ChainId,
    validator_key: PublicKey,
    entries: Vec<PersistedVoteLock>,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
struct PersistedSafetyHalt {
    version: u32,
    chain_id: ChainId,
    validator_key: PublicKey,
    reason: String,
    height: u64,
    epoch: u64,
    first_block_hash: Option<HashOf<BlockHeader>>,
    conflicting_block_hash: Option<HashOf<BlockHeader>>,
    first_parent_state_root: Option<Hash>,
    first_post_state_root: Option<Hash>,
    conflicting_parent_state_root: Option<Hash>,
    conflicting_post_state_root: Option<Hash>,
}

/// Identity-bound sink for the durable consensus safety-halt marker.
///
/// Commit workers receive this value directly so a fatal Kura/sidecar transition can make the
/// halt restart-visible before publishing its result back to the actor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SafetyHaltSink {
    path: PathBuf,
    chain_id: ChainId,
    validator_key: PublicKey,
}

impl SafetyHaltSink {
    /// Bind a halt sink to the journal-adjacent marker path and local validator identity.
    pub(super) fn new(
        vote_lock_path: &Path,
        chain_id: &ChainId,
        validator_key: &PublicKey,
    ) -> Self {
        Self {
            path: safety_halt_path_for(vote_lock_path),
            chain_id: chain_id.clone(),
            validator_key: validator_key.clone(),
        }
    }

    /// Persist and read back an active halt marker using this sink's exact identity and path.
    pub(super) fn record(
        &self,
        halt: &super::status::ConsensusSafetyHaltSnapshot,
    ) -> Result<super::status::ConsensusSafetyHaltSnapshot, LocalVoteLockError> {
        if self.path.as_os_str().is_empty() {
            return Err(LocalVoteLockError::InvalidSafetyHalt(
                "safety-halt path is empty",
            ));
        }
        let unwinding = std::thread::panicking();
        if !unwinding
            && let Some(existing) =
                load_safety_halt(&self.path, &self.chain_id, &self.validator_key)?
        {
            return Ok(existing);
        }
        if !halt.active {
            return Err(LocalVoteLockError::InvalidSafetyHalt(
                "marker is not active",
            ));
        }
        let reason = halt
            .reason
            .as_deref()
            .filter(|reason| !reason.is_empty())
            .ok_or(LocalVoteLockError::InvalidSafetyHalt("reason is empty"))?;
        let payload = PersistedSafetyHalt {
            version: JOURNAL_VERSION,
            chain_id: self.chain_id.clone(),
            validator_key: self.validator_key.clone(),
            reason: reason.to_owned(),
            height: halt.height,
            epoch: halt.epoch,
            first_block_hash: halt.first_block_hash,
            conflicting_block_hash: halt.conflicting_block_hash,
            first_parent_state_root: halt.first_parent_state_root,
            first_post_state_root: halt.first_post_state_root,
            conflicting_parent_state_root: halt.conflicting_parent_state_root,
            conflicting_post_state_root: halt.conflicting_post_state_root,
        };
        let bytes = to_bytes(&payload).map_err(LocalVoteLockError::Encode)?;
        persist_bytes(&self.path, &bytes)?;
        // Norito's guarded decoder lazily installs its panic hook. That operation is forbidden
        // while this thread is already unwinding, so the transition-gate poison destructor stops
        // after the atomic write, file fsync, rename, and directory fsync. Normal worker paths
        // retain the strict authenticated readback below.
        if unwinding {
            return Ok(halt.clone());
        }
        let persisted = load_safety_halt(&self.path, &self.chain_id, &self.validator_key)?.ok_or(
            LocalVoteLockError::InvalidSafetyHalt("marker was not readable after persistence"),
        )?;
        if persisted != *halt {
            return Err(LocalVoteLockError::InvalidSafetyHalt(
                "marker readback does not match requested halt",
            ));
        }
        Ok(persisted)
    }
}

/// Authenticated, bounded local vote history persisted next to Kura.
#[derive(Debug)]
pub(super) struct LocalVoteLockJournal {
    path: PathBuf,
    chain_id: ChainId,
    validator_key: PublicKey,
    entries: Vec<PersistedVoteLock>,
    safety_halt: Option<super::status::ConsensusSafetyHaltSnapshot>,
}

impl LocalVoteLockJournal {
    /// Filename used beneath the Kura store root.
    pub(super) const JOURNAL_FILE: &'static str = "sumeragi-local-vote-locks.norito";
    /// Filename for the independently removable operator safety-halt marker.
    pub(super) const SAFETY_HALT_FILE: &'static str = "sumeragi-consensus-safety-halt.norito";

    /// Build the journal path beneath a Kura store root.
    pub(super) fn journal_path(root: &Path) -> PathBuf {
        if root.as_os_str().is_empty() {
            PathBuf::new()
        } else {
            root.join(Self::JOURNAL_FILE)
        }
    }

    /// Load and authenticate local vote locks, treating a missing file as an empty journal.
    pub(super) fn load(
        path: PathBuf,
        chain_id: &ChainId,
        validator_key: &PublicKey,
    ) -> Result<Self, LocalVoteLockError> {
        let safety_halt_path = safety_halt_path_for(&path);
        let mut journal = Self {
            path: path.clone(),
            chain_id: chain_id.clone(),
            validator_key: validator_key.clone(),
            entries: Vec::new(),
            safety_halt: load_safety_halt(&safety_halt_path, chain_id, validator_key)?,
        };
        if !path.as_os_str().is_empty() && (path.exists() || temp_path(&path).exists()) {
            let tmp = temp_path(&path);
            let (payload, loaded_from_temp) = if tmp.exists() {
                match load_payload(&tmp) {
                    Ok(payload) => (payload, true),
                    Err(_) if path.exists() => (load_payload(&path)?, false),
                    Err(err) => return Err(err),
                }
            } else {
                (load_payload(&path)?, false)
            };
            if payload.version != JOURNAL_VERSION {
                return Err(LocalVoteLockError::UnsupportedVersion(payload.version));
            }
            if payload.chain_id != *chain_id || payload.validator_key != *validator_key {
                return Err(LocalVoteLockError::IdentityMismatch);
            }
            for entry in payload.entries {
                journal.validate_entry(&entry)?;
                journal.insert_checked(entry)?;
            }
            if loaded_from_temp {
                promote_temp(&tmp, &path)?;
            }
        }
        Ok(journal)
    }

    /// Return the recovered signed votes in deterministic journal order.
    pub(super) fn votes(&self) -> impl Iterator<Item = &QcVote> {
        self.entries.iter().map(|entry| &entry.vote)
    }

    /// Return a recovered durable process safety halt, if present.
    pub(super) fn safety_halt(&self) -> Option<&super::status::ConsensusSafetyHaltSnapshot> {
        self.safety_halt.as_ref()
    }

    /// Return an identity-bound sink that can persist the same adjacent halt marker.
    pub(super) fn safety_halt_sink(&self) -> SafetyHaltSink {
        SafetyHaltSink::new(&self.path, &self.chain_id, &self.validator_key)
    }

    /// Persist a process safety halt before allowing the actor to return from detection.
    pub(super) fn record_safety_halt(
        &mut self,
        halt: &super::status::ConsensusSafetyHaltSnapshot,
    ) -> Result<(), LocalVoteLockError> {
        if self.safety_halt.is_some() {
            return Ok(());
        }
        let persisted = self.safety_halt_sink().record(halt)?;
        self.safety_halt = Some(persisted);
        Ok(())
    }

    /// Persist a newly signed local Prepare/Commit vote before it can be broadcast.
    pub(super) fn record(
        &mut self,
        vote: QcVote,
        mode_tag: &str,
        committed_height: u64,
    ) -> Result<(), LocalVoteLockError> {
        if vote.height <= committed_height {
            return Err(LocalVoteLockError::AlreadyCommitted {
                height: vote.height,
                committed_height,
            });
        }
        let entry = PersistedVoteLock {
            mode_tag: mode_tag.to_owned(),
            vote,
        };
        self.validate_entry(&entry)?;
        let changed = self.retain_uncommitted(committed_height);
        if self.entries.iter().any(|existing| existing == &entry) {
            if changed {
                self.persist()?;
            }
            return Ok(());
        }
        self.insert_checked(entry)?;
        self.persist()
    }

    /// Remove locks whose height is already durably committed.
    pub(super) fn prune_committed(
        &mut self,
        committed_height: u64,
    ) -> Result<(), LocalVoteLockError> {
        if self.retain_uncommitted(committed_height) {
            self.persist()?;
        }
        Ok(())
    }

    fn retain_uncommitted(&mut self, committed_height: u64) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|entry| entry.vote.height > committed_height);
        before != self.entries.len()
    }

    fn insert_checked(&mut self, entry: PersistedVoteLock) -> Result<(), LocalVoteLockError> {
        // Commit votes are finality-capable. Once this validator has signed one subject at a
        // chain/epoch/height, a timeout, restart, or later view must never authorize a different
        // block (or different execution roots for the same block). Otherwise a withheld first QC
        // can be combined with our later signature to form two conflicting QCs. Prepare votes do
        // not themselves create this height-wide lock, but they remain constrained by an existing
        // Commit lock so they cannot help build an unsafe unlock proof after restart.
        if let Some(locked) = self.entries.iter().find(|locked| {
            locked.vote.phase == Phase::Commit
                && locked.vote.height == entry.vote.height
                && locked.vote.epoch == entry.vote.epoch
                && locked.vote.view != entry.vote.view
                && (locked.vote.block_hash != entry.vote.block_hash
                    || locked.mode_tag != entry.mode_tag
                    || (entry.vote.phase == Phase::Commit
                        && (locked.vote.parent_state_root != entry.vote.parent_state_root
                            || locked.vote.post_state_root != entry.vote.post_state_root)))
        }) {
            return Err(LocalVoteLockError::ConflictingCommitLock {
                height: locked.vote.height,
                locked_view: locked.vote.view,
                attempted_view: entry.vote.view,
            });
        }
        if let Some(existing) = self.entries.iter().find(|existing| {
            (same_exact_slot(&existing.vote, &entry.vote) && *existing != &entry)
                || (same_round(&existing.vote, &entry.vote)
                    && (existing.vote.block_hash != entry.vote.block_hash
                        || existing.mode_tag != entry.mode_tag
                        || existing.vote.chain_order_hash != entry.vote.chain_order_hash
                        || existing.vote.rechain_seq != entry.vote.rechain_seq
                        || existing.vote.signer != entry.vote.signer))
        }) {
            return Err(LocalVoteLockError::ConflictingSlot {
                height: existing.vote.height,
                view: existing.vote.view,
            });
        }
        if self.entries.len() >= MAX_UNCOMMITTED_VOTES {
            return Err(LocalVoteLockError::CapacityExceeded(MAX_UNCOMMITTED_VOTES));
        }
        self.entries.push(entry);
        self.entries.sort_by(|left, right| {
            (
                left.vote.height,
                left.vote.epoch,
                left.vote.view,
                phase_rank(left.vote.phase),
            )
                .cmp(&(
                    right.vote.height,
                    right.vote.epoch,
                    right.vote.view,
                    phase_rank(right.vote.phase),
                ))
                .then_with(|| left.vote.block_hash.cmp(&right.vote.block_hash))
        });
        Ok(())
    }

    fn validate_entry(&self, entry: &PersistedVoteLock) -> Result<(), LocalVoteLockError> {
        let vote = &entry.vote;
        let invalid = |reason| LocalVoteLockError::InvalidVote {
            height: vote.height,
            view: vote.view,
            reason,
        };
        if !matches!(vote.phase, Phase::Prepare | Phase::Commit) {
            return Err(invalid("phase is not Prepare or Commit"));
        }
        if vote.highest_qc.is_some() {
            return Err(invalid("non-NEW_VIEW vote carries a highest-QC reference"));
        }
        if !matches!(entry.mode_tag.as_str(), PERMISSIONED_TAG | NPOS_TAG) {
            return Err(invalid("unknown consensus mode tag"));
        }
        if vote.bls_sig.is_empty() {
            return Err(invalid("signature is empty"));
        }
        let preimage = vote_preimage(&self.chain_id, &entry.mode_tag, vote);
        verify_signature(&self.validator_key, &preimage, &vote.bls_sig)
            .map_err(|()| invalid("signature does not authenticate the local validator"))
    }

    fn persist(&self) -> Result<(), LocalVoteLockError> {
        let payload = PersistedLocalVoteLocks {
            version: JOURNAL_VERSION,
            chain_id: self.chain_id.clone(),
            validator_key: self.validator_key.clone(),
            entries: self.entries.clone(),
        };
        let bytes = to_bytes(&payload).map_err(LocalVoteLockError::Encode)?;
        persist_bytes(&self.path, &bytes)
    }
}

fn same_exact_slot(left: &QcVote, right: &QcVote) -> bool {
    left.phase == right.phase
        && left.height == right.height
        && left.view == right.view
        && left.epoch == right.epoch
}

fn same_round(left: &QcVote, right: &QcVote) -> bool {
    left.height == right.height && left.view == right.view && left.epoch == right.epoch
}

const fn phase_rank(phase: Phase) -> u8 {
    match phase {
        Phase::Prepare => 0,
        Phase::Commit => 1,
        Phase::NewView => 2,
    }
}

fn verify_signature(public_key: &PublicKey, preimage: &[u8], bytes: &[u8]) -> Result<(), ()> {
    let signature = match public_key.try_algorithm().map_err(|_| ())? {
        Algorithm::Ed25519 => iroha_crypto::ed25519_parse_signature(bytes).map_err(|_| ())?,
        Algorithm::MlDsa => iroha_crypto::mldsa65_parse_signature(bytes).map_err(|_| ())?,
        _ => Signature::try_from_bytes(bytes).map_err(|_| ())?,
    };
    signature.verify(public_key, preimage).map_err(|_| ())
}

fn load_payload(path: &Path) -> Result<PersistedLocalVoteLocks, LocalVoteLockError> {
    let bytes = fs::read(path).map_err(|source| LocalVoteLockError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    decode_from_bytes(&bytes).map_err(|source| LocalVoteLockError::Decode {
        path: path.to_path_buf(),
        source,
    })
}

fn load_safety_halt(
    path: &Path,
    chain_id: &ChainId,
    validator_key: &PublicKey,
) -> Result<Option<super::status::ConsensusSafetyHaltSnapshot>, LocalVoteLockError> {
    if path.as_os_str().is_empty() || (!path.exists() && !temp_path(path).exists()) {
        return Ok(None);
    }
    let tmp = temp_path(path);
    let (payload, loaded_from_temp): (PersistedSafetyHalt, bool) = if tmp.exists() {
        match load_safety_halt_payload(&tmp) {
            Ok(payload) => (payload, true),
            Err(_) if path.exists() => (load_safety_halt_payload(path)?, false),
            Err(err) => return Err(err),
        }
    } else {
        (load_safety_halt_payload(path)?, false)
    };
    if payload.version != JOURNAL_VERSION {
        return Err(LocalVoteLockError::UnsupportedVersion(payload.version));
    }
    if payload.chain_id != *chain_id || payload.validator_key != *validator_key {
        return Err(LocalVoteLockError::IdentityMismatch);
    }
    if payload.reason.is_empty() {
        return Err(LocalVoteLockError::InvalidSafetyHalt("reason is empty"));
    }
    if loaded_from_temp {
        promote_temp(&tmp, path)?;
    }
    Ok(Some(super::status::ConsensusSafetyHaltSnapshot {
        active: true,
        reason: Some(payload.reason),
        height: payload.height,
        epoch: payload.epoch,
        first_block_hash: payload.first_block_hash,
        conflicting_block_hash: payload.conflicting_block_hash,
        first_parent_state_root: payload.first_parent_state_root,
        first_post_state_root: payload.first_post_state_root,
        conflicting_parent_state_root: payload.conflicting_parent_state_root,
        conflicting_post_state_root: payload.conflicting_post_state_root,
    }))
}

fn load_safety_halt_payload(path: &Path) -> Result<PersistedSafetyHalt, LocalVoteLockError> {
    let bytes = fs::read(path).map_err(|source| LocalVoteLockError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    decode_from_bytes(&bytes).map_err(|source| LocalVoteLockError::Decode {
        path: path.to_path_buf(),
        source,
    })
}

fn safety_halt_path_for(vote_path: &Path) -> PathBuf {
    if vote_path.as_os_str().is_empty() {
        PathBuf::new()
    } else {
        vote_path.with_file_name(LocalVoteLockJournal::SAFETY_HALT_FILE)
    }
}

fn persist_bytes(path: &Path, bytes: &[u8]) -> Result<(), LocalVoteLockError> {
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| LocalVoteLockError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let tmp = temp_path(path);
    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)
            .map_err(|source| LocalVoteLockError::Write {
                path: tmp.clone(),
                source,
            })?;
        file.write_all(bytes)
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_data())
            .map_err(|source| LocalVoteLockError::Write {
                path: tmp.clone(),
                source,
            })?;
    }
    promote_temp(&tmp, path)
}

fn temp_path(path: &Path) -> PathBuf {
    path.with_extension("norito.tmp")
}

fn promote_temp(from: &Path, to: &Path) -> Result<(), LocalVoteLockError> {
    fs::rename(from, to)
        .or_else(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                fs::remove_file(to)?;
                fs::rename(from, to)
            } else {
                Err(source)
            }
        })
        .map_err(|source| LocalVoteLockError::Write {
            path: to.to_path_buf(),
            source,
        })?;
    if let Some(parent) = to.parent() {
        let dir = fs::File::open(parent).map_err(|source| LocalVoteLockError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
        dir.sync_all().map_err(|source| LocalVoteLockError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{ChainId, block::BlockHeader, consensus::QcVote};
    use norito::to_bytes;
    use tempfile::tempdir;

    use super::{
        LocalVoteLockError, LocalVoteLockJournal, PERMISSIONED_TAG, PersistedLocalVoteLocks,
        PersistedVoteLock, Phase, persist_bytes,
    };

    fn signed_vote(
        chain: &ChainId,
        key_pair: &KeyPair,
        hash_byte: u8,
        height: u64,
        view: u64,
    ) -> QcVote {
        let mut vote = QcVote {
            phase: Phase::Commit,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [hash_byte; Hash::LENGTH],
            )),
            parent_state_root: Hash::prehashed([1; Hash::LENGTH]),
            post_state_root: Hash::prehashed([2; Hash::LENGTH]),
            height,
            view,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        sign_vote(chain, key_pair, &mut vote);
        vote
    }

    fn sign_vote(chain: &ChainId, key_pair: &KeyPair, vote: &mut QcVote) {
        vote.bls_sig.clear();
        let preimage = crate::sumeragi::consensus::vote_preimage(chain, PERMISSIONED_TAG, vote);
        vote.bls_sig = Signature::try_new(key_pair.private_key(), &preimage)
            .expect("sign vote")
            .payload()
            .to_vec();
    }

    #[test]
    fn journal_recovers_authenticated_votes_and_prunes_only_committed_heights() {
        let dir = tempdir().expect("tempdir");
        let path = LocalVoteLockJournal::journal_path(dir.path());
        let chain: ChainId = "vote-lock-journal-roundtrip".parse().expect("chain id");
        let key_pair =
            KeyPair::try_from_seed(vec![0x61; 32], Algorithm::BlsNormal).expect("derive BLS key");
        let mut journal = LocalVoteLockJournal::load(path.clone(), &chain, key_pair.public_key())
            .expect("load empty journal");
        journal
            .record(
                signed_vote(&chain, &key_pair, 0x11, 11, 1),
                PERMISSIONED_TAG,
                10,
            )
            .expect("record first vote");
        journal
            .record(
                signed_vote(&chain, &key_pair, 0x22, 12, 2),
                PERMISSIONED_TAG,
                10,
            )
            .expect("record second vote");

        let mut reloaded =
            LocalVoteLockJournal::load(path, &chain, key_pair.public_key()).expect("reload");
        assert_eq!(reloaded.votes().count(), 2);
        reloaded.prune_committed(11).expect("prune committed");
        assert_eq!(
            reloaded.votes().map(|vote| vote.height).collect::<Vec<_>>(),
            vec![12]
        );
    }

    #[test]
    fn journal_rejects_conflicting_commit_subject_across_views() {
        let dir = tempdir().expect("tempdir");
        let path = LocalVoteLockJournal::journal_path(dir.path());
        let chain: ChainId = "vote-lock-journal-conflict".parse().expect("chain id");
        let key_pair =
            KeyPair::try_from_seed(vec![0x62; 32], Algorithm::BlsNormal).expect("derive BLS key");
        let mut journal =
            LocalVoteLockJournal::load(path, &chain, key_pair.public_key()).expect("load");
        journal
            .record(
                signed_vote(&chain, &key_pair, 0x31, 20, 4),
                PERMISSIONED_TAG,
                19,
            )
            .expect("record original");
        let err = journal
            .record(
                signed_vote(&chain, &key_pair, 0x32, 20, 4),
                PERMISSIONED_TAG,
                19,
            )
            .expect_err("same exact slot must conflict");
        assert!(matches!(err, LocalVoteLockError::ConflictingSlot { .. }));

        let mut drifted_context = signed_vote(&chain, &key_pair, 0x33, 20, 4);
        drifted_context.signer = 7;
        drifted_context.chain_order_hash = Hash::prehashed([0xA5; Hash::LENGTH]);
        drifted_context.rechain_seq = 9;
        sign_vote(&chain, &key_pair, &mut drifted_context);
        let err = journal
            .record(drifted_context, PERMISSIONED_TAG, 19)
            .expect_err("mutable roster and chain-order context must not change slot identity");
        assert!(matches!(err, LocalVoteLockError::ConflictingSlot { .. }));

        let mut cross_phase = signed_vote(&chain, &key_pair, 0x34, 20, 4);
        cross_phase.phase = Phase::Prepare;
        sign_vote(&chain, &key_pair, &mut cross_phase);
        let err = journal
            .record(cross_phase, PERMISSIONED_TAG, 19)
            .expect_err("Prepare and Commit phases in one round cannot target different blocks");
        assert!(matches!(err, LocalVoteLockError::ConflictingSlot { .. }));

        let err = journal
            .record(
                signed_vote(&chain, &key_pair, 0x32, 20, 5),
                PERMISSIONED_TAG,
                19,
            )
            .expect_err("later views must not supersede a finality-capable Commit lock");
        assert!(matches!(
            err,
            LocalVoteLockError::ConflictingCommitLock {
                height: 20,
                locked_view: 4,
                attempted_view: 5
            }
        ));

        journal
            .record(
                signed_vote(&chain, &key_pair, 0x31, 20, 5),
                PERMISSIONED_TAG,
                19,
            )
            .expect("the identical Commit subject may be retransmitted in a later view");
        assert_eq!(journal.votes().count(), 2);
    }

    #[test]
    fn journal_restart_preserves_height_wide_commit_lock() {
        let dir = tempdir().expect("tempdir");
        let path = LocalVoteLockJournal::journal_path(dir.path());
        let chain: ChainId = "vote-lock-journal-restart-height-lock"
            .parse()
            .expect("chain id");
        let key_pair =
            KeyPair::try_from_seed(vec![0x6A; 32], Algorithm::BlsNormal).expect("derive BLS key");
        let mut journal =
            LocalVoteLockJournal::load(path.clone(), &chain, key_pair.public_key()).expect("load");
        journal
            .record(
                signed_vote(&chain, &key_pair, 0x41, 24, 2),
                PERMISSIONED_TAG,
                23,
            )
            .expect("record original Commit lock");
        drop(journal);

        let mut reloaded =
            LocalVoteLockJournal::load(path, &chain, key_pair.public_key()).expect("reload");
        let err = reloaded
            .record(
                signed_vote(&chain, &key_pair, 0x42, 24, 99),
                PERMISSIONED_TAG,
                23,
            )
            .expect_err("restart must not weaken a Commit lock into a view-local lock");
        assert!(matches!(
            err,
            LocalVoteLockError::ConflictingCommitLock {
                height: 24,
                locked_view: 2,
                attempted_view: 99
            }
        ));
        assert_eq!(reloaded.votes().count(), 1);
    }

    #[test]
    fn journal_rejects_legacy_view_local_vote_lock_format() {
        let dir = tempdir().expect("tempdir");
        let path = LocalVoteLockJournal::journal_path(dir.path());
        let chain: ChainId = "vote-lock-journal-legacy-view-lock"
            .parse()
            .expect("chain id");
        let key_pair =
            KeyPair::try_from_seed(vec![0x6B; 32], Algorithm::BlsNormal).expect("derive BLS key");
        let legacy = PersistedLocalVoteLocks {
            version: 1,
            chain_id: chain.clone(),
            validator_key: key_pair.public_key().clone(),
            entries: vec![PersistedVoteLock {
                mode_tag: PERMISSIONED_TAG.to_owned(),
                vote: signed_vote(&chain, &key_pair, 0x51, 25, 3),
            }],
        };
        let bytes = to_bytes(&legacy).expect("encode legacy journal");
        persist_bytes(&path, &bytes).expect("persist legacy journal");

        let err = LocalVoteLockJournal::load(path, &chain, key_pair.public_key())
            .expect_err("view-local v1 locks are unsafe and must not be migrated implicitly");
        assert!(matches!(err, LocalVoteLockError::UnsupportedVersion(1)));
    }

    #[test]
    fn journal_rejects_same_phase_same_block_with_divergent_execution_roots() {
        let dir = tempdir().expect("tempdir");
        let path = LocalVoteLockJournal::journal_path(dir.path());
        let chain: ChainId = "vote-lock-journal-root-conflict".parse().expect("chain id");
        let key_pair =
            KeyPair::try_from_seed(vec![0x65; 32], Algorithm::BlsNormal).expect("derive BLS key");
        let mut journal =
            LocalVoteLockJournal::load(path, &chain, key_pair.public_key()).expect("load");
        let original = signed_vote(&chain, &key_pair, 0x71, 22, 6);
        journal
            .record(original.clone(), PERMISSIONED_TAG, 21)
            .expect("record original");

        for (parent_state_root, post_state_root) in [
            (
                Hash::prehashed([0x81; Hash::LENGTH]),
                original.post_state_root,
            ),
            (
                original.parent_state_root,
                Hash::prehashed([0x82; Hash::LENGTH]),
            ),
        ] {
            let mut conflicting = original.clone();
            conflicting.parent_state_root = parent_state_root;
            conflicting.post_state_root = post_state_root;
            sign_vote(&chain, &key_pair, &mut conflicting);
            let err = journal
                .record(conflicting, PERMISSIONED_TAG, 21)
                .expect_err("same exact vote slot must bind both execution roots");
            assert!(matches!(err, LocalVoteLockError::ConflictingSlot { .. }));
        }

        let mut later_view_conflict = original.clone();
        later_view_conflict.view = original.view.saturating_add(1);
        later_view_conflict.post_state_root = Hash::prehashed([0x83; Hash::LENGTH]);
        sign_vote(&chain, &key_pair, &mut later_view_conflict);
        let err = journal
            .record(later_view_conflict, PERMISSIONED_TAG, 21)
            .expect_err("later views must not change the execution roots of a Commit lock");
        assert!(matches!(
            err,
            LocalVoteLockError::ConflictingCommitLock { height: 22, .. }
        ));
        assert_eq!(journal.votes().count(), 1);
    }

    #[test]
    fn journal_rejects_cross_phase_immutable_context_drift_for_same_round() {
        let dir = tempdir().expect("tempdir");
        let path = LocalVoteLockJournal::journal_path(dir.path());
        let chain: ChainId = "vote-lock-journal-cross-phase-context"
            .parse()
            .expect("chain id");
        let key_pair =
            KeyPair::try_from_seed(vec![0x66; 32], Algorithm::BlsNormal).expect("derive BLS key");
        let mut journal =
            LocalVoteLockJournal::load(path, &chain, key_pair.public_key()).expect("load");
        let mut prepare = signed_vote(&chain, &key_pair, 0x72, 23, 7);
        prepare.phase = Phase::Prepare;
        prepare.parent_state_root = Hash::prehashed([0; Hash::LENGTH]);
        prepare.post_state_root = Hash::prehashed([0; Hash::LENGTH]);
        sign_vote(&chain, &key_pair, &mut prepare);
        journal
            .record(prepare.clone(), PERMISSIONED_TAG, 22)
            .expect("record prepare");

        let mutations: [fn(&mut QcVote); 3] = [
            |vote: &mut QcVote| vote.chain_order_hash = Hash::prehashed([0x91; Hash::LENGTH]),
            |vote: &mut QcVote| vote.rechain_seq = 4,
            |vote: &mut QcVote| vote.signer = 2,
        ];
        for mutate in mutations {
            let mut commit = signed_vote(&chain, &key_pair, 0x72, 23, 7);
            mutate(&mut commit);
            sign_vote(&chain, &key_pair, &mut commit);
            let err = journal
                .record(commit, PERMISSIONED_TAG, 22)
                .expect_err("cross-phase immutable context drift must fail closed");
            assert!(matches!(err, LocalVoteLockError::ConflictingSlot { .. }));
        }
    }

    #[test]
    fn journal_rejects_committed_height_before_pruning_live_protection() {
        let dir = tempdir().expect("tempdir");
        let path = LocalVoteLockJournal::journal_path(dir.path());
        let chain: ChainId = "vote-lock-journal-committed".parse().expect("chain id");
        let key_pair =
            KeyPair::try_from_seed(vec![0x63; 32], Algorithm::BlsNormal).expect("derive BLS key");
        let mut journal =
            LocalVoteLockJournal::load(path, &chain, key_pair.public_key()).expect("load");
        journal
            .record(
                signed_vote(&chain, &key_pair, 0x41, 20, 4),
                PERMISSIONED_TAG,
                19,
            )
            .expect("record live protection");

        let err = journal
            .record(
                signed_vote(&chain, &key_pair, 0x42, 20, 5),
                PERMISSIONED_TAG,
                20,
            )
            .expect_err("committed-height vote must fail closed");
        assert!(matches!(err, LocalVoteLockError::AlreadyCommitted { .. }));
        assert_eq!(
            journal.votes().count(),
            1,
            "stale vote rejection must not prune the prior safety record"
        );
    }

    #[test]
    fn safety_halt_marker_survives_restart_separately_from_vote_locks() {
        let dir = tempdir().expect("tempdir");
        let path = LocalVoteLockJournal::journal_path(dir.path());
        let chain: ChainId = "vote-lock-journal-safety-halt".parse().expect("chain id");
        let key_pair =
            KeyPair::try_from_seed(vec![0x64; 32], Algorithm::BlsNormal).expect("derive BLS key");
        let mut journal = LocalVoteLockJournal::load(path.clone(), &chain, key_pair.public_key())
            .expect("load journal");
        let halt = crate::sumeragi::status::ConsensusSafetyHaltSnapshot {
            active: true,
            reason: Some("conflicting_commit_qc".to_owned()),
            height: 31,
            epoch: 2,
            first_block_hash: Some(HashOf::from_untyped_unchecked(Hash::prehashed(
                [0x51; Hash::LENGTH],
            ))),
            conflicting_block_hash: Some(HashOf::from_untyped_unchecked(Hash::prehashed(
                [0x52; Hash::LENGTH],
            ))),
            first_parent_state_root: Some(Hash::prehashed([0x53; Hash::LENGTH])),
            first_post_state_root: Some(Hash::prehashed([0x54; Hash::LENGTH])),
            conflicting_parent_state_root: Some(Hash::prehashed([0x55; Hash::LENGTH])),
            conflicting_post_state_root: Some(Hash::prehashed([0x56; Hash::LENGTH])),
        };
        journal
            .record_safety_halt(&halt)
            .expect("persist safety halt");

        let reloaded = LocalVoteLockJournal::load(path, &chain, key_pair.public_key())
            .expect("reload journal");
        assert_eq!(reloaded.safety_halt(), Some(&halt));
        assert_eq!(reloaded.votes().count(), 0);
        assert!(
            dir.path()
                .join(LocalVoteLockJournal::SAFETY_HALT_FILE)
                .exists(),
            "operator halt marker must use an independent file"
        );
    }

    #[test]
    fn journal_rejects_corrupt_vote_and_safety_halt_files() {
        let dir = tempdir().expect("tempdir");
        let path = LocalVoteLockJournal::journal_path(dir.path());
        let chain: ChainId = "vote-lock-journal-corruption".parse().expect("chain id");
        let key_pair =
            KeyPair::try_from_seed(vec![0x67; 32], Algorithm::BlsNormal).expect("derive BLS key");

        std::fs::write(&path, b"not a norito vote journal").expect("write corrupt vote journal");
        assert!(matches!(
            LocalVoteLockJournal::load(path.clone(), &chain, key_pair.public_key()),
            Err(LocalVoteLockError::Decode { .. })
        ));
        std::fs::remove_file(&path).expect("remove corrupt vote journal");

        let halt_path = dir.path().join(LocalVoteLockJournal::SAFETY_HALT_FILE);
        std::fs::write(&halt_path, b"not a norito halt marker").expect("write corrupt halt marker");
        assert!(matches!(
            LocalVoteLockJournal::load(path, &chain, key_pair.public_key()),
            Err(LocalVoteLockError::Decode { .. })
        ));
    }

    #[test]
    fn journal_prunes_after_each_durable_commit_without_restart_growth() {
        let dir = tempdir().expect("tempdir");
        let path = LocalVoteLockJournal::journal_path(dir.path());
        let chain: ChainId = "vote-lock-journal-runtime-prune".parse().expect("chain id");
        let key_pair =
            KeyPair::try_from_seed(vec![0x68; 32], Algorithm::BlsNormal).expect("derive BLS key");
        let mut journal =
            LocalVoteLockJournal::load(path, &chain, key_pair.public_key()).expect("load");

        for height in 1..=256 {
            journal
                .record(
                    signed_vote(&chain, &key_pair, height as u8, height, 0),
                    PERMISSIONED_TAG,
                    height.saturating_sub(1),
                )
                .expect("record live vote");
            journal
                .prune_committed(height)
                .expect("prune after durable commit");
            assert_eq!(journal.votes().count(), 0);
        }
        let reloaded = LocalVoteLockJournal::load(
            LocalVoteLockJournal::journal_path(dir.path()),
            &chain,
            key_pair.public_key(),
        )
        .expect("reload pruned journal");
        assert_eq!(reloaded.votes().count(), 0);
    }
}
