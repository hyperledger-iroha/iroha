//! Branch-safe fractional Kagemusha recursive-spend V2 backend.
//!
//! The V2 circuit deliberately keeps the large recursive IPA verifier slice
//! separate from the compact transition relation.  The latter is exposed as
//! one fixed-height instance column.  Consequently, adding an independently
//! spendable recipient or change branch changes neither the circuit shape nor
//! the proof size.

use ff::PrimeField;
use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    halo2curves::pasta::Fp as Scalar,
    plonk::{Circuit, ConstraintSystem, Error as PlonkError, Expression, Selector},
    poly::Rotation,
};
use iroha_data_model::{
    confidential::ConfidentialStatus,
    offline::{
        KagemushaRecursiveSpendArtifactReferenceV2, KagemushaRecursiveSpendArtifactRoleV2,
        KagemushaRecursiveSpendBranchV2, KagemushaRecursiveSpendPublicStatementV2,
        KagemushaRecursiveSpendRedemptionIntentV2, KagemushaRecursiveSpendSplitIntentV2,
        KagemushaRecursiveSpendTransitionV2,
    },
    proof::{VerifyingKeyBox, VerifyingKeyRecord},
    zk::BackendTag,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use std::io::{Read, Write};

use super::assign_advice_compat;

/// Public-input schema for the branch-safe V2 transition relation.
///
/// All entries are encoded as consecutive rows in one Pasta instance column.
/// The schema is hashed into the verifier record and the streamed proving-key
/// package; changing an offset therefore requires a new circuit generation.
pub const KAGEMUSHA_RECURSIVE_SPEND_V2_PUBLIC_INPUTS_SCHEMA: &[u8] = br#"{"schema":"kagemusha_recursive_spend_v2","layout":"single_column_rows_v1","binds":["canonical_statement_digest","chain_id","asset_definition_id","asset_scale","u128_amounts","parent_bundle_digest","confidential_transfer_v2_public_inputs","recipient_request_digest","operation_ids","branch_selector","branch_path","optional_change","proof_step_count","peer_hop_count","artifact_generation","verifier_key_id"]}"#;

/// Version of the fixed transition instance layout.
pub const KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION: u64 = 1;

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
const I_ARTIFACT_GENERATION_DIGEST: usize = I_TOPUP_OPERATION_ID + 4;
const I_CURRENT_HOP_DOMAIN_TAG: usize = I_ARTIFACT_GENERATION_DIGEST + 4;
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

/// Number of rows in the V2 transition public-instance column.
pub const KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS: usize =
    I_UNSHIELD_PUBLIC_AMOUNT + 1;

const PATH_SELECTOR_COUNT: usize = 64;

/// Fixed public and private witness values for one V2 output branch.
#[derive(Clone, Debug)]
pub struct KagemushaRecursiveSpendTransitionValuesV2 {
    /// Consecutive public rows described by
    /// [`KAGEMUSHA_RECURSIVE_SPEND_V2_PUBLIC_INPUTS_SCHEMA`].
    pub public: [Scalar; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS],
    /// Carry from the low 64-bit limb of recipient + change.
    amount_low_carry: Scalar,
    /// One-hot selector for the parent branch depth on append.
    path_depth_selectors: [Scalar; PATH_SELECTOR_COUNT],
}

impl Default for KagemushaRecursiveSpendTransitionValuesV2 {
    fn default() -> Self {
        let mut public = [Scalar::from(0); KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS];
        public[I_LAYOUT_VERSION] =
            Scalar::from(KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION);
        // Keygen uses this witnessless, internally consistent init shape.
        public[I_PROOF_STEP_COUNT] = Scalar::from(1);
        Self {
            public,
            amount_low_carry: Scalar::from(0),
            path_depth_selectors: [Scalar::from(0); PATH_SELECTOR_COUNT],
        }
    }
}

impl KagemushaRecursiveSpendTransitionValuesV2 {
    fn validate_host_relation(&self) -> Result<(), String> {
        let value = |index: usize| self.public[index];
        let zero = Scalar::from(0);
        let one = Scalar::from(1);
        for (index, field) in [
            (I_APPEND_PROFILE, "append_profile"),
            (I_REDEMPTION_PROFILE, "redemption_profile"),
            (I_BRANCH_CHANGE, "branch_change"),
            (I_HAS_CHANGE, "has_change"),
            (I_RECORD_OUTPUT_SWAP, "record_output_swap"),
            (I_TRANSFER_OUTPUT_SWAP, "transfer_output_swap"),
        ] {
            if value(index) != zero && value(index) != one {
                return Err(format!("Kagemusha V2 {field} must be boolean"));
            }
        }
        if value(I_LAYOUT_VERSION)
            != Scalar::from(KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION)
        {
            return Err("Kagemusha V2 transition layout version mismatch".to_owned());
        }
        if value(I_BRANCH_CHANGE) == one && value(I_HAS_CHANGE) != one {
            return Err("Kagemusha V2 change branch requires a change output".to_owned());
        }
        if value(I_APPEND_PROFILE) == one && value(I_REDEMPTION_PROFILE) == one {
            return Err("Kagemusha V2 transition profiles are mutually exclusive".to_owned());
        }
        if value(I_REDEMPTION_PROFILE) == one
            && (value(I_BRANCH_CHANGE) != one || value(I_HAS_CHANGE) != one)
        {
            return Err(
                "Kagemusha V2 redemption transition must produce the change branch".to_owned(),
            );
        }
        if self.amount_low_carry != zero && self.amount_low_carry != one {
            return Err("Kagemusha V2 amount carry must be boolean".to_owned());
        }
        let selector_sum = self
            .path_depth_selectors
            .iter()
            .copied()
            .fold(zero, |sum, selector| sum + selector);
        if selector_sum != value(I_APPEND_PROFILE) + value(I_REDEMPTION_PROFILE) {
            return Err("Kagemusha V2 branch-depth selector sum mismatch".to_owned());
        }
        Ok(())
    }
}

/// Constraint-system columns for the V2 transition relation.
#[derive(Clone)]
pub struct KagemushaRecursiveSpendTransitionConfigV2 {
    public_advice: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    public_instance: halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
    amount_low_carry: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    path_depth_selector: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    relation: Selector,
}

/// Exact branch-safe split circuit shared by init and append compositions.
#[derive(Clone, Debug, Default)]
pub struct KagemushaRecursiveSpendTransitionCircuitV2 {
    /// Fixed transition witness and public values.
    pub values: KagemushaRecursiveSpendTransitionValuesV2,
}

fn query_at(
    meta: &mut halo2_proofs::plonk::VirtualCells<'_, Scalar>,
    column: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    index: usize,
) -> Expression<Scalar> {
    meta.query_advice(
        column,
        Rotation(i32::try_from(index).expect("V2 transition row offset fits i32")),
    )
}

fn query_instance_at(
    meta: &mut halo2_proofs::plonk::VirtualCells<'_, Scalar>,
    column: halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
    index: usize,
) -> Expression<Scalar> {
    meta.query_instance(
        column,
        Rotation(i32::try_from(index).expect("V2 transition row offset fits i32")),
    )
}

fn select_expression(
    first: Expression<Scalar>,
    second: Expression<Scalar>,
    selector: Expression<Scalar>,
) -> Expression<Scalar> {
    first.clone() + selector * (second - first)
}

impl Circuit<Scalar> for KagemushaRecursiveSpendTransitionCircuitV2 {
    type Config = KagemushaRecursiveSpendTransitionConfigV2;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        meta.set_minimum_degree(3);
        let public_advice = meta.advice_column();
        let public_instance = meta.instance_column();
        let amount_low_carry = meta.advice_column();
        let path_depth_selector = meta.advice_column();
        let relation = meta.selector();

        meta.create_gate("kagemusha_recursive_spend_v2_transition", |meta| {
            let enabled = meta.query_selector(relation);
            let public = (0..KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS)
                .map(|index| query_at(meta, public_advice, index))
                .collect::<Vec<_>>();
            let p = |index: usize| public[index].clone();
            let one = Expression::Constant(Scalar::from(1));
            let zero = Expression::Constant(Scalar::from(0));
            let two_pow_64 = Expression::Constant(Scalar::from_u128(1u128 << 64));
            let append = p(I_APPEND_PROFILE);
            let redemption = p(I_REDEMPTION_PROFILE);
            let extends = append.clone() + redemption.clone();
            let branch = p(I_BRANCH_CHANGE);
            let has_change = p(I_HAS_CHANGE);
            let record_swap = p(I_RECORD_OUTPUT_SWAP);
            let transfer_swap = p(I_TRANSFER_OUTPUT_SWAP);
            let carry = meta.query_advice(amount_low_carry, Rotation::cur());

            let mut constraints = Vec::with_capacity(
                KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS + PATH_SELECTOR_COUNT + 96,
            );
            for index in 0..KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS {
                constraints.push(
                    enabled.clone() * (p(index) - query_instance_at(meta, public_instance, index)),
                );
            }
            constraints.push(
                enabled.clone()
                    * (p(I_LAYOUT_VERSION)
                        - Expression::Constant(Scalar::from(
                            KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION,
                        ))),
            );
            for boolean in [
                append.clone(),
                redemption.clone(),
                branch.clone(),
                has_change.clone(),
                record_swap.clone(),
                transfer_swap.clone(),
                carry.clone(),
            ] {
                constraints.push(enabled.clone() * boolean.clone() * (boolean - one.clone()));
            }
            constraints.push(enabled.clone() * branch.clone() * (one.clone() - has_change.clone()));
            constraints.push(enabled.clone() * append.clone() * redemption.clone());
            constraints.push(enabled.clone() * redemption.clone() * (one.clone() - branch.clone()));
            constraints.push(
                enabled.clone() * redemption.clone() * (one.clone() - has_change.clone()),
            );
            constraints.push(enabled.clone() * redemption.clone() * record_swap.clone());
            constraints.push(enabled.clone() * redemption.clone() * transfer_swap.clone());
            constraints.push(
                enabled.clone()
                    * record_swap.clone()
                    * (one.clone() - has_change.clone())
                    * append.clone(),
            );
            constraints.push(
                enabled.clone()
                    * transfer_swap.clone()
                    * (one.clone() - has_change.clone())
                    * append.clone(),
            );

            // Proof-step, peer-hop, and branch-depth counters advance on the
            // same append relation, but remain separate public quantities.
            constraints.extend([
                enabled.clone()
                    * (p(I_PROOF_STEP_COUNT) - p(I_PREVIOUS_PROOF_STEP_COUNT) - one.clone()),
                enabled.clone()
                    * (p(I_PEER_HOP_COUNT) - p(I_PREVIOUS_PEER_HOP_COUNT) - append.clone()),
                enabled.clone() * (p(I_BRANCH_DEPTH) - p(I_PARENT_BRANCH_DEPTH) - extends.clone()),
                enabled.clone() * (one.clone() - extends.clone()) * p(I_PREVIOUS_PROOF_STEP_COUNT),
                enabled.clone() * (one.clone() - extends.clone()) * p(I_PREVIOUS_PEER_HOP_COUNT),
                enabled.clone() * (one.clone() - extends.clone()) * p(I_PARENT_BRANCH_DEPTH),
            ]);

            // Every amount uses the authoritative asset scale.  An absent
            // change uses canonical all-zero scale/amount/note fields.
            constraints.extend([
                enabled.clone() * (p(I_CURRENT_SCALE) - p(I_ASSET_SCALE)),
                enabled.clone() * (p(I_INPUT_SCALE) - p(I_ASSET_SCALE)),
                enabled.clone() * (p(I_TRANSFER_SCALE) - p(I_ASSET_SCALE)),
                enabled.clone() * (p(I_RECIPIENT_SCALE) - p(I_ASSET_SCALE)),
                enabled.clone() * (p(I_CHANGE_SCALE) - has_change.clone() * p(I_ASSET_SCALE)),
                enabled.clone() * (p(I_TRANSFER_AMOUNT_LO) - p(I_RECIPIENT_AMOUNT_LO)),
                enabled.clone() * (p(I_TRANSFER_AMOUNT_HI) - p(I_RECIPIENT_AMOUNT_HI)),
                enabled.clone()
                    * redemption.clone()
                    * (p(I_UNSHIELD_PUBLIC_AMOUNT)
                        - p(I_RECIPIENT_AMOUNT_LO)
                        - p(I_RECIPIENT_AMOUNT_HI) * two_pow_64.clone()),
                enabled.clone()
                    * (p(I_RECIPIENT_AMOUNT_LO) + p(I_CHANGE_AMOUNT_LO)
                        - p(I_INPUT_AMOUNT_LO)
                        - carry.clone() * two_pow_64),
                enabled.clone()
                    * (p(I_RECIPIENT_AMOUNT_HI) + p(I_CHANGE_AMOUNT_HI) + carry.clone()
                        - p(I_INPUT_AMOUNT_HI)),
                enabled.clone()
                    * (p(I_CURRENT_AMOUNT_LO)
                        - select_expression(
                            p(I_RECIPIENT_AMOUNT_LO),
                            p(I_CHANGE_AMOUNT_LO),
                            branch.clone(),
                        )),
                enabled.clone()
                    * (p(I_CURRENT_AMOUNT_HI)
                        - select_expression(
                            p(I_RECIPIENT_AMOUNT_HI),
                            p(I_CHANGE_AMOUNT_HI),
                            branch.clone(),
                        )),
                enabled.clone() * (one.clone() - has_change.clone()) * p(I_CHANGE_AMOUNT_LO),
                enabled.clone() * (one.clone() - has_change.clone()) * p(I_CHANGE_AMOUNT_HI),
                enabled.clone() * (one.clone() - has_change.clone()) * p(I_CHANGE_COMMITMENT),
                enabled.clone() * (one.clone() - has_change.clone()) * p(I_CHANGE_NULLIFIER),
            ]);

            // Select the independently spendable output.  This is what makes
            // sibling proofs distinct even though they share the same checked
            // confidential transition witness.
            constraints.extend([
                enabled.clone()
                    * (p(I_CURRENT_COMMITMENT)
                        - select_expression(
                            p(I_RECIPIENT_COMMITMENT),
                            p(I_CHANGE_COMMITMENT),
                            branch.clone(),
                        )),
                enabled.clone()
                    * (p(I_CURRENT_NULLIFIER)
                        - select_expression(
                            p(I_RECIPIENT_NULLIFIER),
                            p(I_CHANGE_NULLIFIER),
                            branch.clone(),
                        )),
            ]);

            // The record-backed fold step and the confidential-transfer V2
            // public inputs must describe the same roots and proof arity.
            constraints.extend([
                enabled.clone()
                    * (p(I_RECORD_ROOT_BEFORE)
                        - select_expression(
                            p(I_INITIAL_ROOT),
                            p(I_PARENT_FINAL_ROOT),
                            extends.clone(),
                        )),
                enabled.clone() * (p(I_RECORD_ROOT_AFTER) - p(I_FINAL_ROOT)),
                enabled.clone()
                    * (p(I_TRANSFER_ROOT)
                        - select_expression(
                            p(I_FINAL_ROOT),
                            p(I_PARENT_FINAL_ROOT),
                            redemption.clone(),
                        )),
                enabled.clone() * (p(I_RECORD_INPUT_COUNT) - p(I_TRANSFER_INPUT_COUNT)),
                enabled.clone() * (p(I_RECORD_OUTPUT_COUNT) - p(I_TRANSFER_OUTPUT_COUNT)),
            ]);

            // Extensions consume exactly the parent note. Init may contain a
            // second online input, so these equalities are append-gated.
            constraints.extend([
                enabled.clone() * extends.clone() * (p(I_RECORD_INPUT_COUNT) - one.clone()),
                enabled.clone() * extends.clone() * (p(I_TRANSFER_INPUT_COUNT) - one.clone()),
                enabled.clone()
                    * extends.clone()
                    * (p(I_RECORD_INPUT_NULLIFIER_0) - p(I_INPUT_NULLIFIER)),
                enabled.clone() * extends.clone() * p(I_RECORD_INPUT_NULLIFIER_1),
                enabled.clone()
                    * extends.clone()
                    * (p(I_TRANSFER_INPUT_COMMITMENT_0) - p(I_INPUT_COMMITMENT)),
                enabled.clone() * extends.clone() * p(I_TRANSFER_INPUT_COMMITMENT_1),
                enabled.clone()
                    * extends.clone()
                    * (p(I_TRANSFER_NULLIFIER_0) - p(I_INPUT_NULLIFIER)),
                enabled.clone() * extends.clone() * p(I_TRANSFER_NULLIFIER_1),
            ]);

            // Both record ordering and transfer-proof output ordering are
            // independently canonicalized by a private/public boolean.  The
            // booleans are nevertheless public, making the full relation
            // independently reproducible from the envelope.
            let expected_record_0 = select_expression(
                p(I_RECIPIENT_COMMITMENT),
                p(I_CHANGE_COMMITMENT),
                record_swap.clone(),
            );
            let expected_record_1 = select_expression(
                p(I_CHANGE_COMMITMENT),
                p(I_RECIPIENT_COMMITMENT),
                record_swap.clone(),
            );
            let expected_transfer_0 = select_expression(
                p(I_RECIPIENT_COMMITMENT),
                p(I_CHANGE_COMMITMENT),
                transfer_swap.clone(),
            );
            let expected_transfer_1 = select_expression(
                p(I_CHANGE_COMMITMENT),
                p(I_RECIPIENT_COMMITMENT),
                transfer_swap.clone(),
            );
            constraints.extend([
                enabled.clone() * append.clone() * (p(I_RECORD_OUTPUT_0) - expected_record_0),
                enabled.clone() * append.clone() * (p(I_RECORD_OUTPUT_1) - expected_record_1),
                enabled.clone() * append.clone() * (p(I_TRANSFER_OUTPUT_0) - expected_transfer_0),
                enabled.clone() * append.clone() * (p(I_TRANSFER_OUTPUT_1) - expected_transfer_1),
                enabled.clone()
                    * append.clone()
                    * (p(I_RECORD_OUTPUT_COUNT) - one.clone() - has_change.clone()),
                enabled.clone()
                    * append.clone()
                    * (p(I_TRANSFER_OUTPUT_COUNT) - one.clone() - has_change.clone()),
                enabled.clone()
                    * redemption.clone()
                    * (p(I_RECORD_OUTPUT_COUNT) - one.clone()),
                enabled.clone()
                    * redemption.clone()
                    * (p(I_TRANSFER_OUTPUT_COUNT) - one.clone()),
                enabled.clone()
                    * redemption.clone()
                    * (p(I_RECORD_OUTPUT_0) - p(I_CHANGE_COMMITMENT)),
                enabled.clone() * redemption.clone() * p(I_RECORD_OUTPUT_1),
                enabled.clone()
                    * redemption.clone()
                    * (p(I_TRANSFER_OUTPUT_0) - p(I_CHANGE_COMMITMENT)),
                enabled.clone() * redemption.clone() * p(I_TRANSFER_OUTPUT_1),
                enabled.clone() * redemption.clone() * p(I_RECIPIENT_COMMITMENT),
                enabled.clone() * redemption.clone() * p(I_RECIPIENT_NULLIFIER),
            ]);

            // Init binds the selected first output and also cross-binds the
            // unselected output, if present, between the checked record and the
            // confidential proof.
            let record_selected = select_expression(
                p(I_RECORD_OUTPUT_0),
                p(I_RECORD_OUTPUT_1),
                record_swap.clone(),
            );
            let transfer_selected = select_expression(
                p(I_TRANSFER_OUTPUT_0),
                p(I_TRANSFER_OUTPUT_1),
                transfer_swap.clone(),
            );
            let record_other =
                select_expression(p(I_RECORD_OUTPUT_1), p(I_RECORD_OUTPUT_0), record_swap);
            let transfer_other = select_expression(
                p(I_TRANSFER_OUTPUT_1),
                p(I_TRANSFER_OUTPUT_0),
                transfer_swap,
            );
            constraints.extend([
                enabled.clone()
                    * (one.clone() - extends.clone())
                    * (record_selected - p(I_CURRENT_COMMITMENT)),
                enabled.clone()
                    * (one.clone() - extends.clone())
                    * (transfer_selected - p(I_CURRENT_COMMITMENT)),
                enabled.clone() * (one.clone() - extends.clone()) * (record_other - transfer_other),
                enabled.clone()
                    * (one.clone() - extends.clone())
                    * (p(I_RECORD_OUTPUT_COUNT) - p(I_TRANSFER_OUTPUT_COUNT)),
            ]);

            // Branch path: append selects exactly one parent depth and adds the
            // recipient bit (0) or change bit (1) at that depth.
            let mut selector_sum = zero.clone();
            let mut selected_depth = zero.clone();
            let mut selected_mask = zero;
            for depth in 0..PATH_SELECTOR_COUNT {
                let selector = meta.query_advice(
                    path_depth_selector,
                    Rotation(i32::try_from(depth).expect("path selector row fits i32")),
                );
                constraints
                    .push(enabled.clone() * selector.clone() * (selector.clone() - one.clone()));
                selector_sum = selector_sum + selector.clone();
                selected_depth = selected_depth
                    + selector.clone()
                        * Expression::Constant(Scalar::from(
                            u64::try_from(depth).expect("path depth fits u64"),
                        ));
                selected_mask = selected_mask
                    + selector * Expression::Constant(Scalar::from(1u64 << (63 - depth)));
            }
            constraints.extend([
                enabled.clone() * (selector_sum - extends.clone()),
                enabled.clone() * (selected_depth - p(I_PARENT_BRANCH_DEPTH)),
                enabled.clone()
                    * (p(I_BRANCH_PATH_BITS)
                        - p(I_PARENT_BRANCH_PATH_BITS)
                        - branch * selected_mask),
            ]);
            for limb in 0..4 {
                constraints.push(
                    enabled.clone()
                        * extends.clone()
                        * (p(I_BRANCH_LINEAGE_ROOT + limb)
                            - p(I_PARENT_BRANCH_LINEAGE_ROOT + limb)),
                );
                constraints.push(
                    enabled.clone()
                        * (one.clone() - extends.clone())
                        * p(I_PARENT_BRANCH_LINEAGE_ROOT + limb),
                );
                constraints.push(
                    enabled.clone()
                        * extends.clone()
                        * (p(I_TOPUP_RECEIPT_DIGEST + limb)
                            - p(I_PARENT_TOPUP_RECEIPT_DIGEST + limb)),
                );
                constraints.push(
                    enabled.clone()
                        * (one.clone() - extends.clone())
                        * p(I_PARENT_TOPUP_RECEIPT_DIGEST + limb),
                );
                constraints.push(
                    enabled.clone()
                        * (one.clone() - redemption.clone())
                        * p(I_REDEMPTION_RECIPIENT_DIGEST + limb),
                );
                constraints.push(
                    enabled.clone()
                        * (one.clone() - redemption.clone())
                        * p(I_UNSHIELD_PUBLIC_INPUTS_DIGEST + limb),
                );
            }
            constraints.extend([
                enabled.clone() * (one.clone() - extends.clone()) * p(I_PARENT_FINAL_ROOT),
                enabled.clone() * (one.clone() - redemption.clone()) * p(I_UNSHIELD_PUBLIC_AMOUNT),
            ]);
            constraints
        });

        KagemushaRecursiveSpendTransitionConfigV2 {
            public_advice,
            public_instance,
            amount_low_carry,
            path_depth_selector,
            relation,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), PlonkError> {
        self.values
            .validate_host_relation()
            .map_err(|_| PlonkError::Synthesis)?;
        let values = self.values.clone();
        layouter.assign_region(
            || "kagemusha_recursive_spend_v2_transition",
            |mut region| {
                config.relation.enable(&mut region, 0)?;
                for (row, value) in values.public.iter().copied().enumerate() {
                    assign_advice_compat(
                        &mut region,
                        move || format!("v2_public_{row}"),
                        config.public_advice,
                        row,
                        || Value::known(value),
                    )?;
                }
                assign_advice_compat(
                    &mut region,
                    || "amount_low_carry",
                    config.amount_low_carry,
                    0,
                    || Value::known(values.amount_low_carry),
                )?;
                for (row, selector) in values.path_depth_selectors.iter().copied().enumerate() {
                    assign_advice_compat(
                        &mut region,
                        move || format!("path_depth_selector_{row}"),
                        config.path_depth_selector,
                        row,
                        || Value::known(selector),
                    )?;
                }
                Ok(())
            },
        )
    }
}

fn bytes_to_limbs(bytes: &[u8; 32]) -> [Scalar; 4] {
    super::bytes_to_u64_limbs_le(bytes).map(Scalar::from)
}

fn write_limb_group(
    public: &mut [Scalar; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS],
    start: usize,
    bytes: &[u8; 32],
) {
    public[start..start + 4].copy_from_slice(&bytes_to_limbs(bytes));
}

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

fn split_u128(value: u128) -> [Scalar; 2] {
    [
        Scalar::from(value as u64),
        Scalar::from((value >> 64) as u64),
    ]
}

fn write_amount(
    public: &mut [Scalar; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS],
    start: usize,
    value: u128,
) {
    public[start..start + 2].copy_from_slice(&split_u128(value));
}

fn path_bits_as_u64(path_bits: [u8; 8]) -> u64 {
    u64::from_be_bytes(path_bits)
}

fn branch_selector(branch: KagemushaRecursiveSpendBranchV2) -> Scalar {
    match branch {
        KagemushaRecursiveSpendBranchV2::Recipient => Scalar::from(0),
        KagemushaRecursiveSpendBranchV2::Change => Scalar::from(1),
    }
}

/// Confidential-transfer V2 public words cross-bound by the recursive circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaConfidentialTransferPublicInputsV2 {
    /// Up to two input note commitments.
    pub input_commitments: [[u8; 32]; 2],
    /// Up to two input nullifiers.
    pub nullifiers: [[u8; 32]; 2],
    /// Up to two output note commitments.
    pub output_commitments: [[u8; 32]; 2],
    /// Resulting confidential-state root.
    pub root: [u8; 32],
    /// Canonical confidential asset tag.
    pub asset_tag: [u8; 32],
    /// Canonical confidential chain tag.
    pub chain_tag: [u8; 32],
}

impl KagemushaConfidentialTransferPublicInputsV2 {
    fn from_proof(proof_bytes: &[u8]) -> Result<Self, String> {
        let (input_commitments, nullifiers, output_commitments, root, asset_tag, chain_tag) =
            super::confidential_v2::parse_transfer_public_inputs(proof_bytes)?;
        Ok(Self {
            input_commitments,
            nullifiers,
            output_commitments,
            root,
            asset_tag,
            chain_tag,
        })
    }

    fn input_count(self) -> usize {
        1 + usize::from(self.input_commitments[1] != [0; 32])
    }

    fn output_count(self) -> usize {
        1 + usize::from(self.output_commitments[1] != [0; 32])
    }
}

/// Confidential-unshield-v3 public words cross-bound by the redemption circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaConfidentialUnshieldPublicInputsV3 {
    /// Up to two input note commitments.
    pub input_commitments: [[u8; 32]; 2],
    /// Up to two input nullifiers.
    pub nullifiers: [[u8; 32]; 2],
    /// Partial-redemption change commitment.
    pub change_commitment: [u8; 32],
    /// Root at which the redeemed note is live.
    pub root: [u8; 32],
    /// Exact credited amount encoded as one canonical Pasta word.
    pub public_amount: [u8; 32],
    /// Canonical confidential asset tag.
    pub asset_tag: [u8; 32],
    /// Canonical confidential chain tag.
    pub chain_tag: [u8; 32],
}

impl KagemushaConfidentialUnshieldPublicInputsV3 {
    fn from_proof(proof_bytes: &[u8]) -> Result<Self, String> {
        let (
            input_commitments,
            nullifiers,
            change_commitment,
            root,
            public_amount,
            asset_tag,
            chain_tag,
        ) = super::confidential_v2::parse_unshield_public_inputs_v3(proof_bytes)?;
        Ok(Self {
            input_commitments,
            nullifiers,
            change_commitment,
            root,
            public_amount,
            asset_tag,
            chain_tag,
        })
    }
}

fn output_swap_for_target(outputs: &[[u8; 32]], target: [u8; 32]) -> Result<bool, String> {
    match outputs.iter().position(|output| *output == target) {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        Some(_) => unreachable!("Kagemusha V2 supports at most two outputs"),
        None => Err("Kagemusha V2 checked transfer is missing the selected output".to_owned()),
    }
}

fn fill_common_statement_values(
    public: &mut [Scalar; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS],
    statement: &KagemushaRecursiveSpendPublicStatementV2,
    current_hop_domain_tag: [u8; 32],
    topup_receipt_digest: [u8; 32],
) -> Result<(), String> {
    statement
        .validate_context()
        .map_err(|err| err.to_string())?;
    public[I_LAYOUT_VERSION] = Scalar::from(KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION);
    public[I_PROOF_STEP_COUNT] = Scalar::from(u64::from(statement.proof_step_count));
    public[I_PEER_HOP_COUNT] = Scalar::from(u64::from(statement.peer_hop_count));
    public[I_BRANCH_DEPTH] = Scalar::from(u64::from(statement.branch_path.depth));
    public[I_ASSET_SCALE] = Scalar::from(u64::from(statement.asset_scale));
    public[I_CURRENT_SCALE] = Scalar::from(u64::from(statement.current_note.amount.scale));
    public[I_BRANCH_PATH_BITS] = Scalar::from(path_bits_as_u64(statement.branch_path.path_bits));
    public[I_INITIAL_ROOT] = scalar_from_canonical_bytes(&statement.initial_root, "initial root")?;
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
    write_limb_group(
        public,
        I_BRANCH_LINEAGE_ROOT,
        &statement.branch_path.lineage_root,
    );
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
    write_limb_group(public, I_TOPUP_OPERATION_ID, &statement.topup_operation_id);
    write_limb_group(
        public,
        I_ARTIFACT_GENERATION_DIGEST,
        &canonical_poseidon_digest(&statement.artifact_generation)?,
    );
    write_limb_group(public, I_CURRENT_HOP_DOMAIN_TAG, &current_hop_domain_tag);
    write_limb_group(public, I_TOPUP_RECEIPT_DIGEST, &topup_receipt_digest);
    write_limb_group(
        public,
        I_TOPUP_ANCHOR_DIGEST,
        &canonical_poseidon_digest(&statement.topup_anchor_nullifiers)?,
    );
    public[I_TOPUP_ANCHOR_COUNT] = Scalar::from(
        u64::try_from(statement.topup_anchor_nullifiers.len())
            .map_err(|_| "Kagemusha V2 top-up anchor count does not fit u64".to_owned())?,
    );
    write_limb_group(
        public,
        I_VERIFIER_KEY_ID_DIGEST,
        &canonical_poseidon_digest(&statement.verifier_key_id)?,
    );
    Ok(())
}

fn fill_transfer_values(
    public: &mut [Scalar; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS],
    transfer: KagemushaConfidentialTransferPublicInputsV2,
) -> Result<(), String> {
    public[I_TRANSFER_INPUT_COUNT] = Scalar::from(
        u64::try_from(transfer.input_count())
            .map_err(|_| "Kagemusha V2 transfer input count overflow".to_owned())?,
    );
    public[I_TRANSFER_OUTPUT_COUNT] = Scalar::from(
        u64::try_from(transfer.output_count())
            .map_err(|_| "Kagemusha V2 transfer output count overflow".to_owned())?,
    );
    for (index, bytes) in transfer.input_commitments.iter().enumerate() {
        public[I_TRANSFER_INPUT_COMMITMENT_0 + index] =
            scalar_from_canonical_bytes(bytes, "transfer input commitment")?;
    }
    for (index, bytes) in transfer.nullifiers.iter().enumerate() {
        public[I_TRANSFER_NULLIFIER_0 + index] =
            scalar_from_canonical_bytes(bytes, "transfer nullifier")?;
    }
    for (index, bytes) in transfer.output_commitments.iter().enumerate() {
        public[I_TRANSFER_OUTPUT_0 + index] =
            scalar_from_canonical_bytes(bytes, "transfer output commitment")?;
    }
    public[I_TRANSFER_ROOT] = scalar_from_canonical_bytes(&transfer.root, "transfer root")?;
    public[I_ASSET_TAG] = scalar_from_canonical_bytes(&transfer.asset_tag, "asset tag")?;
    public[I_CHAIN_TAG] = scalar_from_canonical_bytes(&transfer.chain_tag, "chain tag")?;
    Ok(())
}

fn fill_record_values(
    public: &mut [Scalar; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS],
    step: &iroha_data_model::offline::KagemushaVerifiedFoldStep,
) -> Result<(), String> {
    public[I_RECORD_INPUT_COUNT] = Scalar::from(
        u64::try_from(step.input_nullifiers.len())
            .map_err(|_| "Kagemusha V2 record input count overflow".to_owned())?,
    );
    public[I_RECORD_OUTPUT_COUNT] = Scalar::from(
        u64::try_from(step.output_commitments.len())
            .map_err(|_| "Kagemusha V2 record output count overflow".to_owned())?,
    );
    public[I_RECORD_ROOT_BEFORE] =
        scalar_from_canonical_bytes(&step.root_before, "record root before")?;
    public[I_RECORD_ROOT_AFTER] =
        scalar_from_canonical_bytes(&step.root_after, "record root after")?;
    for (index, bytes) in step.input_nullifiers.iter().enumerate().take(2) {
        public[I_RECORD_INPUT_NULLIFIER_0 + index] =
            scalar_from_canonical_bytes(bytes, "record input nullifier")?;
    }
    for (index, bytes) in step.output_commitments.iter().enumerate().take(2) {
        public[I_RECORD_OUTPUT_0 + index] =
            scalar_from_canonical_bytes(bytes, "record output commitment")?;
    }
    Ok(())
}

fn set_amount_carry(
    values: &mut KagemushaRecursiveSpendTransitionValuesV2,
    split: &KagemushaRecursiveSpendSplitIntentV2,
) {
    let recipient_low = split.recipient_output.amount.atomic_units as u64;
    let change_low = split
        .change_output
        .as_ref()
        .map_or(0, |change| change.amount.atomic_units as u64);
    values.amount_low_carry =
        Scalar::from(u64::from(recipient_low.checked_add(change_low).is_none()));
}

fn set_path_selector(
    values: &mut KagemushaRecursiveSpendTransitionValuesV2,
    parent_depth: u8,
) -> Result<(), String> {
    let index = usize::from(parent_depth);
    if index >= PATH_SELECTOR_COUNT {
        return Err("Kagemusha V2 append parent branch depth must be below 64".to_owned());
    }
    values.path_depth_selectors[index] = Scalar::from(1);
    Ok(())
}

/// Build the exact public/witness relation for an initial V2 branch.
pub fn kagemusha_recursive_spend_init_transition_values_v2(
    statement: &KagemushaRecursiveSpendPublicStatementV2,
    anchor: &iroha_data_model::offline::KagemushaRecursiveSpendTopUpAnchorV2,
    step: &iroha_data_model::offline::KagemushaVerifiedFoldStep,
    current_hop_domain_tag: [u8; 32],
) -> Result<KagemushaRecursiveSpendTransitionValuesV2, String> {
    anchor
        .validate_public_binding()
        .map_err(|err| err.to_string())?;
    if statement.transition.is_some() || statement.peer_hop_count != 0 {
        return Err("Kagemusha V2 init statement must not contain a split branch".to_owned());
    }
    if anchor.chain_id != statement.chain_id
        || anchor.asset.definition() != &statement.asset
        || anchor.asset_scale != statement.asset_scale
        || anchor.amount != statement.current_note.amount
        || anchor.current_note != statement.current_note
        || anchor.initial_root != statement.initial_root
        || anchor.finalized_root != statement.final_root
        || anchor.topup_anchor_nullifiers != statement.topup_anchor_nullifiers
        || anchor.topup_operation_id != statement.topup_operation_id
        || anchor.artifact_generation != statement.artifact_generation
        || anchor.transfer_verifier_id != step.attachment.vk_ref
        || step.attachment.vk_commitment != Some(anchor.transfer_verifier_commitment)
    {
        return Err(
            "Kagemusha V2 init statement does not match finalized top-up anchor".to_owned(),
        );
    }
    let transfer =
        KagemushaConfidentialTransferPublicInputsV2::from_proof(&step.attachment.proof.bytes)?;
    let mut values = KagemushaRecursiveSpendTransitionValuesV2::default();
    fill_common_statement_values(
        &mut values.public,
        statement,
        current_hop_domain_tag,
        anchor.anchor_digest,
    )?;
    fill_transfer_values(&mut values.public, transfer)?;
    fill_record_values(&mut values.public, step)?;
    let current = &statement.current_note;
    let record_swap = output_swap_for_target(&step.output_commitments, current.note_commitment)?;
    let transfer_swap =
        output_swap_for_target(&transfer.output_commitments, current.note_commitment)?;
    values.public[I_RECORD_OUTPUT_SWAP] = Scalar::from(u64::from(record_swap));
    values.public[I_TRANSFER_OUTPUT_SWAP] = Scalar::from(u64::from(transfer_swap));
    values.public[I_INPUT_SCALE] = Scalar::from(u64::from(current.amount.scale));
    values.public[I_TRANSFER_SCALE] = Scalar::from(u64::from(current.amount.scale));
    values.public[I_RECIPIENT_SCALE] = Scalar::from(u64::from(current.amount.scale));
    write_amount(
        &mut values.public,
        I_INPUT_AMOUNT_LO,
        current.amount.atomic_units,
    );
    write_amount(
        &mut values.public,
        I_TRANSFER_AMOUNT_LO,
        current.amount.atomic_units,
    );
    write_amount(
        &mut values.public,
        I_RECIPIENT_AMOUNT_LO,
        current.amount.atomic_units,
    );
    values.public[I_RECIPIENT_COMMITMENT] = values.public[I_CURRENT_COMMITMENT];
    values.public[I_RECIPIENT_NULLIFIER] = values.public[I_CURRENT_NULLIFIER];
    values.public[I_PREVIOUS_PROOF_STEP_COUNT] = Scalar::from(0);
    values.public[I_PREVIOUS_PEER_HOP_COUNT] = Scalar::from(0);
    values.public[I_PARENT_BRANCH_DEPTH] = Scalar::from(0);
    values.public[I_PARENT_BRANCH_PATH_BITS] = Scalar::from(0);
    values.validate_host_relation()?;
    Ok(values)
}

/// Build the exact public/witness relation for one append output branch.
pub fn kagemusha_recursive_spend_append_transition_values_v2(
    previous: &KagemushaRecursiveSpendPublicStatementV2,
    statement: &KagemushaRecursiveSpendPublicStatementV2,
    split: &KagemushaRecursiveSpendSplitIntentV2,
    branch: KagemushaRecursiveSpendBranchV2,
    step: &iroha_data_model::offline::KagemushaVerifiedFoldStep,
    parent_bundle_digest: [u8; 32],
    parent_topup_receipt_digest: [u8; 32],
    current_hop_domain_tag: [u8; 32],
) -> Result<KagemushaRecursiveSpendTransitionValuesV2, String> {
    previous.validate_context().map_err(|err| err.to_string())?;
    statement
        .validate_context()
        .map_err(|err| err.to_string())?;
    split
        .validate_public_binding()
        .map_err(|err| err.to_string())?;
    let Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(transition)) = &statement.transition
    else {
        return Err("Kagemusha V2 append statement must carry a peer-split transition".to_owned());
    };
    if transition.intent != *split || transition.branch != branch {
        return Err("Kagemusha V2 output statement split/branch mismatch".to_owned());
    }
    if transition.binding_digest != split.binding_digest().map_err(|err| err.to_string())? {
        return Err("Kagemusha V2 output statement split digest mismatch".to_owned());
    }
    if split.parent_lineage_digest != parent_bundle_digest {
        return Err("Kagemusha V2 split parent bundle digest mismatch".to_owned());
    }
    let transfer =
        KagemushaConfidentialTransferPublicInputsV2::from_proof(&step.attachment.proof.bytes)?;
    let mut values = KagemushaRecursiveSpendTransitionValuesV2::default();
    fill_common_statement_values(
        &mut values.public,
        statement,
        current_hop_domain_tag,
        parent_topup_receipt_digest,
    )?;
    fill_transfer_values(&mut values.public, transfer)?;
    fill_record_values(&mut values.public, step)?;
    values.public[I_APPEND_PROFILE] = Scalar::from(1);
    values.public[I_BRANCH_CHANGE] = branch_selector(branch);
    values.public[I_HAS_CHANGE] = Scalar::from(u64::from(split.change_output.is_some()));
    values.public[I_PREVIOUS_PROOF_STEP_COUNT] = Scalar::from(u64::from(previous.proof_step_count));
    values.public[I_PREVIOUS_PEER_HOP_COUNT] = Scalar::from(u64::from(previous.peer_hop_count));
    values.public[I_PARENT_BRANCH_DEPTH] = Scalar::from(u64::from(split.parent_branch_path.depth));
    values.public[I_PARENT_BRANCH_PATH_BITS] =
        Scalar::from(path_bits_as_u64(split.parent_branch_path.path_bits));
    values.public[I_PARENT_FINAL_ROOT] =
        scalar_from_canonical_bytes(&previous.final_root, "parent final root")?;
    write_limb_group(
        &mut values.public,
        I_PARENT_BRANCH_LINEAGE_ROOT,
        &split.parent_branch_path.lineage_root,
    );
    write_limb_group(
        &mut values.public,
        I_SPLIT_DIGEST,
        &split.binding_digest().map_err(|err| err.to_string())?,
    );
    write_limb_group(
        &mut values.public,
        I_RECIPIENT_REQUEST_DIGEST,
        &split.recipient_request_digest,
    );
    write_limb_group(&mut values.public, I_OPERATION_ID, &split.operation_id);
    write_limb_group(
        &mut values.public,
        I_PARENT_BUNDLE_DIGEST,
        &parent_bundle_digest,
    );
    write_limb_group(
        &mut values.public,
        I_PARENT_TOPUP_RECEIPT_DIGEST,
        &parent_topup_receipt_digest,
    );
    values.public[I_INPUT_SCALE] = Scalar::from(u64::from(split.input_note.amount.scale));
    values.public[I_TRANSFER_SCALE] = Scalar::from(u64::from(split.transfer_amount.scale));
    values.public[I_RECIPIENT_SCALE] = Scalar::from(u64::from(split.recipient_output.amount.scale));
    values.public[I_CHANGE_SCALE] = split
        .change_output
        .as_ref()
        .map_or(Scalar::from(0), |change| {
            Scalar::from(u64::from(change.amount.scale))
        });
    write_amount(
        &mut values.public,
        I_INPUT_AMOUNT_LO,
        split.input_note.amount.atomic_units,
    );
    write_amount(
        &mut values.public,
        I_TRANSFER_AMOUNT_LO,
        split.transfer_amount.atomic_units,
    );
    write_amount(
        &mut values.public,
        I_RECIPIENT_AMOUNT_LO,
        split.recipient_output.amount.atomic_units,
    );
    write_amount(
        &mut values.public,
        I_CHANGE_AMOUNT_LO,
        split
            .change_output
            .as_ref()
            .map_or(0, |change| change.amount.atomic_units),
    );
    values.public[I_INPUT_COMMITMENT] =
        scalar_from_canonical_bytes(&split.input_note.note_commitment, "split input commitment")?;
    values.public[I_INPUT_NULLIFIER] =
        scalar_from_canonical_bytes(&split.input_note.spend_nullifier, "split input nullifier")?;
    values.public[I_RECIPIENT_COMMITMENT] = scalar_from_canonical_bytes(
        &split.recipient_output.note_commitment,
        "recipient output commitment",
    )?;
    values.public[I_RECIPIENT_NULLIFIER] = scalar_from_canonical_bytes(
        &split.recipient_output.spend_nullifier,
        "recipient output nullifier",
    )?;
    if let Some(change) = &split.change_output {
        values.public[I_CHANGE_COMMITMENT] =
            scalar_from_canonical_bytes(&change.note_commitment, "change output commitment")?;
        values.public[I_CHANGE_NULLIFIER] =
            scalar_from_canonical_bytes(&change.spend_nullifier, "change output nullifier")?;
    }
    let record_swap = output_swap_for_target(
        &step.output_commitments,
        split.recipient_output.note_commitment,
    )?;
    let transfer_swap = output_swap_for_target(
        &transfer.output_commitments,
        split.recipient_output.note_commitment,
    )?;
    values.public[I_RECORD_OUTPUT_SWAP] = Scalar::from(u64::from(record_swap));
    values.public[I_TRANSFER_OUTPUT_SWAP] = Scalar::from(u64::from(transfer_swap));
    set_amount_carry(&mut values, split);
    set_path_selector(&mut values, split.parent_branch_path.depth)?;
    values.validate_host_relation()?;
    Ok(values)
}

/// Build the exact public/witness relation for a partial-redemption change child.
pub fn kagemusha_recursive_spend_redeem_change_transition_values_v2(
    previous: &KagemushaRecursiveSpendPublicStatementV2,
    statement: &KagemushaRecursiveSpendPublicStatementV2,
    redemption: &KagemushaRecursiveSpendRedemptionIntentV2,
    step: &iroha_data_model::offline::KagemushaVerifiedFoldStep,
    parent_bundle_digest: [u8; 32],
    parent_topup_receipt_digest: [u8; 32],
    current_hop_domain_tag: [u8; 32],
) -> Result<KagemushaRecursiveSpendTransitionValuesV2, String> {
    previous.validate_context().map_err(|err| err.to_string())?;
    statement
        .validate_context()
        .map_err(|err| err.to_string())?;
    redemption
        .validate_public_binding()
        .map_err(|err| err.to_string())?;
    let Some(KagemushaRecursiveSpendTransitionV2::RedemptionChange(transition)) =
        &statement.transition
    else {
        return Err(
            "Kagemusha V2 redeem-change statement must carry a redemption transition".to_owned(),
        );
    };
    let binding_digest = redemption.binding_digest().map_err(|err| err.to_string())?;
    if transition.intent != *redemption || transition.binding_digest != binding_digest {
        return Err("Kagemusha V2 redeem-change statement transition mismatch".to_owned());
    }
    if redemption.parent_bundle_digest != parent_bundle_digest {
        return Err("Kagemusha V2 redemption parent bundle digest mismatch".to_owned());
    }
    let change = redemption
        .change_output
        .as_ref()
        .ok_or_else(|| "Kagemusha V2 redeem-change requires a change output".to_owned())?;
    if statement.current_note != *change
        || statement.branch_path
            != redemption
                .parent_branch_path
                .child(KagemushaRecursiveSpendBranchV2::Change)
                .map_err(|err| err.to_string())?
    {
        return Err("Kagemusha V2 redeem-change output branch mismatch".to_owned());
    }
    if step.root_before != redemption.input_root
        || step.root_after != statement.final_root
        || step.input_nullifiers.as_slice() != [redemption.input_note.spend_nullifier]
        || step.output_commitments.as_slice() != [change.note_commitment]
    {
        return Err("Kagemusha V2 redeem-change record step mismatch".to_owned());
    }

    let unshield =
        KagemushaConfidentialUnshieldPublicInputsV3::from_proof(&step.attachment.proof.bytes)?;
    let binding = redemption.unshield_public_inputs;
    if unshield.input_commitments
        != [binding.input_commitment_0, binding.input_commitment_1]
        || unshield.nullifiers != [binding.nullifier_0, binding.nullifier_1]
        || unshield.change_commitment != binding.change_output_commitment
        || unshield.root != binding.root
        || unshield.public_amount != binding.public_amount
        || unshield.asset_tag != binding.asset_tag
        || unshield.chain_tag != binding.chain_tag
    {
        return Err("Kagemusha V2 unshield-v3 public-input binding mismatch".to_owned());
    }

    let mut values = KagemushaRecursiveSpendTransitionValuesV2::default();
    fill_common_statement_values(
        &mut values.public,
        statement,
        current_hop_domain_tag,
        parent_topup_receipt_digest,
    )?;
    fill_record_values(&mut values.public, step)?;
    values.public[I_REDEMPTION_PROFILE] = Scalar::from(1);
    values.public[I_BRANCH_CHANGE] = Scalar::from(1);
    values.public[I_HAS_CHANGE] = Scalar::from(1);
    values.public[I_PREVIOUS_PROOF_STEP_COUNT] = Scalar::from(u64::from(previous.proof_step_count));
    values.public[I_PREVIOUS_PEER_HOP_COUNT] = Scalar::from(u64::from(previous.peer_hop_count));
    values.public[I_PARENT_BRANCH_DEPTH] =
        Scalar::from(u64::from(redemption.parent_branch_path.depth));
    values.public[I_PARENT_BRANCH_PATH_BITS] =
        Scalar::from(path_bits_as_u64(redemption.parent_branch_path.path_bits));
    values.public[I_PARENT_FINAL_ROOT] =
        scalar_from_canonical_bytes(&previous.final_root, "parent final root")?;
    write_limb_group(
        &mut values.public,
        I_PARENT_BRANCH_LINEAGE_ROOT,
        &redemption.parent_branch_path.lineage_root,
    );
    write_limb_group(&mut values.public, I_SPLIT_DIGEST, &binding_digest);
    write_limb_group(
        &mut values.public,
        I_OPERATION_ID,
        &redemption.operation_id,
    );
    write_limb_group(
        &mut values.public,
        I_PARENT_BUNDLE_DIGEST,
        &parent_bundle_digest,
    );
    write_limb_group(
        &mut values.public,
        I_PARENT_TOPUP_RECEIPT_DIGEST,
        &parent_topup_receipt_digest,
    );
    write_limb_group(
        &mut values.public,
        I_REDEMPTION_RECIPIENT_DIGEST,
        &canonical_poseidon_digest(&redemption.recipient)?,
    );
    write_limb_group(
        &mut values.public,
        I_UNSHIELD_PUBLIC_INPUTS_DIGEST,
        &redemption.unshield_public_inputs_digest,
    );

    let scale = u64::from(statement.asset_scale);
    values.public[I_INPUT_SCALE] = Scalar::from(scale);
    values.public[I_TRANSFER_SCALE] = Scalar::from(scale);
    values.public[I_RECIPIENT_SCALE] = Scalar::from(scale);
    values.public[I_CHANGE_SCALE] = Scalar::from(scale);
    write_amount(
        &mut values.public,
        I_INPUT_AMOUNT_LO,
        redemption.input_note.amount.atomic_units,
    );
    write_amount(
        &mut values.public,
        I_TRANSFER_AMOUNT_LO,
        redemption.public_amount.atomic_units,
    );
    write_amount(
        &mut values.public,
        I_RECIPIENT_AMOUNT_LO,
        redemption.public_amount.atomic_units,
    );
    write_amount(
        &mut values.public,
        I_CHANGE_AMOUNT_LO,
        change.amount.atomic_units,
    );
    values.public[I_INPUT_COMMITMENT] = scalar_from_canonical_bytes(
        &redemption.input_note.note_commitment,
        "redemption input commitment",
    )?;
    values.public[I_INPUT_NULLIFIER] = scalar_from_canonical_bytes(
        &redemption.input_note.spend_nullifier,
        "redemption input nullifier",
    )?;
    values.public[I_CHANGE_COMMITMENT] =
        scalar_from_canonical_bytes(&change.note_commitment, "redemption change commitment")?;
    values.public[I_CHANGE_NULLIFIER] =
        scalar_from_canonical_bytes(&change.spend_nullifier, "redemption change nullifier")?;

    values.public[I_TRANSFER_INPUT_COUNT] = Scalar::from(1);
    values.public[I_TRANSFER_OUTPUT_COUNT] = Scalar::from(1);
    values.public[I_TRANSFER_INPUT_COMMITMENT_0] =
        scalar_from_canonical_bytes(&unshield.input_commitments[0], "unshield input commitment")?;
    values.public[I_TRANSFER_INPUT_COMMITMENT_1] =
        scalar_from_canonical_bytes(&unshield.input_commitments[1], "unshield input commitment")?;
    values.public[I_TRANSFER_NULLIFIER_0] =
        scalar_from_canonical_bytes(&unshield.nullifiers[0], "unshield nullifier")?;
    values.public[I_TRANSFER_NULLIFIER_1] =
        scalar_from_canonical_bytes(&unshield.nullifiers[1], "unshield nullifier")?;
    values.public[I_TRANSFER_OUTPUT_0] =
        scalar_from_canonical_bytes(&unshield.change_commitment, "unshield change commitment")?;
    values.public[I_TRANSFER_ROOT] = scalar_from_canonical_bytes(&unshield.root, "unshield root")?;
    values.public[I_ASSET_TAG] =
        scalar_from_canonical_bytes(&unshield.asset_tag, "unshield asset tag")?;
    values.public[I_CHAIN_TAG] =
        scalar_from_canonical_bytes(&unshield.chain_tag, "unshield chain tag")?;
    values.public[I_UNSHIELD_PUBLIC_AMOUNT] =
        scalar_from_canonical_bytes(&unshield.public_amount, "unshield public amount")?;

    let public_low = redemption.public_amount.atomic_units as u64;
    let change_low = change.amount.atomic_units as u64;
    values.amount_low_carry = Scalar::from(u64::from(public_low.checked_add(change_low).is_none()));
    set_path_selector(&mut values, redemption.parent_branch_path.depth)?;
    values.validate_host_relation()?;
    Ok(values)
}

/// Return the single public instance column for a V2 transition witness.
#[must_use]
pub fn kagemusha_recursive_spend_transition_instance_column_v2(
    values: &KagemushaRecursiveSpendTransitionValuesV2,
) -> Vec<Scalar> {
    values.public.to_vec()
}

type OneHopLineageCircuit<const LEN: usize> =
    super::pasta_tiny::KagemushaRecursiveAggregationOneHopVerifierSlice<
        LEN,
        { super::KAGEMUSHA_RECURSIVE_VESTA_IPA_WINDOWS },
        { super::KAGEMUSHA_RECURSIVE_VESTA_IPA_WINDOW_BITS },
    >;
type AppendLineageCircuit<const LEN: usize> =
    super::pasta_tiny::KagemushaRecursiveAggregationAppendVerifierSlice<
        LEN,
        { super::KAGEMUSHA_RECURSIVE_VESTA_IPA_WINDOWS },
        { super::KAGEMUSHA_RECURSIVE_VESTA_IPA_WINDOW_BITS },
    >;
type OneHopLineageKeygenShape<const LEN: usize> =
    super::pasta_tiny::KagemushaRecursiveAggregationOneHopVerifierSliceKeygenShape<
        LEN,
        { super::KAGEMUSHA_RECURSIVE_VESTA_IPA_WINDOWS },
        { super::KAGEMUSHA_RECURSIVE_VESTA_IPA_WINDOW_BITS },
    >;
type AppendLineageKeygenShape<const LEN: usize> =
    super::pasta_tiny::KagemushaRecursiveAggregationAppendVerifierSliceKeygenShape<
        LEN,
        { super::KAGEMUSHA_RECURSIVE_VESTA_IPA_WINDOWS },
        { super::KAGEMUSHA_RECURSIVE_VESTA_IPA_WINDOW_BITS },
    >;

/// Composite configuration for a V2 init proof.
#[derive(Clone)]
pub struct KagemushaRecursiveSpendInitConfigV2 {
    lineage: <OneHopLineageCircuit<2> as Circuit<Scalar>>::Config,
    transition: KagemushaRecursiveSpendTransitionConfigV2,
}

/// Composite configuration for a V2 append proof.
#[derive(Clone)]
pub struct KagemushaRecursiveSpendAppendConfigV2 {
    lineage: <AppendLineageCircuit<2> as Circuit<Scalar>>::Config,
    transition: KagemushaRecursiveSpendTransitionConfigV2,
}

/// Composite configuration for the dedicated V2 redemption-change proof.
#[derive(Clone)]
pub struct KagemushaRecursiveSpendRedeemChangeConfigV2 {
    lineage: <AppendLineageCircuit<2> as Circuit<Scalar>>::Config,
    transition: KagemushaRecursiveSpendTransitionConfigV2,
}

// The verifier-slice config does not depend on LEN at the Rust type level, so
// the `<2>` aliases above are usable for every supported const-generic width.

/// V2 Reserved init circuit: one checked confidential hop plus exact V2 state.
#[derive(Clone, Default)]
pub struct KagemushaRecursiveSpendInitCircuitV2<const LEN: usize> {
    /// Real one-hop non-native IPA verifier slice.
    pub lineage: OneHopLineageCircuit<LEN>,
    /// Exact amount, note, operation, and branch relation.
    pub transition: KagemushaRecursiveSpendTransitionCircuitV2,
}

/// V2 Reserved append circuit: previous recursive proof, checked hop, and split.
#[derive(Clone, Default)]
pub struct KagemushaRecursiveSpendAppendCircuitV2<const LEN: usize> {
    /// Real two-opening non-native IPA verifier slice.
    pub lineage: AppendLineageCircuit<LEN>,
    /// Exact amount, note, operation, and branch relation.
    pub transition: KagemushaRecursiveSpendTransitionCircuitV2,
}

/// V2 Reserved partial-redemption circuit: previous proof, checked unshield,
/// and the sole surviving change branch.
#[derive(Clone, Default)]
pub struct KagemushaRecursiveSpendRedeemChangeCircuitV2<const LEN: usize> {
    /// Real two-opening non-native IPA verifier slice.
    pub lineage: AppendLineageCircuit<LEN>,
    /// Exact public-credit and change conservation relation.
    pub transition: KagemushaRecursiveSpendTransitionCircuitV2,
}

/// Keygen-only shape for the V2 init circuit.
#[derive(Clone, Copy, Debug, Default)]
pub struct KagemushaRecursiveSpendInitKeygenShapeV2<const LEN: usize>;

/// Keygen-only shape for the V2 append circuit.
#[derive(Clone, Copy, Debug, Default)]
pub struct KagemushaRecursiveSpendAppendKeygenShapeV2<const LEN: usize>;

/// Keygen-only shape for the V2 redemption-change circuit.
#[derive(Clone, Copy, Debug, Default)]
pub struct KagemushaRecursiveSpendRedeemChangeKeygenShapeV2<const LEN: usize>;

impl<const LEN: usize> Circuit<Scalar> for KagemushaRecursiveSpendInitCircuitV2<LEN> {
    type Config = KagemushaRecursiveSpendInitConfigV2;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        KagemushaRecursiveSpendInitConfigV2 {
            lineage: OneHopLineageCircuit::<LEN>::configure(meta),
            transition: KagemushaRecursiveSpendTransitionCircuitV2::configure(meta),
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), PlonkError> {
        self.lineage
            .synthesize(config.lineage, layouter.namespace(|| "v2_init_lineage"))?;
        self.transition.synthesize(
            config.transition,
            layouter.namespace(|| "v2_init_transition"),
        )
    }
}

impl<const LEN: usize> Circuit<Scalar> for KagemushaRecursiveSpendAppendCircuitV2<LEN> {
    type Config = KagemushaRecursiveSpendAppendConfigV2;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        KagemushaRecursiveSpendAppendConfigV2 {
            lineage: AppendLineageCircuit::<LEN>::configure(meta),
            transition: KagemushaRecursiveSpendTransitionCircuitV2::configure(meta),
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), PlonkError> {
        self.lineage
            .synthesize(config.lineage, layouter.namespace(|| "v2_append_lineage"))?;
        self.transition.synthesize(
            config.transition,
            layouter.namespace(|| "v2_append_transition"),
        )
    }
}

impl<const LEN: usize> Circuit<Scalar> for KagemushaRecursiveSpendRedeemChangeCircuitV2<LEN> {
    type Config = KagemushaRecursiveSpendRedeemChangeConfigV2;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        KagemushaRecursiveSpendRedeemChangeConfigV2 {
            lineage: AppendLineageCircuit::<LEN>::configure(meta),
            transition: KagemushaRecursiveSpendTransitionCircuitV2::configure(meta),
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), PlonkError> {
        self.lineage.synthesize(
            config.lineage,
            layouter.namespace(|| "v2_redeem_change_lineage"),
        )?;
        self.transition.synthesize(
            config.transition,
            layouter.namespace(|| "v2_redeem_change_transition"),
        )
    }
}

impl<const LEN: usize> Circuit<Scalar> for KagemushaRecursiveSpendInitKeygenShapeV2<LEN> {
    type Config = KagemushaRecursiveSpendInitConfigV2;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        *self
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        KagemushaRecursiveSpendInitCircuitV2::<LEN>::configure(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), PlonkError> {
        OneHopLineageKeygenShape::<LEN>::default()
            .synthesize(config.lineage, layouter.namespace(|| "v2_init_lineage"))?;
        KagemushaRecursiveSpendTransitionCircuitV2::default().synthesize(
            config.transition,
            layouter.namespace(|| "v2_init_transition"),
        )
    }
}

impl<const LEN: usize> Circuit<Scalar> for KagemushaRecursiveSpendAppendKeygenShapeV2<LEN> {
    type Config = KagemushaRecursiveSpendAppendConfigV2;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        *self
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        KagemushaRecursiveSpendAppendCircuitV2::<LEN>::configure(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), PlonkError> {
        AppendLineageKeygenShape::<LEN>::default()
            .synthesize(config.lineage, layouter.namespace(|| "v2_append_lineage"))?;
        KagemushaRecursiveSpendTransitionCircuitV2::default().synthesize(
            config.transition,
            layouter.namespace(|| "v2_append_transition"),
        )
    }
}

impl<const LEN: usize> Circuit<Scalar>
    for KagemushaRecursiveSpendRedeemChangeKeygenShapeV2<LEN>
{
    type Config = KagemushaRecursiveSpendRedeemChangeConfigV2;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        *self
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        KagemushaRecursiveSpendRedeemChangeCircuitV2::<LEN>::configure(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), PlonkError> {
        AppendLineageKeygenShape::<LEN>::default().synthesize(
            config.lineage,
            layouter.namespace(|| "v2_redeem_change_lineage"),
        )?;
        KagemushaRecursiveSpendTransitionCircuitV2::default().synthesize(
            config.transition,
            layouter.namespace(|| "v2_redeem_change_transition"),
        )
    }
}

fn init_instance_columns<const LEN: usize>(
    circuit: &KagemushaRecursiveSpendInitCircuitV2<LEN>,
) -> Result<Vec<Vec<Scalar>>, String> {
    let mut columns =
        super::kagemusha_recursive_one_hop_verifier_slice_instance_columns(&circuit.lineage)?;
    columns.push(kagemusha_recursive_spend_transition_instance_column_v2(
        &circuit.transition.values,
    ));
    Ok(super::kagemusha_rectangular_pasta_instance_columns(columns))
}

fn append_instance_columns<const LEN: usize>(
    circuit: &KagemushaRecursiveSpendAppendCircuitV2<LEN>,
) -> Result<Vec<Vec<Scalar>>, String> {
    let mut columns =
        super::kagemusha_recursive_append_verifier_slice_instance_columns(&circuit.lineage)?;
    columns.push(kagemusha_recursive_spend_transition_instance_column_v2(
        &circuit.transition.values,
    ));
    Ok(super::kagemusha_rectangular_pasta_instance_columns(columns))
}

fn redeem_change_instance_columns<const LEN: usize>(
    circuit: &KagemushaRecursiveSpendRedeemChangeCircuitV2<LEN>,
) -> Result<Vec<Vec<Scalar>>, String> {
    let mut columns =
        super::kagemusha_recursive_append_verifier_slice_instance_columns(&circuit.lineage)?;
    columns.push(kagemusha_recursive_spend_transition_instance_column_v2(
        &circuit.transition.values,
    ));
    Ok(super::kagemusha_rectangular_pasta_instance_columns(columns))
}

/// Exact artifact type string accepted by V2 references.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACT_TYPE_V2: &str =
    "KagemushaRecursiveSpendLineageKeyArtifactsV2";
/// Streaming archive format version.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACT_VERSION_V2: u16 = 2;
/// Bridge ABI release bound into every V2 key package.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACT_ABI_V2: u32 = 17;

const KEY_ARTIFACT_MAGIC_V2: &[u8; 8] = b"KRV2KEY\0";
const KEY_ARTIFACT_MAX_HEADER_BYTES_V2: usize = 64 * 1024 * 1024;
const KEY_ARTIFACT_MAX_TOTAL_BYTES_V2: u64 = 4 * 1024 * 1024 * 1024;

/// Canonical small header of the streamed V2 lineage key package.
///
/// The processed proving key immediately follows this Norito header. It is
/// intentionally not a `Vec<u8>` field, so decoding never materializes a
/// second copy of a release-sized proving key.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaRecursiveSpendLineageKeyArtifactsV2 {
    /// Package format version.
    pub version: u16,
    /// Semantic proving role.
    pub role: KagemushaRecursiveSpendArtifactRoleV2,
    /// Human/audit purpose bound to the role.
    pub purpose: String,
    /// Exact V2 circuit id.
    pub circuit_id: String,
    /// Content-addressed release generation.
    pub generation: String,
    /// Source revision used by the release builder.
    pub source_commit: String,
    /// Native bridge ABI version.
    pub bridge_abi_version: u32,
    /// Pallas polynomial opening width verified recursively.
    pub verifier_opening_len: u32,
    /// Halo2 IPA domain exponent.
    pub ipa_k: u32,
    /// Hash committed by the V2 verifier record.
    pub public_inputs_schema_hash: [u8; 32],
    /// Active record containing the exact inline V2 verifier key.
    pub verifier_record: VerifyingKeyRecord,
    /// Commitment of `verifier_record.key`.
    pub verifier_key_commitment: [u8; 32],
    /// Exact processed proving-key payload length following this header.
    pub proving_key_size_bytes: u64,
    /// SHA-256 of only the processed proving-key payload.
    pub proving_key_sha256: [u8; 32],
}

fn artifact_role_contract(
    role: KagemushaRecursiveSpendArtifactRoleV2,
) -> Result<(&'static str, &'static str), String> {
    match role {
        KagemushaRecursiveSpendArtifactRoleV2::LineageInitProver => Ok((
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_RESERVED_INIT_PROOF_CIRCUIT_ID_V2,
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_PURPOSE_LINEAGE_INIT_V2,
        )),
        KagemushaRecursiveSpendArtifactRoleV2::LineageAppendProver => Ok((
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_RESERVED_APPEND_PROOF_CIRCUIT_ID_V2,
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_PURPOSE_LINEAGE_APPEND_V2,
        )),
        KagemushaRecursiveSpendArtifactRoleV2::RedeemChangeProver => Ok((
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_RESERVED_REDEEM_CHANGE_PROOF_CIRCUIT_ID_V2,
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_PURPOSE_REDEEM_CHANGE_V2,
        )),
        _ => Err("Kagemusha V2 artifact role is not a recursive lineage prover".to_owned()),
    }
}

/// Hash used by V2 verifier records and artifact headers.
#[must_use]
pub fn kagemusha_recursive_spend_v2_public_inputs_schema_hash() -> [u8; 32] {
    iroha_crypto::Hash::new(KAGEMUSHA_RECURSIVE_SPEND_V2_PUBLIC_INPUTS_SCHEMA).into()
}

impl KagemushaRecursiveSpendLineageKeyArtifactsV2 {
    /// Validate all small/header bindings before a proving-key payload is read.
    pub fn validate_header(&self) -> Result<(), String> {
        let (expected_circuit, expected_purpose) = artifact_role_contract(self.role)?;
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACT_VERSION_V2 {
            return Err(format!(
                "Kagemusha V2 key artifact version {} is not {}",
                self.version, KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACT_VERSION_V2
            ));
        }
        if self.circuit_id != expected_circuit || self.purpose != expected_purpose {
            return Err("Kagemusha V2 key artifact role/circuit/purpose mismatch".to_owned());
        }
        if self.generation.is_empty()
            || self.generation.len() > 128
            || self.generation.trim() != self.generation
            || self.generation.chars().any(char::is_control)
        {
            return Err("Kagemusha V2 key artifact generation is invalid".to_owned());
        }
        if self.source_commit.is_empty()
            || self.source_commit.len() > 128
            || self.source_commit.trim() != self.source_commit
            || self.source_commit.chars().any(char::is_control)
        {
            return Err("Kagemusha V2 key artifact source commit is invalid".to_owned());
        }
        if self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACT_ABI_V2 {
            return Err("Kagemusha V2 key artifact bridge ABI mismatch".to_owned());
        }
        iroha_data_model::offline::validate_kagemusha_recursive_verifier_opening_len(
            self.verifier_opening_len,
        )
        .map_err(|err| err.to_string())?;
        if self.ipa_k != super::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_IPA_K {
            return Err("Kagemusha V2 key artifact IPA domain mismatch".to_owned());
        }
        let expected_schema_hash = kagemusha_recursive_spend_v2_public_inputs_schema_hash();
        if self.public_inputs_schema_hash != expected_schema_hash
            || self.verifier_record.public_inputs_schema_hash != expected_schema_hash
        {
            return Err("Kagemusha V2 key artifact public-input schema mismatch".to_owned());
        }
        let record = &self.verifier_record;
        if record.namespace != iroha_data_model::offline::KAGEMUSHA_VERIFIER_NAMESPACE
            || record.circuit_id != self.circuit_id
            || record.backend != BackendTag::Halo2IpaPasta
            || record.curve != "pallas"
            || record.commitment == [0; 32]
            || record.commitment != self.verifier_key_commitment
            || !record.status.is_active()
            || record.max_proof_bytes == 0
        {
            return Err("Kagemusha V2 key artifact verifier record mismatch".to_owned());
        }
        let vk_box = record.key.as_ref().ok_or_else(|| {
            "Kagemusha V2 key artifact verifier record has no inline key".to_owned()
        })?;
        if vk_box.backend != super::ZK_BACKEND_HALO2_IPA
            || vk_box.bytes.is_empty()
            || u32::try_from(vk_box.bytes.len()).ok() != Some(record.vk_len)
            || super::hash_vk(vk_box) != self.verifier_key_commitment
        {
            return Err("Kagemusha V2 key artifact verifier key mismatch".to_owned());
        }
        let ipa_k =
            super::zk1::ensure_halo2_ipa_vk_envelope_shape_any_k(&vk_box.bytes, &self.circuit_id)
                .map_err(|err| format!("Kagemusha V2 key artifact verifier envelope {err}"))?;
        if ipa_k != self.ipa_k {
            return Err("Kagemusha V2 key artifact verifier IPAK mismatch".to_owned());
        }
        if self.proving_key_size_bytes == 0 || self.proving_key_sha256 == [0; 32] {
            return Err("Kagemusha V2 key artifact proving-key metadata is empty".to_owned());
        }
        Ok(())
    }

    /// Validate this header against an external content-addressed reference.
    pub fn validate_reference(
        &self,
        reference: &KagemushaRecursiveSpendArtifactReferenceV2,
    ) -> Result<(), String> {
        self.validate_header()?;
        reference
            .validate_for_role(self.role)
            .map_err(|err| err.to_string())?;
        if reference.artifact_type != KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACT_TYPE_V2
            || reference.circuit_id != self.circuit_id
            || reference.generation != self.generation
        {
            return Err("Kagemusha V2 artifact reference/header mismatch".to_owned());
        }
        Ok(())
    }
}

/// Construct an active V2 verifier record from an exact generated key.
pub fn kagemusha_recursive_spend_v2_vk_record_from_box(
    version: u32,
    role: KagemushaRecursiveSpendArtifactRoleV2,
    verifier_opening_len: u32,
    vk_box: VerifyingKeyBox,
) -> Result<VerifyingKeyRecord, String> {
    let (circuit_id, _) = artifact_role_contract(role)?;
    iroha_data_model::offline::validate_kagemusha_recursive_verifier_opening_len(
        verifier_opening_len,
    )
    .map_err(|err| err.to_string())?;
    if vk_box.backend != super::ZK_BACKEND_HALO2_IPA || vk_box.bytes.is_empty() {
        return Err("Kagemusha V2 verifier key must use non-empty halo2/ipa bytes".to_owned());
    }
    let ipa_k = super::zk1::ensure_halo2_ipa_vk_envelope_shape_any_k(&vk_box.bytes, circuit_id)
        .map_err(|err| format!("Kagemusha V2 verifier key {err}"))?;
    if ipa_k != super::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_IPA_K {
        return Err("Kagemusha V2 verifier key IPA domain mismatch".to_owned());
    }
    let vk_len = u32::try_from(vk_box.bytes.len())
        .map_err(|_| "Kagemusha V2 verifier key length exceeds u32".to_owned())?;
    let commitment = super::hash_vk(&vk_box);
    let mut record = VerifyingKeyRecord::new(
        version,
        circuit_id,
        BackendTag::Halo2IpaPasta,
        "pallas",
        kagemusha_recursive_spend_v2_public_inputs_schema_hash(),
        commitment,
    );
    record.namespace = iroha_data_model::offline::KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
    record.vk_len = vk_len;
    record.max_proof_bytes = super::KAGEMUSHA_RECURSIVE_AGGREGATION_MAX_PROOF_BYTES;
    record.gas_schedule_id = Some("halo2_default".to_owned());
    record.key = Some(vk_box);
    record.status = ConfidentialStatus::Active;
    Ok(record)
}

/// Write a canonical V2 streaming package without a Norito proving-key vector.
pub fn write_kagemusha_recursive_spend_lineage_key_artifact_v2<W: Write>(
    writer: &mut W,
    role: KagemushaRecursiveSpendArtifactRoleV2,
    generation: impl Into<String>,
    source_commit: impl Into<String>,
    verifier_opening_len: u32,
    verifier_record: VerifyingKeyRecord,
    proving_key: &[u8],
) -> Result<
    (
        KagemushaRecursiveSpendLineageKeyArtifactsV2,
        KagemushaRecursiveSpendArtifactReferenceV2,
    ),
    String,
> {
    let (circuit_id, purpose) = artifact_role_contract(role)?;
    let proving_key_size_bytes = u64::try_from(proving_key.len())
        .map_err(|_| "Kagemusha V2 proving key length exceeds u64".to_owned())?;
    let proving_key_sha256: [u8; 32] = Sha256::digest(proving_key).into();
    let header = KagemushaRecursiveSpendLineageKeyArtifactsV2 {
        version: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACT_VERSION_V2,
        role,
        purpose: purpose.to_owned(),
        circuit_id: circuit_id.to_owned(),
        generation: generation.into(),
        source_commit: source_commit.into(),
        bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACT_ABI_V2,
        verifier_opening_len,
        ipa_k: super::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_IPA_K,
        public_inputs_schema_hash: kagemusha_recursive_spend_v2_public_inputs_schema_hash(),
        verifier_key_commitment: verifier_record.commitment,
        verifier_record,
        proving_key_size_bytes,
        proving_key_sha256,
    };
    header.validate_header()?;
    let header_bytes = norito::to_bytes(&header)
        .map_err(|err| format!("failed to encode Kagemusha V2 key artifact header: {err}"))?;
    if header_bytes.len() > KEY_ARTIFACT_MAX_HEADER_BYTES_V2 {
        return Err("Kagemusha V2 key artifact header exceeds bounded size".to_owned());
    }
    let header_len = u32::try_from(header_bytes.len())
        .map_err(|_| "Kagemusha V2 key artifact header length exceeds u32".to_owned())?;
    let total_size = u64::try_from(KEY_ARTIFACT_MAGIC_V2.len() + 4)
        .ok()
        .and_then(|prefix| prefix.checked_add(u64::from(header_len)))
        .and_then(|prefix| prefix.checked_add(proving_key_size_bytes))
        .ok_or_else(|| "Kagemusha V2 key artifact total size overflow".to_owned())?;
    if total_size > KEY_ARTIFACT_MAX_TOTAL_BYTES_V2 {
        return Err("Kagemusha V2 key artifact exceeds release size bound".to_owned());
    }
    let header_len_bytes = header_len.to_le_bytes();
    let mut archive_hasher = Sha256::new();
    for bytes in [
        KEY_ARTIFACT_MAGIC_V2.as_slice(),
        header_len_bytes.as_slice(),
        header_bytes.as_slice(),
        proving_key,
    ] {
        writer
            .write_all(bytes)
            .map_err(|err| format!("failed to write Kagemusha V2 key artifact: {err}"))?;
        archive_hasher.update(bytes);
    }
    let reference = KagemushaRecursiveSpendArtifactReferenceV2 {
        role,
        generation: header.generation.clone(),
        circuit_id: header.circuit_id.clone(),
        artifact_type: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACT_TYPE_V2.to_owned(),
        size_bytes: total_size,
        sha256: archive_hasher.finalize().into(),
    };
    header.validate_reference(&reference)?;
    Ok((header, reference))
}

struct CountingSha256Reader<R> {
    inner: R,
    count: u64,
    hasher: Sha256,
}

impl<R> CountingSha256Reader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            count: 0,
            hasher: Sha256::new(),
        }
    }

    fn digest(self) -> [u8; 32] {
        self.hasher.finalize().into()
    }
}

impl<R: Read> Read for CountingSha256Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.count = self
            .count
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| std::io::Error::other("Kagemusha V2 artifact byte count overflow"))?;
        self.hasher.update(&buf[..read]);
        Ok(read)
    }
}

struct BoundedPayloadReader<'a, R> {
    inner: &'a mut CountingSha256Reader<R>,
    remaining: u64,
    hasher: Sha256,
}

impl<'a, R> BoundedPayloadReader<'a, R> {
    fn new(inner: &'a mut CountingSha256Reader<R>, remaining: u64) -> Self {
        Self {
            inner,
            remaining,
            hasher: Sha256::new(),
        }
    }

    fn finish(self) -> Result<[u8; 32], String> {
        if self.remaining != 0 {
            return Err(format!(
                "Kagemusha V2 proving-key decoder left {} payload bytes unread",
                self.remaining
            ));
        }
        Ok(self.hasher.finalize().into())
    }
}

impl<R: Read> Read for BoundedPayloadReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining == 0 {
            return Ok(0);
        }
        let limit = usize::try_from(self.remaining)
            .unwrap_or(usize::MAX)
            .min(buf.len());
        let read = self.inner.read(&mut buf[..limit])?;
        self.remaining -= u64::try_from(read).unwrap_or(0);
        self.hasher.update(&buf[..read]);
        Ok(read)
    }
}

struct LoadedLineageKeyArtifactV2 {
    header: KagemushaRecursiveSpendLineageKeyArtifactsV2,
    params: super::halo2_backend::PastaParams,
    verifying_key: super::halo2_backend::VerifyingKey,
    proving_key: super::halo2_backend::ProvingKey,
}

fn read_key_payload_for_circuit<C, R: Read>(
    reader: &mut CountingSha256Reader<R>,
    header: KagemushaRecursiveSpendLineageKeyArtifactsV2,
) -> Result<LoadedLineageKeyArtifactV2, String>
where
    C: Circuit<Scalar>,
    C::Params: Default,
{
    let vk_box = header
        .verifier_record
        .key
        .as_ref()
        .expect("validated V2 artifact header has inline key");
    let params = super::zkparse::params_any(&vk_box.bytes)
        .ok_or_else(|| "Kagemusha V2 artifact has invalid IPAK parameters".to_owned())?;
    let verifying_key = super::zkparse::vk_from_bytes::<C>(&vk_box.bytes, &params)
        .ok_or_else(|| "Kagemusha V2 artifact has invalid H2VK payload".to_owned())?;
    let mut payload = BoundedPayloadReader::new(reader, header.proving_key_size_bytes);
    let proving_key = super::read_proving_key::<C, _>(&mut payload)
        .map_err(|err| format!("failed to stream-decode Kagemusha V2 proving key: {err}"))?;
    let payload_sha256 = payload.finish()?;
    if payload_sha256 != header.proving_key_sha256 {
        return Err("Kagemusha V2 proving-key payload SHA-256 mismatch".to_owned());
    }
    if super::halo2_backend::proving_key_domain_k(&proving_key) != header.ipa_k
        || super::halo2_backend::proving_key_vk_to_processed_bytes(&proving_key)
            != super::halo2_backend::verifying_key_to_processed_bytes(&verifying_key)
    {
        return Err("Kagemusha V2 proving/verifying key pair mismatch".to_owned());
    }
    Ok(LoadedLineageKeyArtifactV2 {
        header,
        params,
        verifying_key,
        proving_key,
    })
}

fn read_lineage_key_artifact_v2<R: Read>(
    reference: &KagemushaRecursiveSpendArtifactReferenceV2,
    expected_role: KagemushaRecursiveSpendArtifactRoleV2,
    reader: R,
) -> Result<LoadedLineageKeyArtifactV2, String> {
    reference
        .validate_for_role(expected_role)
        .map_err(|err| err.to_string())?;
    if reference.size_bytes == 0 || reference.size_bytes > KEY_ARTIFACT_MAX_TOTAL_BYTES_V2 {
        return Err("Kagemusha V2 artifact reference size is outside the release bound".to_owned());
    }
    let mut reader = CountingSha256Reader::new(reader);
    let mut magic = [0u8; 8];
    reader
        .read_exact(&mut magic)
        .map_err(|err| format!("failed to read Kagemusha V2 artifact magic: {err}"))?;
    if &magic != KEY_ARTIFACT_MAGIC_V2 {
        return Err("Kagemusha V2 artifact magic mismatch".to_owned());
    }
    let mut header_len_bytes = [0u8; 4];
    reader
        .read_exact(&mut header_len_bytes)
        .map_err(|err| format!("failed to read Kagemusha V2 artifact header length: {err}"))?;
    let header_len = usize::try_from(u32::from_le_bytes(header_len_bytes))
        .map_err(|_| "Kagemusha V2 artifact header length overflow".to_owned())?;
    if header_len == 0 || header_len > KEY_ARTIFACT_MAX_HEADER_BYTES_V2 {
        return Err("Kagemusha V2 artifact header exceeds bounded size".to_owned());
    }
    let mut header_bytes = vec![0u8; header_len];
    reader
        .read_exact(&mut header_bytes)
        .map_err(|err| format!("failed to read Kagemusha V2 artifact header: {err}"))?;
    let header: KagemushaRecursiveSpendLineageKeyArtifactsV2 =
        norito::decode_from_bytes(&header_bytes)
            .map_err(|err| format!("failed to decode Kagemusha V2 artifact header: {err}"))?;
    if header.role != expected_role {
        return Err("Kagemusha V2 artifact reader role mismatch".to_owned());
    }
    header.validate_reference(reference)?;
    let expected_total = u64::try_from(KEY_ARTIFACT_MAGIC_V2.len() + 4)
        .ok()
        .and_then(|prefix| prefix.checked_add(u64::try_from(header_len).ok()?))
        .and_then(|prefix| prefix.checked_add(header.proving_key_size_bytes))
        .ok_or_else(|| "Kagemusha V2 artifact declared size overflow".to_owned())?;
    if expected_total != reference.size_bytes {
        return Err("Kagemusha V2 artifact exact size does not match reference".to_owned());
    }

    macro_rules! read_for_len {
        ($circuit:ident) => {
            match header.verifier_opening_len {
                2 => read_key_payload_for_circuit::<$circuit<2>, _>(&mut reader, header),
                4 => read_key_payload_for_circuit::<$circuit<4>, _>(&mut reader, header),
                8 => read_key_payload_for_circuit::<$circuit<8>, _>(&mut reader, header),
                16 => read_key_payload_for_circuit::<$circuit<16>, _>(&mut reader, header),
                32 => read_key_payload_for_circuit::<$circuit<32>, _>(&mut reader, header),
                64 => read_key_payload_for_circuit::<$circuit<64>, _>(&mut reader, header),
                128 => read_key_payload_for_circuit::<$circuit<128>, _>(&mut reader, header),
                other => Err(format!(
                    "Kagemusha V2 artifact opening length {other} is unsupported"
                )),
            }
        };
    }
    let loaded = match expected_role {
        KagemushaRecursiveSpendArtifactRoleV2::LineageInitProver => {
            read_for_len!(KagemushaRecursiveSpendInitCircuitV2)
        }
        KagemushaRecursiveSpendArtifactRoleV2::LineageAppendProver => {
            read_for_len!(KagemushaRecursiveSpendAppendCircuitV2)
        }
        KagemushaRecursiveSpendArtifactRoleV2::RedeemChangeProver => {
            read_for_len!(KagemushaRecursiveSpendRedeemChangeCircuitV2)
        }
        _ => Err("Kagemusha V2 artifact reader role is not implemented".to_owned()),
    }?;
    if reader.count != reference.size_bytes {
        return Err("Kagemusha V2 artifact reader byte count mismatch".to_owned());
    }
    let mut trailing = [0u8; 1];
    if reader
        .read(&mut trailing)
        .map_err(|err| format!("failed to check Kagemusha V2 artifact EOF: {err}"))?
        != 0
    {
        return Err("Kagemusha V2 artifact has trailing bytes".to_owned());
    }
    if reader.digest() != reference.sha256 {
        return Err("Kagemusha V2 artifact archive SHA-256 mismatch".to_owned());
    }
    Ok(loaded)
}

/// Fully stream-validate a V2 package and return its authenticated small header.
pub fn validate_kagemusha_recursive_spend_lineage_key_artifact_v2<R: Read>(
    reference: &KagemushaRecursiveSpendArtifactReferenceV2,
    reader: R,
) -> Result<KagemushaRecursiveSpendLineageKeyArtifactsV2, String> {
    Ok(read_lineage_key_artifact_v2(reference, reference.role, reader)?.header)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_append_values() -> KagemushaRecursiveSpendTransitionValuesV2 {
        let mut values = KagemushaRecursiveSpendTransitionValuesV2::default();
        let p = &mut values.public;
        p[I_LAYOUT_VERSION] = Scalar::from(KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION);
        p[I_APPEND_PROFILE] = Scalar::from(1);
        p[I_HAS_CHANGE] = Scalar::from(1);
        p[I_PROOF_STEP_COUNT] = Scalar::from(2);
        p[I_PEER_HOP_COUNT] = Scalar::from(1);
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

    #[test]
    fn transition_relation_accepts_conserving_recipient_branch() {
        let values = valid_append_values();
        let instances = vec![values.public.to_vec()];
        let prover = halo2_proofs::dev::MockProver::run(
            9,
            &KagemushaRecursiveSpendTransitionCircuitV2 { values },
            instances,
        )
        .expect("mock prover");
        prover.assert_satisfied();
    }

    #[test]
    fn transition_relation_rejects_non_conservation() {
        let mut values = valid_append_values();
        write_amount(&mut values.public, I_CHANGE_AMOUNT_LO, 61);
        let instances = vec![values.public.to_vec()];
        let prover = halo2_proofs::dev::MockProver::run(
            9,
            &KagemushaRecursiveSpendTransitionCircuitV2 { values },
            instances,
        )
        .expect("mock prover");
        assert!(prover.verify().is_err());
    }

    #[test]
    fn transition_relation_binds_sibling_branch_and_path() {
        let mut values = valid_append_values();
        values.public[I_BRANCH_CHANGE] = Scalar::from(1);
        values.public[I_CURRENT_COMMITMENT] = Scalar::from(41);
        values.public[I_CURRENT_NULLIFIER] = Scalar::from(42);
        write_amount(&mut values.public, I_CURRENT_AMOUNT_LO, 60);
        values.public[I_BRANCH_PATH_BITS] = Scalar::from(1u64 << 63);
        let instances = vec![values.public.to_vec()];
        let prover = halo2_proofs::dev::MockProver::run(
            9,
            &KagemushaRecursiveSpendTransitionCircuitV2 { values },
            instances,
        )
        .expect("mock prover");
        prover.assert_satisfied();
    }
}
