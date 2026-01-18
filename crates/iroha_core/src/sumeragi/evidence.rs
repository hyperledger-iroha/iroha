//! Minimal evidence validation helpers plus WSV persistence wiring.
//! This module provides helpers to construct evidence for double-votes,
//! basic commit-certificate shape checks, an in-memory deduplication store for the Sumeragi
//! actor, and routines that persist new evidence records into the world state.

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryFrom,
    time::{SystemTime, UNIX_EPOCH},
};

use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus::{EvidenceRecord, Height, View},
    },
    prelude::ChainId,
};
use mv::storage::StorageReadOnly;

use super::consensus::{Evidence, EvidenceKind, EvidencePayload, Phase, Proposal, Qc, Vote};
use crate::state::{State, WorldReadOnly};

/// Minimum expected length for BLS signatures attached to consensus votes.
///
/// Consensus validators authenticate votes with BLS-normal signatures (96 bytes). Treating this
/// as a lower bound catches truncated payloads emitted by buggy or malicious peers.
const MIN_BLS_SIGNATURE_LEN: usize = 96;

/// Context required to cryptographically validate consensus evidence.
#[derive(Debug, Clone, Copy)]
pub struct EvidenceValidationContext<'a> {
    /// Commit topology used to resolve validator indices for signature checks.
    pub topology: &'a super::network_topology::Topology,
    /// Chain identifier bound into consensus preimages.
    pub chain_id: &'a ChainId,
    /// Consensus mode tag (permissioned or `NPoS`) for preimage separation.
    pub mode_tag: &'a str,
    /// Optional PRF seed for `NPoS` topology rotation.
    pub prf_seed: Option<[u8; 32]>,
}

/// Derive a deterministic deduplication key for an evidence entry.
#[must_use]
pub fn evidence_key(ev: &Evidence) -> Vec<u8> {
    let canonical = canonicalize_evidence(ev);
    evidence_key_inner(&canonical)
}

fn evidence_key_inner(ev: &Evidence) -> Vec<u8> {
    use norito::codec::Encode as _;
    let mut key = Vec::new();
    key.push(ev.kind as u8);
    key.extend_from_slice(&ev.encode());
    key
}

fn canonicalize_evidence(ev: &Evidence) -> Evidence {
    match &ev.payload {
        EvidencePayload::DoubleVote { v1, v2 } => {
            let (first, second) = canonical_vote_pair(v1, v2);
            Evidence {
                kind: ev.kind,
                payload: EvidencePayload::DoubleVote {
                    v1: first,
                    v2: second,
                },
            }
        }
        EvidencePayload::Censorship { tx_hash, receipts } => Evidence {
            kind: ev.kind,
            payload: EvidencePayload::Censorship {
                tx_hash: *tx_hash,
                receipts: canonicalize_censorship_receipts(receipts),
            },
        },
        _ => ev.clone(),
    }
}

fn canonicalize_censorship_receipts(
    receipts: &[iroha_data_model::transaction::TransactionSubmissionReceipt],
) -> Vec<iroha_data_model::transaction::TransactionSubmissionReceipt> {
    use norito::codec::Encode as _;
    let mut keyed: Vec<_> = receipts
        .iter()
        .cloned()
        .map(|receipt| (receipt.encode(), receipt))
        .collect();
    keyed.sort_by(|(left, _), (right, _)| left.cmp(right));
    keyed.into_iter().map(|(_, receipt)| receipt).collect()
}

fn double_vote_kind_for_phases(first: Phase, second: Phase) -> Option<EvidenceKind> {
    match (first, second) {
        (Phase::Prepare, Phase::Prepare) => Some(EvidenceKind::DoublePrepare),
        (Phase::Commit, Phase::Commit | Phase::Prepare) | (Phase::Prepare, Phase::Commit) => {
            // Cross-phase equivocation in the same round is treated as a commit double-vote since
            // the validator advanced while committing to conflicting blocks.
            Some(EvidenceKind::DoubleCommit)
        }
        _ => None,
    }
}

fn canonical_vote_pair(v1: &Vote, v2: &Vote) -> (Vote, Vote) {
    let left = (
        v1.phase as u8,
        v1.block_hash.as_ref(),
        v1.parent_state_root,
        v1.post_state_root,
    );
    let right = (
        v2.phase as u8,
        v2.block_hash.as_ref(),
        v2.parent_state_root,
        v2.post_state_root,
    );
    if left <= right {
        (v1.clone(), v2.clone())
    } else {
        (v2.clone(), v1.clone())
    }
}

/// Check for a double-vote: same (phase,height,view,epoch,signer) on different block hashes.
pub fn check_double_vote(v1: &Vote, v2: &Vote) -> Option<Evidence> {
    if v1.height == v2.height
        && v1.view == v2.view
        && v1.epoch == v2.epoch
        && v1.signer == v2.signer
    {
        let conflicts = if v1.block_hash != v2.block_hash {
            true
        } else if v1.phase == Phase::Commit && v2.phase == Phase::Commit {
            v1.parent_state_root != v2.parent_state_root || v1.post_state_root != v2.post_state_root
        } else {
            false
        };
        if conflicts {
            let (first, second) = canonical_vote_pair(v1, v2);
            return double_vote_kind_for_phases(first.phase, second.phase).map(|kind| Evidence {
                kind,
                payload: EvidencePayload::DoubleVote {
                    v1: first,
                    v2: second,
                },
            });
        }
        None
    } else {
        None
    }
}

/// Very basic commit-certificate invalidity check (shape only; cryptographic validity is not assessed here).
#[allow(dead_code)] // used by future SBV‑AM integration; unit tests cover only vote helpers
pub fn check_invalid_commit_qc_shape(qc: &Qc) -> Option<Evidence> {
    if qc.aggregate.signers_bitmap.is_empty() || qc.view == 0 && qc.height == 0 {
        Some(Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::InvalidQc {
                certificate: qc.clone(),
                reason: "empty signer bitmap or zero (view,height)".to_string(),
            },
        })
    } else {
        None
    }
}

/// Simple in-memory evidence store to deduplicate by a deterministic key.
#[derive(Default)]
pub struct EvidenceStore {
    // Deterministic key set of evidence entries (for quick membership and count)
    seen: BTreeSet<Vec<u8>>, // keys are hashed payloads
    // Optional payload map for audit/listing
    entries: BTreeMap<Vec<u8>, Evidence>,
}

impl EvidenceStore {
    pub fn new() -> Self {
        Self {
            seen: BTreeSet::new(),
            entries: BTreeMap::new(),
        }
    }

    /// Insert evidence if unseen. Returns true if newly inserted.
    pub fn insert(&mut self, ev: &Evidence, context: &EvidenceValidationContext<'_>) -> bool {
        let canonical = canonicalize_evidence(ev);
        if validate_evidence(&canonical, context).is_err() {
            return false;
        }
        let key = evidence_key_inner(&canonical);
        if self.seen.insert(key.clone()) {
            self.entries.insert(key, canonical);
            true
        } else {
            false
        }
    }
}

/// Persist an [`EvidenceRecord`] into the world state if unseen.
///
/// Returns `true` when the supplied evidence was newly inserted, `false`
/// when an identical entry already exists in storage.
pub fn persist_record(
    state: &State,
    evidence: &Evidence,
    context: &EvidenceValidationContext<'_>,
) -> bool {
    let canonical = canonicalize_evidence(evidence);
    if validate_evidence(&canonical, context).is_err() {
        return false;
    }
    let (fallback_height, horizon) = {
        let sv = state.view();
        let current_height = u64::try_from(sv.height()).unwrap_or(0);
        let horizon = sv
            .world()
            .sumeragi_npos_parameters()
            .map(|params| params.evidence_horizon_blocks());
        (current_height, horizon)
    };
    let key = evidence_key_inner(&canonical);
    let view = state.world.consensus_evidence.view();
    if view.get(&key).is_some() {
        return false;
    }
    drop(view);

    let (subject_height, subject_view) = evidence_subject_height_view(&canonical);
    if !evidence_within_configured_horizon(fallback_height, horizon, subject_height) {
        return false;
    }
    let recorded_at_height = subject_height.unwrap_or(fallback_height);
    let recorded_at_view = subject_view.unwrap_or_default();
    let recorded_at_ms = current_unix_ms();

    let record = EvidenceRecord {
        evidence: canonical,
        recorded_at_height,
        recorded_at_view,
        recorded_at_ms,
        penalty_applied: false,
        penalty_applied_at_height: None,
    };

    let mut block = state.world.consensus_evidence.block();
    block.insert(key, record);
    block.commit();
    true
}

/// Detect and persist a double-vote if present, deduplicating via the in-memory store.
///
/// Returns `true` when evidence was newly recorded (store + WSV), `false` otherwise.
pub fn record_double_vote(
    store: &mut EvidenceStore,
    state: &State,
    previous: &Vote,
    current: &Vote,
    context: &EvidenceValidationContext<'_>,
) -> bool {
    let Some(evidence) = check_double_vote(previous, current) else {
        return false;
    };
    if !store.insert(&evidence, context) {
        return false;
    }

    persist_record(state, &evidence, context)
}

/// Extract the height/view referenced by consensus evidence, when present.
pub fn evidence_subject_height_view(evidence: &Evidence) -> (Option<Height>, Option<View>) {
    match &evidence.payload {
        EvidencePayload::DoubleVote { v1, .. } => (Some(v1.height), Some(v1.view)),
        EvidencePayload::InvalidQc { certificate, .. } => {
            (Some(certificate.height), Some(certificate.view))
        }
        EvidencePayload::InvalidProposal { proposal, .. } => {
            (Some(proposal.header.height), Some(proposal.header.view))
        }
        EvidencePayload::Censorship { receipts, .. } => {
            let height = receipts
                .iter()
                .map(|receipt| receipt.payload.submitted_at_height)
                .max();
            (height, None)
        }
    }
}

pub(super) fn evidence_block_refs(evidence: &Evidence) -> Vec<(u64, HashOf<BlockHeader>)> {
    let mut refs = Vec::new();
    match &evidence.payload {
        EvidencePayload::DoubleVote { v1, v2 } => {
            refs.push((v1.height, v1.block_hash));
            if v2.block_hash != v1.block_hash {
                refs.push((v2.height, v2.block_hash));
            }
        }
        EvidencePayload::InvalidQc { certificate, .. } => {
            refs.push((certificate.height, certificate.subject_block_hash));
        }
        EvidencePayload::InvalidProposal { .. } | EvidencePayload::Censorship { .. } => {}
    }
    refs
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(0)
}

fn evidence_within_configured_horizon(
    current_height: u64,
    horizon: Option<u64>,
    subject_height: Option<u64>,
) -> bool {
    let Some(horizon) = horizon else { return true };
    if horizon == 0 {
        return true;
    }
    let reference = subject_height.unwrap_or(current_height);
    let lower_bound = current_height.saturating_sub(horizon);
    reference >= lower_bound
}

/// Errors returned by [`validate_evidence`] when the supplied [`Evidence`] fails basic
/// structural consistency checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceValidationError {
    /// [`EvidenceKind`] does not match the payload variant.
    KindPayloadMismatch,
    /// Double-vote evidence carries votes for mismatched phases.
    PhaseMismatch,
    /// Double-vote evidence carries votes for different block heights.
    HeightMismatch,
    /// Double-vote evidence carries votes for different views.
    ViewMismatch,
    /// Double-vote evidence carries votes for different epochs.
    EpochMismatch,
    /// Double-vote evidence carries votes signed by different validators.
    SignerMismatch,
    /// Double-vote evidence references the same block hash for both votes.
    BlockHashMatch,
    /// Double-vote evidence phase disagrees with its [`EvidenceKind`].
    PhaseKindMismatch,
    /// Evidence references votes that lack the expected BLS signature payload.
    SignatureMissing,
    /// Evidence references votes whose signatures appear truncated or forged.
    SignatureTruncated,
    /// Evidence references votes whose signatures fail cryptographic verification.
    SignatureInvalid,
    /// Proposal evidence references a highest certificate from an unexpected height.
    InvalidProposalHeight,
    /// Proposal evidence view invariant (reserved; view resets per height are expected).
    InvalidProposalView,
    /// Proposal evidence references a certificate whose subject does not match the proposal parent hash.
    InvalidProposalParentMismatch,
    /// Censorship evidence carries no receipts.
    ReceiptMissing,
    /// Censorship evidence receipts refer to different transaction hashes.
    ReceiptTxHashMismatch,
    /// Censorship evidence receipts are signed by non-validators.
    ReceiptSignerOutOfTopology,
    /// Censorship evidence receipt signature verification failed.
    ReceiptSignatureInvalid,
    /// Censorship evidence does not meet the f + 1 receipt threshold.
    ReceiptQuorumMissing,
}

impl std::fmt::Display for EvidenceValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use EvidenceValidationError::*;
        let msg = match self {
            KindPayloadMismatch => "evidence kind does not match payload variant",
            PhaseMismatch => "double-vote evidence phases must match",
            HeightMismatch => "double-vote evidence heights must match",
            ViewMismatch => "double-vote evidence views must match",
            EpochMismatch => "double-vote evidence epochs must match",
            SignerMismatch => "double-vote evidence signers must match",
            BlockHashMatch => "double-vote evidence must reference distinct block hashes",
            PhaseKindMismatch => "double-vote evidence phase disagrees with its kind",
            SignatureMissing => "consensus vote BLS signature payload missing",
            SignatureTruncated => "consensus vote BLS signature payload truncated or forged",
            SignatureInvalid => "consensus vote BLS signature verification failed",
            InvalidProposalHeight => {
                "invalid proposal evidence must advance height beyond the referenced certificate"
            }
            InvalidProposalView => "invalid proposal evidence view invariant violated",
            InvalidProposalParentMismatch => {
                "invalid proposal evidence certificate subject must match header parent hash"
            }
            ReceiptMissing => "censorship evidence must include receipts",
            ReceiptTxHashMismatch => "censorship evidence receipts must match the tx hash",
            ReceiptSignerOutOfTopology => "censorship evidence signer not in topology",
            ReceiptSignatureInvalid => "censorship evidence receipt signature invalid",
            ReceiptQuorumMissing => "censorship evidence below f + 1 receipt threshold",
        };
        write!(f, "{msg}")
    }
}

impl std::error::Error for EvidenceValidationError {}

/// Ensure that [`Evidence`] metadata and attached signatures remain consistent.
///
/// This routine enforces invariants that malicious peers could violate by crafting
/// forged payloads (e.g., mismatching the [`EvidenceKind`] with its payload variant,
/// mixing votes from different heights/views/epochs, or attaching invalid signatures).
/// Downstream consumers expect those invariants to hold when persisting slashing material.
///
/// # Errors
///
/// Returns [`EvidenceValidationError`] when the provided evidence violates one of the
/// invariants (kind/payload mismatch, inconsistent heights/views, or invalid signatures).
pub fn validate_evidence(
    evidence: &Evidence,
    context: &EvidenceValidationContext<'_>,
) -> Result<(), EvidenceValidationError> {
    match (&evidence.kind, &evidence.payload) {
        (
            EvidenceKind::DoublePrepare | EvidenceKind::DoubleCommit,
            EvidencePayload::DoubleVote { v1, v2 },
        ) => validate_double_vote(evidence.kind, v1, v2, context),
        (EvidenceKind::InvalidQc, EvidencePayload::InvalidQc { .. }) => Ok(()),
        (EvidenceKind::InvalidProposal, EvidencePayload::InvalidProposal { proposal, .. }) => {
            validate_invalid_proposal(proposal)
        }
        (EvidenceKind::Censorship, EvidencePayload::Censorship { tx_hash, receipts }) => {
            validate_censorship(tx_hash, receipts, context)
        }
        _ => Err(EvidenceValidationError::KindPayloadMismatch),
    }
}

fn validate_vote_signatures(
    v1: &Vote,
    v2: &Vote,
    context: &EvidenceValidationContext<'_>,
) -> Result<(), EvidenceValidationError> {
    let signature_topology = super::main_loop::topology_for_view(
        context.topology,
        v1.height,
        v1.view,
        context.mode_tag,
        context.prf_seed,
    );
    super::main_loop::vote_signature_check(
        v1,
        &signature_topology,
        context.chain_id,
        context.mode_tag,
    )
    .map_err(|_| EvidenceValidationError::SignatureInvalid)?;
    super::main_loop::vote_signature_check(
        v2,
        &signature_topology,
        context.chain_id,
        context.mode_tag,
    )
    .map_err(|_| EvidenceValidationError::SignatureInvalid)?;
    Ok(())
}

fn validate_double_vote(
    kind: EvidenceKind,
    v1: &Vote,
    v2: &Vote,
    context: &EvidenceValidationContext<'_>,
) -> Result<(), EvidenceValidationError> {
    if v1.bls_sig.is_empty() || v2.bls_sig.is_empty() {
        return Err(EvidenceValidationError::SignatureMissing);
    }
    if v1.bls_sig.len() < MIN_BLS_SIGNATURE_LEN || v2.bls_sig.len() < MIN_BLS_SIGNATURE_LEN {
        return Err(EvidenceValidationError::SignatureTruncated);
    }
    let Some(expected_kind) = double_vote_kind_for_phases(v1.phase, v2.phase) else {
        return Err(EvidenceValidationError::PhaseMismatch);
    };
    if v1.height != v2.height {
        return Err(EvidenceValidationError::HeightMismatch);
    }
    if v1.view != v2.view {
        return Err(EvidenceValidationError::ViewMismatch);
    }
    if v1.epoch != v2.epoch {
        return Err(EvidenceValidationError::EpochMismatch);
    }
    if v1.signer != v2.signer {
        return Err(EvidenceValidationError::SignerMismatch);
    }
    let block_hash_conflict = v1.block_hash != v2.block_hash;
    let root_conflict = v1.phase == Phase::Commit
        && v2.phase == Phase::Commit
        && (v1.parent_state_root != v2.parent_state_root
            || v1.post_state_root != v2.post_state_root);
    if !block_hash_conflict && !root_conflict {
        return Err(EvidenceValidationError::BlockHashMatch);
    }

    match (kind, expected_kind) {
        (EvidenceKind::DoublePrepare, EvidenceKind::DoublePrepare)
        | (EvidenceKind::DoubleCommit, EvidenceKind::DoubleCommit) => {
            validate_vote_signatures(v1, v2, context)?;
            Ok(())
        }
        (EvidenceKind::DoublePrepare, EvidenceKind::DoubleCommit)
        | (EvidenceKind::DoubleCommit, EvidenceKind::DoublePrepare) => {
            Err(EvidenceValidationError::PhaseKindMismatch)
        }
        _ => Err(EvidenceValidationError::KindPayloadMismatch),
    }
}

fn validate_censorship(
    tx_hash: &iroha_crypto::HashOf<iroha_data_model::transaction::SignedTransaction>,
    receipts: &[iroha_data_model::transaction::TransactionSubmissionReceipt],
    context: &EvidenceValidationContext<'_>,
) -> Result<(), EvidenceValidationError> {
    if receipts.is_empty() {
        return Err(EvidenceValidationError::ReceiptMissing);
    }
    let required = context.topology.min_votes_for_view_change();
    let mut unique = BTreeSet::new();
    for receipt in receipts {
        if &receipt.payload.tx_hash != tx_hash {
            return Err(EvidenceValidationError::ReceiptTxHashMismatch);
        }
        if context.topology.position(&receipt.payload.signer).is_none() {
            return Err(EvidenceValidationError::ReceiptSignerOutOfTopology);
        }
        receipt
            .verify()
            .map_err(|_| EvidenceValidationError::ReceiptSignatureInvalid)?;
        unique.insert(receipt.payload.signer.clone());
    }
    if unique.len() < required {
        return Err(EvidenceValidationError::ReceiptQuorumMissing);
    }
    Ok(())
}

fn validate_invalid_proposal(proposal: &Proposal) -> Result<(), EvidenceValidationError> {
    let qc = &proposal.header.highest_qc;
    if proposal.header.height <= qc.height {
        return Err(EvidenceValidationError::InvalidProposalHeight);
    }
    // View numbers reset per height, so only the height ordering is meaningful here.
    if proposal.header.parent_hash != qc.subject_block_hash {
        return Err(EvidenceValidationError::InvalidProposalParentMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{
        block::BlockHeader,
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        parameter::{Parameter, Parameters, system::SumeragiNposParameters},
        peer::PeerId,
        prelude::ChainId,
        transaction::{
            SignedTransaction, TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload,
        },
    };
    use mv::cell::Cell;
    use norito::codec::{Decode, Encode as _};
    use rand::{Rng, SeedableRng, rngs::StdRng, seq::SliceRandom};

    use super::{
        super::consensus::{
            ConsensusBlockHeader, Phase, Proposal, Qc, QcAggregate, QcHeaderRef, Vote,
        },
        *,
    };
    use crate::state::{State, World};

    type EvidenceCase = (EvidenceKind, EvidencePayload, EvidenceValidationError);
    type EvidenceRoundtripCase = (
        &'static str,
        EvidenceValidationError,
        fn(&EvidenceTestContext) -> Evidence,
    );

    struct EvidenceTestContext {
        chain_id: ChainId,
        mode_tag: &'static str,
        prf_seed: [u8; 32],
        keypairs: Vec<KeyPair>,
        topology: super::super::network_topology::Topology,
    }

    impl EvidenceTestContext {
        fn new(peer_count: usize) -> Self {
            let keypairs: Vec<_> = (0..peer_count)
                .map(|_| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                .collect();
            let peers = keypairs
                .iter()
                .map(|kp| PeerId::new(kp.public_key().clone()));
            let topology = super::super::network_topology::Topology::new(peers);
            Self {
                chain_id: ChainId::from("test"),
                mode_tag: super::super::consensus::PERMISSIONED_TAG,
                prf_seed: [0x11; 32],
                keypairs,
                topology,
            }
        }

        fn validation_context(&self) -> EvidenceValidationContext<'_> {
            EvidenceValidationContext {
                topology: &self.topology,
                chain_id: &self.chain_id,
                mode_tag: self.mode_tag,
                prf_seed: Some(self.prf_seed),
            }
        }

        fn signer_keypair_for_view(&self, signer: u32, height: u64, view: u64) -> &KeyPair {
            let idx = usize::try_from(signer).expect("signer index fits usize");
            let rotated = super::super::main_loop::topology_for_view(
                &self.topology,
                height,
                view,
                self.mode_tag,
                Some(self.prf_seed),
            );
            let peer = rotated
                .as_ref()
                .get(idx)
                .expect("signer index must be in range for view-aligned topology");
            self.keypairs
                .iter()
                .find(|kp| kp.public_key() == peer.public_key())
                .expect("signer keypair must exist for view-aligned topology")
        }

        fn sign_vote(&self, vote: &mut Vote) {
            let keypair = self.signer_keypair_for_view(vote.signer, vote.height, vote.view);
            let preimage =
                super::super::consensus::vote_preimage(&self.chain_id, self.mode_tag, vote);
            let signature = Signature::new(keypair.private_key(), &preimage);
            vote.bls_sig = signature.payload().to_vec();
        }
    }

    fn test_context() -> EvidenceTestContext {
        EvidenceTestContext::new(12)
    }

    fn zero_state_root() -> Hash {
        Hash::prehashed([0u8; 32])
    }

    fn sample_validator_set() -> Vec<PeerId> {
        let keypair = KeyPair::from_seed(b"evidence-validator".to_vec(), Algorithm::BlsNormal);
        vec![PeerId::new(keypair.public_key().clone())]
    }

    fn test_state() -> State {
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        State::new_for_testing(World::default(), kura, query)
    }

    fn state_with_horizon(current_height: u64, horizon: u64) -> State {
        let mut params = Parameters::default();
        let npos = SumeragiNposParameters {
            evidence_horizon_blocks: horizon,
            ..SumeragiNposParameters::default()
        };
        params.set_parameter(Parameter::Custom(npos.into_custom_parameter()));
        let mut world = World::default();
        world.parameters = Cell::new(params);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        if current_height > 0 {
            let mut hashes = state.block_hashes.block();
            let len = usize::try_from(current_height)
                .expect("current height must fit into usize for resize");
            hashes.resize(
                len,
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xFF; 32])),
            );
            hashes.commit();
        }
        state
    }

    fn sample_double_vote_pair(ctx: &EvidenceTestContext) -> (Vote, Vote) {
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x80; 32]));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Prepare,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 11,
            view: 5,
            epoch: 3,
            highest_qc: None,
            signer: 2,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x81; 32]));
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        (v1, v2)
    }

    fn sample_tx_hash(tag: u8) -> HashOf<SignedTransaction> {
        HashOf::from_untyped_unchecked(Hash::prehashed([tag; Hash::LENGTH]))
    }

    fn submission_receipt_for(
        ctx: &EvidenceTestContext,
        signer_idx: usize,
        tx_hash: HashOf<SignedTransaction>,
        submitted_at_height: u64,
    ) -> TransactionSubmissionReceipt {
        let keypair = ctx
            .keypairs
            .get(signer_idx)
            .expect("signer index must be in range for test context");
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash,
            submitted_at_ms: 1,
            submitted_at_height,
            signer: keypair.public_key().clone(),
        };
        TransactionSubmissionReceipt::sign(payload, keypair)
    }

    fn submission_receipt_with_invalid_signature(
        ctx: &EvidenceTestContext,
        signer_idx: usize,
        tx_hash: HashOf<SignedTransaction>,
        submitted_at_height: u64,
    ) -> TransactionSubmissionReceipt {
        let signer_key = ctx
            .keypairs
            .get(signer_idx)
            .expect("signer index must be in range for test context");
        let other_idx = if signer_idx + 1 < ctx.keypairs.len() {
            signer_idx + 1
        } else {
            0
        };
        let signing_key = ctx
            .keypairs
            .get(other_idx)
            .expect("backup signer key exists");
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash,
            submitted_at_ms: 1,
            submitted_at_height,
            signer: signer_key.public_key().clone(),
        };
        TransactionSubmissionReceipt::sign(payload, signing_key)
    }

    fn double_vote_with(
        ctx: &EvidenceTestContext,
        mutate: impl FnOnce(&mut Vote, &mut Vote),
    ) -> Evidence {
        let (mut v1, mut v2) = sample_double_vote_pair(ctx);
        mutate(&mut v1, &mut v2);
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        }
    }

    fn double_vote_with_unchecked(
        ctx: &EvidenceTestContext,
        mutate: impl FnOnce(&mut Vote, &mut Vote),
    ) -> Evidence {
        let (mut v1, mut v2) = sample_double_vote_pair(ctx);
        mutate(&mut v1, &mut v2);
        Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        }
    }

    fn roundtrip_case_duplicate_signer(ctx: &EvidenceTestContext) -> Evidence {
        double_vote_with(ctx, |_, v2| v2.signer = v2.signer.saturating_add(1))
    }

    fn roundtrip_case_conflicting_height(ctx: &EvidenceTestContext) -> Evidence {
        double_vote_with(ctx, |_, v2| v2.height = v2.height.saturating_add(1))
    }

    fn roundtrip_case_conflicting_view(ctx: &EvidenceTestContext) -> Evidence {
        double_vote_with(ctx, |_, v2| v2.view = v2.view.saturating_add(1))
    }

    fn roundtrip_case_signature_truncated(ctx: &EvidenceTestContext) -> Evidence {
        double_vote_with_unchecked(ctx, |v1, v2| {
            v1.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
            v2.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
        })
    }

    fn roundtrip_case_mixed_manifest_payload(ctx: &EvidenceTestContext) -> Evidence {
        let (v1, v2) = sample_double_vote_pair(ctx);
        Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        }
    }

    #[allow(clippy::too_many_lines)]
    fn mismatched_payload_cases(ctx: &EvidenceTestContext) -> Vec<EvidenceCase> {
        let (v1, v2) = sample_double_vote_pair(ctx);
        let double_vote_payload = EvidencePayload::DoubleVote { v1, v2 };

        let validator_set = ctx.topology.as_ref().to_vec();
        let zero_root = zero_state_root();
        let qc = Qc {
            phase: Phase::Prepare,
            subject_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [0xC0; 32],
            )),
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 21,
            view: 5,
            epoch: 2,
            mode_tag: ctx.mode_tag.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![0x01],
                bls_aggregate_signature: vec![0xC1; 96],
            },
        };
        let invalid_qc_payload = EvidencePayload::InvalidQc {
            certificate: qc,
            reason: "forged QC payload variant".to_owned(),
        };

        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0xC2; 32],
                )),
                tx_root: Hash::prehashed([0xC3; 32]),
                state_root: Hash::prehashed([0xC4; 32]),
                proposer: 6,
                height: 44,
                view: 9,
                epoch: 3,
                highest_qc: QcHeaderRef {
                    height: 43,
                    view: 8,
                    epoch: 3,
                    subject_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                        Hash::prehashed([0xC5; 32]),
                    ),
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::prehashed([0xC6; 32]),
        };
        let invalid_proposal_payload = EvidencePayload::InvalidProposal {
            proposal,
            reason: "forged proposal payload variant".to_owned(),
        };

        let censorship_payload = EvidencePayload::Censorship {
            tx_hash: sample_tx_hash(0xCC),
            receipts: Vec::new(),
        };

        let expected = EvidenceValidationError::KindPayloadMismatch;

        vec![
            (
                EvidenceKind::InvalidQc,
                double_vote_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::InvalidProposal,
                double_vote_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::DoublePrepare,
                invalid_qc_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::DoubleCommit,
                invalid_qc_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::DoublePrepare,
                invalid_proposal_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::DoubleCommit,
                invalid_proposal_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::InvalidQc,
                invalid_proposal_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::InvalidProposal,
                invalid_qc_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::Censorship,
                invalid_qc_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::InvalidQc,
                censorship_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::InvalidQc,
                invalid_proposal_payload.clone(),
                expected,
            ),
            (
                EvidenceKind::InvalidProposal,
                invalid_qc_payload.clone(),
                expected,
            ),
        ]
    }

    fn assert_invalid_evidence_rejected(
        context: &EvidenceValidationContext<'_>,
        evidence: &Evidence,
        expected_error: EvidenceValidationError,
    ) {
        let encoded = evidence.encode();
        let mut slice = encoded.as_slice();
        let decoded = Evidence::decode(&mut slice).expect("invalid evidence payload must decode");
        assert_eq!(
            decoded, *evidence,
            "encoded/decoded evidence should roundtrip without mutation"
        );
        let evidence = decoded;

        assert_eq!(validate_evidence(&evidence, context), Err(expected_error));

        let mut store = EvidenceStore::new();
        assert!(
            !store.insert(&evidence, context),
            "EvidenceStore must reject {expected_error:?}"
        );
        assert!(store.entries.is_empty());

        let state = test_state();
        assert!(
            !persist_record(&state, &evidence, context),
            "persist_record must reject {expected_error:?}"
        );
        let view = state.world.consensus_evidence.view();
        assert_eq!(view.iter().count(), 0);
    }

    #[test]
    fn detect_double_prevote() {
        let ctx = test_context();
        let h =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([2; 32]));
        let zero_root = iroha_crypto::Hash::prehashed([0u8; 32]);
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Prepare,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 5,
            view: 7,
            epoch: 0,
            highest_qc: None,
            signer: 1,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([3; 32]));
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let ev = check_double_vote(&v1, &v2).expect("should detect double vote");
        assert!(matches!(ev.kind, EvidenceKind::DoublePrepare));
    }

    #[test]
    fn detect_double_precommit() {
        let ctx = test_context();
        let (mut v1, mut v2) = sample_double_vote_pair(&ctx);
        v1.phase = Phase::Commit;
        v2.phase = Phase::Commit;
        let parent_root = iroha_crypto::Hash::prehashed([0xA1; 32]);
        let post_root = iroha_crypto::Hash::prehashed([0xA2; 32]);
        v1.parent_state_root = parent_root;
        v1.post_state_root = post_root;
        v2.parent_state_root = parent_root;
        v2.post_state_root = post_root;
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let ev = check_double_vote(&v1, &v2).expect("should detect double vote");
        assert!(matches!(ev.kind, EvidenceKind::DoubleCommit));
    }

    #[test]
    fn double_vote_detects_commit_root_mismatch() {
        let ctx = test_context();
        let subject = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x22; 32],
        ));
        let parent_root = iroha_crypto::Hash::prehashed([0xA1; 32]);
        let post_root = iroha_crypto::Hash::prehashed([0xA2; 32]);
        let other_post_root = iroha_crypto::Hash::prehashed([0xA3; 32]);
        let mut v1 = Vote {
            phase: Phase::Commit,
            block_hash: subject,
            parent_state_root: parent_root,
            post_state_root: post_root,
            height: 9,
            view: 2,
            epoch: 1,
            highest_qc: None,
            signer: 1,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.post_state_root = other_post_root;
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let ev = check_double_vote(&v1, &v2).expect("commit root mismatch must yield evidence");
        assert!(matches!(ev.kind, EvidenceKind::DoubleCommit));
    }

    #[test]
    fn double_vote_requires_distinct_block_hashes() {
        let ctx = test_context();
        let h = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x10; 32],
        ));
        let zero_root = iroha_crypto::Hash::prehashed([0u8; 32]);
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Prepare,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 11,
            view: 3,
            epoch: 0,
            highest_qc: None,
            signer: 1,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v1);
        let v2 = v1.clone();
        assert!(
            check_double_vote(&v1, &v2).is_none(),
            "identical votes should not yield double-vote evidence"
        );
    }

    #[test]
    fn double_vote_requires_matching_height_and_view() {
        let ctx = test_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x20; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Commit,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 5,
            view: 2,
            epoch: 0,
            highest_qc: None,
            signer: 7,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v1);
        let mut v2 = v1.clone();
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x21; 32]),
        );
        v2.height += 1;
        v2.view += 1;
        ctx.sign_vote(&mut v2);
        assert!(
            check_double_vote(&v1, &v2).is_none(),
            "height/view mismatch must not produce double-vote evidence"
        );

        // Restore height/view but change epoch to confirm epoch mismatch rejects evidence too.
        v2.height = v1.height;
        v2.view = v1.view;
        v2.epoch = 1;
        ctx.sign_vote(&mut v2);
        assert!(
            check_double_vote(&v1, &v2).is_none(),
            "epoch mismatch must not produce double-vote evidence"
        );
    }

    #[test]
    fn double_vote_requires_same_signer() {
        let ctx = test_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x30; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Prepare,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 8,
            view: 4,
            epoch: 0,
            highest_qc: None,
            signer: 3,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v1);
        let mut v2 = v1.clone();
        v2.signer = 4;
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x31; 32]),
        );
        ctx.sign_vote(&mut v2);
        assert!(
            check_double_vote(&v1, &v2).is_none(),
            "votes from different signers must not emit double-vote evidence"
        );
    }

    #[test]
    fn double_vote_detects_cross_phase_conflict() {
        let ctx = test_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x33; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Prepare,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 12,
            view: 6,
            epoch: 0,
            highest_qc: None,
            signer: 1,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.phase = super::super::consensus::Phase::Commit;
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x34; 32]),
        );
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);

        let evidence =
            check_double_vote(&v1, &v2).expect("cross-phase conflict must emit evidence");
        assert_eq!(evidence.kind, EvidenceKind::DoubleCommit);
    }

    #[test]
    fn double_vote_phase_must_match_kind() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x40; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Commit,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 6,
            view: 9,
            epoch: 0,
            highest_qc: None,
            signer: 2,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x41; 32]),
        );
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);

        let forged = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: v1.clone(),
                v2: v2.clone(),
            },
        };
        assert_eq!(
            validate_evidence(&forged, &context),
            Err(EvidenceValidationError::PhaseKindMismatch)
        );

        let valid = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert!(validate_evidence(&valid, &context).is_ok());
    }

    #[test]
    fn validate_double_vote_accepts_cross_phase_conflict() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x43; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Prepare,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 14,
            view: 7,
            epoch: 0,
            highest_qc: None,
            signer: 2,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.phase = Phase::Commit;
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x44; 32]),
        );
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);

        let ev = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert!(validate_evidence(&ev, &context).is_ok());
    }

    #[test]
    fn validate_double_vote_rejects_same_block_hash() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let hash = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x42; 32],
        ));
        let zero_root = zero_state_root();
        let mut vote = Vote {
            phase: Phase::Prepare,
            block_hash: hash,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 9,
            view: 1,
            epoch: 0,
            highest_qc: None,
            signer: 5,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut vote);
        let forged = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: vote.clone(),
                v2: vote.clone(),
            },
        };
        assert_eq!(
            validate_evidence(&forged, &context),
            Err(EvidenceValidationError::BlockHashMatch)
        );
    }

    #[test]
    fn validate_double_vote_rejects_epoch_mismatch() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x43; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Prepare,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 4,
            view: 3,
            epoch: 1,
            highest_qc: None,
            signer: 6,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v1);
        let mut v2 = v1.clone();
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x44; 32]),
        );
        v2.epoch = 2;
        ctx.sign_vote(&mut v2);
        let forged = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert_eq!(
            validate_evidence(&forged, &context),
            Err(EvidenceValidationError::EpochMismatch)
        );
    }

    #[test]
    fn validate_double_vote_rejects_signer_mismatch() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let h1 = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x45; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Prepare,
            block_hash: h1,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 7,
            view: 5,
            epoch: 0,
            highest_qc: None,
            signer: 3,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v1);
        let mut v2 = v1.clone();
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x46; 32]),
        );
        v2.signer = 4;
        ctx.sign_vote(&mut v2);
        let forged = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert_eq!(
            validate_evidence(&forged, &context),
            Err(EvidenceValidationError::SignerMismatch)
        );
    }

    #[test]
    fn kind_payload_mismatch_is_rejected() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Prepare,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0x50; 32]),
            ),
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 10,
            view: 1,
            epoch: 0,
            highest_qc: None,
            signer: 1,
            bls_sig: Vec::new(),
        };
        let mut v2 = Vote {
            phase: Phase::Prepare,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0x51; 32]),
            ),
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 10,
            view: 1,
            epoch: 0,
            highest_qc: None,
            signer: 1,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let ev = Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert_eq!(
            validate_evidence(&ev, &context),
            Err(EvidenceValidationError::KindPayloadMismatch)
        );
    }

    #[test]
    fn store_deduplicates() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let h =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([4; 32]));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Commit,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 1,
            view: 1,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([5; 32]));
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let ev = check_double_vote(&v1, &v2).unwrap();
        let mut store = EvidenceStore::new();
        assert!(store.insert(&ev, &context));
        assert!(!store.insert(&ev, &context));
        // Listing contains exactly one
        assert_eq!(store.entries.len(), 1);
    }

    #[test]
    fn store_rejects_invalid_evidence() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let h = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x60; 32],
        ));
        let zero_root = zero_state_root();
        let mut v = Vote {
            phase: Phase::Prepare,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 3,
            view: 2,
            epoch: 0,
            highest_qc: None,
            signer: 5,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v);
        let forged = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: v.clone(),
                v2: v.clone(),
            },
        };
        let mut store = EvidenceStore::new();
        assert!(
            !store.insert(&forged, &context),
            "forged double-vote evidence must be rejected"
        );
        assert!(store.entries.is_empty());
    }

    #[test]
    fn persist_record_rejects_invalid_double_vote() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let state = test_state();
        let h = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0x70; 32],
        ));
        let zero_root = zero_state_root();
        let mut v = Vote {
            phase: Phase::Prepare,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 12,
            view: 4,
            epoch: 0,
            highest_qc: None,
            signer: 6,
            bls_sig: Vec::new(),
        };
        ctx.sign_vote(&mut v);
        let forged = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: v.clone(),
                v2: v.clone(),
            },
        };
        assert!(
            !persist_record(&state, &forged, &context),
            "persist_record must ignore invalid double-vote evidence"
        );
        let view = state.world.consensus_evidence.view();
        assert_eq!(view.iter().count(), 0);
    }

    #[test]
    fn persist_record_rejects_missing_signature_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let evidence = double_vote_with_unchecked(&ctx, |v1, v2| {
            v1.bls_sig.clear();
            v2.bls_sig.clear();
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::SignatureMissing,
        );
    }

    #[test]
    fn persist_record_rejects_truncated_signature_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let evidence = double_vote_with_unchecked(&ctx, |v1, v2| {
            v1.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
            v2.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::SignatureTruncated,
        );
    }

    #[test]
    fn persist_record_rejects_invalid_signature_payload() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut evidence = double_vote_with(&ctx, |_, _| {});
        if let EvidencePayload::DoubleVote { v1, v2 } = &mut evidence.payload {
            if let Some(byte) = v1.bls_sig.first_mut() {
                *byte ^= 0x5A;
            }
            if let Some(byte) = v2.bls_sig.first_mut() {
                *byte ^= 0xA5;
            }
        }
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::SignatureInvalid,
        );
    }

    #[test]
    fn persist_record_rejects_duplicate_signer_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let evidence = double_vote_with(&ctx, |_, v2| {
            v2.signer = v2.signer.saturating_add(1);
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::SignerMismatch,
        );
    }

    #[test]
    fn persist_record_rejects_height_mismatch_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let evidence = double_vote_with(&ctx, |_, v2| {
            v2.height = v2.height.saturating_add(1);
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::HeightMismatch,
        );
    }

    #[test]
    fn persist_record_rejects_view_mismatch_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let evidence = double_vote_with(&ctx, |_, v2| {
            v2.view = v2.view.saturating_add(1);
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::ViewMismatch,
        );
    }

    #[test]
    fn persist_record_rejects_epoch_mismatch_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let evidence = double_vote_with(&ctx, |_, v2| {
            v2.epoch = v2.epoch.saturating_add(1);
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::EpochMismatch,
        );
    }

    #[test]
    fn persist_record_rejects_phase_kind_mismatch_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let (v1, v2) = sample_double_vote_pair(&ctx);
        // Keep block hashes distinct but forge the evidence kind to mismatch the vote phase.
        let evidence = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::PhaseKindMismatch,
        );
    }

    #[test]
    fn persist_record_rejects_kind_payload_mismatch_mutation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0x90; 32],
                )),
                tx_root: Hash::prehashed([0x91; 32]),
                state_root: Hash::prehashed([0x92; 32]),
                proposer: 7,
                height: 42,
                view: 3,
                epoch: 2,
                highest_qc: QcHeaderRef {
                    height: 41,
                    view: 2,
                    epoch: 2,
                    subject_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                        Hash::prehashed([0x93; 32]),
                    ),
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::prehashed([0x94; 32]),
        };
        let evidence = Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::InvalidProposal {
                proposal,
                reason: "forged payload variant".to_owned(),
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::KindPayloadMismatch,
        );
    }

    #[test]
    fn persist_record_rejects_invalid_proposal_height_invariant() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0xA0; 32],
                )),
                tx_root: Hash::prehashed([0xA1; 32]),
                state_root: Hash::prehashed([0xA2; 32]),
                proposer: 3,
                height: 40,
                view: 5,
                epoch: 1,
                highest_qc: QcHeaderRef {
                    height: 40,
                    view: 4,
                    epoch: 1,
                    subject_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                        Hash::prehashed([0xA3; 32]),
                    ),
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::prehashed([0xA4; 32]),
        };
        let evidence = Evidence {
            kind: EvidenceKind::InvalidProposal,
            payload: EvidencePayload::InvalidProposal {
                proposal,
                reason: "stale highest_qc height".to_owned(),
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::InvalidProposalHeight,
        );
    }

    #[test]
    fn persist_record_accepts_invalid_proposal_view_reset() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let parent = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA5; 32]));
        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: parent,
                tx_root: Hash::prehashed([0xA6; 32]),
                state_root: Hash::prehashed([0xA7; 32]),
                proposer: 4,
                height: 41,
                view: 0,
                epoch: 1,
                highest_qc: QcHeaderRef {
                    height: 40,
                    view: 6,
                    epoch: 1,
                    subject_block_hash: parent,
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::prehashed([0xA9; 32]),
        };
        let evidence = Evidence {
            kind: EvidenceKind::InvalidProposal,
            payload: EvidencePayload::InvalidProposal {
                proposal,
                reason: "view reset after height advance".to_owned(),
            },
        };
        assert!(validate_evidence(&evidence, &context).is_ok());
        let mut store = EvidenceStore::new();
        assert!(store.insert(&evidence, &context));
    }

    #[test]
    fn persist_record_rejects_invalid_proposal_parent_mismatch() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0xAA; 32],
                )),
                tx_root: Hash::prehashed([0xAB; 32]),
                state_root: Hash::prehashed([0xAC; 32]),
                proposer: 5,
                height: 60,
                view: 8,
                epoch: 2,
                highest_qc: QcHeaderRef {
                    height: 59,
                    view: 7,
                    epoch: 2,
                    subject_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                        Hash::prehashed([0xAD; 32]),
                    ),
                    phase: Phase::Commit,
                },
            },
            payload_hash: Hash::prehashed([0xAE; 32]),
        };
        let evidence = Evidence {
            kind: EvidenceKind::InvalidProposal,
            payload: EvidencePayload::InvalidProposal {
                proposal,
                reason: "parent/qc mismatch".to_owned(),
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::InvalidProposalParentMismatch,
        );
    }

    #[test]
    fn censorship_evidence_accepts_quorum_receipts() {
        let ctx = EvidenceTestContext::new(4);
        let context = ctx.validation_context();
        let tx_hash = sample_tx_hash(0xDD);
        let required = ctx.topology.min_votes_for_view_change();
        let receipts: Vec<_> = (0..required)
            .map(|idx| submission_receipt_for(&ctx, idx, tx_hash, 10))
            .collect();
        let evidence = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship { tx_hash, receipts },
        };
        assert!(validate_evidence(&evidence, &context).is_ok());
        let mut store = EvidenceStore::new();
        assert!(store.insert(&evidence, &context));
    }

    #[test]
    fn censorship_evidence_dedups_receipt_order() {
        let ctx = EvidenceTestContext::new(4);
        let context = ctx.validation_context();
        let tx_hash = sample_tx_hash(0xEE);
        let required = ctx.topology.min_votes_for_view_change();
        let receipts: Vec<_> = (0..required)
            .map(|idx| submission_receipt_for(&ctx, idx, tx_hash, 10))
            .collect();
        let mut reversed = receipts.clone();
        reversed.reverse();

        let evidence = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship { tx_hash, receipts },
        };
        let reordered = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: reversed,
            },
        };

        assert_eq!(evidence_key(&evidence), evidence_key(&reordered));

        let mut store = EvidenceStore::new();
        assert!(store.insert(&evidence, &context));
        assert!(
            !store.insert(&reordered, &context),
            "reordered receipts should not create a new evidence entry"
        );
    }

    #[test]
    fn censorship_subject_height_uses_latest_receipt() {
        let ctx = EvidenceTestContext::new(4);
        let tx_hash = sample_tx_hash(0xEF);
        let receipts = vec![
            submission_receipt_for(&ctx, 0, tx_hash, 5),
            submission_receipt_for(&ctx, 1, tx_hash, 12),
            submission_receipt_for(&ctx, 2, tx_hash, 9),
        ];
        let evidence = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship { tx_hash, receipts },
        };

        let (height, view) = evidence_subject_height_view(&evidence);
        assert_eq!(height, Some(12));
        assert_eq!(view, None);
    }

    #[test]
    fn double_vote_evidence_dedups_vote_order() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let (v1, v2) = sample_double_vote_pair(&ctx);
        let evidence = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: v1.clone(),
                v2: v2.clone(),
            },
        };
        let reordered = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1: v2, v2: v1 },
        };

        assert_eq!(evidence_key(&evidence), evidence_key(&reordered));

        let mut store = EvidenceStore::new();
        assert!(store.insert(&evidence, &context));
        assert!(
            !store.insert(&reordered, &context),
            "reordered votes should not create a new evidence entry"
        );
    }

    #[test]
    fn censorship_evidence_rejects_invalid_receipts() {
        let ctx = EvidenceTestContext::new(4);
        let context = ctx.validation_context();
        let tx_hash = sample_tx_hash(0xDE);

        let missing = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: Vec::new(),
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &missing,
            EvidenceValidationError::ReceiptMissing,
        );

        let mismatched = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash: sample_tx_hash(0xDF),
                receipts: vec![submission_receipt_for(&ctx, 0, tx_hash, 10)],
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &mismatched,
            EvidenceValidationError::ReceiptTxHashMismatch,
        );

        let outsider = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash,
            submitted_at_ms: 1,
            submitted_at_height: 3,
            signer: outsider.public_key().clone(),
        };
        let outsider_receipt = TransactionSubmissionReceipt::sign(payload, &outsider);
        let outsider_ev = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: vec![outsider_receipt],
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &outsider_ev,
            EvidenceValidationError::ReceiptSignerOutOfTopology,
        );

        let invalid_sig = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: vec![submission_receipt_with_invalid_signature(
                    &ctx, 0, tx_hash, 3,
                )],
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &invalid_sig,
            EvidenceValidationError::ReceiptSignatureInvalid,
        );

        let below_quorum = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: vec![submission_receipt_for(&ctx, 0, tx_hash, 3)],
            },
        };
        assert_invalid_evidence_rejected(
            &context,
            &below_quorum,
            EvidenceValidationError::ReceiptQuorumMissing,
        );
    }

    #[test]
    fn persist_record_rejects_mixed_manifest_payloads() {
        let ctx = test_context();
        let context = ctx.validation_context();
        for (kind, payload, expected) in mismatched_payload_cases(&ctx) {
            let evidence = Evidence { kind, payload };
            assert_invalid_evidence_rejected(&context, &evidence, expected);
        }
    }

    #[test]
    #[allow(clippy::type_complexity)]
    #[allow(clippy::too_many_lines)]
    fn fuzz_invalid_double_vote_mutations_are_rejected() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut rng = StdRng::seed_from_u64(0xF0_0D);
        let cases: &[(fn(&mut Vote, &mut Vote), EvidenceValidationError)] = &[
            (
                |_, v2| v2.signer = v2.signer.saturating_add(1),
                EvidenceValidationError::SignerMismatch,
            ),
            (
                |_, v2| v2.height = v2.height.saturating_add(1),
                EvidenceValidationError::HeightMismatch,
            ),
            (
                |_, v2| v2.view = v2.view.saturating_add(1),
                EvidenceValidationError::ViewMismatch,
            ),
            (
                |v1, v2| {
                    v2.block_hash = v1.block_hash;
                },
                EvidenceValidationError::BlockHashMatch,
            ),
            (
                |v1, v2| {
                    v1.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
                    v2.bls_sig.truncate(super::MIN_BLS_SIGNATURE_LEN / 2);
                },
                EvidenceValidationError::SignatureTruncated,
            ),
            (
                |_, v2| v2.epoch = v2.epoch.saturating_add(1),
                EvidenceValidationError::EpochMismatch,
            ),
        ];

        for _ in 0..32 {
            for (mutate, expected) in cases {
                let (mut v1, mut v2) = sample_double_vote_pair(&ctx);
                mutate(&mut v1, &mut v2);
                if !matches!(
                    expected,
                    EvidenceValidationError::SignatureMissing
                        | EvidenceValidationError::SignatureTruncated
                ) {
                    ctx.sign_vote(&mut v1);
                    ctx.sign_vote(&mut v2);
                }
                // jitter signatures to ensure encode/decode paths see varied payloads
                let noise: u8 = rng.random();
                v1.bls_sig.push(noise);
                v2.bls_sig.push(noise ^ 0xFF);

                let evidence = Evidence {
                    kind: EvidenceKind::DoublePrepare,
                    payload: EvidencePayload::DoubleVote { v1, v2 },
                };
                assert_invalid_evidence_rejected(&context, &evidence, *expected);
            }
        }

        // Cover signature missing and phase/kind mismatch explicitly.
        let evidence = double_vote_with_unchecked(&ctx, |v1, v2| {
            v1.bls_sig.clear();
            v2.bls_sig.clear();
        });
        assert_invalid_evidence_rejected(
            &context,
            &evidence,
            EvidenceValidationError::SignatureMissing,
        );

        let (v1, v2) = sample_double_vote_pair(&ctx);
        let forged = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        assert_invalid_evidence_rejected(
            &context,
            &forged,
            EvidenceValidationError::PhaseKindMismatch,
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fuzz_evidence_roundtrip_rejects_invalid_cases() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut rng = StdRng::seed_from_u64(0xE1D1_D00D);
        for _ in 0..32 {
            match rng.random::<u8>() % 4 {
                0 => {
                    let (v1, mut v2) = sample_double_vote_pair(&ctx);
                    let bump = u32::from(rng.random::<u8>() % 8 + 1);
                    v2.signer = v2.signer.saturating_add(bump);
                    ctx.sign_vote(&mut v2);
                    let evidence = Evidence {
                        kind: EvidenceKind::DoublePrepare,
                        payload: EvidencePayload::DoubleVote { v1, v2 },
                    };
                    assert_invalid_evidence_rejected(
                        &context,
                        &evidence,
                        EvidenceValidationError::SignerMismatch,
                    );
                }
                1 => {
                    let (v1, mut v2) = sample_double_vote_pair(&ctx);
                    if rng.random::<u8>() & 1 == 0 {
                        let delta = rng.random::<u64>() % 4 + 1;
                        v2.height = v2.height.saturating_add(delta);
                        ctx.sign_vote(&mut v2);
                        let evidence = Evidence {
                            kind: EvidenceKind::DoublePrepare,
                            payload: EvidencePayload::DoubleVote { v1, v2 },
                        };
                        assert_invalid_evidence_rejected(
                            &context,
                            &evidence,
                            EvidenceValidationError::HeightMismatch,
                        );
                    } else {
                        let delta = rng.random::<u64>() % 4 + 1;
                        v2.view = v2.view.saturating_add(delta);
                        ctx.sign_vote(&mut v2);
                        let evidence = Evidence {
                            kind: EvidenceKind::DoublePrepare,
                            payload: EvidencePayload::DoubleVote { v1, v2 },
                        };
                        assert_invalid_evidence_rejected(
                            &context,
                            &evidence,
                            EvidenceValidationError::ViewMismatch,
                        );
                    }
                }
                2 => {
                    let (mut v1, mut v2) = sample_double_vote_pair(&ctx);
                    if rng.random::<u8>() & 1 == 0 {
                        v1.bls_sig.clear();
                        v2.bls_sig.clear();
                        let evidence = Evidence {
                            kind: EvidenceKind::DoublePrepare,
                            payload: EvidencePayload::DoubleVote { v1, v2 },
                        };
                        assert_invalid_evidence_rejected(
                            &context,
                            &evidence,
                            EvidenceValidationError::SignatureMissing,
                        );
                    } else {
                        let bound = u16::try_from(super::MIN_BLS_SIGNATURE_LEN - 1).unwrap();
                        let truncate_to = usize::from(rng.random::<u16>() % bound + 1);
                        v1.bls_sig.truncate(truncate_to);
                        v2.bls_sig.truncate(truncate_to);
                        let evidence = Evidence {
                            kind: EvidenceKind::DoublePrepare,
                            payload: EvidencePayload::DoubleVote { v1, v2 },
                        };
                        assert_invalid_evidence_rejected(
                            &context,
                            &evidence,
                            EvidenceValidationError::SignatureTruncated,
                        );
                    }
                }
                _ => {
                    let horizon = rng.random::<u64>() % 8 + 1;
                    let stale_delta = rng.random::<u64>() % 5 + 1;
                    let current_height = horizon + stale_delta + (rng.random::<u64>() % 16 + 1);
                    let stale_height = current_height - horizon - stale_delta;
                    let mut v1;
                    let mut v2;
                    {
                        let pair = sample_double_vote_pair(&ctx);
                        v1 = pair.0;
                        v2 = pair.1;
                    }
                    v1.height = stale_height;
                    v2.height = stale_height;
                    let view = rng.random::<u64>() % 16 + 1;
                    v1.view = view;
                    v2.view = view;
                    let epoch = rng.random::<u64>() % 5;
                    v1.epoch = epoch;
                    v2.epoch = epoch;
                    ctx.sign_vote(&mut v1);
                    ctx.sign_vote(&mut v2);
                    let evidence = Evidence {
                        kind: EvidenceKind::DoublePrepare,
                        payload: EvidencePayload::DoubleVote {
                            v1: v1.clone(),
                            v2: v2.clone(),
                        },
                    };

                    let bytes = evidence.encode();
                    let mut slice = bytes.as_slice();
                    let decoded =
                        Evidence::decode(&mut slice).expect("decode stale evidence payload");
                    assert_eq!(decoded, evidence);
                    assert!(
                        validate_evidence(&decoded, &context).is_ok(),
                        "stale evidence must pass structural validation"
                    );

                    let state = state_with_horizon(current_height, horizon);
                    assert!(
                        !persist_record(&state, &decoded, &context),
                        "stale evidence (current_height={current_height}, horizon={horizon}, subject_height={stale_height}) must not persist"
                    );
                    let view = state.world.consensus_evidence.view();
                    assert_eq!(view.iter().count(), 0);
                }
            }
        }

        let mut cases = mismatched_payload_cases(&ctx);
        cases.shuffle(&mut rng);
        for (kind, payload, expected) in cases {
            let evidence = Evidence { kind, payload };
            assert_invalid_evidence_rejected(&context, &evidence, expected);
        }
    }

    #[test]
    fn persist_record_inserts_once() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let state = test_state();
        let h =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([6; 32]));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: super::super::consensus::Phase::Prepare,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 3,
            view: 2,
            epoch: 0,
            highest_qc: None,
            signer: 4,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([7; 32]));
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let evidence = check_double_vote(&v1, &v2).expect("double vote expected");

        assert!(persist_record(&state, &evidence, &context));
        // second insertion should be ignored
        assert!(!persist_record(&state, &evidence, &context));

        let view = state.world.consensus_evidence.view();
        let stored: Vec<_> = view.iter().map(|(_, rec)| rec.clone()).collect();
        assert_eq!(stored.len(), 1);
        let rec = &stored[0];
        assert_eq!(rec.evidence, evidence);
        assert_eq!(rec.recorded_at_height, v1.height);
        assert_eq!(rec.recorded_at_view, v1.view);
    }

    #[test]
    fn persist_record_inserts_once_for_precommit() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let state = test_state();
        let (mut v1, mut v2) = sample_double_vote_pair(&ctx);
        v1.phase = Phase::Commit;
        v2.phase = Phase::Commit;
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let evidence = Evidence {
            kind: EvidenceKind::DoubleCommit,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };

        assert!(persist_record(&state, &evidence, &context));
        assert!(
            !persist_record(&state, &evidence, &context),
            "duplicate precommit evidence should be ignored"
        );

        let view = state.world.consensus_evidence.view();
        let stored: Vec<_> = view.iter().map(|(_, rec)| rec.clone()).collect();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].evidence.kind, EvidenceKind::DoubleCommit);
    }

    #[test]
    fn record_double_vote_persists_once() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut store = EvidenceStore::new();
        let state = test_state();
        let (v1, v2) = sample_double_vote_pair(&ctx);

        assert!(
            record_double_vote(&mut store, &state, &v1, &v2, &context),
            "first equivocation must be recorded"
        );
        assert!(
            !record_double_vote(&mut store, &state, &v1, &v2, &context),
            "duplicate equivocation should be deduplicated"
        );

        let view = state.world.consensus_evidence.view();
        let stored: Vec<_> = view.iter().collect();
        assert_eq!(stored.len(), 1);
        assert_eq!(store.entries.len(), 1);
    }

    #[test]
    fn record_double_vote_rejects_persisted_duplicates() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let state = test_state();
        let (v1, v2) = sample_double_vote_pair(&ctx);

        let mut store = EvidenceStore::new();
        assert!(
            record_double_vote(&mut store, &state, &v1, &v2, &context),
            "first equivocation must be recorded"
        );
        assert_eq!(
            state.world.consensus_evidence.view().iter().count(),
            1,
            "evidence should be persisted to WSV"
        );

        // Simulate a restart with a fresh in-memory store.
        let mut fresh_store = EvidenceStore::new();
        assert!(
            !record_double_vote(&mut fresh_store, &state, &v1, &v2, &context),
            "persisted evidence must block duplicates even when the in-memory store is empty"
        );
        assert_eq!(
            state.world.consensus_evidence.view().iter().count(),
            1,
            "WSV must not store duplicates"
        );
        assert_eq!(
            fresh_store.entries.len(),
            1,
            "fresh store should still record the duplicate to avoid relogging"
        );
    }

    #[test]
    fn record_double_vote_handles_precommit_equivocation() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut store = EvidenceStore::new();
        let state = test_state();
        let (mut v1, mut v2) = sample_double_vote_pair(&ctx);
        v1.phase = Phase::Commit;
        v2.phase = Phase::Commit;
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);

        assert!(
            record_double_vote(&mut store, &state, &v1, &v2, &context),
            "precommit equivocation should be recorded"
        );
        let view = state.world.consensus_evidence.view();
        let (_, record) = view.iter().next().expect("evidence must be stored");
        assert_eq!(record.evidence.kind, EvidenceKind::DoubleCommit);
    }

    #[test]
    fn record_double_vote_dedupes_per_phase() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut store = EvidenceStore::new();
        let state = test_state();

        // Prepare equivocation inserts once.
        let (prevote1, prevote2) = sample_double_vote_pair(&ctx);
        assert!(record_double_vote(
            &mut store, &state, &prevote1, &prevote2, &context
        ));
        assert!(
            !record_double_vote(&mut store, &state, &prevote1, &prevote2, &context),
            "duplicate prevote evidence should be ignored"
        );

        // Commit equivocation inserts independently and dedupes.
        let (mut precommit1, mut precommit2) = sample_double_vote_pair(&ctx);
        precommit1.phase = Phase::Commit;
        precommit2.phase = Phase::Commit;
        ctx.sign_vote(&mut precommit1);
        ctx.sign_vote(&mut precommit2);
        assert!(record_double_vote(
            &mut store,
            &state,
            &precommit1,
            &precommit2,
            &context
        ));
        assert!(
            !record_double_vote(&mut store, &state, &precommit1, &precommit2, &context),
            "duplicate precommit evidence should be ignored"
        );

        let view = state.world.consensus_evidence.view();
        let kinds: Vec<EvidenceKind> = view.iter().map(|(_, rec)| rec.evidence.kind).collect();
        assert_eq!(kinds.len(), 2, "one record per phase expected");
        assert!(
            kinds.contains(&EvidenceKind::DoublePrepare)
                && kinds.contains(&EvidenceKind::DoubleCommit),
            "prevote and precommit evidence should both be persisted"
        );
    }

    #[test]
    fn record_double_vote_detects_cross_phase_conflict() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut store = EvidenceStore::new();
        let state = test_state();
        let (v1, mut v2) = sample_double_vote_pair(&ctx);
        v2.phase = Phase::Commit;
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x90; 32]),
        );
        ctx.sign_vote(&mut v2);

        assert!(
            record_double_vote(&mut store, &state, &v1, &v2, &context),
            "cross-phase conflict must be recorded once"
        );
        assert!(
            !record_double_vote(&mut store, &state, &v1, &v2, &context),
            "duplicate cross-phase conflict should be deduplicated"
        );

        let view = state.world.consensus_evidence.view();
        let (_, record) = view.iter().next().expect("evidence must persist to WSV");
        assert_eq!(record.evidence.kind, EvidenceKind::DoubleCommit);
    }

    #[test]
    fn record_double_vote_dedupes_cross_phase_ordering() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let mut store = EvidenceStore::new();
        let state = test_state();
        let (mut prevote, mut precommit) = sample_double_vote_pair(&ctx);
        prevote.phase = Phase::Prepare;
        precommit.phase = Phase::Commit;
        precommit.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x91; 32]),
        );
        ctx.sign_vote(&mut prevote);
        ctx.sign_vote(&mut precommit);

        assert!(
            record_double_vote(&mut store, &state, &prevote, &precommit, &context),
            "first cross-phase conflict must be recorded"
        );
        assert!(
            !record_double_vote(&mut store, &state, &precommit, &prevote, &context),
            "reversed cross-phase detection should deduplicate the same conflict"
        );

        let view = state.world.consensus_evidence.view();
        assert_eq!(view.iter().count(), 1, "WSV must persist only one record");
        assert_eq!(store.entries.len(), 1, "in-memory store must dedupe");
    }

    #[test]
    fn persist_record_rejects_stale_evidence_replay() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let state = state_with_horizon(50, 3);
        let view = state.view();
        assert_eq!(
            view.world()
                .sumeragi_npos_parameters()
                .map(|params| params.evidence_horizon_blocks()),
            Some(3)
        );
        let current_height = u64::try_from(view.height()).unwrap_or(0);
        assert_eq!(current_height, 50);
        drop(view);

        let h = HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
            [0xB3; 32],
        ));
        let zero_root = zero_state_root();
        let mut v1 = Vote {
            phase: Phase::Prepare,
            block_hash: h,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 40,
            view: 2,
            epoch: 0,
            highest_qc: None,
            signer: 7,
            bls_sig: Vec::new(),
        };
        let mut v2 = v1.clone();
        v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0xB4; 32]),
        );
        ctx.sign_vote(&mut v1);
        ctx.sign_vote(&mut v2);
        let original = check_double_vote(&v1, &v2).expect("double vote expected");
        let encoded = original.encode();
        let mut slice = encoded.as_slice();
        let evidence = Evidence::decode(&mut slice).expect("roundtrip stale evidence");
        assert_eq!(evidence, original);
        assert!(
            !persist_record(&state, &evidence, &context),
            "evidence beyond configured horizon must not persist"
        );
        let view = state.world.consensus_evidence.view();
        assert_eq!(view.iter().count(), 0);
    }

    #[test]
    fn invalid_qc_shape_with_empty_bitmap_emits_evidence() {
        let subject =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([8; 32]));
        let validator_set = sample_validator_set();
        let zero_root = zero_state_root();
        let qc = Qc {
            phase: Phase::Prepare,
            subject_block_hash: subject,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 3,
            view: 1,
            epoch: 0,
            mode_tag: super::super::consensus::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: Vec::new(),
                bls_aggregate_signature: Vec::new(),
            },
        };
        let ev = check_invalid_commit_qc_shape(&qc).expect("empty bitmap must produce evidence");
        assert!(matches!(ev.kind, EvidenceKind::InvalidQc));
    }

    #[test]
    fn valid_qc_shape_skips_invalid_evidence() {
        let subject =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([9; 32]));
        let validator_set = sample_validator_set();
        let zero_root = zero_state_root();
        let qc = Qc {
            phase: Phase::Commit,
            subject_block_hash: subject,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 2,
            view: 2,
            epoch: 0,
            mode_tag: super::super::consensus::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![0x01],
                bls_aggregate_signature: Vec::new(),
            },
        };
        assert!(
            check_invalid_commit_qc_shape(&qc).is_none(),
            "valid QC shape should not emit invalid evidence"
        );
    }

    #[test]
    fn roadmap_invalid_evidence_roundtrip_cases() {
        let ctx = test_context();
        let context = ctx.validation_context();
        let cases: &[EvidenceRoundtripCase] = &[
            (
                "duplicate signer",
                EvidenceValidationError::SignerMismatch,
                roundtrip_case_duplicate_signer,
            ),
            (
                "conflicting height",
                EvidenceValidationError::HeightMismatch,
                roundtrip_case_conflicting_height,
            ),
            (
                "conflicting view",
                EvidenceValidationError::ViewMismatch,
                roundtrip_case_conflicting_view,
            ),
            (
                "forged signature length",
                EvidenceValidationError::SignatureTruncated,
                roundtrip_case_signature_truncated,
            ),
            (
                "mixed manifest payload",
                EvidenceValidationError::KindPayloadMismatch,
                roundtrip_case_mixed_manifest_payload,
            ),
        ];

        for (label, expected, build) in cases {
            let evidence = build(&ctx);
            assert_invalid_evidence_rejected(&context, &evidence, *expected);
            assert!(
                validate_evidence(&evidence, &context).is_err(),
                "{label}: expected structural validation to fail"
            );
        }
    }
}
