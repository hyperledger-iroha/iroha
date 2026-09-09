//! FASTPQ-specific data structures shared between the host and prover.

mod balance_key;
mod public_artifact;
mod quantity_units;
mod source_archive;
mod source_statement;
use crate::{account::AccountId, asset::id::AssetDefinitionId};
pub use balance_key::{FastpqBalanceKeyV1, transfer_balance_key};
use iroha_crypto::Hash;
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_schema::IntoSchema;
pub use public_artifact::*;
pub use quantity_units::*;
pub use source_archive::*;
pub use source_statement::*;
use std::collections::{BTreeMap, BTreeSet};
/// Metadata key storing Norito-encoded [`TransferTranscript`] collections for FASTPQ gadgets.
pub const TRANSFER_TRANSCRIPTS_METADATA_KEY: &str = "transfer_transcripts";
/// Canonical first-release Norito schema identity for [`FastpqTransitionBatch`].
pub const FASTPQ_TRANSITION_BATCH_SCHEMA_NAME: &str =
    "iroha_data_model::fastpq::FastpqStateTransitionBatchV1";
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
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::fastpq::TransferTranscript")]
pub struct TransferTranscript {
    /// Execution-call or typed native protocol-purpose hash that emitted this transcript.
    /// Sealed calls retain their inner identity; time invocations use distinct call hashes.
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
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::fastpq::TransferDeltaTranscript")]
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
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::fastpq::TransferSmtWitness")]
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
/// share that target scale, then converted into a non-negative `u64`. Target scales above the
/// ledger maximum are rejected before any scale-dependent work.
#[must_use]
pub fn normalized_numeric_to_u64(value: &Numeric, target_scale: u32) -> Option<u64> {
    if target_scale > iroha_primitives::numeric::MAX_DECIMAL_SCALE {
        return None;
    }
    let quantity = Quantity::from_canonical_numeric(value.clone()).ok()?;
    FastpqQuantityUnits::from_quantity(&quantity, target_scale)?.try_to_u64()
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
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::fastpq::FastpqTransitionBatch",
    frame = "iroha_data_model::fastpq::FastpqStateTransitionBatchV1"
)]
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
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::fastpq::FastpqStateTransition")]
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
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::fastpq::FastpqOperationKind")]
pub enum FastpqOperationKind {
    // The final V1 block starts at 32 so both the experimental 0..=5 wire and
    // the superseded two-operation 16/17 wire fail decoding.
    /// Asset transfer between two existing accounts.
    #[codec(index = 32)]
    Transfer,
    /// Asset mint increasing the committed circulating supply.
    #[codec(index = 33)]
    Mint,
    /// Asset burn decreasing the committed circulating supply.
    #[codec(index = 34)]
    Burn,
    /// Grant one exact permission to a role at the bound epoch.
    #[codec(index = 35)]
    RoleGrant(FastpqRolePermissionDelta),
    /// Revoke one exact permission from a role at the bound epoch.
    #[codec(index = 36)]
    RoleRevoke(FastpqRolePermissionDelta),
    /// Opaque metadata effect whose meaning is authenticated by its outer statement.
    #[codec(index = 37)]
    MetaSet,
}
/// Exact role/permission tuple committed by a FASTPQ permission transition.
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
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::fastpq::FastpqRolePermissionDelta")]
pub struct FastpqRolePermissionDelta {
    /// Canonical 32-byte role identifier.
    pub role_id: [u8; 32],
    /// Canonical 32-byte permission identifier.
    pub permission_id: [u8; 32],
    /// Epoch at which the membership change takes effect.
    pub epoch: u64,
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
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::fastpq::FastpqPublicInputs")]
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
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::fastpq::TransferTranscriptBundle")]
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
    use iroha_primitives::{bigint::BigInt, numeric::Numeric};
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
        assert_eq!(FastpqOperationKind::Transfer.encode(), 32_u32.to_le_bytes());
        assert_eq!(FastpqOperationKind::Mint.encode(), 33_u32.to_le_bytes());
        assert_eq!(FastpqOperationKind::Burn.encode(), 34_u32.to_le_bytes());
        let delta = FastpqRolePermissionDelta {
            role_id: [0x11; 32],
            permission_id: [0x22; 32],
            epoch: 9,
        };
        let grant = FastpqOperationKind::RoleGrant(delta.clone()).encode();
        let revoke = FastpqOperationKind::RoleRevoke(delta.clone()).encode();
        assert_eq!(&grant[..4], 35_u32.to_le_bytes().as_slice());
        assert_eq!(&revoke[..4], 36_u32.to_le_bytes().as_slice());
        assert_eq!(FastpqOperationKind::MetaSet.encode(), 37_u32.to_le_bytes());
        assert_eq!(
            FastpqOperationKind::decode(&mut grant.as_slice()).expect("decode role grant"),
            FastpqOperationKind::RoleGrant(delta.clone())
        );
        assert_eq!(
            FastpqOperationKind::decode(&mut revoke.as_slice()).expect("decode role revoke"),
            FastpqOperationKind::RoleRevoke(delta)
        );
        for retired in 0_u32..32 {
            assert!(
                FastpqOperationKind::decode(&mut retired.to_le_bytes().as_slice()).is_err(),
                "retired pre-release operation index {retired} must not decode"
            );
        }
    }

    #[test]
    fn role_permission_delta_json_is_exactly_fixed_width() {
        let delta = FastpqRolePermissionDelta {
            role_id: [0x11; 32],
            permission_id: [0x22; 32],
            epoch: 9,
        };
        let encoded = norito::json::to_json(&delta).expect("encode role permission delta");
        assert_eq!(
            norito::json::from_str::<FastpqRolePermissionDelta>(&encoded)
                .expect("decode role permission delta"),
            delta
        );
        for length in [31_usize, 33] {
            let malformed_role = format!(
                r#"{{"role_id":"{}","permission_id":"{}","epoch":9}}"#,
                "11".repeat(length),
                "22".repeat(32)
            );
            assert!(
                norito::json::from_str::<FastpqRolePermissionDelta>(&malformed_role).is_err(),
                "{length}-byte role ID must fail closed"
            );
            let malformed_permission = format!(
                r#"{{"role_id":"{}","permission_id":"{}","epoch":9}}"#,
                "11".repeat(32),
                "22".repeat(length)
            );
            assert!(
                norito::json::from_str::<FastpqRolePermissionDelta>(&malformed_permission).is_err(),
                "{length}-byte permission ID must fail closed"
            );
        }
    }

    #[test]
    fn transition_batch_schema_rejects_the_pre_release_header() {
        let expected = norito::core::schema_hash_for_name(FASTPQ_TRANSITION_BATCH_SCHEMA_NAME);
        assert_eq!(
            <FastpqTransitionBatch as norito::NoritoSchema>::frame_name(),
            FASTPQ_TRANSITION_BATCH_SCHEMA_NAME
        );
        assert_eq!(
            norito::schema::identity::frame_hash::<FastpqTransitionBatch>(),
            expected
        );
        assert_eq!(
            norito::schema::identity::frame_hash::<FastpqTransitionBatch>(),
            expected
        );
        assert_eq!(
            norito::schema::identity::frame_hash::<FastpqTransitionBatch>(),
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
        let encoded = norito::to_bytes(&batch).expect("encode release batch DTO");
        assert_eq!(&encoded[6..22], expected.as_slice());
        for retired_name in [
            "iroha_data_model::fastpq::FastpqTransitionBatch",
            "iroha_data_model::fastpq::FastpqTransitionBatchV1",
        ] {
            let mut retired = encoded.clone();
            let retired_schema = norito::core::schema_hash_for_name(retired_name);
            retired[6..22].copy_from_slice(&retired_schema);
            assert!(
                norito::decode_from_bytes::<FastpqTransitionBatch>(&retired).is_err(),
                "retired batch DTO schema {retired_name} must not decode as final V1"
            );
        }
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

#[cfg(test)]
mod captured_fastpq_schema_tests;
