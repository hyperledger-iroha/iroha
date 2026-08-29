//! FASTPQ-specific data structures shared between the host and prover.
use crate::{account::AccountId, asset::id::AssetDefinitionId};
use iroha_crypto::Hash;
use iroha_primitives::{
    bigint::BigInt,
    numeric::{Numeric, Quantity},
};
use iroha_schema::IntoSchema;
use std::collections::{BTreeMap, BTreeSet};
/// Metadata key storing Norito-encoded [`TransferTranscript`] collections for FASTPQ gadgets.
pub const TRANSFER_TRANSCRIPTS_METADATA_KEY: &str = "transfer_transcripts";
/// Canonical first-release Norito schema identity for [`FastpqTransitionBatch`].
pub const FASTPQ_TRANSITION_BATCH_SCHEMA_NAME: &str =
    "iroha_data_model::fastpq::FastpqTransitionBatchV1";
/// Transcript describing one or more deterministic asset transfers within a transaction.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
pub struct TransferTranscript {
    /// Hash of the transaction entrypoint (`hash_as_entrypoint`) that emitted this transcript.
    pub batch_hash: Hash,
    /// Grouped transfer deltas covered by the transcript.
    pub deltas: Vec<TransferDeltaTranscript>,
    /// Host-side digest of the authority set (signers, quorum, etc.).
    pub authority_digest: Hash,
    /// Optional Poseidon digest of the preimage `(from, to, asset, amount, batch_hash)`.
    ///
    /// Present for single-delta transcripts; omitted for multi-delta batches.
    pub poseidon_preimage_digest: Option<Hash>,
}
/// Per-transfer delta describing the balance change for the sender and receiver.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
pub struct TransferDeltaTranscript {
    /// Source account.
    pub from_account: AccountId,
    /// Destination account.
    pub to_account: AccountId,
    /// Asset definition being transferred.
    pub asset_definition: AssetDefinitionId,
    /// Amount being transferred.
    pub amount: Quantity,
    /// Sender balance before the transfer.
    pub from_balance_before: Quantity,
    /// Sender balance after the transfer.
    pub from_balance_after: Quantity,
    /// Receiver balance before the transfer.
    pub to_balance_before: Quantity,
    /// Receiver balance after the transfer.
    pub to_balance_after: Quantity,
    /// Sender sparse-Merkle update witness from the batch root before the debit
    /// to the intermediate root after the debit.
    pub from_smt_witness: TransferSmtWitness,
    /// Receiver sparse-Merkle update witness from the intermediate root after
    /// the debit to the batch root after the credit.
    pub to_smt_witness: TransferSmtWitness,
}
impl TransferDeltaTranscript {
    /// Attach sparse Merkle update witnesses for sender and receiver accounts.
    #[must_use]
    pub fn with_smt_witnesses(
        mut self,
        from_witness: TransferSmtWitness,
        to_witness: TransferSmtWitness,
    ) -> Self {
        self.from_smt_witness = from_witness;
        self.to_smt_witness = to_witness;
        self
    }
    /// Return the common decimal scale used to normalize this delta into FASTPQ witness units.
    #[must_use]
    pub fn normalized_scale(&self) -> u32 {
        [
            trimmed_scale(&self.amount),
            trimmed_scale(&self.from_balance_before),
            trimmed_scale(&self.from_balance_after),
            trimmed_scale(&self.to_balance_before),
            trimmed_scale(&self.to_balance_after),
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
    }
}

/// Derive one stable decimal witness scale per asset for a transcript sequence.
///
/// Repeated balance keys deliberately contribute their scale only on first use. Later transcript
/// entries can carry stale balance snapshots that the witness materializer rewrites while chaining
/// updates; allowing those stale values to select the scale would make the same balance change its
/// integer interpretation midway through the batch. Transfer amounts always contribute because
/// they are never rewritten.
#[must_use]
pub fn transfer_asset_scales(
    transcripts: &[TransferTranscript],
) -> BTreeMap<AssetDefinitionId, u32> {
    let mut scales = BTreeMap::<AssetDefinitionId, u32>::new();
    let mut seeded_balances = BTreeSet::<(AssetDefinitionId, AccountId)>::new();
    for transcript in transcripts {
        for delta in &transcript.deltas {
            let scale = scales.entry(delta.asset_definition.clone()).or_default();
            *scale = (*scale).max(trimmed_scale(&delta.amount));

            for (account, before, after) in [
                (
                    &delta.from_account,
                    &delta.from_balance_before,
                    &delta.from_balance_after,
                ),
                (
                    &delta.to_account,
                    &delta.to_balance_before,
                    &delta.to_balance_after,
                ),
            ] {
                if seeded_balances.insert((delta.asset_definition.clone(), account.clone())) {
                    *scale = (*scale)
                        .max(trimmed_scale(before))
                        .max(trimmed_scale(after));
                }
            }
        }
    }
    scales
}
/// Sparse-Merkle update witness for one transfer participant.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
pub struct TransferSmtWitness {
    /// Root before applying this participant update.
    pub root_before: [u8; 32],
    /// Root after applying this participant update.
    pub root_after: [u8; 32],
    /// Bitset describing the direction taken at each level (LSB-first per byte).
    pub path_bits: Vec<u8>,
    /// Sibling node hashes encountered along the path.
    pub siblings: Vec<[u8; 32]>,
}
impl TransferSmtWitness {
    /// Construct a typed sparse-Merkle update witness.
    #[must_use]
    pub fn new(
        root_before: [u8; 32],
        root_after: [u8; 32],
        path_bits: Vec<u8>,
        siblings: Vec<[u8; 32]>,
    ) -> Self {
        Self {
            root_before,
            root_after,
            path_bits,
            siblings,
        }
    }
}
fn trimmed_scale(value: &Quantity) -> u32 {
    value.scale()
}
/// Normalize an exact decimal into deterministic integer witness units for FASTPQ.
///
/// The caller chooses the target decimal scale. Values are scaled up by powers of ten until they
/// share that target scale, then converted into a non-negative `u64`.
#[must_use]
pub fn normalized_numeric_to_u64(value: &Numeric, target_scale: u32) -> Option<u64> {
    let value = value.clone().trim_trailing_zeros();
    if value.mantissa().is_negative() || value.scale() > target_scale {
        return None;
    }
    let scale_delta = target_scale - value.scale();
    let factor = BigInt::pow10(scale_delta)?;
    let scaled = value.mantissa().checked_mul(&factor).ok()?;
    scaled.to_string().parse::<u64>().ok()
}
/// Canonical FASTPQ transition batch recorded in execution witnesses.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
#[norito(schema_name = "iroha_data_model::fastpq::FastpqTransitionBatchV1")]
pub struct FastpqTransitionBatch {
    /// Parameter set name (`fastpq-state-transition-stark-v1`).
    pub parameter: String,
    /// Public inputs committed by the prover and replayed by the verifier.
    pub public_inputs: FastpqPublicInputs,
    /// Ordered transitions the prover must replay.
    pub transitions: Vec<FastpqStateTransition>,
    /// Arbitrary metadata (e.g., entry hash, transcript count).
    pub metadata: BTreeMap<String, Vec<u8>>,
}
/// Canonical FASTPQ state transition.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
pub struct FastpqStateTransition {
    /// Schema-qualified logical key (asset/account path).
    pub key: Vec<u8>,
    /// Pre-state value prior to executing the transition.
    pub pre_value: Vec<u8>,
    /// Post-state value after executing the transition.
    pub post_value: Vec<u8>,
    /// Operation selector describing the transition semantics.
    pub operation: FastpqOperationKind,
}
/// FASTPQ operation selector recorded in batches.
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
#[norito(tag = "kind", content = "payload")]
pub enum FastpqOperationKind {
    /// Asset transfer between two existing accounts.
    #[codec(index = 16)]
    Transfer,
    /// Opaque metadata effect whose meaning is authenticated by its outer statement.
    #[codec(index = 17)]
    MetaSet,
}
/// Public inputs committed by the FASTPQ prover.
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
pub struct FastpqPublicInputs {
    /// Data-space identifier (little-endian UUID bytes).
    pub dsid: [u8; 16],
    /// Slot timestamp (nanoseconds since epoch).
    pub slot: u64,
    /// Sparse Merkle tree root before executing the batch.
    pub old_root: [u8; 32],
    /// Sparse Merkle tree root after executing the batch.
    pub new_root: [u8; 32],
    /// Permission table commitment for this slot.
    pub perm_root: [u8; 32],
    /// Transaction set hash recorded by the scheduler.
    pub tx_set_hash: [u8; 32],
}
/// Bundle of transcripts keyed by the lane transaction-entrypoint identity.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    IntoSchema,
)]
pub struct TransferTranscriptBundle {
    /// Entry identity associated with the transcripts on the enclosing evidence surface.
    ///
    /// Ordinary execution witnesses use the FASTPQ execution-call identity (the inner signed
    /// transaction hash for a sealed reveal). Autonomous merge-lane carriers instead bind this
    /// field to their canonical outer entrypoint identity; each transcript's `batch_hash` remains
    /// the execution-call hash emitted by execution.
    pub entry_hash: Hash,
    /// Recorded transcripts for the entry.
    pub transcripts: Vec<TransferTranscript>,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{account::AccountId, asset::id::AssetDefinitionId, domain::DomainId, name::Name};
    use iroha_primitives::numeric::Numeric;
    use norito::codec::{Decode, Encode};
    use std::str::FromStr;
    const SIGNATORY: &str =
        "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";
    fn account(label: &str) -> AccountId {
        let _ = label;
        AccountId::new(SIGNATORY.parse().expect("valid public key"))
    }
    fn asset(label: &str) -> AssetDefinitionId {
        let name = Name::from_str(label).expect("valid asset name");
        let domain = DomainId::try_new("wonderland", "universal").expect("valid domain id");
        AssetDefinitionId::derive_from_components(domain, name)
    }
    fn quantity<T: Into<BigInt>>(mantissa: T, scale: u32) -> Quantity {
        Quantity::try_from_numeric(Numeric::new(mantissa, scale))
            .expect("non-negative canonical quantity")
    }

    #[test]
    fn operation_wire_indices_reject_the_pre_release_enum() {
        assert_eq!(FastpqOperationKind::Transfer.encode(), 16_u32.to_le_bytes());
        assert_eq!(FastpqOperationKind::MetaSet.encode(), 17_u32.to_le_bytes());
        for retired in 0_u32..=5 {
            assert!(
                FastpqOperationKind::decode(&mut retired.to_le_bytes().as_slice()).is_err(),
                "retired pre-release operation index {retired} must not decode"
            );
        }
    }

    #[test]
    fn transition_batch_schema_rejects_the_pre_release_header() {
        let expected = norito::core::schema_hash_for_name(FASTPQ_TRANSITION_BATCH_SCHEMA_NAME);
        assert_eq!(
            <FastpqTransitionBatch as norito::NoritoSerialize>::schema_hash(),
            expected
        );
        assert_eq!(
            <FastpqTransitionBatch as norito::NoritoDeserialize<'static>>::schema_hash(),
            expected
        );
        let batch = FastpqTransitionBatch {
            parameter: "fastpq-state-transition-stark-v1".into(),
            public_inputs: FastpqPublicInputs {
                dsid: [0; 16],
                slot: 0,
                old_root: [0; 32],
                new_root: [0; 32],
                perm_root: [0; 32],
                tx_set_hash: [0; 32],
            },
            transitions: Vec::new(),
            metadata: BTreeMap::new(),
        };
        let mut encoded = norito::to_bytes(&batch).expect("encode release batch DTO");
        assert_eq!(&encoded[6..22], expected.as_slice());
        let pre_release =
            norito::core::schema_hash_for_name("iroha_data_model::fastpq::FastpqTransitionBatch");
        encoded[6..22].copy_from_slice(&pre_release);
        assert!(
            norito::decode_from_bytes::<FastpqTransitionBatch>(&encoded).is_err(),
            "the pre-release batch DTO schema must not decode as release V1"
        );
    }

    #[derive(Encode)]
    struct ForgedTransferDeltaTranscript {
        from_account: AccountId,
        to_account: AccountId,
        asset_definition: AssetDefinitionId,
        amount: Numeric,
        from_balance_before: Numeric,
        from_balance_after: Numeric,
        to_balance_before: Numeric,
        to_balance_after: Numeric,
        from_smt_witness: TransferSmtWitness,
        to_smt_witness: TransferSmtWitness,
    }
    #[test]
    fn transfer_delta_transcript_attaches_smt_witnesses() {
        let delta = TransferDeltaTranscript {
            from_account: account("alice"),
            to_account: account("bob"),
            asset_definition: asset("xor"),
            amount: quantity(10, 0),
            from_balance_before: quantity(100, 0),
            from_balance_after: quantity(90, 0),
            to_balance_before: quantity(50, 0),
            to_balance_after: quantity(60, 0),
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        };
        let from_witness = TransferSmtWitness::new([1; 32], [2; 32], vec![0xAA], vec![[3; 32]]);
        let to_witness = TransferSmtWitness::new([2; 32], [4; 32], vec![0x55], vec![[5; 32]]);
        let updated = delta.with_smt_witnesses(from_witness.clone(), to_witness.clone());
        assert_eq!(updated.from_smt_witness, from_witness);
        assert_eq!(updated.to_smt_witness, to_witness);
    }
    #[test]
    fn transfer_delta_normalized_scale_uses_highest_numeric_scale() {
        let delta = TransferDeltaTranscript {
            from_account: account("alice"),
            to_account: account("bob"),
            asset_definition: asset("xor"),
            amount: quantity(5, 1),
            from_balance_before: quantity(1, 0),
            from_balance_after: quantity(5, 1),
            to_balance_before: quantity(0, 0),
            to_balance_after: quantity(5, 1),
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        };
        assert_eq!(delta.normalized_scale(), 1);
    }
    #[test]
    fn transfer_delta_normalized_scale_trims_trailing_zero_padding() {
        let delta = TransferDeltaTranscript {
            from_account: account("alice"),
            to_account: account("bob"),
            asset_definition: asset("xor"),
            amount: quantity(11, 3),
            from_balance_before: quantity(120_000_000_000_000_000_000_000_i128, 18),
            from_balance_after: quantity(119_999_989_000_000_000_000_000_i128, 18),
            to_balance_before: Quantity::zero(),
            to_balance_after: quantity(11_000_000_000_000_000_i128, 18),
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        };
        assert_eq!(delta.normalized_scale(), 3);
    }
    #[test]
    fn normalized_numeric_to_u64_scales_to_requested_precision() {
        let whole = quantity(1, 0);
        let fractional = quantity(5, 1);
        assert_eq!(normalized_numeric_to_u64(whole.as_numeric(), 1), Some(10));
        assert_eq!(
            normalized_numeric_to_u64(fractional.as_numeric(), 1),
            Some(5)
        );
    }
    #[test]
    fn normalized_numeric_to_u64_accepts_trimmed_trailing_zero_scale() {
        let padded = quantity(120_000_000_000_000_000_000_000_i128, 18);
        assert_eq!(
            normalized_numeric_to_u64(padded.as_numeric(), 3),
            Some(120_000_000)
        );
    }

    #[test]
    fn transfer_asset_scale_ignores_stale_repeated_balance_precision() {
        let asset = asset("xor");
        let alice = account("alice");
        let bob = account("bob");
        let first = TransferDeltaTranscript {
            from_account: alice.clone(),
            to_account: bob.clone(),
            asset_definition: asset.clone(),
            amount: quantity(42, 0),
            from_balance_before: quantity(200, 0),
            from_balance_after: quantity(158, 0),
            to_balance_before: quantity(1, 0),
            to_balance_after: quantity(43, 0),
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        };
        let repeated = TransferDeltaTranscript {
            from_account: alice,
            to_account: bob,
            asset_definition: asset.clone(),
            amount: quantity(5, 1),
            // These repeated-key snapshots may be stale and are repaired by the witness builder.
            // Their precision must not rescale the already-seeded balance leaves.
            from_balance_before: quantity(158_001, 3),
            from_balance_after: quantity(157_501, 3),
            to_balance_before: quantity(43_001, 3),
            to_balance_after: quantity(43_501, 3),
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        };
        let transcripts = [TransferTranscript {
            batch_hash: Hash::prehashed([0x11; 32]),
            deltas: vec![first, repeated],
            authority_digest: Hash::prehashed([0x22; 32]),
            poseidon_preimage_digest: None,
        }];

        assert_eq!(transfer_asset_scales(&transcripts).get(&asset), Some(&1));
    }
    #[test]
    fn negative_numeric_payload_cannot_decode_as_transfer_delta_quantity() {
        let forged = ForgedTransferDeltaTranscript {
            from_account: account("alice"),
            to_account: account("bob"),
            asset_definition: asset("xor"),
            amount: Numeric::new(-1_i32, 0),
            from_balance_before: Numeric::from(10_u32),
            from_balance_after: Numeric::from(9_u32),
            to_balance_before: Numeric::zero(),
            to_balance_after: Numeric::from(1_u32),
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        };
        let encoded = forged.encode();
        assert!(
            TransferDeltaTranscript::decode(&mut encoded.as_slice()).is_err(),
            "a signed negative payload must not cross the FASTPQ quantity boundary"
        );
    }
}
