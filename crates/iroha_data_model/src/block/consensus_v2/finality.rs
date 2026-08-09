//! Durable, versioned finality records for canonical Sumeragi v2 blocks.
//!
//! These values are structural persistence records. Their validation checks
//! every redundant protocol, context, height, subject, and block-hash binding,
//! but deliberately leaves aggregate-signature verification to the consensus
//! cryptography adapter.

use core::fmt;
use std::vec::Vec;

use iroha_crypto::{Algorithm, HashOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use super::{
    BlockSubject, ConsensusMode, DualQuorum, GlobalPhase, Height, HeightContext, HeightContextId,
    PROTOCOL_VERSION, QuorumCertificate, ValidationError, ValidatorPower, Vote,
};
use crate::block::BlockHeader;

/// Current Norito layout version of [`V2FinalityArtifact`].
pub const V2_FINALITY_ARTIFACT_VERSION: u16 = 4;
/// Maximum encoded BLS proof-of-possession bytes retained per validator.
pub const MAX_VALIDATOR_POP_BYTES: usize = 256;

/// Election inputs finalized for the epoch immediately following a block.
///
/// The snapshot is optional because most blocks do not finalize an epoch
/// transition. When present, it carries the complete ordered equal-vote
/// roster rather than a reference to mutable world state.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct FinalizedNextEpochSnapshot {
    /// Epoch immediately following the artifact's height context epoch.
    pub epoch: u64,
    /// Last height governed by the next epoch.
    pub epoch_end_height: Height,
    /// Genesis-selected consensus mode used to select the committee.
    pub mode: ConsensusMode,
    /// Canonically ordered voting roster; every entry has one consensus vote.
    pub roster: Vec<ValidatorPower>,
    /// BLS proofs of possession aligned exactly with `roster`.
    pub validator_set_pops: Vec<Vec<u8>>,
    /// Canonical `2f + 1` quorum derived from `roster`.
    pub quorum: DualQuorum,
    /// Finalized seed used for deterministic leader rotation in this epoch.
    pub leader_seed: [u8; 32],
}

impl FinalizedNextEpochSnapshot {
    pub(super) fn validate_against(&self, context: &HeightContext) -> Result<(), ValidationError> {
        let expected_epoch = context
            .epoch
            .checked_add(1)
            .ok_or(ValidationError::InvalidNextEpoch)?;
        if self.epoch != expected_epoch {
            return Err(ValidationError::InvalidNextEpoch);
        }
        let successor_height = context
            .height
            .checked_add(1)
            .ok_or(ValidationError::NextEpochEndsBeforeSuccessor)?;
        if self.epoch_end_height < successor_height {
            return Err(ValidationError::NextEpochEndsBeforeSuccessor);
        }
        if self.mode != context.mode {
            return Err(ValidationError::NextEpochModeMismatch);
        }
        let canonical = DualQuorum::from_roster(&self.roster)?;
        if self.quorum != canonical {
            return Err(ValidationError::NextEpochQuorumMismatch);
        }
        if self.validator_set_pops.len() != self.roster.len() {
            return Err(ValidationError::NextEpochProofOfPossessionCount);
        }
        if self.validator_set_pops.iter().any(Vec::is_empty) {
            return Err(ValidationError::MissingNextEpochProofOfPossession);
        }
        if self
            .validator_set_pops
            .iter()
            .any(|proof| proof.len() > MAX_VALIDATOR_POP_BYTES)
        {
            return Err(ValidationError::NextEpochProofOfPossessionTooLarge);
        }
        if self.roster.iter().any(|validator| validator.power != 1) {
            return Err(ValidationError::NextEpochVotingPowerNotOne);
        }
        Ok(())
    }
}

/// Canonical, versioned finality evidence persisted alongside one block.
///
/// Redundant fields are intentional: they make a malformed or mis-associated
/// sidecar fail structural validation before any signature verification or
/// state replay is attempted.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct V2FinalityArtifact {
    /// Norito layout version; currently [`V2_FINALITY_ARTIFACT_VERSION`].
    pub format_version: u16,
    /// Consensus protocol revision; must equal [`PROTOCOL_VERSION`].
    pub protocol_version: u16,
    /// Block height repeated for file association and early rejection.
    pub height: Height,
    /// Complete immutable consensus context governing this height.
    pub height_context: HeightContext,
    /// Exact subject certified by the `CommitQC`.
    pub subject: BlockSubject,
    /// Canonical block header hash, repeated from `subject`.
    pub block_hash: HashOf<BlockHeader>,
    /// Commit certificate that finalized `subject`.
    pub commit_qc: QuorumCertificate,
    /// Durable BLS proofs of possession in the exact frozen-roster order.
    ///
    /// Keeping this material beside the certificate makes historical finality
    /// independently verifiable after validator keys rotate or retire from
    /// mutable world state.
    pub validator_set_pops: Vec<Vec<u8>>,
}

impl V2FinalityArtifact {
    /// Construct the canonical current-layout artifact.
    #[must_use]
    pub fn new(
        height_context: HeightContext,
        subject: BlockSubject,
        commit_qc: QuorumCertificate,
        validator_set_pops: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            format_version: V2_FINALITY_ARTIFACT_VERSION,
            protocol_version: PROTOCOL_VERSION,
            height: height_context.height,
            height_context,
            subject,
            block_hash: subject.block_hash,
            commit_qc,
            validator_set_pops,
        }
    }

    /// Return the frozen context identifier authenticated by this artifact.
    #[must_use]
    pub fn context_id(&self) -> HeightContextId {
        self.height_context.id()
    }

    /// Validate all structural bindings within this artifact.
    ///
    /// This validates certificate quorum structure and signature presence, but
    /// does not cryptographically verify the aggregate signature.
    ///
    /// # Errors
    ///
    /// Returns an error when any version, context, height, phase, subject,
    /// block-hash, quorum, or next-epoch binding is inconsistent.
    pub fn validate(&self) -> Result<(), V2FinalityValidationError> {
        if self.format_version != V2_FINALITY_ARTIFACT_VERSION {
            return Err(V2FinalityValidationError::UnsupportedFormatVersion {
                expected: V2_FINALITY_ARTIFACT_VERSION,
                actual: self.format_version,
            });
        }
        if self.protocol_version != PROTOCOL_VERSION
            || self.height_context.protocol_version != self.protocol_version
        {
            return Err(V2FinalityValidationError::ProtocolVersionMismatch {
                expected: PROTOCOL_VERSION,
                artifact: self.protocol_version,
                context: self.height_context.protocol_version,
            });
        }
        self.height_context
            .validate()
            .map_err(V2FinalityValidationError::InvalidHeightContext)?;
        if self.height != self.height_context.height {
            return Err(V2FinalityValidationError::HeightContextMismatch {
                artifact: self.height,
                context: self.height_context.height,
            });
        }
        if self.commit_qc.round.context_id != self.height_context.id() {
            return Err(V2FinalityValidationError::CertificateContextMismatch);
        }
        if self.commit_qc.round.height != self.height {
            return Err(V2FinalityValidationError::CertificateHeightMismatch {
                artifact: self.height,
                certificate: self.commit_qc.round.height,
            });
        }
        if self.commit_qc.phase != GlobalPhase::Commit {
            return Err(V2FinalityValidationError::CertificateIsNotCommit);
        }
        if self.commit_qc.subject != self.subject {
            return Err(V2FinalityValidationError::CertificateSubjectMismatch);
        }
        if self.block_hash != self.subject.block_hash {
            return Err(V2FinalityValidationError::SubjectBlockHashMismatch);
        }
        let expected_parent = self
            .height_context
            .parent_commit_qc
            .as_ref()
            .map(|parent| parent.subject.block_hash)
            .or_else(|| {
                self.height_context
                    .snapshot_bootstrap
                    .as_ref()
                    .map(|anchor| anchor.snapshot_block_hash)
            });
        if self.subject.parent_block_hash != expected_parent {
            return Err(V2FinalityValidationError::ParentBlockHashMismatch);
        }
        self.commit_qc
            .validate(&self.height_context)
            .map_err(V2FinalityValidationError::InvalidCommitCertificate)?;
        if self.validator_set_pops.len() != self.height_context.roster.len() {
            return Err(V2FinalityValidationError::ProofOfPossessionCount {
                expected: self.height_context.roster.len(),
                actual: self.validator_set_pops.len(),
            });
        }
        if let Some(index) = self.validator_set_pops.iter().position(Vec::is_empty) {
            return Err(V2FinalityValidationError::MissingProofOfPossession {
                index: u32::try_from(index).unwrap_or(u32::MAX),
            });
        }
        if let Some((index, proof)) = self
            .validator_set_pops
            .iter()
            .enumerate()
            .find(|(_, proof)| proof.len() > MAX_VALIDATOR_POP_BYTES)
        {
            return Err(V2FinalityValidationError::ProofOfPossessionTooLarge {
                index: u32::try_from(index).unwrap_or(u32::MAX),
                actual: proof.len(),
                max: MAX_VALIDATOR_POP_BYTES,
            });
        }
        Ok(())
    }

    /// Validate this artifact against an externally selected canonical block.
    ///
    /// # Errors
    ///
    /// Returns an error when internal validation fails or the selected block's
    /// height or header hash differs from the finalized subject.
    pub fn validate_for_block(
        &self,
        height: Height,
        block_hash: HashOf<BlockHeader>,
    ) -> Result<(), V2FinalityValidationError> {
        self.validate()?;
        if self.height != height {
            return Err(V2FinalityValidationError::AssociatedHeightMismatch {
                artifact: self.height,
                block: height,
            });
        }
        if self.block_hash != block_hash {
            return Err(V2FinalityValidationError::AssociatedBlockHashMismatch);
        }
        Ok(())
    }

    /// Validate this artifact against a complete canonical block header.
    ///
    /// In addition to height and header-hash association, this binds the
    /// header's predecessor to the exact `CommitQC` subject and requires the
    /// authenticated decision round to cover the header's body-construction
    /// view. An unchanged body may be re-proposed and
    /// certified in a later round, but a certificate can never precede the
    /// view committed inside the block header.
    ///
    /// # Errors
    ///
    /// Returns an error when internal validation fails or any height, hash,
    /// predecessor, or decision/header-view ordering differs from the supplied header.
    pub fn validate_for_header(
        &self,
        header: &BlockHeader,
    ) -> Result<(), V2FinalityValidationError> {
        self.validate_for_block(header.height().get(), header.hash())?;
        if self.subject.parent_block_hash != header.prev_block_hash() {
            return Err(V2FinalityValidationError::AssociatedParentBlockHashMismatch);
        }
        if !self.decision_round_covers_header_view(header) {
            return Err(V2FinalityValidationError::DecisionRoundBeforeBlockView {
                decision: self.commit_qc.round.view,
                block: header.view_change_index(),
            });
        }
        Ok(())
    }

    /// Return whether the authenticated decision round covers the immutable
    /// body-construction view carried by a complete block header.
    #[must_use]
    pub(crate) fn decision_round_covers_header_view(&self, header: &BlockHeader) -> bool {
        self.commit_qc.round.view >= header.view_change_index()
    }

    /// Verify the artifact's commit certificate against its frozen roster and
    /// roster-aligned BLS proofs of possession.
    ///
    /// This is the canonical cryptographic verification boundary shared by
    /// consensus recovery, bridge proof construction, and light-client proof
    /// verification. Structural validation, the exact equal-vote quorum, every
    /// signer index, every roster proof of possession, and the aggregate
    /// signature are checked over the exact Sumeragi-v2 vote preimage.
    ///
    /// # Errors
    ///
    /// Returns an error when the artifact is structurally invalid, the proofs
    /// of possession are not aligned with its frozen roster, or BLS
    /// verification fails.
    pub fn verify(&self) -> Result<(), V2QuorumCertificateVerificationError> {
        self.validate()
            .map_err(V2QuorumCertificateVerificationError::InvalidArtifact)?;
        // Authenticate the context (which commits any future snapshot) before
        // doing full-roster future-PoP work. The certificate path already
        // verifies every signer PoP; only the current non-signers remain.
        verify_quorum_certificate_with_validator_pops(
            &self.height_context,
            &self.commit_qc,
            &self.validator_set_pops,
        )?;
        verify_validator_power_roster_pops_except(
            &self.height_context.roster,
            &self.validator_set_pops,
            &self.commit_qc.signers,
        )?;
        if let Some(snapshot) = &self.height_context.next_epoch_snapshot {
            verify_validator_power_roster_pops(&snapshot.roster, &snapshot.validator_set_pops)?;
        }
        Ok(())
    }
}

/// Cryptographic verification failure for one Sumeragi-v2 quorum certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum V2QuorumCertificateVerificationError {
    /// The enclosing finality artifact failed its structural bindings.
    #[error("invalid Sumeragi-v2 finality artifact: {0}")]
    InvalidArtifact(V2FinalityValidationError),
    /// The quorum certificate is not structurally valid under its frozen context.
    #[error("invalid Sumeragi-v2 quorum certificate: {0}")]
    InvalidCertificate(ValidationError),
    /// The supplied `PoP` vector is not aligned with the frozen voting roster.
    #[error(
        "Sumeragi-v2 proof-of-possession count {actual} does not match roster length {expected}"
    )]
    ProofOfPossessionCount {
        /// Frozen voting-roster length.
        expected: usize,
        /// Supplied proof count.
        actual: usize,
    },
    /// A frozen validator key is malformed.
    #[error("Sumeragi-v2 validator key at index {index} is malformed")]
    MalformedValidatorPublicKey {
        /// Index in the frozen voting roster.
        index: u32,
    },
    /// A frozen validator does not use BLS-normal.
    #[error("Sumeragi-v2 validator key at index {index} uses {algorithm:?}; expected BLS-normal")]
    InvalidValidatorKeyAlgorithm {
        /// Index in the frozen voting roster.
        index: u32,
        /// Advertised key algorithm.
        algorithm: Algorithm,
    },
    /// A selected validator's proof of possession is invalid.
    #[error("invalid Sumeragi-v2 proof of possession at roster index {index}")]
    InvalidProofOfPossession {
        /// Index in the frozen voting roster.
        index: u32,
    },
    /// The BLS aggregate signature does not authenticate the exact v2 vote preimage.
    #[error("invalid Sumeragi-v2 quorum-certificate aggregate signature")]
    InvalidAggregateSignature,
}

/// Verify a Sumeragi-v2 quorum certificate against its immutable height context
/// and roster-aligned BLS proofs of possession.
///
/// The certificate's own validation enforces strictly ordered, in-range
/// signers plus the `2f + 1` threshold under a unit-vote `3f + 1` roster.
/// This helper then verifies the selected BLS keys, `PoPs`,
/// and aggregate signature over [`QuorumCertificate::signer_preimage`].
///
/// # Errors
///
/// Returns a typed error for malformed context/certificate bindings, `PoP`
/// misalignment, unsupported keys, or invalid cryptography.
pub fn verify_quorum_certificate_with_validator_pops(
    context: &HeightContext,
    certificate: &QuorumCertificate,
    validator_set_pops: &[Vec<u8>],
) -> Result<(), V2QuorumCertificateVerificationError> {
    certificate
        .validate(context)
        .map_err(V2QuorumCertificateVerificationError::InvalidCertificate)?;
    if validator_set_pops.len() != context.roster.len() {
        return Err(
            V2QuorumCertificateVerificationError::ProofOfPossessionCount {
                expected: context.roster.len(),
                actual: validator_set_pops.len(),
            },
        );
    }
    let first_signer = certificate.signers.first().copied().ok_or(
        V2QuorumCertificateVerificationError::InvalidCertificate(
            ValidationError::InsufficientSignerCount,
        ),
    )?;
    // The certificate was validated above. Rebuilding the signer-independent
    // vote preimage directly avoids a second full context validation/hash.
    let preimage = Vote {
        round: certificate.round,
        proposal_round: certificate.proposal_round,
        phase: certificate.phase,
        subject: certificate.subject,
        execution_commitment: certificate.execution_commitment,
        signer: first_signer,
        signature: Vec::new(),
    }
    .signature_preimage();
    let mut public_keys = Vec::with_capacity(certificate.signers.len());
    let mut pops = Vec::with_capacity(certificate.signers.len());
    for signer in &certificate.signers {
        let index = usize::try_from(*signer).map_err(|_| {
            V2QuorumCertificateVerificationError::InvalidCertificate(
                ValidationError::SignerOutOfRange,
            )
        })?;
        let entry = context.roster.get(index).ok_or(
            V2QuorumCertificateVerificationError::InvalidCertificate(
                ValidationError::SignerOutOfRange,
            ),
        )?;
        let algorithm = entry.validator.public_key().try_algorithm().map_err(|_| {
            V2QuorumCertificateVerificationError::MalformedValidatorPublicKey { index: *signer }
        })?;
        if algorithm != Algorithm::BlsNormal {
            return Err(
                V2QuorumCertificateVerificationError::InvalidValidatorKeyAlgorithm {
                    index: *signer,
                    algorithm,
                },
            );
        }
        public_keys.push(entry.validator.public_key());
        pops.push(validator_set_pops[index].as_slice());
    }

    for (signer, (public_key, pop)) in certificate
        .signers
        .iter()
        .zip(public_keys.iter().zip(pops.iter()))
    {
        iroha_crypto::bls_normal_pop_verify(public_key, pop).map_err(|_| {
            V2QuorumCertificateVerificationError::InvalidProofOfPossession { index: *signer }
        })?;
    }
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &certificate.aggregate_signature,
        &public_keys,
        &pops,
    )
    .map_err(|_| V2QuorumCertificateVerificationError::InvalidAggregateSignature)
}

/// Verify every BLS key and proof of possession in one frozen v2 roster.
///
/// # Errors
///
/// Returns a typed error when the `PoP` vector is misaligned, a validator does
/// not use BLS-normal, or any proof of possession is invalid.
pub fn verify_validator_roster_pops(
    context: &HeightContext,
    validator_set_pops: &[Vec<u8>],
) -> Result<(), V2QuorumCertificateVerificationError> {
    verify_validator_power_roster_pops(&context.roster, validator_set_pops)
}

/// Verify BLS keys and proofs for one canonical powered roster.
///
/// # Errors
///
/// Returns a typed error for count drift, malformed or non-BLS keys, or an
/// invalid proof of possession.
pub fn verify_validator_power_roster_pops(
    roster: &[ValidatorPower],
    validator_set_pops: &[Vec<u8>],
) -> Result<(), V2QuorumCertificateVerificationError> {
    verify_validator_power_roster_pops_except(roster, validator_set_pops, &[])
}

fn verify_validator_power_roster_pops_except(
    roster: &[ValidatorPower],
    validator_set_pops: &[Vec<u8>],
    excluded_indices: &[u32],
) -> Result<(), V2QuorumCertificateVerificationError> {
    if validator_set_pops.len() != roster.len() {
        return Err(
            V2QuorumCertificateVerificationError::ProofOfPossessionCount {
                expected: roster.len(),
                actual: validator_set_pops.len(),
            },
        );
    }
    for (index, (entry, pop)) in roster.iter().zip(validator_set_pops).enumerate() {
        let index = u32::try_from(index).map_err(|_| {
            V2QuorumCertificateVerificationError::InvalidCertificate(
                ValidationError::RosterTooLarge,
            )
        })?;
        if excluded_indices.binary_search(&index).is_ok() {
            continue;
        }
        let algorithm = entry.validator.public_key().try_algorithm().map_err(|_| {
            V2QuorumCertificateVerificationError::MalformedValidatorPublicKey { index }
        })?;
        if algorithm != Algorithm::BlsNormal {
            return Err(
                V2QuorumCertificateVerificationError::InvalidValidatorKeyAlgorithm {
                    index,
                    algorithm,
                },
            );
        }
        iroha_crypto::bls_normal_pop_verify(entry.validator.public_key(), pop).map_err(|_| {
            V2QuorumCertificateVerificationError::InvalidProofOfPossession { index }
        })?;
    }
    Ok(())
}

/// Structural validation failure for a [`V2FinalityArtifact`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V2FinalityValidationError {
    /// The persisted Norito layout version is unsupported.
    UnsupportedFormatVersion {
        /// Supported layout version.
        expected: u16,
        /// Persisted layout version.
        actual: u16,
    },
    /// Artifact and embedded context protocol revisions are inconsistent.
    ProtocolVersionMismatch {
        /// Required protocol version.
        expected: u16,
        /// Artifact protocol version.
        artifact: u16,
        /// Embedded height-context protocol version.
        context: u16,
    },
    /// The embedded height context is structurally invalid.
    InvalidHeightContext(ValidationError),
    /// Artifact height differs from its embedded context height.
    HeightContextMismatch {
        /// Artifact height.
        artifact: Height,
        /// Embedded context height.
        context: Height,
    },
    /// `CommitQC` context identifier differs from the embedded context.
    CertificateContextMismatch,
    /// `CommitQC` height differs from the artifact height.
    CertificateHeightMismatch {
        /// Artifact height.
        artifact: Height,
        /// Certificate height.
        certificate: Height,
    },
    /// The finality certificate is not in the Commit phase.
    CertificateIsNotCommit,
    /// `CommitQC` subject differs from the artifact subject.
    CertificateSubjectMismatch,
    /// Repeated block hash differs from the subject block hash.
    SubjectBlockHashMismatch,
    /// Subject parent hash differs from the parent `CommitQC` in the context.
    ParentBlockHashMismatch,
    /// `CommitQC` quorum or signature structure is invalid.
    InvalidCommitCertificate(ValidationError),
    /// Durable `PoPs` are not aligned one-for-one with the frozen roster.
    ProofOfPossessionCount {
        /// Frozen roster length.
        expected: usize,
        /// Durable proof count.
        actual: usize,
    },
    /// A durable roster slot contains no proof-of-possession bytes.
    MissingProofOfPossession {
        /// Frozen roster index.
        index: u32,
    },
    /// One durable proof exceeds the protocol's bounded representation.
    ProofOfPossessionTooLarge {
        /// Frozen roster index.
        index: u32,
        /// Supplied proof size.
        actual: usize,
        /// Protocol maximum.
        max: usize,
    },
    /// Artifact height differs from the externally associated block height.
    AssociatedHeightMismatch {
        /// Artifact height.
        artifact: Height,
        /// Canonical block height.
        block: Height,
    },
    /// Artifact block hash differs from the externally associated block hash.
    AssociatedBlockHashMismatch,
    /// Artifact subject parent differs from the associated header predecessor.
    AssociatedParentBlockHashMismatch,
    /// `CommitQC` decision round precedes the view committed by the associated header.
    DecisionRoundBeforeBlockView {
        /// View carried by the `CommitQC` round.
        decision: u64,
        /// View carried by the canonical block header.
        block: u64,
    },
}

impl fmt::Display for V2FinalityValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormatVersion { expected, actual } => write!(
                f,
                "unsupported v2 finality artifact version {actual}; expected {expected}"
            ),
            Self::ProtocolVersionMismatch {
                expected,
                artifact,
                context,
            } => write!(
                f,
                "v2 finality protocol mismatch: expected {expected}, artifact {artifact}, context {context}"
            ),
            Self::InvalidHeightContext(error) => {
                write!(f, "invalid finality height context: {error}")
            }
            Self::HeightContextMismatch { artifact, context } => write!(
                f,
                "v2 finality height mismatch: artifact {artifact}, context {context}"
            ),
            Self::CertificateContextMismatch => {
                f.write_str("CommitQC is bound to another height context")
            }
            Self::CertificateHeightMismatch {
                artifact,
                certificate,
            } => write!(
                f,
                "v2 finality height mismatch: artifact {artifact}, CommitQC {certificate}"
            ),
            Self::CertificateIsNotCommit => {
                f.write_str("v2 finality artifact carries a non-Commit certificate")
            }
            Self::CertificateSubjectMismatch => {
                f.write_str("v2 finality artifact and CommitQC subjects differ")
            }
            Self::SubjectBlockHashMismatch => {
                f.write_str("v2 finality subject and repeated block hash differ")
            }
            Self::ParentBlockHashMismatch => {
                f.write_str("v2 finality subject does not extend the context parent CommitQC")
            }
            Self::InvalidCommitCertificate(error) => {
                write!(f, "invalid v2 finality CommitQC: {error}")
            }
            Self::ProofOfPossessionCount { expected, actual } => write!(
                f,
                "v2 finality PoP count {actual} does not match frozen roster length {expected}"
            ),
            Self::MissingProofOfPossession { index } => {
                write!(f, "v2 finality PoP at roster index {index} is empty")
            }
            Self::ProofOfPossessionTooLarge { index, actual, max } => write!(
                f,
                "v2 finality PoP at roster index {index} is {actual} bytes; maximum is {max}"
            ),
            Self::AssociatedHeightMismatch { artifact, block } => write!(
                f,
                "v2 finality artifact height {artifact} does not match block height {block}"
            ),
            Self::AssociatedBlockHashMismatch => {
                f.write_str("v2 finality artifact does not match the canonical block hash")
            }
            Self::AssociatedParentBlockHashMismatch => f.write_str(
                "v2 finality subject parent does not match the canonical block predecessor",
            ),
            Self::DecisionRoundBeforeBlockView { decision, block } => write!(
                f,
                "v2 finality decision round {decision} precedes block construction view {block}"
            ),
        }
    }
}

impl std::error::Error for V2FinalityValidationError {}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use norito::codec::{DecodeAll, Encode};

    use super::*;
    use crate::block::consensus_v2::{
        ConsensusRound, DataAvailabilityLayout, PayloadEncoding, ValidatorIndex,
    };
    use crate::{NetworkId, peer::PeerId};

    fn network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([seed; Hash::LENGTH]),
        ))
    }

    fn roster() -> Vec<ValidatorPower> {
        let mut peers = (1_u8..=4)
            .map(|seed| {
                let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("derive deterministic finality fixture keypair");
                PeerId::new(key_pair.public_key().clone())
            })
            .collect::<Vec<_>>();
        peers.sort();
        peers
            .into_iter()
            .map(|validator| ValidatorPower {
                validator,
                power: 1,
            })
            .collect()
    }

    fn context() -> HeightContext {
        let roster = roster();
        let next_epoch_snapshot = FinalizedNextEpochSnapshot {
            epoch: 8,
            epoch_end_height: 9,
            mode: ConsensusMode::Permissioned,
            roster: roster.clone(),
            quorum: DualQuorum::from_roster(&roster).expect("valid next-epoch quorum"),
            validator_set_pops: vec![vec![0xC2]; roster.len()],
            leader_seed: [0xC3; 32],
        };
        HeightContext {
            network_id: network_id(0xA1),
            protocol_version: PROTOCOL_VERSION,
            height: 1,
            epoch: 7,
            epoch_end_height: 1,
            next_epoch_snapshot: Some(next_epoch_snapshot),
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("valid fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"finality nexus amx context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [0x5A; 32],
        }
    }

    fn subject(seed: u8) -> BlockSubject {
        BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([seed, 1])),
            payload_hash: Hash::new([seed, 2]),
        }
    }

    fn execution_commitment(seed: u8) -> super::super::ExecutionCommitment {
        super::super::ExecutionCommitment::new_without_merge_carrier(
            Hash::new([seed, 3]),
            Hash::new([seed, 4]),
            Hash::new([seed, 5]),
            None,
            0,
            1,
            Hash::new([seed, 6]),
        )
        .expect("canonical fixture execution commitment")
    }

    fn artifact() -> V2FinalityArtifact {
        let context = context();
        let subject = subject(3);
        let round = ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        };
        let signers: Vec<ValidatorIndex> = vec![0, 1, 2];
        let commit_qc = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment: execution_commitment(3),
            signers,
            aggregate_signature: vec![0xA5; 48],
        };
        let validator_set_pops = vec![vec![0x5C]; context.roster.len()];
        V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops)
    }

    fn artifact_bound_to_header(
        header_view: u64,
        decision_view: u64,
    ) -> (V2FinalityArtifact, BlockHeader) {
        let mut artifact = artifact();
        let header = BlockHeader::new(
            NonZeroU64::new(artifact.height).expect("non-zero artifact height"),
            artifact.subject.parent_block_hash,
            None,
            None,
            0,
            header_view,
        );
        artifact.subject.block_hash = header.hash();
        artifact.block_hash = artifact.subject.block_hash;
        artifact.commit_qc.subject = artifact.subject;
        artifact.commit_qc.round.view = decision_view;
        artifact.commit_qc.proposal_round = artifact.commit_qc.round;
        (artifact, header)
    }

    #[test]
    fn artifact_roundtrip_preserves_validated_bindings() {
        let artifact = artifact();
        artifact.validate().expect("fixture is valid");

        let encoded = artifact.encode();
        let mut cursor = encoded.as_slice();
        let decoded = V2FinalityArtifact::decode_all(&mut cursor).expect("decode artifact");

        assert_eq!(decoded, artifact);
        decoded.validate().expect("roundtrip remains valid");
    }

    #[test]
    fn artifact_rejects_legacy_v3_layout() {
        let mut legacy = artifact();
        legacy.format_version = 3;

        assert_eq!(
            legacy.validate(),
            Err(V2FinalityValidationError::UnsupportedFormatVersion {
                expected: V2_FINALITY_ARTIFACT_VERSION,
                actual: 3,
            })
        );
    }

    #[test]
    fn header_binding_allows_unchanged_reproposal_but_rejects_earlier_decision_round() {
        let (delayed, header) = artifact_bound_to_header(3, 5);
        delayed
            .validate_for_header(&header)
            .expect("a later same-round CommitQC can certify unchanged body bytes");

        let (substituted, header) = artifact_bound_to_header(3, 2);
        assert_eq!(
            substituted.validate_for_header(&header),
            Err(V2FinalityValidationError::DecisionRoundBeforeBlockView {
                decision: 2,
                block: 3,
            })
        );
    }

    #[test]
    fn genesis_header_binding_accepts_a_later_first_decision_round() {
        let (delayed_genesis, header) = artifact_bound_to_header(0, 3);
        delayed_genesis
            .validate_for_header(&header)
            .expect("canonical genesis bytes retain view zero across a delayed first proposal");
    }

    #[test]
    fn decoded_artifact_rejects_protocol_context_height_subject_and_phase_mismatches() {
        let cases = [
            {
                let mut value = artifact();
                value.protocol_version = PROTOCOL_VERSION + 1;
                value
            },
            {
                let mut value = artifact();
                value.commit_qc.round.context_id =
                    HeightContextId(HashOf::from_untyped_unchecked(Hash::new(b"wrong context")));
                value
            },
            {
                let mut value = artifact();
                value.height += 1;
                value
            },
            {
                let mut value = artifact();
                value.commit_qc.subject = subject(9);
                value
            },
            {
                let mut value = artifact();
                value.commit_qc.phase = GlobalPhase::Prepare;
                value
            },
        ];

        for invalid in cases {
            let encoded = invalid.encode();
            let mut cursor = encoded.as_slice();
            let decoded =
                V2FinalityArtifact::decode_all(&mut cursor).expect("decode malformed case");
            assert!(
                decoded.validate().is_err(),
                "malformed binding was accepted"
            );
        }
    }

    #[test]
    fn next_epoch_snapshot_must_be_canonical_and_immediate() {
        let mut wrong_epoch = artifact();
        wrong_epoch
            .height_context
            .next_epoch_snapshot
            .as_mut()
            .expect("snapshot")
            .epoch += 1;
        assert!(matches!(
            wrong_epoch.validate(),
            Err(V2FinalityValidationError::InvalidHeightContext(
                ValidationError::InvalidNextEpoch
            ))
        ));

        let mut wrong_quorum = artifact();
        wrong_quorum
            .height_context
            .next_epoch_snapshot
            .as_mut()
            .expect("snapshot")
            .quorum
            .min_signers -= 1;
        assert_eq!(
            wrong_quorum.validate(),
            Err(V2FinalityValidationError::InvalidHeightContext(
                ValidationError::NextEpochQuorumMismatch
            ))
        );
    }

    #[test]
    fn epoch_snapshot_is_present_exactly_at_the_frozen_boundary() {
        let mut missing = artifact();
        missing.height_context.next_epoch_snapshot = None;
        assert_eq!(
            missing.validate(),
            Err(V2FinalityValidationError::InvalidHeightContext(
                ValidationError::MissingNextEpochSnapshot
            ))
        );

        let mut premature = artifact();
        premature.height_context.epoch_end_height = premature.height + 1;
        assert_eq!(
            premature.validate(),
            Err(V2FinalityValidationError::InvalidHeightContext(
                ValidationError::UnexpectedNextEpochSnapshot
            ))
        );
    }

    #[test]
    fn durable_validator_pops_are_exactly_aligned_nonempty_and_bounded() {
        let mut missing = artifact();
        missing.validator_set_pops.pop();
        assert!(matches!(
            missing.validate(),
            Err(V2FinalityValidationError::ProofOfPossessionCount { .. })
        ));

        let mut empty = artifact();
        empty.validator_set_pops[1].clear();
        assert_eq!(
            empty.validate(),
            Err(V2FinalityValidationError::MissingProofOfPossession { index: 1 })
        );

        let mut oversized = artifact();
        oversized.validator_set_pops[2] = vec![0xA4; MAX_VALIDATOR_POP_BYTES + 1];
        assert_eq!(
            oversized.validate(),
            Err(V2FinalityValidationError::ProofOfPossessionTooLarge {
                index: 2,
                actual: MAX_VALIDATOR_POP_BYTES + 1,
                max: MAX_VALIDATOR_POP_BYTES,
            })
        );

        let mut oversized_future = artifact();
        oversized_future
            .height_context
            .next_epoch_snapshot
            .as_mut()
            .expect("boundary snapshot")
            .validator_set_pops[0] = vec![0xA5; MAX_VALIDATOR_POP_BYTES + 1];
        assert_eq!(
            oversized_future.validate(),
            Err(V2FinalityValidationError::InvalidHeightContext(
                ValidationError::NextEpochProofOfPossessionTooLarge
            ))
        );
    }

    #[test]
    fn artifact_rejects_an_oversized_commit_aggregate_before_cryptography() {
        let mut oversized = artifact();
        oversized.commit_qc.aggregate_signature =
            vec![0xA6; crate::block::consensus_v2::MAX_CONSENSUS_SIGNATURE_BYTES + 1];
        assert_eq!(
            oversized.validate(),
            Err(V2FinalityValidationError::InvalidCommitCertificate(
                ValidationError::SignatureTooLarge
            ))
        );
    }

    #[test]
    fn commit_qc_context_id_authenticates_the_exact_epoch_transition() {
        let canonical = artifact();
        canonical.validate().expect("canonical artifact");

        let mut forged_seed = canonical.clone();
        forged_seed
            .height_context
            .next_epoch_snapshot
            .as_mut()
            .expect("boundary snapshot")
            .leader_seed[0] ^= 0x80;

        let mut forged_roster = canonical.clone();
        let snapshot = forged_roster
            .height_context
            .next_epoch_snapshot
            .as_mut()
            .expect("boundary snapshot");
        let replacement = KeyPair::try_from_seed(vec![0xE1; 32], Algorithm::Ed25519)
            .expect("replacement validator key");
        snapshot.roster[0].validator = PeerId::new(replacement.public_key().clone());
        snapshot
            .roster
            .sort_by(|left, right| left.validator.cmp(&right.validator));
        snapshot.quorum = DualQuorum::from_roster(&snapshot.roster).expect("mutated valid roster");

        for forged in [forged_seed, forged_roster] {
            assert_ne!(forged.height_context.id(), canonical.height_context.id());
            assert_eq!(
                forged.validate(),
                Err(V2FinalityValidationError::CertificateContextMismatch)
            );
        }
    }
}
