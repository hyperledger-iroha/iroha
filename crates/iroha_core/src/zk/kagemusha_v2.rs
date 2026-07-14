//! Branch-safe fractional Kagemusha recursive-spend V2 backend.
//!
//! The V2 circuit deliberately keeps the large recursive IPA verifier slice
//! separate from the compact transition relation. The latter is compiled into
//! three streaming add/multiply instance columns with no long rotations.
//! Consequently, adding an independently
//! spendable recipient or change branch changes neither the circuit shape nor
//! the proof size.

use ff::PrimeField;
use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    halo2curves::pasta::{Fp as Scalar, Fq},
    plonk::{Circuit, ConstraintSystem, Error as PlonkError, Selector},
    poly::Rotation,
};
use iroha_data_model::{
    offline::{
        KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_CLAIMS_V2,
        KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2, KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2,
        KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
        KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2,
        KagemushaRecursiveSpendPublicStatementV2, KagemushaRecursiveSpendTransitionV2,
    },
    proof::VerifyingKeyRecord,
};
use norito::codec::{Decode, Encode};

use super::assign_advice_compat;

#[cfg(test)]
use iroha_data_model::offline::KagemushaRecursiveSpendBranchV2;

/// Public-input schema for the branch-safe V2 transition relation.
///
/// Canonical fields are deterministically compiled into three add/multiply
/// trace columns. The schema is hashed into the verifier record and streamed
/// proving-key package; changing the trace compiler requires a new circuit
/// generation.
pub const KAGEMUSHA_RECURSIVE_SPEND_V2_PUBLIC_INPUTS_SCHEMA: &[u8] = br#"{"schema":"kagemusha_recursive_spend_v2","layout":"streaming_r1cs_three_columns_v1","instance_columns":3,"trace_rows":748,"binds":["canonical_statement_digest","chain_id","asset_definition_id","asset_scale","u128_amounts","parent_bundle_digest","confidential_transfer_v2_public_inputs","recipient_request_digest","operation_ids","branch_selector","branch_path","optional_change","proof_step_count","peer_hop_count","artifact_manifest_sha256","verifier_key_id"]}"#;

/// Version of the fixed transition instance layout.
pub const KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION: u64 = 1;

/// Version of the exact field-neutral state vector carried across the Pasta cycle.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V1: u32 = 1;
/// Number of canonical `u32` limbs in one recursive continuing-state vector.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1: usize = 889;
/// Number of `u32` limbs in one exact 32-byte value.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_DIGEST_LIMBS_V1: usize = 8;
/// Number of `u32` limbs in one exact 192-bit transition-choice tag.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V1: usize =
    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2 / 4;
/// Number of padded transition-tag limbs retained by one branch claim.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_HISTORY_LIMBS_V1: usize =
    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V1
        * KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2 as usize;
/// Number of fixed limbs occupied by one canonical branch claim.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V1: usize =
    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_DIGEST_LIMBS_V1
        + 1
        + 2
        + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_HISTORY_LIMBS_V1;

const S_VERSION: usize = 0;
const S_CHAIN_TAG: usize = S_VERSION + 1;
const S_ASSET_TAG: usize = S_CHAIN_TAG + 8;
const S_ASSET_SCALE: usize = S_ASSET_TAG + 8;
const S_FINAL_ROOT: usize = S_ASSET_SCALE + 1;
const S_TOPUP_ANCHOR_COUNT: usize = S_FINAL_ROOT + 8;
const S_TOPUP_ANCHORS: usize = S_TOPUP_ANCHOR_COUNT + 1;
const S_PROOF_STEP_COUNT: usize = S_TOPUP_ANCHORS + 16 * KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2;
const S_PEER_HOP_COUNT: usize = S_PROOF_STEP_COUNT + 1;
const S_CURRENT_COMMITMENT: usize = S_PEER_HOP_COUNT + 1;
const S_CURRENT_NULLIFIER: usize = S_CURRENT_COMMITMENT + 8;
const S_CURRENT_AMOUNT: usize = S_CURRENT_NULLIFIER + 8;
const S_CURRENT_SCALE: usize = S_CURRENT_AMOUNT + 4;
const S_BRANCH_CLAIM_COUNT: usize = S_CURRENT_SCALE + 1;
const S_BRANCH_CLAIMS: usize = S_BRANCH_CLAIM_COUNT + 1;
const S_ARTIFACT_MANIFEST_SHA256: usize = S_BRANCH_CLAIMS
    + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V1
        * KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_CLAIMS_V2;
const S_VERIFIER_KEY_ID: usize = S_ARTIFACT_MANIFEST_SHA256 + 8;
const S_END: usize = S_VERIFIER_KEY_ID + 8;

const _: [(); KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1] = [(); S_END];

/// Compile-time layout table for the exact recursive continuing-state vector.
///
/// The tuple values are `(field, first_limb, limb_count)`. Variable-count
/// collections always occupy their complete padded allocation; their count
/// limb and zero padding are part of the circuit relation.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_V1: &[(&str, usize, usize)] = &[
    ("layout_version", S_VERSION, 1),
    ("chain_tag", S_CHAIN_TAG, 8),
    ("asset_tag", S_ASSET_TAG, 8),
    ("asset_scale", S_ASSET_SCALE, 1),
    ("final_root", S_FINAL_ROOT, 8),
    ("topup_anchor_count", S_TOPUP_ANCHOR_COUNT, 1),
    (
        "topup_anchors",
        S_TOPUP_ANCHORS,
        16 * KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2,
    ),
    ("proof_step_count", S_PROOF_STEP_COUNT, 1),
    ("peer_hop_count", S_PEER_HOP_COUNT, 1),
    ("current_commitment", S_CURRENT_COMMITMENT, 8),
    ("current_nullifier", S_CURRENT_NULLIFIER, 8),
    ("current_amount", S_CURRENT_AMOUNT, 4),
    ("current_scale", S_CURRENT_SCALE, 1),
    ("branch_claim_count", S_BRANCH_CLAIM_COUNT, 1),
    (
        "branch_claims",
        S_BRANCH_CLAIMS,
        KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V1
            * KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_CLAIMS_V2,
    ),
    ("artifact_manifest_sha256", S_ARTIFACT_MANIFEST_SHA256, 8),
    ("verifier_key_id", S_VERIFIER_KEY_ID, 8),
];

/// Audit map from every continuing statement field to its exact state-vector slot.
///
/// Transition payloads are intentionally absent: they are consumed by the Eq
/// application relation and summarized by the branch history that continues
/// into the next state. Artifact generation is authenticated by the manifest
/// hash, and verifier-key text is authenticated by its canonical identity.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_COVERAGE_V1: &[(&str, &str)] = &[
    ("statement.chain_id", "chain_tag"),
    ("statement.asset", "asset_tag"),
    ("statement.asset_scale", "asset_scale"),
    ("statement.final_root", "final_root"),
    ("statement.topup_anchor_refs", "topup_anchors"),
    ("statement.proof_step_count", "proof_step_count"),
    ("statement.peer_hop_count", "peer_hop_count"),
    (
        "statement.current_note.note_commitment",
        "current_commitment",
    ),
    (
        "statement.current_note.spend_nullifier",
        "current_nullifier",
    ),
    (
        "statement.current_note.amount.atomic_units",
        "current_amount",
    ),
    ("statement.current_note.amount.scale", "current_scale"),
    ("statement.branch_claims", "branch_claims"),
    (
        "statement.artifact_binding.manifest_sha256",
        "artifact_manifest_sha256",
    ),
    ("statement.verifier_key_id", "verifier_key_id"),
];

/// Exact field-neutral recursive state represented only by canonical `u32` limbs.
///
/// No limb is reduced modulo either Pasta field. Both recursive circuits range
/// constrain every public limb to 32 bits and copy the vector limb-for-limb.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaRecursiveSpendStateVectorV1 {
    /// Fixed continuing-state limbs in [`KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_V1`] order.
    pub limbs: [u32; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1],
}

/// Public operation selected by one logical recursive transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaRecursiveSpendOperationKindV1 {
    /// First note created from one independently finalized top-up.
    Init,
    /// Offline peer split extending one or two prior states.
    Append,
    /// Partial redemption that retains a confidential change state.
    RedemptionChange,
}

// The transition column is intentionally explicit.  Keeping named offsets
// makes envelope validation fail closed when fields are added or reordered.
const I_LAYOUT_VERSION: usize = 0;
const I_APPEND_PROFILE: usize = I_LAYOUT_VERSION + 1;
const I_BRANCH_CHANGE: usize = I_APPEND_PROFILE + 1;
const I_HAS_CHANGE: usize = I_BRANCH_CHANGE + 1;
const I_RECORD_OUTPUT_SWAP: usize = I_HAS_CHANGE + 1;
const I_TRANSFER_OUTPUT_SWAP: usize = I_RECORD_OUTPUT_SWAP + 1;
const I_PROOF_STEP_COUNT: usize = I_TRANSFER_OUTPUT_SWAP + 1;
const I_PEER_HOP_COUNT: usize = I_PROOF_STEP_COUNT + 1;
const I_PREVIOUS_PROOF_STEP_COUNT: usize = I_PEER_HOP_COUNT + 1;
const I_PREVIOUS_PEER_HOP_COUNT: usize = I_PREVIOUS_PROOF_STEP_COUNT + 1;
const I_BRANCH_DEPTH: usize = I_PREVIOUS_PEER_HOP_COUNT + 1;
const I_PARENT_BRANCH_DEPTH: usize = I_BRANCH_DEPTH + 1;
const I_ASSET_SCALE: usize = I_PARENT_BRANCH_DEPTH + 1;
const I_INPUT_SCALE: usize = I_ASSET_SCALE + 1;
const I_TRANSFER_SCALE: usize = I_INPUT_SCALE + 1;
const I_RECIPIENT_SCALE: usize = I_TRANSFER_SCALE + 1;
const I_CHANGE_SCALE: usize = I_RECIPIENT_SCALE + 1;
const I_CURRENT_SCALE: usize = I_CHANGE_SCALE + 1;
const I_RECORD_INPUT_COUNT: usize = I_CURRENT_SCALE + 1;
const I_RECORD_OUTPUT_COUNT: usize = I_RECORD_INPUT_COUNT + 1;
const I_TRANSFER_INPUT_COUNT: usize = I_RECORD_OUTPUT_COUNT + 1;
const I_TRANSFER_OUTPUT_COUNT: usize = I_TRANSFER_INPUT_COUNT + 1;
const I_CURRENT_AMOUNT_LO: usize = I_TRANSFER_OUTPUT_COUNT + 1;
const I_CURRENT_AMOUNT_HI: usize = I_CURRENT_AMOUNT_LO + 1;
const I_INPUT_AMOUNT_LO: usize = I_CURRENT_AMOUNT_HI + 1;
const I_INPUT_AMOUNT_HI: usize = I_INPUT_AMOUNT_LO + 1;
const I_TRANSFER_AMOUNT_LO: usize = I_INPUT_AMOUNT_HI + 1;
const I_TRANSFER_AMOUNT_HI: usize = I_TRANSFER_AMOUNT_LO + 1;
const I_RECIPIENT_AMOUNT_LO: usize = I_TRANSFER_AMOUNT_HI + 1;
const I_RECIPIENT_AMOUNT_HI: usize = I_RECIPIENT_AMOUNT_LO + 1;
const I_CHANGE_AMOUNT_LO: usize = I_RECIPIENT_AMOUNT_HI + 1;
const I_CHANGE_AMOUNT_HI: usize = I_CHANGE_AMOUNT_LO + 1;
const I_BRANCH_PATH_BITS: usize = I_CHANGE_AMOUNT_HI + 1;
const I_PARENT_BRANCH_PATH_BITS: usize = I_BRANCH_PATH_BITS + 1;
const I_INITIAL_ROOT: usize = I_PARENT_BRANCH_PATH_BITS + 1;
const I_FINAL_ROOT: usize = I_INITIAL_ROOT + 1;
const I_RECORD_ROOT_BEFORE: usize = I_FINAL_ROOT + 1;
const I_RECORD_ROOT_AFTER: usize = I_RECORD_ROOT_BEFORE + 1;
const I_TRANSFER_ROOT: usize = I_RECORD_ROOT_AFTER + 1;
const I_CURRENT_COMMITMENT: usize = I_TRANSFER_ROOT + 1;
const I_CURRENT_NULLIFIER: usize = I_CURRENT_COMMITMENT + 1;
const I_INPUT_COMMITMENT: usize = I_CURRENT_NULLIFIER + 1;
const I_INPUT_NULLIFIER: usize = I_INPUT_COMMITMENT + 1;
const I_RECIPIENT_COMMITMENT: usize = I_INPUT_NULLIFIER + 1;
const I_RECIPIENT_NULLIFIER: usize = I_RECIPIENT_COMMITMENT + 1;
const I_CHANGE_COMMITMENT: usize = I_RECIPIENT_NULLIFIER + 1;
const I_CHANGE_NULLIFIER: usize = I_CHANGE_COMMITMENT + 1;
const I_RECORD_INPUT_NULLIFIER_0: usize = I_CHANGE_NULLIFIER + 1;
const I_RECORD_INPUT_NULLIFIER_1: usize = I_RECORD_INPUT_NULLIFIER_0 + 1;
const I_RECORD_OUTPUT_0: usize = I_RECORD_INPUT_NULLIFIER_1 + 1;
const I_RECORD_OUTPUT_1: usize = I_RECORD_OUTPUT_0 + 1;
const I_TRANSFER_INPUT_COMMITMENT_0: usize = I_RECORD_OUTPUT_1 + 1;
const I_TRANSFER_INPUT_COMMITMENT_1: usize = I_TRANSFER_INPUT_COMMITMENT_0 + 1;
const I_TRANSFER_NULLIFIER_0: usize = I_TRANSFER_INPUT_COMMITMENT_1 + 1;
const I_TRANSFER_NULLIFIER_1: usize = I_TRANSFER_NULLIFIER_0 + 1;
const I_TRANSFER_OUTPUT_0: usize = I_TRANSFER_NULLIFIER_1 + 1;
const I_TRANSFER_OUTPUT_1: usize = I_TRANSFER_OUTPUT_0 + 1;
const I_ASSET_TAG: usize = I_TRANSFER_OUTPUT_1 + 1;
const I_CHAIN_TAG: usize = I_ASSET_TAG + 1;
const I_STATEMENT_DIGEST: usize = I_CHAIN_TAG + 1;
const I_SPLIT_DIGEST: usize = I_STATEMENT_DIGEST + 4;
const I_RECIPIENT_REQUEST_DIGEST: usize = I_SPLIT_DIGEST + 4;
const I_OPERATION_ID: usize = I_RECIPIENT_REQUEST_DIGEST + 4;
const I_PARENT_BUNDLE_DIGEST: usize = I_OPERATION_ID + 4;
const I_BRANCH_LINEAGE_ROOT: usize = I_PARENT_BUNDLE_DIGEST + 4;
const I_PARENT_BRANCH_LINEAGE_ROOT: usize = I_BRANCH_LINEAGE_ROOT + 4;
const I_CHAIN_ID_DIGEST: usize = I_PARENT_BRANCH_LINEAGE_ROOT + 4;
const I_ASSET_ID_DIGEST: usize = I_CHAIN_ID_DIGEST + 4;
const I_TOPUP_OPERATION_ID: usize = I_ASSET_ID_DIGEST + 4;
const I_ARTIFACT_MANIFEST_SHA256: usize = I_TOPUP_OPERATION_ID + 4;
const I_CURRENT_HOP_DOMAIN_TAG: usize = I_ARTIFACT_MANIFEST_SHA256 + 4;
const I_TOPUP_RECEIPT_DIGEST: usize = I_CURRENT_HOP_DOMAIN_TAG + 4;
const I_PARENT_TOPUP_RECEIPT_DIGEST: usize = I_TOPUP_RECEIPT_DIGEST + 4;
const I_TOPUP_ANCHOR_DIGEST: usize = I_PARENT_TOPUP_RECEIPT_DIGEST + 4;
const I_TOPUP_ANCHOR_COUNT: usize = I_TOPUP_ANCHOR_DIGEST + 4;
const I_VERIFIER_KEY_ID_DIGEST: usize = I_TOPUP_ANCHOR_COUNT + 1;
const I_REDEMPTION_PROFILE: usize = I_VERIFIER_KEY_ID_DIGEST + 4;
const I_PARENT_FINAL_ROOT: usize = I_REDEMPTION_PROFILE + 1;
const I_REDEMPTION_RECIPIENT_DIGEST: usize = I_PARENT_FINAL_ROOT + 1;
const I_UNSHIELD_PUBLIC_INPUTS_DIGEST: usize = I_REDEMPTION_RECIPIENT_DIGEST + 4;
const I_UNSHIELD_PUBLIC_AMOUNT: usize = I_UNSHIELD_PUBLIC_INPUTS_DIGEST + 4;

/// Number of canonical public values from which the V2 transition trace is compiled.
pub const KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS: usize = I_UNSHIELD_PUBLIC_AMOUNT + 1;
/// Number of instance columns in the compiled streaming transition trace.
pub const KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS: usize = 3;
/// Exact row count of each of the three compiled streaming trace columns.
///
/// This is part of the authenticated verifier shape. Any relation change that
/// alters the count requires a new circuit generation and schema hash.
pub const KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_TRACE_ROWS: usize = 748;
/// Total field elements supplied to the transition verifier as public instances.
pub const KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_CELLS: usize =
    KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS
        * KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_TRACE_ROWS;

const PATH_SELECTOR_COUNT: usize = 64;
const PEER_HOP_SELECTOR_COUNT: usize = KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2 as usize + 1;

/// Canonical values and deterministic auxiliary trace values for one V2 output branch.
#[derive(Clone, Debug)]
pub struct KagemushaRecursiveSpendTransitionValuesV2<F: PrimeField = Scalar> {
    /// Consecutive public rows described by
    /// [`KAGEMUSHA_RECURSIVE_SPEND_V2_PUBLIC_INPUTS_SCHEMA`].
    pub public: [F; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS],
    /// Recomputed carry from the low 64-bit limb of recipient + change.
    ///
    /// This is expanded into the verifier-supplied trace; it is not accepted
    /// as an unauthenticated private witness.
    amount_low_carry: F,
    /// Recomputed one-hot selector for the parent branch depth on append.
    path_depth_selectors: [F; PATH_SELECTOR_COUNT],
    /// Recomputed one-hot selector constraining the current peer-hop count to `0..=8`.
    peer_hop_selectors: [F; PEER_HOP_SELECTOR_COUNT],
}

impl<F: PrimeField> Default for KagemushaRecursiveSpendTransitionValuesV2<F> {
    fn default() -> Self {
        let mut public = [F::ZERO; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS];
        public[I_LAYOUT_VERSION] = F::from(KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION);
        // Keygen uses this witnessless, internally consistent init shape.
        public[I_PROOF_STEP_COUNT] = F::ONE;
        let mut peer_hop_selectors = [F::ZERO; PEER_HOP_SELECTOR_COUNT];
        peer_hop_selectors[0] = F::ONE;
        Self {
            public,
            amount_low_carry: F::ZERO,
            path_depth_selectors: [F::ZERO; PATH_SELECTOR_COUNT],
            peer_hop_selectors,
        }
    }
}

/// Constraint-system columns for the V2 transition relation.
#[derive(Clone, Copy)]
pub struct KagemushaRecursiveSpendTransitionConfigV2 {
    advice: [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>;
        KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS],
    add: Selector,
    mul: Selector,
}

/// Exact branch-safe split circuit shared by init and append compositions.
#[derive(Clone, Debug, Default)]
pub struct KagemushaRecursiveSpendTransitionCircuitV2<F: PrimeField = Scalar> {
    /// Fixed transition witness and public values.
    pub values: KagemushaRecursiveSpendTransitionValuesV2<F>,
}

/// Eq/Fp implementation of only the symmetric application transition.
///
/// This is deliberately not named as a recursive step circuit: a key generated
/// for this transition-only relation must never satisfy a V3 StepEq artifact
/// role or terminal recursive verifier.
pub type KagemushaRecursiveSpendTransitionEqCircuitV2 =
    KagemushaRecursiveSpendTransitionCircuitV2<Scalar>;
/// Ep/Fq implementation of only the identical symmetric application transition.
pub type KagemushaRecursiveSpendTransitionEpCircuitV2 =
    KagemushaRecursiveSpendTransitionCircuitV2<Fq>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransitionTraceRowKind {
    Add,
    Mul,
}

#[derive(Clone, Copy, Debug)]
struct TransitionTraceRow<F> {
    kind: TransitionTraceRowKind,
    values: [F; 3],
}

#[derive(Clone, Debug, Default)]
struct TransitionTrace<F> {
    rows: Vec<TransitionTraceRow<F>>,
}

impl<F: PrimeField> TransitionTrace<F> {
    fn push_add(&mut self, lhs: F, rhs: F, result: F) {
        self.rows.push(TransitionTraceRow {
            kind: TransitionTraceRowKind::Add,
            values: [lhs, rhs, result],
        });
    }

    fn push_mul(&mut self, lhs: F, rhs: F, result: F) {
        self.rows.push(TransitionTraceRow {
            kind: TransitionTraceRowKind::Mul,
            values: [lhs, rhs, result],
        });
    }

    fn bind(&mut self, value: F) {
        self.push_add(value, F::ZERO, value);
    }

    fn add(&mut self, lhs: F, rhs: F) -> F {
        let result = lhs + rhs;
        self.push_add(lhs, rhs, result);
        result
    }

    fn sub(&mut self, lhs: F, rhs: F) -> F {
        let result = lhs - rhs;
        self.push_add(result, rhs, lhs);
        result
    }

    fn mul(&mut self, lhs: F, rhs: F) -> F {
        let result = lhs * rhs;
        self.push_mul(lhs, rhs, result);
        result
    }

    fn assert_equal(&mut self, lhs: F, rhs: F) {
        self.push_add(lhs, F::ZERO, rhs);
    }

    fn assert_boolean(&mut self, value: F) {
        self.push_mul(value, value, value);
    }

    fn assert_mul_zero(&mut self, lhs: F, rhs: F) {
        self.push_mul(lhs, rhs, F::ZERO);
    }

    fn assert_gated_equal(&mut self, gate: F, lhs: F, rhs: F) {
        let difference = self.sub(lhs, rhs);
        self.assert_mul_zero(gate, difference);
    }

    fn select(&mut self, first: F, second: F, selector: F) -> F {
        let difference = self.sub(second, first);
        let selected_difference = self.mul(selector, difference);
        self.add(first, selected_difference)
    }

    fn sum(&mut self, values: impl IntoIterator<Item = F>) -> F {
        values
            .into_iter()
            .fold(F::ZERO, |sum, value| self.add(sum, value))
    }

    fn instance_columns(&self) -> Vec<Vec<F>> {
        (0..KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS)
            .map(|column| self.rows.iter().map(|row| row.values[column]).collect())
            .collect()
    }
}

fn kagemusha_recursive_spend_transition_trace_v2<F: PrimeField>(
    values: &KagemushaRecursiveSpendTransitionValuesV2<F>,
) -> TransitionTrace<F> {
    let mut trace = TransitionTrace::default();
    for value in values.public {
        trace.bind(value);
    }

    let p = |index: usize| values.public[index];
    let one = F::ONE;
    let two_pow_64 = F::from_u128(1_u128 << 64);
    let append = p(I_APPEND_PROFILE);
    let redemption = p(I_REDEMPTION_PROFILE);
    let extends = trace.add(append, redemption);
    let branch = p(I_BRANCH_CHANGE);
    let has_change = p(I_HAS_CHANGE);
    let record_swap = p(I_RECORD_OUTPUT_SWAP);
    let transfer_swap = p(I_TRANSFER_OUTPUT_SWAP);
    let carry = values.amount_low_carry;
    let not_has_change = trace.sub(one, has_change);
    let not_extends = trace.sub(one, extends);
    let not_redemption = trace.sub(one, redemption);

    trace.assert_equal(
        p(I_LAYOUT_VERSION),
        F::from(KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION),
    );
    for boolean in [
        append,
        redemption,
        branch,
        has_change,
        record_swap,
        transfer_swap,
        carry,
    ] {
        trace.assert_boolean(boolean);
    }
    trace.assert_mul_zero(branch, not_has_change);
    trace.assert_mul_zero(append, redemption);
    let not_branch = trace.sub(one, branch);
    trace.assert_mul_zero(redemption, not_branch);
    trace.assert_mul_zero(redemption, not_has_change);
    trace.assert_mul_zero(redemption, record_swap);
    trace.assert_mul_zero(redemption, transfer_swap);
    let record_without_change = trace.mul(record_swap, not_has_change);
    trace.assert_mul_zero(record_without_change, append);
    let transfer_without_change = trace.mul(transfer_swap, not_has_change);
    trace.assert_mul_zero(transfer_without_change, append);

    let next_proof_step = trace.add(p(I_PREVIOUS_PROOF_STEP_COUNT), one);
    trace.assert_equal(p(I_PROOF_STEP_COUNT), next_proof_step);
    let next_peer_hop = trace.add(p(I_PREVIOUS_PEER_HOP_COUNT), append);
    trace.assert_equal(p(I_PEER_HOP_COUNT), next_peer_hop);
    let next_branch_depth = trace.add(p(I_PARENT_BRANCH_DEPTH), extends);
    trace.assert_equal(p(I_BRANCH_DEPTH), next_branch_depth);
    trace.assert_mul_zero(not_extends, p(I_PREVIOUS_PROOF_STEP_COUNT));
    trace.assert_mul_zero(not_extends, p(I_PREVIOUS_PEER_HOP_COUNT));
    trace.assert_mul_zero(not_extends, p(I_PARENT_BRANCH_DEPTH));

    for selector in values.peer_hop_selectors {
        trace.assert_boolean(selector);
    }
    let peer_hop_selector_sum = trace.sum(values.peer_hop_selectors);
    trace.assert_equal(peer_hop_selector_sum, one);
    let selected_peer_hops = values
        .peer_hop_selectors
        .into_iter()
        .enumerate()
        .map(|(hop, selector)| {
            trace.mul(
                selector,
                F::from(u64::try_from(hop).expect("peer hop fits u64")),
            )
        })
        .collect::<Vec<_>>();
    let selected_peer_hop = trace.sum(selected_peer_hops);
    trace.assert_equal(selected_peer_hop, p(I_PEER_HOP_COUNT));

    for index in [
        I_CURRENT_SCALE,
        I_INPUT_SCALE,
        I_TRANSFER_SCALE,
        I_RECIPIENT_SCALE,
    ] {
        trace.assert_equal(p(index), p(I_ASSET_SCALE));
    }
    let expected_change_scale = trace.mul(has_change, p(I_ASSET_SCALE));
    trace.assert_equal(p(I_CHANGE_SCALE), expected_change_scale);
    trace.assert_equal(p(I_TRANSFER_AMOUNT_LO), p(I_RECIPIENT_AMOUNT_LO));
    trace.assert_equal(p(I_TRANSFER_AMOUNT_HI), p(I_RECIPIENT_AMOUNT_HI));
    let recipient_high_scaled = trace.mul(p(I_RECIPIENT_AMOUNT_HI), two_pow_64);
    let recipient_amount = trace.add(p(I_RECIPIENT_AMOUNT_LO), recipient_high_scaled);
    trace.assert_gated_equal(redemption, p(I_UNSHIELD_PUBLIC_AMOUNT), recipient_amount);
    let recipient_change_low = trace.add(p(I_RECIPIENT_AMOUNT_LO), p(I_CHANGE_AMOUNT_LO));
    let carry_scaled = trace.mul(carry, two_pow_64);
    let input_low_with_carry = trace.add(p(I_INPUT_AMOUNT_LO), carry_scaled);
    trace.assert_equal(recipient_change_low, input_low_with_carry);
    let recipient_change_high = trace.add(p(I_RECIPIENT_AMOUNT_HI), p(I_CHANGE_AMOUNT_HI));
    let recipient_change_high = trace.add(recipient_change_high, carry);
    trace.assert_equal(recipient_change_high, p(I_INPUT_AMOUNT_HI));
    let selected_amount_low = trace.select(p(I_RECIPIENT_AMOUNT_LO), p(I_CHANGE_AMOUNT_LO), branch);
    trace.assert_equal(p(I_CURRENT_AMOUNT_LO), selected_amount_low);
    let selected_amount_high =
        trace.select(p(I_RECIPIENT_AMOUNT_HI), p(I_CHANGE_AMOUNT_HI), branch);
    trace.assert_equal(p(I_CURRENT_AMOUNT_HI), selected_amount_high);
    for index in [
        I_CHANGE_AMOUNT_LO,
        I_CHANGE_AMOUNT_HI,
        I_CHANGE_COMMITMENT,
        I_CHANGE_NULLIFIER,
    ] {
        trace.assert_mul_zero(not_has_change, p(index));
    }

    let selected_commitment =
        trace.select(p(I_RECIPIENT_COMMITMENT), p(I_CHANGE_COMMITMENT), branch);
    trace.assert_equal(p(I_CURRENT_COMMITMENT), selected_commitment);
    let selected_nullifier = trace.select(p(I_RECIPIENT_NULLIFIER), p(I_CHANGE_NULLIFIER), branch);
    trace.assert_equal(p(I_CURRENT_NULLIFIER), selected_nullifier);

    let expected_record_root_before =
        trace.select(p(I_INITIAL_ROOT), p(I_PARENT_FINAL_ROOT), extends);
    trace.assert_equal(p(I_RECORD_ROOT_BEFORE), expected_record_root_before);
    trace.assert_equal(p(I_RECORD_ROOT_AFTER), p(I_FINAL_ROOT));
    let expected_transfer_root = trace.select(p(I_FINAL_ROOT), p(I_PARENT_FINAL_ROOT), redemption);
    trace.assert_equal(p(I_TRANSFER_ROOT), expected_transfer_root);
    trace.assert_equal(p(I_RECORD_INPUT_COUNT), p(I_TRANSFER_INPUT_COUNT));
    trace.assert_equal(p(I_RECORD_OUTPUT_COUNT), p(I_TRANSFER_OUTPUT_COUNT));

    for index in [I_RECORD_INPUT_COUNT, I_TRANSFER_INPUT_COUNT] {
        trace.assert_gated_equal(extends, p(index), one);
    }
    trace.assert_gated_equal(extends, p(I_RECORD_INPUT_NULLIFIER_0), p(I_INPUT_NULLIFIER));
    trace.assert_mul_zero(extends, p(I_RECORD_INPUT_NULLIFIER_1));
    trace.assert_gated_equal(
        extends,
        p(I_TRANSFER_INPUT_COMMITMENT_0),
        p(I_INPUT_COMMITMENT),
    );
    trace.assert_mul_zero(extends, p(I_TRANSFER_INPUT_COMMITMENT_1));
    trace.assert_gated_equal(extends, p(I_TRANSFER_NULLIFIER_0), p(I_INPUT_NULLIFIER));
    trace.assert_mul_zero(extends, p(I_TRANSFER_NULLIFIER_1));

    let expected_record_0 = trace.select(
        p(I_RECIPIENT_COMMITMENT),
        p(I_CHANGE_COMMITMENT),
        record_swap,
    );
    let expected_record_1 = trace.select(
        p(I_CHANGE_COMMITMENT),
        p(I_RECIPIENT_COMMITMENT),
        record_swap,
    );
    let expected_transfer_0 = trace.select(
        p(I_RECIPIENT_COMMITMENT),
        p(I_CHANGE_COMMITMENT),
        transfer_swap,
    );
    let expected_transfer_1 = trace.select(
        p(I_CHANGE_COMMITMENT),
        p(I_RECIPIENT_COMMITMENT),
        transfer_swap,
    );
    trace.assert_gated_equal(append, p(I_RECORD_OUTPUT_0), expected_record_0);
    trace.assert_gated_equal(append, p(I_RECORD_OUTPUT_1), expected_record_1);
    trace.assert_gated_equal(append, p(I_TRANSFER_OUTPUT_0), expected_transfer_0);
    trace.assert_gated_equal(append, p(I_TRANSFER_OUTPUT_1), expected_transfer_1);
    let expected_output_count = trace.add(one, has_change);
    trace.assert_gated_equal(append, p(I_RECORD_OUTPUT_COUNT), expected_output_count);
    trace.assert_gated_equal(append, p(I_TRANSFER_OUTPUT_COUNT), expected_output_count);
    for index in [I_RECORD_OUTPUT_COUNT, I_TRANSFER_OUTPUT_COUNT] {
        trace.assert_gated_equal(redemption, p(index), one);
    }
    trace.assert_gated_equal(redemption, p(I_RECORD_OUTPUT_0), p(I_CHANGE_COMMITMENT));
    trace.assert_mul_zero(redemption, p(I_RECORD_OUTPUT_1));
    trace.assert_gated_equal(redemption, p(I_TRANSFER_OUTPUT_0), p(I_CHANGE_COMMITMENT));
    for index in [
        I_TRANSFER_OUTPUT_1,
        I_RECIPIENT_COMMITMENT,
        I_RECIPIENT_NULLIFIER,
    ] {
        trace.assert_mul_zero(redemption, p(index));
    }

    let record_selected = trace.select(p(I_RECORD_OUTPUT_0), p(I_RECORD_OUTPUT_1), record_swap);
    let transfer_selected = trace.select(
        p(I_TRANSFER_OUTPUT_0),
        p(I_TRANSFER_OUTPUT_1),
        transfer_swap,
    );
    let record_other = trace.select(p(I_RECORD_OUTPUT_1), p(I_RECORD_OUTPUT_0), record_swap);
    let transfer_other = trace.select(
        p(I_TRANSFER_OUTPUT_1),
        p(I_TRANSFER_OUTPUT_0),
        transfer_swap,
    );
    trace.assert_gated_equal(not_extends, record_selected, p(I_CURRENT_COMMITMENT));
    trace.assert_gated_equal(not_extends, transfer_selected, p(I_CURRENT_COMMITMENT));
    trace.assert_gated_equal(not_extends, record_other, transfer_other);
    trace.assert_gated_equal(
        not_extends,
        p(I_RECORD_OUTPUT_COUNT),
        p(I_TRANSFER_OUTPUT_COUNT),
    );

    for selector in values.path_depth_selectors {
        trace.assert_boolean(selector);
    }
    let path_selector_sum = trace.sum(values.path_depth_selectors);
    trace.assert_equal(path_selector_sum, extends);
    let selected_depths = values
        .path_depth_selectors
        .into_iter()
        .enumerate()
        .map(|(depth, selector)| {
            trace.mul(
                selector,
                F::from(u64::try_from(depth).expect("path depth fits u64")),
            )
        })
        .collect::<Vec<_>>();
    let selected_depth = trace.sum(selected_depths);
    trace.assert_equal(selected_depth, p(I_PARENT_BRANCH_DEPTH));
    let selected_masks = values
        .path_depth_selectors
        .into_iter()
        .enumerate()
        .map(|(depth, selector)| trace.mul(selector, F::from(1_u64 << (63 - depth))))
        .collect::<Vec<_>>();
    let selected_mask = trace.sum(selected_masks);
    let branch_mask = trace.mul(branch, selected_mask);
    let expected_branch_path = trace.add(p(I_PARENT_BRANCH_PATH_BITS), branch_mask);
    trace.assert_equal(p(I_BRANCH_PATH_BITS), expected_branch_path);

    for limb in 0..4 {
        trace.assert_gated_equal(
            extends,
            p(I_BRANCH_LINEAGE_ROOT + limb),
            p(I_PARENT_BRANCH_LINEAGE_ROOT + limb),
        );
        trace.assert_mul_zero(not_extends, p(I_PARENT_BRANCH_LINEAGE_ROOT + limb));
        trace.assert_gated_equal(
            extends,
            p(I_TOPUP_RECEIPT_DIGEST + limb),
            p(I_PARENT_TOPUP_RECEIPT_DIGEST + limb),
        );
        trace.assert_mul_zero(not_extends, p(I_PARENT_TOPUP_RECEIPT_DIGEST + limb));
        trace.assert_mul_zero(not_redemption, p(I_REDEMPTION_RECIPIENT_DIGEST + limb));
        trace.assert_mul_zero(not_redemption, p(I_UNSHIELD_PUBLIC_INPUTS_DIGEST + limb));
    }
    trace.assert_mul_zero(not_extends, p(I_PARENT_FINAL_ROOT));
    trace.assert_mul_zero(not_redemption, p(I_UNSHIELD_PUBLIC_AMOUNT));

    assert_eq!(
        trace.rows.len(),
        KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_TRACE_ROWS,
        "the streaming transition trace shape is release-authenticated"
    );
    trace
}

impl<F: PrimeField> Circuit<F> for KagemushaRecursiveSpendTransitionCircuitV2<F> {
    type Config = KagemushaRecursiveSpendTransitionConfigV2;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        meta.set_minimum_degree(3);
        let advice = std::array::from_fn(|_| meta.advice_column());
        let instance: [halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>;
            KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS] =
            std::array::from_fn(|_| meta.instance_column());
        let add = meta.selector();
        let mul = meta.selector();

        meta.create_gate("kagemusha_recursive_spend_v2_stream_add", |meta| {
            let enabled = meta.query_selector(add);
            let advice = advice.map(|column| meta.query_advice(column, Rotation::cur()));
            let instance = instance.map(|column| meta.query_instance(column, Rotation::cur()));
            vec![
                enabled.clone() * (advice[0].clone() - instance[0].clone()),
                enabled.clone() * (advice[1].clone() - instance[1].clone()),
                enabled.clone() * (advice[2].clone() - instance[2].clone()),
                enabled * (advice[0].clone() + advice[1].clone() - advice[2].clone()),
            ]
        });
        meta.create_gate("kagemusha_recursive_spend_v2_stream_mul", |meta| {
            let enabled = meta.query_selector(mul);
            let advice = advice.map(|column| meta.query_advice(column, Rotation::cur()));
            let instance = instance.map(|column| meta.query_instance(column, Rotation::cur()));
            vec![
                enabled.clone() * (advice[0].clone() - instance[0].clone()),
                enabled.clone() * (advice[1].clone() - instance[1].clone()),
                enabled.clone() * (advice[2].clone() - instance[2].clone()),
                enabled * (advice[0].clone() * advice[1].clone() - advice[2].clone()),
            ]
        });

        KagemushaRecursiveSpendTransitionConfigV2 { advice, add, mul }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        let trace = kagemusha_recursive_spend_transition_trace_v2(&self.values);
        layouter.assign_region(
            || "kagemusha_recursive_spend_v2_streaming_transition",
            |mut region| {
                for (row, trace_row) in trace.rows.iter().enumerate() {
                    match trace_row.kind {
                        TransitionTraceRowKind::Add => config.add.enable(&mut region, row)?,
                        TransitionTraceRowKind::Mul => config.mul.enable(&mut region, row)?,
                    }
                    for (column, value) in trace_row.values.iter().copied().enumerate() {
                        assign_advice_compat(
                            &mut region,
                            move || format!("v2_stream_{row}_{column}"),
                            config.advice[column],
                            row,
                            || Value::known(value),
                        )?;
                    }
                }
                Ok(())
            },
        )
    }
}

/// Public-input contract for the secure Kagemusha output-membership relation.
///
/// Paths remain witness values, but every path direction is constrained to the
/// corresponding public leaf index and every recomputed path root is constrained
/// to these public roots and commitments.  The relation uses the same Axiom
/// Poseidon specification and leaf/node domains as confidential transfer V3.
pub const KAGEMUSHA_OUTPUT_MEMBERSHIP_PUBLIC_INPUTS_SCHEMA_V3: &[u8] = br#"{"schema":"kagemusha_output_membership_v3","hash":"axiom_poseidon_t3_r2_rf8_rp57_mds0","merkle_leaf_domain":"cfleaf03","merkle_node_domain":"cfnode03","public_inputs":["is_init","is_split","is_redemption_change","has_change","initial_root","final_root","recipient_commitment","recipient_leaf_index","change_commitment","change_leaf_index","dummy_leaf_index"]}"#;

/// IPA domain exponent used by the fixed output-membership relation.
pub const KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V3: u32 = 12;
/// Number of one-row instance columns exposed by the output-membership relation.
pub const KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V3: usize = 11;

/// Operation profile selected by one output-membership proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaOutputMembershipOperationV3 {
    /// Initial note inserted by a finalized top-up.
    Init,
    /// Offline split with one recipient output and optional sender change.
    Split,
    /// Partial redemption with one confidential change output.
    RedemptionChange,
}

/// One proof-bound output and both paths needed to authenticate its creation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaOutputMembershipLeafV3 {
    /// Exact confidential note commitment inserted into the tree.
    pub commitment: [u8; 32],
    /// Exact leaf position, encoded again by both path direction vectors.
    pub leaf_index: u32,
    /// Empty-leaf path before this output is inserted.
    pub update_path: iroha_data_model::offline::KagemushaConfidentialMerklePathV2,
    /// Commitment path after every output in the operation has been inserted.
    pub membership_path: iroha_data_model::offline::KagemushaConfidentialMerklePathV2,
}

/// Complete witness for one fixed-shape Poseidon output-membership update.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaOutputMembershipWitnessV3 {
    /// Operation whose exact output-presence rules are enforced in-circuit.
    pub operation: KagemushaOutputMembershipOperationV3,
    /// Commitment-tree root before inserting this operation's outputs.
    pub initial_root: [u8; 32],
    /// Commitment-tree root after inserting every present output.
    pub final_root: [u8; 32],
    /// Recipient output. Required by init/split and absent for redemption change.
    pub recipient: Option<KagemushaOutputMembershipLeafV3>,
    /// Sender change. Optional for split, required for redemption change, absent for init.
    pub change: Option<KagemushaOutputMembershipLeafV3>,
    /// Distinct empty leaf retained for the fixed two-input confidential relation.
    pub dummy_leaf_index: u32,
    /// Empty-leaf membership path under `final_root` for `dummy_leaf_index`.
    pub dummy_path: iroha_data_model::offline::KagemushaConfidentialMerklePathV2,
}

/// Eq/Fp secure Poseidon relation proving output insertion and final membership.
///
/// This circuit is intentionally separate from the symmetric transition-only
/// relation: confidential-tree roots are Pallas scalar-field elements and must
/// not be re-hashed natively in the reciprocal Fq step.  A production StepEq
/// composition must consume this relation together with the field-neutral
/// state boundary; the backend availability gate remains fail-closed independently.
#[derive(Clone, Debug, Default)]
pub struct KagemushaOutputMembershipCircuitV3 {
    witness: Option<KagemushaOutputMembershipWitnessV3>,
}

impl KagemushaOutputMembershipCircuitV3 {
    /// Construct the circuit after checking fixed path sizes and scalar encodings.
    pub fn new(witness: KagemushaOutputMembershipWitnessV3) -> Result<Self, String> {
        output_membership_v3::validate_witness_encoding(&witness)?;
        Ok(Self {
            witness: Some(witness),
        })
    }

    /// Return the exact public columns expected by `MockProver` or IPA proving.
    pub fn public_instances(&self) -> Result<Vec<Vec<Scalar>>, String> {
        let witness = self
            .witness
            .as_ref()
            .ok_or_else(|| "Kagemusha output-membership witness is absent".to_owned())?;
        output_membership_v3::public_instances(witness)
    }
}

mod output_membership_v3 {
    use ff::Field as _;
    use halo2_base::{
        AssignedValue, Context,
        gates::{
            GateInstructions, RangeInstructions,
            circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
        },
    };
    use halo2_proofs::{
        circuit::Layouter,
        plonk::{Circuit, ConstraintSystem, Error as PlonkError},
    };

    use super::{
        KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V3, KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V3,
        KagemushaOutputMembershipCircuitV3, KagemushaOutputMembershipLeafV3,
        KagemushaOutputMembershipOperationV3, KagemushaOutputMembershipWitnessV3, Scalar,
    };
    use crate::zk::confidential_v2::{
        CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3, CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
        confidential_relation_gadget::ConfidentialPoseidonChipV3, scalar_from_repr,
    };

    const TREE_DEPTH: usize = iroha_data_model::offline::KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2;
    const MINIMUM_UNUSABLE_ROWS: usize = 9;

    fn path_values(
        path: Option<&iroha_data_model::offline::KagemushaConfidentialMerklePathV2>,
    ) -> (Vec<[u8; 32]>, Vec<u8>, [u8; 32]) {
        path.map_or_else(
            || (vec![[0; 32]; TREE_DEPTH], vec![0; TREE_DEPTH], [0; 32]),
            |path| (path.siblings.clone(), path.directions.clone(), path.root),
        )
    }

    fn validate_path_encoding(
        path: &iroha_data_model::offline::KagemushaConfidentialMerklePathV2,
        label: &str,
    ) -> Result<(), String> {
        if path.siblings.len() != TREE_DEPTH || path.directions.len() != TREE_DEPTH {
            return Err(format!(
                "{label} must contain exactly {TREE_DEPTH} siblings and directions"
            ));
        }
        for (level, sibling) in path.siblings.iter().copied().enumerate() {
            scalar_from_repr(sibling).ok_or_else(|| {
                format!("{label} sibling[{level}] is not a canonical Pallas scalar")
            })?;
        }
        scalar_from_repr(path.root)
            .ok_or_else(|| format!("{label} root is not a canonical Pallas scalar"))?;
        Ok(())
    }

    fn validate_leaf_encoding(
        leaf: &KagemushaOutputMembershipLeafV3,
        label: &str,
    ) -> Result<(), String> {
        scalar_from_repr(leaf.commitment)
            .ok_or_else(|| format!("{label} commitment is not a canonical Pallas scalar"))?;
        validate_path_encoding(&leaf.update_path, &format!("{label} update path"))?;
        validate_path_encoding(&leaf.membership_path, &format!("{label} membership path"))
    }

    pub(super) fn validate_witness_encoding(
        witness: &KagemushaOutputMembershipWitnessV3,
    ) -> Result<(), String> {
        scalar_from_repr(witness.initial_root)
            .ok_or_else(|| "initial root is not a canonical Pallas scalar".to_owned())?;
        scalar_from_repr(witness.final_root)
            .ok_or_else(|| "final root is not a canonical Pallas scalar".to_owned())?;
        if let Some(recipient) = &witness.recipient {
            validate_leaf_encoding(recipient, "recipient")?;
        }
        if let Some(change) = &witness.change {
            validate_leaf_encoding(change, "change")?;
        }
        validate_path_encoding(&witness.dummy_path, "dummy membership path")
    }

    fn scalar(bytes: [u8; 32], label: &str) -> Result<Scalar, String> {
        scalar_from_repr(bytes).ok_or_else(|| format!("{label} is not a canonical Pallas scalar"))
    }

    fn profile_flags(operation: KagemushaOutputMembershipOperationV3) -> [Scalar; 3] {
        match operation {
            KagemushaOutputMembershipOperationV3::Init => [Scalar::ONE, Scalar::ZERO, Scalar::ZERO],
            KagemushaOutputMembershipOperationV3::Split => {
                [Scalar::ZERO, Scalar::ONE, Scalar::ZERO]
            }
            KagemushaOutputMembershipOperationV3::RedemptionChange => {
                [Scalar::ZERO, Scalar::ZERO, Scalar::ONE]
            }
        }
    }

    pub(super) fn public_instances(
        witness: &KagemushaOutputMembershipWitnessV3,
    ) -> Result<Vec<Vec<Scalar>>, String> {
        validate_witness_encoding(witness)?;
        let [is_init, is_split, is_redemption] = profile_flags(witness.operation);
        let has_change = if witness.change.is_some() {
            Scalar::ONE
        } else {
            Scalar::ZERO
        };
        let recipient_commitment = witness
            .recipient
            .as_ref()
            .map_or(Ok(Scalar::ZERO), |leaf| {
                scalar(leaf.commitment, "recipient commitment")
            })?;
        let recipient_index = Scalar::from(u64::from(
            witness.recipient.as_ref().map_or(0, |leaf| leaf.leaf_index),
        ));
        let change_commitment = witness.change.as_ref().map_or(Ok(Scalar::ZERO), |leaf| {
            scalar(leaf.commitment, "change commitment")
        })?;
        let change_index = Scalar::from(u64::from(
            witness.change.as_ref().map_or(0, |leaf| leaf.leaf_index),
        ));
        let instances = [
            is_init,
            is_split,
            is_redemption,
            has_change,
            scalar(witness.initial_root, "initial root")?,
            scalar(witness.final_root, "final root")?,
            recipient_commitment,
            recipient_index,
            change_commitment,
            change_index,
            Scalar::from(u64::from(witness.dummy_leaf_index)),
        ]
        .map(|value| vec![value])
        .to_vec();
        debug_assert_eq!(
            instances.len(),
            KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V3
        );
        Ok(instances)
    }

    fn assert_equal(
        ctx: &mut Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        lhs: AssignedValue<Scalar>,
        rhs: AssignedValue<Scalar>,
    ) {
        let difference = range.gate().sub(ctx, lhs, rhs);
        range
            .gate()
            .assert_is_const(ctx, &difference, &Scalar::ZERO);
    }

    fn assert_equal_if(
        ctx: &mut Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        enabled: AssignedValue<Scalar>,
        lhs: AssignedValue<Scalar>,
        rhs: AssignedValue<Scalar>,
    ) {
        let difference = range.gate().sub(ctx, lhs, rhs);
        let selected = range.gate().mul(ctx, enabled, difference);
        range.gate().assert_is_const(ctx, &selected, &Scalar::ZERO);
    }

    fn assert_present_exactly(
        ctx: &mut Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        value: AssignedValue<Scalar>,
        present: AssignedValue<Scalar>,
    ) {
        let is_zero = range.gate().is_zero(ctx, value);
        let absent = range.gate().not(ctx, present);
        assert_equal(ctx, range, is_zero, absent);
    }

    struct AssignedMerklePath {
        siblings: Vec<AssignedValue<Scalar>>,
        directions: Vec<AssignedValue<Scalar>>,
        carried_root: AssignedValue<Scalar>,
    }

    fn load_path(
        ctx: &mut Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        path: Option<&iroha_data_model::offline::KagemushaConfidentialMerklePathV2>,
        index_bits: &[AssignedValue<Scalar>],
    ) -> AssignedMerklePath {
        let (siblings, directions, carried_root) = path_values(path);
        let mut assigned_siblings = Vec::with_capacity(TREE_DEPTH);
        let mut assigned_directions = Vec::with_capacity(TREE_DEPTH);
        for level in 0..TREE_DEPTH {
            let sibling = ctx.load_witness(
                scalar_from_repr(siblings[level]).expect("validated path sibling encoding"),
            );
            let direction = ctx.load_witness(Scalar::from(u64::from(directions[level])));
            range.gate().assert_bit(ctx, direction);
            assert_equal(ctx, range, direction, index_bits[level]);
            assigned_siblings.push(sibling);
            assigned_directions.push(direction);
        }
        let carried_root =
            ctx.load_witness(scalar_from_repr(carried_root).expect("validated path root encoding"));
        AssignedMerklePath {
            siblings: assigned_siblings,
            directions: assigned_directions,
            carried_root,
        }
    }

    fn path_root(
        ctx: &mut Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        poseidon: &ConfidentialPoseidonChipV3<Scalar>,
        commitment: AssignedValue<Scalar>,
        path: &AssignedMerklePath,
    ) -> AssignedValue<Scalar> {
        let mut node = poseidon.hash(
            ctx,
            range,
            CONFIDENTIAL_POSEIDON_MERKLE_LEAF_DOMAIN_V3,
            &[commitment],
        );
        for level in 0..TREE_DEPTH {
            let sibling = path.siblings[level];
            let direction = path.directions[level];
            let left = range.gate().select(ctx, sibling, node, direction);
            let right = range.gate().select(ctx, node, sibling, direction);
            node = poseidon.hash(
                ctx,
                range,
                CONFIDENTIAL_POSEIDON_MERKLE_NODE_DOMAIN_V3,
                &[left, right],
            );
        }
        node
    }

    fn update_roots(
        ctx: &mut Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        poseidon: &ConfidentialPoseidonChipV3<Scalar>,
        commitment: AssignedValue<Scalar>,
        path: Option<&iroha_data_model::offline::KagemushaConfidentialMerklePathV2>,
        index_bits: &[AssignedValue<Scalar>],
        present: AssignedValue<Scalar>,
    ) -> (AssignedValue<Scalar>, AssignedValue<Scalar>) {
        let zero = ctx.load_zero();
        let path = load_path(ctx, range, path, index_bits);
        let before = path_root(ctx, range, poseidon, zero, &path);
        assert_equal_if(ctx, range, present, before, path.carried_root);
        let after = path_root(ctx, range, poseidon, commitment, &path);
        (before, after)
    }

    fn membership_root(
        ctx: &mut Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        poseidon: &ConfidentialPoseidonChipV3<Scalar>,
        commitment: AssignedValue<Scalar>,
        path: Option<&iroha_data_model::offline::KagemushaConfidentialMerklePathV2>,
        index_bits: &[AssignedValue<Scalar>],
        present: AssignedValue<Scalar>,
        final_root: AssignedValue<Scalar>,
    ) {
        let path = load_path(ctx, range, path, index_bits);
        let computed = path_root(ctx, range, poseidon, commitment, &path);
        assert_equal_if(ctx, range, present, computed, path.carried_root);
        assert_equal_if(ctx, range, present, computed, final_root);
    }

    fn builder(
        witness: Option<&KagemushaOutputMembershipWitnessV3>,
    ) -> Result<BaseCircuitBuilder<Scalar>, String> {
        if let Some(witness) = witness {
            validate_witness_encoding(witness)?;
        }
        let mut builder = BaseCircuitBuilder::new(false)
            .use_k(
                usize::try_from(KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V3)
                    .expect("Kagemusha membership IPA k fits usize"),
            )
            .use_lookup_bits(
                usize::try_from(KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V3 - 1)
                    .expect("Kagemusha membership lookup bits fit usize"),
            )
            .use_instance_columns(KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V3);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let gate = range.gate();

        let flags = witness.map_or([Scalar::ZERO; 3], |witness| {
            profile_flags(witness.operation)
        });
        let [is_init, is_split, is_redemption] = flags.map(|flag| ctx.load_witness(flag));
        for flag in [is_init, is_split, is_redemption] {
            gate.assert_bit(ctx, flag);
        }
        let profile_sum = gate.add(ctx, is_init, is_split);
        let profile_sum = gate.add(ctx, profile_sum, is_redemption);
        gate.assert_is_const(ctx, &profile_sum, &Scalar::ONE);

        let has_change = ctx.load_witness(if witness.is_some_and(|value| value.change.is_some()) {
            Scalar::ONE
        } else {
            Scalar::ZERO
        });
        gate.assert_bit(ctx, has_change);
        let not_change = gate.not(ctx, has_change);
        let invalid_init_change = gate.mul(ctx, is_init, has_change);
        gate.assert_is_const(ctx, &invalid_init_change, &Scalar::ZERO);
        let missing_redemption_change = gate.mul(ctx, is_redemption, not_change);
        gate.assert_is_const(ctx, &missing_redemption_change, &Scalar::ZERO);
        let recipient_present = gate.add(ctx, is_init, is_split);

        let initial_root = ctx.load_witness(witness.map_or(Scalar::ZERO, |value| {
            scalar(value.initial_root, "validated initial root").expect("validated initial root")
        }));
        let final_root = ctx.load_witness(witness.map_or(Scalar::ZERO, |value| {
            scalar(value.final_root, "validated final root").expect("validated final root")
        }));
        for root in [initial_root, final_root] {
            let is_zero = gate.is_zero(ctx, root);
            gate.assert_is_const(ctx, &is_zero, &Scalar::ZERO);
        }
        let unchanged = gate.is_equal(ctx, initial_root, final_root);
        gate.assert_is_const(ctx, &unchanged, &Scalar::ZERO);

        let recipient = witness.and_then(|value| value.recipient.as_ref());
        let change = witness.and_then(|value| value.change.as_ref());
        let recipient_commitment = ctx.load_witness(recipient.map_or(Scalar::ZERO, |leaf| {
            scalar(leaf.commitment, "validated recipient commitment")
                .expect("validated recipient commitment")
        }));
        let change_commitment = ctx.load_witness(change.map_or(Scalar::ZERO, |leaf| {
            scalar(leaf.commitment, "validated change commitment")
                .expect("validated change commitment")
        }));
        assert_present_exactly(ctx, &range, recipient_commitment, recipient_present);
        assert_present_exactly(ctx, &range, change_commitment, has_change);

        let recipient_index = ctx.load_witness(Scalar::from(u64::from(
            recipient.map_or(0, |leaf| leaf.leaf_index),
        )));
        let change_index = ctx.load_witness(Scalar::from(u64::from(
            change.map_or(0, |leaf| leaf.leaf_index),
        )));
        let dummy_index = ctx.load_witness(Scalar::from(u64::from(
            witness.map_or(0, |value| value.dummy_leaf_index),
        )));
        for index in [recipient_index, change_index, dummy_index] {
            range.range_check(ctx, index, TREE_DEPTH);
        }
        let recipient_absent = gate.not(ctx, recipient_present);
        let absent_recipient_index = gate.mul(ctx, recipient_absent, recipient_index);
        gate.assert_is_const(ctx, &absent_recipient_index, &Scalar::ZERO);
        let absent_change_index = gate.mul(ctx, not_change, change_index);
        gate.assert_is_const(ctx, &absent_change_index, &Scalar::ZERO);

        let recipient_index_bits = gate.num_to_bits(ctx, recipient_index, TREE_DEPTH);
        let change_index_bits = gate.num_to_bits(ctx, change_index, TREE_DEPTH);
        let dummy_index_bits = gate.num_to_bits(ctx, dummy_index, TREE_DEPTH);
        let poseidon = ConfidentialPoseidonChipV3::new(ctx, &range);

        let recipient_update_path = recipient.map(|leaf| &leaf.update_path);
        let (recipient_before, recipient_after) = update_roots(
            ctx,
            &range,
            &poseidon,
            recipient_commitment,
            recipient_update_path,
            &recipient_index_bits,
            recipient_present,
        );
        assert_equal_if(
            ctx,
            &range,
            recipient_present,
            recipient_before,
            initial_root,
        );
        let root_after_recipient =
            gate.select(ctx, recipient_after, initial_root, recipient_present);

        let change_update_path = change.map(|leaf| &leaf.update_path);
        let (change_before, change_after) = update_roots(
            ctx,
            &range,
            &poseidon,
            change_commitment,
            change_update_path,
            &change_index_bits,
            has_change,
        );
        assert_equal_if(ctx, &range, has_change, change_before, root_after_recipient);
        let computed_final = gate.select(ctx, change_after, root_after_recipient, has_change);
        assert_equal(ctx, &range, computed_final, final_root);

        let split_with_change = gate.mul(ctx, is_split, has_change);
        let one = ctx.load_constant(Scalar::ONE);
        let next_recipient_index = gate.add(ctx, recipient_index, one);
        assert_equal_if(
            ctx,
            &range,
            split_with_change,
            change_index,
            next_recipient_index,
        );
        let same_output_index = gate.is_equal(ctx, recipient_index, change_index);
        let both_outputs = gate.mul(ctx, recipient_present, has_change);
        let duplicate_output_index = gate.mul(ctx, both_outputs, same_output_index);
        gate.assert_is_const(ctx, &duplicate_output_index, &Scalar::ZERO);
        let same_commitment = gate.is_equal(ctx, recipient_commitment, change_commitment);
        let duplicate_commitment = gate.mul(ctx, both_outputs, same_commitment);
        gate.assert_is_const(ctx, &duplicate_commitment, &Scalar::ZERO);

        membership_root(
            ctx,
            &range,
            &poseidon,
            recipient_commitment,
            recipient.map(|leaf| &leaf.membership_path),
            &recipient_index_bits,
            recipient_present,
            final_root,
        );
        membership_root(
            ctx,
            &range,
            &poseidon,
            change_commitment,
            change.map(|leaf| &leaf.membership_path),
            &change_index_bits,
            has_change,
            final_root,
        );
        let zero = ctx.load_zero();
        membership_root(
            ctx,
            &range,
            &poseidon,
            zero,
            witness.map(|value| &value.dummy_path),
            &dummy_index_bits,
            one,
            final_root,
        );

        let dummy_is_recipient = gate.is_equal(ctx, dummy_index, recipient_index);
        let invalid_dummy_recipient = gate.mul(ctx, recipient_present, dummy_is_recipient);
        gate.assert_is_const(ctx, &invalid_dummy_recipient, &Scalar::ZERO);
        let dummy_is_change = gate.is_equal(ctx, dummy_index, change_index);
        let invalid_dummy_change = gate.mul(ctx, has_change, dummy_is_change);
        gate.assert_is_const(ctx, &invalid_dummy_change, &Scalar::ZERO);

        builder.assigned_instances = vec![
            vec![is_init],
            vec![is_split],
            vec![is_redemption],
            vec![has_change],
            vec![initial_root],
            vec![final_root],
            vec![recipient_commitment],
            vec![recipient_index],
            vec![change_commitment],
            vec![change_index],
            vec![dummy_index],
        ];
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        Ok(builder)
    }

    impl Circuit<Scalar> for KagemushaOutputMembershipCircuitV3 {
        type Config = BaseConfig<Scalar>;
        type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let params: BaseCircuitParams = builder(None)
                .expect("witness-free output-membership relation has a fixed shape")
                .config_params;
            BaseConfig::configure(meta, params)
        }

        fn synthesize(
            &self,
            config: Self::Config,
            layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let builder = builder(self.witness.as_ref()).map_err(|_| PlonkError::Synthesis)?;
            <BaseCircuitBuilder<Scalar> as Circuit<Scalar>>::synthesize(&builder, config, layouter)
        }
    }
}
#[cfg(test)]
fn bytes_to_limbs(bytes: &[u8; 32]) -> [Scalar; 4] {
    super::bytes_to_u64_limbs_le(bytes).map(Scalar::from)
}

#[cfg(test)]
fn write_limb_group(
    public: &mut [Scalar; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS],
    start: usize,
    bytes: &[u8; 32],
) {
    public[start..start + 4].copy_from_slice(&bytes_to_limbs(bytes));
}

#[cfg(test)]
fn scalar_from_canonical_bytes(bytes: &[u8; 32], field: &str) -> Result<Scalar, String> {
    let mut repr = <Scalar as PrimeField>::Repr::default();
    repr.as_mut().copy_from_slice(bytes);
    Option::from(Scalar::from_repr(repr))
        .ok_or_else(|| format!("Kagemusha V2 {field} is not a canonical Pasta scalar"))
}

fn canonical_poseidon_digest<T: Encode>(value: &T) -> Result<[u8; 32], String> {
    let bytes = norito::to_bytes(value)
        .map_err(|err| format!("failed to encode Kagemusha V2 binding value: {err}"))?;
    Ok(iroha_zkp_halo2::poseidon::hash_bytes(&bytes))
}

fn write_exact_u32_limbs<const N: usize>(target: &mut [u32], bytes: &[u8; N]) {
    assert_eq!(N % 4, 0, "exact state-vector byte strings use u32 limbs");
    assert_eq!(
        target.len(),
        N / 4,
        "state-vector limb slice has fixed size"
    );
    for (limb, chunk) in target.iter_mut().zip(bytes.chunks_exact(4)) {
        *limb = u32::from_le_bytes(chunk.try_into().expect("four-byte exact limb"));
    }
}

fn bytes_to_exact_u32_limbs(bytes: &[u8; 32]) -> [u32; 8] {
    std::array::from_fn(|index| {
        let start = index * 4;
        u32::from_le_bytes(
            bytes[start..start + 4]
                .try_into()
                .expect("32-byte values contain eight exact u32 limbs"),
        )
    })
}

impl KagemushaRecursiveSpendOperationKindV1 {
    /// Derive the operation kind from the exact submitted statement.
    pub fn from_statement(
        statement: &KagemushaRecursiveSpendPublicStatementV2,
    ) -> Result<Self, String> {
        statement
            .validate_public_binding()
            .map_err(|err| err.to_string())?;
        Ok(match statement.transition {
            None => Self::Init,
            Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(_)) => Self::Append,
            Some(KagemushaRecursiveSpendTransitionV2::RedemptionChange(_)) => {
                Self::RedemptionChange
            }
        })
    }

    /// Return the exact number of predecessor proofs required by this kind.
    #[must_use]
    pub const fn minimum_predecessor_count(self) -> usize {
        match self {
            Self::Init => 0,
            Self::Append | Self::RedemptionChange => 1,
        }
    }
}

impl KagemushaRecursiveSpendStateVectorV1 {
    /// Embedded proof-step count used by the paired recursion relation.
    #[must_use]
    pub(crate) const fn proof_step_count(&self) -> u32 {
        self.limbs[S_PROOF_STEP_COUNT]
    }

    /// Embedded peer-hop count used by the paired recursion relation.
    #[must_use]
    pub(crate) const fn peer_hop_count(&self) -> u32 {
        self.limbs[S_PEER_HOP_COUNT]
    }

    /// Embedded authenticated artifact-manifest identity.
    #[must_use]
    pub(crate) fn manifest_sha256_limbs(&self) -> [u32; 8] {
        self.limbs[S_ARTIFACT_MANIFEST_SHA256..S_ARTIFACT_MANIFEST_SHA256 + 8]
            .try_into()
            .expect("manifest slot has eight exact limbs")
    }

    /// Reconstruct the complete continuing-state vector from a validated statement.
    pub fn from_statement(
        statement: &KagemushaRecursiveSpendPublicStatementV2,
    ) -> Result<Self, String> {
        statement
            .validate_public_binding()
            .map_err(|err| err.to_string())?;
        let vector = Self::from_statement_inner(statement)?;
        vector.validate_against_statement(statement)?;
        Ok(vector)
    }

    fn from_statement_inner(
        statement: &KagemushaRecursiveSpendPublicStatementV2,
    ) -> Result<Self, String> {
        let mut limbs = [0_u32; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1];
        limbs[S_VERSION] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V1;

        let chain_tag =
            super::confidential_v2::derive_confidential_chain_tag_v3(statement.chain_id.as_str())?;
        write_exact_u32_limbs(&mut limbs[S_CHAIN_TAG..S_CHAIN_TAG + 8], &chain_tag);
        let asset_tag =
            super::confidential_v2::derive_confidential_asset_tag_v3(&statement.asset.to_string())?;
        write_exact_u32_limbs(&mut limbs[S_ASSET_TAG..S_ASSET_TAG + 8], &asset_tag);
        limbs[S_ASSET_SCALE] = statement.asset_scale;
        write_exact_u32_limbs(
            &mut limbs[S_FINAL_ROOT..S_FINAL_ROOT + 8],
            &statement.final_root,
        );

        limbs[S_TOPUP_ANCHOR_COUNT] = u32::try_from(statement.topup_anchor_refs.len())
            .map_err(|_| "Kagemusha top-up anchor count does not fit u32".to_owned())?;
        for (index, anchor) in statement.topup_anchor_refs.iter().enumerate() {
            let start = S_TOPUP_ANCHORS + index * 16;
            write_exact_u32_limbs(&mut limbs[start..start + 8], &anchor.topup_operation_id);
            write_exact_u32_limbs(&mut limbs[start + 8..start + 16], &anchor.anchor_digest);
        }

        limbs[S_PROOF_STEP_COUNT] = statement.proof_step_count;
        limbs[S_PEER_HOP_COUNT] = statement.peer_hop_count;
        write_exact_u32_limbs(
            &mut limbs[S_CURRENT_COMMITMENT..S_CURRENT_COMMITMENT + 8],
            &statement.current_note.note_commitment,
        );
        write_exact_u32_limbs(
            &mut limbs[S_CURRENT_NULLIFIER..S_CURRENT_NULLIFIER + 8],
            &statement.current_note.spend_nullifier,
        );
        write_exact_u32_limbs(
            &mut limbs[S_CURRENT_AMOUNT..S_CURRENT_AMOUNT + 4],
            &statement.current_note.amount.atomic_units.to_le_bytes(),
        );
        limbs[S_CURRENT_SCALE] = statement.current_note.amount.scale;

        limbs[S_BRANCH_CLAIM_COUNT] = u32::try_from(statement.branch_claims.len())
            .map_err(|_| "Kagemusha branch-claim count does not fit u32".to_owned())?;
        for (index, claim) in statement.branch_claims.iter().enumerate() {
            let start =
                S_BRANCH_CLAIMS + index * KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V1;
            write_exact_u32_limbs(&mut limbs[start..start + 8], &claim.path.lineage_root);
            limbs[start + 8] = u32::from(claim.path.depth);
            write_exact_u32_limbs(&mut limbs[start + 9..start + 11], &claim.path.path_bits);
            let history = start + 11;
            for (tag_index, tag) in claim
                .transition_tags
                .chunks_exact(KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2)
                .enumerate()
            {
                let tag_start = history
                    + tag_index * KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V1;
                for (limb, chunk) in limbs[tag_start
                    ..tag_start + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V1]
                    .iter_mut()
                    .zip(tag.chunks_exact(4))
                {
                    *limb = u32::from_le_bytes(
                        chunk
                            .try_into()
                            .expect("transition tag has exact u32 limbs"),
                    );
                }
            }
        }

        write_exact_u32_limbs(
            &mut limbs[S_ARTIFACT_MANIFEST_SHA256..S_ARTIFACT_MANIFEST_SHA256 + 8],
            &statement.artifact_binding.manifest_sha256,
        );
        let verifier_key_id = canonical_poseidon_digest(&statement.verifier_key_id)?;
        write_exact_u32_limbs(
            &mut limbs[S_VERIFIER_KEY_ID..S_VERIFIER_KEY_ID + 8],
            &verifier_key_id,
        );

        Ok(Self { limbs })
    }

    /// Validate every count, ordering, padding, and limb against a submitted statement.
    pub fn validate_against_statement(
        &self,
        statement: &KagemushaRecursiveSpendPublicStatementV2,
    ) -> Result<(), String> {
        statement
            .validate_public_binding()
            .map_err(|err| err.to_string())?;
        if self.limbs[S_VERSION] != KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V1 {
            return Err("Kagemusha recursive state-vector version mismatch".to_owned());
        }

        // Rebuild the exact vector without calling this validator recursively.
        let rebuilt = Self::from_statement_inner(statement)?;
        if self != &rebuilt {
            return Err(
                "Kagemusha recursive state vector does not exactly match the submitted statement"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
fn split_u128(value: u128) -> [Scalar; 2] {
    [
        Scalar::from(value as u64),
        Scalar::from((value >> 64) as u64),
    ]
}

#[cfg(test)]
fn write_amount(
    public: &mut [Scalar; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS],
    start: usize,
    value: u128,
) {
    public[start..start + 2].copy_from_slice(&split_u128(value));
}

#[cfg(test)]
fn path_bits_as_u64(path_bits: [u8; 8]) -> u64 {
    u64::from_be_bytes(path_bits)
}

#[cfg(test)]
fn branch_selector(branch: KagemushaRecursiveSpendBranchV2) -> Scalar {
    match branch {
        KagemushaRecursiveSpendBranchV2::Recipient => Scalar::from(0),
        KagemushaRecursiveSpendBranchV2::Change => Scalar::from(1),
    }
}

#[cfg(test)]
fn fill_common_statement_values(
    public: &mut [Scalar; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS],
    statement: &KagemushaRecursiveSpendPublicStatementV2,
    init_root: Option<[u8; 32]>,
    current_hop_domain_tag: [u8; 32],
    topup_receipt_digest: [u8; 32],
) -> Result<(), String> {
    statement
        .validate_public_binding()
        .map_err(|err| err.to_string())?;
    let [branch_claim] = statement.branch_claims.as_slice() else {
        return Err(
            "current Kagemusha V2 transition layout cannot represent joined branch claims"
                .to_owned(),
        );
    };
    let [topup_anchor_ref] = statement.topup_anchor_refs.as_slice() else {
        return Err(
            "current Kagemusha V2 transition layout cannot represent multiple top-up origins"
                .to_owned(),
        );
    };
    let branch_path = &branch_claim.path;
    public[I_LAYOUT_VERSION] = Scalar::from(KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION);
    public[I_PROOF_STEP_COUNT] = Scalar::from(u64::from(statement.proof_step_count));
    public[I_PEER_HOP_COUNT] = Scalar::from(u64::from(statement.peer_hop_count));
    public[I_BRANCH_DEPTH] = Scalar::from(u64::from(branch_path.depth));
    public[I_ASSET_SCALE] = Scalar::from(u64::from(statement.asset_scale));
    public[I_CURRENT_SCALE] = Scalar::from(u64::from(statement.current_note.amount.scale));
    public[I_BRANCH_PATH_BITS] = Scalar::from(path_bits_as_u64(branch_path.path_bits));
    public[I_INITIAL_ROOT] =
        scalar_from_canonical_bytes(&init_root.unwrap_or([0; 32]), "initial root")?;
    public[I_FINAL_ROOT] = scalar_from_canonical_bytes(&statement.final_root, "final root")?;
    public[I_CURRENT_COMMITMENT] = scalar_from_canonical_bytes(
        &statement.current_note.note_commitment,
        "current note commitment",
    )?;
    public[I_CURRENT_NULLIFIER] = scalar_from_canonical_bytes(
        &statement.current_note.spend_nullifier,
        "current note nullifier",
    )?;
    write_amount(
        public,
        I_CURRENT_AMOUNT_LO,
        statement.current_note.amount.atomic_units,
    );
    write_limb_group(
        public,
        I_STATEMENT_DIGEST,
        &statement.digest().map_err(|err| err.to_string())?,
    );
    write_limb_group(public, I_BRANCH_LINEAGE_ROOT, &branch_path.lineage_root);
    write_limb_group(
        public,
        I_CHAIN_ID_DIGEST,
        &canonical_poseidon_digest(&statement.chain_id)?,
    );
    write_limb_group(
        public,
        I_ASSET_ID_DIGEST,
        &canonical_poseidon_digest(&statement.asset)?,
    );
    write_limb_group(
        public,
        I_TOPUP_OPERATION_ID,
        &topup_anchor_ref.topup_operation_id,
    );
    write_limb_group(
        public,
        I_ARTIFACT_MANIFEST_SHA256,
        &statement.artifact_binding.manifest_sha256,
    );
    write_limb_group(public, I_CURRENT_HOP_DOMAIN_TAG, &current_hop_domain_tag);
    write_limb_group(public, I_TOPUP_RECEIPT_DIGEST, &topup_receipt_digest);
    write_limb_group(
        public,
        I_TOPUP_ANCHOR_DIGEST,
        &topup_anchor_ref.anchor_digest,
    );
    public[I_TOPUP_ANCHOR_COUNT] = Scalar::from(
        u64::try_from(statement.topup_anchor_refs.len())
            .map_err(|_| "Kagemusha V2 top-up anchor count does not fit u64".to_owned())?,
    );
    write_limb_group(
        public,
        I_VERIFIER_KEY_ID_DIGEST,
        &canonical_poseidon_digest(&statement.verifier_key_id)?,
    );
    Ok(())
}

#[cfg(test)]
fn ensure_transition_statement_binding(
    statement: &KagemushaRecursiveSpendPublicStatementV2,
    transition: &[Scalar],
) -> Result<(), String> {
    statement
        .validate_public_binding()
        .map_err(|err| err.to_string())?;
    if transition.len() < KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS {
        return Err("Kagemusha V2 recursive proof transition instance is truncated".to_owned());
    }

    let mut expected = [Scalar::from(0); KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS];
    fill_common_statement_values(&mut expected, statement, None, [0; 32], [0; 32])?;
    let asset_tag =
        super::confidential_v2::derive_confidential_asset_tag_v3(&statement.asset.to_string())?;
    expected[I_ASSET_TAG] = scalar_from_canonical_bytes(&asset_tag, "statement asset tag")?;
    let chain_tag =
        super::confidential_v2::derive_confidential_chain_tag_v3(statement.chain_id.as_str())?;
    expected[I_CHAIN_TAG] = scalar_from_canonical_bytes(&chain_tag, "statement chain tag")?;

    let require_row = |index: usize, field: &str| {
        if transition[index] != expected[index] {
            return Err(format!(
                "Kagemusha V2 recursive proof {field} does not match the submitted statement"
            ));
        }
        Ok(())
    };
    for (index, field) in [
        (I_LAYOUT_VERSION, "layout version"),
        (I_PROOF_STEP_COUNT, "proof-step count"),
        (I_PEER_HOP_COUNT, "peer-hop count"),
        (I_BRANCH_DEPTH, "branch depth"),
        (I_ASSET_SCALE, "asset scale"),
        (I_CURRENT_SCALE, "current-note scale"),
        (I_CURRENT_AMOUNT_LO, "current-note amount low limb"),
        (I_CURRENT_AMOUNT_HI, "current-note amount high limb"),
        (I_BRANCH_PATH_BITS, "branch path"),
        (I_FINAL_ROOT, "final root"),
        (I_CURRENT_COMMITMENT, "current-note commitment"),
        (I_CURRENT_NULLIFIER, "current-note nullifier"),
        (I_ASSET_TAG, "confidential asset tag"),
        (I_CHAIN_TAG, "confidential chain tag"),
        (I_TOPUP_ANCHOR_COUNT, "top-up anchor count"),
    ] {
        require_row(index, field)?;
    }
    for (start, field) in [
        (I_STATEMENT_DIGEST, "statement digest"),
        (I_BRANCH_LINEAGE_ROOT, "branch lineage root"),
        (I_CHAIN_ID_DIGEST, "chain id"),
        (I_ASSET_ID_DIGEST, "asset definition id"),
        (I_TOPUP_OPERATION_ID, "top-up operation id"),
        (I_ARTIFACT_MANIFEST_SHA256, "artifact manifest SHA-256"),
        (I_TOPUP_ANCHOR_DIGEST, "top-up anchor digest"),
        (I_VERIFIER_KEY_ID_DIGEST, "verifier key id"),
    ] {
        for index in start..start + 4 {
            require_row(index, field)?;
        }
    }

    let zero = Scalar::from(0);
    let one = Scalar::from(1);
    let require_value = |index: usize, value: Scalar, field: &str| {
        if transition[index] != value {
            return Err(format!(
                "Kagemusha V2 recursive proof {field} does not match the submitted transition"
            ));
        }
        Ok(())
    };
    let require_limbs = |start: usize, value: &[u8; 32], field: &str| {
        if transition[start..start + 4] != bytes_to_limbs(value) {
            return Err(format!(
                "Kagemusha V2 recursive proof {field} does not match the submitted transition"
            ));
        }
        Ok(())
    };
    let require_parent_path = |branch: KagemushaRecursiveSpendBranchV2| -> Result<(), String> {
        let [claim] = statement.branch_claims.as_slice() else {
            return Err(
                "current Kagemusha V2 transition layout cannot bind joined branch claims"
                    .to_owned(),
            );
        };
        let parent = claim.path.parent().ok_or_else(|| {
            "Kagemusha V2 extending transition has no parent branch path".to_owned()
        })?;
        if parent.child(branch).map_err(|err| err.to_string())? != claim.path {
            return Err(
                "Kagemusha V2 statement branch path does not match its selected branch".to_owned(),
            );
        }
        require_value(
            I_PARENT_BRANCH_DEPTH,
            Scalar::from(u64::from(parent.depth)),
            "parent branch depth",
        )?;
        require_value(
            I_PARENT_BRANCH_PATH_BITS,
            Scalar::from(path_bits_as_u64(parent.path_bits)),
            "parent branch path",
        )?;
        require_limbs(
            I_PARENT_BRANCH_LINEAGE_ROOT,
            &parent.lineage_root,
            "parent branch lineage root",
        )
    };

    match &statement.transition {
        None => {
            for (index, field) in [
                (I_APPEND_PROFILE, "append profile"),
                (I_REDEMPTION_PROFILE, "redemption profile"),
                (I_BRANCH_CHANGE, "init branch selector"),
                (I_HAS_CHANGE, "init change selector"),
            ] {
                require_value(index, zero, field)?;
            }
            for (start, field) in [
                (I_SPLIT_DIGEST, "init split digest"),
                (I_RECIPIENT_REQUEST_DIGEST, "init recipient request"),
                (I_OPERATION_ID, "init transition operation"),
                (I_PARENT_BUNDLE_DIGEST, "init parent bundle"),
            ] {
                require_limbs(start, &[0; 32], field)?;
            }
        }
        Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(split)) => {
            require_value(I_APPEND_PROFILE, one, "append profile")?;
            require_value(I_REDEMPTION_PROFILE, zero, "redemption profile")?;
            require_value(
                I_BRANCH_CHANGE,
                branch_selector(split.branch),
                "peer-split branch selector",
            )?;
            require_limbs(I_SPLIT_DIGEST, &split.binding_digest, "peer-split digest")?;
            require_limbs(
                I_RECIPIENT_REQUEST_DIGEST,
                &split.recipient_request_digest,
                "recipient request digest",
            )?;
            require_limbs(
                I_OPERATION_ID,
                &split.operation_id,
                "peer-split operation id",
            )?;
            require_value(
                I_PREVIOUS_PROOF_STEP_COUNT,
                Scalar::from(u64::from(split.parent_max_proof_step_count)),
                "parent proof-step count",
            )?;
            require_value(
                I_PREVIOUS_PEER_HOP_COUNT,
                Scalar::from(u64::from(split.parent_max_peer_hop_count)),
                "parent peer-hop count",
            )?;
            require_parent_path(split.branch)?;
        }
        Some(KagemushaRecursiveSpendTransitionV2::RedemptionChange(redemption)) => {
            require_value(I_APPEND_PROFILE, zero, "append profile")?;
            require_value(I_REDEMPTION_PROFILE, one, "redemption profile")?;
            require_value(I_BRANCH_CHANGE, one, "redemption branch selector")?;
            require_value(I_HAS_CHANGE, one, "redemption change selector")?;
            require_limbs(
                I_SPLIT_DIGEST,
                &redemption.binding_digest,
                "redemption binding digest",
            )?;
            require_limbs(
                I_OPERATION_ID,
                &redemption.operation_id,
                "redemption operation id",
            )?;
            require_limbs(
                I_PARENT_BUNDLE_DIGEST,
                &redemption.parent_bundle_digest,
                "redemption parent bundle digest",
            )?;
            require_limbs(
                I_RECIPIENT_REQUEST_DIGEST,
                &[0; 32],
                "redemption recipient request digest",
            )?;
            require_value(
                I_PREVIOUS_PROOF_STEP_COUNT,
                Scalar::from(u64::from(redemption.parent_proof_step_count)),
                "redemption parent proof-step count",
            )?;
            require_value(
                I_PREVIOUS_PEER_HOP_COUNT,
                Scalar::from(u64::from(redemption.parent_peer_hop_count)),
                "redemption parent peer-hop count",
            )?;
            require_parent_path(KagemushaRecursiveSpendBranchV2::Change)?;
        }
    }
    Ok(())
}

/// Return the three streaming public instance columns for a V2 transition.
///
/// The columns are a deterministic R1CS trace derived from the structured
/// transition values. Callers never accept these expanded columns from the
/// wire; terminal verification reconstructs them from the canonical bundle.
#[must_use]
pub fn kagemusha_recursive_spend_transition_instance_columns_v2<F: PrimeField>(
    values: &KagemushaRecursiveSpendTransitionValuesV2<F>,
) -> Vec<Vec<F>> {
    kagemusha_recursive_spend_transition_trace_v2(values).instance_columns()
}

/// Validate the chain-visible binding between a V2 bundle and its proof envelope.
///
/// Cryptographic verification alone proves the instance columns embedded in the
/// envelope. Consensus admission must additionally bind every statement-derived
/// transition row (not only the statement digest) to the exact canonical bundle
/// submitted to the ledger. Otherwise an attacker could pair a valid proof for
/// one note, root, branch, asset, or chain with the digest of another statement.
/// This helper performs only that metadata/instance binding. Callers must still
/// verify the Halo2 proof with the registered key, and the unavailable composite
/// backend must remain disabled until its lineage/transition equality constraints
/// are implemented in-circuit.
pub fn ensure_kagemusha_recursive_spend_v2_proof_envelope_binding(
    bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV2,
    step_eq_record: &VerifyingKeyRecord,
    step_ep_record: &VerifyingKeyRecord,
) -> Result<(), String> {
    use iroha_data_model::offline::{
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1, KagemushaPastaCycleProofEnvelopeV1,
    };
    use sha2::{Digest as _, Sha256};

    bundle
        .validate_public_binding()
        .map_err(|err| err.to_string())?;
    let proof = &bundle.recursive_proof.proof;
    if proof.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1 {
        return Err("Kagemusha V2 recursive proof schema/backend mismatch".to_owned());
    }
    let step_eq_verifier_key = step_eq_record
        .key
        .as_ref()
        .ok_or_else(|| "Kagemusha Eq recursive verifier has no inline key".to_owned())?;
    let step_ep_verifier_key = step_ep_record
        .key
        .as_ref()
        .ok_or_else(|| "Kagemusha Ep recursive verifier has no inline key".to_owned())?;
    let envelope: KagemushaPastaCycleProofEnvelopeV1 = norito::decode_from_bytes(&proof.bytes)
        .map_err(|_| "Kagemusha V2 recursive proof is not a Pasta-cycle envelope".to_owned())?;
    let canonical_envelope = norito::to_bytes(&envelope)
        .map_err(|err| format!("failed to re-encode Kagemusha Pasta-cycle envelope: {err}"))?;
    if canonical_envelope != proof.bytes {
        return Err("Kagemusha V2 recursive proof envelope is not canonical".to_owned());
    }
    envelope
        .validate()
        .map_err(|err| format!("Kagemusha V2 recursive proof envelope is invalid: {err}"))?;

    let step_eq_verifier_key_sha256: [u8; 32] = Sha256::digest(&step_eq_verifier_key.bytes).into();
    let step_ep_verifier_key_sha256: [u8; 32] = Sha256::digest(&step_ep_verifier_key.bytes).into();
    if envelope.step_eq_circuit_id != step_eq_record.circuit_id
        || envelope.step_ep_circuit_id != step_ep_record.circuit_id
        || bundle.statement.verifier_key_id.name
            != iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1
        || envelope.artifact_generation != bundle.statement.artifact_binding.generation
        || envelope.manifest_sha256 != bundle.statement.artifact_binding.manifest_sha256
        || envelope.step_eq_verifier_key_sha256 != step_eq_verifier_key_sha256
        || envelope.step_ep_verifier_key_sha256 != step_ep_verifier_key_sha256
        || step_eq_record.public_inputs_schema_hash
            != iroha_data_model::offline::kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v3()
        || step_ep_record.public_inputs_schema_hash
            != iroha_data_model::offline::kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v3()
    {
        return Err("Kagemusha V2 recursive proof envelope metadata mismatch".to_owned());
    }

    let proof_pair: super::kagemusha_recursion_adapter::KagemushaPastaCycleProofPairV1 =
        norito::decode_from_bytes(&envelope.proof.bytes)
            .map_err(|_| "Kagemusha recursive Eq/Ep proof pair is malformed".to_owned())?;
    let canonical_pair = norito::to_bytes(&proof_pair)
        .map_err(|err| format!("failed to re-encode Kagemusha proof pair: {err}"))?;
    if canonical_pair != envelope.proof.bytes {
        return Err("Kagemusha recursive Eq/Ep proof pair is not canonical".to_owned());
    }
    proof_pair.validate()?;

    let statement_digest = bundle.statement.digest().map_err(|err| err.to_string())?;
    let expected_state = KagemushaRecursiveSpendStateVectorV1::from_statement(&bundle.statement)?;
    let expected_state_boundary =
        iroha_data_model::offline::KagemushaRecursiveSpendStateBoundaryV1::new(
            expected_state.limbs.to_vec(),
        )
        .map_err(|err| err.to_string())?;
    let statement_limbs = bytes_to_exact_u32_limbs(&statement_digest);
    let manifest_limbs = bytes_to_exact_u32_limbs(&envelope.manifest_sha256);
    if proof_pair.proof_step_count != bundle.statement.proof_step_count
        || proof_pair.public_inputs.public_statement_digest != statement_limbs
        || proof_pair.public_inputs.result_state != expected_state.limbs
        || proof_pair.public_inputs.result_state != envelope.state_boundary.state_limbs
        || envelope.state_boundary != expected_state_boundary
        || proof_pair.public_inputs.manifest_sha256 != manifest_limbs
    {
        return Err("Kagemusha V2 recursive proof state/instance binding mismatch".to_owned());
    }
    Ok(())
}

/// Hash of the exact transition-instance schema bound by V3 verifier records.
#[must_use]
pub fn kagemusha_recursive_spend_v2_public_inputs_schema_hash() -> [u8; 32] {
    iroha_crypto::Hash::new(KAGEMUSHA_RECURSIVE_SPEND_V2_PUBLIC_INPUTS_SCHEMA).into()
}

/// Exact artifact type selected by the ABI-19 Pasta-cycle contract.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_TYPE_V3: &str =
    "KagemushaRecursiveSpendPastaCycleArtifactsV3";
/// Streaming archive format version selected by the ABI-19 contract.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_VERSION_V3: u16 = 3;
/// Framing magic for a streamed ABI-19 Pasta-cycle artifact.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3: &[u8; 8] =
    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V3;

/// Small authenticated header preceding one streamed Pasta-cycle artifact file.
///
/// This type deliberately carries no payload vector. Release tooling must hash
/// and size-check the following bytes incrementally before atomically exposing
/// them to the prover.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaRecursiveSpendPastaCycleArtifactsV3 {
    /// Package layout version.
    pub version: u16,
    /// Exact manifest schema that authorized the package.
    pub manifest_schema: String,
    /// Native bridge ABI required by the package.
    pub bridge_abi_version: u32,
    /// Exact two-layer backend profile.
    pub proof_backend: String,
    /// Exact circuit-native transcript profile.
    pub transcript_profile: String,
    /// Human-readable release generation selected by the manifest.
    pub generation: String,
    /// Curve/parity selected by this artifact.
    pub parity: iroha_data_model::offline::KagemushaPastaCycleParityV1,
    /// Exact fixed circuit id for `parity`.
    pub circuit_id: String,
    /// Canonical `ParamsIPA` generation identifier.
    pub parameter_generation: String,
    /// Halo2 IPA domain exponent.
    pub ipa_k: u32,
    /// Kind of the following payload.
    pub kind: iroha_data_model::offline::KagemushaPastaCycleArtifactKindV3,
    /// Exact following payload length.
    pub payload_size_bytes: u64,
    /// SHA-256 of only the following payload.
    pub payload_sha256: [u8; 32],
}

impl KagemushaRecursiveSpendPastaCycleArtifactsV3 {
    /// Validate all small bindings before any release-sized payload is read.
    pub fn validate_header(&self) -> Result<(), String> {
        use iroha_data_model::offline::{
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3,
            KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V1,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1, KagemushaPastaCycleParityV1,
        };

        let expected_circuit = match self.parity {
            KagemushaPastaCycleParityV1::StepEq => KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1,
            KagemushaPastaCycleParityV1::StepEp => KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V1,
        };
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_VERSION_V3
            || self.manifest_schema != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3
            || self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1
            || !iroha_data_model::offline::is_kagemusha_v3_portable_identifier(&self.generation)
            || self.circuit_id != expected_circuit
            || !iroha_data_model::offline::is_kagemusha_v3_portable_identifier(
                &self.parameter_generation,
            )
            || self.ipa_k != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1
            || self.payload_size_bytes == 0
            || self.payload_size_bytes > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3
            || self.payload_sha256 == [0; 32]
        {
            return Err("Kagemusha Pasta-cycle V3 artifact header mismatch".to_owned());
        }
        Ok(())
    }

    /// Bind this decoded header to one exact descriptor in an authenticated manifest.
    pub fn validate_against_manifest(
        &self,
        manifest: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactManifestV3,
        descriptor: &iroha_data_model::offline::KagemushaPastaCycleArtifactV3,
    ) -> Result<(), String> {
        self.validate_header()?;
        manifest.validate().map_err(|error| error.to_string())?;
        let profile = manifest
            .profiles
            .iter()
            .find(|profile| profile.parity == self.parity)
            .ok_or_else(|| "Kagemusha Pasta-cycle V3 manifest parity mismatch".to_owned())?;
        if self.manifest_schema != manifest.schema
            || self.bridge_abi_version != manifest.bridge_abi_version
            || self.proof_backend != manifest.proof_backend
            || self.transcript_profile != manifest.transcript_profile
            || self.generation != manifest.generation
            || self.circuit_id != profile.circuit_id
            || self.parameter_generation != profile.parameter_generation
            || self.ipa_k != profile.ipa_k
            || descriptor.kind != self.kind
            || descriptor.payload_size_bytes != self.payload_size_bytes
            || descriptor.payload_sha256 != self.payload_sha256
            || !profile
                .artifacts
                .iter()
                .any(|artifact| artifact == descriptor)
        {
            return Err("Kagemusha Pasta-cycle V3 artifact manifest binding mismatch".to_owned());
        }
        Ok(())
    }
}

/// Fully authenticated unframed material from one V3 artifact role.
///
/// The caller must keep the underlying installed-generation handle pinned for
/// the complete proof operation. This value authenticates bytes and role; it
/// does not turn proving-key material into a consensus admission input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaValidatedArtifactPayloadV3 {
    /// Canonical role header authenticated by the manifest descriptor.
    header: KagemushaRecursiveSpendPastaCycleArtifactsV3,
    /// Exact unframed payload bytes.
    payload: Vec<u8>,
}

impl KagemushaValidatedArtifactPayloadV3 {
    /// Return the authenticated role header.
    #[must_use]
    pub fn header(&self) -> &KagemushaRecursiveSpendPastaCycleArtifactsV3 {
        &self.header
    }

    /// Return the exact authenticated, unframed payload bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    fn validate_payload(&self) -> Result<(), String> {
        use sha2::{Digest as _, Sha256};

        self.header.validate_header()?;
        if u64::try_from(self.payload.len())
            .ok()
            .is_none_or(|len| len != self.header.payload_size_bytes)
            || <[u8; 32]>::from(Sha256::digest(&self.payload)) != self.header.payload_sha256
        {
            return Err("Kagemusha V3 authenticated artifact payload mismatch".to_owned());
        }
        Ok(())
    }
}

/// Exact four-file verifier material rebound to one authenticated V3 manifest.
///
/// The fields are private so downstream code cannot create a role-confused
/// verifier set. Construction rechecks the payload digests even when the
/// individual files were authenticated earlier at the streaming boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaPastaCycleVerifierArtifactsV3 {
    manifest_sha256: [u8; 32],
    step_eq_parameters: KagemushaValidatedArtifactPayloadV3,
    step_eq_verifying_key: KagemushaValidatedArtifactPayloadV3,
    step_ep_parameters: KagemushaValidatedArtifactPayloadV3,
    step_ep_verifying_key: KagemushaValidatedArtifactPayloadV3,
}

impl KagemushaPastaCycleVerifierArtifactsV3 {
    /// Bind the four exact verifier roles to one authenticated manifest.
    pub fn new(
        manifest: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactManifestV3,
        step_eq_parameters: KagemushaValidatedArtifactPayloadV3,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV3,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV3,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV3,
    ) -> Result<Self, String> {
        use iroha_data_model::offline::{
            KagemushaPastaCycleArtifactKindV3, KagemushaPastaCycleParityV1,
        };
        use sha2::{Digest as _, Sha256};

        manifest.validate().map_err(|error| error.to_string())?;
        let artifacts = [
            (
                &step_eq_parameters,
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV3::Parameters,
            ),
            (
                &step_eq_verifying_key,
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV3::VerifyingKey,
            ),
            (
                &step_ep_parameters,
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV3::Parameters,
            ),
            (
                &step_ep_verifying_key,
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV3::VerifyingKey,
            ),
        ];
        let mut payload_digests = std::collections::BTreeSet::new();
        for (artifact, parity, kind) in artifacts {
            artifact.validate_payload()?;
            if artifact.header.parity != parity || artifact.header.kind != kind {
                return Err("Kagemusha V3 verifier artifact role mismatch".to_owned());
            }
            let descriptor = manifest
                .profiles
                .iter()
                .find(|profile| profile.parity == parity)
                .and_then(|profile| {
                    profile
                        .artifacts
                        .iter()
                        .find(|descriptor| descriptor.kind == kind)
                })
                .ok_or_else(|| "Kagemusha V3 verifier manifest role is absent".to_owned())?;
            artifact
                .header
                .validate_against_manifest(manifest, descriptor)?;
            if !payload_digests.insert(artifact.header.payload_sha256) {
                return Err("Kagemusha V3 verifier artifact payloads are not distinct".to_owned());
            }
        }

        Ok(Self {
            manifest_sha256: Sha256::digest(norito::to_bytes(manifest).map_err(|error| {
                format!("failed to encode Kagemusha V3 artifact manifest: {error}")
            })?)
            .into(),
            step_eq_parameters,
            step_eq_verifying_key,
            step_ep_parameters,
            step_ep_verifying_key,
        })
    }

    /// SHA-256 of the exact manifest that authenticated all four roles.
    #[must_use]
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }

    #[cfg(all(feature = "zk-halo2-ipa", any(test, feature = "bench")))]
    pub(crate) fn step_eq_parameters(&self) -> &[u8] {
        self.step_eq_parameters.payload()
    }

    #[cfg(all(feature = "zk-halo2-ipa", any(test, feature = "bench")))]
    pub(crate) fn step_eq_verifying_key(&self) -> &[u8] {
        self.step_eq_verifying_key.payload()
    }

    #[cfg(all(feature = "zk-halo2-ipa", any(test, feature = "bench")))]
    pub(crate) fn step_ep_parameters(&self) -> &[u8] {
        self.step_ep_parameters.payload()
    }

    #[cfg(all(feature = "zk-halo2-ipa", any(test, feature = "bench")))]
    pub(crate) fn step_ep_verifying_key(&self) -> &[u8] {
        self.step_ep_verifying_key.payload()
    }
}

/// Exact six-file prover material rebound to one authenticated V3 manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaPastaCycleProverArtifactsV3 {
    verifier: KagemushaPastaCycleVerifierArtifactsV3,
    step_eq_proving_key: KagemushaValidatedArtifactPayloadV3,
    step_ep_proving_key: KagemushaValidatedArtifactPayloadV3,
}

impl KagemushaPastaCycleProverArtifactsV3 {
    /// Bind all six exact prover/verifier roles to one authenticated manifest.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        manifest: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactManifestV3,
        step_eq_parameters: KagemushaValidatedArtifactPayloadV3,
        step_eq_proving_key: KagemushaValidatedArtifactPayloadV3,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV3,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV3,
        step_ep_proving_key: KagemushaValidatedArtifactPayloadV3,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV3,
    ) -> Result<Self, String> {
        use iroha_data_model::offline::{
            KagemushaPastaCycleArtifactKindV3, KagemushaPastaCycleParityV1,
        };

        for (artifact, parity) in [
            (&step_eq_proving_key, KagemushaPastaCycleParityV1::StepEq),
            (&step_ep_proving_key, KagemushaPastaCycleParityV1::StepEp),
        ] {
            artifact.validate_payload()?;
            if artifact.header.parity != parity
                || artifact.header.kind != KagemushaPastaCycleArtifactKindV3::ProvingKey
            {
                return Err("Kagemusha V3 prover artifact role mismatch".to_owned());
            }
            let descriptor = manifest
                .profiles
                .iter()
                .find(|profile| profile.parity == parity)
                .and_then(|profile| {
                    profile.artifacts.iter().find(|descriptor| {
                        descriptor.kind == KagemushaPastaCycleArtifactKindV3::ProvingKey
                    })
                })
                .ok_or_else(|| "Kagemusha V3 prover manifest role is absent".to_owned())?;
            artifact
                .header
                .validate_against_manifest(manifest, descriptor)?;
        }
        let verifier = KagemushaPastaCycleVerifierArtifactsV3::new(
            manifest,
            step_eq_parameters,
            step_eq_verifying_key,
            step_ep_parameters,
            step_ep_verifying_key,
        )?;
        let digests = [
            verifier.step_eq_parameters.header.payload_sha256,
            step_eq_proving_key.header.payload_sha256,
            verifier.step_eq_verifying_key.header.payload_sha256,
            verifier.step_ep_parameters.header.payload_sha256,
            step_ep_proving_key.header.payload_sha256,
            verifier.step_ep_verifying_key.header.payload_sha256,
        ];
        if digests
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != digests.len()
        {
            return Err("Kagemusha V3 prover artifact payloads are not distinct".to_owned());
        }
        Ok(Self {
            verifier,
            step_eq_proving_key,
            step_ep_proving_key,
        })
    }

    /// SHA-256 of the exact manifest that authenticated all six roles.
    #[must_use]
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.verifier.manifest_sha256()
    }

    #[cfg(test)]
    pub(crate) fn verifier(&self) -> &KagemushaPastaCycleVerifierArtifactsV3 {
        &self.verifier
    }

    #[cfg(test)]
    pub(crate) fn step_eq_proving_key(&self) -> &[u8] {
        self.step_eq_proving_key.payload()
    }

    #[cfg(test)]
    pub(crate) fn step_ep_proving_key(&self) -> &[u8] {
        self.step_ep_proving_key.payload()
    }
}

/// Read and authenticate one complete framed V3 artifact from a pinned handle.
///
/// Hashes, lengths, canonical header bytes, parity, circuit, parameter
/// generation, and material kind are all checked before the payload is
/// returned. The reader must contain exactly one artifact and no trailing
/// bytes.
pub fn read_kagemusha_pasta_cycle_artifact_v3<R: std::io::Read>(
    reader: &mut R,
    manifest: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactManifestV3,
    descriptor: &iroha_data_model::offline::KagemushaPastaCycleArtifactV3,
) -> Result<KagemushaValidatedArtifactPayloadV3, String> {
    use sha2::{Digest as _, Sha256};

    const MAX_HEADER_BYTES: usize = 64 * 1024;
    manifest.validate().map_err(|error| error.to_string())?;
    descriptor.validate().map_err(|error| error.to_string())?;

    let mut framed_hasher = Sha256::new();
    let mut magic = [0_u8; KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3.len()];
    reader
        .read_exact(&mut magic)
        .map_err(|error| format!("failed to read Kagemusha artifact magic: {error}"))?;
    framed_hasher.update(magic);
    if &magic != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3 {
        return Err("Kagemusha V3 artifact magic mismatch".to_owned());
    }

    let mut header_len_bytes = [0_u8; 4];
    reader
        .read_exact(&mut header_len_bytes)
        .map_err(|error| format!("failed to read Kagemusha artifact header length: {error}"))?;
    framed_hasher.update(header_len_bytes);
    let header_len = usize::try_from(u32::from_le_bytes(header_len_bytes))
        .map_err(|_| "Kagemusha V3 artifact header length does not fit usize".to_owned())?;
    let prefix_len = magic
        .len()
        .checked_add(header_len_bytes.len())
        .and_then(|len| len.checked_add(header_len))
        .ok_or_else(|| "Kagemusha V3 artifact prefix length overflow".to_owned())?;
    if header_len == 0
        || header_len > MAX_HEADER_BYTES
        || u64::try_from(prefix_len)
            .ok()
            .is_none_or(|prefix| prefix >= descriptor.size_bytes)
    {
        return Err("Kagemusha V3 artifact header length is invalid".to_owned());
    }

    let mut header_bytes = vec![0_u8; header_len];
    reader
        .read_exact(&mut header_bytes)
        .map_err(|error| format!("failed to read Kagemusha artifact header: {error}"))?;
    framed_hasher.update(&header_bytes);
    let header: KagemushaRecursiveSpendPastaCycleArtifactsV3 =
        norito::decode_from_bytes(&header_bytes)
            .map_err(|_| "Kagemusha V3 artifact header is malformed".to_owned())?;
    if norito::to_bytes(&header)
        .map_err(|error| format!("failed to re-encode Kagemusha artifact header: {error}"))?
        != header_bytes
    {
        return Err("Kagemusha V3 artifact header is not canonical".to_owned());
    }
    header.validate_against_manifest(manifest, descriptor)?;
    if u64::try_from(prefix_len)
        .ok()
        .and_then(|prefix| prefix.checked_add(header.payload_size_bytes))
        != Some(descriptor.size_bytes)
    {
        return Err("Kagemusha V3 artifact payload length mismatch".to_owned());
    }

    let payload_len = usize::try_from(header.payload_size_bytes)
        .map_err(|_| "Kagemusha V3 artifact payload length does not fit usize".to_owned())?;
    let mut payload = vec![0_u8; payload_len];
    reader
        .read_exact(&mut payload)
        .map_err(|error| format!("failed to read Kagemusha artifact payload: {error}"))?;
    let payload_sha256: [u8; 32] = Sha256::digest(&payload).into();
    framed_hasher.update(&payload);
    let framed_sha256: [u8; 32] = framed_hasher.finalize().into();
    let mut trailing = [0_u8; 1];
    if reader
        .read(&mut trailing)
        .map_err(|error| format!("failed to check Kagemusha artifact trailing bytes: {error}"))?
        != 0
        || payload_sha256 != descriptor.payload_sha256
        || framed_sha256 != descriptor.sha256
    {
        return Err("Kagemusha V3 artifact content digest mismatch".to_owned());
    }
    Ok(KagemushaValidatedArtifactPayloadV3 { header, payload })
}

/// Select the exact verifier-only artifact descriptor for one parity.
///
/// Consensus admission uses this selector and therefore cannot accidentally
/// accept a proving key under a verifier role.
pub fn kagemusha_verifier_artifact_descriptor_v3(
    manifest: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactManifestV3,
    parity: iroha_data_model::offline::KagemushaPastaCycleParityV1,
) -> Result<&iroha_data_model::offline::KagemushaPastaCycleArtifactV3, String> {
    use iroha_data_model::offline::KagemushaPastaCycleArtifactKindV3;

    manifest.validate().map_err(|error| error.to_string())?;
    let profile = manifest
        .profiles
        .iter()
        .find(|profile| profile.parity == parity)
        .ok_or_else(|| "Kagemusha V3 verifier parity is absent".to_owned())?;
    let [_, _, verifier] = profile.artifacts.as_slice() else {
        return Err("Kagemusha V3 verifier profile inventory mismatch".to_owned());
    };
    if verifier.kind != KagemushaPastaCycleArtifactKindV3::VerifyingKey {
        return Err("Kagemusha V3 verifier artifact role mismatch".to_owned());
    }
    Ok(verifier)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ff::Field as _;

    fn artifact_manifest_and_frames() -> (
        iroha_data_model::offline::KagemushaRecursiveSpendArtifactManifestV3,
        Vec<Vec<u8>>,
    ) {
        use iroha_data_model::{
            ChainId,
            asset::AssetDefinitionId,
            domain::DomainId,
            offline::{
                KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3,
                KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3,
                KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1,
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1,
                KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V3,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V3,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V3,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V3,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V3,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V3,
                KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2,
                KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2,
                KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2,
                KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2, KagemushaPastaCycleArtifactKindV3,
                KagemushaPastaCycleArtifactV3, KagemushaPastaCycleParityV1,
                KagemushaPastaCycleProofProfileV1, KagemushaTopUpFinalityRosterArtifactReferenceV2,
            },
        };
        use sha2::{Digest as _, Sha256};

        let generation = "test-release-generation";
        let parameter_generation = "test-params-generation";
        let roles = [
            (
                KagemushaPastaCycleParityV1::StepEq,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1,
                [
                    (
                        KagemushaPastaCycleArtifactKindV3::Parameters,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V3,
                    ),
                    (
                        KagemushaPastaCycleArtifactKindV3::ProvingKey,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V3,
                    ),
                    (
                        KagemushaPastaCycleArtifactKindV3::VerifyingKey,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V3,
                    ),
                ],
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V1,
                [
                    (
                        KagemushaPastaCycleArtifactKindV3::Parameters,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V3,
                    ),
                    (
                        KagemushaPastaCycleArtifactKindV3::ProvingKey,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V3,
                    ),
                    (
                        KagemushaPastaCycleArtifactKindV3::VerifyingKey,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V3,
                    ),
                ],
            ),
        ];
        let mut frames = Vec::with_capacity(6);
        let mut profiles = Vec::with_capacity(2);
        for (parity_index, (parity, circuit_id, role_specs)) in roles.into_iter().enumerate() {
            let mut artifacts = Vec::with_capacity(3);
            for (role_index, (kind, file_name)) in role_specs.into_iter().enumerate() {
                let payload = vec![
                    u8::try_from(1 + parity_index * 3 + role_index)
                        .expect("bounded fixture role");
                    48 + role_index
                ];
                let payload_sha256: [u8; 32] = Sha256::digest(&payload).into();
                let header = KagemushaRecursiveSpendPastaCycleArtifactsV3 {
                    version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_VERSION_V3,
                    manifest_schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3
                        .to_owned(),
                    bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
                    proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1.to_owned(),
                    transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1
                        .to_owned(),
                    generation: generation.to_owned(),
                    parity,
                    circuit_id: circuit_id.to_owned(),
                    parameter_generation: parameter_generation.to_owned(),
                    ipa_k: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
                    kind,
                    payload_size_bytes: u64::try_from(payload.len()).expect("fixture payload"),
                    payload_sha256,
                };
                let header_bytes = norito::to_bytes(&header).expect("encode artifact header");
                let mut frame = Vec::new();
                frame.extend_from_slice(KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3);
                frame.extend_from_slice(
                    &u32::try_from(header_bytes.len())
                        .expect("fixture header")
                        .to_le_bytes(),
                );
                frame.extend_from_slice(&header_bytes);
                frame.extend_from_slice(&payload);
                let descriptor = KagemushaPastaCycleArtifactV3 {
                    kind,
                    file_name: file_name.to_owned(),
                    size_bytes: u64::try_from(frame.len()).expect("fixture frame"),
                    sha256: Sha256::digest(&frame).into(),
                    payload_size_bytes: u64::try_from(payload.len()).expect("fixture payload"),
                    payload_sha256,
                };
                artifacts.push(descriptor);
                frames.push(frame);
            }
            profiles.push(KagemushaPastaCycleProofProfileV1 {
                parity,
                circuit_id: circuit_id.to_owned(),
                parameter_generation: parameter_generation.to_owned(),
                ipa_k: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
                artifacts,
            });
        }
        let manifest = iroha_data_model::offline::KagemushaRecursiveSpendArtifactManifestV3 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1.to_owned(),
            generation: generation.to_owned(),
            source_commit: "1234567890abcdef1234567890abcdef12345678".to_owned(),
            chain_id: ChainId::from("kagemusha-artifact-parser"),
            asset: AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").expect("asset domain"),
                "rose".parse().expect("asset name"),
            ),
            asset_scale: 9,
            activation_height: 1,
            withdrawal_height: 100,
            max_proof_bytes: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3,
            profiles,
            topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactReferenceV2 {
                file_name: KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2.to_owned(),
                size_bytes: 32,
                sha256: [0xA1; 32],
                artifact_generation: generation.to_owned(),
                circuit_id: KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2.to_owned(),
                purpose: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2.to_owned(),
                artifact_type: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2.to_owned(),
                required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
            },
            benchmark_evidence_sha256: [0xB1; 32],
            cryptographic_review_sha256: [0xC1; 32],
            release_attestation_sha256: [0xD1; 32],
        };
        manifest.validate().expect("valid artifact parser fixture");
        (manifest, frames)
    }

    #[test]
    fn pasta_cycle_v3_release_rejects_missing_evidence_and_unsigned_attestation() {
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::offline::{
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V1,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1, KagemushaAuthenticatedReleaseV3,
            KagemushaRecursiveSpendReleaseApprovalRoleV1,
            KagemushaRecursiveSpendReleaseAttestationV1, KagemushaRecursiveSpendReleasePolicyV1,
            KagemushaRecursiveSpendReleaseRolePolicyV1, KagemushaReleaseVerificationError,
            kagemusha_recursive_spend_release_sha256,
        };

        let (mut manifest, _) = artifact_manifest_and_frames();
        let benchmark_evidence = b"signed physical-device measurements";
        let cryptographic_review = b"independent cryptographic review";
        manifest.benchmark_evidence_sha256 =
            kagemusha_recursive_spend_release_sha256(benchmark_evidence);
        manifest.cryptographic_review_sha256 =
            kagemusha_recursive_spend_release_sha256(cryptographic_review);
        let attestation = KagemushaRecursiveSpendReleaseAttestationV1 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            subject: manifest
                .release_attestation_subject()
                .expect("release subject"),
            approvals: Vec::new(),
        };
        manifest.release_attestation_sha256 = kagemusha_recursive_spend_release_sha256(
            &norito::to_bytes(&attestation).expect("unsigned attestation bytes"),
        );
        manifest.validate().expect("release fixture");

        let roles = [
            KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
        ];
        let policy = KagemushaRecursiveSpendReleasePolicyV1 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            policy_id: "test-release-policy".to_owned(),
            roles: roles
                .into_iter()
                .enumerate()
                .map(|(index, role)| KagemushaRecursiveSpendReleaseRolePolicyV1 {
                    role,
                    threshold: 1,
                    authorized_signers: vec![
                        KeyPair::try_from_seed(
                            vec![u8::try_from(index + 1).expect("bounded signer"); 32],
                            Algorithm::Ed25519,
                        )
                        .expect("release authority")
                        .public_key()
                        .clone(),
                    ],
                })
                .collect(),
        };
        policy.validate().expect("release policy");

        assert_eq!(
            KagemushaAuthenticatedReleaseV3::verify(
                &manifest,
                &policy,
                &attestation,
                b"",
                cryptographic_review,
            )
            .expect_err("missing benchmark evidence must reject"),
            KagemushaReleaseVerificationError::EvidenceMismatch {
                role: KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
            }
        );
        assert_eq!(
            KagemushaAuthenticatedReleaseV3::verify(
                &manifest,
                &policy,
                &attestation,
                benchmark_evidence,
                cryptographic_review,
            )
            .expect_err("unsigned release must reject"),
            KagemushaReleaseVerificationError::InvalidAttestation
        );
    }

    fn scalar_bytes(value: u64) -> [u8; 32] {
        let repr = Scalar::from(value).to_repr();
        let mut bytes = [0; 32];
        bytes.copy_from_slice(repr.as_ref());
        bytes
    }

    fn init_statement() -> KagemushaRecursiveSpendPublicStatementV2 {
        use iroha_data_model::{
            ChainId,
            asset::AssetDefinitionId,
            domain::DomainId,
            offline::{
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendArtifactBindingV3, KagemushaRecursiveSpendBranchClaimV2,
                KagemushaRecursiveSpendBranchPathV2, KagemushaRecursiveSpendTopUpAnchorRefV2,
                KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2,
            },
            proof::VerifyingKeyId,
        };

        let chain_id = ChainId::from("kagemusha-v2-statement-binding");
        let asset = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("asset domain"),
            "rose".parse().expect("asset name"),
        );
        let anchor = KagemushaRecursiveSpendTopUpAnchorRefV2 {
            topup_operation_id: [0x41; 32],
            anchor_digest: [0x42; 32],
        };
        KagemushaRecursiveSpendPublicStatementV2 {
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            asset_scale: 9,
            final_root: scalar_bytes(12),
            topup_anchor_refs: vec![anchor],
            proof_step_count: 1,
            peer_hop_count: 0,
            current_note: KagemushaSpendableNoteDescriptorV2 {
                chain_id,
                asset,
                note_commitment: scalar_bytes(31),
                spend_nullifier: scalar_bytes(32),
                amount: KagemushaScaledAmountV2::new(10_750_000_000, 9).expect("amount"),
            },
            branch_claims: vec![KagemushaRecursiveSpendBranchClaimV2 {
                path: KagemushaRecursiveSpendBranchPathV2::root(anchor.anchor_digest)
                    .expect("root path"),
                transition_tags: Vec::new(),
            }],
            transition: None,
            artifact_binding: KagemushaRecursiveSpendArtifactBindingV3 {
                generation: "release-generation-1".to_owned(),
                manifest_sha256: [0x43; 32],
            },
            verifier_key_id: VerifyingKeyId::new(
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1,
            ),
        }
    }

    fn statement_bound_transition(
        statement: &KagemushaRecursiveSpendPublicStatementV2,
    ) -> [Scalar; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS] {
        let mut transition = [Scalar::from(0); KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS];
        fill_common_statement_values(&mut transition, statement, None, [0; 32], [0; 32])
            .expect("common statement values");
        transition[I_ASSET_TAG] = scalar_from_canonical_bytes(
            &super::super::confidential_v2::derive_confidential_asset_tag_v2(
                &statement.asset.to_string(),
            ),
            "asset tag",
        )
        .expect("canonical asset tag");
        transition[I_CHAIN_TAG] = scalar_from_canonical_bytes(
            &super::super::confidential_v2::derive_confidential_chain_tag_v2(
                statement.chain_id.as_str(),
            ),
            "chain tag",
        )
        .expect("canonical chain tag");
        transition
    }

    fn append_statement() -> KagemushaRecursiveSpendPublicStatementV2 {
        use iroha_data_model::offline::{
            KagemushaRecursiveSpendBranchV2, KagemushaRecursiveSpendPeerSplitTransitionV2,
            KagemushaRecursiveSpendTransitionV2, kagemusha_recursive_spend_transition_tag_v2,
        };

        let mut statement = init_statement();
        let binding_digest = [0x61; 32];
        let branch = KagemushaRecursiveSpendBranchV2::Recipient;
        statement.current_note.note_commitment = scalar_bytes(51);
        statement.current_note.spend_nullifier = scalar_bytes(52);
        statement.final_root = scalar_bytes(13);
        statement.proof_step_count = 2;
        statement.peer_hop_count = 1;
        statement.branch_claims[0].path = statement.branch_claims[0]
            .path
            .child(branch)
            .expect("recipient child");
        statement.branch_claims[0].transition_tags =
            kagemusha_recursive_spend_transition_tag_v2(binding_digest)
                .expect("transition tag")
                .to_vec();
        statement.transition = Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(
            KagemushaRecursiveSpendPeerSplitTransitionV2 {
                binding_digest,
                branch,
                recipient_request_digest: [0x62; 32],
                operation_id: [0x63; 32],
                parent_max_proof_step_count: 1,
                parent_max_peer_hop_count: 0,
            },
        ));
        statement
    }

    fn append_statement_bound_transition(
        statement: &KagemushaRecursiveSpendPublicStatementV2,
    ) -> [Scalar; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS] {
        let mut transition = statement_bound_transition(statement);
        let Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(split)) = &statement.transition
        else {
            panic!("append statement has peer split");
        };
        let parent = statement.branch_claims[0]
            .path
            .parent()
            .expect("append parent path");
        transition[I_APPEND_PROFILE] = Scalar::from(1);
        transition[I_BRANCH_CHANGE] = branch_selector(split.branch);
        transition[I_PREVIOUS_PROOF_STEP_COUNT] =
            Scalar::from(u64::from(split.parent_max_proof_step_count));
        transition[I_PREVIOUS_PEER_HOP_COUNT] =
            Scalar::from(u64::from(split.parent_max_peer_hop_count));
        transition[I_PARENT_BRANCH_DEPTH] = Scalar::from(u64::from(parent.depth));
        transition[I_PARENT_BRANCH_PATH_BITS] = Scalar::from(path_bits_as_u64(parent.path_bits));
        write_limb_group(
            &mut transition,
            I_PARENT_BRANCH_LINEAGE_ROOT,
            &parent.lineage_root,
        );
        write_limb_group(&mut transition, I_SPLIT_DIGEST, &split.binding_digest);
        write_limb_group(
            &mut transition,
            I_RECIPIENT_REQUEST_DIGEST,
            &split.recipient_request_digest,
        );
        write_limb_group(&mut transition, I_OPERATION_ID, &split.operation_id);
        transition
    }

    #[test]
    fn recursive_state_vector_layout_is_contiguous_and_exact() {
        use std::collections::BTreeSet;

        let mut next = 0;
        let mut layout_fields = BTreeSet::new();
        for &(field, start, len) in KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_V1 {
            assert_ne!(field, "");
            assert!(
                layout_fields.insert(field),
                "duplicate layout field {field}"
            );
            assert_eq!(start, next, "state-vector field {field} must be contiguous");
            assert!(len > 0, "state-vector field {field} must not be empty");
            next += len;
        }
        assert_eq!(next, KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1);
        assert_eq!(next, 889);
        assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V1, 395);
        let mut statement_fields = BTreeSet::new();
        for &(statement_field, vector_field) in KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_COVERAGE_V1 {
            assert!(statement_fields.insert(statement_field));
            assert!(
                layout_fields.contains(vector_field),
                "continuing field {statement_field} maps to absent slot {vector_field}"
            );
        }
        assert_eq!(statement_fields.len(), 14);
    }

    #[test]
    fn recursive_state_vector_is_exact_and_zero_padded() {
        let statement = init_statement();
        let vector = KagemushaRecursiveSpendStateVectorV1::from_statement(&statement)
            .expect("canonical init state vector");
        assert_eq!(
            vector.limbs[S_VERSION],
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V1
        );
        assert_eq!(vector.limbs[S_TOPUP_ANCHOR_COUNT], 1);
        assert_eq!(vector.limbs[S_BRANCH_CLAIM_COUNT], 1);
        assert_eq!(vector.limbs[S_PROOF_STEP_COUNT], 1);
        assert_eq!(vector.limbs[S_PEER_HOP_COUNT], 0);
        assert!(
            vector.limbs[S_TOPUP_ANCHORS + 16..S_PROOF_STEP_COUNT]
                .iter()
                .all(|limb| *limb == 0)
        );
        let first_history = S_BRANCH_CLAIMS + 11;
        assert!(
            vector.limbs[first_history
                ..S_BRANCH_CLAIMS + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V1]
                .iter()
                .all(|limb| *limb == 0)
        );
        let second_claim = S_BRANCH_CLAIMS + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V1;
        assert!(
            vector.limbs[second_claim..S_ARTIFACT_MANIFEST_SHA256]
                .iter()
                .all(|limb| *limb == 0)
        );

        for &index in &[
            S_VERSION,
            S_CHAIN_TAG,
            S_ASSET_TAG,
            S_ASSET_SCALE,
            S_FINAL_ROOT,
            S_TOPUP_ANCHOR_COUNT,
            S_TOPUP_ANCHORS,
            S_PROOF_STEP_COUNT,
            S_PEER_HOP_COUNT,
            S_CURRENT_COMMITMENT,
            S_CURRENT_NULLIFIER,
            S_CURRENT_AMOUNT,
            S_CURRENT_SCALE,
            S_BRANCH_CLAIM_COUNT,
            S_BRANCH_CLAIMS,
            S_ARTIFACT_MANIFEST_SHA256,
            S_VERIFIER_KEY_ID,
        ] {
            let mut substituted = vector.clone();
            substituted.limbs[index] ^= 1;
            assert!(
                substituted.validate_against_statement(&statement).is_err(),
                "state-vector substitution at limb {index} must reject"
            );
        }
    }

    #[test]
    fn recursive_state_vector_reference_encoding_is_deterministic() {
        use sha2::{Digest as _, Sha256};

        let vector = KagemushaRecursiveSpendStateVectorV1::from_statement(&init_statement())
            .expect("canonical init state vector");
        let bytes = vector
            .limbs
            .iter()
            .flat_map(|limb| limb.to_le_bytes())
            .collect::<Vec<_>>();
        let actual: [u8; 32] = Sha256::digest(bytes).into();
        let repeated = KagemushaRecursiveSpendStateVectorV1::from_statement(&init_statement())
            .expect("repeated canonical init state vector")
            .limbs
            .iter()
            .flat_map(|limb| limb.to_le_bytes())
            .collect::<Vec<_>>();
        assert_ne!(actual, [0; 32]);
        assert_eq!(actual.as_slice(), Sha256::digest(repeated).as_slice());
    }

    fn v3_bound_init_bundle_and_record() -> (
        iroha_data_model::offline::KagemushaRecursiveSpendBundleV2,
        VerifyingKeyRecord,
        VerifyingKeyRecord,
    ) {
        use iroha_data_model::{
            confidential::ConfidentialStatus,
            offline::{
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1,
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V1,
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1, KAGEMUSHA_VERIFIER_NAMESPACE,
                KagemushaPastaCycleProofEnvelopeV1, KagemushaRecursiveSpendBundleV2,
                KagemushaRecursiveSpendProofV2, KagemushaRecursiveSpendStateBoundaryV1,
            },
            proof::{ProofBox, VerifyingKeyBox},
            zk::BackendTag,
        };
        use sha2::{Digest as _, Sha256};

        let statement = init_statement();
        let statement_digest = statement.digest().expect("statement digest");
        let state = KagemushaRecursiveSpendStateVectorV1::from_statement(&statement)
            .expect("exact recursive state");
        let public_inputs =
            crate::zk::kagemusha_recursion_adapter::KagemushaPastaCyclePublicInputsV1 {
                public_statement_digest: bytes_to_exact_u32_limbs(&statement_digest),
                parent_count: 0,
                parent_states: std::array::from_fn(|_| {
                    vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1]
                }),
                result_state: state.limbs.to_vec(),
                manifest_sha256: bytes_to_exact_u32_limbs(
                    &statement.artifact_binding.manifest_sha256,
                ),
                parent_eq_deferred_sha256: [[0; 8]; 2],
                parent_ep_deferred_sha256: [[0; 8]; 2],
            };
        let proof_pair = crate::zk::kagemusha_recursion_adapter::KagemushaPastaCycleProofPairV1 {
            version: crate::zk::kagemusha_recursion_adapter::KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V1,
            proof_step_count: 1,
            public_inputs,
            step_eq_proof_bytes: vec![0x91; 128],
            step_ep_proof_bytes: vec![0x92; 128],
        };
        proof_pair.validate().expect("proof pair");

        let step_eq_key_bytes = vec![0xA5; 96];
        let step_ep_key_bytes = vec![0xB6; 96];
        let step_eq_verifier_key_sha256 = Sha256::digest(&step_eq_key_bytes).into();
        let step_ep_verifier_key_sha256 = Sha256::digest(&step_ep_key_bytes).into();
        let envelope = KagemushaPastaCycleProofEnvelopeV1 {
            version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V1,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1.to_owned(),
            step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1.to_owned(),
            step_ep_circuit_id:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V1
                    .to_owned(),
            artifact_generation: statement.artifact_binding.generation.clone(),
            manifest_sha256: statement.artifact_binding.manifest_sha256,
            step_eq_parameter_generation: "params-generation-1".to_owned(),
            step_ep_parameter_generation: "params-generation-1".to_owned(),
            step_eq_verifier_key_sha256,
            step_ep_verifier_key_sha256,
            state_boundary: KagemushaRecursiveSpendStateBoundaryV1::new(state.limbs.to_vec())
                .expect("valid exact state boundary"),
            proof: ProofBox::new(
                "halo2/ipa".parse().expect("nested proof backend"),
                norito::to_bytes(&proof_pair).expect("proof pair bytes"),
            ),
        };
        envelope.validate().expect("proof envelope");
        let outer_proof = ProofBox::new(
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
                .parse()
                .expect("outer proof backend"),
            norito::to_bytes(&envelope).expect("proof envelope bytes"),
        );
        let bundle = KagemushaRecursiveSpendBundleV2 {
            recursive_proof: KagemushaRecursiveSpendProofV2 {
                verifier_key_id: statement.verifier_key_id.clone(),
                public_statement_digest: statement_digest,
                proof: outer_proof,
            },
            statement,
        };
        bundle.validate_public_binding().unwrap_or_else(|error| {
            panic!(
                "bound bundle (outer proof bytes {}): {error:?}",
                bundle.recursive_proof.proof.bytes.len()
            )
        });

        let step_eq_key = VerifyingKeyBox::new(
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
                .parse()
                .expect("key backend"),
            step_eq_key_bytes,
        );
        let step_ep_key = VerifyingKeyBox::new(
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
                .parse()
                .expect("key backend"),
            step_ep_key_bytes,
        );
        let mut step_eq_record = VerifyingKeyRecord::new_with_owner(
            1,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1,
            None,
            KAGEMUSHA_VERIFIER_NAMESPACE,
            BackendTag::Halo2IpaPasta,
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V3,
            iroha_data_model::offline::kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v3(),
            super::super::hash_vk(&step_eq_key),
        );
        step_eq_record.vk_len = u32::try_from(step_eq_key.bytes.len()).expect("key length");
        step_eq_record.max_proof_bytes =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3;
        step_eq_record.key = Some(step_eq_key);
        step_eq_record.status = ConfidentialStatus::Active;
        let mut step_ep_record = VerifyingKeyRecord::new_with_owner(
            1,
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V1,
            None,
            KAGEMUSHA_VERIFIER_NAMESPACE,
            BackendTag::Halo2IpaPasta,
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V3,
            iroha_data_model::offline::kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v3(),
            super::super::hash_vk(&step_ep_key),
        );
        step_ep_record.vk_len = u32::try_from(step_ep_key.bytes.len()).expect("key length");
        step_ep_record.max_proof_bytes =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3;
        step_ep_record.key = Some(step_ep_key);
        step_ep_record.status = ConfidentialStatus::Active;
        (bundle, step_eq_record, step_ep_record)
    }

    #[test]
    fn v3_envelope_binding_rejects_metadata_and_instance_substitution() {
        let (bundle, step_eq_record, step_ep_record) = v3_bound_init_bundle_and_record();
        ensure_kagemusha_recursive_spend_v2_proof_envelope_binding(
            &bundle,
            &step_eq_record,
            &step_ep_record,
        )
        .expect("canonical V3 binding");

        for mutation in [
            "manifest",
            "eq_vk",
            "ep_vk",
            "eq_circuit",
            "ep_circuit",
            "eq_generation",
            "ep_generation",
            "duplicate_proof",
            "boundary",
            "statement_instance",
            "manifest_instance",
            "trailing",
        ] {
            let mut candidate = bundle.clone();
            let mut envelope: iroha_data_model::offline::KagemushaPastaCycleProofEnvelopeV1 =
                norito::decode_from_bytes(&candidate.recursive_proof.proof.bytes)
                    .expect("decode envelope");
            if mutation == "trailing" {
                candidate.recursive_proof.proof.bytes.push(0);
            } else {
                let mut proof_pair: crate::zk::kagemusha_recursion_adapter::KagemushaPastaCycleProofPairV1 =
                    norito::decode_from_bytes(&envelope.proof.bytes).expect("decode proof pair");
                match mutation {
                    "manifest" => envelope.manifest_sha256[0] ^= 1,
                    "eq_vk" => envelope.step_eq_verifier_key_sha256[0] ^= 1,
                    "ep_vk" => envelope.step_ep_verifier_key_sha256[0] ^= 1,
                    "eq_circuit" => envelope.step_eq_circuit_id.push('x'),
                    "ep_circuit" => envelope.step_ep_circuit_id.push('x'),
                    "eq_generation" => envelope.step_eq_parameter_generation.push('x'),
                    "ep_generation" => envelope.step_ep_parameter_generation.push('x'),
                    "duplicate_proof" => {
                        proof_pair.step_ep_proof_bytes = proof_pair.step_eq_proof_bytes.clone();
                    }
                    "boundary" => envelope.state_boundary.state_limbs[1] ^= 1,
                    "statement_instance" => {
                        proof_pair.public_inputs.public_statement_digest[0] ^= 1
                    }
                    "manifest_instance" => proof_pair.public_inputs.manifest_sha256[0] ^= 1,
                    _ => unreachable!(),
                }
                envelope.proof.bytes = norito::to_bytes(&proof_pair).expect("encode proof pair");
                candidate.recursive_proof.proof.bytes =
                    norito::to_bytes(&envelope).expect("encode envelope");
            }
            assert!(
                ensure_kagemusha_recursive_spend_v2_proof_envelope_binding(
                    &candidate,
                    &step_eq_record,
                    &step_ep_record,
                )
                .is_err(),
                "V3 envelope mutation {mutation} must reject"
            );
        }
    }

    #[test]
    fn maximum_two_parent_pair_envelope_and_bundle_sizes_are_measured() {
        let (mut bundle, _, _) = v3_bound_init_bundle_and_record();
        let mut envelope: iroha_data_model::offline::KagemushaPastaCycleProofEnvelopeV1 =
            norito::decode_from_bytes(&bundle.recursive_proof.proof.bytes)
                .expect("decode canonical paired envelope");
        let mut pair: crate::zk::kagemusha_recursion_adapter::KagemushaPastaCycleProofPairV1 =
            norito::decode_from_bytes(&envelope.proof.bytes).expect("decode canonical proof pair");
        pair.step_eq_proof_bytes = vec![
            0xE1;
            crate::zk::kagemusha_recursion_adapter::KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1
        ];
        pair.step_ep_proof_bytes = vec![
            0xE2;
            crate::zk::kagemusha_recursion_adapter::KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1
        ];
        let pair_bytes = norito::to_bytes(&pair).expect("encode maximum proof pair");
        assert_eq!(
            pair_bytes.len(),
            crate::zk::kagemusha_recursion_adapter::KAGEMUSHA_PASTA_PROOF_PAIR_MAX_BYTES_V1
        );
        envelope.proof.bytes = pair_bytes;
        let envelope_bytes = norito::to_bytes(&envelope).expect("encode maximum envelope");
        eprintln!(
            "Kagemusha maximum pair/envelope bytes: {}/{}",
            envelope.proof.bytes.len(),
            envelope_bytes.len()
        );
        assert_eq!(
            envelope_bytes.len(),
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3
                as usize
        );
        bundle.recursive_proof.proof.bytes = envelope_bytes;
        let bundle_bytes = norito::to_bytes(&bundle).expect("encode maximum init bundle");
        eprintln!(
            "Kagemusha maximum-proof init bundle bytes: {}",
            bundle_bytes.len()
        );
        assert!(
            bundle_bytes.len()
                <= iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2
        );
    }

    fn valid_append_values() -> KagemushaRecursiveSpendTransitionValuesV2 {
        let mut values = KagemushaRecursiveSpendTransitionValuesV2::default();
        let p = &mut values.public;
        p[I_LAYOUT_VERSION] = Scalar::from(KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION);
        p[I_APPEND_PROFILE] = Scalar::from(1);
        p[I_HAS_CHANGE] = Scalar::from(1);
        p[I_PROOF_STEP_COUNT] = Scalar::from(2);
        p[I_PEER_HOP_COUNT] = Scalar::from(1);
        values.peer_hop_selectors[0] = Scalar::from(0);
        values.peer_hop_selectors[1] = Scalar::from(1);
        p[I_PREVIOUS_PROOF_STEP_COUNT] = Scalar::from(1);
        p[I_PREVIOUS_PEER_HOP_COUNT] = Scalar::from(0);
        p[I_BRANCH_DEPTH] = Scalar::from(1);
        p[I_PARENT_BRANCH_DEPTH] = Scalar::from(0);
        p[I_ASSET_SCALE] = Scalar::from(2);
        p[I_INPUT_SCALE] = Scalar::from(2);
        p[I_TRANSFER_SCALE] = Scalar::from(2);
        p[I_RECIPIENT_SCALE] = Scalar::from(2);
        p[I_CHANGE_SCALE] = Scalar::from(2);
        p[I_CURRENT_SCALE] = Scalar::from(2);
        p[I_RECORD_INPUT_COUNT] = Scalar::from(1);
        p[I_TRANSFER_INPUT_COUNT] = Scalar::from(1);
        p[I_RECORD_OUTPUT_COUNT] = Scalar::from(2);
        p[I_TRANSFER_OUTPUT_COUNT] = Scalar::from(2);
        write_amount(p, I_CURRENT_AMOUNT_LO, 40);
        write_amount(p, I_INPUT_AMOUNT_LO, 100);
        write_amount(p, I_TRANSFER_AMOUNT_LO, 40);
        write_amount(p, I_RECIPIENT_AMOUNT_LO, 40);
        write_amount(p, I_CHANGE_AMOUNT_LO, 60);
        p[I_INITIAL_ROOT] = Scalar::from(11);
        p[I_FINAL_ROOT] = Scalar::from(12);
        p[I_PARENT_FINAL_ROOT] = Scalar::from(11);
        p[I_RECORD_ROOT_BEFORE] = Scalar::from(11);
        p[I_RECORD_ROOT_AFTER] = Scalar::from(12);
        p[I_TRANSFER_ROOT] = Scalar::from(12);
        p[I_CURRENT_COMMITMENT] = Scalar::from(31);
        p[I_CURRENT_NULLIFIER] = Scalar::from(32);
        p[I_INPUT_COMMITMENT] = Scalar::from(21);
        p[I_INPUT_NULLIFIER] = Scalar::from(22);
        p[I_RECIPIENT_COMMITMENT] = Scalar::from(31);
        p[I_RECIPIENT_NULLIFIER] = Scalar::from(32);
        p[I_CHANGE_COMMITMENT] = Scalar::from(41);
        p[I_CHANGE_NULLIFIER] = Scalar::from(42);
        p[I_RECORD_INPUT_NULLIFIER_0] = Scalar::from(22);
        p[I_RECORD_OUTPUT_0] = Scalar::from(31);
        p[I_RECORD_OUTPUT_1] = Scalar::from(41);
        p[I_TRANSFER_INPUT_COMMITMENT_0] = Scalar::from(21);
        p[I_TRANSFER_NULLIFIER_0] = Scalar::from(22);
        p[I_TRANSFER_OUTPUT_0] = Scalar::from(31);
        p[I_TRANSFER_OUTPUT_1] = Scalar::from(41);
        p[I_BRANCH_PATH_BITS] = Scalar::from(0);
        p[I_PARENT_BRANCH_PATH_BITS] = Scalar::from(0);
        for limb in 0..4 {
            p[I_BRANCH_LINEAGE_ROOT + limb] = Scalar::from(u64::try_from(limb + 1).unwrap());
            p[I_PARENT_BRANCH_LINEAGE_ROOT + limb] = Scalar::from(u64::try_from(limb + 1).unwrap());
        }
        values.path_depth_selectors[0] = Scalar::from(1);
        values
    }

    fn valid_redeem_change_values() -> KagemushaRecursiveSpendTransitionValuesV2 {
        let mut values = KagemushaRecursiveSpendTransitionValuesV2::default();
        let p = &mut values.public;
        p[I_LAYOUT_VERSION] = Scalar::from(KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION);
        p[I_REDEMPTION_PROFILE] = Scalar::from(1);
        p[I_BRANCH_CHANGE] = Scalar::from(1);
        p[I_HAS_CHANGE] = Scalar::from(1);
        p[I_PROOF_STEP_COUNT] = Scalar::from(2);
        p[I_PEER_HOP_COUNT] = Scalar::from(3);
        values.peer_hop_selectors[0] = Scalar::from(0);
        values.peer_hop_selectors[3] = Scalar::from(1);
        p[I_PREVIOUS_PROOF_STEP_COUNT] = Scalar::from(1);
        p[I_PREVIOUS_PEER_HOP_COUNT] = Scalar::from(3);
        p[I_BRANCH_DEPTH] = Scalar::from(4);
        p[I_PARENT_BRANCH_DEPTH] = Scalar::from(3);
        p[I_BRANCH_PATH_BITS] = Scalar::from(1u64 << 60);
        p[I_PARENT_BRANCH_PATH_BITS] = Scalar::from(0);
        p[I_ASSET_SCALE] = Scalar::from(2);
        p[I_INPUT_SCALE] = Scalar::from(2);
        p[I_TRANSFER_SCALE] = Scalar::from(2);
        p[I_RECIPIENT_SCALE] = Scalar::from(2);
        p[I_CHANGE_SCALE] = Scalar::from(2);
        p[I_CURRENT_SCALE] = Scalar::from(2);
        p[I_RECORD_INPUT_COUNT] = Scalar::from(1);
        p[I_TRANSFER_INPUT_COUNT] = Scalar::from(1);
        p[I_RECORD_OUTPUT_COUNT] = Scalar::from(1);
        p[I_TRANSFER_OUTPUT_COUNT] = Scalar::from(1);
        write_amount(p, I_CURRENT_AMOUNT_LO, 60);
        write_amount(p, I_INPUT_AMOUNT_LO, 100);
        write_amount(p, I_TRANSFER_AMOUNT_LO, 40);
        write_amount(p, I_RECIPIENT_AMOUNT_LO, 40);
        write_amount(p, I_CHANGE_AMOUNT_LO, 60);
        p[I_UNSHIELD_PUBLIC_AMOUNT] = Scalar::from(40);
        p[I_INITIAL_ROOT] = Scalar::from(7);
        p[I_PARENT_FINAL_ROOT] = Scalar::from(11);
        p[I_FINAL_ROOT] = Scalar::from(12);
        p[I_RECORD_ROOT_BEFORE] = Scalar::from(11);
        p[I_RECORD_ROOT_AFTER] = Scalar::from(12);
        p[I_TRANSFER_ROOT] = Scalar::from(11);
        p[I_CURRENT_COMMITMENT] = Scalar::from(41);
        p[I_CURRENT_NULLIFIER] = Scalar::from(42);
        p[I_INPUT_COMMITMENT] = Scalar::from(21);
        p[I_INPUT_NULLIFIER] = Scalar::from(22);
        p[I_CHANGE_COMMITMENT] = Scalar::from(41);
        p[I_CHANGE_NULLIFIER] = Scalar::from(42);
        p[I_RECORD_INPUT_NULLIFIER_0] = Scalar::from(22);
        p[I_RECORD_OUTPUT_0] = Scalar::from(41);
        p[I_TRANSFER_INPUT_COMMITMENT_0] = Scalar::from(21);
        p[I_TRANSFER_NULLIFIER_0] = Scalar::from(22);
        p[I_TRANSFER_OUTPUT_0] = Scalar::from(41);
        for limb in 0..4 {
            p[I_BRANCH_LINEAGE_ROOT + limb] = Scalar::from(u64::try_from(limb + 1).unwrap());
            p[I_PARENT_BRANCH_LINEAGE_ROOT + limb] = Scalar::from(u64::try_from(limb + 1).unwrap());
            p[I_TOPUP_RECEIPT_DIGEST + limb] = Scalar::from(u64::try_from(limb + 5).unwrap());
            p[I_PARENT_TOPUP_RECEIPT_DIGEST + limb] =
                Scalar::from(u64::try_from(limb + 5).unwrap());
        }
        values.path_depth_selectors[3] = Scalar::from(1);
        values
    }

    fn as_step_ep_values(
        values: &KagemushaRecursiveSpendTransitionValuesV2,
    ) -> KagemushaRecursiveSpendTransitionValuesV2<Fq> {
        let convert = |value: Scalar| {
            let source = value.to_repr();
            let mut target = <Fq as PrimeField>::Repr::default();
            target.as_mut().copy_from_slice(source.as_ref());
            Option::<Fq>::from(Fq::from_repr(target))
                .expect("all field-neutral transition fixture values fit both Pasta fields")
        };
        KagemushaRecursiveSpendTransitionValuesV2 {
            public: values.public.map(convert),
            amount_low_carry: convert(values.amount_low_carry),
            path_depth_selectors: values.path_depth_selectors.map(convert),
            peer_hop_selectors: values.peer_hop_selectors.map(convert),
        }
    }

    #[test]
    fn envelope_statement_binding_rejects_every_mutable_common_row() {
        let statement = init_statement();
        let transition = statement_bound_transition(&statement);
        ensure_transition_statement_binding(&statement, &transition)
            .expect("canonical statement rows");

        let mut statement_bound_rows = vec![
            I_LAYOUT_VERSION,
            I_PROOF_STEP_COUNT,
            I_PEER_HOP_COUNT,
            I_BRANCH_DEPTH,
            I_ASSET_SCALE,
            I_CURRENT_SCALE,
            I_CURRENT_AMOUNT_LO,
            I_CURRENT_AMOUNT_HI,
            I_BRANCH_PATH_BITS,
            I_FINAL_ROOT,
            I_CURRENT_COMMITMENT,
            I_CURRENT_NULLIFIER,
            I_ASSET_TAG,
            I_CHAIN_TAG,
            I_TOPUP_ANCHOR_COUNT,
        ];
        for start in [
            I_STATEMENT_DIGEST,
            I_BRANCH_LINEAGE_ROOT,
            I_CHAIN_ID_DIGEST,
            I_ASSET_ID_DIGEST,
            I_TOPUP_OPERATION_ID,
            I_ARTIFACT_MANIFEST_SHA256,
            I_TOPUP_ANCHOR_DIGEST,
            I_VERIFIER_KEY_ID_DIGEST,
        ] {
            statement_bound_rows.extend(start..start + 4);
        }

        for row in statement_bound_rows {
            let mut tampered = transition;
            tampered[row] += Scalar::from(1);
            assert!(
                ensure_transition_statement_binding(&statement, &tampered).is_err(),
                "statement-derived row {row} must not be independently malleable"
            );
        }
    }

    #[test]
    fn envelope_statement_binding_rejects_init_transition_smuggling() {
        let statement = init_statement();
        let transition = statement_bound_transition(&statement);
        for row in [
            I_APPEND_PROFILE,
            I_REDEMPTION_PROFILE,
            I_BRANCH_CHANGE,
            I_HAS_CHANGE,
        ] {
            let mut tampered = transition;
            tampered[row] = Scalar::from(1);
            assert!(
                ensure_transition_statement_binding(&statement, &tampered).is_err(),
                "init profile row {row} must be canonical zero"
            );
        }
        for start in [
            I_SPLIT_DIGEST,
            I_RECIPIENT_REQUEST_DIGEST,
            I_OPERATION_ID,
            I_PARENT_BUNDLE_DIGEST,
        ] {
            let mut tampered = transition;
            tampered[start] = Scalar::from(1);
            assert!(
                ensure_transition_statement_binding(&statement, &tampered).is_err(),
                "init-only proof must reject transition metadata at row {start}"
            );
        }
    }

    #[test]
    fn envelope_statement_binding_rejects_peer_profile_and_branch_substitution() {
        let statement = append_statement();
        let transition = append_statement_bound_transition(&statement);
        ensure_transition_statement_binding(&statement, &transition)
            .expect("canonical append statement rows");

        for row in [
            I_APPEND_PROFILE,
            I_REDEMPTION_PROFILE,
            I_BRANCH_CHANGE,
            I_PREVIOUS_PROOF_STEP_COUNT,
            I_PREVIOUS_PEER_HOP_COUNT,
            I_PARENT_BRANCH_DEPTH,
            I_PARENT_BRANCH_PATH_BITS,
        ] {
            let mut tampered = transition;
            tampered[row] += Scalar::from(1);
            assert!(
                ensure_transition_statement_binding(&statement, &tampered).is_err(),
                "peer transition row {row} must match the statement"
            );
        }
        for start in [
            I_SPLIT_DIGEST,
            I_RECIPIENT_REQUEST_DIGEST,
            I_OPERATION_ID,
            I_PARENT_BRANCH_LINEAGE_ROOT,
        ] {
            let mut tampered = transition;
            tampered[start] += Scalar::from(1);
            assert!(
                ensure_transition_statement_binding(&statement, &tampered).is_err(),
                "peer transition limb group at {start} must match the statement"
            );
        }

        let mut wrong_path_statement = statement.clone();
        let parent = wrong_path_statement.branch_claims[0]
            .path
            .parent()
            .expect("parent path");
        wrong_path_statement.branch_claims[0].path = parent
            .child(KagemushaRecursiveSpendBranchV2::Change)
            .expect("change child");
        let wrong_path_transition = append_statement_bound_transition(&wrong_path_statement);
        assert!(
            ensure_transition_statement_binding(&wrong_path_statement, &wrong_path_transition)
                .is_err(),
            "a recipient transition must not claim the sender-change path"
        );
    }

    #[test]
    fn transition_relation_accepts_conserving_recipient_branch() {
        let values = valid_append_values();
        let instances = kagemusha_recursive_spend_transition_instance_columns_v2(&values);
        let prover = halo2_proofs::dev::MockProver::run(
            12,
            &KagemushaRecursiveSpendTransitionCircuitV2 { values },
            instances,
        )
        .expect("mock prover");
        prover.assert_satisfied();
    }

    #[test]
    fn transition_relation_rejects_mutation_at_every_public_trace_boundary() {
        let values = valid_append_values();
        let canonical = kagemusha_recursive_spend_transition_instance_columns_v2(&values);
        assert_eq!(
            canonical.len(),
            KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS
        );
        assert_eq!(
            canonical[0].len(),
            KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_TRACE_ROWS
        );
        assert!(
            canonical
                .iter()
                .all(|column| column.len() == canonical[0].len())
        );
        assert_eq!(
            canonical.iter().map(Vec::len).sum::<usize>(),
            KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_CELLS
        );

        let last = canonical[0].len() - 1;
        for column in 0..canonical.len() {
            for row in [0, canonical[0].len() / 2, last] {
                let mut tampered = canonical.clone();
                tampered[column][row] += Scalar::ONE;
                let prover = halo2_proofs::dev::MockProver::run(
                    12,
                    &KagemushaRecursiveSpendTransitionCircuitV2 {
                        values: values.clone(),
                    },
                    tampered,
                )
                .expect("mutated trace remains a well-shaped prover input");
                assert!(
                    prover.verify().is_err(),
                    "public trace mutation at column {column}, row {row} must fail"
                );
            }
        }
    }

    #[test]
    fn transition_relation_rejects_adversarial_semantic_substitutions() {
        type Mutation = fn(&mut KagemushaRecursiveSpendTransitionValuesV2);
        let mutations: [(&str, Mutation); 12] = [
            ("layout version", |values| {
                values.public[I_LAYOUT_VERSION] += Scalar::ONE;
            }),
            ("non-boolean profile", |values| {
                values.public[I_APPEND_PROFILE] = Scalar::from(2);
            }),
            ("mutually enabled profiles", |values| {
                values.public[I_REDEMPTION_PROFILE] = Scalar::ONE;
            }),
            ("change branch without change", |values| {
                values.public[I_BRANCH_CHANGE] = Scalar::ONE;
                values.public[I_HAS_CHANGE] = Scalar::ZERO;
            }),
            ("non-boolean carry", |values| {
                values.amount_low_carry = Scalar::from(2);
            }),
            ("duplicate path selector", |values| {
                values.path_depth_selectors[1] = Scalar::ONE;
            }),
            ("duplicate peer-hop selector", |values| {
                values.peer_hop_selectors[2] = Scalar::ONE;
            }),
            ("skipped proof step", |values| {
                values.public[I_PROOF_STEP_COUNT] += Scalar::ONE;
            }),
            ("wrong branch-path bit", |values| {
                values.public[I_BRANCH_PATH_BITS] = Scalar::from(1_u64 << 63);
            }),
            ("asset-scale substitution", |values| {
                values.public[I_CURRENT_SCALE] += Scalar::ONE;
            }),
            ("record-output substitution", |values| {
                values.public[I_RECORD_OUTPUT_0] += Scalar::ONE;
            }),
            ("parent-lineage substitution", |values| {
                values.public[I_PARENT_BRANCH_LINEAGE_ROOT] += Scalar::ONE;
            }),
        ];

        for (name, mutate) in mutations {
            let mut values = valid_append_values();
            mutate(&mut values);
            let instances = kagemusha_recursive_spend_transition_instance_columns_v2(&values);
            let prover = halo2_proofs::dev::MockProver::run(
                12,
                &KagemushaRecursiveSpendTransitionCircuitV2 { values },
                instances,
            )
            .expect("adversarial transition remains a well-shaped prover input");
            assert!(
                prover.verify().is_err(),
                "semantic substitution `{name}` must fail the streaming relation"
            );
        }
    }

    #[test]
    fn transition_relation_is_identical_on_both_pasta_step_parities() {
        let eq_values = valid_append_values();
        let ep_values = as_step_ep_values(&eq_values);
        let instances = kagemusha_recursive_spend_transition_instance_columns_v2(&ep_values);
        let prover = halo2_proofs::dev::MockProver::run(
            12,
            &KagemushaRecursiveSpendTransitionEpCircuitV2 { values: ep_values },
            instances,
        )
        .expect("StepEp mock prover");
        prover.assert_satisfied();

        let mut non_conserving = as_step_ep_values(&eq_values);
        non_conserving.public[I_CHANGE_AMOUNT_LO] += Fq::ONE;
        let instances = kagemusha_recursive_spend_transition_instance_columns_v2(&non_conserving);
        let prover = halo2_proofs::dev::MockProver::run(
            12,
            &KagemushaRecursiveSpendTransitionEpCircuitV2 {
                values: non_conserving,
            },
            instances,
        )
        .expect("StepEp adversarial mock prover");
        assert!(prover.verify().is_err());
    }

    #[test]
    fn transition_relation_rejects_non_conservation() {
        let mut values = valid_append_values();
        write_amount(&mut values.public, I_CHANGE_AMOUNT_LO, 61);
        let instances = kagemusha_recursive_spend_transition_instance_columns_v2(&values);
        let prover = halo2_proofs::dev::MockProver::run(
            12,
            &KagemushaRecursiveSpendTransitionCircuitV2 { values },
            instances,
        )
        .expect("mock prover");
        assert!(prover.verify().is_err());
    }

    #[test]
    fn transition_relation_accepts_low_limb_carry() {
        let mut values = valid_append_values();
        write_amount(
            &mut values.public,
            I_CURRENT_AMOUNT_LO,
            u128::from(u64::MAX),
        );
        write_amount(&mut values.public, I_INPUT_AMOUNT_LO, 1u128 << 64);
        write_amount(
            &mut values.public,
            I_TRANSFER_AMOUNT_LO,
            u128::from(u64::MAX),
        );
        write_amount(
            &mut values.public,
            I_RECIPIENT_AMOUNT_LO,
            u128::from(u64::MAX),
        );
        write_amount(&mut values.public, I_CHANGE_AMOUNT_LO, 1);
        values.amount_low_carry = Scalar::from(1);
        let instances = kagemusha_recursive_spend_transition_instance_columns_v2(&values);
        let prover = halo2_proofs::dev::MockProver::run(
            12,
            &KagemushaRecursiveSpendTransitionCircuitV2 { values },
            instances,
        )
        .expect("mock prover");
        prover.assert_satisfied();
    }

    #[test]
    fn transition_relation_binds_sibling_branch_and_path() {
        let mut values = valid_append_values();
        values.public[I_BRANCH_CHANGE] = Scalar::from(1);
        values.public[I_CURRENT_COMMITMENT] = Scalar::from(41);
        values.public[I_CURRENT_NULLIFIER] = Scalar::from(42);
        write_amount(&mut values.public, I_CURRENT_AMOUNT_LO, 60);
        values.public[I_BRANCH_PATH_BITS] = Scalar::from(1u64 << 63);
        let instances = kagemusha_recursive_spend_transition_instance_columns_v2(&values);
        let prover = halo2_proofs::dev::MockProver::run(
            12,
            &KagemushaRecursiveSpendTransitionCircuitV2 { values },
            instances,
        )
        .expect("mock prover");
        prover.assert_satisfied();
    }

    #[test]
    fn transition_relation_accepts_partial_redemption_change() {
        let values = valid_redeem_change_values();
        let instances = kagemusha_recursive_spend_transition_instance_columns_v2(&values);
        let prover = halo2_proofs::dev::MockProver::run(
            12,
            &KagemushaRecursiveSpendTransitionCircuitV2 { values },
            instances,
        )
        .expect("mock prover");
        prover.assert_satisfied();
    }

    #[test]
    fn transition_relation_rejects_unbound_redemption_credit() {
        let mut values = valid_redeem_change_values();
        values.public[I_UNSHIELD_PUBLIC_AMOUNT] = Scalar::from(41);
        let instances = kagemusha_recursive_spend_transition_instance_columns_v2(&values);
        let prover = halo2_proofs::dev::MockProver::run(
            12,
            &KagemushaRecursiveSpendTransitionCircuitV2 { values },
            instances,
        )
        .expect("mock prover");
        assert!(prover.verify().is_err());
    }

    #[test]
    fn transition_relation_rejects_second_redemption_input() {
        let mut values = valid_redeem_change_values();
        values.public[I_TRANSFER_INPUT_COMMITMENT_1] = Scalar::from(99);
        values.public[I_TRANSFER_NULLIFIER_1] = Scalar::from(100);
        let instances = kagemusha_recursive_spend_transition_instance_columns_v2(&values);
        let prover = halo2_proofs::dev::MockProver::run(
            12,
            &KagemushaRecursiveSpendTransitionCircuitV2 { values },
            instances,
        )
        .expect("mock prover");
        assert!(prover.verify().is_err());
    }

    #[test]
    fn transition_relation_enforces_eight_peer_hops_independently_of_branch_depth() {
        let mut at_limit = valid_append_values();
        at_limit.public[I_PEER_HOP_COUNT] = Scalar::from(8);
        at_limit.public[I_PREVIOUS_PEER_HOP_COUNT] = Scalar::from(7);
        at_limit.public[I_BRANCH_DEPTH] = Scalar::from(8);
        at_limit.public[I_PARENT_BRANCH_DEPTH] = Scalar::from(7);
        at_limit.peer_hop_selectors[1] = Scalar::from(0);
        at_limit.peer_hop_selectors[8] = Scalar::from(1);
        at_limit.path_depth_selectors[0] = Scalar::from(0);
        at_limit.path_depth_selectors[7] = Scalar::from(1);
        let instances = kagemusha_recursive_spend_transition_instance_columns_v2(&at_limit);
        let prover = halo2_proofs::dev::MockProver::run(
            12,
            &KagemushaRecursiveSpendTransitionCircuitV2 { values: at_limit },
            instances,
        )
        .expect("mock prover at peer-hop limit");
        prover.assert_satisfied();

        let mut above_limit = valid_append_values();
        above_limit.public[I_PEER_HOP_COUNT] = Scalar::from(9);
        above_limit.public[I_PREVIOUS_PEER_HOP_COUNT] = Scalar::from(8);
        let instances = kagemusha_recursive_spend_transition_instance_columns_v2(&above_limit);
        let prover = halo2_proofs::dev::MockProver::run(
            12,
            &KagemushaRecursiveSpendTransitionCircuitV2 {
                values: above_limit,
            },
            instances,
        )
        .expect("mock prover above peer-hop limit");
        assert!(
            prover.verify().is_err(),
            "a ninth peer hop must fail the streaming relation"
        );
    }

    #[test]
    fn pasta_cycle_v3_artifact_header_binds_parity_and_release_limits() {
        let header = KagemushaRecursiveSpendPastaCycleArtifactsV3 {
            version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_VERSION_V3,
            manifest_schema:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3
                    .to_owned(),
            bridge_abi_version:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
            proof_backend:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
                    .to_owned(),
            transcript_profile:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1
                    .to_owned(),
            generation: "release-generation-1".to_owned(),
            parity: iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
            circuit_id: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1
                .to_owned(),
            parameter_generation: "params-generation-1".to_owned(),
            ipa_k: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
            kind: iroha_data_model::offline::KagemushaPastaCycleArtifactKindV3::ProvingKey,
            payload_size_bytes: 1_024,
            payload_sha256: [0x52; 32],
        };
        header.validate_header().expect("valid V3 header");
        assert_eq!(
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3,
            b"KRV3KEY\0"
        );

        let mut wrong_parity = header.clone();
        wrong_parity.parity = iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp;
        assert!(wrong_parity.validate_header().is_err());

        let mut oversized = header;
        oversized.payload_size_bytes =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3 + 1;
        assert!(oversized.validate_header().is_err());
    }

    #[test]
    fn pasta_cycle_v3_artifact_reader_binds_every_role_and_rejects_corruption() {
        use std::io::Cursor;

        let (manifest, frames) = artifact_manifest_and_frames();
        let descriptors = manifest
            .profiles
            .iter()
            .flat_map(|profile| profile.artifacts.iter())
            .collect::<Vec<_>>();
        for (frame, descriptor) in frames.iter().zip(&descriptors) {
            let parsed = read_kagemusha_pasta_cycle_artifact_v3(
                &mut Cursor::new(frame),
                &manifest,
                descriptor,
            )
            .expect("authenticated artifact");
            assert_eq!(parsed.header().kind, descriptor.kind);
            assert_eq!(parsed.header().payload_sha256, descriptor.payload_sha256);
        }

        for parity in [
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
        ] {
            let descriptor = kagemusha_verifier_artifact_descriptor_v3(&manifest, parity)
                .expect("verifier descriptor");
            assert_eq!(
                descriptor.kind,
                iroha_data_model::offline::KagemushaPastaCycleArtifactKindV3::VerifyingKey
            );
        }

        let descriptor = descriptors[2];
        let frame = &frames[2];
        for mutation in ["truncated", "trailing", "payload", "role_replay"] {
            let mut candidate = frame.clone();
            let selected_descriptor = match mutation {
                "truncated" => {
                    candidate.pop();
                    descriptor
                }
                "trailing" => {
                    candidate.push(0);
                    descriptor
                }
                "payload" => {
                    *candidate.last_mut().expect("payload byte") ^= 1;
                    descriptor
                }
                "role_replay" => descriptors[1],
                _ => unreachable!(),
            };
            assert!(
                read_kagemusha_pasta_cycle_artifact_v3(
                    &mut Cursor::new(candidate),
                    &manifest,
                    selected_descriptor,
                )
                .is_err(),
                "artifact mutation {mutation} must reject"
            );
        }
    }

    #[test]
    fn pasta_cycle_authenticated_sets_reject_role_manifest_and_payload_substitution() {
        use std::io::Cursor;

        use sha2::{Digest as _, Sha256};

        let (manifest, frames) = artifact_manifest_and_frames();
        let descriptors = manifest
            .profiles
            .iter()
            .flat_map(|profile| profile.artifacts.iter())
            .collect::<Vec<_>>();
        let artifacts = frames
            .iter()
            .zip(&descriptors)
            .map(|(frame, descriptor)| {
                read_kagemusha_pasta_cycle_artifact_v3(
                    &mut Cursor::new(frame),
                    &manifest,
                    descriptor,
                )
                .expect("authenticated role")
            })
            .collect::<Vec<_>>();
        let verifier = KagemushaPastaCycleVerifierArtifactsV3::new(
            &manifest,
            artifacts[0].clone(),
            artifacts[2].clone(),
            artifacts[3].clone(),
            artifacts[5].clone(),
        )
        .expect("exact verifier roles");
        let manifest_sha256: [u8; 32] = Sha256::digest(
            norito::to_bytes(&manifest).expect("canonical manifest for authenticated set"),
        )
        .into();
        assert_eq!(verifier.manifest_sha256(), manifest_sha256);
        KagemushaPastaCycleProverArtifactsV3::new(
            &manifest,
            artifacts[0].clone(),
            artifacts[1].clone(),
            artifacts[2].clone(),
            artifacts[3].clone(),
            artifacts[4].clone(),
            artifacts[5].clone(),
        )
        .expect("exact prover roles");

        assert!(
            KagemushaPastaCycleVerifierArtifactsV3::new(
                &manifest,
                artifacts[3].clone(),
                artifacts[2].clone(),
                artifacts[0].clone(),
                artifacts[5].clone(),
            )
            .is_err(),
            "Eq/Ep parameter substitution must reject"
        );
        assert!(
            KagemushaPastaCycleProverArtifactsV3::new(
                &manifest,
                artifacts[0].clone(),
                artifacts[2].clone(),
                artifacts[1].clone(),
                artifacts[3].clone(),
                artifacts[4].clone(),
                artifacts[5].clone(),
            )
            .is_err(),
            "proving/verifying role substitution must reject"
        );

        let mut corrupted_payload = artifacts[0].clone();
        corrupted_payload.payload[0] ^= 1;
        assert!(
            KagemushaPastaCycleVerifierArtifactsV3::new(
                &manifest,
                corrupted_payload,
                artifacts[2].clone(),
                artifacts[3].clone(),
                artifacts[5].clone(),
            )
            .is_err(),
            "post-authentication payload mutation must reject"
        );

        let mut other_manifest = manifest.clone();
        other_manifest.generation = "other-release-generation".to_owned();
        other_manifest
            .topup_finality_roster_artifact
            .artifact_generation = other_manifest.generation.clone();
        other_manifest
            .validate()
            .expect("well-formed other manifest");
        assert!(
            KagemushaPastaCycleVerifierArtifactsV3::new(
                &other_manifest,
                artifacts[0].clone(),
                artifacts[2].clone(),
                artifacts[3].clone(),
                artifacts[5].clone(),
            )
            .is_err(),
            "roles authenticated by another manifest must reject"
        );
    }

    fn output_membership_path(
        commitments: &[[u8; 32]],
        leaf_index: usize,
    ) -> iroha_data_model::offline::KagemushaConfidentialMerklePathV2 {
        let path = super::super::confidential_v2::compute_confidential_merkle_path_v3(
            commitments,
            leaf_index,
        )
        .expect("canonical output-membership path");
        let (siblings, directions, _witness_nodes, root) = path.into_parts();
        iroha_data_model::offline::KagemushaConfidentialMerklePathV2 {
            siblings,
            directions,
            root,
        }
    }

    fn output_membership_fixture(
        operation: KagemushaOutputMembershipOperationV3,
        include_change: bool,
    ) -> KagemushaOutputMembershipWitnessV3 {
        let mut commitments = match operation {
            KagemushaOutputMembershipOperationV3::Init => Vec::new(),
            KagemushaOutputMembershipOperationV3::Split
            | KagemushaOutputMembershipOperationV3::RedemptionChange => {
                vec![scalar_bytes(700)]
            }
        };
        let initial_root =
            super::super::confidential_v2::compute_confidential_root_v3(&commitments)
                .expect("initial confidential root");

        let mut recipient = None;
        if !matches!(
            operation,
            KagemushaOutputMembershipOperationV3::RedemptionChange
        ) {
            let leaf_index = u32::try_from(commitments.len()).expect("bounded recipient index");
            let update_path = output_membership_path(
                &commitments,
                usize::try_from(leaf_index).expect("recipient index fits usize"),
            );
            let commitment = scalar_bytes(701);
            commitments.push(commitment);
            recipient = Some((commitment, leaf_index, update_path));
        }

        let mut change = None;
        if include_change {
            let leaf_index = u32::try_from(commitments.len()).expect("bounded change index");
            let update_path = output_membership_path(
                &commitments,
                usize::try_from(leaf_index).expect("change index fits usize"),
            );
            let commitment = scalar_bytes(702);
            commitments.push(commitment);
            change = Some((commitment, leaf_index, update_path));
        }

        let final_root = super::super::confidential_v2::compute_confidential_root_v3(&commitments)
            .expect("final confidential root");
        let recipient = recipient.map(|(commitment, leaf_index, update_path)| {
            KagemushaOutputMembershipLeafV3 {
                commitment,
                leaf_index,
                update_path,
                membership_path: output_membership_path(
                    &commitments,
                    usize::try_from(leaf_index).expect("recipient index fits usize"),
                ),
            }
        });
        let change =
            change.map(
                |(commitment, leaf_index, update_path)| KagemushaOutputMembershipLeafV3 {
                    commitment,
                    leaf_index,
                    update_path,
                    membership_path: output_membership_path(
                        &commitments,
                        usize::try_from(leaf_index).expect("change index fits usize"),
                    ),
                },
            );
        let dummy_leaf_index = u32::try_from(commitments.len()).expect("bounded dummy index");
        let dummy_path = output_membership_path(
            &commitments,
            usize::try_from(dummy_leaf_index).expect("dummy index fits usize"),
        );
        KagemushaOutputMembershipWitnessV3 {
            operation,
            initial_root,
            final_root,
            recipient,
            change,
            dummy_leaf_index,
            dummy_path,
        }
    }

    fn assert_output_membership_satisfied(witness: KagemushaOutputMembershipWitnessV3) {
        let circuit =
            KagemushaOutputMembershipCircuitV3::new(witness).expect("encoded membership witness");
        let instances = circuit.public_instances().expect("membership instances");
        let prover = halo2_proofs::dev::MockProver::run(
            KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V3,
            &circuit,
            instances,
        )
        .expect("output-membership mock prover");
        prover.assert_satisfied();
    }

    fn assert_output_membership_rejected(witness: KagemushaOutputMembershipWitnessV3) {
        let circuit =
            KagemushaOutputMembershipCircuitV3::new(witness).expect("encoded membership witness");
        let instances = circuit.public_instances().expect("membership instances");
        let prover = halo2_proofs::dev::MockProver::run(
            KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V3,
            &circuit,
            instances,
        )
        .expect("well-shaped adversarial membership circuit");
        assert!(
            prover.verify().is_err(),
            "adversarial output membership must not satisfy the circuit"
        );
    }

    fn bump_scalar_bytes(bytes: [u8; 32]) -> [u8; 32] {
        let value = super::super::confidential_v2::scalar_from_repr(bytes)
            .expect("fixture scalar encoding");
        let repr = (value + Scalar::ONE).to_repr();
        let mut bytes = [0; 32];
        bytes.copy_from_slice(repr.as_ref());
        bytes
    }

    #[test]
    fn output_membership_poseidon_update_accepts_every_operation_shape() {
        for witness in [
            output_membership_fixture(KagemushaOutputMembershipOperationV3::Init, false),
            output_membership_fixture(KagemushaOutputMembershipOperationV3::Split, false),
            output_membership_fixture(KagemushaOutputMembershipOperationV3::Split, true),
            output_membership_fixture(KagemushaOutputMembershipOperationV3::RedemptionChange, true),
        ] {
            assert_output_membership_satisfied(witness);
        }
    }

    #[test]
    fn output_membership_poseidon_update_rejects_public_substitution() {
        let witness = output_membership_fixture(KagemushaOutputMembershipOperationV3::Split, true);
        let circuit = KagemushaOutputMembershipCircuitV3::new(witness)
            .expect("encoded two-output membership witness");
        let canonical = circuit.public_instances().expect("membership instances");
        for column in 0..KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V3 {
            let mut substituted = canonical.clone();
            substituted[column][0] += Scalar::ONE;
            let prover = halo2_proofs::dev::MockProver::run(
                KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V3,
                &circuit,
                substituted,
            )
            .expect("well-shaped substituted public instances");
            assert!(
                prover.verify().is_err(),
                "public output-membership column {column} must be proof-bound"
            );
        }
    }

    #[test]
    fn output_membership_poseidon_update_rejects_path_index_root_and_commitment_attacks() {
        type Mutation = fn(&mut KagemushaOutputMembershipWitnessV3);
        let mutations: [(&str, Mutation); 14] = [
            ("initial root", |witness| {
                witness.initial_root = bump_scalar_bytes(witness.initial_root);
            }),
            ("final root", |witness| {
                witness.final_root = bump_scalar_bytes(witness.final_root);
            }),
            ("recipient commitment", |witness| {
                let recipient = witness.recipient.as_mut().expect("recipient");
                recipient.commitment = bump_scalar_bytes(recipient.commitment);
            }),
            ("recipient index", |witness| {
                witness.recipient.as_mut().expect("recipient").leaf_index += 1;
            }),
            ("recipient update sibling", |witness| {
                let path = &mut witness.recipient.as_mut().expect("recipient").update_path;
                path.siblings[0] = bump_scalar_bytes(path.siblings[0]);
            }),
            ("recipient update direction", |witness| {
                let path = &mut witness.recipient.as_mut().expect("recipient").update_path;
                path.directions[0] ^= 1;
            }),
            ("recipient update root", |witness| {
                let path = &mut witness.recipient.as_mut().expect("recipient").update_path;
                path.root = bump_scalar_bytes(path.root);
            }),
            ("recipient membership sibling", |witness| {
                let path = &mut witness
                    .recipient
                    .as_mut()
                    .expect("recipient")
                    .membership_path;
                path.siblings[1] = bump_scalar_bytes(path.siblings[1]);
            }),
            ("change commitment", |witness| {
                let change = witness.change.as_mut().expect("change");
                change.commitment = bump_scalar_bytes(change.commitment);
            }),
            ("change index", |witness| {
                witness.change.as_mut().expect("change").leaf_index += 1;
            }),
            ("change update sibling", |witness| {
                let path = &mut witness.change.as_mut().expect("change").update_path;
                path.siblings[0] = bump_scalar_bytes(path.siblings[0]);
            }),
            ("change membership root", |witness| {
                let path = &mut witness.change.as_mut().expect("change").membership_path;
                path.root = bump_scalar_bytes(path.root);
            }),
            ("dummy index", |witness| {
                witness.dummy_leaf_index += 1;
            }),
            ("dummy path", |witness| {
                witness.dummy_path.siblings[0] = bump_scalar_bytes(witness.dummy_path.siblings[0]);
            }),
        ];
        for (name, mutate) in mutations {
            let mut witness =
                output_membership_fixture(KagemushaOutputMembershipOperationV3::Split, true);
            mutate(&mut witness);
            let circuit = KagemushaOutputMembershipCircuitV3::new(witness)
                .expect("adversarial scalar encodings remain canonical");
            let instances = circuit.public_instances().expect("membership instances");
            let prover = halo2_proofs::dev::MockProver::run(
                KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V3,
                &circuit,
                instances,
            )
            .expect("well-shaped adversarial membership circuit");
            assert!(
                prover.verify().is_err(),
                "{name} substitution must not satisfy output membership"
            );
        }
    }

    #[test]
    fn output_membership_poseidon_update_rejects_profile_presence_smuggling() {
        let mut init_with_change =
            output_membership_fixture(KagemushaOutputMembershipOperationV3::Split, true);
        init_with_change.operation = KagemushaOutputMembershipOperationV3::Init;
        assert_output_membership_rejected(init_with_change);

        let mut redemption_without_change =
            output_membership_fixture(KagemushaOutputMembershipOperationV3::Split, false);
        redemption_without_change.operation =
            KagemushaOutputMembershipOperationV3::RedemptionChange;
        assert_output_membership_rejected(redemption_without_change);

        let mut redemption_with_recipient =
            output_membership_fixture(KagemushaOutputMembershipOperationV3::Split, true);
        redemption_with_recipient.operation =
            KagemushaOutputMembershipOperationV3::RedemptionChange;
        assert_output_membership_rejected(redemption_with_recipient);
    }

    #[test]
    fn pasta_cycle_backend_stays_disabled_until_soundness_and_device_gates_pass() {
        let capabilities =
            iroha_data_model::offline::kagemusha_recursive_spend_native_capabilities_v1();
        capabilities.validate().expect("canonical capabilities");
        assert!(
            !iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE,
            "the proof backend must remain unavailable until every soundness and device gate passes"
        );
        let expected = [
            "paired_deferred_verifier",
            "proof_bound_output_membership_witnesses",
            "authenticated_release_envelope",
            "independent_cryptographic_review",
            "physical_device_performance_evidence",
        ]
        .map(str::to_owned)
        .to_vec();
        assert_eq!(
            capabilities.missing_gates, expected,
            "the fail-closed blocker set is release-authenticated"
        );

        let mut omitted = expected.clone();
        omitted.pop();
        let mut duplicate = expected.clone();
        duplicate.push(expected[0].clone());
        let mut unknown = expected.clone();
        unknown[0] = "unrecognized_release_gate".to_owned();
        let mut reordered = expected.clone();
        reordered.swap(0, 1);
        for (name, missing_gates) in [
            ("omitted", omitted),
            ("duplicate", duplicate),
            ("unknown", unknown),
            ("reordered", reordered),
        ] {
            let mut candidate = capabilities.clone();
            candidate.missing_gates = missing_gates;
            assert!(
                candidate.validate().is_err(),
                "{name} Kagemusha release-gate set must reject"
            );
        }
    }
}
