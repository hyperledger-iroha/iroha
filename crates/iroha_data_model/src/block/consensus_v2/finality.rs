//! Durable, versioned finality records for canonical Sumeragi v2 blocks.
//!
//! These values are structural persistence records. Their validation checks
//! every redundant protocol, context, height, subject, and block-hash binding,
//! but deliberately leaves aggregate-signature verification to the consensus
//! cryptography adapter.

use core::fmt;
use std::vec::Vec;

use iroha_crypto::HashOf;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{
    BlockSubject, ConsensusMode, DualQuorum, GlobalPhase, Height, HeightContext, HeightContextId,
    PROTOCOL_VERSION, QuorumCertificate, ValidationError, ValidatorPower,
};
use crate::block::BlockHeader;

/// Current Norito layout version of [`V2FinalityArtifact`].
pub const V2_FINALITY_ARTIFACT_VERSION: u16 = 1;

/// Election inputs finalized for the epoch immediately following a block.
///
/// The snapshot is optional because most blocks do not finalize an epoch
/// transition. When present, it carries the complete ordered voting-power
/// roster rather than a reference to mutable world state.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FinalizedNextEpochSnapshot {
    /// Epoch immediately following the artifact's height context epoch.
    pub epoch: u64,
    /// Genesis-selected consensus mode used to interpret voting powers.
    pub mode: ConsensusMode,
    /// Canonically ordered voting roster and exact integer powers.
    pub roster: Vec<ValidatorPower>,
    /// Canonical dual quorum derived from `roster`.
    pub quorum: DualQuorum,
    /// Finalized seed used for deterministic leader rotation in this epoch.
    pub leader_seed: [u8; 32],
}

impl FinalizedNextEpochSnapshot {
    fn validate_against(&self, context: &HeightContext) -> Result<(), V2FinalityValidationError> {
        let expected_epoch = context
            .epoch
            .checked_add(1)
            .ok_or(V2FinalityValidationError::EpochOverflow)?;
        if self.epoch != expected_epoch {
            return Err(V2FinalityValidationError::NextEpochMismatch {
                expected: expected_epoch,
                actual: self.epoch,
            });
        }
        if self.mode != context.mode {
            return Err(V2FinalityValidationError::NextEpochModeMismatch);
        }
        let canonical = DualQuorum::from_roster(&self.roster)
            .map_err(V2FinalityValidationError::InvalidNextEpochRoster)?;
        if self.quorum != canonical {
            return Err(V2FinalityValidationError::NextEpochQuorumMismatch);
        }
        if self.mode == ConsensusMode::Permissioned
            && self.roster.iter().any(|validator| validator.power != 1)
        {
            return Err(V2FinalityValidationError::NextEpochPermissionedPowerNotOne);
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
pub struct V2FinalityArtifact {
    /// Norito layout version; currently [`V2_FINALITY_ARTIFACT_VERSION`].
    pub format_version: u16,
    /// Consensus protocol revision; must equal [`PROTOCOL_VERSION`].
    pub protocol_version: u16,
    /// Block height repeated for file association and early rejection.
    pub height: Height,
    /// Complete immutable consensus context governing this height.
    pub height_context: HeightContext,
    /// Exact subject certified by the CommitQC.
    pub subject: BlockSubject,
    /// Canonical block header hash, repeated from `subject`.
    pub block_hash: HashOf<BlockHeader>,
    /// Commit certificate that finalized `subject`.
    pub commit_qc: QuorumCertificate,
    /// Finalized next-epoch election snapshot, when this is an epoch boundary.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub next_epoch_snapshot: Option<FinalizedNextEpochSnapshot>,
}

impl V2FinalityArtifact {
    /// Construct the canonical current-layout artifact.
    #[must_use]
    pub fn new(
        height_context: HeightContext,
        subject: BlockSubject,
        commit_qc: QuorumCertificate,
        next_epoch_snapshot: Option<FinalizedNextEpochSnapshot>,
    ) -> Self {
        Self {
            format_version: V2_FINALITY_ARTIFACT_VERSION,
            protocol_version: PROTOCOL_VERSION,
            height: height_context.height,
            height_context,
            subject,
            block_hash: subject.block_hash,
            commit_qc,
            next_epoch_snapshot,
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
            .map(|parent| parent.subject.block_hash);
        if self.subject.parent_block_hash != expected_parent {
            return Err(V2FinalityValidationError::ParentBlockHashMismatch);
        }
        self.commit_qc
            .validate(&self.height_context)
            .map_err(V2FinalityValidationError::InvalidCommitCertificate)?;
        match (
            self.height == self.height_context.epoch_end_height,
            self.next_epoch_snapshot.as_ref(),
        ) {
            (true, Some(snapshot)) => snapshot.validate_against(&self.height_context)?,
            (true, None) => return Err(V2FinalityValidationError::MissingNextEpochSnapshot),
            (false, Some(_)) => {
                return Err(V2FinalityValidationError::UnexpectedNextEpochSnapshot);
            }
            (false, None) => {}
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
    /// CommitQC context identifier differs from the embedded context.
    CertificateContextMismatch,
    /// CommitQC height differs from the artifact height.
    CertificateHeightMismatch {
        /// Artifact height.
        artifact: Height,
        /// Certificate height.
        certificate: Height,
    },
    /// The finality certificate is not in the Commit phase.
    CertificateIsNotCommit,
    /// CommitQC subject differs from the artifact subject.
    CertificateSubjectMismatch,
    /// Repeated block hash differs from the subject block hash.
    SubjectBlockHashMismatch,
    /// Subject parent hash differs from the parent CommitQC in the context.
    ParentBlockHashMismatch,
    /// CommitQC quorum or signature structure is invalid.
    InvalidCommitCertificate(ValidationError),
    /// The current epoch cannot be incremented.
    EpochOverflow,
    /// Optional snapshot is not for the immediately following epoch.
    NextEpochMismatch {
        /// Required next epoch.
        expected: u64,
        /// Snapshot epoch.
        actual: u64,
    },
    /// Optional snapshot changes the genesis-selected consensus mode.
    NextEpochModeMismatch,
    /// Optional snapshot roster is structurally invalid.
    InvalidNextEpochRoster(ValidationError),
    /// Optional snapshot quorum is not canonically derived from its roster.
    NextEpochQuorumMismatch,
    /// Permissioned next-epoch voting powers are not all one.
    NextEpochPermissionedPowerNotOne,
    /// An epoch-ending block omitted the mandatory next-epoch snapshot.
    MissingNextEpochSnapshot,
    /// A non-boundary block attempted to change the election snapshot.
    UnexpectedNextEpochSnapshot,
    /// Artifact height differs from the externally associated block height.
    AssociatedHeightMismatch {
        /// Artifact height.
        artifact: Height,
        /// Canonical block height.
        block: Height,
    },
    /// Artifact block hash differs from the externally associated block hash.
    AssociatedBlockHashMismatch,
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
            Self::EpochOverflow => f.write_str("v2 finality context epoch overflows u64"),
            Self::NextEpochMismatch { expected, actual } => write!(
                f,
                "v2 finality next epoch mismatch: expected {expected}, got {actual}"
            ),
            Self::NextEpochModeMismatch => {
                f.write_str("v2 finality next-epoch snapshot changes consensus mode")
            }
            Self::InvalidNextEpochRoster(error) => {
                write!(f, "invalid v2 finality next-epoch roster: {error}")
            }
            Self::NextEpochQuorumMismatch => {
                f.write_str("v2 finality next-epoch quorum does not match its roster")
            }
            Self::NextEpochPermissionedPowerNotOne => f.write_str(
                "v2 finality permissioned next-epoch snapshot contains non-unit voting power",
            ),
            Self::MissingNextEpochSnapshot => {
                f.write_str("epoch-ending v2 finality artifact is missing its next-epoch snapshot")
            }
            Self::UnexpectedNextEpochSnapshot => {
                f.write_str("non-boundary v2 finality artifact contains a next-epoch snapshot")
            }
            Self::AssociatedHeightMismatch { artifact, block } => write!(
                f,
                "v2 finality artifact height {artifact} does not match block height {block}"
            ),
            Self::AssociatedBlockHashMismatch => {
                f.write_str("v2 finality artifact does not match the canonical block hash")
            }
        }
    }
}

impl std::error::Error for V2FinalityValidationError {}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use norito::codec::{DecodeAll, Encode};

    use super::*;
    use crate::block::consensus_v2::{
        ConsensusRound, DataAvailabilityLayout, PayloadEncoding, ValidatorIndex,
    };
    use crate::{ChainId, peer::PeerId};

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
        HeightContext {
            chain_id: ChainId::from("v2-finality-test"),
            protocol_version: PROTOCOL_VERSION,
            height: 1,
            epoch: 7,
            epoch_end_height: 1,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: None,
            quorum: DualQuorum::from_roster(&roster).expect("valid fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"finality nexus amx context"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 4096,
                max_chunk_count: 4,
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
            phase: GlobalPhase::Commit,
            subject,
            signers,
            aggregate_signature: vec![0xA5; 48],
        };
        let next_epoch_snapshot = FinalizedNextEpochSnapshot {
            epoch: context.epoch + 1,
            mode: context.mode,
            roster: context.roster.clone(),
            quorum: context.quorum,
            leader_seed: [0xC3; 32],
        };
        V2FinalityArtifact::new(context, subject, commit_qc, Some(next_epoch_snapshot))
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
            .next_epoch_snapshot
            .as_mut()
            .expect("snapshot")
            .epoch += 1;
        assert!(matches!(
            wrong_epoch.validate(),
            Err(V2FinalityValidationError::NextEpochMismatch { .. })
        ));

        let mut wrong_quorum = artifact();
        wrong_quorum
            .next_epoch_snapshot
            .as_mut()
            .expect("snapshot")
            .quorum
            .min_signers -= 1;
        assert_eq!(
            wrong_quorum.validate(),
            Err(V2FinalityValidationError::NextEpochQuorumMismatch)
        );
    }

    #[test]
    fn epoch_snapshot_is_present_exactly_at_the_frozen_boundary() {
        let mut missing = artifact();
        missing.next_epoch_snapshot = None;
        assert_eq!(
            missing.validate(),
            Err(V2FinalityValidationError::MissingNextEpochSnapshot)
        );

        let mut premature = artifact();
        premature.height_context.epoch_end_height = premature.height + 1;
        premature.commit_qc.round.context_id = premature.height_context.id();
        assert_eq!(
            premature.validate(),
            Err(V2FinalityValidationError::UnexpectedNextEpochSnapshot)
        );
    }
}
