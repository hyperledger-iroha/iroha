//! Bridge-related data types for wrapped assets and receipts.
//! Feature-gated behind `bridge`.

use std::{collections::BTreeSet, string::String, vec::Vec};

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    ChainId, consensus::VALIDATOR_SET_HASH_VERSION_V1, nexus::LaneId, peer::PeerId, proof::ProofBox,
};

/// Definition metadata for a wrapped asset originating from another chain.
///
/// Stored alongside an Iroha asset definition to bind it to its origin.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct WrappedAssetDef {
    /// Origin chain identifier (canonical bytes, e.g., "btc", "evm-eth").
    pub origin_chain: Vec<u8>,
    /// Origin asset identifier on the origin chain (canonical bytes).
    pub origin_asset_id: Vec<u8>,
    /// Bridge lane identifier that minted this wrapped asset (canonical bytes).
    pub bridge_id: Vec<u8>,
}

/// A receipt emitted by the bridge lane to record a cross-chain action.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeReceipt {
    /// Lane identifier (e.g., "btc→iroha", "iroha↔evm").
    pub lane: LaneId,
    /// Direction of the action: "lock", "mint", "burn", or "release".
    pub direction: Vec<u8>,
    /// Source transaction or message hash (32 bytes canonical).
    pub source_tx: [u8; 32],
    /// Optional destination transaction hash, if known.
    pub dest_tx: Option<[u8; 32]>,
    /// Hash of the verification proof submitted for this action.
    pub proof_hash: [u8; 32],
    /// Amount transferred (integer units matching the asset definition).
    pub amount: u128,
    /// Canonical Iroha asset id bytes.
    pub asset_id: Vec<u8>,
    /// Recipient identifier bytes (Iroha account id or external address payload).
    pub recipient: Vec<u8>,
}

/// Hash function used by bridge Merkle proofs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(tag = "hash_function", content = "value")]
pub enum BridgeHashFunction {
    /// SHA-256 (ICS-style hash-only light clients).
    Sha256,
    /// Blake2b (mirrors Iroha’s internal hash).
    Blake2b,
}

/// Height range covered by a bridge proof artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeProofRange {
    /// Inclusive start height of the batch.
    pub start_height: u64,
    /// Inclusive end height of the batch.
    pub end_height: u64,
}

impl BridgeProofRange {
    /// Returns `true` if the range is non-empty and ordered.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.start_height <= self.end_height
    }

    /// Length of the covered window (`end_height - start_height + 1`).
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.end_height
            .saturating_sub(self.start_height)
            .saturating_add(1)
    }

    /// Returns `true` when the range is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// ICS-style proof payload (hash-only light client).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeIcsProof {
    /// State root advertised by the counterparty chain.
    pub state_root: [u8; 32],
    /// Leaf hash being proven.
    pub leaf_hash: [u8; 32],
    /// Compact Merkle path from leaf to root.
    pub proof: iroha_crypto::MerkleProof<[u8; 32]>,
    /// Hash function used when computing parent nodes.
    pub hash_function: BridgeHashFunction,
}

/// Transparent ZK proof payload (rolling recursive proof).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeTransparentProof {
    /// Opaque proof bytes tagged with backend identifier.
    pub proof: ProofBox,
    /// Optional recursion depth claimed by the prover.
    pub recursion_depth: Option<u32>,
}

/// Bridge proof payload kinds supported by the data model.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(tag = "kind", content = "payload")]
pub enum BridgeProofPayload {
    /// ICS-23-style inclusion proof against a state root.
    Ics(BridgeIcsProof),
    /// Transparent recursive ZK proof.
    TransparentZk(BridgeTransparentProof),
}

/// Bridge proof artifact with manifest binding and retention hints.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeProof {
    /// Height range covered by this proof.
    pub range: BridgeProofRange,
    /// Manifest integrity hash (32-byte commitment to verifier manifest).
    pub manifest_hash: [u8; 32],
    /// Proof payload (ICS or transparent ZK).
    pub payload: BridgeProofPayload,
    /// When set, retention will avoid pruning this artifact.
    pub pinned: bool,
}

impl BridgeProof {
    /// Return a backend label suitable for hashing/id construction.
    #[must_use]
    pub fn backend_label(&self) -> String {
        match &self.payload {
            BridgeProofPayload::Ics(_) => "bridge/ics23".to_owned(),
            BridgeProofPayload::TransparentZk(p) => {
                format!("bridge/{}", p.proof.backend)
            }
        }
    }
}

/// Stored bridge proof record with size metadata and commitment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeProofRecord {
    /// Recorded proof artifact.
    pub proof: BridgeProof,
    /// Hash commitment for the proof bytes (backend-specific).
    pub commitment: [u8; 32],
    /// Total encoded size of the stored proof (bytes).
    pub size_bytes: u32,
}

/// Finality proof for an Iroha block built from the consensus commit certificate.
///
/// This proof is self-contained: it carries the block header, its hash, and the
/// commit certificate (validator set + signatures) produced by the active
/// validator set for that height. Verifiers recompute the block hash from the
/// header and validate the commit certificate signatures against the provided
/// validator set.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveFastJson,
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeFinalityProof {
    /// Height of the finalized block.
    pub height: u64,
    /// Chain identifier to prevent cross-chain replay.
    pub chain_id: crate::ChainId,
    /// Block header for the finalized block.
    pub block_header: crate::block::BlockHeader,
    /// Hash of the block header.
    pub block_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
    /// Commit certificate collected for the block.
    pub commit_certificate: crate::consensus::CommitCertificate,
}

/// Authority set snapshot used for bridge commitments.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeAuthoritySet {
    /// Monotonically increasing authority set identifier.
    pub id: u64,
    /// Ordered validator set at this authority set id.
    pub validator_set: Vec<crate::peer::PeerId>,
    /// Hash of the validator set using the configured hash version.
    pub validator_set_hash: iroha_crypto::HashOf<Vec<crate::peer::PeerId>>,
    /// Hash version used when computing `validator_set_hash`.
    pub validator_set_hash_version: u16,
}

/// Commitment covering a block hash and authority set.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeCommitment {
    /// Chain identifier to prevent cross-chain replay.
    pub chain_id: crate::ChainId,
    /// Authority set that signed this commitment.
    pub authority_set: BridgeAuthoritySet,
    /// Block height bound into the commitment.
    pub block_height: u64,
    /// Block hash bound into the commitment (used as the leaf hash in the MMR).
    pub block_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
    /// Optional MMR root covering recent blocks. When present, verifiers should
    /// prefer MMR inclusion proofs over direct hash checks.
    pub mmr_root: Option<[u8; 32]>,
    /// Optional leaf index in the MMR for this block (0-based).
    pub mmr_leaf_index: Option<u64>,
    /// Optional list of MMR peaks associated with `mmr_root` to help external
    /// verifiers reconstruct the root without replaying the full chain.
    pub mmr_peaks: Option<Vec<[u8; 32]>>,
    /// Optional next authority set advertised by this commitment.
    pub next_authority_set: Option<BridgeAuthoritySet>,
}

/// Justification (signatures) for a bridge commitment.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeCommitmentJustification {
    /// Signatures from the authority set over the commitment payload.
    pub signatures: Vec<crate::block::BlockSignature>,
}

/// Bundle containing a commitment, justification, and block details.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeFinalityBundle {
    /// Commitment binding the block hash and authority set.
    pub commitment: BridgeCommitment,
    /// Justification (signatures) for the commitment.
    pub justification: BridgeCommitmentJustification,
    /// Block header for the finalized block.
    pub block_header: crate::block::BlockHeader,
    /// Commit certificate for the block.
    pub commit_certificate: crate::consensus::CommitCertificate,
}

/// Errors surfaced when verifying bridge finality proofs.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BridgeFinalityVerifyError {
    /// Proof is bound to a different chain id.
    #[error("chain id mismatch: expected {expected}, got {got}")]
    ChainIdMismatch {
        /// Expected chain id.
        expected: ChainId,
        /// Chain id carried inside the proof.
        got: ChainId,
    },
    /// Commit certificate height disagrees with the proof height.
    #[error(
        "commit certificate height {certificate_height} does not match proof height {proof_height}"
    )]
    CertificateHeightMismatch {
        /// Height recorded in the proof.
        proof_height: u64,
        /// Height recorded in the commit certificate.
        certificate_height: u64,
    },
    /// Block hash is inconsistent across the header/proof/certificate tuple.
    #[error(
        "block hash mismatch (header {header_hash:?}, proof field {proof_hash:?}, certificate {certificate_hash:?})"
    )]
    BlockHashMismatch {
        /// Hash recomputed from the block header.
        header_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
        /// Hash advertised by the proof.
        proof_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
        /// Hash advertised by the commit certificate.
        certificate_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
    },
    /// Validator-set hash version is unknown.
    #[error("validator set hash version {version} is not supported")]
    UnsupportedValidatorSetHashVersion {
        /// Validator-set hash version carried in the proof.
        version: u16,
    },
    /// Validator-set hash does not match the recorded validator set.
    #[error(
        "validator set hash mismatch: recorded {recorded:?}, computed {computed:?} (version {version})"
    )]
    ValidatorSetHashMismatch {
        /// Hash recorded in the proof.
        recorded: iroha_crypto::HashOf<Vec<PeerId>>,
        /// Hash recomputed from the validator set.
        computed: iroha_crypto::HashOf<Vec<PeerId>>,
        /// Validator-set hash version recorded in the proof.
        version: u16,
    },
    /// Proof was built with a different validator set than the expected anchor.
    #[error("validator set hash {got:?} does not match expected {expected:?}")]
    UnexpectedValidatorSet {
        /// Expected validator-set hash anchor.
        expected: iroha_crypto::HashOf<Vec<PeerId>>,
        /// Validator-set hash carried in the proof.
        got: iroha_crypto::HashOf<Vec<PeerId>>,
    },
    /// Proof was produced for a different epoch than the expected anchor.
    #[error("commit certificate epoch {got} does not match expected {expected}")]
    UnexpectedEpoch {
        /// Expected epoch anchor.
        expected: u64,
        /// Epoch carried in the proof.
        got: u64,
    },
    /// Proof carries an empty validator set.
    #[error("validator set is empty")]
    EmptyValidatorSet,
    /// Signature index references a validator outside the roster bounds.
    #[error("signature index {index} is out of range for validator set length {len}")]
    SignatureIndexOutOfRange {
        /// Index recorded in the signature.
        index: u64,
        /// Validator-set length.
        len: usize,
    },
    /// Proof carries duplicate signatures from the same validator index.
    #[error("duplicate signature index {index}")]
    DuplicateSignatureIndex {
        /// Duplicate index encountered.
        index: u64,
    },
    /// Signature failed to verify against the advertised block header.
    #[error("invalid signature for validator index {index}")]
    InvalidSignature {
        /// Signer index that failed verification.
        index: u64,
    },
    /// Proof does not contain enough unique signatures to satisfy quorum.
    #[error("insufficient signatures: required {required}, collected {collected}")]
    InsufficientSignatures {
        /// Quorum required for the advertised validator set.
        required: usize,
        /// Unique signatures collected in the proof.
        collected: usize,
    },
    /// Proof height is older than the latest verified height.
    #[error("proof height {height} is stale relative to latest verified height {latest}")]
    StaleHeight {
        /// Latest height accepted by the verifier.
        latest: u64,
        /// Height carried by the proof.
        height: u64,
    },
    /// Proof height skips past the next expected height.
    #[error("proof height {height} advances past the next expected height after {latest}")]
    AdvancedHeight {
        /// Latest height accepted by the verifier.
        latest: u64,
        /// Height carried by the proof.
        height: u64,
    },
}

/// Stateful verifier for bridge finality proofs.
///
/// The verifier enforces the canonical `(block_header, block_hash, commit_certificate)` tuple,
/// binds proofs to a chain id, and checks commit-certificate signatures against the advertised
/// validator set with the production quorum rule. It tracks the latest verified height to reject
/// stale or skipped proofs, can anchor to a trusted validator-set hash, and optionally fixes the
/// expected epoch to reject replays across topology changes.
#[derive(Debug, Clone)]
pub struct BridgeFinalityVerifier {
    expected_chain_id: ChainId,
    expected_validator_set_hash: Option<iroha_crypto::HashOf<Vec<PeerId>>>,
    validator_set_hash_version: u16,
    expected_epoch: Option<u64>,
    latest_height: Option<u64>,
}

impl BridgeFinalityVerifier {
    /// Construct a verifier bound to the expected `chain_id`.
    #[must_use]
    pub fn new(expected_chain_id: ChainId) -> Self {
        Self {
            expected_chain_id,
            expected_validator_set_hash: None,
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            expected_epoch: None,
            latest_height: None,
        }
    }

    /// Construct a verifier bound to the expected `chain_id` and validator-set hash anchor.
    #[must_use]
    pub fn with_validator_set(
        expected_chain_id: ChainId,
        validator_set_hash: iroha_crypto::HashOf<Vec<PeerId>>,
        validator_set_hash_version: u16,
    ) -> Self {
        Self {
            expected_chain_id,
            expected_validator_set_hash: Some(validator_set_hash),
            validator_set_hash_version,
            expected_epoch: None,
            latest_height: None,
        }
    }

    /// Construct a verifier bound to the expected `chain_id`, validator-set hash, and epoch anchor.
    #[must_use]
    pub fn with_validator_set_and_epoch(
        expected_chain_id: ChainId,
        validator_set_hash: iroha_crypto::HashOf<Vec<PeerId>>,
        validator_set_hash_version: u16,
        expected_epoch: u64,
    ) -> Self {
        Self {
            expected_chain_id,
            expected_validator_set_hash: Some(validator_set_hash),
            validator_set_hash_version,
            expected_epoch: Some(expected_epoch),
            latest_height: None,
        }
    }

    /// Update the expected validator-set hash anchor used when verifying proofs.
    pub fn set_validator_set_anchor(
        &mut self,
        validator_set_hash: iroha_crypto::HashOf<Vec<PeerId>>,
        validator_set_hash_version: u16,
    ) {
        self.expected_validator_set_hash = Some(validator_set_hash);
        self.validator_set_hash_version = validator_set_hash_version;
    }

    /// Update the expected epoch anchor used when verifying proofs.
    pub fn set_epoch_anchor(&mut self, expected_epoch: u64) {
        self.expected_epoch = Some(expected_epoch);
    }

    /// Update both the validator-set and epoch anchors together to reflect a topology change.
    pub fn set_validator_set_and_epoch_anchor(
        &mut self,
        validator_set_hash: iroha_crypto::HashOf<Vec<PeerId>>,
        validator_set_hash_version: u16,
        expected_epoch: u64,
    ) {
        self.set_validator_set_anchor(validator_set_hash, validator_set_hash_version);
        self.expected_epoch = Some(expected_epoch);
    }

    /// Verify a bridge finality proof against the configured expectations.
    ///
    /// On success, advances the latest verified height and, when no anchor is set, captures the
    /// proof's validator-set hash and epoch for replay detection.
    ///
    /// # Errors
    /// Returns [`BridgeFinalityVerifyError`] when the proof's chain id, height continuity,
    /// hashes, epoch anchor, validator-set hash/version, or commit signatures are invalid.
    pub fn verify(&mut self, proof: &BridgeFinalityProof) -> Result<(), BridgeFinalityVerifyError> {
        if proof.chain_id != self.expected_chain_id {
            return Err(BridgeFinalityVerifyError::ChainIdMismatch {
                expected: self.expected_chain_id.clone(),
                got: proof.chain_id.clone(),
            });
        }

        if let Some(latest) = self.latest_height {
            if proof.height <= latest {
                return Err(BridgeFinalityVerifyError::StaleHeight {
                    latest,
                    height: proof.height,
                });
            }
            if proof.height > latest.saturating_add(1) {
                return Err(BridgeFinalityVerifyError::AdvancedHeight {
                    latest,
                    height: proof.height,
                });
            }
        }

        if proof.commit_certificate.height != proof.height {
            return Err(BridgeFinalityVerifyError::CertificateHeightMismatch {
                proof_height: proof.height,
                certificate_height: proof.commit_certificate.height,
            });
        }

        let header_hash = iroha_crypto::HashOf::new(&proof.block_header);
        let proof_hash = proof.block_hash;
        let certificate_hash = proof.commit_certificate.block_hash;
        if header_hash != proof_hash || header_hash != certificate_hash {
            return Err(BridgeFinalityVerifyError::BlockHashMismatch {
                header_hash,
                proof_hash,
                certificate_hash,
            });
        }

        if let Some(expected_epoch) = self
            .expected_epoch
            .filter(|expected| proof.commit_certificate.epoch != *expected)
        {
            return Err(BridgeFinalityVerifyError::UnexpectedEpoch {
                expected: expected_epoch,
                got: proof.commit_certificate.epoch,
            });
        }

        let recorded_version = proof.commit_certificate.validator_set_hash_version;
        if recorded_version != self.validator_set_hash_version {
            return Err(
                BridgeFinalityVerifyError::UnsupportedValidatorSetHashVersion {
                    version: recorded_version,
                },
            );
        }

        let recorded_hash = proof.commit_certificate.validator_set_hash;
        let computed_hash = iroha_crypto::HashOf::new(&proof.commit_certificate.validator_set);
        if computed_hash != recorded_hash {
            return Err(BridgeFinalityVerifyError::ValidatorSetHashMismatch {
                recorded: recorded_hash,
                computed: computed_hash,
                version: recorded_version,
            });
        }

        if let Some(expected) = self.expected_validator_set_hash {
            if recorded_hash != expected {
                return Err(BridgeFinalityVerifyError::UnexpectedValidatorSet {
                    expected,
                    got: recorded_hash,
                });
            }
        } else {
            // Adopt the recorded validator-set hash as an anchor for future proofs to detect
            // replay across epochs/rosters unless the caller replaces it explicitly.
            self.expected_validator_set_hash = Some(recorded_hash);
        }

        let validator_set = &proof.commit_certificate.validator_set;
        if validator_set.is_empty() {
            return Err(BridgeFinalityVerifyError::EmptyValidatorSet);
        }

        Self::validate_signatures(
            validator_set,
            &proof.commit_certificate.signatures,
            &proof.block_header,
        )?;

        self.latest_height = Some(proof.height);
        if self.expected_epoch.is_none() {
            self.expected_epoch = Some(proof.commit_certificate.epoch);
        }
        Ok(())
    }

    fn validate_signatures(
        validator_set: &[PeerId],
        signatures: &[crate::block::BlockSignature],
        header: &crate::block::BlockHeader,
    ) -> Result<(), BridgeFinalityVerifyError> {
        let mut seen = BTreeSet::new();
        let mut collected = 0usize;
        let required = Self::min_signatures(validator_set.len());
        for signature in signatures {
            let index = signature.index();
            let idx_usize: usize = index.try_into().map_err(|_| {
                BridgeFinalityVerifyError::SignatureIndexOutOfRange {
                    index,
                    len: validator_set.len(),
                }
            })?;

            if idx_usize >= validator_set.len() {
                return Err(BridgeFinalityVerifyError::SignatureIndexOutOfRange {
                    index,
                    len: validator_set.len(),
                });
            }

            if !seen.insert(index) {
                return Err(BridgeFinalityVerifyError::DuplicateSignatureIndex { index });
            }

            let public_key = &validator_set[idx_usize].public_key;
            if signature.signature().verify(public_key, header).is_err() {
                return Err(BridgeFinalityVerifyError::InvalidSignature { index });
            }

            collected += 1;
        }

        if collected < required {
            return Err(BridgeFinalityVerifyError::InsufficientSignatures {
                required,
                collected,
            });
        }

        Ok(())
    }

    const fn min_signatures(len: usize) -> usize {
        if len > 3 {
            ((len.saturating_sub(1)) / 3) * 2 + 1
        } else {
            len
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{HashOf, KeyPair, SignatureOf};
    use iroha_version::DecodeAll;

    use super::*;

    fn validator_set_from_keys(keys: &[KeyPair]) -> Vec<PeerId> {
        keys.iter()
            .map(|kp| PeerId::from(kp.public_key().clone()))
            .collect()
    }

    fn make_finality_proof(
        chain_id: &str,
        height: u64,
        epoch: u64,
        keys: &[KeyPair],
    ) -> BridgeFinalityProof {
        let header = crate::block::BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let block_hash = HashOf::new(&header);
        let validator_set = validator_set_from_keys(keys);
        let validator_set_hash = HashOf::new(&validator_set);
        let signatures = keys
            .iter()
            .enumerate()
            .map(|(idx, kp)| {
                let signature = SignatureOf::new(kp.private_key(), &header);
                crate::block::BlockSignature::new(idx as u64, signature)
            })
            .collect();
        let commit_certificate = crate::consensus::CommitCertificate {
            height,
            block_hash,
            view: 0,
            epoch,
            validator_set_hash,
            validator_set_hash_version: crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            signatures,
        };

        BridgeFinalityProof {
            height,
            chain_id: chain_id.parse().expect("chain id"),
            block_header: header,
            block_hash,
            commit_certificate,
        }
    }

    #[test]
    fn wrapped_asset_roundtrip() {
        let def = WrappedAssetDef {
            origin_chain: b"btc".to_vec(),
            origin_asset_id: b"btc:mainnet".to_vec(),
            bridge_id: b"btc->iroha".to_vec(),
        };
        let buf = def.encode();
        let dec = WrappedAssetDef::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(def, dec);
    }

    #[test]
    fn receipt_roundtrip() {
        let r = BridgeReceipt {
            lane: LaneId::from(1),
            direction: b"mint".to_vec(),
            source_tx: [0x11; 32],
            dest_tx: Some([0x22; 32]),
            proof_hash: [0x33; 32],
            amount: 42,
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
        };
        let buf = r.encode();
        let dec = BridgeReceipt::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(r, dec);
    }

    #[test]
    fn bridge_proof_roundtrip() {
        let leaves = vec![[0xAA; 32], [0xBB; 32]];
        let tree = iroha_crypto::MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves.clone());
        let root_bytes: [u8; 32] = *tree.root().expect("root").as_ref();
        let proof = tree.get_proof(0).expect("proof");

        let proof = BridgeProof {
            range: BridgeProofRange {
                start_height: 1,
                end_height: 2,
            },
            manifest_hash: [0x55; 32],
            payload: BridgeProofPayload::Ics(BridgeIcsProof {
                state_root: root_bytes,
                leaf_hash: leaves[0],
                proof,
                hash_function: BridgeHashFunction::Sha256,
            }),
            pinned: true,
        };
        let buf = proof.encode();
        let dec = BridgeProof::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(proof, dec);
    }

    #[test]
    fn bridge_finality_proof_roundtrip() {
        use std::num::NonZeroU64;

        use iroha_crypto::{HashOf, KeyPair};

        use crate::block::BlockHeader;

        let _kp = KeyPair::random();
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero"),
            None,
            None,
            None,
            0,
            0,
        );
        let block_hash = HashOf::new(&header);
        let commit_certificate = crate::consensus::CommitCertificate {
            height: 1,
            block_hash,
            view: 0,
            epoch: 0,
            validator_set_hash: HashOf::new(&Vec::<crate::peer::PeerId>::new()),
            validator_set_hash_version: crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: Vec::new(),
            signatures: Vec::new(),
        };
        let proof = BridgeFinalityProof {
            height: 1,
            chain_id: "proof-chain".parse().expect("chain id"),
            block_header: header,
            block_hash,
            commit_certificate,
        };
        let buf = proof.encode();
        let dec = BridgeFinalityProof::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(proof, dec);
    }

    #[test]
    fn verifier_rejects_wrong_chain_id() {
        let keys: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let proof = make_finality_proof("chain-a", 1, 0, &keys);
        let mut verifier = BridgeFinalityVerifier::new("chain-b".parse().expect("chain id parses"));

        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::ChainIdMismatch { .. }
        ));
    }

    #[test]
    fn verifier_rejects_stale_and_advanced_heights() {
        let keys: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let mut verifier = BridgeFinalityVerifier::new("chain-a".parse().expect("chain id parses"));

        let first = make_finality_proof("chain-a", 1, 0, &keys);
        verifier.verify(&first).expect("first proof accepted");

        let stale_err = verifier.verify(&first).unwrap_err();
        assert!(matches!(
            stale_err,
            BridgeFinalityVerifyError::StaleHeight {
                latest: 1,
                height: 1
            }
        ));

        let advanced = make_finality_proof("chain-a", 3, 0, &keys);
        let advanced_err = verifier.verify(&advanced).unwrap_err();
        assert!(matches!(
            advanced_err,
            BridgeFinalityVerifyError::AdvancedHeight {
                latest: 1,
                height: 3
            }
        ));
    }

    #[test]
    fn verifier_rejects_replayed_validator_set_after_anchor() {
        let old_keys: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let new_keys: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let expected_hash = HashOf::new(&validator_set_from_keys(&new_keys));
        let mut verifier = BridgeFinalityVerifier::with_validator_set(
            "chain-a".parse().expect("chain id parses"),
            expected_hash,
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        );

        let proof = make_finality_proof("chain-a", 1, 0, &old_keys);
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::UnexpectedValidatorSet { .. }
        ));
    }

    #[test]
    fn verifier_rejects_unexpected_epoch_anchor() {
        let keys: Vec<_> = (0..3).map(|_| KeyPair::random()).collect();
        let expected_hash = HashOf::new(&validator_set_from_keys(&keys));
        let mut verifier = BridgeFinalityVerifier::with_validator_set_and_epoch(
            "chain-a".parse().expect("chain id parses"),
            expected_hash,
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            5,
        );

        let proof = make_finality_proof("chain-a", 1, 4, &keys);
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::UnexpectedEpoch {
                expected: 5,
                got: 4
            }
        ));
    }

    #[test]
    fn verifier_rejects_tampered_validator_set_hash() {
        let keys: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        proof.commit_certificate.validator_set_hash = HashOf::new(&Vec::<PeerId>::new());

        let mut verifier = BridgeFinalityVerifier::new("chain-a".parse().expect("chain id parses"));
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::ValidatorSetHashMismatch { .. }
        ));
    }

    #[test]
    fn verifier_rejects_prior_epoch_after_anchor_rotation() {
        let epoch0_keys: Vec<_> = (0..3).map(|_| KeyPair::random()).collect();
        let epoch1_keys: Vec<_> = (0..3).map(|_| KeyPair::random()).collect();
        let mut verifier = BridgeFinalityVerifier::new("chain-a".parse().expect("chain id parses"));

        let proof_epoch0 = make_finality_proof("chain-a", 1, 0, &epoch0_keys);
        verifier
            .verify(&proof_epoch0)
            .expect("initial proof should set anchors");

        verifier.set_validator_set_and_epoch_anchor(
            HashOf::new(&validator_set_from_keys(&epoch1_keys)),
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            1,
        );

        let replayed = make_finality_proof("chain-a", 2, 0, &epoch0_keys);
        let err = verifier.verify(&replayed).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::UnexpectedEpoch {
                expected: 1,
                got: 0
            }
        ));
    }

    #[test]
    fn verifier_accepts_roster_change_after_anchor_update() {
        let roster_a: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let roster_b: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let mut verifier = BridgeFinalityVerifier::new("chain-a".parse().expect("chain id parses"));

        let proof_a = make_finality_proof("chain-a", 1, 0, &roster_a);
        verifier.verify(&proof_a).expect("first proof accepted");

        let proof_b = make_finality_proof("chain-a", 2, 0, &roster_b);
        let err = verifier.verify(&proof_b).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::UnexpectedValidatorSet { .. }
        ));

        verifier.set_validator_set_anchor(
            HashOf::new(&validator_set_from_keys(&roster_b)),
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        );

        verifier.verify(&proof_b).expect("anchor swap accepted");
    }
}
