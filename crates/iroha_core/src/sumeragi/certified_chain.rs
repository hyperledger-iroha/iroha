//! The certified-chain reader: committed Sumeragi blocks as their Kura frames certify them
//! (`specs/sumeragi.md` §12.7).
//!
//! Kura holds one frame per committed height: the result-bearing iroha block and its
//! [`CommitCertificate`] — the canonical core [`BlockHeader`], the `CommitQC` and the preimage of
//! the certified result `R` ([`ExecutionResultCommitment`]). Genesis carries a result-only
//! certificate (no header, no `CommitQC`). Its signed body and registered authority are the
//! network-pinned trust root. The result-only execution preimage is added after genesis executes;
//! genesis signatures do not authenticate that preimage. An exact-quorum successor authenticates
//! `R_g` through its signed `parent_result`; a genesis-only result requires local deterministic
//! execution trust or a separately authenticated replay anchor.
//!
//! The reader offers two reads with different trust models:
//!
//! - [`committed_block`]: **consensus-visible data only**, authenticated by the
//!   original native execution tip captured in the same State publication generation.
//!   A bounded reverse walk checks native `parent_hash`, `parent_result` and the
//!   Iroha parent hash before interpreting the requested result. The tip is issued
//!   only by the original verified worker (or original signed-genesis execution),
//!   outside World to avoid an R self-reference. Snapshot claims are verified
//!   against an actual certified native prefix before this authority is restored.
//!   Local `CommitQC` bytes are never decoded by this deterministic path: honest
//!   nodes may retain different valid exact quorums for the same header and R.
//!   Hash journals and structural frame decoding alone grant no execution authority.
//! - [`CertifiedChain::certified`]: the committed read **plus the local `CommitQC`**. The
//!   certificate must certify exactly this header and result (kind, height, block hash, result,
//!   attestation flag, instance) and verify under the committee of its height (see below). The
//!   default verifier checks both the exact BLS quorum and source-complete native paired-Pasta
//!   attestations. A caller may explicitly supply an application verifier; there is no
//!   signature-only finality fallback. Off-chain consumers use this read.
//!
//! **Epoch authority.** Every result retains its complete native epoch and schedule graph.
//! The reader verifies the prefix iteratively from the signed genesis context, checks both
//! parent links and the parent result at every height, and carries a bounded schedule plus
//! active authority. Chain parameters retain lag two; a successor authority is admitted only
//! after the incumbent exact quorum certifies the mandatory attested boundary. Every signature
//! binds the scheduling epoch and complete context identity. The preceding certified pulse
//! supplies fresh leader randomness, even when an authority generation is retained.
//! Missing, reordered or invalid authority fails closed even after that committee has rotated
//! out of World. There is no trust-in-local-storage certificate verdict.
//!
//! **Chains.** [`CertifiedChain::walk`] reads consecutive heights and additionally checks that
//! each block extends the previous one: its core header binds the parent's block hash and `R`,
//! and its iroha header the parent's iroha hash.
//!
//! **Restoration.** `CertifiedChain::from_pinned` runs this same verifier before a State exists,
//! against an explicit network, configured chain instance and exact committed hash cut. It
//! accepts no World authority. Each body is checked against the cut before its prefix is
//! authenticated; the lone-genesis execution trust limitation is unchanged.

use std::{num::NonZeroUsize, sync::Arc};

use iroha_crypto::{Hash, HashOf, PublicKey as IrohaPublicKey};
use iroha_data_model::{
    block::{
        BlockHeader as IrohaHeader, CommitCertificate, SignedBlock,
        consensus::HeightContextId,
        proofs::{
            TrustedBlockProofAnchor, TrustedBlockProofAnchorError, TrustedExecutionOutputAnchor,
        },
    },
    parameter::system::ConsensusMode,
    sumeragi::epoch::ValidatorEpochContextV1,
    transaction::TransactionEntrypoint,
};
use iroha_sumeragi::{
    crypto::{AttestationVerifier, CertError},
    message::{BlockHeader, Qc, VoteKind},
    preimage::TAG_PAY,
    types::{Committee, EpochId, Hash32},
};

use super::{
    commitment::{ExecutionResultCommitment, result_of_preimage},
    crypto::{BlsCrypto, core_key},
    node::global_instance,
    schedule,
    startup::{GENESIS_HEIGHT, core_hash_of},
};
use crate::{
    kura::Kura,
    state::{StateReadOnly, StateView},
};
use iroha_data_model::NetworkId;
use iroha_model_base::chain::ChainId;

/// Domain tag of a certified block id: `H(tag ‖ block_hash ‖ R)`.
pub const CERTIFIED_BLOCK_ID_TAG: &[u8] = b"iroha/sumeragi/certified-block/v1";

/// Why a committed height could not be read or verified. Every variant is local: a missing
/// height, a view/Kura disagreement, local corruption or a certificate that does not verify.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ChainReadError {
    /// The height is zero or above this view's committed height.
    #[error("height {height} is not committed in this view")]
    NotCommitted {
        /// Requested height.
        height: u64,
    },
    /// Kura does not hold the block this view committed at the height.
    #[error("Kura does not hold the block this view committed at {height}")]
    NotInView {
        /// Requested height.
        height: u64,
    },
    /// The block carries no commit certificate.
    #[error("the block at {height} carries no commit certificate")]
    MissingCertificate {
        /// Requested height.
        height: u64,
    },
    /// A certificate part is not one canonical frame, or has the wrong shape.
    #[error("the commit certificate at {height} is malformed: {reason}")]
    Malformed {
        /// Requested height.
        height: u64,
        /// What is wrong.
        reason: String,
    },
    /// The certified header or `CommitQC` does not belong to the stored block at this height.
    #[error("the certified header does not match the block at {height}")]
    HeaderMismatch {
        /// Requested height.
        height: u64,
    },
    /// The result preimage does not hash to the certified result.
    #[error("the result preimage at {height} does not hash to the certified result")]
    ResultMismatch {
        /// Requested height.
        height: u64,
    },
    /// The certified result does not commit the stored result-bearing block.
    #[error("the certified result at {height} does not commit the stored block")]
    ExecutionMismatch {
        /// Requested height.
        height: u64,
    },
    /// The certificate names another chain instance.
    #[error("the block at {height} is certified for another chain instance")]
    WrongInstance {
        /// Requested height.
        height: u64,
    },
    /// The block does not extend the block before it.
    #[error("the block at {height} does not extend its parent")]
    Discontinuous {
        /// Requested height.
        height: u64,
    },
    /// Kura's genesis is not this view's network genesis.
    #[error("Kura's genesis is not this view's network genesis")]
    ForeignGenesis,
    /// The committee of the height cannot be formed.
    #[error("no committee for height {height}: {reason}")]
    Committee {
        /// Requested height.
        height: u64,
        /// What is wrong.
        reason: String,
    },
    /// The `CommitQC` does not verify under the committee of its height.
    #[error("the commit certificate at {height} does not verify: {error:?}")]
    Certificate {
        /// Requested height.
        height: u64,
        /// The failed check.
        error: CertError,
    },
}

// Keep the committed-source failure typed across the portable data-model boundary.
// Local allocation/admission errors never pass through this conversion.
impl From<ChainReadError> for iroha_data_model::sumeragi_finality::ScheduleSourceError {
    fn from(error: ChainReadError) -> Self {
        match error {
            ChainReadError::NotCommitted { height } => Self::NotCommitted { height },
            ChainReadError::NotInView { height } => Self::NotInView { height },
            ChainReadError::MissingCertificate { height } => Self::MissingCertificate { height },
            ChainReadError::Malformed { height, reason } => Self::Malformed { height, reason },
            ChainReadError::HeaderMismatch { height } => Self::HeaderMismatch { height },
            ChainReadError::ResultMismatch { height } => Self::ResultMismatch { height },
            ChainReadError::ExecutionMismatch { height } => Self::ExecutionMismatch { height },
            ChainReadError::WrongInstance { height } => Self::WrongInstance { height },
            ChainReadError::Discontinuous { height } => Self::Discontinuous { height },
            ChainReadError::ForeignGenesis => Self::ForeignGenesis,
            ChainReadError::Committee { height, reason } => Self::Committee { height, reason },
            ChainReadError::Certificate { height, error } => Self::Certificate { height, error },
        }
    }
}

impl From<ChainReadError> for iroha_data_model::sumeragi_finality::ScheduleError {
    fn from(error: ChainReadError) -> Self {
        Self::CommittedSource(error.into())
    }
}

/// The consensus-visible receipt of a committed height: identical on every honest node, derived
/// from the stored block, its core header and its result preimage (never from the `CommitQC`).
#[derive(Clone, Debug)]
pub struct CommittedBlock {
    height: u64,
    block: Arc<SignedBlock>,
    header: Option<BlockHeader>,
    core_hash: Hash32,
    result: Hash32,
    commitment: ExecutionResultCommitment,
}

impl CommittedBlock {
    /// The one-based height.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.height
    }

    /// The committed result-bearing iroha block as Kura stores it. Its
    /// [`commit_certificate`](SignedBlock::commit_certificate) is node-local: deterministic code
    /// must not read it.
    #[must_use]
    pub fn block(&self) -> &Arc<SignedBlock> {
        &self.block
    }

    /// The iroha block hash.
    #[must_use]
    pub fn block_hash(&self) -> HashOf<IrohaHeader> {
        self.block.hash()
    }

    /// The certified core header (`None` for genesis).
    #[must_use]
    pub fn header(&self) -> Option<&BlockHeader> {
        self.header.as_ref()
    }

    /// The core block hash: the hash of the core header, or the iroha hash bytes for genesis.
    #[must_use]
    pub fn core_hash(&self) -> Hash32 {
        self.core_hash
    }

    /// The certified execution result `R`.
    #[must_use]
    pub fn result(&self) -> Hash32 {
        self.result
    }

    /// The decoded preimage of `R`.
    #[must_use]
    pub fn commitment(&self) -> &ExecutionResultCommitment {
        &self.commitment
    }

    /// The certified block id `H(CERTIFIED_BLOCK_ID_TAG ‖ core_hash ‖ R)`: what a `CommitQC` of
    /// this height certifies, as one identifier. Signer floors and proofs pin it in their
    /// `context_id` fields (the data-model field keeps that name until the wire cleanup).
    #[must_use]
    pub fn id(&self) -> HeightContextId {
        certified_block_id(&self.core_hash, &self.result)
    }

    /// The block time in milliseconds.
    #[must_use]
    pub fn block_time_ms(&self) -> u64 {
        u64::try_from(self.block.header().creation_time().as_millis()).unwrap_or(u64::MAX)
    }

    /// Whether `self` directly extends `parent`: consecutive heights, the core header binds the
    /// parent's block hash and `R`, and the iroha header the parent's iroha hash.
    #[must_use]
    pub fn extends(&self, parent: &Self) -> bool {
        parent.height.checked_add(1) == Some(self.height)
            && self.block.header().prev_block_hash() == Some(parent.block_hash())
            && self.header.as_ref().is_some_and(|header| {
                header.parent_hash == parent.core_hash && header.parent_result == parent.result
            })
    }

    /// The inclusion anchor of the network entrypoint `entry_hash`, bound to the executed wire
    /// that `R` commits.
    ///
    /// # Errors
    /// The entry is absent or the block's Merkle material is inconsistent.
    pub fn entry_anchor(
        &self,
        entry_hash: &HashOf<TransactionEntrypoint>,
    ) -> Result<TrustedBlockProofAnchor, TrustedBlockProofAnchorError> {
        let execution = &self.commitment.execution;
        TrustedBlockProofAnchor::from_committed_execution(
            &self.block,
            execution.executed_block_wire_len,
            execution.executed_block_wire_hash,
            entry_hash,
        )
    }

    /// The anchor of the typed output at `output_index`, bound to the executed wire that `R`
    /// commits.
    ///
    /// # Errors
    /// The output is absent or the block's Merkle material is inconsistent.
    pub fn output_anchor(
        &self,
        output_index: u32,
    ) -> Result<TrustedExecutionOutputAnchor, TrustedBlockProofAnchorError> {
        let execution = &self.commitment.execution;
        TrustedExecutionOutputAnchor::from_committed_execution(
            &self.block,
            execution.executed_block_wire_len,
            execution.executed_block_wire_hash,
            output_index,
        )
    }
}

/// `H(CERTIFIED_BLOCK_ID_TAG ‖ block_hash ‖ R)` as the data model's id type.
#[must_use]
pub fn certified_block_id(block_hash: &Hash32, result: &Hash32) -> HeightContextId {
    let mut bytes = Vec::with_capacity(CERTIFIED_BLOCK_ID_TAG.len() + 64);
    bytes.extend_from_slice(CERTIFIED_BLOCK_ID_TAG);
    bytes.extend_from_slice(&block_hash.0);
    bytes.extend_from_slice(&result.0);
    HeightContextId(HashOf::from_untyped_unchecked(Hash::new(&bytes)))
}

/// Read the consensus-visible receipt of the block `view` committed at `height` (see the module
/// documentation). Deterministic: every honest node with the same committed history reads the
/// same receipt, whatever `CommitQC` it stored.
///
/// # Errors
/// The height is not committed, Kura does not hold the view's block, or the frame's header or
/// result preimage does not certify it.
pub fn committed_block(
    view: &(impl StateReadOnly + ?Sized),
    height: u64,
) -> Result<CommittedBlock, ChainReadError> {
    let index = usize::try_from(height)
        .ok()
        .and_then(NonZeroUsize::new)
        .filter(|index| index.get() <= view.block_hashes().len())
        .ok_or(ChainReadError::NotCommitted { height })?;
    view.canonical_history()
        .executed_receipt(index, |_, _| Ok(()))
        .map_err(|error| ChainReadError::Malformed {
            height,
            reason: error.to_string(),
        })
}

/// Structural interpretation only: the caller must authenticate core hash and R
/// from an original State tip or the full native certificate prefix.
pub(crate) fn read_frame(
    block: Arc<SignedBlock>,
    height: u64,
) -> Result<CommittedBlock, ChainReadError> {
    #[cfg(test)]
    relation_counts::frame(height);
    let malformed = |reason: String| ChainReadError::Malformed { height, reason };
    // Hashing only: the chain hash `H` needs no admitted key.
    let hasher = BlsCrypto::new();
    let certificate = block
        .commit_certificate()
        .ok_or(ChainReadError::MissingCertificate { height })?;
    let genesis = height == GENESIS_HEIGHT;
    let (header, core_hash) = if genesis {
        if !certificate.consensus_header().is_empty()
            || !certificate.commit_qc().is_empty()
            || !certificate.availability().is_empty()
        {
            return Err(malformed(
                "genesis carries a result-only certificate".into(),
            ));
        }
        (None, core_hash_of(&block))
    } else {
        let header: BlockHeader = norito::decode_canonical(certificate.consensus_header())
            .map_err(|error| malformed(error.to_string()))?;
        let payload_len = block
            .resultless_proposal_wire_len()
            .map_err(|error| malformed(error.to_string()))?;
        let payload_hash = Hash::new_from_writer(|writer| {
            writer.write_all(TAG_PAY)?;
            block
                .write_resultless_proposal_wire(writer)
                .map_err(std::io::Error::other)
        })
        .map_err(|error| malformed(error.to_string()))?;
        if certificate.availability().is_empty()
            || header.height != height
            || header.payload_len == 0
            || u32::try_from(payload_len).ok() != Some(header.payload_len)
            || Hash32(*payload_hash.as_ref()) != header.payload_hash
        {
            return Err(ChainReadError::HeaderMismatch { height });
        }
        let core_hash = header.hash(&hasher);
        (Some(header), core_hash)
    };
    let result = result_of_preimage(certificate.result_preimage());
    let commitment = ExecutionResultCommitment::decode(certificate.result_preimage())
        .map_err(|error| malformed(error.to_string()))?;
    if let Some(header) = &header {
        super::epoch_beacon::control::verify_result(
            &header.control_witness,
            commitment.beacon.as_ref(),
        )
        .map_err(|error| malformed(error.to_string()))?;
        if commitment.beacon.as_ref().is_some_and(|pulse| {
            pulse.context.instance != header.instance.0
                || pulse.context.epoch != header.epoch.epoch
                || pulse.context.epoch_context_id != header.epoch.context.0
                || pulse.context.parent_consensus_hash != header.parent_hash.0
                || pulse.context.parent_result != header.parent_result.0
        }) {
            return Err(ChainReadError::HeaderMismatch { height });
        }
        let epoch = schedule::core_epoch(&commitment.schedule.current)
            .map_err(|error| malformed(error.to_string()))?;
        if header.epoch != epoch.id || (commitment.schedule.boundary.is_some() && !header.attest) {
            return Err(ChainReadError::HeaderMismatch { height });
        }
    }
    let (wire_len, wire_hash) = block
        .executed_block_wire_identity()
        .map_err(|error| malformed(error.to_string()))?;
    if commitment.height != height
        || block.header().height().get() != height
        || commitment.execution.executed_block_wire_len != wire_len
        || commitment.execution.executed_block_wire_hash != wire_hash
    {
        return Err(ChainReadError::ExecutionMismatch { height });
    }
    if commitment.beacon.as_ref().is_some_and(|pulse| {
        Some(pulse.finalized_chain_anchor.block_hash) != block.header().prev_block_hash()
    }) {
        return Err(malformed(
            "beacon pulse names another committed parent".into(),
        ));
    }
    Ok(CommittedBlock {
        height,
        block,
        header,
        core_hash,
        result,
        commitment,
    })
}

/// How the local `CommitQC` of a height was checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QcVerification {
    /// Genesis body signatures and network identity verified, without a `CommitQC`.
    /// This alone does not authenticate its post-execution result preimage. Consumers needing
    /// independent genesis execution evidence require a verified successor or replay anchor.
    Genesis,
    /// The `CommitQC` verified under the committee of its height.
    Verified,
}

/// A committed height with its local `CommitQC` checked.
#[derive(Clone, Debug)]
pub struct CertifiedBlock {
    committed: CommittedBlock,
    commit_qc: Option<Qc>,
    verification: QcVerification,
    certificate_len: usize,
}

impl CertifiedBlock {
    /// The consensus-visible receipt.
    #[must_use]
    pub fn committed(&self) -> &CommittedBlock {
        &self.committed
    }

    /// Take the consensus-visible receipt.
    #[must_use]
    pub fn into_committed(self) -> CommittedBlock {
        self.committed
    }

    /// This node's `CommitQC` of the block (`None` for genesis).
    #[must_use]
    pub fn commit_qc(&self) -> Option<&Qc> {
        self.commit_qc.as_ref()
    }

    /// How the `CommitQC` was checked.
    #[must_use]
    pub fn verification(&self) -> QcVerification {
        self.verification
    }

    /// The node-local commit certificate (header, `CommitQC`, result preimage); always present
    /// on a certified read.
    #[must_use]
    pub fn certificate(&self) -> Option<&CommitCertificate> {
        self.committed.block.commit_certificate()
    }

    /// Canonical frame bytes of the certificate, for callers that bound what they read.
    #[must_use]
    pub fn certificate_len(&self) -> usize {
        self.certificate_len
    }
}

impl core::ops::Deref for CertifiedBlock {
    type Target = CommittedBlock;

    fn deref(&self) -> &CommittedBlock {
        &self.committed
    }
}

/// One complete epoch authority. Its crypto owner admits only this roster's original proofs.
struct VerifiedAuthority {
    material: ValidatorEpochContextV1,
    epoch: EpochId,
    committee: Committee,
    crypto: BlsCrypto,
}

impl VerifiedAuthority {
    fn new(material: ValidatorEpochContextV1, height: u64) -> Result<Self, ChainReadError> {
        let malformed = |reason: String| ChainReadError::Committee { height, reason };
        let epoch = schedule::core_epoch(&material)
            .map_err(|error| malformed(error.to_string()))?
            .id;
        let crypto = BlsCrypto::new();
        let keys = material
            .committee
            .iter()
            .map(|member| core_key(member.validator.public_key()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| malformed(error.to_string()))?;
        let committee =
            schedule::global_committee(keys).map_err(|error| malformed(error.to_string()))?;
        crypto
            .admit_committee(material.committee.iter().map(|member| {
                (
                    member.validator.public_key(),
                    member.proof_of_possession.as_slice(),
                )
            }))
            .map_err(|(index, error)| malformed(format!("member {index}: {error}")))?;
        Ok(Self {
            material,
            epoch,
            committee,
            crypto,
        })
    }
}

/// Constant-size cursor: one committed parent, the bounded native schedule and active authority.
/// A sequential walk verifies each height once; older random reads replay from signed genesis.
struct VerifiedPrefix {
    tip: CommittedBlock,
    schedule: schedule::ConsensusSchedule,
    authority: Arc<VerifiedAuthority>,
}

/// One crypto context for both random pinned reads and one-pass externally streamed evidence.
struct PrefixVerifierContext<'a> {
    instance: Hash32,
    network: NetworkId,
    attestations: Option<&'a dyn AttestationVerifier>,
}
impl PrefixVerifierContext<'_> {
    fn advance_prefix(
        &self,
        prefix: &mut VerifiedPrefix,
        committed: CommittedBlock,
    ) -> Result<CertifiedBlock, ChainReadError> {
        let height = committed.height;
        if !committed.extends(&prefix.tip) {
            return Err(ChainReadError::Discontinuous { height });
        }
        let malformed = |reason: String| ChainReadError::Committee { height, reason };
        let scheduled = prefix
            .schedule
            .ready(height)
            .map_err(|error| malformed(error.to_string()))?;
        let authority = if scheduled.epoch == prefix.authority.material {
            Arc::clone(&prefix.authority)
        } else {
            Arc::new(VerifiedAuthority::new(scheduled.epoch.clone(), height)?)
        };
        if committed.commitment.schedule.current != authority.material {
            return Err(malformed(
                "result changes its authenticated incumbent context".into(),
            ));
        }
        let config = scheduled
            .height_config()
            .map_err(|error| malformed(error.to_string()))?;
        let certified = self.verify_certificate(committed, &authority, Some(&config))?;
        if let Some(boundary) = &certified.commitment.schedule.boundary {
            if boundary.selection_anchor != prefix.tip.block_hash() {
                return Err(malformed(
                    "boundary selection anchor differs from certified parent".into(),
                ));
            }
            let pulse = prefix.tip.commitment.beacon.as_ref().ok_or_else(|| {
                malformed("boundary predecessor omits its certified selection pulse".into())
            })?;
            let expected_seed = crate::beacon::global_threshold_beacon_npos_successor_seed_v1(
                pulse,
                height,
                boundary.next.authorization.epoch,
            );
            if boundary.next.leader_seed != expected_seed {
                return Err(malformed(
                    "boundary leader seed differs from certified fresh pulse".into(),
                ));
            }
            if let Some(preparation) = &boundary.preparation {
                let expected = super::epoch_election::election_seed(
                    authority.material.network_id,
                    authority.material.authorization.epoch,
                    pulse,
                )
                .map_err(malformed)?;
                if preparation.election_seed != expected {
                    return Err(malformed(
                        "frozen election seed differs from certified fresh pulse".into(),
                    ));
                }
            }
        }
        let schedule = prefix
            .schedule
            .advanced(&certified.commitment.schedule)
            .map_err(|error| malformed(error.to_string()))?;
        prefix.tip = certified.committed.clone();
        prefix.schedule = schedule;
        prefix.authority = authority;
        Ok(certified)
    }

    fn verify_certificate(
        &self,
        committed: CommittedBlock,
        authority: &VerifiedAuthority,
        config: Option<&iroha_sumeragi::types::HeightConfig>,
    ) -> Result<CertifiedBlock, ChainReadError> {
        let height = committed.height;
        let malformed = |reason: String| ChainReadError::Malformed { height, reason };
        let certificate = committed
            .block
            .commit_certificate()
            .ok_or(ChainReadError::MissingCertificate { height })?;
        let certificate_len = norito::canonical_frame_len(certificate)
            .map_err(|error| malformed(error.to_string()))?;
        let Some(header) = committed.header.as_ref() else {
            return Ok(CertifiedBlock {
                committed,
                commit_qc: None,
                verification: QcVerification::Genesis,
                certificate_len,
            });
        };
        let commit_qc: Qc = norito::decode_canonical(certificate.commit_qc())
            .map_err(|error| malformed(error.to_string()))?;
        if header.epoch != authority.epoch
            || commit_qc.epoch != authority.epoch
            || height < authority.material.authorization.first_height
            || height > authority.material.authorization.last_height
            || (authority.material.mode == ConsensusMode::Npos
                && height == authority.material.authorization.last_height
                && !header.attest)
            || commit_qc.kind != VoteKind::Commit
            || commit_qc.height != height
            || commit_qc.block_hash != committed.core_hash
            || commit_qc.attest != header.attest
        {
            return Err(ChainReadError::HeaderMismatch { height });
        }
        if commit_qc.result != committed.result {
            return Err(ChainReadError::ResultMismatch { height });
        }
        if header.instance != self.instance || commit_qc.instance != self.instance {
            return Err(ChainReadError::WrongInstance { height });
        }
        let native = super::attestation::NativePastaVerifier::new(self.instance, self.network);
        let verifier = self.attestations.unwrap_or(&native);
        #[cfg(test)]
        relation_counts::qc(height);
        let checked = iroha_sumeragi::crypto::Verifier::new(
            &authority.crypto,
            &self.instance,
            &authority.epoch,
            &authority.committee,
        )
        .verify_qc(verifier, &commit_qc);
        checked.map_err(|error| ChainReadError::Certificate { height, error })?;
        // Parent-authenticated parameters and authority also bind the original signed row
        // table. A valid CommitQC alone does not certify possession of these payload bytes.
        let config = config.ok_or_else(|| {
            malformed("non-genesis certificate lacks parent-authenticated configuration".into())
        })?;
        verify_availability(&committed, config, &authority.crypto)?;
        Ok(CertifiedBlock {
            committed,
            commit_qc: Some(commit_qc),
            verification: QcVerification::Verified,
            certificate_len,
        })
    }
}

fn verify_availability(
    committed: &CommittedBlock,
    config: &iroha_sumeragi::types::HeightConfig,
    crypto: &BlsCrypto,
) -> Result<(), ChainReadError> {
    use iroha_sumeragi::availability::{AvailabilityFrame, MAX_AVAILABILITY_FRAME_BYTES};
    let height = committed.height;
    let malformed = |reason: String| ChainReadError::Malformed { height, reason };
    let header = committed
        .header
        .as_ref()
        .ok_or_else(|| malformed("availability requires a non-genesis header".into()))?;
    let certificate = committed
        .block
        .commit_certificate()
        .ok_or(ChainReadError::MissingCertificate { height })?;
    // Bounded read-only verification has no native allocation or custody authority.
    if certificate.availability().len() > MAX_AVAILABILITY_FRAME_BYTES.saturating_add(128)
        || header.payload_len as usize > 64 * 1024 * 1024
    {
        return Err(malformed(
            "availability exceeds certified reader bound".into(),
        ));
    }
    let table: AvailabilityFrame = norito::decode_canonical(certificate.availability())
        .map_err(|error| malformed(error.to_string()))?;
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(header.payload_len as usize)
        .map_err(|error| malformed(error.to_string()))?;
    committed
        .block
        .write_resultless_proposal_wire(&mut payload)
        .map_err(|error| malformed(error.to_string()))?;
    iroha_data_model::sumeragi_finality::verify_payload_availability(
        header.instance,
        config,
        header,
        &table,
        &payload,
        crypto,
    )
    .map_err(|error| malformed(error.to_string()))
}

fn make_genesis_prefix(
    tip: CommittedBlock,
    material: ValidatorEpochContextV1,
) -> Result<VerifiedPrefix, ChainReadError> {
    if tip.commitment.schedule.current != material {
        return Err(ChainReadError::Committee {
            height: GENESIS_HEIGHT,
            reason: "genesis result context differs from its signed body".into(),
        });
    }
    let schedule = schedule::ConsensusSchedule::from_genesis_outcome(&tip.commitment.schedule)
        .map_err(|error| ChainReadError::Committee {
            height: GENESIS_HEIGHT,
            reason: error.to_string(),
        })?;
    let authority = Arc::new(VerifiedAuthority::new(material, GENESIS_HEIGHT)?);
    Ok(VerifiedPrefix {
        tip,
        schedule,
        authority,
    })
}

/// Genesis execution authenticated by an actual verified height-two successor.
///
/// This move-only receipt is never decoded or publicly constructed. The genesis has no QC;
/// the successor's exact quorum authenticates its `parent_result` and complete predecessor.
#[derive(Debug)]
pub struct GenesisExecutionAnchor {
    committed: CommittedBlock,
    successor: Hash32,
}
impl GenesisExecutionAnchor {
    /// Original result-bearing genesis whose execution is authenticated by the successor.
    #[must_use]
    pub fn committed(&self) -> &CommittedBlock {
        &self.committed
    }
    /// Native hash of the independently verified successor that signs this result.
    #[must_use]
    pub const fn successor(&self) -> Hash32 {
        self.successor
    }
    /// Move the authenticated execution receipt into its consuming evidence owner.
    #[must_use]
    pub fn into_committed(self) -> CommittedBlock {
        self.committed
    }
}

/// One chronologically verified native successor and, exactly once at H2, its genesis anchor.
#[derive(Debug)]
pub struct CertifiedPrefixStep {
    current: CertifiedBlock,
    genesis: Option<GenesisExecutionAnchor>,
}
impl CertifiedPrefixStep {
    /// Consume both actual verification receipts without rebuilding or converting a proof.
    #[must_use]
    pub fn into_parts(self) -> (CertifiedBlock, Option<GenesisExecutionAnchor>) {
        (self.current, self.genesis)
    }
}

/// One-pass native certificate verification from independently pinned signed genesis.
///
/// The working authority is bounded: the original parent, current authority and lag-two
/// schedule only. Every supplied height is checked once by the same transition verifier as
/// [`CertifiedChain`]. There is no all-history committee cache or World authority lookup.
/// Callers must bound canonical frame allocations before supplying each decoded block.
/// Construction authenticates genesis header/body and its epoch only. No genesis execution
/// receipt is exported until a real verified H2 binds its result through `parent_result`.
pub struct CertifiedPrefix {
    network: NetworkId,
    instance: Hash32,
    prefix: VerifiedPrefix,
}
impl CertifiedPrefix {
    /// Start with the exact configured chain identity and independently pinned genesis network.
    ///
    /// # Errors
    /// Rejects foreign or unsigned genesis, an invalid complete epoch, or a malformed result
    /// frame. A well-formed genesis result is still untrusted until the first successor.
    pub fn new(
        chain_id: &ChainId,
        network: NetworkId,
        genesis: Arc<SignedBlock>,
    ) -> Result<Self, ChainReadError> {
        let (epoch, instance) = authenticate_genesis(&genesis, &network, chain_id)?;
        let tip = read_frame(genesis, GENESIS_HEIGHT)?;
        Ok(Self {
            network,
            instance,
            prefix: make_genesis_prefix(tip, epoch)?,
        })
    }

    /// The native chain instance bound by every subsequent signature.
    #[must_use]
    pub const fn instance(&self) -> Hash32 {
        self.instance
    }

    /// Verify the exact next canonical carrier with full BLS and native paired-Pasta checks.
    ///
    /// # Errors
    /// Rejects changed/skipped parents, result/context substitutions, malformed certificates,
    /// invalid signatures, incomplete authority, boundary decisions or beacon source links.
    /// A rejected frame does not advance the original verified cursor.
    pub fn push(&mut self, block: Arc<SignedBlock>) -> Result<CertifiedPrefixStep, ChainReadError> {
        let height = block.header().height().get();
        if self.prefix.tip.height.checked_add(1) != Some(height) {
            return Err(ChainReadError::Discontinuous { height });
        }
        let committed = read_frame(block, height)?;
        let genesis = (self.prefix.tip.height == GENESIS_HEIGHT).then(|| self.prefix.tip.clone());
        let current = PrefixVerifierContext {
            instance: self.instance,
            network: self.network,
            attestations: None,
        }
        .advance_prefix(&mut self.prefix, committed)?;
        let genesis = genesis.map(|committed| GenesisExecutionAnchor {
            committed,
            successor: current.core_hash,
        });
        Ok(CertifiedPrefixStep { current, genesis })
    }
}

fn authenticate_genesis(
    genesis: &SignedBlock,
    network: &NetworkId,
    chain_id: &ChainId,
) -> Result<(ValidatorEpochContextV1, Hash32), ChainReadError> {
    if genesis.hash().as_ref() != network.as_bytes()
        || !genesis.header().is_genesis()
        || genesis.validate_proposal_commitments().is_err()
    {
        return Err(ChainReadError::ForeignGenesis);
    }
    let epoch = super::epoch::genesis_epoch(genesis).map_err(|_| ChainReadError::ForeignGenesis)?;
    Ok((epoch, global_instance(genesis, &chain_id.to_string())))
}

/// The exact source cut being verified. Pinned restoration never supplies a World or roster.
enum ChainSource<'v, V: StateReadOnly + ?Sized> {
    State(&'v V),
    Frames {
        chain_id: &'v ChainId,
        network: &'v NetworkId,
        hashes: &'v [HashOf<IrohaHeader>],
        frames: &'v [Arc<SignedBlock>],
    },
    Pinned {
        chain_id: &'v ChainId,
        network: &'v NetworkId,
        hashes: &'v [HashOf<IrohaHeader>],
        kura: &'v Kura,
    },
}
impl<V: StateReadOnly + ?Sized> ChainSource<'_, V> {
    fn network_id(&self) -> &NetworkId {
        match self {
            Self::State(view) => view.network_id(),
            Self::Pinned { network, .. } | Self::Frames { network, .. } => network,
        }
    }
    fn chain_id(&self) -> &ChainId {
        match self {
            Self::State(view) => view.chain_id(),
            Self::Pinned { chain_id, .. } | Self::Frames { chain_id, .. } => chain_id,
        }
    }
    fn block(&self, height: u64) -> Result<Arc<SignedBlock>, ChainReadError> {
        let index = usize::try_from(height)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or(ChainReadError::NotCommitted { height })?;
        match self {
            Self::State(view) => {
                if index.get() > view.block_hashes().len() {
                    return Err(ChainReadError::NotCommitted { height });
                }
                view.canonical_block_by_height(index)
                    .map_err(|_| ChainReadError::NotInView { height })
            }
            Self::Frames { hashes, frames, .. } => {
                let expected = hashes
                    .get(index.get() - 1)
                    .ok_or(ChainReadError::NotCommitted { height })?;
                let block = frames
                    .get(index.get() - 1)
                    .ok_or(ChainReadError::NotInView { height })?;
                if block.hash() != *expected || block.header().height().get() != height {
                    return Err(ChainReadError::NotInView { height });
                }
                Ok(Arc::clone(block))
            }
            Self::Pinned { hashes, kura, .. } => {
                let expected = hashes
                    .get(index.get() - 1)
                    .ok_or(ChainReadError::NotCommitted { height })?;
                if kura.is_canonical_body_missing(index) {
                    return Err(ChainReadError::NotInView { height });
                }
                let block = kura
                    .get_block(index)
                    .ok_or(ChainReadError::NotInView { height })?;
                if block.hash() != *expected || block.header().height().get() != height {
                    return Err(ChainReadError::NotInView { height });
                }
                Ok(block)
            }
        }
    }
}

/// Certified history over one immutable State view or explicit pinned restoration cut.
pub struct CertifiedChain<'v, V: StateReadOnly + ?Sized> {
    source: ChainSource<'v, V>,
    genesis: Arc<SignedBlock>,
    genesis_epoch: ValidatorEpochContextV1,
    instance: Hash32,
    attestations: Option<&'v dyn AttestationVerifier>,
    prefix: parking_lot::Mutex<Option<VerifiedPrefix>>,
}

impl<V: StateReadOnly + ?Sized> core::fmt::Debug for CertifiedChain<'_, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CertifiedChain")
            .field("instance", &self.instance)
            .finish_non_exhaustive()
    }
}

impl<'v, V: StateReadOnly + ?Sized> CertifiedChain<'v, V> {
    /// A reader over `view` with exact BLS quorum and native paired-Pasta attestation checks.
    ///
    /// # Errors
    /// The view has no genesis, or Kura's genesis is not the view's network genesis.
    pub fn new(view: &'v V) -> Result<Self, ChainReadError> {
        Self::from_source(ChainSource::State(view))
    }

    fn from_source(source: ChainSource<'v, V>) -> Result<Self, ChainReadError> {
        let genesis = source.block(GENESIS_HEIGHT)?;
        let (genesis_epoch, instance) =
            authenticate_genesis(&genesis, source.network_id(), source.chain_id())?;
        Ok(Self {
            source,
            genesis,
            genesis_epoch,
            instance,
            attestations: None,
            prefix: parking_lot::Mutex::new(None),
        })
    }

    /// Verify `CommitQC`s fully with `verifier`, including the attestations of flagged blocks
    /// (F8 rule H6 (a)). Any previously verified prefix is discarded so the next read checks
    /// every historical certificate under this verifier too.
    #[must_use]
    pub fn with_attestation_verifier(mut self, verifier: &'v dyn AttestationVerifier) -> Self {
        self.attestations = Some(verifier);
        *self.prefix.get_mut() = None;
        self
    }

    fn verification_context(&self) -> PrefixVerifierContext<'_> {
        PrefixVerifierContext {
            instance: self.instance,
            network: *self.source.network_id(),
            attestations: self.attestations,
        }
    }

    /// The chain instance `I` every certificate must name.
    #[must_use]
    pub fn instance(&self) -> Hash32 {
        self.instance
    }

    /// The signed genesis body (the chain's trust root). Its result-only certificate was added
    /// after execution and is not authenticated by the genesis signatures.
    #[must_use]
    pub fn genesis(&self) -> &Arc<SignedBlock> {
        &self.genesis
    }

    /// The source-pinned receipt of `height` ([`committed_block`]).
    /// This method checks the frame and source identity, not local quorum signatures. In the
    /// pinned restoration mode, consume [`Self::certified`] or [`Self::walk`] before trusting
    /// execution or epoch progress; a pinned header hash alone does not authenticate those.
    ///
    /// # Errors
    /// See [`committed_block`].
    pub fn committed(&self, height: u64) -> Result<CommittedBlock, ChainReadError> {
        match &self.source {
            ChainSource::State(view) => committed_block(*view, height),
            ChainSource::Pinned { .. } | ChainSource::Frames { .. } => {
                read_frame(self.source.block(height)?, height)
            }
        }
    }

    /// The committed block at `height` with its local `CommitQC` checked (see the module
    /// documentation).
    ///
    /// # Errors
    /// The committed read fails, the certificate does not certify the stored header and result,
    /// names another instance, or does not verify under the committee of its height.
    pub fn certified(&self, height: u64) -> Result<CertifiedBlock, ChainReadError> {
        self.check_certificate(read_frame(self.source.block(height)?, height)?)
    }

    /// The exact authenticated epoch committee and its original proofs of possession.
    /// The complete prefix authenticates historical authority even after World rotates it out.
    ///
    /// # Errors
    /// The signed genesis or certified prefix does not authenticate this height's authority.
    pub fn proof_committee(
        &self,
        height: u64,
    ) -> Result<Vec<(IrohaPublicKey, Vec<u8>)>, ChainReadError> {
        let certified = self.certified(height)?;
        Ok(certified
            .commitment
            .schedule
            .current
            .committee
            .iter()
            .map(|member| {
                (
                    member.validator.public_key().clone(),
                    member.proof_of_possession.clone(),
                )
            })
            .collect())
    }

    /// The certified blocks `from..=to`, oldest first, each checked to extend the previous one.
    /// The iterator stops after the first error.
    pub fn walk(
        &self,
        from: u64,
        to: u64,
    ) -> impl Iterator<Item = Result<CertifiedBlock, ChainReadError>> + '_ {
        let mut parent: Option<CommittedBlock> = None;
        let mut failed = false;
        (from..=to).map_while(move |height| {
            if failed {
                return None;
            }
            let read = self.certified(height).and_then(|block| match &parent {
                Some(parent) if !block.extends(parent) => {
                    Err(ChainReadError::Discontinuous { height })
                }
                _ => Ok(block),
            });
            match &read {
                Ok(block) => parent = Some(block.committed.clone()),
                Err(_) => failed = true,
            }
            Some(read)
        })
    }

    /// Derive authority exclusively from signed genesis, then check the result graph against it.
    /// The graph's execution/parameter data is not independently final until a successor signs Rg.
    fn genesis_prefix(&self) -> Result<VerifiedPrefix, ChainReadError> {
        make_genesis_prefix(
            read_frame(self.source.block(GENESIS_HEIGHT)?, GENESIS_HEIGHT)?,
            self.genesis_epoch.clone(),
        )
    }

    /// Verify the complete prefix with a bounded working set. Sequential reads reuse its
    /// cursor; an earlier-height read restarts at genesis instead of trusting an unbounded cache.
    fn check_certificate(
        &self,
        committed: CommittedBlock,
    ) -> Result<CertifiedBlock, ChainReadError> {
        let height = committed.height;
        let mut cursor = self.prefix.lock();
        if cursor
            .as_ref()
            .is_none_or(|prefix| prefix.tip.height >= height)
        {
            *cursor = Some(self.genesis_prefix()?);
        }
        let prefix = cursor.as_mut().ok_or(ChainReadError::ForeignGenesis)?;
        if height == GENESIS_HEIGHT {
            if committed.core_hash != prefix.tip.core_hash || committed.result != prefix.tip.result
            {
                return Err(ChainReadError::ForeignGenesis);
            }
            return self.verification_context().verify_certificate(
                committed,
                &prefix.authority,
                None,
            );
        }
        while prefix
            .tip
            .height
            .checked_add(1)
            .is_some_and(|next| next < height)
        {
            let next_height = prefix.tip.height + 1;
            let next = read_frame(self.source.block(next_height)?, next_height)?;
            self.verification_context().advance_prefix(prefix, next)?;
        }
        self.verification_context()
            .advance_prefix(prefix, committed)
    }
}

impl<'v> CertifiedChain<'v, StateView<'v>> {
    /// Read a bounded, externally pinned, contiguous journal of canonical native frames.
    ///
    /// The caller must apply its explicit byte/allocation limits before decoding the frames.
    /// This constructor borrows that one source; it neither clones block bodies nor builds
    /// another storage owner. `network` must be independently configured, never taken from the
    /// supplied journal. It pins signed genesis; `chain_id` pins every successor's instance.
    /// The supplied hash cut must be exactly coextensive with the frames. A hash alone does not
    /// authenticate a result: consume `certified`/`walk`. Flagged certificates use the full
    /// source-complete native paired-Pasta verifier by default. An explicitly supplied
    /// application/test attestation verifier replaces that check for this reader only. H1 has
    /// only signed-body authority until a genuine successor or independent local execution
    /// authenticates its result.
    ///
    /// # Errors
    /// Rejects an empty or mismatched cut, foreign genesis, or malformed signed genesis.
    /// Subsequent reads reject mismatched bodies, parent links, results, epochs and certificates.
    pub fn from_frames(
        chain_id: &'v ChainId,
        network: &'v NetworkId,
        hashes: &'v [HashOf<IrohaHeader>],
        frames: &'v [Arc<SignedBlock>],
    ) -> Result<Self, ChainReadError> {
        if hashes.len() != frames.len() {
            return Err(ChainReadError::NotInView {
                height: GENESIS_HEIGHT,
            });
        }
        Self::from_source(ChainSource::Frames {
            chain_id,
            network,
            hashes,
            frames,
        })
    }

    /// Verify an exact pinned history cut before a restored State exists.
    ///
    /// No StateView is constructed: that concrete generic type only selects this constructor.
    /// Every body must match the supplied one-based hash cut. Authority comes from verified
    /// signed genesis and the same exact-quorum prefix walk as State-backed offchain reads.
    /// The configured chain ID enters the signature instance checked by every successor QC.
    ///
    /// This is a restoration/offchain boundary. Deterministic instructions must continue using
    /// State-anchored [`committed_block`] and must not depend on local QC availability. A lone
    /// genesis returns only [`QcVerification::Genesis`]: its unsigned result preimage still
    /// requires deterministic replay or an actually verified successor to authenticate execution.
    ///
    /// # Errors
    /// Rejects an empty cut, absent/mismatched genesis body or foreign signed genesis network.
    /// Later reads reject missing bodies, changed pinned hashes, wrong instances and certificates.
    pub(crate) fn from_pinned(
        chain_id: &'v ChainId,
        network: &'v NetworkId,
        hashes: &'v [HashOf<IrohaHeader>],
        kura: &'v Kura,
    ) -> Result<Self, ChainReadError> {
        Self::from_source(ChainSource::Pinned {
            chain_id,
            network,
            hashes,
            kura,
        })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn schedule_source_projection_preserves_every_recovery_variant() {
    use iroha_data_model::sumeragi_finality::{ScheduleError, ScheduleSourceError};
    let variants = [
        (
            ChainReadError::NotCommitted { height: 19 },
            ScheduleSourceError::NotCommitted { height: 19 },
        ),
        (
            ChainReadError::NotInView { height: 19 },
            ScheduleSourceError::NotInView { height: 19 },
        ),
        (
            ChainReadError::MissingCertificate { height: 19 },
            ScheduleSourceError::MissingCertificate { height: 19 },
        ),
        (
            ChainReadError::Malformed {
                height: 19,
                reason: "exact source reason".into(),
            },
            ScheduleSourceError::Malformed {
                height: 19,
                reason: "exact source reason".into(),
            },
        ),
        (
            ChainReadError::HeaderMismatch { height: 19 },
            ScheduleSourceError::HeaderMismatch { height: 19 },
        ),
        (
            ChainReadError::ResultMismatch { height: 19 },
            ScheduleSourceError::ResultMismatch { height: 19 },
        ),
        (
            ChainReadError::ExecutionMismatch { height: 19 },
            ScheduleSourceError::ExecutionMismatch { height: 19 },
        ),
        (
            ChainReadError::WrongInstance { height: 19 },
            ScheduleSourceError::WrongInstance { height: 19 },
        ),
        (
            ChainReadError::Discontinuous { height: 19 },
            ScheduleSourceError::Discontinuous { height: 19 },
        ),
        (
            ChainReadError::ForeignGenesis,
            ScheduleSourceError::ForeignGenesis,
        ),
        (
            ChainReadError::Committee {
                height: 19,
                reason: "exact source reason".into(),
            },
            ScheduleSourceError::Committee {
                height: 19,
                reason: "exact source reason".into(),
            },
        ),
        (
            ChainReadError::Certificate {
                height: 19,
                error: CertError::WrongEpoch,
            },
            ScheduleSourceError::Certificate {
                height: 19,
                error: CertError::WrongEpoch,
            },
        ),
    ];
    for (source, expected) in variants {
        assert_eq!(source.to_string(), expected.to_string());
        assert_eq!(ScheduleSourceError::from(source.clone()), expected);
        assert_eq!(
            ScheduleError::from(source),
            ScheduleError::CommittedSource(expected)
        );
    }
}

mod execution_read;
pub use execution_read::{
    AuthenticatedExecutionBlock, NativeExecutionRead, NativeExecutionReadError,
    NativeExecutionReadLimits, NativeExecutionReadResource, read_authenticated_execution,
};

#[cfg(test)]
pub(crate) mod relation_counts;
