//! Exact signed-transaction binding for the global private-settlement carrier.
//!
//! A receipt instruction reached through a trigger, contract, custom executor,
//! proved overlay, or mixed batch must not finalize state. Admission derives a
//! one-shot binding only for one exact direct instruction whose signer and fee
//! intent match the certified manifest.

use iroha_crypto::Hash;
use iroha_data_model::{
    ValidationFail,
    isi::private_settlement::FinalizeAtomicPrivateSettlementV1,
    nexus::PrivateSettlementCommitBundleV1,
    transaction::{Executable, SignedTransaction},
};
use norito::codec::Encode;
use thiserror::Error;

const COMMIT_BUNDLE_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:commit-bundle:v1\0";
const CARRIER_INSTRUCTION_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:carrier-instruction:v1\0";

fn canonical_digest_v1<T: Encode>(domain: &[u8], value: &T) -> Result<Hash, norito::Error> {
    let encoded = norito::encode_canonical(value)?;
    let encoded_len = u64::try_from(encoded.len())
        .map_err(|_| norito::Error::Io(std::io::Error::other("canonical value is too large")))?;
    Ok(Hash::new_from_chunks(&[
        domain,
        &encoded_len.to_le_bytes(),
        encoded.as_slice(),
    ]))
}

/// Hash one exact pre-finality committee-certified bundle.
pub(crate) fn private_settlement_commit_bundle_digest_v1(
    bundle: &PrivateSettlementCommitBundleV1,
) -> Result<Hash, norito::Error> {
    canonical_digest_v1(COMMIT_BUNDLE_DIGEST_DOMAIN_V1, bundle)
}

/// Hash the exact direct carrier instruction authorized by the sponsor.
pub(crate) fn private_settlement_carrier_instruction_digest_v1(
    instruction: &FinalizeAtomicPrivateSettlementV1,
) -> Result<Hash, norito::Error> {
    canonical_digest_v1(CARRIER_INSTRUCTION_DIGEST_DOMAIN_V1, instruction)
}

/// One-shot identity installed from an exact signed carrier transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PrivateSettlementCarrierBindingV1 {
    commit_bundle_digest: Hash,
    instruction_digest: Hash,
    consumed: bool,
}

impl PrivateSettlementCarrierBindingV1 {
    fn new(commit_bundle_digest: Hash, instruction_digest: Hash) -> Self {
        Self {
            commit_bundle_digest,
            instruction_digest,
            consumed: false,
        }
    }

    /// Consume the exact carrier once after all comparisons succeed.
    pub(crate) fn consume(
        &mut self,
        commit_bundle_digest: Hash,
        instruction_digest: Hash,
    ) -> Result<(), PrivateSettlementCarrierBindingErrorV1> {
        if self.commit_bundle_digest != commit_bundle_digest {
            return Err(PrivateSettlementCarrierBindingErrorV1::BundleMismatch);
        }
        if self.instruction_digest != instruction_digest {
            return Err(PrivateSettlementCarrierBindingErrorV1::InstructionMismatch);
        }
        if self.consumed {
            return Err(PrivateSettlementCarrierBindingErrorV1::AlreadyConsumed);
        }
        self.consumed = true;
        Ok(())
    }
}

/// Derive the optional one-shot carrier identity from a signed transaction.
///
/// Ordinary transactions return `Ok(None)`. If a carrier is present, the
/// executable must contain that carrier and nothing else, the signed authority
/// must be the manifest sponsor, and the signed fee intent must be byte-for-byte
/// equal to the manifest fee intent.
pub(crate) fn signed_private_settlement_carrier_binding_v1(
    transaction: &SignedTransaction,
) -> Result<Option<PrivateSettlementCarrierBindingV1>, ValidationFail> {
    let explicit_carrier_count = transaction
        .instructions()
        .explicit_instructions()
        .filter(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<FinalizeAtomicPrivateSettlementV1>()
                .is_some()
        })
        .count();
    if explicit_carrier_count == 0 {
        return Ok(None);
    }

    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(not_permitted());
    };
    if instructions.len() != 1 || explicit_carrier_count != 1 {
        return Err(not_permitted());
    }
    let carrier = instructions[0]
        .as_any()
        .downcast_ref::<FinalizeAtomicPrivateSettlementV1>()
        .ok_or_else(not_permitted)?;
    let manifest = &carrier.commit_bundle.manifest;
    let structural_receipt = carrier
        .commit_bundle
        .clone()
        .into_receipt(manifest.authority_context_height);
    structural_receipt
        .validate_shape()
        .map_err(|_| not_permitted())?;
    if transaction.authority() != &manifest.sponsor
        || transaction.fee_payment_intent() != &manifest.public_fee_intent
    {
        return Err(not_permitted());
    }
    let commit_bundle_digest = private_settlement_commit_bundle_digest_v1(&carrier.commit_bundle)
        .map_err(|error| {
        ValidationFail::InternalError(format!(
            "failed to encode private-settlement commit bundle: {error}"
        ))
    })?;
    let instruction_digest =
        private_settlement_carrier_instruction_digest_v1(carrier).map_err(|error| {
            ValidationFail::InternalError(format!(
                "failed to encode private-settlement carrier: {error}"
            ))
        })?;
    Ok(Some(PrivateSettlementCarrierBindingV1::new(
        commit_bundle_digest,
        instruction_digest,
    )))
}

fn not_permitted() -> ValidationFail {
    ValidationFail::NotPermitted(
        "private-settlement finalization requires one exact sponsor-signed direct carrier with the committed fee intent"
            .to_owned(),
    )
}

/// Closed failure while consuming a transaction-scoped carrier binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum PrivateSettlementCarrierBindingErrorV1 {
    /// No exact direct carrier was installed from the current signed payload.
    #[error("the current signed transaction has no bound private-settlement carrier")]
    MissingBinding,
    /// The certified bundle differs from the exact signed carrier.
    #[error("private-settlement commit bundle differs from the signed carrier")]
    BundleMismatch,
    /// The complete instruction differs from the exact signed carrier.
    #[error("private-settlement instruction differs from the signed carrier")]
    InstructionMismatch,
    /// A nested or repeated execution attempted to replay the carrier.
    #[error("private-settlement carrier was already consumed")]
    AlreadyConsumed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId, account::AccountId, block::BlockHeader, nexus::AtomicPrivateSettlementV1,
    };

    #[test]
    fn one_shot_binding_rejects_substitution_and_replay() {
        let bundle_digest = Hash::new(b"bundle-a");
        let instruction_digest = Hash::new(b"instruction-a");
        let mut binding = PrivateSettlementCarrierBindingV1::new(bundle_digest, instruction_digest);

        assert_eq!(
            binding.consume(Hash::new(b"bundle-b"), instruction_digest),
            Err(PrivateSettlementCarrierBindingErrorV1::BundleMismatch)
        );
        assert_eq!(
            binding.consume(bundle_digest, Hash::new(b"instruction-b")),
            Err(PrivateSettlementCarrierBindingErrorV1::InstructionMismatch)
        );
        assert_eq!(binding.consume(bundle_digest, instruction_digest), Ok(()));
        assert_eq!(
            binding.consume(bundle_digest, instruction_digest),
            Err(PrivateSettlementCarrierBindingErrorV1::AlreadyConsumed)
        );
    }

    #[test]
    fn commit_bundle_digest_is_domain_separated_and_deterministic() {
        let bundle = PrivateSettlementCommitBundleV1 {
            version: 1,
            manifest: AtomicPrivateSettlementV1 {
                version: 1,
                network_id: NetworkId::from_genesis_hash(
                    HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"network")),
                ),
                bundle_id: Hash::new(b"invalid-fixture-bundle"),
                authority_context_height: 1,
                expiry_height: 2,
                sponsor: AccountId::new(
                    KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519)
                        .public_key()
                        .clone(),
                ),
                public_fee_intent: iroha_data_model::transaction::FeePaymentIntent::authority(
                    Vec::new(),
                    None,
                ),
                fee_intent_digest: Hash::new(b"fee"),
                reimbursement_terms_commitment: Hash::new(b"reimbursement"),
                reimbursement_leg_ordinal: 0,
                legs: Vec::new(),
            },
            authority_catalog: Vec::new(),
            legs: Vec::new(),
        };
        let first = private_settlement_commit_bundle_digest_v1(&bundle)
            .expect("fixture encodes canonically");
        let second = private_settlement_commit_bundle_digest_v1(&bundle)
            .expect("fixture encodes canonically");
        assert_eq!(first, second);
        assert_ne!(
            first,
            Hash::new(&norito::encode_canonical(&bundle).unwrap())
        );
    }
}
