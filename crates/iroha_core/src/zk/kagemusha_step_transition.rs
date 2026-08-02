//! Exact field-neutral operation ABI and assigned Kagemusha Step transition.
//!
//! The recursive Step circuits receive operation values as unreduced `u32`
//! limbs.  This module is deliberately independent of the legacy
//! host-compiled transition trace: it constrains the exact cells already
//! assigned by the Step circuit and never reloads a state or operation value
//! from a host witness.

use ff::{Field as _, PrimeField};
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions, RangeChip, RangeInstructions},
    utils::BigPrimeField,
};
use halo2_proofs::halo2curves::pasta::Fp;
use iroha_data_model::offline::{
    KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2, KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_DOMAIN_V2,
    KagemushaRecursiveSpendBranchClaimV2, KagemushaRecursiveSpendBranchV2,
    KagemushaRecursiveSpendInitRequestV4, KagemushaRecursiveSpendOperationVectorV4,
    KagemushaRecursiveSpendPublicStatementV4, KagemushaRecursiveSpendRedemptionIntentV4,
    KagemushaRecursiveSpendSplitIntentV4, KagemushaRecursiveSpendTransitionV4,
    kagemusha_confidential_amount_encoding_v2, kagemusha_recursive_spend_transition_tag_v2,
};
use norito::codec::{Decode, Encode};

use super::kagemusha_sha256_v4::{
    KagemushaSha256BitV4, KagemushaSha256ByteV4, KagemushaSha256JobsV4,
};
use super::kagemusha_v2::{
    I_APPEND_PROFILE as O_APPEND_PROFILE, I_ARTIFACT_MANIFEST_SHA256 as O_ARTIFACT_MANIFEST_SHA256,
    I_ASSET_ID_DIGEST as O_ASSET_ID_DIGEST, I_ASSET_SCALE as O_ASSET_SCALE,
    I_ASSET_TAG as O_ASSET_TAG, I_BRANCH_CHANGE as O_BRANCH_CHANGE,
    I_BRANCH_DEPTH as O_BRANCH_DEPTH, I_BRANCH_LINEAGE_ROOT as O_BRANCH_LINEAGE_ROOT,
    I_BRANCH_PATH_BITS as O_BRANCH_PATH_BITS, I_CHAIN_ID_DIGEST as O_CHAIN_ID_DIGEST,
    I_CHAIN_TAG as O_CHAIN_TAG, I_CHANGE_AMOUNT_HI as O_CHANGE_AMOUNT_HI,
    I_CHANGE_AMOUNT_LO as O_CHANGE_AMOUNT_LO, I_CHANGE_COMMITMENT as O_CHANGE_COMMITMENT,
    I_CHANGE_NULLIFIER as O_CHANGE_NULLIFIER, I_CHANGE_SCALE as O_CHANGE_SCALE,
    I_CURRENT_AMOUNT_HI as O_CURRENT_AMOUNT_HI, I_CURRENT_AMOUNT_LO as O_CURRENT_AMOUNT_LO,
    I_CURRENT_COMMITMENT as O_CURRENT_COMMITMENT,
    I_CURRENT_HOP_DOMAIN_TAG as O_CURRENT_HOP_DOMAIN_TAG,
    I_CURRENT_NULLIFIER as O_CURRENT_NULLIFIER, I_CURRENT_SCALE as O_CURRENT_SCALE,
    I_FINAL_ROOT as O_FINAL_ROOT, I_HAS_CHANGE as O_HAS_CHANGE, I_INITIAL_ROOT as O_INITIAL_ROOT,
    I_INPUT_AMOUNT_LO as O_INPUT_AMOUNT_LO, I_INPUT_COMMITMENT as O_INPUT_COMMITMENT,
    I_INPUT_NULLIFIER as O_INPUT_NULLIFIER, I_INPUT_SCALE as O_INPUT_SCALE,
    I_LAYOUT_VERSION as O_LAYOUT_VERSION, I_OPERATION_ID as O_OPERATION_ID,
    I_PARENT_BRANCH_DEPTH as O_PARENT_BRANCH_DEPTH,
    I_PARENT_BRANCH_LINEAGE_ROOT as O_PARENT_BRANCH_LINEAGE_ROOT,
    I_PARENT_BRANCH_PATH_BITS as O_PARENT_BRANCH_PATH_BITS,
    I_PARENT_BUNDLE_DIGEST as O_PARENT_BUNDLE_DIGEST, I_PARENT_FINAL_ROOT as O_PARENT_FINAL_ROOT,
    I_PARENT_TOPUP_RECEIPT_DIGEST as O_PARENT_TOPUP_RECEIPT_DIGEST,
    I_PEER_HOP_COUNT as O_PEER_HOP_COUNT, I_PREVIOUS_PEER_HOP_COUNT as O_PREVIOUS_PEER_HOP_COUNT,
    I_PREVIOUS_PROOF_STEP_COUNT as O_PREVIOUS_PROOF_STEP_COUNT,
    I_PROOF_STEP_COUNT as O_PROOF_STEP_COUNT, I_RECIPIENT_AMOUNT_LO as O_RECIPIENT_AMOUNT_LO,
    I_RECIPIENT_COMMITMENT as O_RECIPIENT_COMMITMENT,
    I_RECIPIENT_NULLIFIER as O_RECIPIENT_NULLIFIER,
    I_RECIPIENT_REQUEST_DIGEST as O_RECIPIENT_REQUEST_DIGEST,
    I_RECIPIENT_SCALE as O_RECIPIENT_SCALE, I_RECORD_INPUT_COUNT as O_RECORD_INPUT_COUNT,
    I_RECORD_INPUT_NULLIFIER_0 as O_RECORD_INPUT_NULLIFIER_0,
    I_RECORD_OUTPUT_0 as O_RECORD_OUTPUT_0, I_RECORD_OUTPUT_1 as O_RECORD_OUTPUT_1,
    I_RECORD_OUTPUT_COUNT as O_RECORD_OUTPUT_COUNT, I_RECORD_OUTPUT_SWAP as O_RECORD_OUTPUT_SWAP,
    I_RECORD_ROOT_AFTER as O_RECORD_ROOT_AFTER, I_RECORD_ROOT_BEFORE as O_RECORD_ROOT_BEFORE,
    I_REDEMPTION_PROFILE as O_REDEMPTION_PROFILE,
    I_REDEMPTION_RECIPIENT_DIGEST as O_REDEMPTION_RECIPIENT_DIGEST,
    I_SPLIT_DIGEST as O_SPLIT_DIGEST, I_STATEMENT_DIGEST as O_STATEMENT_DIGEST,
    I_TOPUP_ANCHOR_COUNT as O_TOPUP_ANCHOR_COUNT, I_TOPUP_ANCHOR_DIGEST as O_TOPUP_ANCHOR_DIGEST,
    I_TOPUP_OPERATION_ID as O_TOPUP_OPERATION_ID, I_TOPUP_RECEIPT_DIGEST as O_TOPUP_RECEIPT_DIGEST,
    I_TRANSFER_AMOUNT_LO as O_TRANSFER_AMOUNT_LO,
    I_TRANSFER_INPUT_COMMITMENT_0 as O_TRANSFER_INPUT_COMMITMENT_0,
    I_TRANSFER_INPUT_COMMITMENT_1 as O_TRANSFER_INPUT_COMMITMENT_1,
    I_TRANSFER_INPUT_COUNT as O_TRANSFER_INPUT_COUNT,
    I_TRANSFER_NULLIFIER_0 as O_TRANSFER_NULLIFIER_0,
    I_TRANSFER_NULLIFIER_1 as O_TRANSFER_NULLIFIER_1, I_TRANSFER_OUTPUT_0 as O_TRANSFER_OUTPUT_0,
    I_TRANSFER_OUTPUT_1 as O_TRANSFER_OUTPUT_1, I_TRANSFER_OUTPUT_COUNT as O_TRANSFER_OUTPUT_COUNT,
    I_TRANSFER_OUTPUT_SWAP as O_TRANSFER_OUTPUT_SWAP, I_TRANSFER_ROOT as O_TRANSFER_ROOT,
    I_TRANSFER_SCALE as O_TRANSFER_SCALE, I_UNSHIELD_PUBLIC_AMOUNT as O_UNSHIELD_PUBLIC_AMOUNT,
    I_UNSHIELD_PUBLIC_INPUTS_DIGEST as O_UNSHIELD_PUBLIC_INPUTS_DIGEST,
    I_VERIFIER_KEY_ID_DIGEST as O_VERIFIER_KEY_ID_DIGEST,
    KAGEMUSHA_RECURSIVE_SPEND_STATE_HISTORY_SHA256_DOMAIN_V5,
    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_HISTORY_ACCUMULATOR_LIMBS_V5,
    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V5,
    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5,
    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V2,
    KagemushaOutputMembershipCircuitV4, KagemushaOutputMembershipOperationV4,
    KagemushaOutputMembershipWitnessV4, S_ARTIFACT_MANIFEST_SHA256, S_ASSET_SCALE, S_ASSET_TAG,
    S_BRANCH_CLAIM_COUNT, S_BRANCH_CLAIMS, S_CHAIN_TAG, S_CURRENT_AMOUNT, S_CURRENT_COMMITMENT,
    S_CURRENT_NULLIFIER, S_CURRENT_SCALE, S_FINAL_ROOT, S_NEXT_ZERO_LEAF_INDEX, S_PEER_HOP_COUNT,
    S_PROOF_STEP_COUNT, S_TOPUP_ANCHOR_COUNT, S_TOPUP_ANCHORS, S_VERIFIER_KEY_ID, S_VERSION,
};

/// Number of canonical Pallas-field elements in one ABI-21 V4 operation row.
pub const KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4: usize = 135;
/// Exact number of little-endian `u32` limbs carrying the operation row.
pub const KAGEMUSHA_STEP_OPERATION_LIMBS_V4: usize = KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4 * 8;
const _: [(); KAGEMUSHA_STEP_OPERATION_LIMBS_V4] =
    [(); iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4];

/// Pallas `Fp` modulus as eight little-endian `u32` limbs.
///
/// Both Pasta parities use this bound. `Fp` is the smaller Pasta modulus, so a
/// canonical value reconstructed in either native field cannot wrap.
pub const KAGEMUSHA_STEP_OPERATION_FP_MODULUS_U32_LE_V4: [u32; 8] =
    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_OPERATION_FP_MODULUS_U32_LE_V4;

const _: [(); KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4] = [(); O_UNSHIELD_PUBLIC_AMOUNT + 1];
const _: [(); KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5] = [(); S_VERIFIER_KEY_ID + 8];

const ANCHOR_LIMBS: usize = 16;
const ANCHOR_SLOTS: usize = 2;
const CLAIM_SLOTS: usize = 2;
const CLAIM_LINEAGE_ROOT: usize = 0;
const CLAIM_DEPTH: usize = 8;
const CLAIM_PATH: usize = 9;
const CLAIM_HISTORY_ACCUMULATOR: usize = 11;

/// Exact fixed-size field-neutral operation vector shared by StepEq and StepEp.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaStepOperationVectorV4 {
    /// Eight little-endian `u32` limbs per canonical Pallas-field element.
    pub limbs: [u32; KAGEMUSHA_STEP_OPERATION_LIMBS_V4],
}

impl From<&KagemushaRecursiveSpendOperationVectorV4> for KagemushaStepOperationVectorV4 {
    fn from(operation: &KagemushaRecursiveSpendOperationVectorV4) -> Self {
        Self {
            limbs: operation.limbs,
        }
    }
}

impl From<&KagemushaStepOperationVectorV4> for KagemushaRecursiveSpendOperationVectorV4 {
    fn from(operation: &KagemushaStepOperationVectorV4) -> Self {
        Self {
            limbs: operation.limbs,
        }
    }
}

impl Default for KagemushaStepOperationVectorV4 {
    fn default() -> Self {
        Self {
            limbs: [0; KAGEMUSHA_STEP_OPERATION_LIMBS_V4],
        }
    }
}

impl KagemushaStepOperationVectorV4 {
    /// Encode 135 canonical Pallas elements as exact little-endian limbs.
    #[must_use]
    pub fn from_fields(fields: [Fp; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4]) -> Self {
        let mut limbs = [0_u32; KAGEMUSHA_STEP_OPERATION_LIMBS_V4];
        for (field_index, field) in fields.into_iter().enumerate() {
            let repr = field.to_repr();
            for (limb_index, chunk) in repr.as_ref().chunks_exact(4).enumerate() {
                limbs[field_index * 8 + limb_index] =
                    u32::from_le_bytes(chunk.try_into().expect("four-byte scalar limb"));
            }
        }
        Self { limbs }
    }

    /// Decode every exact limb group, rejecting values at or above `Fp::MODULUS`.
    pub fn to_fields(&self) -> Result<[Fp; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4], String> {
        let mut fields = [Fp::ZERO; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4];
        for (field_index, field) in fields.iter_mut().enumerate() {
            let mut repr = <Fp as PrimeField>::Repr::default();
            for (chunk, limb) in repr
                .as_mut()
                .chunks_exact_mut(4)
                .zip(&self.limbs[field_index * 8..field_index * 8 + 8])
            {
                chunk.copy_from_slice(&limb.to_le_bytes());
            }
            *field = Option::<Fp>::from(Fp::from_repr(repr)).ok_or_else(|| {
                format!("Kagemusha Step operation field {field_index} is not canonical Fp")
            })?;
        }
        Ok(fields)
    }

    /// Match every operation field derivable from an ABI-21 terminal
    /// statement before accepting the proof pair.
    ///
    /// The Step relation exposes the statement digest and the semantic
    /// operation as separate public wires.  Proving either wire is therefore
    /// insufficient at the terminal boundary: a verifier must also establish
    /// that the operation's statement-derived fields describe the statement
    /// it was asked to verify.  Fields that require the original confidential
    /// operation context remain covered by the exact-operation verifier path.
    pub(crate) fn validate_terminal_statement_v4(
        &self,
        statement: &KagemushaRecursiveSpendPublicStatementV4,
    ) -> Result<(), String> {
        let actual = self.to_fields()?;
        let mut expected = fill_statement_fields_v4(statement)?;
        if statement.transition.is_none() {
            let first_anchor = statement.topup_anchor_refs.first().ok_or_else(|| {
                "Kagemusha V4 initialization statement has no top-up anchor".to_owned()
            })?;
            let operation_tag = super::confidential_v2::derive_kagemusha_topup_operation_tag_v3(
                &first_anchor.topup_operation_id,
            )?;
            put_digest(&mut expected, O_OPERATION_ID, operation_tag);
        }

        let require_range = |start: usize, len: usize, label: &str| {
            if actual[start..start + len] == expected[start..start + len] {
                Ok(())
            } else {
                Err(format!(
                    "Kagemusha V4 terminal operation {label} does not match the public statement"
                ))
            }
        };

        for (start, len, label) in [
            (O_LAYOUT_VERSION, 1, "layout version"),
            (O_PROOF_STEP_COUNT, 1, "proof-step count"),
            (O_PEER_HOP_COUNT, 1, "peer-hop count"),
            (O_BRANCH_DEPTH, 1, "branch depth"),
            (O_ASSET_SCALE, 1, "asset scale"),
            (O_CURRENT_SCALE, 1, "current-note scale"),
            (O_CURRENT_AMOUNT_LO, 2, "current-note amount"),
            (O_BRANCH_PATH_BITS, 1, "branch path"),
            (O_FINAL_ROOT, 1, "final root"),
            (O_CURRENT_COMMITMENT, 1, "current commitment"),
            (O_CURRENT_NULLIFIER, 1, "current nullifier"),
            (O_ASSET_TAG, 1, "asset tag"),
            (O_CHAIN_TAG, 1, "chain tag"),
            (O_STATEMENT_DIGEST, 4, "statement digest"),
            (O_BRANCH_LINEAGE_ROOT, 4, "branch lineage root"),
            (O_CHAIN_ID_DIGEST, 4, "chain identity"),
            (O_ASSET_ID_DIGEST, 4, "asset identity"),
            (O_TOPUP_OPERATION_ID, 4, "top-up operation identity"),
            (O_ARTIFACT_MANIFEST_SHA256, 4, "artifact manifest identity"),
            (O_TOPUP_RECEIPT_DIGEST, 4, "top-up receipt identity"),
            (O_TOPUP_ANCHOR_DIGEST, 4, "top-up anchor identity"),
            (O_TOPUP_ANCHOR_COUNT, 1, "top-up anchor count"),
            (O_VERIFIER_KEY_ID_DIGEST, 4, "verifier-key identity"),
        ] {
            require_range(start, len, label)?;
        }

        let require_transition_ranges =
            |ranges: &[(usize, usize, &'static str)]| -> Result<(), String> {
                for &(start, len, label) in ranges {
                    require_range(start, len, label)?;
                }
                Ok(())
            };

        match statement.transition.as_ref() {
            None => {
                if statement.proof_step_count != 1 || statement.peer_hop_count != 0 {
                    return Err(
                        "Kagemusha V4 initialization must be proof step one at peer hop zero"
                            .to_owned(),
                    );
                }
                // Initialization repurposes the recipient-request slot for
                // the secure payer tag. The statement has no payer, so that
                // sole group requires exact finalized-anchor context. The
                // operation tag is derived from the public top-up operation
                // identity and is terminally pinned here.
                require_transition_ranges(&[
                    (O_APPEND_PROFILE, 1, "initialization profile"),
                    (O_BRANCH_CHANGE, 1, "initialization branch selector"),
                    (O_HAS_CHANGE, 1, "initialization change selector"),
                    (
                        O_PREVIOUS_PROOF_STEP_COUNT,
                        1,
                        "initialization parent proof-step count",
                    ),
                    (
                        O_PREVIOUS_PEER_HOP_COUNT,
                        1,
                        "initialization parent peer-hop count",
                    ),
                    (O_PARENT_BRANCH_DEPTH, 1, "initialization parent depth"),
                    (O_PARENT_BRANCH_PATH_BITS, 1, "initialization parent path"),
                    (O_SPLIT_DIGEST, 4, "initialization split digest"),
                    (O_OPERATION_ID, 4, "initialization operation tag"),
                    (O_PARENT_BUNDLE_DIGEST, 4, "initialization parent bundle"),
                    (
                        O_PARENT_BRANCH_LINEAGE_ROOT,
                        4,
                        "initialization parent lineage root",
                    ),
                    (O_CURRENT_HOP_DOMAIN_TAG, 4, "initialization transition tag"),
                    (
                        O_PARENT_TOPUP_RECEIPT_DIGEST,
                        4,
                        "initialization parent receipt",
                    ),
                    (O_REDEMPTION_PROFILE, 1, "initialization redemption profile"),
                    (
                        O_REDEMPTION_RECIPIENT_DIGEST,
                        4,
                        "initialization redemption recipient",
                    ),
                    (
                        O_UNSHIELD_PUBLIC_INPUTS_DIGEST,
                        4,
                        "initialization unshield public inputs",
                    ),
                    (
                        O_UNSHIELD_PUBLIC_AMOUNT,
                        1,
                        "initialization unshield amount",
                    ),
                ])
            }
            Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(transition)) => {
                if transition.parent_max_proof_step_count == 0 {
                    return Err(
                        "Kagemusha V4 peer split must consume an initialized parent".to_owned()
                    );
                }
                let proof_step_count = transition
                    .parent_max_proof_step_count
                    .checked_add(1)
                    .ok_or_else(|| {
                        "Kagemusha V4 peer-split proof-step count overflow".to_owned()
                    })?;
                let peer_hop_count = transition
                    .parent_max_peer_hop_count
                    .checked_add(1)
                    .ok_or_else(|| "Kagemusha V4 peer-split hop count overflow".to_owned())?;
                if statement.proof_step_count != proof_step_count
                    || statement.peer_hop_count != peer_hop_count
                {
                    return Err(
                        "Kagemusha V4 peer-split counters do not extend their parents".to_owned(),
                    );
                }
                require_transition_ranges(&[
                    (O_APPEND_PROFILE, 1, "peer-split profile"),
                    (O_BRANCH_CHANGE, 1, "peer-split branch selector"),
                    (
                        O_PREVIOUS_PROOF_STEP_COUNT,
                        1,
                        "peer-split parent proof-step count",
                    ),
                    (
                        O_PREVIOUS_PEER_HOP_COUNT,
                        1,
                        "peer-split parent peer-hop count",
                    ),
                    (O_PARENT_BRANCH_DEPTH, 1, "peer-split parent depth"),
                    (O_PARENT_BRANCH_PATH_BITS, 1, "peer-split parent path"),
                    (O_SPLIT_DIGEST, 4, "peer-split binding digest"),
                    (
                        O_RECIPIENT_REQUEST_DIGEST,
                        4,
                        "peer-split recipient request",
                    ),
                    (O_OPERATION_ID, 4, "peer-split operation identity"),
                    (O_PARENT_BUNDLE_DIGEST, 4, "peer-split parent bundle"),
                    (
                        O_PARENT_BRANCH_LINEAGE_ROOT,
                        4,
                        "peer-split parent lineage root",
                    ),
                    (O_CURRENT_HOP_DOMAIN_TAG, 4, "peer-split transition tag"),
                    (
                        O_PARENT_TOPUP_RECEIPT_DIGEST,
                        4,
                        "peer-split parent receipt",
                    ),
                    (O_REDEMPTION_PROFILE, 1, "peer-split redemption profile"),
                    (
                        O_REDEMPTION_RECIPIENT_DIGEST,
                        4,
                        "peer-split redemption recipient",
                    ),
                    (
                        O_UNSHIELD_PUBLIC_INPUTS_DIGEST,
                        4,
                        "peer-split unshield public inputs",
                    ),
                    (O_UNSHIELD_PUBLIC_AMOUNT, 1, "peer-split unshield amount"),
                ])?;
                let has_change = actual[O_HAS_CHANGE];
                if has_change != Fp::ZERO && has_change != Fp::ONE {
                    return Err("Kagemusha V4 peer-split change selector is not boolean".to_owned());
                }
                if matches!(transition.branch, KagemushaRecursiveSpendBranchV2::Change)
                    && has_change != Fp::ONE
                {
                    return Err(
                        "Kagemusha V4 change branch claims an absent change output".to_owned()
                    );
                }
                Ok(())
            }
            Some(KagemushaRecursiveSpendTransitionV4::RedemptionChange(transition)) => {
                if transition.parent_proof_step_count == 0 {
                    return Err(
                        "Kagemusha V4 redemption change must consume an initialized parent"
                            .to_owned(),
                    );
                }
                let proof_step_count = transition
                    .parent_proof_step_count
                    .checked_add(1)
                    .ok_or_else(|| {
                        "Kagemusha V4 redemption proof-step count overflow".to_owned()
                    })?;
                if statement.proof_step_count != proof_step_count
                    || statement.peer_hop_count != transition.parent_peer_hop_count
                {
                    return Err(
                        "Kagemusha V4 redemption counters do not extend their parent".to_owned(),
                    );
                }
                // Recipient and unshield fields are derived from the original
                // redemption intent, not from the continuing child statement;
                // the exact-operation verifier path compares those fields.
                require_transition_ranges(&[
                    (O_APPEND_PROFILE, 1, "redemption append profile"),
                    (O_BRANCH_CHANGE, 1, "redemption branch selector"),
                    (O_HAS_CHANGE, 1, "redemption change selector"),
                    (
                        O_PREVIOUS_PROOF_STEP_COUNT,
                        1,
                        "redemption parent proof-step count",
                    ),
                    (
                        O_PREVIOUS_PEER_HOP_COUNT,
                        1,
                        "redemption parent peer-hop count",
                    ),
                    (O_PARENT_BRANCH_DEPTH, 1, "redemption parent depth"),
                    (O_PARENT_BRANCH_PATH_BITS, 1, "redemption parent path"),
                    (O_SPLIT_DIGEST, 4, "redemption binding digest"),
                    (
                        O_RECIPIENT_REQUEST_DIGEST,
                        4,
                        "redemption recipient-request slot",
                    ),
                    (O_OPERATION_ID, 4, "redemption operation identity"),
                    (O_PARENT_BUNDLE_DIGEST, 4, "redemption parent bundle"),
                    (
                        O_PARENT_BRANCH_LINEAGE_ROOT,
                        4,
                        "redemption parent lineage root",
                    ),
                    (O_CURRENT_HOP_DOMAIN_TAG, 4, "redemption transition tag"),
                    (
                        O_PARENT_TOPUP_RECEIPT_DIGEST,
                        4,
                        "redemption parent receipt",
                    ),
                    (O_REDEMPTION_PROFILE, 1, "redemption profile"),
                ])
            }
        }
    }
}

/// Exact public cells emitted by the secure transfer relation, represented in
/// canonical byte form before they enter StepEq.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaStepTransferPublicV4 {
    /// Input commitments in parent-slot order; slot 1 is zero when absent.
    pub input_commitments: [[u8; 32]; 2],
    /// Input nullifiers in parent-slot order; slot 1 is zero when absent.
    pub input_nullifiers: [[u8; 32]; 2],
    /// Recipient then optional change commitments.
    pub output_commitments: [[u8; 32]; 2],
    /// Common authenticated input root.
    pub root: [u8; 32],
    /// Secure-relation asset tag.
    pub asset_tag: [u8; 32],
    /// Secure-relation chain tag.
    pub chain_tag: [u8; 32],
}

fn fp_from_bytes(bytes: [u8; 32], label: &str) -> Result<Fp, String> {
    let mut repr = <Fp as PrimeField>::Repr::default();
    repr.as_mut().copy_from_slice(&bytes);
    Option::<Fp>::from(Fp::from_repr(repr))
        .ok_or_else(|| format!("Kagemusha Step {label} is not a canonical Fp scalar"))
}

fn put_full_field(
    fields: &mut [Fp; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4],
    index: usize,
    bytes: [u8; 32],
    label: &str,
) -> Result<(), String> {
    fields[index] = fp_from_bytes(bytes, label)?;
    Ok(())
}

fn put_digest(
    fields: &mut [Fp; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4],
    index: usize,
    bytes: [u8; 32],
) {
    for (offset, chunk) in bytes.chunks_exact(8).enumerate() {
        fields[index + offset] = Fp::from(u64::from_le_bytes(
            chunk.try_into().expect("eight-byte digest limb"),
        ));
    }
}

fn put_amount(
    fields: &mut [Fp; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4],
    index: usize,
    amount: u128,
) {
    fields[index] = Fp::from(amount as u64);
    fields[index + 1] = Fp::from((amount >> 64) as u64);
}

fn canonical_binding_digest<T: Encode>(value: &T) -> Result<[u8; 32], String> {
    let encoded = norito::encode_canonical(value)
        .map_err(|error| format!("failed to encode Kagemusha Step binding: {error}"))?;
    Ok(iroha_zkp_halo2::poseidon::hash_bytes(&encoded))
}

fn encode_u32_scalar(value: u32) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    bytes[..4].copy_from_slice(&value.to_le_bytes());
    bytes
}

fn fill_statement_fields_v4(
    statement: &KagemushaRecursiveSpendPublicStatementV4,
) -> Result<[Fp; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4], String> {
    statement
        .validate_public_binding()
        .map_err(|error| error.to_string())?;
    let mut fields = [Fp::ZERO; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4];
    fields[O_LAYOUT_VERSION] = Fp::ONE;
    fields[O_PROOF_STEP_COUNT] = Fp::from(u64::from(statement.proof_step_count));
    fields[O_PEER_HOP_COUNT] = Fp::from(u64::from(statement.peer_hop_count));
    fields[O_ASSET_SCALE] = Fp::from(u64::from(statement.asset_scale));
    fields[O_CURRENT_SCALE] = Fp::from(u64::from(statement.current_note.amount.scale));
    put_amount(
        &mut fields,
        O_CURRENT_AMOUNT_LO,
        statement.current_note.amount.atomic_units,
    );
    put_full_field(
        &mut fields,
        O_FINAL_ROOT,
        statement.final_root,
        "V4 result root",
    )?;
    put_full_field(
        &mut fields,
        O_CURRENT_COMMITMENT,
        statement.current_note.note_commitment,
        "V4 current commitment",
    )?;
    put_full_field(
        &mut fields,
        O_CURRENT_NULLIFIER,
        statement.current_note.spend_nullifier,
        "V4 current nullifier",
    )?;

    let asset_tag =
        super::confidential_v2::derive_confidential_asset_tag_v3(&statement.asset.to_string())?;
    let chain_tag =
        super::confidential_v2::derive_confidential_chain_tag_v3(statement.chain_id.as_str())?;
    put_full_field(&mut fields, O_ASSET_TAG, asset_tag, "V4 asset tag")?;
    put_full_field(&mut fields, O_CHAIN_TAG, chain_tag, "V4 chain tag")?;
    put_digest(
        &mut fields,
        O_STATEMENT_DIGEST,
        statement.digest().map_err(|error| error.to_string())?,
    );
    put_digest(
        &mut fields,
        O_CHAIN_ID_DIGEST,
        canonical_binding_digest(&statement.chain_id)?,
    );
    put_digest(
        &mut fields,
        O_ASSET_ID_DIGEST,
        canonical_binding_digest(&statement.asset)?,
    );
    put_digest(
        &mut fields,
        O_ARTIFACT_MANIFEST_SHA256,
        statement.artifact_binding.manifest_sha256,
    );
    put_digest(
        &mut fields,
        O_VERIFIER_KEY_ID_DIGEST,
        canonical_binding_digest(&statement.verifier_key_id)?,
    );

    let first_anchor = statement
        .topup_anchor_refs
        .first()
        .ok_or_else(|| "Kagemusha Step V4 statement has no top-up anchor".to_owned())?;
    fields[O_TOPUP_ANCHOR_COUNT] = Fp::from(
        u64::try_from(statement.topup_anchor_refs.len())
            .map_err(|_| "Kagemusha Step V4 anchor count does not fit u64")?,
    );
    put_digest(
        &mut fields,
        O_TOPUP_OPERATION_ID,
        first_anchor.topup_operation_id,
    );
    put_digest(
        &mut fields,
        O_TOPUP_ANCHOR_DIGEST,
        first_anchor.anchor_digest,
    );
    put_digest(
        &mut fields,
        O_TOPUP_RECEIPT_DIGEST,
        first_anchor.anchor_digest,
    );

    let first_claim = statement
        .branch_claims
        .first()
        .ok_or_else(|| "Kagemusha Step V4 statement has no branch claim".to_owned())?;
    fields[O_BRANCH_DEPTH] = Fp::from(u64::from(first_claim.path.depth));
    fields[O_BRANCH_PATH_BITS] = Fp::from(u64::from_be_bytes(first_claim.path.path_bits));
    put_digest(
        &mut fields,
        O_BRANCH_LINEAGE_ROOT,
        first_claim.path.lineage_root,
    );

    match statement.transition.as_ref() {
        None => {}
        Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(transition)) => {
            fields[O_APPEND_PROFILE] = Fp::ONE;
            fields[O_BRANCH_CHANGE] = Fp::from(matches!(
                transition.branch,
                KagemushaRecursiveSpendBranchV2::Change
            ) as u64);
            fields[O_PREVIOUS_PROOF_STEP_COUNT] =
                Fp::from(u64::from(transition.parent_max_proof_step_count));
            fields[O_PREVIOUS_PEER_HOP_COUNT] =
                Fp::from(u64::from(transition.parent_max_peer_hop_count));
            put_digest(&mut fields, O_SPLIT_DIGEST, transition.binding_digest);
            put_digest(
                &mut fields,
                O_RECIPIENT_REQUEST_DIGEST,
                transition.recipient_request_digest,
            );
            put_digest(&mut fields, O_OPERATION_ID, transition.operation_id);
            let tag = kagemusha_recursive_spend_transition_tag_v2(transition.binding_digest)
                .map_err(|error| error.to_string())?;
            let mut padded_tag = [0_u8; 32];
            padded_tag[..tag.len()].copy_from_slice(&tag);
            put_digest(&mut fields, O_CURRENT_HOP_DOMAIN_TAG, padded_tag);
            let parent = first_claim
                .path
                .parent()
                .ok_or_else(|| "Kagemusha V4 append first claim has no parent".to_owned())?;
            fields[O_PARENT_BRANCH_DEPTH] = Fp::from(u64::from(parent.depth));
            fields[O_PARENT_BRANCH_PATH_BITS] = Fp::from(u64::from_be_bytes(parent.path_bits));
            put_digest(
                &mut fields,
                O_PARENT_BRANCH_LINEAGE_ROOT,
                parent.lineage_root,
            );
            put_digest(
                &mut fields,
                O_PARENT_TOPUP_RECEIPT_DIGEST,
                first_anchor.anchor_digest,
            );
        }
        Some(KagemushaRecursiveSpendTransitionV4::RedemptionChange(transition)) => {
            fields[O_REDEMPTION_PROFILE] = Fp::ONE;
            fields[O_BRANCH_CHANGE] = Fp::ONE;
            fields[O_HAS_CHANGE] = Fp::ONE;
            fields[O_PREVIOUS_PROOF_STEP_COUNT] =
                Fp::from(u64::from(transition.parent_proof_step_count));
            fields[O_PREVIOUS_PEER_HOP_COUNT] =
                Fp::from(u64::from(transition.parent_peer_hop_count));
            put_digest(&mut fields, O_SPLIT_DIGEST, transition.binding_digest);
            put_digest(
                &mut fields,
                O_PARENT_BUNDLE_DIGEST,
                transition.parent_bundle_digest,
            );
            put_digest(&mut fields, O_OPERATION_ID, transition.operation_id);
            let tag = kagemusha_recursive_spend_transition_tag_v2(transition.binding_digest)
                .map_err(|error| error.to_string())?;
            let mut padded_tag = [0_u8; 32];
            padded_tag[..tag.len()].copy_from_slice(&tag);
            put_digest(&mut fields, O_CURRENT_HOP_DOMAIN_TAG, padded_tag);
            let parent = first_claim
                .path
                .parent()
                .ok_or_else(|| "Kagemusha V4 redemption first claim has no parent".to_owned())?;
            fields[O_PARENT_BRANCH_DEPTH] = Fp::from(u64::from(parent.depth));
            fields[O_PARENT_BRANCH_PATH_BITS] = Fp::from(u64::from_be_bytes(parent.path_bits));
            put_digest(
                &mut fields,
                O_PARENT_BRANCH_LINEAGE_ROOT,
                parent.lineage_root,
            );
            put_digest(
                &mut fields,
                O_PARENT_TOPUP_RECEIPT_DIGEST,
                first_anchor.anchor_digest,
            );
        }
    }
    Ok(fields)
}

#[allow(clippy::too_many_arguments)]
fn build_init_operation_v4(
    statement: &KagemushaRecursiveSpendPublicStatementV4,
    topup: &super::confidential_v2::KagemushaTopUpShieldPublicInputsV2,
    membership: &KagemushaOutputMembershipWitnessV4,
    anchor_ref: iroha_data_model::offline::KagemushaRecursiveSpendTopUpAnchorRefV2,
    amount: u128,
    leaf_index: u32,
    expected_payer_tag: [u8; 32],
    expected_operation_tag: [u8; 32],
) -> Result<KagemushaStepOperationVectorV4, String> {
    let expected_claims = vec![
        KagemushaRecursiveSpendBranchClaimV2::root(anchor_ref.anchor_digest)
            .map_err(|error| error.to_string())?,
    ];
    let expected_asset_tag =
        super::confidential_v2::derive_confidential_asset_tag_v3(&statement.asset.to_string())?;
    let expected_chain_tag =
        super::confidential_v2::derive_confidential_chain_tag_v3(statement.chain_id.as_str())?;
    if statement.transition.is_some()
        || statement.proof_step_count != 1
        || statement.peer_hop_count != 0
        || statement.topup_anchor_refs.as_slice() != [anchor_ref]
        || statement.branch_claims != expected_claims
        || statement.current_note.amount.atomic_units != amount
        || statement.final_root != membership.final_root
        || statement.next_zero_leaf_index != membership.dummy_leaf_index
        || membership.operation != KagemushaOutputMembershipOperationV4::Init
        || membership.initial_root != topup.initial_root
        || membership.final_root != topup.finalized_root
        || membership.recipient.as_ref().map(|leaf| leaf.commitment)
            != Some(statement.current_note.note_commitment)
        || membership.recipient.as_ref().map(|leaf| leaf.leaf_index) != Some(leaf_index)
        || leaf_index.checked_add(1) != Some(membership.dummy_leaf_index)
        || membership.change.is_some()
        || topup.output_commitment != statement.current_note.note_commitment
        || topup.spend_nullifier != statement.current_note.spend_nullifier
        || topup.atomic_amount != kagemusha_confidential_amount_encoding_v2(amount)
        || topup.asset_scale != encode_u32_scalar(statement.asset_scale)
        || topup.leaf_index != encode_u32_scalar(leaf_index)
        || topup.asset_tag != expected_asset_tag
        || topup.chain_tag != expected_chain_tag
        || topup.payer_tag != expected_payer_tag
        || topup.operation_tag != expected_operation_tag
    {
        return Err("Kagemusha Step V4 init semantic bindings mismatch".to_owned());
    }

    let mut fields = fill_statement_fields_v4(statement)?;
    put_digest(&mut fields, O_RECIPIENT_REQUEST_DIGEST, expected_payer_tag);
    put_digest(&mut fields, O_OPERATION_ID, expected_operation_tag);
    fields[O_INPUT_SCALE] = fields[O_ASSET_SCALE];
    fields[O_TRANSFER_SCALE] = fields[O_ASSET_SCALE];
    fields[O_RECIPIENT_SCALE] = fields[O_ASSET_SCALE];
    put_amount(&mut fields, O_INPUT_AMOUNT_LO, amount);
    put_amount(&mut fields, O_TRANSFER_AMOUNT_LO, amount);
    put_amount(&mut fields, O_RECIPIENT_AMOUNT_LO, amount);
    fields[O_RECORD_OUTPUT_COUNT] = Fp::ONE;
    fields[O_TRANSFER_OUTPUT_COUNT] = Fp::ONE;
    put_full_field(
        &mut fields,
        O_INITIAL_ROOT,
        membership.initial_root,
        "V4 init root",
    )?;
    put_full_field(
        &mut fields,
        O_RECORD_ROOT_BEFORE,
        membership.initial_root,
        "V4 init root before",
    )?;
    fields[O_RECORD_ROOT_AFTER] = fields[O_FINAL_ROOT];
    fields[O_TRANSFER_ROOT] = fields[O_FINAL_ROOT];
    fields[O_RECIPIENT_COMMITMENT] = fields[O_CURRENT_COMMITMENT];
    fields[O_RECIPIENT_NULLIFIER] = fields[O_CURRENT_NULLIFIER];
    fields[O_RECORD_OUTPUT_0] = fields[O_CURRENT_COMMITMENT];
    fields[O_TRANSFER_OUTPUT_0] = fields[O_CURRENT_COMMITMENT];
    Ok(KagemushaStepOperationVectorV4::from_fields(fields))
}

impl KagemushaStepOperationVectorV4 {
    /// Construct an ABI-21 initialization operation directly from the V4
    /// finalized receipt and V4 public statement. The V4 carriers are
    /// validated in place without projecting through a retired lifecycle carrier.
    pub fn from_init_v4(
        request: &KagemushaRecursiveSpendInitRequestV4,
        statement: &KagemushaRecursiveSpendPublicStatementV4,
        topup: &super::confidential_v2::KagemushaTopUpShieldPublicInputsV2,
        membership: &KagemushaOutputMembershipWitnessV4,
    ) -> Result<Self, String> {
        request
            .validate_public_binding()
            .map_err(|error| error.to_string())?;
        statement
            .validate_public_binding()
            .map_err(|error| error.to_string())?;
        KagemushaOutputMembershipCircuitV4::new(membership.clone())?;
        let anchor = &request.topup_anchor;
        let anchor_ref = anchor.compact_ref().map_err(|error| error.to_string())?;
        let expected_claims = vec![
            KagemushaRecursiveSpendBranchClaimV2::root(anchor.anchor_digest)
                .map_err(|error| error.to_string())?,
        ];
        if statement.transition.is_some()
            || statement.artifact_binding != request.artifact_binding
            || statement.chain_id != anchor.chain_id
            || statement.asset != *anchor.asset.definition()
            || statement.asset_scale != anchor.asset_scale
            || statement.current_note != anchor.current_note
            || statement.final_root != anchor.finalized_root
            || statement.next_zero_leaf_index != membership.dummy_leaf_index
            || statement.topup_anchor_refs.as_slice() != [anchor_ref]
            || statement.branch_claims != expected_claims
            || membership.operation != KagemushaOutputMembershipOperationV4::Init
            || membership.initial_root != anchor.initial_root
            || membership.final_root != anchor.finalized_root
            || membership.recipient.as_ref().map(|leaf| leaf.commitment)
                != Some(anchor.current_note.note_commitment)
            || membership.recipient.as_ref().map(|leaf| leaf.leaf_index)
                != Some(anchor.shield_leaf_index)
            || anchor.shield_leaf_index.checked_add(1) != Some(membership.dummy_leaf_index)
            || membership.change.is_some()
            || topup.output_commitment != anchor.current_note.note_commitment
            || topup.spend_nullifier != anchor.current_note.spend_nullifier
            || topup.initial_root != anchor.initial_root
            || topup.finalized_root != anchor.finalized_root
            || topup.atomic_amount
                != kagemusha_confidential_amount_encoding_v2(anchor.amount.atomic_units)
            || topup.asset_scale != encode_u32_scalar(anchor.asset_scale)
            || topup.leaf_index != encode_u32_scalar(anchor.shield_leaf_index)
        {
            return Err(
                "Kagemusha Step V4 init bindings do not match the finalized top-up".to_owned(),
            );
        }
        let expected_asset_tag =
            super::confidential_v2::derive_confidential_asset_tag_v3(&statement.asset.to_string())?;
        let expected_chain_tag =
            super::confidential_v2::derive_confidential_chain_tag_v3(statement.chain_id.as_str())?;
        let expected_payer_tag =
            super::confidential_v2::derive_kagemusha_topup_payer_tag_v3(&anchor.payer.to_string())?;
        let expected_operation_tag =
            super::confidential_v2::derive_kagemusha_topup_operation_tag_v3(
                &anchor.topup_operation_id,
            )?;
        if topup.asset_tag != expected_asset_tag
            || topup.chain_tag != expected_chain_tag
            || topup.payer_tag != expected_payer_tag
            || topup.operation_tag != expected_operation_tag
        {
            return Err("Kagemusha Step V4 init confidential tags mismatch".to_owned());
        }
        build_init_operation_v4(
            statement,
            topup,
            membership,
            anchor_ref,
            anchor.amount.atomic_units,
            anchor.shield_leaf_index,
            expected_payer_tag,
            expected_operation_tag,
        )
    }

    /// Construct the real initialization operation used to qualify an exact
    /// pre-promotion candidate, without fabricating consensus finality.
    ///
    /// Candidate generation and every later receipt verifier share this exact
    /// constructor. It does not fabricate consensus finality; finality admission
    /// remains solely in [`Self::from_init_v4`].
    pub(super) fn from_candidate_qualification_init_v4(
        statement: &KagemushaRecursiveSpendPublicStatementV4,
        topup: &super::confidential_v2::KagemushaTopUpShieldPublicInputsV2,
        membership: &KagemushaOutputMembershipWitnessV4,
        payer: &str,
    ) -> Result<Self, String> {
        statement
            .validate_public_binding()
            .map_err(|error| error.to_string())?;
        KagemushaOutputMembershipCircuitV4::new(membership.clone())?;
        let [anchor_ref] = statement.topup_anchor_refs.as_slice() else {
            return Err(
                "Kagemusha candidate qualification init must have one top-up anchor".to_owned(),
            );
        };
        let expected_payer_tag =
            super::confidential_v2::derive_kagemusha_topup_payer_tag_v3(payer)?;
        let expected_operation_tag =
            super::confidential_v2::derive_kagemusha_topup_operation_tag_v3(
                &anchor_ref.topup_operation_id,
            )?;
        let amount = statement.current_note.amount.atomic_units;
        let leaf_index = membership
            .recipient
            .as_ref()
            .map(|leaf| leaf.leaf_index)
            .ok_or_else(|| {
                "Kagemusha candidate qualification init omits its recipient leaf".to_owned()
            })?;
        build_init_operation_v4(
            statement,
            topup,
            membership,
            *anchor_ref,
            amount,
            leaf_index,
            expected_payer_tag,
            expected_operation_tag,
        )
    }

    /// Construct an ABI-21 append operation directly from the V4 split intent
    /// and selected V4 child statement.
    pub fn from_append_v4(
        split: &KagemushaRecursiveSpendSplitIntentV4,
        statement: &KagemushaRecursiveSpendPublicStatementV4,
        transfer: &KagemushaStepTransferPublicV4,
        membership: &KagemushaOutputMembershipWitnessV4,
    ) -> Result<Self, String> {
        split
            .validate_public_binding()
            .map_err(|error| error.to_string())?;
        statement
            .validate_public_binding()
            .map_err(|error| error.to_string())?;
        KagemushaOutputMembershipCircuitV4::new(membership.clone())?;
        let Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(transition)) =
            statement.transition.as_ref()
        else {
            return Err("Kagemusha Step V4 append has no peer-split transition".to_owned());
        };
        let selected_note = match transition.branch {
            KagemushaRecursiveSpendBranchV2::Recipient => &split.recipient_output,
            KagemushaRecursiveSpendBranchV2::Change => split
                .change_output
                .as_ref()
                .ok_or_else(|| "Kagemusha Step V4 selected an absent change branch".to_owned())?,
        };
        let root = split
            .inputs
            .first()
            .ok_or_else(|| "Kagemusha Step V4 append has no input".to_owned())?
            .input_root;
        let binding_digest = split.binding_digest().map_err(|error| error.to_string())?;
        let expected_claims = split
            .output_branch_claims(transition.branch)
            .map_err(|error| error.to_string())?;
        let parent_max_proof_step_count = split
            .inputs
            .iter()
            .map(|input| input.proof_step_count)
            .max()
            .unwrap_or(0);
        let parent_max_peer_hop_count = split
            .inputs
            .iter()
            .map(|input| input.peer_hop_count)
            .max()
            .unwrap_or(0);
        if split.inputs.iter().any(|input| input.input_root != root)
            || statement.chain_id != split.chain_id
            || statement.asset != split.asset
            || statement.asset_scale != split.asset_scale
            || statement.artifact_binding != split.output_artifact_binding
            || statement.topup_anchor_refs != split.topup_anchor_refs
            || statement.branch_claims != expected_claims
            || transition.binding_digest != binding_digest
            || transition.recipient_request_digest != split.recipient_request_digest
            || transition.operation_id != split.operation_id
            || transition.parent_max_proof_step_count != parent_max_proof_step_count
            || transition.parent_max_peer_hop_count != parent_max_peer_hop_count
            || statement.current_note != *selected_note
            || statement.final_root != membership.final_root
            || statement.next_zero_leaf_index != membership.dummy_leaf_index
            || membership.operation != KagemushaOutputMembershipOperationV4::Split
            || membership.initial_root != root
            || membership.recipient.as_ref().map(|leaf| leaf.commitment)
                != Some(split.recipient_output.note_commitment)
            || membership.change.as_ref().map(|leaf| leaf.commitment)
                != split
                    .change_output
                    .as_ref()
                    .map(|note| note.note_commitment)
            || transfer.root != root
            || transfer.input_commitments[0] != split.inputs[0].input_note.note_commitment
            || transfer.input_nullifiers[0] != split.inputs[0].input_note.spend_nullifier
            || transfer.input_commitments[1]
                != split
                    .inputs
                    .get(1)
                    .map_or([0; 32], |input| input.input_note.note_commitment)
            || transfer.input_nullifiers[1]
                != split
                    .inputs
                    .get(1)
                    .map_or([0; 32], |input| input.input_note.spend_nullifier)
            || transfer.output_commitments[0] != split.recipient_output.note_commitment
            || transfer.output_commitments[1]
                != split
                    .change_output
                    .as_ref()
                    .map_or([0; 32], |note| note.note_commitment)
        {
            return Err("Kagemusha Step V4 append public bindings mismatch".to_owned());
        }
        let expected_asset_tag =
            super::confidential_v2::derive_confidential_asset_tag_v3(&split.asset.to_string())?;
        let expected_chain_tag =
            super::confidential_v2::derive_confidential_chain_tag_v3(split.chain_id.as_str())?;
        if transfer.asset_tag != expected_asset_tag || transfer.chain_tag != expected_chain_tag {
            return Err("Kagemusha Step V4 append confidential tags mismatch".to_owned());
        }

        let input_amount = split.input_amount().map_err(|error| error.to_string())?;
        let mut fields = fill_statement_fields_v4(statement)?;
        fields[O_HAS_CHANGE] = Fp::from(split.change_output.is_some() as u64);
        fields[O_INPUT_SCALE] = fields[O_ASSET_SCALE];
        fields[O_TRANSFER_SCALE] = fields[O_ASSET_SCALE];
        fields[O_RECIPIENT_SCALE] = fields[O_ASSET_SCALE];
        fields[O_CHANGE_SCALE] = Fp::from(
            split
                .change_output
                .as_ref()
                .map_or(0, |note| u64::from(note.amount.scale)),
        );
        let input_count = u64::try_from(split.inputs.len())
            .map_err(|_| "Kagemusha Step V4 input count does not fit u64")?;
        fields[O_RECORD_INPUT_COUNT] = Fp::from(input_count);
        fields[O_TRANSFER_INPUT_COUNT] = Fp::from(input_count);
        let output_count = 1 + u64::from(split.change_output.is_some());
        fields[O_RECORD_OUTPUT_COUNT] = Fp::from(output_count);
        fields[O_TRANSFER_OUTPUT_COUNT] = Fp::from(output_count);
        put_amount(&mut fields, O_INPUT_AMOUNT_LO, input_amount.atomic_units);
        put_amount(
            &mut fields,
            O_TRANSFER_AMOUNT_LO,
            split.transfer_amount.atomic_units,
        );
        put_amount(
            &mut fields,
            O_RECIPIENT_AMOUNT_LO,
            split.transfer_amount.atomic_units,
        );
        put_amount(
            &mut fields,
            O_CHANGE_AMOUNT_LO,
            split
                .change_output
                .as_ref()
                .map_or(0, |note| note.amount.atomic_units),
        );
        put_full_field(
            &mut fields,
            O_RECORD_ROOT_BEFORE,
            root,
            "V4 append input root",
        )?;
        fields[O_RECORD_ROOT_AFTER] = fields[O_FINAL_ROOT];
        fields[O_TRANSFER_ROOT] = fields[O_FINAL_ROOT];
        fields[O_PARENT_FINAL_ROOT] = fields[O_RECORD_ROOT_BEFORE];
        for slot in 0..2 {
            let input = split.inputs.get(slot);
            let commitment = input.map_or([0; 32], |input| input.input_note.note_commitment);
            let nullifier = input.map_or([0; 32], |input| input.input_note.spend_nullifier);
            put_full_field(
                &mut fields,
                O_TRANSFER_INPUT_COMMITMENT_0 + slot,
                commitment,
                "V4 append input commitment",
            )?;
            put_full_field(
                &mut fields,
                O_TRANSFER_NULLIFIER_0 + slot,
                nullifier,
                "V4 append input nullifier",
            )?;
            fields[O_RECORD_INPUT_NULLIFIER_0 + slot] = fields[O_TRANSFER_NULLIFIER_0 + slot];
        }
        fields[O_INPUT_COMMITMENT] = fields[O_TRANSFER_INPUT_COMMITMENT_0];
        fields[O_INPUT_NULLIFIER] = fields[O_TRANSFER_NULLIFIER_0];
        put_full_field(
            &mut fields,
            O_RECIPIENT_COMMITMENT,
            split.recipient_output.note_commitment,
            "V4 append recipient commitment",
        )?;
        put_full_field(
            &mut fields,
            O_RECIPIENT_NULLIFIER,
            split.recipient_output.spend_nullifier,
            "V4 append recipient nullifier",
        )?;
        if let Some(change) = &split.change_output {
            put_full_field(
                &mut fields,
                O_CHANGE_COMMITMENT,
                change.note_commitment,
                "V4 append change commitment",
            )?;
            put_full_field(
                &mut fields,
                O_CHANGE_NULLIFIER,
                change.spend_nullifier,
                "V4 append change nullifier",
            )?;
        }
        fields[O_RECORD_OUTPUT_0] = fields[O_RECIPIENT_COMMITMENT];
        fields[O_TRANSFER_OUTPUT_0] = fields[O_RECIPIENT_COMMITMENT];
        fields[O_RECORD_OUTPUT_1] = fields[O_CHANGE_COMMITMENT];
        fields[O_TRANSFER_OUTPUT_1] = fields[O_CHANGE_COMMITMENT];
        Ok(Self::from_fields(fields))
    }

    /// Construct an ABI-21 partial-redemption operation directly from the V4
    /// redemption intent and V4 change statement.
    pub fn from_redemption_change_v4(
        intent: &KagemushaRecursiveSpendRedemptionIntentV4,
        statement: &KagemushaRecursiveSpendPublicStatementV4,
        membership: &KagemushaOutputMembershipWitnessV4,
    ) -> Result<Self, String> {
        KagemushaOutputMembershipCircuitV4::new(membership.clone())?;
        let change = intent
            .change_output
            .as_ref()
            .ok_or_else(|| "Kagemusha Step V4 redemption has no continuing change".to_owned())?;
        if statement.final_root != membership.final_root
            || statement.next_zero_leaf_index != membership.dummy_leaf_index
            || membership.operation != KagemushaOutputMembershipOperationV4::RedemptionChange
            || membership.initial_root != intent.input_root
            || membership.recipient.is_some()
            || membership.change.as_ref().map(|leaf| leaf.commitment)
                != Some(change.note_commitment)
        {
            return Err("Kagemusha Step V4 redemption membership bindings mismatch".to_owned());
        }
        Self::from_redemption_change_public_v4(intent, statement)
    }

    /// Reconstruct the exact public ABI-21 redemption-change operation at a
    /// terminal verifier. Confidential membership paths are proved inside the
    /// carried pair; no private witness is accepted or synthesized here.
    pub fn from_redemption_change_public_v4(
        intent: &KagemushaRecursiveSpendRedemptionIntentV4,
        statement: &KagemushaRecursiveSpendPublicStatementV4,
    ) -> Result<Self, String> {
        intent
            .validate_public_binding()
            .map_err(|error| error.to_string())?;
        statement
            .validate_public_binding()
            .map_err(|error| error.to_string())?;
        let change = intent
            .change_output
            .as_ref()
            .ok_or_else(|| "Kagemusha Step V4 redemption has no continuing change".to_owned())?;
        let change_binding = intent.change_artifact_binding.as_ref().ok_or_else(|| {
            "Kagemusha Step V4 redemption change has no artifact binding".to_owned()
        })?;
        let Some(KagemushaRecursiveSpendTransitionV4::RedemptionChange(transition)) =
            statement.transition.as_ref()
        else {
            return Err(
                "Kagemusha Step V4 redemption result has no redemption transition".to_owned(),
            );
        };
        let binding_digest = intent.binding_digest().map_err(|error| error.to_string())?;
        let expected_claims = intent
            .change_branch_claims()
            .map_err(|error| error.to_string())?;
        let public = &intent.unshield_public_inputs;
        if statement.chain_id != intent.chain_id
            || statement.asset != intent.asset
            || statement.asset_scale != intent.public_amount.scale
            || statement.artifact_binding != *change_binding
            || statement.topup_anchor_refs != intent.parent_topup_anchor_refs
            || statement.branch_claims != expected_claims
            || statement.current_note != *change
            || statement.final_root == intent.input_root
            || transition.binding_digest != binding_digest
            || transition.parent_bundle_digest != intent.parent_bundle_digest
            || transition.operation_id != intent.operation_id
            || transition.parent_proof_step_count != intent.parent_proof_step_count
            || transition.parent_peer_hop_count != intent.parent_peer_hop_count
            || public.input_commitment_0 != intent.input_note.note_commitment
            || public.input_commitment_1 != [0; 32]
            || public.nullifier_0 != intent.input_note.spend_nullifier
            || public.nullifier_1 != [0; 32]
            || public.change_output_commitment != change.note_commitment
            || public.root != intent.input_root
            || public.public_amount
                != kagemusha_confidential_amount_encoding_v2(intent.public_amount.atomic_units)
        {
            return Err("Kagemusha Step V4 redemption public bindings mismatch".to_owned());
        }

        let mut fields = fill_statement_fields_v4(statement)?;
        fields[O_INPUT_SCALE] = fields[O_ASSET_SCALE];
        fields[O_TRANSFER_SCALE] = fields[O_ASSET_SCALE];
        fields[O_RECIPIENT_SCALE] = fields[O_ASSET_SCALE];
        fields[O_CHANGE_SCALE] = fields[O_ASSET_SCALE];
        fields[O_RECORD_INPUT_COUNT] = Fp::ONE;
        fields[O_TRANSFER_INPUT_COUNT] = Fp::ONE;
        fields[O_RECORD_OUTPUT_COUNT] = Fp::ONE;
        fields[O_TRANSFER_OUTPUT_COUNT] = Fp::ONE;
        put_amount(
            &mut fields,
            O_INPUT_AMOUNT_LO,
            intent.input_note.amount.atomic_units,
        );
        put_amount(
            &mut fields,
            O_TRANSFER_AMOUNT_LO,
            intent.public_amount.atomic_units,
        );
        put_amount(
            &mut fields,
            O_RECIPIENT_AMOUNT_LO,
            intent.public_amount.atomic_units,
        );
        put_amount(&mut fields, O_CHANGE_AMOUNT_LO, change.amount.atomic_units);
        fields[O_UNSHIELD_PUBLIC_AMOUNT] = Fp::from_u128(intent.public_amount.atomic_units);
        put_full_field(
            &mut fields,
            O_RECORD_ROOT_BEFORE,
            intent.input_root,
            "V4 redemption input root",
        )?;
        fields[O_RECORD_ROOT_AFTER] = fields[O_FINAL_ROOT];
        fields[O_TRANSFER_ROOT] = fields[O_RECORD_ROOT_BEFORE];
        fields[O_PARENT_FINAL_ROOT] = fields[O_RECORD_ROOT_BEFORE];
        put_full_field(
            &mut fields,
            O_TRANSFER_INPUT_COMMITMENT_0,
            intent.input_note.note_commitment,
            "V4 redemption input commitment",
        )?;
        put_full_field(
            &mut fields,
            O_TRANSFER_NULLIFIER_0,
            intent.input_note.spend_nullifier,
            "V4 redemption input nullifier",
        )?;
        fields[O_INPUT_COMMITMENT] = fields[O_TRANSFER_INPUT_COMMITMENT_0];
        fields[O_INPUT_NULLIFIER] = fields[O_TRANSFER_NULLIFIER_0];
        fields[O_RECORD_INPUT_NULLIFIER_0] = fields[O_TRANSFER_NULLIFIER_0];
        fields[O_CHANGE_COMMITMENT] = fields[O_CURRENT_COMMITMENT];
        fields[O_CHANGE_NULLIFIER] = fields[O_CURRENT_NULLIFIER];
        fields[O_RECORD_OUTPUT_0] = fields[O_CHANGE_COMMITMENT];
        fields[O_TRANSFER_OUTPUT_0] = fields[O_CHANGE_COMMITMENT];
        put_digest(
            &mut fields,
            O_REDEMPTION_RECIPIENT_DIGEST,
            canonical_binding_digest(&intent.recipient)?,
        );
        put_digest(
            &mut fields,
            O_UNSHIELD_PUBLIC_INPUTS_DIGEST,
            intent.unshield_public_inputs_digest,
        );
        put_full_field(
            &mut fields,
            O_ASSET_TAG,
            public.asset_tag,
            "V4 unshield asset tag",
        )?;
        put_full_field(
            &mut fields,
            O_CHAIN_TAG,
            public.chain_tag,
            "V4 unshield chain tag",
        )?;
        Ok(Self::from_fields(fields))
    }
}

/// Assigned canonical operation values reconstructed from the exact public limbs.
#[derive(Clone, Debug)]
pub struct AssignedKagemushaStepOperationV4<F: BigPrimeField> {
    /// The original public cells; no limb is re-assigned by this module. The
    /// exact array is boxed because carrying 1,080 `AssignedValue`s by value
    /// makes nested circuit-construction frames exceed mobile thread stacks.
    pub limbs: Box<[AssignedValue<F>; KAGEMUSHA_STEP_OPERATION_LIMBS_V4]>,
    /// Canonically reconstructed native values in existing V2 row order. This
    /// array is boxed for the same bounded-stack guarantee as `limbs`.
    pub fields: Box<[AssignedValue<F>; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4]>,
}

fn boxed_assigned_values_exact<F: BigPrimeField, const N: usize>(
    values: Vec<AssignedValue<F>>,
) -> Box<[AssignedValue<F>; N]> {
    let actual_len = values.len();
    values.into_boxed_slice().try_into().unwrap_or_else(|_| {
        panic!("assigned-value length mismatch: expected {N}, got {actual_len}")
    })
}

fn assert_equal<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    lhs: AssignedValue<F>,
    rhs: impl Into<QuantumCell<F>>,
) {
    let difference = range.gate.sub(ctx, lhs, rhs);
    range.gate.assert_is_const(ctx, &difference, &F::ZERO);
}

fn assert_equal_if<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    condition: AssignedValue<F>,
    lhs: AssignedValue<F>,
    rhs: impl Into<QuantumCell<F>>,
) {
    let difference = range.gate.sub(ctx, lhs, rhs);
    let selected = range.gate.mul(ctx, condition, difference);
    range.gate.assert_is_const(ctx, &selected, &F::ZERO);
}

fn assert_zero_if<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    condition: AssignedValue<F>,
    value: AssignedValue<F>,
) {
    let selected = range.gate.mul(ctx, condition, value);
    range.gate.assert_is_const(ctx, &selected, &F::ZERO);
}

fn field_limb_range(field: usize) -> std::ops::Range<usize> {
    field * 8..field * 8 + 8
}

fn enforce_fp_canonical_limbs<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    limbs: &[AssignedValue<F>],
) {
    let gate = &range.gate;
    let mut prefix_equal = ctx.load_constant(F::ONE);
    let mut is_less = ctx.load_constant(F::ZERO);
    for index in (0..8).rev() {
        let modulus_limb = F::from(u64::from(
            KAGEMUSHA_STEP_OPERATION_FP_MODULUS_U32_LE_V4[index],
        ));
        let limb_less =
            range.is_less_than(ctx, limbs[index], QuantumCell::Constant(modulus_limb), 32);
        let first_less = gate.mul(ctx, prefix_equal, limb_less);
        is_less = gate.add(ctx, is_less, first_less);
        let limb_equal = gate.is_equal(ctx, limbs[index], QuantumCell::Constant(modulus_limb));
        prefix_equal = gate.mul(ctx, prefix_equal, limb_equal);
    }
    gate.assert_is_const(ctx, &is_less, &F::ONE);
}

fn reconstruct_u32_limbs<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    limbs: &[AssignedValue<F>],
) -> AssignedValue<F> {
    let radix = F::from(1_u64 << 32);
    let mut weight = F::ONE;
    let mut weights = Vec::with_capacity(limbs.len());
    for _ in limbs {
        weights.push(QuantumCell::Constant(weight));
        weight *= radix;
    }
    range
        .gate
        .inner_product(ctx, limbs.iter().copied(), weights)
}

fn constrain_typed_operation_fields<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    fields: &[AssignedValue<F>; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4],
) {
    let gate = &range.gate;
    gate.assert_is_const(ctx, &fields[O_LAYOUT_VERSION], &F::ONE);
    for index in [
        O_APPEND_PROFILE,
        O_BRANCH_CHANGE,
        O_HAS_CHANGE,
        O_RECORD_OUTPUT_SWAP,
        O_TRANSFER_OUTPUT_SWAP,
        O_REDEMPTION_PROFILE,
    ] {
        gate.assert_bit(ctx, fields[index]);
    }
    for index in O_PROOF_STEP_COUNT..=O_TRANSFER_OUTPUT_COUNT {
        range.range_check(ctx, fields[index], 32);
    }
    for index in O_CURRENT_AMOUNT_LO..=O_CHANGE_AMOUNT_HI {
        range.range_check(ctx, fields[index], 64);
    }
    for index in [O_BRANCH_PATH_BITS, O_PARENT_BRANCH_PATH_BITS] {
        range.range_check(ctx, fields[index], 64);
    }
    for index in O_STATEMENT_DIGEST..O_PARENT_FINAL_ROOT {
        if matches!(index, O_TOPUP_ANCHOR_COUNT | O_REDEMPTION_PROFILE) {
            continue;
        }
        range.range_check(ctx, fields[index], 64);
    }
    range.range_check(ctx, fields[O_TOPUP_ANCHOR_COUNT], 32);
    for index in O_REDEMPTION_RECIPIENT_DIGEST..=O_UNSHIELD_PUBLIC_INPUTS_DIGEST + 3 {
        range.range_check(ctx, fields[index], 64);
    }
    range.range_check(
        ctx,
        fields[O_UNSHIELD_PUBLIC_AMOUNT],
        super::confidential_v2::ConfidentialUnshieldChangePublicInputV1::PublicAmount
            .range()
            .expect("recursive unshield public amount range is specified")
            .bits(),
    );
}

/// Range-check, canonically bound, and reconstruct the shared operation vector.
///
/// Every one of the 1080 source cells is used directly. In particular, this
/// does not call `load_witness`, and it proves `< Fp::MODULUS` limbwise before
/// reconstructing a native value in either Pasta field.
pub fn assign_kagemusha_step_operation_v4<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    operation_limbs: &[AssignedValue<F>; KAGEMUSHA_STEP_OPERATION_LIMBS_V4],
) -> AssignedKagemushaStepOperationV4<F> {
    for limb in operation_limbs {
        range.range_check(ctx, *limb, 32);
    }
    let fields = boxed_assigned_values_exact(
        (0..KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4)
            .map(|field| {
                let limbs = &operation_limbs[field_limb_range(field)];
                enforce_fp_canonical_limbs(ctx, range, limbs);
                reconstruct_u32_limbs(ctx, range, limbs)
            })
            .collect(),
    );
    constrain_typed_operation_fields(ctx, range, fields.as_ref());
    AssignedKagemushaStepOperationV4 {
        limbs: boxed_assigned_values_exact(operation_limbs.to_vec()),
        fields,
    }
}

/// Named exact cells exported to the Step recursion and Eq-only relations.
#[derive(Clone, Debug)]
pub struct NamedTransitionBindings<F: BigPrimeField> {
    /// Canonically reconstructed operation row.
    pub operation: AssignedKagemushaStepOperationV4<F>,
    /// Initialization profile (`1 - append - redemption`).
    pub is_init: AssignedValue<F>,
    /// Peer split profile.
    pub is_append: AssignedValue<F>,
    /// Partial redemption/change profile.
    pub is_redemption: AssignedValue<F>,
    /// Optional change-output flag.
    pub has_change: AssignedValue<F>,
    /// Common input/record root (operation field 36).
    pub input_root: AssignedValue<F>,
    /// Result/output root (operation field 35/37).
    pub output_root: AssignedValue<F>,
    /// Append-only frontier inherited from every present parent.
    pub input_next_zero_leaf_index: AssignedValue<F>,
    /// Append-only frontier committed by the result state.
    pub output_next_zero_leaf_index: AssignedValue<F>,
    /// Exact input commitments in parent-slot order.
    pub input_commitments: [AssignedValue<F>; 2],
    /// Exact input nullifiers in parent-slot order.
    pub input_nullifiers: [AssignedValue<F>; 2],
    /// Recipient output commitment.
    pub recipient_commitment: AssignedValue<F>,
    /// Change output commitment.
    pub change_commitment: AssignedValue<F>,
    /// Statement digest as eight exact `u32` cells.
    pub statement_digest_limbs: [AssignedValue<F>; 8],
    /// Init payer tag encoded in operation fields 67..=70 as exact `u32` limbs.
    /// This aliases the append recipient-request digest and is meaningful only
    /// when `is_init == 1`.
    pub init_payer_tag_limbs: [AssignedValue<F>; 8],
    /// Init operation tag encoded in operation fields 71..=74 as exact `u32`
    /// limbs. This aliases the descendant operation-id digest and is selected
    /// by `is_init` at the StepEq boundary.
    pub init_operation_tag_limbs: [AssignedValue<F>; 8],
}

/// Reconstruct one exact little-endian 256-bit scalar from eight already
/// range-checked operation limbs. The operation loader has proved that the
/// value is below the Pallas base-field modulus, so this cannot wrap in either
/// Pasta recursion parity.
pub(crate) fn reconstruct_kagemusha_step_scalar_v4<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    limbs: &[AssignedValue<F>; 8],
) -> AssignedValue<F> {
    reconstruct_u32_limbs(ctx, range, limbs)
}

/// Copy-bind the two init-only scalar tags to outputs 9 and 10 of the secure
/// top-up relation. The constraint is profile-gated because the same operation
/// slots carry descendant metadata for append and redemption.
pub(crate) fn constrain_kagemusha_step_init_topup_tags_v4<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bindings: &NamedTransitionBindings<F>,
    secure_payer_tag: AssignedValue<F>,
    secure_operation_tag: AssignedValue<F>,
) {
    let payer = reconstruct_kagemusha_step_scalar_v4(ctx, range, &bindings.init_payer_tag_limbs);
    let operation =
        reconstruct_kagemusha_step_scalar_v4(ctx, range, &bindings.init_operation_tag_limbs);
    assert_equal_if(ctx, range, bindings.is_init, payer, secure_payer_tag);
    assert_equal_if(
        ctx,
        range,
        bindings.is_init,
        operation,
        secure_operation_tag,
    );
}

fn operation_full_limbs<F: BigPrimeField>(
    operation: &AssignedKagemushaStepOperationV4<F>,
    field: usize,
) -> &[AssignedValue<F>] {
    &operation.limbs[field_limb_range(field)]
}

fn operation_digest_limbs<F: BigPrimeField>(
    operation: &AssignedKagemushaStepOperationV4<F>,
    first_field: usize,
) -> [AssignedValue<F>; 8] {
    std::array::from_fn(|limb| {
        let field = first_field + limb / 2;
        operation.limbs[field * 8 + limb % 2]
    })
}

fn assert_slices_equal_if<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    condition: AssignedValue<F>,
    lhs: &[AssignedValue<F>],
    rhs: &[AssignedValue<F>],
) {
    assert_eq!(lhs.len(), rhs.len());
    for (lhs, rhs) in lhs.iter().copied().zip(rhs.iter().copied()) {
        assert_equal_if(ctx, range, condition, lhs, rhs);
    }
}

fn assert_slice_zero_if<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    condition: AssignedValue<F>,
    values: &[AssignedValue<F>],
) {
    for value in values {
        assert_zero_if(ctx, range, condition, *value);
    }
}

fn slices_equal<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    lhs: &[AssignedValue<F>],
    rhs: &[AssignedValue<F>],
) -> AssignedValue<F> {
    assert_eq!(lhs.len(), rhs.len());
    lhs.iter().copied().zip(rhs.iter().copied()).fold(
        ctx.load_constant(F::ONE),
        |equal, (lhs, rhs)| {
            let limb_equal = range.gate.is_equal(ctx, lhs, rhs);
            range.gate.mul(ctx, equal, limb_equal)
        },
    )
}

fn any<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    values: impl IntoIterator<Item = AssignedValue<F>>,
) -> AssignedValue<F> {
    values
        .into_iter()
        .fold(ctx.load_constant(F::ZERO), |result, value| {
            range.gate.or(ctx, result, value)
        })
}

fn constrain_small_count<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    count: AssignedValue<F>,
) -> [AssignedValue<F>; 2] {
    range.range_check(ctx, count, 2);
    let below_three = range.is_less_than(ctx, count, QuantumCell::Constant(F::from(3)), 2);
    range.gate.assert_is_const(ctx, &below_three, &F::ONE);
    let is_zero = range.gate.is_zero(ctx, count);
    let first = range.gate.not(ctx, is_zero);
    let second = range
        .gate
        .is_equal(ctx, count, QuantumCell::Constant(F::from(2)));
    [first, second]
}

fn reconstruct_state_u128<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    limbs: &[AssignedValue<F>],
) -> AssignedValue<F> {
    debug_assert_eq!(limbs.len(), 4);
    reconstruct_u32_limbs(ctx, range, limbs)
}

fn operation_u128<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    operation: &AssignedKagemushaStepOperationV4<F>,
    low_field: usize,
) -> AssignedValue<F> {
    range.gate.mul_add(
        ctx,
        operation.fields[low_field + 1],
        QuantumCell::Constant(F::from_u128(1_u128 << 64)),
        operation.fields[low_field],
    )
}

fn u32_le_bytes<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: AssignedValue<F>,
) -> [AssignedValue<F>; 4] {
    let bits = range.gate.num_to_bits(ctx, value, 32);
    std::array::from_fn(|byte| {
        range.gate.inner_product(
            ctx,
            bits[byte * 8..byte * 8 + 8].iter().copied(),
            (0..8).map(|bit| QuantumCell::Constant(F::from(1_u64 << bit))),
        )
    })
}

fn byte_swap_u32<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: AssignedValue<F>,
) -> AssignedValue<F> {
    let bytes = u32_le_bytes(ctx, range, value);
    range.gate.inner_product(
        ctx,
        bytes.into_iter(),
        [
            QuantumCell::Constant(F::from(1_u64 << 24)),
            QuantumCell::Constant(F::from(1_u64 << 16)),
            QuantumCell::Constant(F::from(1_u64 << 8)),
            QuantumCell::Constant(F::ONE),
        ],
    )
}

fn native_bytes<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    limbs: &[AssignedValue<F>],
) -> Vec<AssignedValue<F>> {
    limbs
        .iter()
        .copied()
        .flat_map(|limb| u32_le_bytes(ctx, range, limb))
        .collect()
}

fn sha256_native_bytes<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    limbs: &[AssignedValue<F>],
) -> Vec<KagemushaSha256ByteV4<F>> {
    let gate = range.gate();
    let mut bytes = Vec::with_capacity(limbs.len() * 4);
    for limb in limbs.iter().copied() {
        let bits = KagemushaSha256BitV4::decompose(ctx, gate, limb, 32);
        for byte in 0..4 {
            bytes.push(KagemushaSha256ByteV4::from_bits_le(
                ctx,
                gate,
                &bits[byte * 8..byte * 8 + 8],
            ));
        }
    }
    bytes
}

fn lex_less<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    lhs: &[AssignedValue<F>],
    rhs: &[AssignedValue<F>],
    bits: usize,
) -> AssignedValue<F> {
    assert_eq!(lhs.len(), rhs.len());
    let mut prefix_equal = ctx.load_constant(F::ONE);
    let mut less = ctx.load_constant(F::ZERO);
    for (lhs, rhs) in lhs.iter().copied().zip(rhs.iter().copied()) {
        let limb_less = range.is_less_than(ctx, lhs, rhs, bits);
        let first_less = range.gate.mul(ctx, prefix_equal, limb_less);
        less = range.gate.add(ctx, less, first_less);
        let limb_equal = range.gate.is_equal(ctx, lhs, rhs);
        prefix_equal = range.gate.mul(ctx, prefix_equal, limb_equal);
    }
    less
}

fn anchor_less<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    lhs: &[AssignedValue<F>],
    rhs: &[AssignedValue<F>],
) -> AssignedValue<F> {
    let lhs = native_bytes(ctx, range, lhs);
    let rhs = native_bytes(ctx, range, rhs);
    lex_less(ctx, range, &lhs, &rhs, 8)
}

fn claim_path_less<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    lhs: &[AssignedValue<F>],
    rhs: &[AssignedValue<F>],
) -> AssignedValue<F> {
    let mut lhs_key = native_bytes(ctx, range, &lhs[CLAIM_LINEAGE_ROOT..CLAIM_DEPTH]);
    let mut rhs_key = native_bytes(ctx, range, &rhs[CLAIM_LINEAGE_ROOT..CLAIM_DEPTH]);
    lhs_key.push(lhs[CLAIM_DEPTH]);
    rhs_key.push(rhs[CLAIM_DEPTH]);
    lhs_key.extend(native_bytes(
        ctx,
        range,
        &lhs[CLAIM_PATH..CLAIM_HISTORY_ACCUMULATOR],
    ));
    rhs_key.extend(native_bytes(
        ctx,
        range,
        &rhs[CLAIM_PATH..CLAIM_HISTORY_ACCUMULATOR],
    ));
    // Depth is <= 64 and every other key component is a byte; eight bits cover both.
    lex_less(ctx, range, &lhs_key, &rhs_key, 8)
}

fn constrain_set_union<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    sources: &[(&[AssignedValue<F>], AssignedValue<F>)],
    results: &[(&[AssignedValue<F>], AssignedValue<F>)],
) {
    for (source, source_present) in sources {
        let candidates = results
            .iter()
            .map(|(result, result_present)| {
                let equal = slices_equal(ctx, range, source, result);
                range.gate.mul(ctx, *result_present, equal)
            })
            .collect::<Vec<_>>();
        let member = any(ctx, range, candidates);
        assert_equal_if(
            ctx,
            range,
            *source_present,
            member,
            QuantumCell::Constant(F::ONE),
        );
    }
    for (result, result_present) in results {
        let candidates = sources
            .iter()
            .map(|(source, source_present)| {
                let equal = slices_equal(ctx, range, result, source);
                range.gate.mul(ctx, *source_present, equal)
            })
            .collect::<Vec<_>>();
        let member = any(ctx, range, candidates);
        assert_equal_if(
            ctx,
            range,
            *result_present,
            member,
            QuantumCell::Constant(F::ONE),
        );
    }
}

fn constrain_lineage_root_set_equality<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    anchor_roots: [&[AssignedValue<F>]; ANCHOR_SLOTS],
    anchor_present: [AssignedValue<F>; ANCHOR_SLOTS],
    claim_roots: [&[AssignedValue<F>]; CLAIM_SLOTS],
    claim_present: [AssignedValue<F>; CLAIM_SLOTS],
) {
    let anchors: [(&[AssignedValue<F>], AssignedValue<F>); ANCHOR_SLOTS] =
        std::array::from_fn(|slot| (anchor_roots[slot], anchor_present[slot]));
    let claims: [(&[AssignedValue<F>], AssignedValue<F>); CLAIM_SLOTS] =
        std::array::from_fn(|slot| (claim_roots[slot], claim_present[slot]));
    constrain_set_union(ctx, range, &anchors, &claims);
}

fn operation_path_from_state<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    path: &[AssignedValue<F>],
) -> AssignedValue<F> {
    debug_assert_eq!(path.len(), 2);
    let bytes = native_bytes(ctx, range, path);
    range.gate.inner_product(
        ctx,
        bytes.into_iter(),
        (0..8).map(|index| QuantumCell::Constant(F::from_u128(1_u128 << (8 * (7 - index))))),
    )
}

fn extend_claim<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    sha_jobs: &mut KagemushaSha256JobsV4<F>,
    claim: &[AssignedValue<F>],
    branch: AssignedValue<F>,
    tag: &[AssignedValue<F>; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V2],
) -> Result<Vec<AssignedValue<F>>, String> {
    debug_assert_eq!(
        claim.len(),
        KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V5
    );
    let gate = &range.gate;
    let depth = claim[CLAIM_DEPTH];
    range.range_check(ctx, depth, 7);
    let depth_below_max = range.is_less_than(ctx, depth, QuantumCell::Constant(F::from(64)), 7);
    gate.assert_is_const(ctx, &depth_below_max, &F::ONE);
    let depth_selectors: [AssignedValue<F>; 64] = std::array::from_fn(|candidate| {
        gate.is_equal(ctx, depth, QuantumCell::Constant(F::from(candidate as u64)))
    });

    let mut extended = Vec::with_capacity(KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V5);
    extended.extend_from_slice(&claim[CLAIM_LINEAGE_ROOT..CLAIM_DEPTH]);
    extended.push(gate.add(ctx, depth, QuantumCell::Constant(F::ONE)));
    for path_limb in 0..2 {
        let added_bit = (0..64).filter(|depth| depth / 32 == path_limb).fold(
            ctx.load_constant(F::ZERO),
            |sum, depth| {
                let byte = depth / 8;
                let bit_in_byte = 7 - depth % 8;
                let bit_in_limb = (byte % 4) * 8 + bit_in_byte;
                let selected = gate.mul(
                    ctx,
                    depth_selectors[depth],
                    QuantumCell::Constant(F::from(1_u64 << bit_in_limb)),
                );
                gate.add(ctx, sum, selected)
            },
        );
        let branch_bit = gate.mul(ctx, branch, added_bit);
        extended.push(gate.add(ctx, claim[CLAIM_PATH + path_limb], branch_bit));
    }

    let mut history_preimage = KAGEMUSHA_RECURSIVE_SPEND_STATE_HISTORY_SHA256_DOMAIN_V5
        .iter()
        .copied()
        .chain(std::iter::once(0))
        .map(KagemushaSha256ByteV4::constant)
        .collect::<Vec<_>>();
    history_preimage.extend(sha256_native_bytes(
        ctx,
        range,
        &claim[CLAIM_HISTORY_ACCUMULATOR
            ..CLAIM_HISTORY_ACCUMULATOR
                + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_HISTORY_ACCUMULATOR_LIMBS_V5],
    ));
    history_preimage.extend(sha256_native_bytes(ctx, range, tag));
    let history_words = sha_jobs.digest_constrained(ctx, &history_preimage)?;
    extended.extend(
        history_words
            .into_iter()
            .map(|word| byte_swap_u32(ctx, range, word)),
    );
    Ok(extended)
}

fn assert_nonzero_if<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    condition: AssignedValue<F>,
    value: AssignedValue<F>,
) {
    let is_zero = range.gate.is_zero(ctx, value);
    let selected = range.gate.mul(ctx, condition, is_zero);
    range.gate.assert_is_const(ctx, &selected, &F::ZERO);
}

fn bind_full_field_to_state_if<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    condition: AssignedValue<F>,
    operation: &AssignedKagemushaStepOperationV4<F>,
    field: usize,
    state: &[AssignedValue<F>],
) {
    assert_slices_equal_if(
        ctx,
        range,
        condition,
        operation_full_limbs(operation, field),
        state,
    );
}

fn bind_digest_to_state_if<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    condition: AssignedValue<F>,
    operation: &AssignedKagemushaStepOperationV4<F>,
    first_field: usize,
    state: &[AssignedValue<F>],
) {
    let digest = operation_digest_limbs(operation, first_field);
    assert_slices_equal_if(ctx, range, condition, &digest, state);
}

/// Constrain the exact two-input application transition over already-assigned cells.
///
/// The relation is symmetric in the two parent slots. Absent parents are
/// mandatory all-zero vectors. Active parents bind their exact note material,
/// amount, and common historical root to the operation row; the historical
/// root is field 36 (`record_root_before`) and is intentionally **not** field
/// 38 (`transfer_root`). Top-up anchors and extended branch claims are
/// constrained as canonical set unions with exact zero padding.
///
/// This function does not prove confidential openings or Merkle paths. StepEq,
/// the pair's semantic authority, additionally copy-binds the returned cells to
/// the assigned secure relation from `confidential_v2`. StepEp is the
/// lineage-and-reciprocal wrapper: it shares StepEq's compact public header but
/// intentionally does not duplicate this application relation.
pub fn constrain_two_input_step_transition_v4<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    sha_jobs: &mut KagemushaSha256JobsV4<F>,
    parent_count: AssignedValue<F>,
    parent_states: [&[AssignedValue<F>]; 2],
    result_state: &[AssignedValue<F>],
    operation_limbs: &[AssignedValue<F>; KAGEMUSHA_STEP_OPERATION_LIMBS_V4],
) -> Result<NamedTransitionBindings<F>, String> {
    if parent_states
        .iter()
        .any(|state| state.len() != KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5)
        || result_state.len() != KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5
    {
        return Err("Kagemusha Step state-vector length mismatch".to_owned());
    }
    for limb in parent_states
        .iter()
        .flat_map(|state| state.iter())
        .chain(result_state)
    {
        range.range_check(ctx, *limb, 32);
    }

    let operation = assign_kagemusha_step_operation_v4(ctx, range, operation_limbs);
    let gate = &range.gate;
    let fields = &operation.fields;
    let one = ctx.load_constant(F::ONE);
    let zero = ctx.load_constant(F::ZERO);

    let append = fields[O_APPEND_PROFILE];
    let redemption = fields[O_REDEMPTION_PROFILE];
    let extends = gate.add(ctx, append, redemption);
    gate.assert_bit(ctx, extends);
    let init = gate.not(ctx, extends);
    let profile_overlap = gate.mul(ctx, append, redemption);
    gate.assert_is_const(ctx, &profile_overlap, &F::ZERO);

    let parent_present = constrain_small_count(ctx, range, parent_count);
    let parent_absent = parent_present.map(|present| gate.not(ctx, present));
    for slot in 0..2 {
        assert_slice_zero_if(ctx, range, parent_absent[slot], parent_states[slot]);
        assert_equal_if(
            ctx,
            range,
            parent_present[slot],
            parent_states[slot][S_VERSION],
            QuantumCell::Constant(F::from(u64::from(
                super::kagemusha_v2::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5,
            ))),
        );
    }
    gate.assert_is_const(
        ctx,
        &result_state[S_VERSION],
        &F::from(u64::from(
            super::kagemusha_v2::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5,
        )),
    );

    let input_next_zero_leaf_index = parent_states[0][S_NEXT_ZERO_LEAF_INDEX];
    let output_next_zero_leaf_index = result_state[S_NEXT_ZERO_LEAF_INDEX];
    range.range_check(
        ctx,
        output_next_zero_leaf_index,
        KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2,
    );
    for slot in 0..2 {
        range.range_check(
            ctx,
            parent_states[slot][S_NEXT_ZERO_LEAF_INDEX],
            KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2,
        );
        assert_equal_if(
            ctx,
            range,
            parent_present[slot],
            parent_states[slot][S_NEXT_ZERO_LEAF_INDEX],
            input_next_zero_leaf_index,
        );
    }

    // Parent cardinality is operation input cardinality. Init consumes zero,
    // append consumes one or two, and the current redemption wire consumes one.
    assert_equal(ctx, range, parent_count, fields[O_RECORD_INPUT_COUNT]);
    assert_equal(ctx, range, parent_count, fields[O_TRANSFER_INPUT_COUNT]);
    assert_zero_if(ctx, range, init, parent_count);
    assert_nonzero_if(ctx, range, append, parent_count);
    assert_equal_if(
        ctx,
        range,
        redemption,
        parent_count,
        QuantumCell::Constant(F::ONE),
    );
    let expected_output_next_zero_leaf_index = gate.add(
        ctx,
        input_next_zero_leaf_index,
        fields[O_RECORD_OUTPUT_COUNT],
    );
    assert_equal_if(
        ctx,
        range,
        extends,
        output_next_zero_leaf_index,
        expected_output_next_zero_leaf_index,
    );

    let branch = fields[O_BRANCH_CHANGE];
    let has_change = fields[O_HAS_CHANGE];
    let not_has_change = gate.not(ctx, has_change);
    let not_redemption = gate.not(ctx, redemption);
    let not_append = gate.not(ctx, append);
    assert_zero_if(ctx, range, init, branch);
    assert_zero_if(ctx, range, init, has_change);
    let branch_without_change = gate.mul(ctx, branch, not_has_change);
    gate.assert_is_const(ctx, &branch_without_change, &F::ZERO);
    assert_equal_if(
        ctx,
        range,
        redemption,
        branch,
        QuantumCell::Constant(F::ONE),
    );
    assert_equal_if(
        ctx,
        range,
        redemption,
        has_change,
        QuantumCell::Constant(F::ONE),
    );
    for swap in [fields[O_RECORD_OUTPUT_SWAP], fields[O_TRANSFER_OUTPUT_SWAP]] {
        assert_zero_if(ctx, range, init, swap);
        assert_zero_if(ctx, range, redemption, swap);
        assert_zero_if(ctx, range, not_has_change, swap);
    }

    // Exact chain/asset context and artifact identity across active parents.
    bind_full_field_to_state_if(
        ctx,
        range,
        one,
        &operation,
        O_CHAIN_TAG,
        &result_state[S_CHAIN_TAG..S_CHAIN_TAG + 8],
    );
    bind_full_field_to_state_if(
        ctx,
        range,
        one,
        &operation,
        O_ASSET_TAG,
        &result_state[S_ASSET_TAG..S_ASSET_TAG + 8],
    );
    assert_equal(
        ctx,
        range,
        result_state[S_ASSET_SCALE],
        fields[O_ASSET_SCALE],
    );
    bind_digest_to_state_if(
        ctx,
        range,
        one,
        &operation,
        O_ARTIFACT_MANIFEST_SHA256,
        &result_state[S_ARTIFACT_MANIFEST_SHA256..S_ARTIFACT_MANIFEST_SHA256 + 8],
    );
    bind_digest_to_state_if(
        ctx,
        range,
        one,
        &operation,
        O_VERIFIER_KEY_ID_DIGEST,
        &result_state[S_VERIFIER_KEY_ID..S_VERIFIER_KEY_ID + 8],
    );
    for slot in 0..2 {
        let parent = parent_states[slot];
        assert_slices_equal_if(
            ctx,
            range,
            parent_present[slot],
            &parent[S_CHAIN_TAG..S_CHAIN_TAG + 8],
            &result_state[S_CHAIN_TAG..S_CHAIN_TAG + 8],
        );
        assert_slices_equal_if(
            ctx,
            range,
            parent_present[slot],
            &parent[S_ASSET_TAG..S_ASSET_TAG + 8],
            &result_state[S_ASSET_TAG..S_ASSET_TAG + 8],
        );
        assert_equal_if(
            ctx,
            range,
            parent_present[slot],
            parent[S_ASSET_SCALE],
            result_state[S_ASSET_SCALE],
        );
        assert_slices_equal_if(
            ctx,
            range,
            parent_present[slot],
            &parent[S_ARTIFACT_MANIFEST_SHA256..S_ARTIFACT_MANIFEST_SHA256 + 8],
            &result_state[S_ARTIFACT_MANIFEST_SHA256..S_ARTIFACT_MANIFEST_SHA256 + 8],
        );
        assert_slices_equal_if(
            ctx,
            range,
            parent_present[slot],
            &parent[S_VERIFIER_KEY_ID..S_VERIFIER_KEY_ID + 8],
            &result_state[S_VERIFIER_KEY_ID..S_VERIFIER_KEY_ID + 8],
        );
    }
    for scale in [
        O_INPUT_SCALE,
        O_TRANSFER_SCALE,
        O_RECIPIENT_SCALE,
        O_CURRENT_SCALE,
    ] {
        assert_equal(ctx, range, fields[scale], fields[O_ASSET_SCALE]);
    }
    let expected_change_scale = gate.mul(ctx, has_change, fields[O_ASSET_SCALE]);
    assert_equal(ctx, range, fields[O_CHANGE_SCALE], expected_change_scale);

    // Parent historical roots are equal by the local append contract and bind
    // to field 36. Field 38 remains operation-specific and is not used here as
    // a parent creation root.
    for slot in 0..2 {
        bind_full_field_to_state_if(
            ctx,
            range,
            parent_present[slot],
            &operation,
            O_RECORD_ROOT_BEFORE,
            &parent_states[slot][S_FINAL_ROOT..S_FINAL_ROOT + 8],
        );
    }
    bind_full_field_to_state_if(
        ctx,
        range,
        one,
        &operation,
        O_FINAL_ROOT,
        &result_state[S_FINAL_ROOT..S_FINAL_ROOT + 8],
    );
    assert_equal(
        ctx,
        range,
        fields[O_RECORD_ROOT_AFTER],
        fields[O_FINAL_ROOT],
    );
    assert_equal_if(
        ctx,
        range,
        init,
        fields[O_RECORD_ROOT_BEFORE],
        fields[O_INITIAL_ROOT],
    );
    assert_zero_if(ctx, range, extends, fields[O_INITIAL_ROOT]);
    assert_equal_if(
        ctx,
        range,
        extends,
        fields[O_PARENT_FINAL_ROOT],
        fields[O_RECORD_ROOT_BEFORE],
    );
    assert_zero_if(ctx, range, init, fields[O_PARENT_FINAL_ROOT]);
    assert_equal_if(
        ctx,
        range,
        append,
        fields[O_TRANSFER_ROOT],
        fields[O_FINAL_ROOT],
    );
    assert_equal_if(
        ctx,
        range,
        redemption,
        fields[O_TRANSFER_ROOT],
        fields[O_RECORD_ROOT_BEFORE],
    );
    assert_equal_if(
        ctx,
        range,
        init,
        fields[O_TRANSFER_ROOT],
        fields[O_FINAL_ROOT],
    );
    for root in [fields[O_RECORD_ROOT_BEFORE], fields[O_FINAL_ROOT]] {
        assert_nonzero_if(ctx, range, one, root);
    }
    let roots_equal = gate.is_equal(ctx, fields[O_RECORD_ROOT_BEFORE], fields[O_FINAL_ROOT]);
    gate.assert_is_const(ctx, &roots_equal, &F::ZERO);

    // Bind exact parent note material to both confidential/record views.
    for slot in 0..2 {
        let parent = parent_states[slot];
        let commitment_field = O_TRANSFER_INPUT_COMMITMENT_0 + slot;
        let nullifier_field = O_TRANSFER_NULLIFIER_0 + slot;
        bind_full_field_to_state_if(
            ctx,
            range,
            one,
            &operation,
            commitment_field,
            &parent[S_CURRENT_COMMITMENT..S_CURRENT_COMMITMENT + 8],
        );
        bind_full_field_to_state_if(
            ctx,
            range,
            one,
            &operation,
            nullifier_field,
            &parent[S_CURRENT_NULLIFIER..S_CURRENT_NULLIFIER + 8],
        );
        assert_equal(
            ctx,
            range,
            fields[O_RECORD_INPUT_NULLIFIER_0 + slot],
            fields[nullifier_field],
        );
        assert_nonzero_if(ctx, range, parent_present[slot], fields[commitment_field]);
        assert_nonzero_if(ctx, range, parent_present[slot], fields[nullifier_field]);
    }
    assert_equal(
        ctx,
        range,
        fields[O_INPUT_COMMITMENT],
        fields[O_TRANSFER_INPUT_COMMITMENT_0],
    );
    assert_equal(
        ctx,
        range,
        fields[O_INPUT_NULLIFIER],
        fields[O_TRANSFER_NULLIFIER_0],
    );
    let duplicate_input_commitments = gate.is_equal(
        ctx,
        fields[O_TRANSFER_INPUT_COMMITMENT_0],
        fields[O_TRANSFER_INPUT_COMMITMENT_1],
    );
    assert_zero_if(ctx, range, parent_present[1], duplicate_input_commitments);
    let duplicate_input_nullifiers = gate.is_equal(
        ctx,
        fields[O_TRANSFER_NULLIFIER_0],
        fields[O_TRANSFER_NULLIFIER_1],
    );
    assert_zero_if(ctx, range, parent_present[1], duplicate_input_nullifiers);

    // Exact u128 conservation and result-note mapping.
    let parent_amounts: [AssignedValue<F>; 2] = std::array::from_fn(|slot| {
        reconstruct_state_u128(
            ctx,
            range,
            &parent_states[slot][S_CURRENT_AMOUNT..S_CURRENT_AMOUNT + 4],
        )
    });
    for slot in 0..2 {
        assert_nonzero_if(ctx, range, parent_present[slot], parent_amounts[slot]);
    }
    let parent_sum = gate.add(ctx, parent_amounts[0], parent_amounts[1]);
    let input_amount = operation_u128(ctx, range, &operation, O_INPUT_AMOUNT_LO);
    // An extending operation consumes the exact parent sum. Initialization
    // has no parent but uses this slot for the shielded public amount, so it is
    // tied to the sole recipient/top-up amount below instead of to zero.
    assert_equal_if(ctx, range, extends, parent_sum, input_amount);
    range.range_check(
        ctx,
        input_amount,
        super::confidential_v2::ConfidentialUnsignedRangeV1::Amount.bits(),
    );
    let recipient_amount = operation_u128(ctx, range, &operation, O_RECIPIENT_AMOUNT_LO);
    let change_amount = operation_u128(ctx, range, &operation, O_CHANGE_AMOUNT_LO);
    let output_sum = gate.add(ctx, recipient_amount, change_amount);
    assert_equal(ctx, range, input_amount, output_sum);
    let transfer_amount = operation_u128(ctx, range, &operation, O_TRANSFER_AMOUNT_LO);
    assert_equal(ctx, range, transfer_amount, recipient_amount);
    assert_nonzero_if(ctx, range, not_redemption, recipient_amount);
    assert_nonzero_if(ctx, range, has_change, change_amount);
    assert_zero_if(ctx, range, not_has_change, change_amount);

    let current_amount = operation_u128(ctx, range, &operation, O_CURRENT_AMOUNT_LO);
    let selected_amount = gate.select(ctx, change_amount, recipient_amount, branch);
    assert_equal(ctx, range, current_amount, selected_amount);
    assert_equal(
        ctx,
        range,
        result_state[S_CURRENT_AMOUNT],
        operation.limbs[O_CURRENT_AMOUNT_LO * 8],
    );
    assert_equal(
        ctx,
        range,
        result_state[S_CURRENT_AMOUNT + 1],
        operation.limbs[O_CURRENT_AMOUNT_LO * 8 + 1],
    );
    assert_equal(
        ctx,
        range,
        result_state[S_CURRENT_AMOUNT + 2],
        operation.limbs[O_CURRENT_AMOUNT_HI * 8],
    );
    assert_equal(
        ctx,
        range,
        result_state[S_CURRENT_AMOUNT + 3],
        operation.limbs[O_CURRENT_AMOUNT_HI * 8 + 1],
    );
    assert_equal(
        ctx,
        range,
        result_state[S_CURRENT_SCALE],
        fields[O_CURRENT_SCALE],
    );

    let recipient_present = gate.not(ctx, redemption);
    assert_nonzero_if(
        ctx,
        range,
        recipient_present,
        fields[O_RECIPIENT_COMMITMENT],
    );
    assert_nonzero_if(ctx, range, recipient_present, fields[O_RECIPIENT_NULLIFIER]);
    assert_zero_if(ctx, range, redemption, fields[O_RECIPIENT_COMMITMENT]);
    assert_zero_if(ctx, range, redemption, fields[O_RECIPIENT_NULLIFIER]);
    assert_nonzero_if(ctx, range, has_change, fields[O_CHANGE_COMMITMENT]);
    assert_nonzero_if(ctx, range, has_change, fields[O_CHANGE_NULLIFIER]);
    assert_zero_if(ctx, range, not_has_change, fields[O_CHANGE_COMMITMENT]);
    assert_zero_if(ctx, range, not_has_change, fields[O_CHANGE_NULLIFIER]);
    let selected_commitment = gate.select(
        ctx,
        fields[O_CHANGE_COMMITMENT],
        fields[O_RECIPIENT_COMMITMENT],
        branch,
    );
    let selected_nullifier = gate.select(
        ctx,
        fields[O_CHANGE_NULLIFIER],
        fields[O_RECIPIENT_NULLIFIER],
        branch,
    );
    assert_equal(
        ctx,
        range,
        fields[O_CURRENT_COMMITMENT],
        selected_commitment,
    );
    assert_equal(ctx, range, fields[O_CURRENT_NULLIFIER], selected_nullifier);
    bind_full_field_to_state_if(
        ctx,
        range,
        one,
        &operation,
        O_CURRENT_COMMITMENT,
        &result_state[S_CURRENT_COMMITMENT..S_CURRENT_COMMITMENT + 8],
    );
    bind_full_field_to_state_if(
        ctx,
        range,
        one,
        &operation,
        O_CURRENT_NULLIFIER,
        &result_state[S_CURRENT_NULLIFIER..S_CURRENT_NULLIFIER + 8],
    );

    let append_change = gate.mul(ctx, append, has_change);
    let expected_output_count = gate.add(ctx, one, append_change);
    assert_equal(
        ctx,
        range,
        fields[O_RECORD_OUTPUT_COUNT],
        expected_output_count,
    );
    assert_equal(
        ctx,
        range,
        fields[O_TRANSFER_OUTPUT_COUNT],
        expected_output_count,
    );
    let record_0 = gate.select(
        ctx,
        fields[O_CHANGE_COMMITMENT],
        fields[O_RECIPIENT_COMMITMENT],
        fields[O_RECORD_OUTPUT_SWAP],
    );
    let record_1 = gate.select(
        ctx,
        fields[O_RECIPIENT_COMMITMENT],
        fields[O_CHANGE_COMMITMENT],
        fields[O_RECORD_OUTPUT_SWAP],
    );
    let transfer_0 = gate.select(
        ctx,
        fields[O_CHANGE_COMMITMENT],
        fields[O_RECIPIENT_COMMITMENT],
        fields[O_TRANSFER_OUTPUT_SWAP],
    );
    let transfer_1 = gate.select(
        ctx,
        fields[O_RECIPIENT_COMMITMENT],
        fields[O_CHANGE_COMMITMENT],
        fields[O_TRANSFER_OUTPUT_SWAP],
    );
    let primary_output = gate.select(ctx, fields[O_CHANGE_COMMITMENT], record_0, redemption);
    let primary_transfer = gate.select(ctx, fields[O_CHANGE_COMMITMENT], transfer_0, redemption);
    assert_equal(ctx, range, fields[O_RECORD_OUTPUT_0], primary_output);
    assert_equal(ctx, range, fields[O_TRANSFER_OUTPUT_0], primary_transfer);
    assert_equal_if(ctx, range, append, fields[O_RECORD_OUTPUT_1], record_1);
    assert_equal_if(ctx, range, append, fields[O_TRANSFER_OUTPUT_1], transfer_1);
    assert_zero_if(ctx, range, not_append, fields[O_RECORD_OUTPUT_1]);
    assert_zero_if(ctx, range, not_append, fields[O_TRANSFER_OUTPUT_1]);

    // Counters are exact max-parent transitions, not host-provided summaries.
    let parent_step_lt = range.is_less_than(
        ctx,
        parent_states[0][S_PROOF_STEP_COUNT],
        parent_states[1][S_PROOF_STEP_COUNT],
        32,
    );
    let max_parent_step = gate.select(
        ctx,
        parent_states[1][S_PROOF_STEP_COUNT],
        parent_states[0][S_PROOF_STEP_COUNT],
        parent_step_lt,
    );
    let parent_hop_lt = range.is_less_than(
        ctx,
        parent_states[0][S_PEER_HOP_COUNT],
        parent_states[1][S_PEER_HOP_COUNT],
        32,
    );
    let max_parent_hop = gate.select(
        ctx,
        parent_states[1][S_PEER_HOP_COUNT],
        parent_states[0][S_PEER_HOP_COUNT],
        parent_hop_lt,
    );
    let expected_proof_step = gate.add(ctx, max_parent_step, one);
    let expected_peer_hop = gate.add(ctx, max_parent_hop, append);
    assert_equal(
        ctx,
        range,
        result_state[S_PROOF_STEP_COUNT],
        expected_proof_step,
    );
    assert_equal(
        ctx,
        range,
        result_state[S_PEER_HOP_COUNT],
        expected_peer_hop,
    );
    assert_equal(ctx, range, fields[O_PROOF_STEP_COUNT], expected_proof_step);
    assert_equal(ctx, range, fields[O_PEER_HOP_COUNT], expected_peer_hop);
    assert_equal(
        ctx,
        range,
        fields[O_PREVIOUS_PROOF_STEP_COUNT],
        max_parent_step,
    );
    assert_equal(
        ctx,
        range,
        fields[O_PREVIOUS_PEER_HOP_COUNT],
        max_parent_hop,
    );

    // Canonical top-up anchor union.
    let result_anchor_count = result_state[S_TOPUP_ANCHOR_COUNT];
    let result_anchor_present = constrain_small_count(ctx, range, result_anchor_count);
    assert_nonzero_if(ctx, range, one, result_anchor_count);
    assert_equal(
        ctx,
        range,
        result_anchor_count,
        fields[O_TOPUP_ANCHOR_COUNT],
    );
    let result_anchors: [&[AssignedValue<F>]; ANCHOR_SLOTS] = std::array::from_fn(|slot| {
        let start = S_TOPUP_ANCHORS + slot * ANCHOR_LIMBS;
        &result_state[start..start + ANCHOR_LIMBS]
    });
    let result_second_anchor_absent = gate.not(ctx, result_anchor_present[1]);
    assert_slice_zero_if(ctx, range, result_second_anchor_absent, result_anchors[1]);
    let anchors_ordered = anchor_less(ctx, range, result_anchors[0], result_anchors[1]);
    assert_equal_if(
        ctx,
        range,
        result_anchor_present[1],
        anchors_ordered,
        QuantumCell::Constant(F::ONE),
    );

    let topup_operation_id = operation_digest_limbs(&operation, O_TOPUP_OPERATION_ID);
    let topup_anchor_digest = operation_digest_limbs(&operation, O_TOPUP_ANCHOR_DIGEST);
    assert_slices_equal_if(
        ctx,
        range,
        one,
        &topup_operation_id,
        &result_anchors[0][..8],
    );
    assert_slices_equal_if(
        ctx,
        range,
        one,
        &topup_anchor_digest,
        &result_anchors[0][8..],
    );
    assert_equal_if(
        ctx,
        range,
        init,
        result_anchor_count,
        QuantumCell::Constant(F::ONE),
    );

    let mut parent_anchor_storage: Vec<(&[AssignedValue<F>], AssignedValue<F>)> = Vec::new();
    for slot in 0..2 {
        let parent = parent_states[slot];
        let count = parent[S_TOPUP_ANCHOR_COUNT];
        let local_present = constrain_small_count(ctx, range, count);
        assert_nonzero_if(ctx, range, parent_present[slot], count);
        assert_zero_if(ctx, range, parent_absent[slot], count);
        for anchor_slot in 0..2 {
            let start = S_TOPUP_ANCHORS + anchor_slot * ANCHOR_LIMBS;
            let present = gate.mul(ctx, parent_present[slot], local_present[anchor_slot]);
            let absent = gate.not(ctx, present);
            assert_slice_zero_if(ctx, range, absent, &parent[start..start + ANCHOR_LIMBS]);
            parent_anchor_storage.push((&parent[start..start + ANCHOR_LIMBS], present));
        }
        let ordered = anchor_less(
            ctx,
            range,
            &parent[S_TOPUP_ANCHORS..S_TOPUP_ANCHORS + ANCHOR_LIMBS],
            &parent[S_TOPUP_ANCHORS + ANCHOR_LIMBS..S_TOPUP_ANCHORS + 2 * ANCHOR_LIMBS],
        );
        let require_order = gate.mul(ctx, parent_present[slot], local_present[1]);
        assert_equal_if(
            ctx,
            range,
            require_order,
            ordered,
            QuantumCell::Constant(F::ONE),
        );
    }
    let result_anchor_storage = [
        (result_anchors[0], result_anchor_present[0]),
        (result_anchors[1], result_anchor_present[1]),
    ];
    // Init seeds one anchor; extending transitions inherit the exact set union.
    let gated_parent_anchor_storage = parent_anchor_storage
        .iter()
        .map(|(anchor, present)| (*anchor, gate.mul(ctx, extends, *present)))
        .collect::<Vec<_>>();
    let gated_result_anchor_storage = result_anchor_storage
        .iter()
        .map(|(anchor, present)| (*anchor, gate.mul(ctx, extends, *present)))
        .collect::<Vec<_>>();
    constrain_set_union(
        ctx,
        range,
        &gated_parent_anchor_storage,
        &gated_result_anchor_storage,
    );

    // Exact branch-claim extension and canonical union.
    let result_claim_count = result_state[S_BRANCH_CLAIM_COUNT];
    let result_claim_present = constrain_small_count(ctx, range, result_claim_count);
    assert_nonzero_if(ctx, range, one, result_claim_count);
    assert_equal(ctx, range, result_claim_count, result_anchor_count);
    let result_claims: [&[AssignedValue<F>]; CLAIM_SLOTS] = std::array::from_fn(|slot| {
        let start = S_BRANCH_CLAIMS + slot * KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V5;
        &result_state[start..start + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V5]
    });
    let result_second_claim_absent = gate.not(ctx, result_claim_present[1]);
    assert_slice_zero_if(ctx, range, result_second_claim_absent, result_claims[1]);
    let claims_ordered = claim_path_less(ctx, range, result_claims[0], result_claims[1]);
    assert_equal_if(
        ctx,
        range,
        result_claim_present[1],
        claims_ordered,
        QuantumCell::Constant(F::ONE),
    );

    let tag8 = operation_digest_limbs(&operation, O_CURRENT_HOP_DOMAIN_TAG);
    let tag: [AssignedValue<F>; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V2] =
        tag8[..KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V2]
            .try_into()
            .expect("transition tag is six limbs");
    assert_slice_zero_if(ctx, range, one, &tag8[6..]);
    let split_digest_limbs = operation_digest_limbs(&operation, O_SPLIT_DIGEST);
    let split_digest_bytes = sha256_native_bytes(ctx, range, &split_digest_limbs);
    let mut tag_preimage = KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_DOMAIN_V2
        .bytes()
        .chain(std::iter::once(0))
        .map(KagemushaSha256ByteV4::constant)
        .collect::<Vec<_>>();
    tag_preimage.extend(split_digest_bytes);
    let expected_tag_words = sha_jobs.digest_constrained(ctx, &tag_preimage)?;
    for index in 0..tag.len() {
        let expected = byte_swap_u32(ctx, range, expected_tag_words[index]);
        assert_equal_if(ctx, range, extends, tag[index], expected);
    }
    let tag_is_zero = slices_equal(ctx, range, &tag, &[zero; 6]);
    assert_zero_if(ctx, range, extends, tag_is_zero);
    assert_slice_zero_if(ctx, range, init, &tag);

    // Init root claim is the exact anchor digest with an empty path and the
    // all-zero initial history accumulator.
    assert_slices_equal_if(
        ctx,
        range,
        init,
        &result_claims[0][CLAIM_LINEAGE_ROOT..CLAIM_DEPTH],
        &result_anchors[0][8..16],
    );
    assert_zero_if(ctx, range, init, result_claims[0][CLAIM_DEPTH]);
    assert_slice_zero_if(ctx, range, init, &result_claims[0][CLAIM_PATH..]);

    let mut extended_claims = Vec::new();
    let mut extended_presence = Vec::new();
    for slot in 0..2 {
        let parent = parent_states[slot];
        let count = parent[S_BRANCH_CLAIM_COUNT];
        let local_present = constrain_small_count(ctx, range, count);
        assert_nonzero_if(ctx, range, parent_present[slot], count);
        assert_zero_if(ctx, range, parent_absent[slot], count);
        assert_equal_if(
            ctx,
            range,
            parent_present[slot],
            count,
            parent[S_TOPUP_ANCHOR_COUNT],
        );
        for claim_slot in 0..2 {
            let start = S_BRANCH_CLAIMS
                + claim_slot * KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V5;
            let claim =
                &parent[start..start + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V5];
            let present = gate.mul(ctx, parent_present[slot], local_present[claim_slot]);
            let absent = gate.not(ctx, present);
            assert_slice_zero_if(ctx, range, absent, claim);
            extended_claims.push(extend_claim(ctx, range, sha_jobs, claim, branch, &tag)?);
            extended_presence.push(present);
        }
        let first = S_BRANCH_CLAIMS;
        let second = first + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V5;
        let ordered = claim_path_less(
            ctx,
            range,
            &parent[first..first + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V5],
            &parent[second..second + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V5],
        );
        let require_order = gate.mul(ctx, parent_present[slot], local_present[1]);
        assert_equal_if(
            ctx,
            range,
            require_order,
            ordered,
            QuantumCell::Constant(F::ONE),
        );
    }
    let extended_sources = extended_claims
        .iter()
        .zip(&extended_presence)
        .map(|(claim, present)| (claim.as_slice(), gate.mul(ctx, extends, *present)))
        .collect::<Vec<_>>();
    let extended_results = result_claims
        .iter()
        .zip(result_claim_present)
        .map(|(claim, present)| (*claim, gate.mul(ctx, extends, present)))
        .collect::<Vec<_>>();
    constrain_set_union(ctx, range, &extended_sources, &extended_results);

    // Anchor references are canonicalized by `(topup_operation_id,
    // anchor_digest)`, while claims are canonicalized by lineage path beginning
    // with `anchor_digest`. Those independent orders need not agree, so bind
    // the two unique lineage-root sets rather than equating matching slots.
    let result_anchor_roots: [&[AssignedValue<F>]; ANCHOR_SLOTS] =
        std::array::from_fn(|slot| &result_anchors[slot][8..16]);
    let result_claim_roots: [&[AssignedValue<F>]; CLAIM_SLOTS] =
        std::array::from_fn(|slot| &result_claims[slot][CLAIM_LINEAGE_ROOT..CLAIM_DEPTH]);
    constrain_lineage_root_set_equality(
        ctx,
        range,
        result_anchor_roots,
        result_anchor_present,
        result_claim_roots,
        result_claim_present,
    );

    // Bind canonical first-claim operation fields deterministically while the
    // rolling history accumulators remain in the exact result state. The
    // complete tag sequence remains in the public branch claim.
    bind_digest_to_state_if(
        ctx,
        range,
        one,
        &operation,
        O_BRANCH_LINEAGE_ROOT,
        &result_claims[0][CLAIM_LINEAGE_ROOT..CLAIM_DEPTH],
    );
    assert_equal(
        ctx,
        range,
        fields[O_BRANCH_DEPTH],
        result_claims[0][CLAIM_DEPTH],
    );
    let result_path = operation_path_from_state(
        ctx,
        range,
        &result_claims[0][CLAIM_PATH..CLAIM_HISTORY_ACCUMULATOR],
    );
    assert_equal(ctx, range, fields[O_BRANCH_PATH_BITS], result_path);
    let expected_parent_depth = gate.sub(ctx, fields[O_BRANCH_DEPTH], extends);
    assert_equal(
        ctx,
        range,
        fields[O_PARENT_BRANCH_DEPTH],
        expected_parent_depth,
    );
    for (index, limb) in operation_digest_limbs(&operation, O_PARENT_BRANCH_LINEAGE_ROOT)
        .into_iter()
        .enumerate()
    {
        let result_limb = result_claims[0][CLAIM_LINEAGE_ROOT + index];
        assert_equal_if(ctx, range, extends, limb, result_limb);
        assert_zero_if(ctx, range, init, limb);
    }
    let mut parent_path = fields[O_BRANCH_PATH_BITS];
    for depth in 0..64 {
        let selected_depth = gate.is_equal(
            ctx,
            fields[O_PARENT_BRANCH_DEPTH],
            QuantumCell::Constant(F::from(depth as u64)),
        );
        let selected_branch = gate.mul(ctx, branch, selected_depth);
        let bit = gate.mul(
            ctx,
            selected_branch,
            QuantumCell::Constant(F::from_u128(1_u128 << (63 - depth))),
        );
        parent_path = gate.sub(ctx, parent_path, bit);
    }
    assert_equal(ctx, range, fields[O_PARENT_BRANCH_PATH_BITS], parent_path);

    // Receipt/finality binding hook. This does not treat a host finality flag
    // as proof: init exposes its exact receipt, while descendants carry the
    // same receipt binding from the operation row.
    let receipt = operation_digest_limbs(&operation, O_TOPUP_RECEIPT_DIGEST);
    let parent_receipt = operation_digest_limbs(&operation, O_PARENT_TOPUP_RECEIPT_DIGEST);
    assert_slices_equal_if(ctx, range, one, &receipt, &topup_anchor_digest);
    assert_slice_zero_if(ctx, range, init, &parent_receipt);
    assert_slices_equal_if(ctx, range, extends, &receipt, &parent_receipt);
    let receipt_zero = slices_equal(ctx, range, &receipt, &[zero; 8]);
    assert_zero_if(ctx, range, one, receipt_zero);

    let statement_digest_limbs = operation_digest_limbs(&operation, O_STATEMENT_DIGEST);
    let init_payer_tag_limbs = operation_digest_limbs(&operation, O_RECIPIENT_REQUEST_DIGEST);
    let init_operation_tag_limbs = operation_digest_limbs(&operation, O_OPERATION_ID);
    Ok(NamedTransitionBindings {
        input_root: fields[O_RECORD_ROOT_BEFORE],
        output_root: fields[O_FINAL_ROOT],
        input_next_zero_leaf_index,
        output_next_zero_leaf_index,
        input_commitments: [
            fields[O_TRANSFER_INPUT_COMMITMENT_0],
            fields[O_TRANSFER_INPUT_COMMITMENT_1],
        ],
        input_nullifiers: [
            fields[O_TRANSFER_NULLIFIER_0],
            fields[O_TRANSFER_NULLIFIER_1],
        ],
        recipient_commitment: fields[O_RECIPIENT_COMMITMENT],
        change_commitment: fields[O_CHANGE_COMMITMENT],
        statement_digest_limbs,
        init_payer_tag_limbs,
        init_operation_tag_limbs,
        operation,
        is_init: init,
        is_append: append,
        is_redemption: redemption,
        has_change,
    })
}

#[cfg(test)]
mod tests {
    use crate::zk::kagemusha_v2::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5;
    use halo2_base::{
        AssignedValue, gates::circuit::builder::BaseCircuitBuilder, utils::BigPrimeField,
    };
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
    };

    use super::*;

    #[test]
    fn step_binding_digest_ignores_ambient_norito_layout() {
        let binding = "kagemusha-step-binding".to_owned();
        let expected = canonical_binding_digest(&binding).expect("canonical binding digest");
        let canonical = norito::encode_canonical(&binding).expect("canonical binding frame");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_ne!(
            norito::to_bytes(&binding).expect("alternate-layout binding frame"),
            canonical
        );
        assert_eq!(
            canonical_binding_digest(&binding).expect("ambient-independent binding digest"),
            expected
        );
    }

    #[test]
    fn v4_operation_constructors_have_direct_typed_inputs() {
        let _: fn(
            &KagemushaRecursiveSpendInitRequestV4,
            &KagemushaRecursiveSpendPublicStatementV4,
            &super::super::confidential_v2::KagemushaTopUpShieldPublicInputsV2,
            &KagemushaOutputMembershipWitnessV4,
        ) -> Result<KagemushaStepOperationVectorV4, String> =
            KagemushaStepOperationVectorV4::from_init_v4;
        let _: fn(
            &KagemushaRecursiveSpendSplitIntentV4,
            &KagemushaRecursiveSpendPublicStatementV4,
            &KagemushaStepTransferPublicV4,
            &KagemushaOutputMembershipWitnessV4,
        ) -> Result<KagemushaStepOperationVectorV4, String> =
            KagemushaStepOperationVectorV4::from_append_v4;
        let _: fn(
            &KagemushaRecursiveSpendRedemptionIntentV4,
            &KagemushaRecursiveSpendPublicStatementV4,
            &KagemushaOutputMembershipWitnessV4,
        ) -> Result<KagemushaStepOperationVectorV4, String> =
            KagemushaStepOperationVectorV4::from_redemption_change_v4;
    }

    fn scalar_limbs(value: Fp) -> [u32; 8] {
        let repr = value.to_repr();
        std::array::from_fn(|index| {
            u32::from_le_bytes(
                repr.as_ref()[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("four-byte scalar limb"),
            )
        })
    }

    fn scalar_bytes(value: Fp) -> [u8; 32] {
        value
            .to_repr()
            .as_ref()
            .try_into()
            .expect("Pallas scalar representation")
    }

    fn terminal_init_statement_v4() -> KagemushaRecursiveSpendPublicStatementV4 {
        use iroha_data_model::{
            ChainId,
            asset::AssetDefinitionId,
            domain::DomainId,
            offline::{
                KagemushaPastaCycleParityV1, KagemushaRecursiveSpendArtifactBindingV4,
                KagemushaRecursiveSpendTopUpAnchorRefV2, KagemushaScaledAmountV2,
                KagemushaSpendableNoteDescriptorV2, kagemusha_recursive_spend_verifier_key_id_v4,
            },
        };

        let chain_id = ChainId::from("kagemusha-v4-terminal-operation-binding");
        let asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("asset domain"),
            "rose".parse().expect("asset name"),
        );
        let anchor = KagemushaRecursiveSpendTopUpAnchorRefV2 {
            topup_operation_id: [0x41; 32],
            anchor_digest: [0x42; 32],
        };
        let artifact_binding = KagemushaRecursiveSpendArtifactBindingV4 {
            version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: "release-generation-4".to_owned(),
            manifest_sha256: [0x43; 32],
        };
        let verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEq,
            artifact_binding.manifest_sha256,
        );
        KagemushaRecursiveSpendPublicStatementV4 {
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            asset_scale: 9,
            final_root: scalar_bytes(Fp::from(12)),
            next_zero_leaf_index: 8,
            topup_anchor_refs: vec![anchor],
            proof_step_count: 1,
            peer_hop_count: 0,
            current_note: KagemushaSpendableNoteDescriptorV2 {
                chain_id,
                asset,
                note_commitment: scalar_bytes(Fp::from(31)),
                spend_nullifier: scalar_bytes(Fp::from(32)),
                amount: KagemushaScaledAmountV2::new(10_750_000_000, 9).expect("amount"),
            },
            branch_claims: vec![
                KagemushaRecursiveSpendBranchClaimV2::root(anchor.anchor_digest)
                    .expect("root claim"),
            ],
            transition: None,
            artifact_binding,
            verifier_key_id,
        }
    }

    #[test]
    fn terminal_v4_operation_projection_rejects_init_profile_and_tag_substitution() {
        let statement = terminal_init_statement_v4();
        let mut fields = fill_statement_fields_v4(&statement).expect("valid V4 statement");
        let operation_tag = super::super::confidential_v2::derive_kagemusha_topup_operation_tag_v3(
            &statement.topup_anchor_refs[0].topup_operation_id,
        )
        .expect("top-up operation tag");
        put_digest(&mut fields, O_OPERATION_ID, operation_tag);
        // The payer tag is contextual and deliberately not derivable from the
        // compact public anchor reference.
        put_digest(&mut fields, O_RECIPIENT_REQUEST_DIGEST, [0x55; 32]);
        let operation = KagemushaStepOperationVectorV4::from_fields(fields);
        operation
            .validate_terminal_statement_v4(&statement)
            .expect("canonical init operation projection");

        let mut wrong_tag = operation.to_fields().expect("canonical operation fields");
        wrong_tag[O_OPERATION_ID] += Fp::ONE;
        assert!(
            KagemushaStepOperationVectorV4::from_fields(wrong_tag)
                .validate_terminal_statement_v4(&statement)
                .is_err()
        );

        let mut wrong_profile = operation.to_fields().expect("canonical operation fields");
        wrong_profile[O_REDEMPTION_PROFILE] = Fp::ONE;
        assert!(
            KagemushaStepOperationVectorV4::from_fields(wrong_profile)
                .validate_terminal_statement_v4(&statement)
                .is_err()
        );
    }

    fn exact_limbs(bytes: [u8; 32]) -> [u32; 8] {
        std::array::from_fn(|index| {
            u32::from_le_bytes(
                bytes[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("four-byte exact limb"),
            )
        })
    }

    fn write_full_state(state: &mut [u32], start: usize, value: Fp) {
        state[start..start + 8].copy_from_slice(&scalar_limbs(value));
    }

    fn write_digest_fields(
        fields: &mut [Fp; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4],
        start: usize,
        bytes: [u8; 32],
    ) {
        put_digest(fields, start, bytes);
    }

    fn init_fixture() -> (KagemushaStepOperationVectorV4, [Vec<u32>; 2], Vec<u32>) {
        let mut fields = [Fp::ZERO; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4];
        fields[O_LAYOUT_VERSION] = Fp::ONE;
        fields[O_PROOF_STEP_COUNT] = Fp::ONE;
        fields[O_ASSET_SCALE] = Fp::from(2);
        for index in [
            O_INPUT_SCALE,
            O_TRANSFER_SCALE,
            O_RECIPIENT_SCALE,
            O_CURRENT_SCALE,
        ] {
            fields[index] = Fp::from(2);
        }
        fields[O_RECORD_OUTPUT_COUNT] = Fp::ONE;
        fields[O_TRANSFER_OUTPUT_COUNT] = Fp::ONE;
        for index in [
            O_CURRENT_AMOUNT_LO,
            O_INPUT_AMOUNT_LO,
            O_TRANSFER_AMOUNT_LO,
            O_RECIPIENT_AMOUNT_LO,
        ] {
            fields[index] = Fp::from(100);
        }
        fields[O_INITIAL_ROOT] = Fp::from(11);
        fields[O_FINAL_ROOT] = Fp::from(12);
        fields[O_RECORD_ROOT_BEFORE] = Fp::from(11);
        fields[O_RECORD_ROOT_AFTER] = Fp::from(12);
        fields[O_TRANSFER_ROOT] = Fp::from(12);
        fields[O_CURRENT_COMMITMENT] = Fp::from(31);
        fields[O_CURRENT_NULLIFIER] = Fp::from(32);
        fields[O_RECIPIENT_COMMITMENT] = Fp::from(31);
        fields[O_RECIPIENT_NULLIFIER] = Fp::from(32);
        fields[O_RECORD_OUTPUT_0] = Fp::from(31);
        fields[O_TRANSFER_OUTPUT_0] = Fp::from(31);
        fields[O_ASSET_TAG] = Fp::from(51);
        fields[O_CHAIN_TAG] = Fp::from(52);
        fields[O_TOPUP_ANCHOR_COUNT] = Fp::ONE;

        let operation_id = [0x21; 32];
        let anchor_digest = [0x31; 32];
        let manifest = [0x41; 32];
        let verifier = [0x51; 32];
        write_digest_fields(&mut fields, O_STATEMENT_DIGEST, [0x11; 32]);
        write_digest_fields(
            &mut fields,
            O_RECIPIENT_REQUEST_DIGEST,
            scalar_bytes(Fp::from(61)),
        );
        write_digest_fields(&mut fields, O_OPERATION_ID, scalar_bytes(Fp::from(62)));
        write_digest_fields(&mut fields, O_TOPUP_OPERATION_ID, operation_id);
        write_digest_fields(&mut fields, O_TOPUP_ANCHOR_DIGEST, anchor_digest);
        write_digest_fields(&mut fields, O_TOPUP_RECEIPT_DIGEST, anchor_digest);
        write_digest_fields(&mut fields, O_BRANCH_LINEAGE_ROOT, anchor_digest);
        write_digest_fields(&mut fields, O_ARTIFACT_MANIFEST_SHA256, manifest);
        write_digest_fields(&mut fields, O_VERIFIER_KEY_ID_DIGEST, verifier);

        let mut result = vec![0_u32; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5];
        result[S_VERSION] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5;
        result[S_NEXT_ZERO_LEAF_INDEX] = 8;
        write_full_state(&mut result, S_CHAIN_TAG, Fp::from(52));
        write_full_state(&mut result, S_ASSET_TAG, Fp::from(51));
        result[S_ASSET_SCALE] = 2;
        write_full_state(&mut result, S_FINAL_ROOT, Fp::from(12));
        result[S_TOPUP_ANCHOR_COUNT] = 1;
        result[S_TOPUP_ANCHORS..S_TOPUP_ANCHORS + 8].copy_from_slice(&exact_limbs(operation_id));
        result[S_TOPUP_ANCHORS + 8..S_TOPUP_ANCHORS + 16]
            .copy_from_slice(&exact_limbs(anchor_digest));
        result[S_PROOF_STEP_COUNT] = 1;
        write_full_state(&mut result, S_CURRENT_COMMITMENT, Fp::from(31));
        write_full_state(&mut result, S_CURRENT_NULLIFIER, Fp::from(32));
        result[S_CURRENT_AMOUNT] = 100;
        result[S_CURRENT_SCALE] = 2;
        result[S_BRANCH_CLAIM_COUNT] = 1;
        result[S_BRANCH_CLAIMS..S_BRANCH_CLAIMS + 8].copy_from_slice(&exact_limbs(anchor_digest));
        result[S_ARTIFACT_MANIFEST_SHA256..S_ARTIFACT_MANIFEST_SHA256 + 8]
            .copy_from_slice(&exact_limbs(manifest));
        result[S_VERIFIER_KEY_ID..S_VERIFIER_KEY_ID + 8].copy_from_slice(&exact_limbs(verifier));

        (
            KagemushaStepOperationVectorV4::from_fields(fields),
            std::array::from_fn(|_| vec![0_u32; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5]),
            result,
        )
    }

    fn transition_builder<F: BigPrimeField>(
        operation: &KagemushaStepOperationVectorV4,
        parent_count: u32,
        parents: &[Vec<u32>; 2],
        result: &[u32],
        secure_init_tags: Option<[[u32; 8]; 2]>,
    ) -> BaseCircuitBuilder<F> {
        let mut builder = BaseCircuitBuilder::new(false).use_k(18).use_lookup_bits(17);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let operation_cells: [AssignedValue<F>; KAGEMUSHA_STEP_OPERATION_LIMBS_V4] = ctx
            .assign_witnesses(
                operation
                    .limbs
                    .iter()
                    .copied()
                    .map(|limb| F::from(u64::from(limb))),
            )
            .try_into()
            .expect("fixed operation limbs");
        let parent_cells = parents.each_ref().map(|parent| {
            ctx.assign_witnesses(parent.iter().copied().map(|limb| F::from(u64::from(limb))))
        });
        let result_cells =
            ctx.assign_witnesses(result.iter().copied().map(|limb| F::from(u64::from(limb))));
        let parent_count = ctx.load_witness(F::from(u64::from(parent_count)));
        let mut sha_jobs = KagemushaSha256JobsV4::default();
        let bindings = constrain_two_input_step_transition_v4(
            ctx,
            &range,
            &mut sha_jobs,
            parent_count,
            [parent_cells[0].as_slice(), parent_cells[1].as_slice()],
            &result_cells,
            &operation_cells,
        )
        .expect("fixed transition shape");
        if let Some([payer_limbs, operation_limbs]) = secure_init_tags {
            let from_limbs = |limbs: [u32; 8]| {
                let radix = F::from(1_u64 << 32);
                limbs.into_iter().rev().fold(F::ZERO, |value, limb| {
                    value * radix + F::from(u64::from(limb))
                })
            };
            let payer = ctx.load_witness(from_limbs(payer_limbs));
            let operation = ctx.load_witness(from_limbs(operation_limbs));
            constrain_kagemusha_step_init_topup_tags_v4(ctx, &range, &bindings, payer, operation);
        }
        builder.calculate_params(Some(9));
        builder
    }

    fn lineage_root_set_builder(
        anchor_roots: [[u32; 8]; ANCHOR_SLOTS],
        claim_roots: [[u32; 8]; CLAIM_SLOTS],
    ) -> BaseCircuitBuilder<Fp> {
        let mut builder = BaseCircuitBuilder::new(false).use_k(17).use_lookup_bits(16);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let anchor_cells = anchor_roots.map(|root| {
            ctx.assign_witnesses(root.into_iter().map(|limb| Fp::from(u64::from(limb))))
        });
        let claim_cells = claim_roots.map(|root| {
            ctx.assign_witnesses(root.into_iter().map(|limb| Fp::from(u64::from(limb))))
        });
        let one = ctx.load_constant(Fp::ONE);
        constrain_lineage_root_set_equality(
            ctx,
            &range,
            [anchor_cells[0].as_slice(), anchor_cells[1].as_slice()],
            [one; ANCHOR_SLOTS],
            [claim_cells[0].as_slice(), claim_cells[1].as_slice()],
            [one; CLAIM_SLOTS],
        );
        builder.calculate_params(Some(9));
        builder
    }

    fn claim_extension_builder(
        claim: &[u32; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V5],
        branch: u32,
        tag: &[u32; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V2],
        expected: &[u32; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V5],
    ) -> BaseCircuitBuilder<Fp> {
        let mut builder = BaseCircuitBuilder::new(false).use_k(18).use_lookup_bits(17);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let claim =
            ctx.assign_witnesses(claim.iter().copied().map(|limb| Fp::from(u64::from(limb))));
        let tag: [AssignedValue<Fp>;
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V2] = ctx
            .assign_witnesses(tag.iter().copied().map(|limb| Fp::from(u64::from(limb))))
            .try_into()
            .expect("fixed transition tag");
        let branch = ctx.load_witness(Fp::from(u64::from(branch)));
        let mut sha_jobs = KagemushaSha256JobsV4::default();
        let extended = extend_claim(ctx, &range, &mut sha_jobs, &claim, branch, &tag)
            .expect("fixed claim extension");
        let expected = ctx.assign_witnesses(
            expected
                .iter()
                .copied()
                .map(|limb| Fp::from(u64::from(limb))),
        );
        let one = ctx.load_constant(Fp::ONE);
        assert_slices_equal_if(ctx, &range, one, &extended, &expected);
        builder.calculate_params(Some(9));
        builder
    }

    #[test]
    fn operation_vector_round_trips_and_rejects_fp_modulus() {
        let (vector, _, _) = init_fixture();
        assert_eq!(
            KagemushaStepOperationVectorV4::from_fields(
                vector.to_fields().expect("canonical operation")
            ),
            vector
        );
        let mut noncanonical = vector;
        noncanonical.limbs[field_limb_range(O_CHAIN_TAG)]
            .copy_from_slice(&KAGEMUSHA_STEP_OPERATION_FP_MODULUS_U32_LE_V4);
        assert!(noncanonical.to_fields().is_err());
    }

    #[test]
    fn claim_extension_constrains_the_v5_rolling_history_accumulator() {
        let mut claim = [0_u32; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V5];
        claim[..8].copy_from_slice(&exact_limbs([0x31; 32]));
        let tag_bytes = [0x42; 24];
        let tag = std::array::from_fn(|index| {
            u32::from_le_bytes(
                tag_bytes[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("four-byte tag limb"),
            )
        });
        let accumulator =
            super::super::kagemusha_v2::kagemusha_recursive_spend_state_history_accumulator_v5(
                &tag_bytes,
            )
            .expect("canonical one-tag accumulator");
        let mut expected = claim;
        expected[CLAIM_DEPTH] = 1;
        expected[CLAIM_PATH] = 0x80;
        expected[CLAIM_HISTORY_ACCUMULATOR..].copy_from_slice(&exact_limbs(accumulator));

        let builder = claim_extension_builder(&claim, 1, &tag, &expected);
        MockProver::run(builder.config_params.k as u32, &builder, vec![])
            .expect("rolling-history claim-extension prover")
            .assert_satisfied();

        let mut substituted = expected;
        substituted[CLAIM_HISTORY_ACCUMULATOR] ^= 1;
        let builder = claim_extension_builder(&claim, 1, &tag, &substituted);
        assert!(
            MockProver::run(builder.config_params.k as u32, &builder, vec![])
                .expect("rolling-history substitution prover")
                .verify()
                .is_err()
        );
    }

    #[test]
    fn assigned_operation_keeps_large_exact_arrays_off_stack() {
        assert_eq!(
            std::mem::size_of::<AssignedKagemushaStepOperationV4<Fp>>(),
            2 * std::mem::size_of::<usize>()
        );
    }

    #[test]
    fn assigned_loader_enforces_fp_canonicality_in_both_pasta_fields() {
        fn check<F: BigPrimeField>() {
            let (vector, parents, result) = init_fixture();
            let builder = transition_builder::<F>(&vector, 0, &parents, &result, None);
            MockProver::run(builder.config_params.k as u32, &builder, vec![])
                .expect("Kagemusha Step mock prover")
                .assert_satisfied();
        }
        check::<Fp>();
        check::<Fq>();
    }

    #[test]
    fn init_secure_payer_and_operation_tags_are_copy_bound() {
        let (operation, parents, result) = init_fixture();
        let correct = [scalar_limbs(Fp::from(61)), scalar_limbs(Fp::from(62))];
        let builder = transition_builder::<Fp>(&operation, 0, &parents, &result, Some(correct));
        MockProver::run(builder.config_params.k as u32, &builder, vec![])
            .expect("init tag binding prover")
            .assert_satisfied();

        let wrong = [scalar_limbs(Fp::from(63)), scalar_limbs(Fp::from(62))];
        let builder = transition_builder::<Fp>(&operation, 0, &parents, &result, Some(wrong));
        assert!(
            MockProver::run(builder.config_params.k as u32, &builder, vec![])
                .expect("init tag substitution prover")
                .verify()
                .is_err()
        );
    }

    #[test]
    fn lineage_root_set_accepts_anchor_order_opposite_claim_order() {
        // Anchor references sort first by top-up operation id, whereas claims
        // sort first by lineage root. Model the valid case where those orders
        // disagree: the lower operation id carries the higher lineage root.
        let low_lineage_root = exact_limbs([0x11; 32]);
        let high_lineage_root = exact_limbs([0x22; 32]);
        let builder = lineage_root_set_builder(
            [high_lineage_root, low_lineage_root],
            [low_lineage_root, high_lineage_root],
        );
        MockProver::run(builder.config_params.k as u32, &builder, vec![])
            .expect("opposite-order lineage-root set prover")
            .assert_satisfied();

        let unrelated_lineage_root = exact_limbs([0x33; 32]);
        let wrong = lineage_root_set_builder(
            [high_lineage_root, low_lineage_root],
            [low_lineage_root, unrelated_lineage_root],
        );
        assert!(
            MockProver::run(wrong.config_params.k as u32, &wrong, vec![])
                .expect("mismatched lineage-root set prover")
                .verify()
                .is_err()
        );
    }

    #[test]
    fn transition_rejects_root_note_and_parent_count_substitution() {
        let (operation, parents, result) = init_fixture();
        let mut wrong_root = operation.clone();
        wrong_root.limbs[O_RECORD_ROOT_BEFORE * 8] ^= 1;
        let wrong_root_builder = transition_builder::<Fp>(&wrong_root, 0, &parents, &result, None);
        assert!(
            MockProver::run(
                wrong_root_builder.config_params.k as u32,
                &wrong_root_builder,
                vec![],
            )
            .expect("root substitution prover")
            .verify()
            .is_err()
        );

        let mut wrong_note = result.clone();
        wrong_note[S_CURRENT_COMMITMENT] ^= 1;
        let wrong_note_builder =
            transition_builder::<Fp>(&operation, 0, &parents, &wrong_note, None);
        assert!(
            MockProver::run(
                wrong_note_builder.config_params.k as u32,
                &wrong_note_builder,
                vec![]
            )
            .expect("note substitution prover")
            .verify()
            .is_err()
        );

        let wrong_parent_count = transition_builder::<Fp>(&operation, 1, &parents, &result, None);
        assert!(
            MockProver::run(
                wrong_parent_count.config_params.k as u32,
                &wrong_parent_count,
                vec![]
            )
            .expect("parent-count substitution prover")
            .verify()
            .is_err()
        );
    }
}
