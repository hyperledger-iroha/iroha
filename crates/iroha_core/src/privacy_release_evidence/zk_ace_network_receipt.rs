//! Canonical receipt for the non-shipping ZK-ACE committed-network canary.
//!
//! The receipt deliberately omits a source revision and the final public pin:
//! either would make the receipt digest self-referential. Instead it binds the
//! exact immutable candidate profile, all four previously reviewed stage pins,
//! and the complete canonical network semantics needed to review and populate
//! the still-zero network-semantic pin.

use crate::privacy_engines::zk_ace::{
    ZK_ACE_RELEASE_STAGE_EVIDENCE_SHA256_V2, zk_ace_compiled_profile_digest_v1,
    zk_ace_public_release_pins_complete_v2, zk_ace_release_evidence_pins_complete_v2,
};
use iroha_data_model::{
    NetworkId,
    prelude::{AccountId, AssetDefinitionId, PeerId},
    privacy::{
        PrivacyNullifierV1, PrivacyPolicyIdV1, PrivacyStatementDigestV1,
        PrivacyZkAceReplayNullifierProvenanceV1,
    },
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Canonical receipt schema version.
pub const PRIVACY_ZK_ACE_NETWORK_SEMANTIC_RECEIPT_VERSION_V1: u16 = 1;
/// Exact validator count required by the Taira canary.
pub const PRIVACY_ZK_ACE_NETWORK_SEMANTIC_VALIDATOR_COUNT_V1: usize = 4;
/// Exact activation notice required by the release plan.
pub const PRIVACY_ZK_ACE_NETWORK_SEMANTIC_ACTIVATION_NOTICE_BLOCKS_V1: u64 = 300;
/// Conservative outer bound for the complete canonical receipt.
pub const PRIVACY_ZK_ACE_NETWORK_SEMANTIC_RECEIPT_MAX_BYTES_V1: usize = 32 * 1024;

/// Explicit execution corridor authenticated by this receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum PrivacyZkAceNetworkSemanticCorridorV1 {
    /// Candidate execution compiled only by `privacy-release-evidence`.
    NonshippingPrivacyReleaseEvidenceCandidate,
    /// Ordinary public execution after the reviewed receipt pin is populated.
    PublicPostPin,
}

/// Canonical transaction and carrier-block binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PrivacyZkAceCanonicalTransactionAnchorV1 {
    /// Hash of the exact signed transaction.
    pub signed_transaction_hash: [u8; 32],
    /// Hash of its exact external transaction entrypoint.
    pub entrypoint_hash: [u8; 32],
    /// SHA-256 of the canonical Norito `SignedTransaction`.
    pub canonical_transaction_sha256: [u8; 32],
    /// Exact committed carrier height.
    pub carrier_height: u64,
    /// Exact canonical carrier block hash.
    pub carrier_block_hash: [u8; 32],
}

/// Governed activation registration and 300-block notice binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PrivacyZkAceActivationReceiptV1 {
    /// Canonical activation-registration transaction and carrier.
    pub registration: PrivacyZkAceCanonicalTransactionAnchorV1,
    /// Height encoded in the registered `Proposed` lifecycle.
    pub registration_height: u64,
    /// Exact notice interval.
    pub activation_notice_blocks: u64,
    /// Height at which the lifecycle became active.
    pub activation_height: u64,
}

/// Canonical successful 19-unit transparent-transfer binding.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PrivacyZkAceAppliedTransferReceiptV1 {
    /// Canonical signed transfer and its committed carrier.
    pub transaction: PrivacyZkAceCanonicalTransactionAnchorV1,
    /// SHA-256 of the exact native proof wire.
    pub proof_sha256: [u8; 32],
    /// SHA-256 of the canonical Norito typed statement.
    pub canonical_statement_sha256: [u8; 32],
    /// Native statement digest committed in the envelope and provenance.
    pub statement_digest: PrivacyStatementDigestV1,
    /// Governed policy lineage.
    pub policy_id: PrivacyPolicyIdV1,
    /// Exact consumed replay nullifier.
    pub replay_nullifier: PrivacyNullifierV1,
    /// Public source account.
    pub source: AccountId,
    /// Public destination account.
    pub destination: AccountId,
    /// Exact transferred asset.
    pub asset_definition_id: AssetDefinitionId,
    /// Exact canonical amount.
    pub amount: u128,
    /// Source balance before transfer.
    pub source_balance_before: u128,
    /// Destination balance before transfer.
    pub destination_balance_before: u128,
    /// Source balance after transfer.
    pub source_balance_after: u128,
    /// Destination balance after transfer.
    pub destination_balance_after: u128,
}

/// Stable typed rejection class required for the replay transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum PrivacyZkAceReplayRejectionKindV1 {
    /// `SubmitPrivacyProofV1` rejected an already-consumed ZK-ACE nullifier.
    ConsumedReplayNullifier,
}

/// Canonical independently randomized replay and committed rejection binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PrivacyZkAceRejectedReplayReceiptV1 {
    /// Canonical signed replay and its committed rejection carrier.
    pub transaction: PrivacyZkAceCanonicalTransactionAnchorV1,
    /// SHA-256 of the independently randomized native proof wire.
    pub proof_sha256: [u8; 32],
    /// The exact closed typed rejection class.
    pub rejection_kind: PrivacyZkAceReplayRejectionKindV1,
    /// SHA-256 of the canonical Norito committed `TransactionRejectionReason`.
    pub canonical_typed_rejection_sha256: [u8; 32],
    /// Source balance after the rejected replay.
    pub source_balance_after_replay: u128,
    /// Destination balance after the rejected replay.
    pub destination_balance_after_replay: u128,
}

/// One validator's finalized view of the consumed replay-nullifier marker.
///
/// Keeping the validator and provenance in one canonical value prevents a
/// consumer from accidentally re-pairing two parallel arrays after decoding.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PrivacyZkAceValidatorReplayObservationV1 {
    /// Validator that served the finalized typed query.
    pub validator: PeerId,
    /// Exact finalized marker returned by that validator.
    pub provenance: PrivacyZkAceReplayNullifierProvenanceV1,
}

/// Complete bounded committed-network receipt reviewed before public release.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PrivacyZkAceNetworkSemanticReceiptV1 {
    /// Exact schema version.
    pub version: u16,
    /// Explicit candidate or post-pin execution corridor.
    pub corridor: PrivacyZkAceNetworkSemanticCorridorV1,
    /// Frozen digest of every exact candidate profile field.
    pub candidate_profile_digest: [u8; 32],
    /// Four reviewed stage pins in canonical evidence-case order.
    pub release_stage_evidence_sha256: [[u8; 32]; 4],
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Exact canonical genesis block hash.
    pub genesis_block_hash: [u8; 32],
    /// Governed activation registration and notice.
    pub activation: PrivacyZkAceActivationReceiptV1,
    /// Successful canonical transfer.
    pub transfer: PrivacyZkAceAppliedTransferReceiptV1,
    /// Four finalized replay-marker observations in strict validator order.
    pub replay_nullifier_finality: [PrivacyZkAceValidatorReplayObservationV1;
        PRIVACY_ZK_ACE_NETWORK_SEMANTIC_VALIDATOR_COUNT_V1],
    /// Independently randomized committed replay rejection.
    pub replay: PrivacyZkAceRejectedReplayReceiptV1,
}

impl PrivacyZkAceNetworkSemanticReceiptV1 {
    /// Decode one exact bounded canonical Norito receipt.
    ///
    /// # Errors
    ///
    /// Rejects oversized, malformed, non-canonical, or semantically invalid
    /// receipt bytes before they may be reviewed as a public-pin candidate.
    pub fn decode_canonical_norito(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.is_empty() || bytes.len() > PRIVACY_ZK_ACE_NETWORK_SEMANTIC_RECEIPT_MAX_BYTES_V1 {
            return Err("ZK-ACE network-semantic receipt bytes are empty or oversized");
        }
        let receipt = norito::decode_from_bytes::<Self>(bytes)
            .map_err(|_| "ZK-ACE network-semantic receipt decoding failed")?;
        let canonical = norito::to_bytes(&receipt)
            .map_err(|_| "ZK-ACE network-semantic receipt re-encoding failed")?;
        if canonical != bytes {
            return Err("ZK-ACE network-semantic receipt is not exact canonical Norito");
        }
        receipt.validate()?;
        Ok(receipt)
    }

    /// Validate every fixed release semantic before hashing or persistence.
    ///
    /// # Errors
    ///
    /// Returns a stable fail-closed reason for the first mismatched binding.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version != PRIVACY_ZK_ACE_NETWORK_SEMANTIC_RECEIPT_VERSION_V1 {
            return Err("ZK-ACE network-semantic receipt version mismatch");
        }
        if self.candidate_profile_digest != zk_ace_compiled_profile_digest_v1() {
            return Err("ZK-ACE network-semantic candidate profile digest mismatch");
        }
        if !zk_ace_release_evidence_pins_complete_v2()
            || self.release_stage_evidence_sha256 != ZK_ACE_RELEASE_STAGE_EVIDENCE_SHA256_V2
        {
            return Err("ZK-ACE network-semantic stage pins are incomplete or substituted");
        }
        if self.corridor == PrivacyZkAceNetworkSemanticCorridorV1::PublicPostPin
            && !zk_ace_public_release_pins_complete_v2()
        {
            return Err("ZK-ACE public post-pin receipt corridor is not available");
        }
        if self.network_id.as_bytes() != &self.genesis_block_hash
            || is_zero_digest(&self.genesis_block_hash)
        {
            return Err("ZK-ACE network-semantic NetworkId/genesis binding mismatch");
        }
        validate_anchor(&self.activation.registration)?;
        if self.activation.registration_height == 0
            || self.activation.registration.carrier_height != self.activation.registration_height
            || self.activation.activation_notice_blocks
                != PRIVACY_ZK_ACE_NETWORK_SEMANTIC_ACTIVATION_NOTICE_BLOCKS_V1
            || self
                .activation
                .registration_height
                .checked_add(self.activation.activation_notice_blocks)
                != Some(self.activation.activation_height)
        {
            return Err("ZK-ACE network-semantic activation notice binding mismatch");
        }
        validate_anchor(&self.transfer.transaction)?;
        if is_zero_digest(&self.transfer.proof_sha256)
            || is_zero_digest(&self.transfer.canonical_statement_sha256)
            || self.transfer.statement_digest.is_zero()
            || self.transfer.policy_id.is_zero()
            || self.transfer.replay_nullifier.is_zero()
            || self.transfer.source == self.transfer.destination
            || self.transfer.amount != 19
            || self.transfer.source_balance_before != 100
            || self.transfer.destination_balance_before != 0
            || self.transfer.source_balance_after != 81
            || self.transfer.destination_balance_after != 19
            || self.transfer.transaction.carrier_height < self.activation.activation_height
        {
            return Err("ZK-ACE network-semantic canonical transfer mismatch");
        }
        let first_provenance = &self.replay_nullifier_finality[0].provenance;
        for (index, observation) in self.replay_nullifier_finality.iter().enumerate() {
            if index > 0
                && self.replay_nullifier_finality[index - 1].validator >= observation.validator
            {
                return Err("ZK-ACE network-semantic validators are not strictly ordered");
            }
            let provenance = &observation.provenance;
            provenance
                .validate()
                .map_err(|_| "ZK-ACE network-semantic replay provenance is invalid")?;
            if provenance.network_id != self.network_id
                || provenance.policy_id != self.transfer.policy_id
                || provenance.replay_nullifier != self.transfer.replay_nullifier
                || provenance.statement_digest != self.transfer.statement_digest
                || provenance.admitted_at_height != self.transfer.transaction.carrier_height
                || provenance.action_index != 0
                || provenance.policy_record_digest != first_provenance.policy_record_digest
                || provenance.statement_digest != first_provenance.statement_digest
                || provenance.admitted_at_height != first_provenance.admitted_at_height
                || provenance.action_index != first_provenance.action_index
                || provenance.finalized_height < self.replay.transaction.carrier_height
            {
                return Err("ZK-ACE network-semantic replay provenance binding mismatch");
            }
        }
        validate_anchor(&self.replay.transaction)?;
        if self.replay.transaction.signed_transaction_hash
            == self.transfer.transaction.signed_transaction_hash
            || self.replay.transaction.entrypoint_hash == self.transfer.transaction.entrypoint_hash
            || self.replay.transaction.canonical_transaction_sha256
                == self.transfer.transaction.canonical_transaction_sha256
            || self.replay.proof_sha256 == self.transfer.proof_sha256
            || is_zero_digest(&self.replay.proof_sha256)
            || is_zero_digest(&self.replay.canonical_typed_rejection_sha256)
            || self.replay.transaction.carrier_height < self.transfer.transaction.carrier_height
            || self.replay.source_balance_after_replay != self.transfer.source_balance_after
            || self.replay.destination_balance_after_replay
                != self.transfer.destination_balance_after
        {
            return Err("ZK-ACE network-semantic replay rejection binding mismatch");
        }
        Ok(())
    }

    /// Return the complete canonical Norito receipt bytes after validation.
    ///
    /// # Errors
    ///
    /// Rejects an invalid receipt, encoding failure, or an encoded receipt
    /// exceeding the fixed outer bound.
    pub fn canonical_norito_bytes(&self) -> Result<Vec<u8>, &'static str> {
        self.validate()?;
        let bytes = norito::to_bytes(self)
            .map_err(|_| "ZK-ACE network-semantic receipt encoding failed")?;
        if bytes.len() > PRIVACY_ZK_ACE_NETWORK_SEMANTIC_RECEIPT_MAX_BYTES_V1 {
            return Err("ZK-ACE network-semantic receipt exceeds its canonical byte bound");
        }
        Ok(bytes)
    }

    /// SHA-256 over the complete canonical Norito receipt.
    ///
    /// This is the value reviewed for the future public network-semantic pin.
    /// It intentionally is not a field of the receipt itself.
    ///
    /// # Errors
    ///
    /// Propagates receipt validation or canonical encoding failure.
    pub fn canonical_norito_sha256(&self) -> Result<[u8; 32], &'static str> {
        Ok(Sha256::digest(self.canonical_norito_bytes()?).into())
    }
}

fn validate_anchor(anchor: &PrivacyZkAceCanonicalTransactionAnchorV1) -> Result<(), &'static str> {
    if is_zero_digest(&anchor.signed_transaction_hash)
        || is_zero_digest(&anchor.entrypoint_hash)
        || is_zero_digest(&anchor.canonical_transaction_sha256)
        || anchor.carrier_height == 0
        || is_zero_digest(&anchor.carrier_block_hash)
    {
        return Err("ZK-ACE network-semantic transaction anchor is incomplete");
    }
    Ok(())
}

fn is_zero_digest(digest: &[u8; 32]) -> bool {
    digest.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        block::BlockHeader, domain::DomainId, name::Name, privacy::PrivacyZkAcePolicyRecordDigestV1,
    };
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use std::str::FromStr as _;

    fn hash(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn block_hash(byte: u8) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed(hash(byte)))
    }

    fn anchor(byte: u8, height: u64) -> PrivacyZkAceCanonicalTransactionAnchorV1 {
        PrivacyZkAceCanonicalTransactionAnchorV1 {
            signed_transaction_hash: hash(byte),
            entrypoint_hash: hash(byte.wrapping_add(1)),
            canonical_transaction_sha256: hash(byte.wrapping_add(2)),
            carrier_height: height,
            carrier_block_hash: hash(byte.wrapping_add(3)),
        }
    }

    fn fixture() -> PrivacyZkAceNetworkSemanticReceiptV1 {
        let genesis = hash(0x31);
        let network_id = NetworkId::from_genesis_hash(block_hash(0x31));
        let policy_id = PrivacyPolicyIdV1::new(hash(0x41));
        let replay_nullifier = PrivacyNullifierV1::new(hash(0x42));
        let statement_digest = PrivacyStatementDigestV1::new(hash(0x43));
        let mut validators = (1_u8..=4)
            .map(|seed| {
                PeerId::new(
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                        .expect("derive validator key")
                        .public_key()
                        .clone(),
                )
            })
            .collect::<Vec<_>>();
        validators.sort();
        let validators: [PeerId; PRIVACY_ZK_ACE_NETWORK_SEMANTIC_VALIDATOR_COUNT_V1] =
            validators.try_into().expect("four validators");
        let provenance = PrivacyZkAceReplayNullifierProvenanceV1 {
            network_id,
            policy_id,
            replay_nullifier,
            policy_record_digest: PrivacyZkAcePolicyRecordDigestV1::new(hash(0x44)),
            statement_digest,
            admitted_at_height: 315,
            action_index: 0,
            finalized_height: 318,
            finalized_block_hash: block_hash(0x45),
        };
        PrivacyZkAceNetworkSemanticReceiptV1 {
            version: PRIVACY_ZK_ACE_NETWORK_SEMANTIC_RECEIPT_VERSION_V1,
            corridor:
                PrivacyZkAceNetworkSemanticCorridorV1::NonshippingPrivacyReleaseEvidenceCandidate,
            candidate_profile_digest: zk_ace_compiled_profile_digest_v1(),
            release_stage_evidence_sha256: ZK_ACE_RELEASE_STAGE_EVIDENCE_SHA256_V2,
            network_id,
            genesis_block_hash: genesis,
            activation: PrivacyZkAceActivationReceiptV1 {
                registration: anchor(0x51, 10),
                registration_height: 10,
                activation_notice_blocks:
                    PRIVACY_ZK_ACE_NETWORK_SEMANTIC_ACTIVATION_NOTICE_BLOCKS_V1,
                activation_height: 310,
            },
            transfer: PrivacyZkAceAppliedTransferReceiptV1 {
                transaction: anchor(0x61, 315),
                proof_sha256: hash(0x65),
                canonical_statement_sha256: hash(0x66),
                statement_digest,
                policy_id,
                replay_nullifier,
                source: ALICE_ID.clone(),
                destination: BOB_ID.clone(),
                asset_definition_id: AssetDefinitionId::derive_from_components(
                    DomainId::try_new("wonderland", "universal").expect("test domain"),
                    Name::from_str("zkace").expect("test asset"),
                ),
                amount: 19,
                source_balance_before: 100,
                destination_balance_before: 0,
                source_balance_after: 81,
                destination_balance_after: 19,
            },
            replay_nullifier_finality: validators.map(|validator| {
                PrivacyZkAceValidatorReplayObservationV1 {
                    validator,
                    provenance,
                }
            }),
            replay: PrivacyZkAceRejectedReplayReceiptV1 {
                transaction: anchor(0x71, 318),
                proof_sha256: hash(0x75),
                rejection_kind: PrivacyZkAceReplayRejectionKindV1::ConsumedReplayNullifier,
                canonical_typed_rejection_sha256: hash(0x76),
                source_balance_after_replay: 81,
                destination_balance_after_replay: 19,
            },
        }
    }

    #[test]
    fn canonical_receipt_is_bounded_deterministic_and_mutation_closed() {
        let receipt = fixture();
        receipt.validate().expect("valid canonical receipt");
        let bytes = receipt.canonical_norito_bytes().expect("canonical Norito");
        assert!(bytes.len() <= PRIVACY_ZK_ACE_NETWORK_SEMANTIC_RECEIPT_MAX_BYTES_V1);
        assert_eq!(
            receipt.canonical_norito_sha256().expect("receipt digest"),
            Sha256::digest(&bytes).into()
        );
        assert_eq!(
            PrivacyZkAceNetworkSemanticReceiptV1::decode_canonical_norito(&bytes)
                .expect("decode exact canonical receipt"),
            receipt
        );
        let mut replay_reuses_proof = receipt.clone();
        replay_reuses_proof.replay.proof_sha256 = replay_reuses_proof.transfer.proof_sha256;
        assert!(replay_reuses_proof.validate().is_err());
        let mut premature_public = receipt.clone();
        premature_public.corridor = PrivacyZkAceNetworkSemanticCorridorV1::PublicPostPin;
        assert!(premature_public.validate().is_err());
        let mut unsorted_validators = receipt;
        unsorted_validators.replay_nullifier_finality.swap(0, 1);
        assert!(unsorted_validators.validate().is_err());
    }
}
