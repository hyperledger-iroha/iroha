//! Durable authenticated history for Core-owned Unix wallet lanes.
//!
//! The journal is host recovery material, never the latest hardware authority. Every committed
//! root selection retains and reauthenticates its original device certificate. Opening a journal
//! must be followed by state-machine restoration against a freshly obtained hardware anchor.
//! Prepare, commit, and abort are synced before their shared deterministic state plan is applied.
//! Committed nodes and terminal tombstones are retained without count, age, or byte eviction.

use super::*;
use crate::kagemusha_v1_state::KagemushaLaneIdV1;
#[cfg(test)]
use crate::kagemusha_v1_state::private_journal::{
    FRAME_HEADER_BYTES, JournalFileVersion, TestPersistenceFailure,
};
use crate::kagemusha_v1_state::private_journal::{
    PrivateJournal, PrivateJournalError, PrivateJournalFormat,
};
use iroha_data_model::kagemusha::KagemushaAuthenticatedReleaseV1;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

const JOURNAL_FILE: &str = "history.norito.wal";
const JOURNAL_FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: JOURNAL_FILE,
    magic: b"IKGHW1\0\0",
    hash_domain: b"iroha:kagemusha:v1:history-disk:frame\0",
    maximum_payload_bytes: 64 * 1024 * 1024,
};

/// Externally pinned historical device keys for one governed hardware profile.
///
/// This is deliberately not serializable and is never learned from a journal. The Core owner
/// obtains these bindings from authenticated device/release provisioning, including previous
/// epochs needed to verify retained history. No signing key or generated default is accepted.
#[derive(Clone)]
pub struct KagemushaHistoryDeviceCredentialsV1 {
    profile_id: DigestV1,
    epoch_keys: BTreeMap<u128, KagemushaDevicePublicKeyV1>,
}

impl KagemushaHistoryDeviceCredentialsV1 {
    /// Authenticate the complete retained epoch key inventory against the independently pinned
    /// release and exact wallet lane before any host history is opened. Historical credentials
    /// are checked at their original issuance; recovery does not renew their monetary validity.
    pub fn authenticate(
        release: &KagemushaAuthenticatedReleaseV1,
        lane: &KagemushaLaneIdV1,
        profile_id: DigestV1,
        credentials: impl IntoIterator<
            Item = iroha_data_model::kagemusha::KagemushaHardwareCredentialV1,
        >,
    ) -> Result<Self, KagemushaHistoryStoreErrorV1> {
        lane.validate()
            .map_err(|_| KagemushaHistoryStoreErrorV1::InvalidCertificate)?;
        let profile = release
            .enabled_profile(profile_id)
            .ok_or(KagemushaHistoryStoreErrorV1::InvalidCertificate)?;
        let mut keys = Vec::new();
        for credential in credentials {
            credential
                .validate_against_profile(&profile.hardware_profile)
                .map_err(|_| KagemushaHistoryStoreErrorV1::InvalidCertificate)?;
            if credential.network_id != lane.network_id
                || credential.lane_commitment != lane.device_lane_id
                || credential.suite_id != profile.suite_id
            {
                return Err(KagemushaHistoryStoreErrorV1::InvalidCertificate);
            }
            keys.push((
                u128::from(credential.hardware_epoch_generation),
                credential.device_public_key,
            ));
        }
        Self::new(profile_id, keys)
    }
    /// Construct one exact profile/epoch credential history, rejecting duplicate epochs.
    pub(crate) fn new(
        profile_id: DigestV1,
        keys: impl IntoIterator<Item = (u128, KagemushaDevicePublicKeyV1)>,
    ) -> Result<Self, KagemushaHistoryStoreErrorV1> {
        if digest_is_zero(profile_id) {
            return Err(KagemushaHistoryStoreErrorV1::InvalidCertificate);
        }
        let mut epoch_keys = BTreeMap::new();
        for (epoch, key) in keys {
            if epoch == 0 || key.validate().is_err() || epoch_keys.insert(epoch, key).is_some() {
                return Err(KagemushaHistoryStoreErrorV1::InvalidCertificate);
            }
        }
        if epoch_keys.is_empty() {
            return Err(KagemushaHistoryStoreErrorV1::InvalidCertificate);
        }
        Ok(Self {
            profile_id,
            epoch_keys,
        })
    }

    /// Require the current state's exact profile, epoch, and device-key reference.
    pub(crate) fn require_current_binding(
        &self,
        profile_id: DigestV1,
        epoch: u128,
        key_reference: DigestV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        let key = self
            .epoch_keys
            .get(&epoch)
            .ok_or(KagemushaHistoryStoreErrorV1::InvalidCertificate)?;
        if self.profile_id != profile_id
            || iroha_data_model::kagemusha::kagemusha_device_key_reference_v1(key) != key_reference
        {
            return Err(KagemushaHistoryStoreErrorV1::InvalidCertificate);
        }
        Ok(())
    }

    fn verify(
        &self,
        certificate: KagemushaHistoryRootSelectionCertificateV1,
    ) -> Result<VerifiedKagemushaHistoryRootSelectionV1, KagemushaHistoryStoreErrorV1> {
        let key = self
            .epoch_keys
            .get(&certificate.subject.hardware_epoch)
            .ok_or(KagemushaHistoryStoreErrorV1::InvalidCertificate)?;
        certificate.verify(self.profile_id, key)
    }
}

// Norito's canonical header declares the layout. The frame magic fixes this journal at V1;
// callers cannot select another schema/codec or decode a verified capability from disk.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::sparse_merkle::authenticated_history::disk_history_store::JournalRecordV1"
)]
enum JournalRecordV1 {
    Initialize {
        lane_binding: DigestV1,
        hardware_profile_id: DigestV1,
    },
    Prepare(KagemushaPreparedHistoryCasV1),
    Commit(KagemushaHistoryRootSelectionCertificateV1),
    Abort(DigestV1),
}

/// Descriptor-locked, append-only disk implementation of Core's authenticated-history contract.
/// The shared private journal owns durable bytes; this layer alone verifies history certificates.
pub struct KagemushaDiskAuthenticatedHistoryStoreV1 {
    state: KagemushaMemoryAuthenticatedHistoryStoreV1,
    wal: PrivateJournal,
    credentials: KagemushaHistoryDeviceCredentialsV1,
    last_commit_position: Option<(u128, u128)>,
    // An exact earlier prefix replayed from this held WAL, never a caller-selected root.
    // Only private incoming recovery can enter it; no usable Core escapes before its original
    // proof/Guard is checked and the actual committed store is selected again.
    pending_incoming: Option<PendingIncomingHistory>,
}

struct PendingIncomingHistory {
    predecessor: KagemushaMemoryAuthenticatedHistoryStoreV1,
    transaction: KagemushaPreparedHistoryCasV1,
    certificate: KagemushaHistoryRootSelectionCertificateV1,
    successor_roots: KagemushaHistoryRootsV1,
}

impl KagemushaDiskAuthenticatedHistoryStoreV1 {
    /// Create new private history; existing paths are never reset or reused.
    pub fn create_new(
        path: &Path,
        lane_binding: DigestV1,
        credentials: KagemushaHistoryDeviceCredentialsV1,
        overlay_capacity_bytes: u64,
    ) -> Result<Self, KagemushaHistoryStoreErrorV1> {
        if digest_is_zero(lane_binding) {
            return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
        }
        let wal = PrivateJournal::create_new(path, JOURNAL_FORMAT).map_err(journal_error)?;
        let mut store = Self {
            state: KagemushaMemoryAuthenticatedHistoryStoreV1::new(overlay_capacity_bytes),
            wal,
            credentials,
            last_commit_position: None,
            pending_incoming: None,
        };
        store.persist(&JournalRecordV1::Initialize {
            lane_binding,
            hardware_profile_id: store.credentials.profile_id,
        })?;
        Ok(store)
    }

    /// Recover host history without creating missing files or granting monetary authority.
    /// The owner must restore the full snapshot against the latest hardware anchor afterward.
    pub fn open_existing(
        path: &Path,
        lane_binding: DigestV1,
        credentials: KagemushaHistoryDeviceCredentialsV1,
        overlay_capacity_bytes: u64,
    ) -> Result<Self, KagemushaHistoryStoreErrorV1> {
        let wal = PrivateJournal::open_existing(path, JOURNAL_FORMAT).map_err(journal_error)?;
        let mut store = Self {
            state: KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX),
            wal,
            credentials,
            last_commit_position: None,
            pending_incoming: None,
        };
        while let Some((sequence, payload)) = store.wal.replay_next().map_err(journal_error)? {
            let record = norito::decode_canonical::<JournalRecordV1>(&payload)
                .map_err(|_| KagemushaHistoryStoreErrorV1::JournalCorrupt)?;
            if norito::encode_canonical(&record)
                .map_err(|_| KagemushaHistoryStoreErrorV1::JournalCorrupt)?
                != payload
            {
                return Err(KagemushaHistoryStoreErrorV1::JournalCorrupt);
            }
            store.replay_record(record, lane_binding, sequence)?;
        }
        validate_committed_history_v1(&store.state)?;
        store.state.overlay_capacity_bytes = overlay_capacity_bytes;
        store.check_owned()?;
        Ok(store)
    }

    fn check_owned(&self) -> Result<(), KagemushaHistoryStoreErrorV1> {
        self.wal.check_owned().map_err(journal_error)
    }

    /// Select the actual credential inventory already authenticated for this held lane.
    #[cfg(feature = "zk-halo2-ipa")]
    pub(crate) fn current_device_key(
        &self,
        profile: DigestV1,
        epoch: u128,
        reference: DigestV1,
    ) -> Result<KagemushaDevicePublicKeyV1, KagemushaHistoryStoreErrorV1> {
        self.check_owned()?;
        self.credentials
            .require_current_binding(profile, epoch, reference)?;
        self.credentials
            .epoch_keys
            .get(&epoch)
            .copied()
            .ok_or(KagemushaHistoryStoreErrorV1::InvalidCertificate)
    }

    // This is not a historical-root factory. Every prefix is replayed from the held original
    // journal, all device signatures are rechecked, and the entire actual tail must equal the
    // sole original incoming CAS successor. A later unrelated commit cannot be adopted.
    #[cfg(feature = "zk-halo2-ipa")]
    pub(crate) fn select_pending_incoming_predecessor(
        &mut self,
        lane_binding: DigestV1,
        transaction: &KagemushaPreparedHistoryCasV1,
        certificate: KagemushaHistoryRootSelectionCertificateV1,
        predecessor_roots: KagemushaHistoryRootsV1,
        predecessor_commitment: DigestV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        self.check_owned()?;
        if self.pending_incoming.is_some() {
            return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
        }
        transaction.validate()?;
        let verified = self.credentials.verify(certificate)?;
        if verified.transaction_id() != transaction.transaction_id()
            || verified.root_selection() != transaction.root_selection()
        {
            return Err(KagemushaHistoryStoreErrorV1::InvalidCertificate);
        }
        let successor_roots = transaction.successor_roots_from(predecessor_roots)?;
        if self.state.committed_roots() != successor_roots {
            return Err(KagemushaHistoryStoreErrorV1::CommittedRootsMismatch {
                expected: successor_roots,
                actual: self.state.committed_roots(),
            });
        }
        let mut replay = KagemushaMemoryAuthenticatedHistoryStoreV1::new(u64::MAX);
        let mut preceding = None;
        let mut position = None;
        self.wal
            .scan_complete(|sequence, bytes| {
                let record = norito::decode_canonical::<JournalRecordV1>(bytes)
                    .map_err(|_| PrivateJournalError::Corrupt)?;
                if norito::encode_canonical(&record).map_err(|_| PrivateJournalError::Corrupt)?
                    != bytes
                {
                    return Err(PrivateJournalError::Corrupt);
                }
                let result = (|| -> Result<(), KagemushaHistoryStoreErrorV1> {
                    if sequence == 0 {
                        return match record {
                            JournalRecordV1::Initialize {
                                lane_binding: bound,
                                hardware_profile_id,
                            } if bound == lane_binding
                                && !digest_is_zero(bound)
                                && hardware_profile_id == self.credentials.profile_id =>
                            {
                                Ok(())
                            }
                            _ => Err(KagemushaHistoryStoreErrorV1::JournalCorrupt),
                        };
                    }
                    match record {
                        JournalRecordV1::Initialize { .. } => {
                            Err(KagemushaHistoryStoreErrorV1::JournalCorrupt)
                        }
                        JournalRecordV1::Prepare(original) => {
                            let plan = replay.plan_prepare_cas(original)?;
                            if plan.outcome != KagemushaHistoryPrepareOutcomeV1::Prepared {
                                return Err(KagemushaHistoryStoreErrorV1::JournalCorrupt);
                            }
                            replay.apply_plan(plan);
                            Ok(())
                        }
                        JournalRecordV1::Abort(id) => {
                            let plan = replay.plan_abort_prepared(id)?;
                            if plan.outcome != KagemushaHistoryAbortOutcomeV1::Aborted {
                                return Err(KagemushaHistoryStoreErrorV1::JournalCorrupt);
                            }
                            replay.apply_plan(plan);
                            Ok(())
                        }
                        JournalRecordV1::Commit(original) => {
                            let selected = self.credentials.verify(original)?;
                            let next = (selected.hardware_epoch(), selected.monotonic_counter());
                            if position.is_some_and(|old| old >= next) {
                                return Err(KagemushaHistoryStoreErrorV1::InvalidCertificate);
                            }
                            if original == certificate {
                                if preceding.is_some()
                                    || replay.committed_roots() != predecessor_roots
                                {
                                    return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
                                }
                                replay.require_prepared(transaction)?;
                                replay.validate_recovery_checkpoint(predecessor_commitment)?;
                                preceding = Some(replay.clone());
                            }
                            let plan = replay.plan_commit_prepared(selected)?;
                            if !matches!(
                                plan.outcome,
                                KagemushaHistoryCommitOutcomeV1::Committed { .. }
                            ) {
                                return Err(KagemushaHistoryStoreErrorV1::JournalCorrupt);
                            }
                            replay.apply_plan(plan);
                            position = Some(next);
                            Ok(())
                        }
                    }
                })();
                result.map_err(|_| PrivateJournalError::Corrupt)
            })
            .map_err(journal_error)?;
        if replay.committed_roots() != successor_roots
            || replay.recovery_commitment()? != self.state.recovery_commitment()?
            || replay.terminal != self.state.terminal
            || replay.prepared != self.state.prepared
        {
            return Err(KagemushaHistoryStoreErrorV1::JournalCorrupt);
        }
        let mut predecessor = preceding.ok_or(KagemushaHistoryStoreErrorV1::InvalidTransaction)?;
        predecessor.overlay_capacity_bytes = self.state.overlay_capacity_bytes;
        self.check_owned()?;
        self.pending_incoming = Some(PendingIncomingHistory {
            predecessor,
            transaction: transaction.clone(),
            certificate,
            successor_roots,
        });
        Ok(())
    }

    fn selected_state(&self) -> &KagemushaMemoryAuthenticatedHistoryStoreV1 {
        self.pending_incoming
            .as_ref()
            .map_or(&self.state, |pending| &pending.predecessor)
    }

    #[cfg(feature = "zk-halo2-ipa")]
    pub(crate) fn require_actual_incoming_history(
        &self,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        self.check_owned()?;
        if self.pending_incoming.is_some() {
            return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
        }
        validate_committed_history_v1(&self.state).map(|_| ())
    }

    fn persist(&mut self, record: &JournalRecordV1) -> Result<(), KagemushaHistoryStoreErrorV1> {
        let payload = norito::encode_canonical(record)
            .map_err(|_| KagemushaHistoryStoreErrorV1::CanonicalEncoding)?;
        self.wal.append(&payload).map_err(journal_error)
    }

    fn replay_record(
        &mut self,
        record: JournalRecordV1,
        lane_binding: DigestV1,
        sequence: u64,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        if sequence == 0 {
            return match record {
                JournalRecordV1::Initialize {
                    lane_binding: bound,
                    hardware_profile_id,
                } if !digest_is_zero(bound)
                    && bound == lane_binding
                    && hardware_profile_id == self.credentials.profile_id =>
                {
                    Ok(())
                }
                _ => Err(KagemushaHistoryStoreErrorV1::JournalCorrupt),
            };
        }
        match record {
            JournalRecordV1::Initialize { .. } => Err(KagemushaHistoryStoreErrorV1::JournalCorrupt),
            JournalRecordV1::Prepare(transaction) => {
                let plan = self.state.plan_prepare_cas(transaction)?;
                if plan.outcome != KagemushaHistoryPrepareOutcomeV1::Prepared {
                    return Err(KagemushaHistoryStoreErrorV1::JournalCorrupt);
                }
                self.state.apply_plan(plan);
                Ok(())
            }
            JournalRecordV1::Commit(certificate) => {
                let verified = self.credentials.verify(certificate)?;
                self.require_new_commit_position(verified)?;
                let plan = self.state.plan_commit_prepared(verified)?;
                if !matches!(
                    plan.outcome,
                    KagemushaHistoryCommitOutcomeV1::Committed { .. }
                ) {
                    return Err(KagemushaHistoryStoreErrorV1::JournalCorrupt);
                }
                self.state.apply_plan(plan);
                self.last_commit_position =
                    Some((verified.hardware_epoch(), verified.monotonic_counter()));
                Ok(())
            }
            JournalRecordV1::Abort(transaction_id) => {
                let plan = self.state.plan_abort_prepared(transaction_id)?;
                if plan.outcome != KagemushaHistoryAbortOutcomeV1::Aborted {
                    return Err(KagemushaHistoryStoreErrorV1::JournalCorrupt);
                }
                self.state.apply_plan(plan);
                Ok(())
            }
        }
    }

    fn require_new_commit_position(
        &self,
        certificate: VerifiedKagemushaHistoryRootSelectionV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        let position = (
            certificate.hardware_epoch(),
            certificate.monotonic_counter(),
        );
        if position.1 == 0
            || self
                .last_commit_position
                .is_some_and(|previous| position <= previous)
        {
            return Err(KagemushaHistoryStoreErrorV1::CertificateMismatch);
        }
        Ok(())
    }
}

impl KagemushaAuthenticatedHistoryStoreV1 for KagemushaDiskAuthenticatedHistoryStoreV1 {
    fn committed_roots(&self) -> KagemushaHistoryRootsV1 {
        self.selected_state().committed_roots()
    }

    fn recovery_commitment(&self) -> Result<DigestV1, KagemushaHistoryStoreErrorV1> {
        self.check_owned()?;
        self.selected_state().recovery_commitment()
    }

    fn validate_recovery_checkpoint(
        &self,
        expected: DigestV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        self.check_owned()?;
        self.selected_state().validate_recovery_checkpoint(expected)
    }

    fn validate_tree(
        &self,
        tree: KagemushaHistoryTreeV1,
        root: DigestV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        self.check_owned()?;
        self.selected_state().validate_tree(tree, root)
    }

    fn require_prepared(
        &self,
        transaction: &KagemushaPreparedHistoryCasV1,
    ) -> Result<(), KagemushaHistoryStoreErrorV1> {
        self.check_owned()?;
        self.selected_state().require_prepared(transaction)
    }

    #[cfg(test)]
    fn overlay_usage(&self) -> KagemushaHistoryOverlayUsageV1 {
        self.state.overlay_usage()
    }

    fn read_node(
        &self,
        address: DigestV1,
    ) -> Result<Option<KagemushaHistoryNodeRecordV1>, KagemushaHistoryStoreErrorV1> {
        self.check_owned()?;
        self.selected_state().read_node(address)
    }

    #[cfg(test)]
    fn read_committed_root(
        &self,
        tree: KagemushaHistoryTreeV1,
    ) -> Result<KagemushaCommittedRootReadV1, KagemushaHistoryStoreErrorV1> {
        if self.check_owned().is_err() {
            return Ok(KagemushaCommittedRootReadV1::Unavailable {
                root: self.state.committed_roots().for_tree(tree),
            });
        }
        self.state.read_committed_root(tree)
    }

    fn prepare_cas(
        &mut self,
        transaction: KagemushaPreparedHistoryCasV1,
    ) -> Result<KagemushaHistoryPrepareOutcomeV1, KagemushaHistoryStoreErrorV1> {
        self.check_owned()?;
        if let Some(pending) = &self.pending_incoming {
            if transaction != pending.transaction {
                return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
            }
            pending.predecessor.require_prepared(&transaction)?;
            return Ok(KagemushaHistoryPrepareOutcomeV1::AlreadyPrepared);
        }
        let plan = self.state.plan_prepare_cas(transaction.clone())?;
        if plan.mutation.is_some() {
            self.persist(&JournalRecordV1::Prepare(transaction))?;
        }
        Ok(self.state.apply_plan(plan))
    }

    fn commit_prepared(
        &mut self,
        certificate: VerifiedKagemushaHistoryRootSelectionV1,
    ) -> Result<KagemushaHistoryCommitOutcomeV1, KagemushaHistoryStoreErrorV1> {
        self.check_owned()?;
        // A caller might have verified the typestate under another key. This concrete lane
        // independently applies its release-pinned credential history, including on retries.
        let certificate = self.credentials.verify(certificate.certificate)?;
        if let Some(pending) = &self.pending_incoming {
            if certificate.certificate != pending.certificate
                || self.state.committed_roots() != pending.successor_roots
            {
                return Err(KagemushaHistoryStoreErrorV1::InvalidCertificate);
            }
            let committed_roots = pending.successor_roots;
            self.check_owned()?;
            self.pending_incoming = None;
            return Ok(KagemushaHistoryCommitOutcomeV1::AlreadyCommitted { committed_roots });
        }
        let plan = self.state.plan_commit_prepared(certificate)?;
        if plan.mutation.is_some() {
            self.require_new_commit_position(certificate)?;
            self.persist(&JournalRecordV1::Commit(certificate.certificate))?;
            self.last_commit_position = Some((
                certificate.hardware_epoch(),
                certificate.monotonic_counter(),
            ));
        }
        Ok(self.state.apply_plan(plan))
    }

    fn abort_prepared(
        &mut self,
        transaction_id: DigestV1,
    ) -> Result<KagemushaHistoryAbortOutcomeV1, KagemushaHistoryStoreErrorV1> {
        self.check_owned()?;
        if self.pending_incoming.is_some() {
            return Err(KagemushaHistoryStoreErrorV1::InvalidTransaction);
        }
        let plan = self.state.plan_abort_prepared(transaction_id)?;
        if plan.mutation.is_some() {
            self.persist(&JournalRecordV1::Abort(transaction_id))?;
        }
        Ok(self.state.apply_plan(plan))
    }

    fn recover_prepared(
        &mut self,
        certificate: VerifiedKagemushaHistoryRootSelectionV1,
    ) -> Result<KagemushaHistoryRecoveryOutcomeV1, KagemushaHistoryStoreErrorV1> {
        self.commit_prepared(certificate)
            .map(|outcome| match outcome {
                KagemushaHistoryCommitOutcomeV1::Committed { committed_roots } => {
                    KagemushaHistoryRecoveryOutcomeV1::Committed { committed_roots }
                }
                KagemushaHistoryCommitOutcomeV1::AlreadyCommitted { committed_roots } => {
                    KagemushaHistoryRecoveryOutcomeV1::AlreadyCommitted { committed_roots }
                }
                KagemushaHistoryCommitOutcomeV1::Aborted => {
                    KagemushaHistoryRecoveryOutcomeV1::Aborted
                }
            })
    }
}

fn journal_error(error: PrivateJournalError) -> KagemushaHistoryStoreErrorV1 {
    match error {
        PrivateJournalError::StorageUnavailable => KagemushaHistoryStoreErrorV1::StorageUnavailable,
        PrivateJournalError::AlreadyOpen => KagemushaHistoryStoreErrorV1::StoreAlreadyOpen,
        PrivateJournalError::Corrupt => KagemushaHistoryStoreErrorV1::JournalCorrupt,
        PrivateJournalError::Uncertain => KagemushaHistoryStoreErrorV1::DurabilityUncertain,
    }
}

#[cfg(test)]
#[path = "disk_history_store_tests.rs"]
mod tests;

#[cfg(test)]
#[test]
fn captured_state_frame_owners() {
    crate::kagemusha_v1_state::state_frame_identity_tests::observed::<JournalRecordV1>(
        "iroha_core::zk::kagemusha_v1_state::sparse_merkle::authenticated_history::disk_history_store::JournalRecordV1",
    );
}
