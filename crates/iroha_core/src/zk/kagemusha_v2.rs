//! Branch-safe fractional Kagemusha recursive-spend V2 backend.
//!
//! The V2 circuit deliberately keeps the large recursive IPA verifier slice
//! separate from the compact transition relation.  The latter is exposed as
//! one fixed-height instance column.  Consequently, adding an independently
//! spendable recipient or change branch changes neither the circuit shape nor
//! the proof size.
//!
//! This relation is a fail-closed scaffold, not an available proof backend.
//! A split has one branch-independent statement and proof. The relation binds
//! both output commitments into a fresh canonical output accumulator; sibling
//! bundles select independently spendable leaves with verified membership
//! paths into that exact root.

use ff::PrimeField;
use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    halo2curves::pasta::Fp as Scalar,
    plonk::{Circuit, ConstraintSystem, Error as PlonkError, Expression, Selector},
    poly::Rotation,
};
use iroha_data_model::{
    offline::{
        KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2, KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
        KagemushaRecursiveSpendPublicStatementV2, KagemushaRecursiveSpendTransitionV2,
    },
    proof::VerifyingKeyRecord,
    zk::BackendTag,
};
use norito::codec::{Decode, Encode};

use super::assign_advice_compat;

/// Public-input schema for the branch-safe V2 transition relation.
///
/// All entries are encoded as consecutive rows in one Pasta instance column.
/// The schema is hashed into the verifier record and the streamed proving-key
/// package; changing an offset therefore requires a new circuit generation.
pub const KAGEMUSHA_RECURSIVE_SPEND_V2_PUBLIC_INPUTS_SCHEMA: &[u8] = br#"{"schema":"kagemusha_recursive_spend_v2","layout":"single_column_rows_v3","binds":["shared_transition_statement_digest","chain_id","asset_definition_id","asset_scale","single_parent_bundle_digest","confidential_transfer_v2_public_inputs","recipient_and_change_commitments","fresh_output_accumulator_root","recipient_request_digest","operation_ids","optional_change","proof_step_count","peer_hop_count","artifact_manifest_sha256","verifier_key_id"]}"#;

/// Version of the fixed transition instance layout.
pub const KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION: u64 = 3;

// The transition column is intentionally explicit.  Keeping named offsets
// makes envelope validation fail closed when fields are added or reordered.
const I_LAYOUT_VERSION: usize = 0;
const I_APPEND_PROFILE: usize = I_LAYOUT_VERSION + 1;
const I_HAS_CHANGE: usize = I_APPEND_PROFILE + 1;
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
const I_RECORD_INPUT_COUNT: usize = I_CHANGE_SCALE + 1;
const I_RECORD_OUTPUT_COUNT: usize = I_RECORD_INPUT_COUNT + 1;
const I_TRANSFER_INPUT_COUNT: usize = I_RECORD_OUTPUT_COUNT + 1;
const I_TRANSFER_OUTPUT_COUNT: usize = I_TRANSFER_INPUT_COUNT + 1;
const I_INPUT_AMOUNT_LO: usize = I_TRANSFER_OUTPUT_COUNT + 1;
const I_INPUT_AMOUNT_HI: usize = I_INPUT_AMOUNT_LO + 1;
const I_TRANSFER_AMOUNT_LO: usize = I_INPUT_AMOUNT_HI + 1;
const I_TRANSFER_AMOUNT_HI: usize = I_TRANSFER_AMOUNT_LO + 1;
const I_RECIPIENT_AMOUNT_LO: usize = I_TRANSFER_AMOUNT_HI + 1;
const I_RECIPIENT_AMOUNT_HI: usize = I_RECIPIENT_AMOUNT_LO + 1;
const I_CHANGE_AMOUNT_LO: usize = I_RECIPIENT_AMOUNT_HI + 1;
const I_CHANGE_AMOUNT_HI: usize = I_CHANGE_AMOUNT_LO + 1;
const I_BRANCH_PATH_BITS: usize = I_CHANGE_AMOUNT_HI + 1;
const I_PARENT_BRANCH_PATH_BITS: usize = I_BRANCH_PATH_BITS + 1;
const I_FINAL_ROOT: usize = I_PARENT_BRANCH_PATH_BITS + 1;
const I_RECORD_ROOT_BEFORE: usize = I_FINAL_ROOT + 1;
const I_RECORD_ROOT_AFTER: usize = I_RECORD_ROOT_BEFORE + 1;
const I_TRANSFER_ROOT: usize = I_RECORD_ROOT_AFTER + 1;
const I_INPUT_COMMITMENT: usize = I_TRANSFER_ROOT + 1;
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
const I_PARENT_BRANCH_CLAIM_DIGEST: usize = I_PARENT_BUNDLE_DIGEST + 4;
const I_BRANCH_LINEAGE_ROOT: usize = I_PARENT_BRANCH_CLAIM_DIGEST + 4;
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

/// Number of rows in the V2 transition public-instance column.
pub const KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS: usize = I_UNSHIELD_PUBLIC_AMOUNT + 1;

const PATH_SELECTOR_COUNT: usize = 64;
const PEER_HOP_SELECTOR_COUNT: usize = KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2 as usize + 1;
const OUTPUT_ACCUMULATOR_DEPTH: usize = KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2;

/// Fixed public and private witness values for one V2 output branch.
#[derive(Clone, Debug)]
pub struct KagemushaRecursiveSpendTransitionValuesV2<F: PrimeField = Scalar> {
    /// Consecutive public rows described by
    /// [`KAGEMUSHA_RECURSIVE_SPEND_V2_PUBLIC_INPUTS_SCHEMA`].
    pub public: [F; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS],
    /// Carry from the low 64-bit limb of recipient + change.
    amount_low_carry: F,
    /// One-hot selector for the parent branch depth on append.
    path_depth_selectors: [F; PATH_SELECTOR_COUNT],
    /// One-hot selector constraining the current peer-hop count to `0..=64`.
    peer_hop_selectors: [F; PEER_HOP_SELECTOR_COUNT],
    /// Poseidon nodes from the two output leaves to the fresh output root.
    output_root_nodes: [F; OUTPUT_ACCUMULATOR_DEPTH],
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
            output_root_nodes: [F::ZERO; OUTPUT_ACCUMULATOR_DEPTH],
        }
    }
}

impl<F: PrimeField> KagemushaRecursiveSpendTransitionValuesV2<F> {
    fn validate_host_relation(&self) -> Result<(), String> {
        let value = |index: usize| self.public[index];
        let zero = F::ZERO;
        let one = F::ONE;
        for (index, field) in [
            (I_APPEND_PROFILE, "append_profile"),
            (I_REDEMPTION_PROFILE, "redemption_profile"),
            (I_HAS_CHANGE, "has_change"),
            (I_RECORD_OUTPUT_SWAP, "record_output_swap"),
            (I_TRANSFER_OUTPUT_SWAP, "transfer_output_swap"),
        ] {
            if value(index) != zero && value(index) != one {
                return Err(format!("Kagemusha V2 {field} must be boolean"));
            }
        }
        if value(I_LAYOUT_VERSION) != F::from(KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION)
        {
            return Err("Kagemusha V2 transition layout version mismatch".to_owned());
        }
        if value(I_APPEND_PROFILE) == one && value(I_REDEMPTION_PROFILE) == one {
            return Err("Kagemusha V2 transition profiles are mutually exclusive".to_owned());
        }
        if value(I_PROOF_STEP_COUNT) != value(I_BRANCH_DEPTH) + one {
            return Err("Kagemusha V2 lineage proof-step/depth mismatch".to_owned());
        }
        if value(I_REDEMPTION_PROFILE) == one && value(I_HAS_CHANGE) != one {
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
        let peer_hop_selector_sum = self
            .peer_hop_selectors
            .iter()
            .copied()
            .fold(zero, |sum, selector| sum + selector);
        let peer_hop_count = self.peer_hop_selectors.iter().copied().enumerate().fold(
            zero,
            |sum, (hop, selector)| {
                sum + selector * F::from(u64::try_from(hop).expect("peer hop fits u64"))
            },
        );
        if self
            .peer_hop_selectors
            .iter()
            .any(|selector| *selector != zero && *selector != one)
            || peer_hop_selector_sum != one
            || peer_hop_count != value(I_PEER_HOP_COUNT)
        {
            return Err("Kagemusha V2 peer-hop count exceeds the 64-hop bound".to_owned());
        }
        Ok(())
    }
}

/// Constraint-system columns for the V2 transition relation.
#[derive(Clone, Copy)]
pub struct KagemushaRecursiveSpendTransitionConfigV2 {
    public_advice: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    amount_low_carry: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    path_depth_selector: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    peer_hop_selector: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    output_root_nodes:
        [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; OUTPUT_ACCUMULATOR_DEPTH],
    relation: Selector,
}

/// Exact branch-safe split circuit shared by init and append compositions.
#[derive(Clone, Debug, Default)]
pub struct KagemushaRecursiveSpendTransitionCircuitV2<F: PrimeField = Scalar> {
    /// Fixed transition witness and public values.
    pub values: KagemushaRecursiveSpendTransitionValuesV2<F>,
}

fn query_at<F: PrimeField>(
    meta: &mut halo2_proofs::plonk::VirtualCells<'_, F>,
    column: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    index: usize,
) -> Expression<F> {
    meta.query_advice(
        column,
        Rotation(i32::try_from(index).expect("V2 transition row offset fits i32")),
    )
}

fn query_instance_at<F: PrimeField>(
    meta: &mut halo2_proofs::plonk::VirtualCells<'_, F>,
    column: halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
    index: usize,
) -> Expression<F> {
    meta.query_instance(
        column,
        Rotation(i32::try_from(index).expect("V2 transition row offset fits i32")),
    )
}

fn select_expression<F: PrimeField>(
    first: Expression<F>,
    second: Expression<F>,
    selector: Expression<F>,
) -> Expression<F> {
    first.clone() + selector * (second - first)
}

fn output_poseidon_pair<F: PrimeField>(lhs: F, rhs: F) -> F {
    let lhs = lhs + F::from(7);
    let rhs = rhs + F::from(13);
    let lhs_sq = lhs * lhs;
    let lhs_fourth = lhs_sq * lhs_sq;
    let rhs_sq = rhs * rhs;
    let rhs_fourth = rhs_sq * rhs_sq;
    F::from(2) * (lhs_fourth * lhs) + F::from(3) * (rhs_fourth * rhs)
}

fn output_poseidon_pair_expression<F: PrimeField>(
    lhs: Expression<F>,
    rhs: Expression<F>,
) -> Expression<F> {
    let lhs = lhs + Expression::Constant(F::from(7));
    let rhs = rhs + Expression::Constant(F::from(13));
    let lhs_sq = lhs.clone() * lhs.clone();
    let lhs_fourth = lhs_sq.clone() * lhs_sq;
    let rhs_sq = rhs.clone() * rhs.clone();
    let rhs_fourth = rhs_sq.clone() * rhs_sq;
    Expression::Constant(F::from(2)) * (lhs_fourth * lhs)
        + Expression::Constant(F::from(3)) * (rhs_fourth * rhs)
}

fn output_accumulator_nodes<F: PrimeField>(
    recipient: F,
    change: F,
) -> [F; OUTPUT_ACCUMULATOR_DEPTH] {
    let mut nodes = [F::ZERO; OUTPUT_ACCUMULATOR_DEPTH];
    let mut node = output_poseidon_pair(recipient, change);
    nodes[0] = node;
    let mut zero_subtree = F::ZERO;
    for output_node in nodes.iter_mut().skip(1) {
        zero_subtree = output_poseidon_pair(zero_subtree, zero_subtree);
        node = output_poseidon_pair(node, zero_subtree);
        *output_node = node;
    }
    nodes
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
        let public_advice = meta.advice_column();
        let public_instance = meta.instance_column();
        let amount_low_carry = meta.advice_column();
        let path_depth_selector = meta.advice_column();
        let peer_hop_selector = meta.advice_column();
        let output_root_nodes = std::array::from_fn(|_| meta.advice_column());
        let relation = meta.selector();

        meta.create_gate("kagemusha_recursive_spend_v2_transition", |meta| {
            let enabled = meta.query_selector(relation);
            let public = (0..KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS)
                .map(|index| query_at(meta, public_advice, index))
                .collect::<Vec<_>>();
            let p = |index: usize| public[index].clone();
            let one = Expression::Constant(F::ONE);
            let zero = Expression::Constant(F::ZERO);
            let two_pow_64 = Expression::Constant(F::from_u128(1u128 << 64));
            let append = p(I_APPEND_PROFILE);
            let redemption = p(I_REDEMPTION_PROFILE);
            let extends = append.clone() + redemption.clone();
            let has_change = p(I_HAS_CHANGE);
            let record_swap = p(I_RECORD_OUTPUT_SWAP);
            let transfer_swap = p(I_TRANSFER_OUTPUT_SWAP);
            let carry = meta.query_advice(amount_low_carry, Rotation::cur());
            let output_nodes = output_root_nodes
                .iter()
                .map(|column| meta.query_advice(*column, Rotation::cur()))
                .collect::<Vec<_>>();

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
                        - Expression::Constant(F::from(
                            KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION,
                        ))),
            );
            for boolean in [
                append.clone(),
                redemption.clone(),
                has_change.clone(),
                record_swap.clone(),
                transfer_swap.clone(),
                carry.clone(),
            ] {
                constraints.push(enabled.clone() * boolean.clone() * (boolean - one.clone()));
            }
            constraints.push(enabled.clone() * append.clone() * redemption.clone());
            constraints
                .push(enabled.clone() * redemption.clone() * (one.clone() - has_change.clone()));
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
                enabled.clone() * (p(I_PROOF_STEP_COUNT) - p(I_BRANCH_DEPTH) - one.clone()),
                enabled.clone() * (one.clone() - extends.clone()) * p(I_PREVIOUS_PROOF_STEP_COUNT),
                enabled.clone() * (one.clone() - extends.clone()) * p(I_PREVIOUS_PEER_HOP_COUNT),
                enabled.clone() * (one.clone() - extends.clone()) * p(I_PARENT_BRANCH_DEPTH),
            ]);

            // Peer transfers use the exact 64-hop first-release capacity.
            // Redemption-change transitions can extend a branch without adding
            // a peer hop, so the proof-step and peer-hop selectors stay distinct.
            let mut peer_hop_selector_sum = zero.clone();
            let mut selected_peer_hop = zero.clone();
            for hop in 0..PEER_HOP_SELECTOR_COUNT {
                let selector = meta.query_advice(
                    peer_hop_selector,
                    Rotation(i32::try_from(hop).expect("peer-hop selector row fits i32")),
                );
                constraints
                    .push(enabled.clone() * selector.clone() * (selector.clone() - one.clone()));
                peer_hop_selector_sum = peer_hop_selector_sum + selector.clone();
                selected_peer_hop = selected_peer_hop
                    + selector
                        * Expression::Constant(F::from(
                            u64::try_from(hop).expect("peer hop fits u64"),
                        ));
            }
            constraints.extend([
                enabled.clone() * (peer_hop_selector_sum - one.clone()),
                enabled.clone() * (selected_peer_hop - p(I_PEER_HOP_COUNT)),
            ]);

            // Every amount uses the authoritative asset scale.  An absent
            // change uses canonical all-zero scale/amount/note fields.
            constraints.extend([
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
                enabled.clone() * (one.clone() - has_change.clone()) * p(I_CHANGE_AMOUNT_LO),
                enabled.clone() * (one.clone() - has_change.clone()) * p(I_CHANGE_AMOUNT_HI),
                enabled.clone() * (one.clone() - has_change.clone()) * p(I_CHANGE_COMMITMENT),
                enabled.clone() * (one.clone() - has_change.clone()) * p(I_CHANGE_NULLIFIER),
            ]);

            // Every transition creates a fresh two-output accumulator. The
            // recipient is canonical leaf 0, optional change is leaf 1, and
            // every remaining leaf is empty. This prevents reusing the parent
            // input root and makes either sibling path independently checkable
            // against the exact proof-bound `final_root`.
            let first_output_node =
                output_poseidon_pair_expression(p(I_RECIPIENT_COMMITMENT), p(I_CHANGE_COMMITMENT));
            constraints.push(enabled.clone() * (output_nodes[0].clone() - first_output_node));
            let mut previous_output_node = output_nodes[0].clone();
            let mut zero_subtree = F::ZERO;
            for output_node in output_nodes.iter().skip(1) {
                zero_subtree = output_poseidon_pair(zero_subtree, zero_subtree);
                let expected_output_node = output_poseidon_pair_expression(
                    previous_output_node,
                    Expression::Constant(zero_subtree),
                );
                constraints.push(enabled.clone() * (output_node.clone() - expected_output_node));
                previous_output_node = output_node.clone();
            }
            constraints.push(enabled.clone() * (previous_output_node - p(I_FINAL_ROOT)));

            // The record-backed fold step and the confidential-transfer V2
            // public inputs must describe the same roots and proof arity.
            constraints.extend([
                enabled.clone() * (p(I_RECORD_ROOT_BEFORE) - p(I_PARENT_FINAL_ROOT)),
                enabled.clone() * (p(I_RECORD_ROOT_AFTER) - p(I_FINAL_ROOT)),
                enabled.clone() * (p(I_TRANSFER_ROOT) - p(I_PARENT_FINAL_ROOT)),
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
                enabled.clone() * redemption.clone() * (p(I_RECORD_OUTPUT_COUNT) - one.clone()),
                enabled.clone() * redemption.clone() * (p(I_TRANSFER_OUTPUT_COUNT) - one.clone()),
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
                    * (record_selected - p(I_RECIPIENT_COMMITMENT)),
                enabled.clone()
                    * (one.clone() - extends.clone())
                    * (transfer_selected - p(I_RECIPIENT_COMMITMENT)),
                enabled.clone() * (one.clone() - extends.clone()) * (record_other - transfer_other),
                enabled.clone()
                    * (one.clone() - extends.clone())
                    * (p(I_RECORD_OUTPUT_COUNT) - p(I_TRANSFER_OUTPUT_COUNT)),
            ]);

            // The shared peer proof uses the canonical recipient edge (0);
            // redemption-change uses the change edge (1). Output projections
            // reconstruct the corresponding parent prefix from their claim.
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
                        * Expression::Constant(F::from(
                            u64::try_from(depth).expect("path depth fits u64"),
                        ));
                selected_mask =
                    selected_mask + selector * Expression::Constant(F::from(1u64 << (63 - depth)));
            }
            constraints.extend([
                enabled.clone() * (selector_sum - extends.clone()),
                enabled.clone() * (selected_depth - p(I_PARENT_BRANCH_DEPTH)),
                enabled.clone()
                    * (p(I_BRANCH_PATH_BITS)
                        - p(I_PARENT_BRANCH_PATH_BITS)
                        - redemption.clone() * selected_mask),
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
            constraints.extend([enabled.clone()
                * (one.clone() - redemption.clone())
                * p(I_UNSHIELD_PUBLIC_AMOUNT)]);
            constraints
        });

        KagemushaRecursiveSpendTransitionConfigV2 {
            public_advice,
            amount_low_carry,
            path_depth_selector,
            peer_hop_selector,
            output_root_nodes,
            relation,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
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
                for (row, selector) in values.peer_hop_selectors.iter().copied().enumerate() {
                    assign_advice_compat(
                        &mut region,
                        move || format!("peer_hop_selector_{row}"),
                        config.peer_hop_selector,
                        row,
                        || Value::known(selector),
                    )?;
                }
                for (level, value) in values.output_root_nodes.iter().copied().enumerate() {
                    assign_advice_compat(
                        &mut region,
                        move || format!("output_root_node_{level}"),
                        config.output_root_nodes[level],
                        0,
                        || Value::known(value),
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

fn fill_common_statement_values(
    public: &mut [Scalar; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS],
    statement: &KagemushaRecursiveSpendPublicStatementV2,
    current_hop_domain_tag: [u8; 32],
    topup_receipt_digest: [u8; 32],
) -> Result<(), String> {
    statement
        .validate_public_binding()
        .map_err(|err| err.to_string())?;
    let [topup_anchor_ref] = statement.topup_anchor_refs.as_slice() else {
        return Err(
            "current Kagemusha V2 transition layout cannot represent multiple top-up origins"
                .to_owned(),
        );
    };
    public[I_LAYOUT_VERSION] = Scalar::from(KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION);
    public[I_PROOF_STEP_COUNT] = Scalar::from(u64::from(statement.proof_step_count));
    public[I_PEER_HOP_COUNT] = Scalar::from(u64::from(statement.peer_hop_count));
    public[I_BRANCH_DEPTH] = Scalar::from(u64::from(statement.proof_step_count.saturating_sub(1)));
    public[I_ASSET_SCALE] = Scalar::from(u64::from(statement.asset_scale));
    public[I_PARENT_FINAL_ROOT] = scalar_from_canonical_bytes(&statement.input_root, "input root")?;
    public[I_FINAL_ROOT] = scalar_from_canonical_bytes(&statement.final_root, "final root")?;
    write_limb_group(
        public,
        I_STATEMENT_DIGEST,
        &statement.digest().map_err(|err| err.to_string())?,
    );
    write_limb_group(
        public,
        I_BRANCH_LINEAGE_ROOT,
        &topup_anchor_ref.anchor_digest,
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
    fill_common_statement_values(&mut expected, statement, [0; 32], [0; 32])?;
    expected[I_ASSET_TAG] = scalar_from_canonical_bytes(
        &super::confidential_v2::derive_confidential_asset_tag_v2(&statement.asset.to_string()),
        "statement asset tag",
    )?;
    expected[I_CHAIN_TAG] = scalar_from_canonical_bytes(
        &super::confidential_v2::derive_confidential_chain_tag_v2(statement.chain_id.as_str()),
        "statement chain tag",
    )?;

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
        (I_PARENT_FINAL_ROOT, "input root"),
        (I_FINAL_ROOT, "final root"),
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
    let require_shared_parent = || -> Result<(), String> {
        let parent_depth = statement
            .proof_step_count
            .checked_sub(2)
            .ok_or_else(|| "Kagemusha V2 extending transition has no parent depth".to_owned())?;
        require_value(
            I_PARENT_BRANCH_DEPTH,
            Scalar::from(u64::from(parent_depth)),
            "parent branch depth",
        )?;
        require_value(
            I_PARENT_BRANCH_PATH_BITS,
            Scalar::from(0),
            "shared parent path",
        )?;
        require_limbs(
            I_PARENT_BRANCH_LINEAGE_ROOT,
            &statement.topup_anchor_refs[0].anchor_digest,
            "parent branch lineage root",
        )
    };

    match &statement.transition {
        None => {
            for (index, field) in [
                (I_APPEND_PROFILE, "append profile"),
                (I_REDEMPTION_PROFILE, "redemption profile"),
                (I_HAS_CHANGE, "init change selector"),
            ] {
                require_value(index, zero, field)?;
            }
            for (start, field) in [
                (I_SPLIT_DIGEST, "init split digest"),
                (I_RECIPIENT_REQUEST_DIGEST, "init recipient request"),
                (I_OPERATION_ID, "init transition operation"),
                (I_PARENT_BUNDLE_DIGEST, "init parent bundle"),
                (I_PARENT_BRANCH_CLAIM_DIGEST, "init parent branch claim"),
            ] {
                require_limbs(start, &[0; 32], field)?;
            }
        }
        Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(split)) => {
            require_value(I_APPEND_PROFILE, one, "append profile")?;
            require_value(I_REDEMPTION_PROFILE, zero, "redemption profile")?;
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
            require_limbs(
                I_PARENT_BRANCH_CLAIM_DIGEST,
                &split.parent_branch_claim_digest,
                "peer-split parent branch claim digest",
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
            require_shared_parent()?;
        }
        Some(KagemushaRecursiveSpendTransitionV2::RedemptionChange(redemption)) => {
            require_value(I_APPEND_PROFILE, zero, "append profile")?;
            require_value(I_REDEMPTION_PROFILE, one, "redemption profile")?;
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
                I_PARENT_BRANCH_CLAIM_DIGEST,
                &redemption.parent_branch_claim_digest,
                "redemption parent branch claim digest",
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
            require_shared_parent()?;
        }
    }
    Ok(())
}

/// Return the single public instance column for a V2 transition witness.
#[must_use]
pub fn kagemusha_recursive_spend_transition_instance_column_v2<F: PrimeField>(
    values: &KagemushaRecursiveSpendTransitionValuesV2<F>,
) -> Vec<F> {
    values.public.to_vec()
}

/// Recompute the real and canonical-empty membership paths for one output.
///
/// The shared transition circuit fixes recipient at leaf 0, optional change at
/// leaf 1, and every remaining leaf to zero. Therefore a valid path into the
/// proof-bound `final_root` is an unambiguous output-membership proof rather
/// than a structural path against the consumed parent root.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn verify_kagemusha_output_membership_v2(
    note_commitment: [u8; 32],
    branch: iroha_data_model::offline::KagemushaRecursiveSpendBranchV2,
    final_root: [u8; 32],
    witness: &iroha_data_model::offline::KagemushaNoteMembershipWitnessV2,
) -> Result<(), String> {
    use iroha_data_model::offline::KagemushaRecursiveSpendBranchV2;

    witness
        .validate_for_root(final_root)
        .map_err(|error| error.to_string())?;
    let expected_leaf_index = match branch {
        KagemushaRecursiveSpendBranchV2::Recipient => 0,
        KagemushaRecursiveSpendBranchV2::Change => 1,
    };
    if witness.leaf_index != expected_leaf_index {
        return Err("Kagemusha output membership uses a non-canonical output leaf".to_owned());
    }
    let input_path = super::confidential_v2::ConfidentialMerklePathV2 {
        siblings: witness.input_path.siblings.clone(),
        directions: witness.input_path.directions.clone(),
        witness_nodes: Vec::new(),
        root: witness.input_path.root,
    };
    super::confidential_v2::normalize_supplied_confidential_merkle_path_v2(
        note_commitment,
        Some(usize::try_from(expected_leaf_index).expect("output leaf index fits usize")),
        &input_path,
        final_root,
        "Kagemusha proof-bound output path",
    )?;

    let first_sibling_is_empty = witness.input_path.siblings[0] == [0; 32];
    let expected_dummy_leaf_index = match branch {
        KagemushaRecursiveSpendBranchV2::Recipient if first_sibling_is_empty => 1,
        KagemushaRecursiveSpendBranchV2::Recipient | KagemushaRecursiveSpendBranchV2::Change => 2,
    };
    let dummy_leaf_index = witness
        .dummy_input_path
        .leaf_index()
        .map_err(|error| error.to_string())?;
    if dummy_leaf_index != expected_dummy_leaf_index {
        return Err("Kagemusha output membership uses a non-canonical empty leaf".to_owned());
    }
    let dummy_path = super::confidential_v2::ConfidentialMerklePathV2 {
        siblings: witness.dummy_input_path.siblings.clone(),
        directions: witness.dummy_input_path.directions.clone(),
        witness_nodes: Vec::new(),
        root: witness.dummy_input_path.root,
    };
    super::confidential_v2::normalize_supplied_confidential_merkle_path_v2(
        [0; 32],
        Some(usize::try_from(dummy_leaf_index).expect("dummy leaf index fits usize")),
        &dummy_path,
        final_root,
        "Kagemusha proof-bound empty-output path",
    )?;
    Ok(())
}

fn ensure_kagemusha_recursive_step_record(
    record: &VerifyingKeyRecord,
    parity: iroha_data_model::offline::KagemushaPastaCycleParityV3,
) -> Result<(), String> {
    use iroha_data_model::offline::{
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V3,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V3,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V3,
        KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_KEY_BACKEND_V3, KagemushaPastaCycleParityV3,
        kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v3,
        kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v3,
    };

    let (circuit_id, curve, schema_hash) = match parity {
        KagemushaPastaCycleParityV3::StepEq => (
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V3,
            kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v3(),
        ),
        KagemushaPastaCycleParityV3::StepEp => (
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V3,
            kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v3(),
        ),
    };
    let key = record
        .key
        .as_ref()
        .ok_or_else(|| "Kagemusha recursive-step verifier has no inline key".to_owned())?;
    if record.circuit_id != circuit_id
        || record.curve != curve
        || record.backend != BackendTag::Halo2IpaPasta
        || record.public_inputs_schema_hash != schema_hash
        || record.commitment == [0; 32]
        || record.max_proof_bytes
            < u32::try_from(
                super::kagemusha_recursion_adapter::KAGEMUSHA_LEAPFROG_STEP_PROOF_BYTES_V3,
            )
            .expect("fixed proof size fits u32")
        || record.max_proof_bytes > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3
        || key.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_KEY_BACKEND_V3
        || key.bytes.is_empty()
        || u32::try_from(key.bytes.len()).ok() != Some(record.vk_len)
        || super::hash_vk(key) != record.commitment
    {
        return Err("Kagemusha recursive-step verifier record mismatch".to_owned());
    }
    let ipa_k = super::zk1::ensure_halo2_ipa_vk_envelope_shape_any_k(&key.bytes, circuit_id)
        .map_err(|error| format!("Kagemusha recursive-step verifier key is invalid: {error}"))?;
    if ipa_k != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V3 {
        return Err("Kagemusha recursive-step verifier IPA domain mismatch".to_owned());
    }
    Ok(())
}

fn kagemusha_step_public_inputs_from_statement(
    statement: &KagemushaRecursiveSpendPublicStatementV2,
    predecessor_proof_sha256: [u8; 32],
    predecessor_deferred_equation_digest: [u8; 32],
) -> Result<super::kagemusha_recursion_adapter::KagemushaLeapfrogStepPublicInputsV3, String> {
    statement
        .validate_public_binding()
        .map_err(|error| error.to_string())?;
    let [anchor] = statement.topup_anchor_refs.as_slice() else {
        return Err("Kagemusha step requires exactly one top-up anchor".to_owned());
    };
    let (
        transition_profile,
        transition_binding_digest,
        recipient_request_digest,
        operation_id,
        parent_bundle_digest,
        parent_branch_claim_digest,
        parent_proof_step_count,
        parent_peer_hop_count,
    ) = match &statement.transition {
        None => (0, [0; 32], [0; 32], [0; 32], [0; 32], [0; 32], 0, 0),
        Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(transition)) => (
            1,
            transition.binding_digest,
            transition.recipient_request_digest,
            transition.operation_id,
            [0; 32],
            transition.parent_branch_claim_digest,
            transition.parent_max_proof_step_count,
            transition.parent_max_peer_hop_count,
        ),
        Some(KagemushaRecursiveSpendTransitionV2::RedemptionChange(transition)) => (
            2,
            transition.binding_digest,
            [0; 32],
            transition.operation_id,
            transition.parent_bundle_digest,
            transition.parent_branch_claim_digest,
            transition.parent_proof_step_count,
            transition.parent_peer_hop_count,
        ),
    };
    Ok(
        super::kagemusha_recursion_adapter::KagemushaLeapfrogStepPublicInputsV3 {
            chain_id_digest: canonical_poseidon_digest(&statement.chain_id)?,
            asset_definition_id_digest: canonical_poseidon_digest(&statement.asset)?,
            input_root: statement.input_root,
            final_root: statement.final_root,
            topup_operation_id: anchor.topup_operation_id,
            topup_anchor_digest: anchor.anchor_digest,
            transition_binding_digest,
            recipient_request_digest,
            operation_id,
            parent_bundle_digest,
            parent_branch_claim_digest,
            manifest_sha256: statement.artifact_binding.manifest_sha256,
            verifier_key_id_digest: canonical_poseidon_digest(&statement.verifier_key_id)?,
            predecessor_proof_sha256,
            predecessor_deferred_equation_digest,
            asset_scale: statement.asset_scale,
            proof_step_count: statement.proof_step_count,
            peer_hop_count: statement.peer_hop_count,
            transition_profile,
            parent_proof_step_count,
            parent_peer_hop_count,
        },
    )
}

fn ensure_kagemusha_step_public_inputs_statement_binding(
    inputs: &super::kagemusha_recursion_adapter::KagemushaLeapfrogStepPublicInputsV3,
    statement: &KagemushaRecursiveSpendPublicStatementV2,
) -> Result<(), String> {
    let expected = kagemusha_step_public_inputs_from_statement(
        statement,
        inputs.predecessor_proof_sha256,
        inputs.predecessor_deferred_equation_digest,
    )?;
    if inputs != &expected {
        return Err("Kagemusha recursive proof exact semantic instances mismatch".to_owned());
    }
    Ok(())
}

/// Validate the canonical compact proof window against both authenticated step records.
///
/// This helper binds the newest statement and the constant newest/predecessor
/// window. It does not grant cryptographic acceptance; consensus must call
/// [`verify_kagemusha_recursive_spend_v2_terminal`], which remains fail-closed
/// until both native proof decisions and the cross-layer deferred digest are
/// implemented.
pub fn ensure_kagemusha_recursive_spend_v2_proof_envelope_binding(
    bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV2,
    step_eq_record: &VerifyingKeyRecord,
    step_ep_record: &VerifyingKeyRecord,
) -> Result<(), String> {
    use iroha_data_model::offline::{
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3, KagemushaPastaCycleParityV3,
        kagemusha_recursive_spend_step_parity_v3,
    };

    bundle
        .validate_public_binding()
        .map_err(|error| error.to_string())?;
    ensure_kagemusha_recursive_step_record(step_eq_record, KagemushaPastaCycleParityV3::StepEq)?;
    ensure_kagemusha_recursive_step_record(step_ep_record, KagemushaPastaCycleParityV3::StepEp)?;

    let proof = &bundle.recursive_proof.proof;
    if proof.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3 {
        return Err("Kagemusha recursive proof backend mismatch".to_owned());
    }
    let window: super::kagemusha_recursion_adapter::KagemushaLeapfrogProofWindowV3 =
        norito::decode_from_bytes(&proof.bytes)
            .map_err(|_| "Kagemusha recursive proof window is malformed".to_owned())?;
    window.validate()?;
    let canonical = norito::to_bytes(&window)
        .map_err(|error| format!("failed to encode Kagemusha proof window: {error}"))?;
    if canonical != proof.bytes {
        return Err("Kagemusha recursive proof window is not canonical".to_owned());
    }

    let statement_digest = bundle
        .statement
        .digest()
        .map_err(|error| error.to_string())?;
    let expected_parity =
        kagemusha_recursive_spend_step_parity_v3(bundle.statement.proof_step_count)
            .map_err(|error| error.to_string())?;
    if window.newest.parity != expected_parity
        || bundle.recursive_proof.public_statement_digest != statement_digest
    {
        return Err("Kagemusha recursive proof window statement binding mismatch".to_owned());
    }
    ensure_kagemusha_step_public_inputs_statement_binding(
        &window.newest.public_inputs,
        &bundle.statement,
    )?;
    Ok(())
}

/// Perform the witnessless terminal decision for a compact Kagemusha proof window.
///
/// The function intentionally fails closed until the Poseidon verifier can
/// natively verify the newest and predecessor proofs under their parity keys,
/// reconstruct the predecessor residual, and compare its deferred digest. A
/// generic one-proof `OpenVerifyEnvelope` verifier is not sound for this wire.
pub fn verify_kagemusha_recursive_spend_v2_terminal(
    bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV2,
    step_eq_record: &VerifyingKeyRecord,
    step_ep_record: &VerifyingKeyRecord,
) -> Result<(), String> {
    ensure_kagemusha_recursive_spend_v2_proof_envelope_binding(
        bundle,
        step_eq_record,
        step_ep_record,
    )?;
    Err(
        "Kagemusha leapfrog terminal verifier is unavailable until both proof decisions and the deferred-equation check are wired"
            .to_owned(),
    )
}

/// Verify one spendable output projection, including its proof-bound path.
///
/// This is the receiver/redemption admission entry point. It deliberately
/// performs the terminal recursive decision before accepting the external path
/// that selects one sibling from the shared output accumulator.
pub fn verify_kagemusha_recursive_spend_v2_output(
    bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV2,
    membership_witness: &iroha_data_model::offline::KagemushaNoteMembershipWitnessV2,
    step_eq_record: &VerifyingKeyRecord,
    step_ep_record: &VerifyingKeyRecord,
) -> Result<(), String> {
    verify_kagemusha_recursive_spend_v2_terminal(bundle, step_eq_record, step_ep_record)?;
    verify_kagemusha_output_membership_v2(
        bundle.current_note.note_commitment,
        bundle.branch,
        bundle.statement.final_root,
        membership_witness,
    )
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
    pub parity: iroha_data_model::offline::KagemushaPastaCycleParityV3,
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
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V3,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3, KagemushaPastaCycleParityV3,
        };

        let expected_circuit = match self.parity {
            KagemushaPastaCycleParityV3::StepEq => KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3,
            KagemushaPastaCycleParityV3::StepEp => KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3,
        };
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_VERSION_V3
            || self.manifest_schema != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3
            || self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V3
            || !iroha_data_model::offline::is_kagemusha_v3_portable_identifier(&self.generation)
            || self.circuit_id != expected_circuit
            || !iroha_data_model::offline::is_kagemusha_v3_portable_identifier(
                &self.parameter_generation,
            )
            || self.ipa_k != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V3
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

#[cfg(test)]
mod tests {
    use super::*;

    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use halo2_proofs::halo2curves::pasta::Fq;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use sha2::{Digest as _, Sha256};

    #[derive(Clone, norito::Encode)]
    struct RecipientOutputProverMaterialFixtureV2 {
        amount: u128,
        rho: [u8; 32],
        owner_tag: [u8; 32],
    }

    const PEER_TEXT_LIMIT: usize = 12 * 1024;
    const PEER_TEXT_PREFIX_BYTES: usize = 6;
    const QR_DATA_CHUNK_BYTES: usize = 256;
    const QR_PARITY_GROUP: usize = 4;
    const NFC_SAFE_CHUNK_BYTES: usize = 220;

    fn div_ceil(value: usize, divisor: usize) -> usize {
        value.div_ceil(divisor)
    }

    fn canonical_peer_text(prefix: &str, archive: &[u8]) -> Result<String, &'static str> {
        if prefix.len() != PEER_TEXT_PREFIX_BYTES
            || archive.is_empty()
            || archive.len()
                > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2
        {
            return Err("peer archive shape");
        }
        let text = format!("{prefix}{}", URL_SAFE_NO_PAD.encode(archive));
        if text.len() > PEER_TEXT_LIMIT {
            return Err("peer text size");
        }
        Ok(text)
    }

    fn strict_peer_text_archive(prefix: &str, text: &str) -> Result<Vec<u8>, &'static str> {
        if prefix.len() != PEER_TEXT_PREFIX_BYTES
            || text.len() > PEER_TEXT_LIMIT
            || !text.starts_with(prefix)
        {
            return Err("peer text framing");
        }
        let body = &text[prefix.len()..];
        if body.is_empty()
            || body.len() % 4 == 1
            || !body
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err("peer base64url alphabet");
        }
        let archive = URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_| "peer base64url decode")?;
        let canonical =
            canonical_peer_text(prefix, &archive).map_err(|_| "peer text is not canonical")?;
        if archive.is_empty()
            || archive.len()
                > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2
            || canonical != text
        {
            return Err("peer text is not canonical");
        }
        Ok(archive)
    }

    fn qr_frame_count(archive_bytes: usize) -> usize {
        let data_frames = div_ceil(archive_bytes, QR_DATA_CHUNK_BYTES);
        1 + data_frames + div_ceil(data_frames, QR_PARITY_GROUP)
    }

    fn nfc_data_chunk_count(text_bytes: usize) -> usize {
        div_ceil(text_bytes, NFC_SAFE_CHUNK_BYTES)
    }

    fn scalar_bytes(value: u64) -> [u8; 32] {
        let repr = Scalar::from(value).to_repr();
        let mut bytes = [0; 32];
        bytes.copy_from_slice(repr.as_ref());
        bytes
    }

    fn transport_request_fixture() -> (
        iroha_data_model::offline::KagemushaRecipientPaymentRequestV2,
        KeyPair,
    ) {
        use iroha_data_model::{
            ChainId,
            account::AccountId,
            asset::AssetDefinitionId,
            domain::DomainId,
            offline::{
                KagemushaRecipientPaymentRequestSigningPayloadV2,
                KagemushaRecipientPaymentRequestV2, KagemushaScaledAmountV2,
                KagemushaSpendableNoteDescriptorV2, kagemusha_receiver_key_reference_v2,
            },
        };

        let chain_id = ChainId::from("pk-cbdc-mainnet");
        let asset = AssetDefinitionId::new(
            DomainId::try_new("sbp", "universal").expect("asset domain"),
            "pkr".parse().expect("asset name"),
        );
        let keypair =
            KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519).expect("fixture keypair");
        let amount = KagemushaScaledAmountV2::new(50_000_000, 9).expect("fixture amount");
        let recipient_output = KagemushaSpendableNoteDescriptorV2 {
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            note_commitment: [0x31; 32],
            spend_nullifier: [0x32; 32],
            amount,
        };
        let payload = KagemushaRecipientPaymentRequestSigningPayloadV2 {
            chain_id,
            asset,
            amount,
            recipient: AccountId::new(keypair.public_key().clone()),
            recipient_key_reference: kagemusha_receiver_key_reference_v2(keypair.public_key())
                .expect("receiver key reference"),
            receiver_device_id: "iphone17promax-production-secure-enclave".to_owned(),
            receiver_public_key: keypair.public_key().clone(),
            request_id: [0x33; 32],
            issued_at_ms: 1_700_000_000_000,
            expires_at_ms: 1_700_000_300_000,
            recipient_output,
            sender_output_prover_material: norito::to_bytes(
                &RecipientOutputProverMaterialFixtureV2 {
                    amount: amount.atomic_units,
                    rho: [0x34; 32],
                    owner_tag: [0x35; 32],
                },
            )
            .expect("prover material"),
        };
        let signature = Signature::new(
            keypair.private_key(),
            &payload.signing_bytes().expect("request signing bytes"),
        );
        let request = KagemushaRecipientPaymentRequestV2::from_signed_payload(payload, signature)
            .expect("signed request");
        (request, keypair)
    }

    fn transport_payment_fixture(
        depth: u8,
        peer_hop_count: u32,
        request: &iroha_data_model::offline::KagemushaRecipientPaymentRequestV2,
    ) -> iroha_data_model::offline::KagemushaRecursiveSpendPeerPaymentV2 {
        use iroha_data_model::{
            offline::{
                KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2,
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3,
                KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_KEY_BACKEND_V3,
                KagemushaConfidentialMerklePathV2, KagemushaNoteMembershipWitnessV2,
                KagemushaRecursiveSpendArtifactBindingV3, KagemushaRecursiveSpendBranchClaimV2,
                KagemushaRecursiveSpendBranchV2, KagemushaRecursiveSpendBundleV2,
                KagemushaRecursiveSpendPeerPaymentV2, KagemushaRecursiveSpendPeerSplitTransitionV2,
                KagemushaRecursiveSpendProofV2, KagemushaRecursiveSpendPublicStatementV2,
                KagemushaRecursiveSpendTopUpAnchorRefV2, KagemushaRecursiveSpendTransitionV2,
                kagemusha_recursive_spend_lineage_root_v2,
                kagemusha_recursive_spend_step_parity_v3,
                kagemusha_recursive_spend_step_verifier_role_v3,
            },
            proof::{ProofBox, VerifyingKeyId},
        };

        let binding_digest = [0x80; 32];
        let anchor = KagemushaRecursiveSpendTopUpAnchorRefV2 {
            topup_operation_id: [0x11; 32],
            anchor_digest: [0x21; 32],
        };
        let lineage_root =
            kagemusha_recursive_spend_lineage_root_v2(anchor.anchor_digest).expect("lineage root");
        let mut parent_claim =
            KagemushaRecursiveSpendBranchClaimV2::root(lineage_root).expect("root claim");
        for _ in 1..depth {
            parent_claim = parent_claim
                .child(KagemushaRecursiveSpendBranchV2::Recipient, binding_digest)
                .expect("parent recipient claim child");
        }
        let claim = parent_claim
            .child(KagemushaRecursiveSpendBranchV2::Recipient, binding_digest)
            .expect("recipient output claim");
        let proof_step_count = u32::from(depth) + 1;
        let verifier_key_id = VerifyingKeyId::new(
            KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_KEY_BACKEND_V3,
            kagemusha_recursive_spend_step_verifier_role_v3(proof_step_count)
                .expect("step verifier role"),
        );
        let statement = KagemushaRecursiveSpendPublicStatementV2 {
            chain_id: request.chain_id.clone(),
            asset: request.asset.clone(),
            asset_scale: request.amount.scale,
            input_root: [0x43; 32],
            final_root: [0x44; 32],
            topup_anchor_refs: vec![anchor],
            proof_step_count,
            peer_hop_count,
            transition: Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(
                KagemushaRecursiveSpendPeerSplitTransitionV2 {
                    binding_digest,
                    recipient_request_digest: request.digest().expect("request digest"),
                    operation_id: [0x45; 32],
                    parent_branch_claim_digest: parent_claim.digest().expect("parent claim digest"),
                    parent_max_proof_step_count: proof_step_count - 1,
                    parent_max_peer_hop_count: peer_hop_count - 1,
                },
            )),
            artifact_binding: KagemushaRecursiveSpendArtifactBindingV3 {
                generation: "kagemusha-prod-2026-07".to_owned(),
                manifest_sha256: [0x46; 32],
            },
            verifier_key_id: verifier_key_id.clone(),
        };
        statement
            .validate_public_binding()
            .expect("valid transport statement and verifier id");
        let statement_digest = statement.digest().expect("statement digest");

        let predecessor_step = proof_step_count - 1;
        let predecessor_proof_bytes = vec![0x5A; 1_536];
        let mut predecessor_statement = statement.clone();
        predecessor_statement.input_root = [0x42; 32];
        predecessor_statement.final_root = statement.input_root;
        predecessor_statement.proof_step_count = predecessor_step;
        predecessor_statement.peer_hop_count = peer_hop_count - 1;
        predecessor_statement.transition = if predecessor_step == 1 {
            None
        } else {
            Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(
                KagemushaRecursiveSpendPeerSplitTransitionV2 {
                    binding_digest: [0x56; 32],
                    recipient_request_digest: [0x57; 32],
                    operation_id: [0x58; 32],
                    parent_branch_claim_digest: [0x59; 32],
                    parent_max_proof_step_count: predecessor_step - 1,
                    parent_max_peer_hop_count: peer_hop_count - 2,
                },
            ))
        };
        predecessor_statement.verifier_key_id = VerifyingKeyId::new(
            KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_KEY_BACKEND_V3,
            kagemusha_recursive_spend_step_verifier_role_v3(predecessor_step)
                .expect("predecessor verifier role"),
        );
        let predecessor_predecessor_proof_sha256 = if predecessor_step == 1 {
            [0; 32]
        } else {
            [0x54; 32]
        };
        let predecessor_deferred_equation_digest = if predecessor_step == 1 {
            [0; 32]
        } else {
            [0x55; 32]
        };
        let predecessor = super::super::kagemusha_recursion_adapter::KagemushaLeapfrogStepProofV3 {
            parity: kagemusha_recursive_spend_step_parity_v3(predecessor_step)
                .expect("predecessor parity"),
            public_inputs: kagemusha_step_public_inputs_from_statement(
                &predecessor_statement,
                predecessor_predecessor_proof_sha256,
                predecessor_deferred_equation_digest,
            )
            .expect("predecessor semantic instances"),
            proof_bytes: predecessor_proof_bytes,
        };
        let predecessor_proof_sha256: [u8; 32] = Sha256::digest(&predecessor.proof_bytes).into();
        let window =
            super::super::kagemusha_recursion_adapter::KagemushaLeapfrogProofWindowV3 {
                version: super::super::kagemusha_recursion_adapter::KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V3,
                newest:
                    super::super::kagemusha_recursion_adapter::KagemushaLeapfrogStepProofV3 {
                        parity: kagemusha_recursive_spend_step_parity_v3(proof_step_count)
                            .expect("newest parity"),
                        public_inputs: kagemusha_step_public_inputs_from_statement(
                            &statement,
                            predecessor_proof_sha256,
                            [0x53; 32],
                        )
                        .expect("newest semantic instances"),
                        proof_bytes: vec![0xA5; 1_536],
                    },
                predecessor: Some(predecessor),
            };
        window.validate().expect("canonical leapfrog window");
        let proof_bytes = norito::to_bytes(&window).expect("proof window archive");
        assert_eq!(
            proof_bytes.len(),
            super::super::kagemusha_recursion_adapter::KAGEMUSHA_LEAPFROG_PROOF_WINDOW_BYTES_V3
        );
        let bundle = KagemushaRecursiveSpendBundleV2 {
            statement,
            recursive_proof: KagemushaRecursiveSpendProofV2 {
                verifier_key_id,
                public_statement_digest: statement_digest,
                proof: ProofBox::new(
                    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3
                        .parse()
                        .expect("proof backend"),
                    proof_bytes,
                ),
            },
            branch: KagemushaRecursiveSpendBranchV2::Recipient,
            current_note: request.recipient_output.clone(),
            branch_claims: vec![claim],
        };
        let input_path = KagemushaConfidentialMerklePathV2 {
            siblings: vec![[0x61; 32]; KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2],
            directions: vec![0; KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2],
            root: [0x44; 32],
        };
        let mut dummy_directions = vec![0; KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2];
        dummy_directions[1] = 1;
        let payment = KagemushaRecursiveSpendPeerPaymentV2 {
            recipient_bundle: bundle,
            recipient_membership_witness: KagemushaNoteMembershipWitnessV2 {
                leaf_index: 0,
                input_path,
                dummy_input_path: KagemushaConfidentialMerklePathV2 {
                    siblings: vec![[0x62; 32]; KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2],
                    directions: dummy_directions,
                    root: [0x44; 32],
                },
            },
        };
        payment
            .validate_public_binding()
            .expect("valid canonical peer payment");
        payment
    }

    fn init_statement() -> KagemushaRecursiveSpendPublicStatementV2 {
        use iroha_data_model::{
            ChainId,
            asset::AssetDefinitionId,
            domain::DomainId,
            offline::{
                KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_KEY_BACKEND_V3,
                KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V3, KagemushaRecursiveSpendArtifactBindingV3,
                KagemushaRecursiveSpendTopUpAnchorRefV2,
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
            input_root: scalar_bytes(11),
            final_root: scalar_bytes(12),
            topup_anchor_refs: vec![anchor],
            proof_step_count: 1,
            peer_hop_count: 0,
            transition: None,
            artifact_binding: KagemushaRecursiveSpendArtifactBindingV3 {
                generation: "release-generation-1".to_owned(),
                manifest_sha256: [0x43; 32],
            },
            verifier_key_id: VerifyingKeyId::new(
                KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_KEY_BACKEND_V3,
                KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V3,
            ),
        }
    }

    fn statement_bound_transition(
        statement: &KagemushaRecursiveSpendPublicStatementV2,
    ) -> [Scalar; KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS] {
        let mut transition = [Scalar::from(0); KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS];
        fill_common_statement_values(&mut transition, statement, [0; 32], [0; 32])
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
            KagemushaRecursiveSpendBranchClaimV2, KagemushaRecursiveSpendPeerSplitTransitionV2,
            KagemushaRecursiveSpendTransitionV2, kagemusha_recursive_spend_lineage_root_v2,
        };

        let mut statement = init_statement();
        let binding_digest = [0x61; 32];
        statement.final_root = scalar_bytes(13);
        statement.proof_step_count = 2;
        statement.peer_hop_count = 1;
        statement.transition = Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(
            KagemushaRecursiveSpendPeerSplitTransitionV2 {
                binding_digest,
                recipient_request_digest: [0x62; 32],
                operation_id: [0x63; 32],
                parent_branch_claim_digest: KagemushaRecursiveSpendBranchClaimV2::root(
                    kagemusha_recursive_spend_lineage_root_v2(
                        statement.topup_anchor_refs[0].anchor_digest,
                    )
                    .expect("lineage root"),
                )
                .expect("parent root claim")
                .digest()
                .expect("parent root claim digest"),
                parent_max_proof_step_count: 1,
                parent_max_peer_hop_count: 0,
            },
        ));
        statement.verifier_key_id = iroha_data_model::proof::VerifyingKeyId::new(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_KEY_BACKEND_V3,
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V3,
        );
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
        transition[I_APPEND_PROFILE] = Scalar::from(1);
        transition[I_PREVIOUS_PROOF_STEP_COUNT] =
            Scalar::from(u64::from(split.parent_max_proof_step_count));
        transition[I_PREVIOUS_PEER_HOP_COUNT] =
            Scalar::from(u64::from(split.parent_max_peer_hop_count));
        transition[I_PARENT_BRANCH_DEPTH] = Scalar::from(0);
        write_limb_group(
            &mut transition,
            I_PARENT_BRANCH_LINEAGE_ROOT,
            &statement.topup_anchor_refs[0].anchor_digest,
        );
        write_limb_group(&mut transition, I_SPLIT_DIGEST, &split.binding_digest);
        write_limb_group(
            &mut transition,
            I_RECIPIENT_REQUEST_DIGEST,
            &split.recipient_request_digest,
        );
        write_limb_group(&mut transition, I_OPERATION_ID, &split.operation_id);
        write_limb_group(
            &mut transition,
            I_PARENT_BRANCH_CLAIM_DIGEST,
            &split.parent_branch_claim_digest,
        );
        transition
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
        p[I_RECORD_INPUT_COUNT] = Scalar::from(1);
        p[I_TRANSFER_INPUT_COUNT] = Scalar::from(1);
        p[I_RECORD_OUTPUT_COUNT] = Scalar::from(2);
        p[I_TRANSFER_OUTPUT_COUNT] = Scalar::from(2);
        write_amount(p, I_INPUT_AMOUNT_LO, 100);
        write_amount(p, I_TRANSFER_AMOUNT_LO, 40);
        write_amount(p, I_RECIPIENT_AMOUNT_LO, 40);
        write_amount(p, I_CHANGE_AMOUNT_LO, 60);
        p[I_PARENT_FINAL_ROOT] = Scalar::from(11);
        p[I_RECORD_ROOT_BEFORE] = Scalar::from(11);
        p[I_TRANSFER_ROOT] = Scalar::from(11);
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
        let output_root_nodes =
            output_accumulator_nodes(p[I_RECIPIENT_COMMITMENT], p[I_CHANGE_COMMITMENT]);
        p[I_FINAL_ROOT] = output_root_nodes[OUTPUT_ACCUMULATOR_DEPTH - 1];
        p[I_RECORD_ROOT_AFTER] = p[I_FINAL_ROOT];
        values.output_root_nodes = output_root_nodes;
        values
    }

    fn valid_redeem_change_values() -> KagemushaRecursiveSpendTransitionValuesV2 {
        let mut values = KagemushaRecursiveSpendTransitionValuesV2::default();
        let p = &mut values.public;
        p[I_LAYOUT_VERSION] = Scalar::from(KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_LAYOUT_VERSION);
        p[I_REDEMPTION_PROFILE] = Scalar::from(1);
        p[I_HAS_CHANGE] = Scalar::from(1);
        p[I_PROOF_STEP_COUNT] = Scalar::from(2);
        p[I_PEER_HOP_COUNT] = Scalar::from(0);
        p[I_PREVIOUS_PROOF_STEP_COUNT] = Scalar::from(1);
        p[I_PREVIOUS_PEER_HOP_COUNT] = Scalar::from(0);
        p[I_BRANCH_DEPTH] = Scalar::from(1);
        p[I_PARENT_BRANCH_DEPTH] = Scalar::from(0);
        p[I_BRANCH_PATH_BITS] = Scalar::from(1u64 << 63);
        p[I_PARENT_BRANCH_PATH_BITS] = Scalar::from(0);
        p[I_ASSET_SCALE] = Scalar::from(2);
        p[I_INPUT_SCALE] = Scalar::from(2);
        p[I_TRANSFER_SCALE] = Scalar::from(2);
        p[I_RECIPIENT_SCALE] = Scalar::from(2);
        p[I_CHANGE_SCALE] = Scalar::from(2);
        p[I_RECORD_INPUT_COUNT] = Scalar::from(1);
        p[I_TRANSFER_INPUT_COUNT] = Scalar::from(1);
        p[I_RECORD_OUTPUT_COUNT] = Scalar::from(1);
        p[I_TRANSFER_OUTPUT_COUNT] = Scalar::from(1);
        write_amount(p, I_INPUT_AMOUNT_LO, 100);
        write_amount(p, I_TRANSFER_AMOUNT_LO, 40);
        write_amount(p, I_RECIPIENT_AMOUNT_LO, 40);
        write_amount(p, I_CHANGE_AMOUNT_LO, 60);
        p[I_UNSHIELD_PUBLIC_AMOUNT] = Scalar::from(40);
        p[I_PARENT_FINAL_ROOT] = Scalar::from(11);
        p[I_RECORD_ROOT_BEFORE] = Scalar::from(11);
        p[I_TRANSFER_ROOT] = Scalar::from(11);
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
        values.path_depth_selectors[0] = Scalar::from(1);
        let output_root_nodes =
            output_accumulator_nodes(p[I_RECIPIENT_COMMITMENT], p[I_CHANGE_COMMITMENT]);
        p[I_FINAL_ROOT] = output_root_nodes[OUTPUT_ACCUMULATOR_DEPTH - 1];
        p[I_RECORD_ROOT_AFTER] = p[I_FINAL_ROOT];
        values.output_root_nodes = output_root_nodes;
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
        let mut converted = KagemushaRecursiveSpendTransitionValuesV2 {
            public: values.public.map(convert),
            amount_low_carry: convert(values.amount_low_carry),
            path_depth_selectors: values.path_depth_selectors.map(convert),
            peer_hop_selectors: values.peer_hop_selectors.map(convert),
            output_root_nodes: values.output_root_nodes.map(convert),
        };
        // The symmetric scaffold evaluates the output accumulator in each
        // native Pasta field. Production remains gated on the cross-field
        // leapfrog verifier, which must carry the canonical Fp root into the
        // reciprocal step rather than reinterpret it as native Fq arithmetic.
        let output_root_nodes = output_accumulator_nodes(
            converted.public[I_RECIPIENT_COMMITMENT],
            converted.public[I_CHANGE_COMMITMENT],
        );
        converted.public[I_FINAL_ROOT] = output_root_nodes[OUTPUT_ACCUMULATOR_DEPTH - 1];
        converted.public[I_RECORD_ROOT_AFTER] = converted.public[I_FINAL_ROOT];
        converted.output_root_nodes = output_root_nodes;
        converted
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
            I_PARENT_FINAL_ROOT,
            I_FINAL_ROOT,
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
        for row in [I_APPEND_PROFILE, I_REDEMPTION_PROFILE, I_HAS_CHANGE] {
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
            I_PARENT_BRANCH_CLAIM_DIGEST,
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
    fn envelope_statement_binding_rejects_peer_profile_substitution() {
        let statement = append_statement();
        let transition = append_statement_bound_transition(&statement);
        ensure_transition_statement_binding(&statement, &transition)
            .expect("canonical append statement rows");

        for row in [
            I_APPEND_PROFILE,
            I_REDEMPTION_PROFILE,
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
            I_PARENT_BRANCH_CLAIM_DIGEST,
        ] {
            let mut tampered = transition;
            tampered[start] += Scalar::from(1);
            assert!(
                ensure_transition_statement_binding(&statement, &tampered).is_err(),
                "peer transition limb group at {start} must match the statement"
            );
        }
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
    fn transition_relation_rejects_sibling_output_substitution() {
        let values = valid_append_values();
        for row in [I_RECIPIENT_COMMITMENT, I_CHANGE_COMMITMENT] {
            let mut tampered = values.clone();
            tampered.public[row] += Scalar::from(1);
            let instances = vec![tampered.public.to_vec()];
            let prover = halo2_proofs::dev::MockProver::run(
                9,
                &KagemushaRecursiveSpendTransitionCircuitV2 { values: tampered },
                instances,
            )
            .expect("mock prover");
            assert!(
                prover.verify().is_err(),
                "substituting output commitment row {row} must break the proof-bound output root"
            );
        }
    }

    #[test]
    fn sibling_membership_paths_recompute_the_proof_bound_output_root() {
        use iroha_data_model::offline::{
            KagemushaConfidentialMerklePathV2, KagemushaNoteMembershipWitnessV2,
            KagemushaRecursiveSpendBranchV2,
        };

        fn wire_path(
            path: super::super::confidential_v2::ConfidentialMerklePathV2,
        ) -> KagemushaConfidentialMerklePathV2 {
            let (siblings, directions, _, root) = path.into_parts();
            KagemushaConfidentialMerklePathV2 {
                siblings,
                directions,
                root,
            }
        }

        let recipient = scalar_bytes(31);
        let change = scalar_bytes(41);
        let commitments = [recipient, change];
        let root = super::super::confidential_v2::compute_confidential_root_v2(&commitments)
            .expect("fresh output root");
        let dummy = wire_path(
            super::super::confidential_v2::compute_confidential_merkle_path_v2(&commitments, 2)
                .expect("canonical empty path"),
        );
        let recipient_witness = KagemushaNoteMembershipWitnessV2 {
            leaf_index: 0,
            input_path: wire_path(
                super::super::confidential_v2::compute_confidential_merkle_path_v2(&commitments, 0)
                    .expect("recipient path"),
            ),
            dummy_input_path: dummy.clone(),
        };
        let change_witness = KagemushaNoteMembershipWitnessV2 {
            leaf_index: 1,
            input_path: wire_path(
                super::super::confidential_v2::compute_confidential_merkle_path_v2(&commitments, 1)
                    .expect("change path"),
            ),
            dummy_input_path: dummy,
        };
        verify_kagemusha_output_membership_v2(
            recipient,
            KagemushaRecursiveSpendBranchV2::Recipient,
            root,
            &recipient_witness,
        )
        .expect("recipient output membership");
        verify_kagemusha_output_membership_v2(
            change,
            KagemushaRecursiveSpendBranchV2::Change,
            root,
            &change_witness,
        )
        .expect("change output membership");

        let mut substituted = recipient_witness;
        substituted.input_path.siblings[0] = scalar_bytes(99);
        assert!(
            verify_kagemusha_output_membership_v2(
                recipient,
                KagemushaRecursiveSpendBranchV2::Recipient,
                root,
                &substituted,
            )
            .is_err(),
            "a substituted sibling must not authenticate against the shared output root"
        );
    }

    #[test]
    fn transition_relation_is_identical_on_both_pasta_step_parities() {
        let eq_values = valid_append_values();
        let ep_values = as_step_ep_values(&eq_values);
        let instances = vec![ep_values.public.to_vec()];
        let prover = halo2_proofs::dev::MockProver::run(
            9,
            &KagemushaRecursiveSpendTransitionCircuitV2::<Fq> { values: ep_values },
            instances,
        )
        .expect("StepEp mock prover");
        prover.assert_satisfied();

        let mut non_conserving = as_step_ep_values(&eq_values);
        non_conserving.public[I_CHANGE_AMOUNT_LO] += Fq::from(1);
        let instances = vec![non_conserving.public.to_vec()];
        let prover = halo2_proofs::dev::MockProver::run(
            9,
            &KagemushaRecursiveSpendTransitionCircuitV2::<Fq> {
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
    fn transition_relation_rejects_lineage_counter_drift() {
        let mut values = valid_append_values();
        values.public[I_PROOF_STEP_COUNT] = Scalar::from(3);
        values.public[I_PREVIOUS_PROOF_STEP_COUNT] = Scalar::from(2);
        assert!(
            halo2_proofs::dev::MockProver::run(
                9,
                &KagemushaRecursiveSpendTransitionCircuitV2 { values },
                vec![vec![
                    Scalar::from(0);
                    KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS
                ]],
            )
            .is_err(),
            "inconsistent proof-step/branch-depth counters must fail before proof construction"
        );
    }

    #[test]
    fn transition_relation_accepts_low_limb_carry() {
        let mut values = valid_append_values();
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
    fn transition_relation_rejects_change_path_on_shared_peer_proof() {
        let mut values = valid_append_values();
        values.public[I_BRANCH_PATH_BITS] = Scalar::from(1u64 << 63);
        let instances = vec![values.public.to_vec()];
        let prover = halo2_proofs::dev::MockProver::run(
            9,
            &KagemushaRecursiveSpendTransitionCircuitV2 { values },
            instances,
        )
        .expect("mock prover");
        assert!(
            prover.verify().is_err(),
            "peer split must have one canonical shared path, not a branch-selected proof"
        );
    }

    #[test]
    fn transition_relation_accepts_partial_redemption_change() {
        let values = valid_redeem_change_values();
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
    fn transition_relation_rejects_unbound_redemption_credit() {
        let mut values = valid_redeem_change_values();
        values.public[I_UNSHIELD_PUBLIC_AMOUNT] = Scalar::from(41);
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
    fn transition_relation_rejects_second_redemption_input() {
        let mut values = valid_redeem_change_values();
        values.public[I_TRANSFER_INPUT_COMMITMENT_1] = Scalar::from(99);
        values.public[I_TRANSFER_NULLIFIER_1] = Scalar::from(100);
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
    fn transition_relation_enforces_sixty_four_peer_hops_independently_of_branch_depth() {
        let mut at_limit = valid_append_values();
        let maximum_hops = usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2)
            .expect("peer-hop limit fits usize");
        at_limit.public[I_PEER_HOP_COUNT] =
            Scalar::from(u64::from(KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2));
        at_limit.public[I_PREVIOUS_PEER_HOP_COUNT] =
            Scalar::from(u64::from(KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2 - 1));
        at_limit.public[I_BRANCH_DEPTH] = Scalar::from(64);
        at_limit.public[I_PARENT_BRANCH_DEPTH] = Scalar::from(63);
        at_limit.public[I_PROOF_STEP_COUNT] = Scalar::from(65);
        at_limit.public[I_PREVIOUS_PROOF_STEP_COUNT] = Scalar::from(64);
        at_limit.peer_hop_selectors[1] = Scalar::from(0);
        at_limit.peer_hop_selectors[maximum_hops] = Scalar::from(1);
        at_limit.path_depth_selectors[0] = Scalar::from(0);
        at_limit.path_depth_selectors[63] = Scalar::from(1);
        let instances = vec![at_limit.public.to_vec()];
        let prover = halo2_proofs::dev::MockProver::run(
            9,
            &KagemushaRecursiveSpendTransitionCircuitV2 { values: at_limit },
            instances,
        )
        .expect("mock prover at peer-hop limit");
        prover.assert_satisfied();

        let mut above_limit = valid_append_values();
        above_limit.public[I_PEER_HOP_COUNT] =
            Scalar::from(u64::from(KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2 + 1));
        above_limit.public[I_PREVIOUS_PEER_HOP_COUNT] =
            Scalar::from(u64::from(KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2));
        assert!(
            halo2_proofs::dev::MockProver::run(
                9,
                &KagemushaRecursiveSpendTransitionCircuitV2 {
                    values: above_limit,
                },
                vec![vec![
                    Scalar::from(0);
                    KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS
                ]],
            )
            .is_err(),
            "a peer hop above the configured limit must fail before proof construction"
        );
    }

    #[test]
    fn complete_peer_archives_fit_real_text_qr_and_nfc_limits_through_branch_depth_64() {
        use iroha_data_model::offline::{
            KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3,
            KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_KEY_BACKEND_V3,
            KagemushaReceiverAcknowledgementPayloadV2, KagemushaReceiverAcknowledgementV2,
            kagemusha_recursive_spend_step_verifier_role_v3,
        };

        let (request, keypair) = transport_request_fixture();
        request
            .validate_public_binding()
            .expect("valid signed receive request");
        let request_archive = norito::to_bytes(&request).expect("request archive");
        let request_text = canonical_peer_text("PKK2R.", &request_archive).expect("request text");
        assert_eq!((request_archive.len(), request_text.len()), (824, 1_105));
        assert_eq!(
            strict_peer_text_archive("PKK2R.", &request_text).expect("request round trip"),
            request_archive
        );
        assert_eq!(qr_frame_count(request_archive.len()), 6);
        assert_eq!(nfc_data_chunk_count(request_text.len()), 6);
        assert_eq!(nfc_data_chunk_count(request_text.len()) + 2, 8);

        let expected = [
            (
                1_u8,
                1_u32,
                6_677_usize,
                8_909_usize,
                35_usize,
                41_usize,
                43_usize,
            ),
            (8, 8, 6_848, 9_137, 35, 42, 44),
            (16, 16, 7_040, 9_393, 36, 43, 45),
            (32, 32, 7_424, 9_905, 38, 46, 48),
            (64, 64, 8_192, 10_929, 41, 50, 52),
        ];
        let mut deepest_payment = None;
        for (depth, peer_hops, raw_bytes, text_bytes, qr_frames, nfc_chunks, nfc_commands) in
            expected
        {
            let payment = transport_payment_fixture(depth, peer_hops, &request);
            let statement = &payment.recipient_bundle.statement;
            let expected_role =
                kagemusha_recursive_spend_step_verifier_role_v3(statement.proof_step_count)
                    .expect("verifier role");
            assert_eq!(
                statement.verifier_key_id.backend.as_str(),
                KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_KEY_BACKEND_V3,
                "depth {depth} must use the registry backend, not the proof backend"
            );
            assert_eq!(statement.verifier_key_id.name.as_str(), expected_role);
            assert_eq!(
                payment.recipient_bundle.recursive_proof.verifier_key_id,
                statement.verifier_key_id
            );
            assert_eq!(
                payment
                    .recipient_bundle
                    .recursive_proof
                    .proof
                    .backend
                    .as_str(),
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3
            );
            payment
                .validate_public_binding()
                .expect("fixture verifier IDs and peer payment validate before measurement");

            let archive = norito::to_bytes(&payment).expect("payment archive");
            let text = canonical_peer_text("PKK2P.", &archive).expect("payment text");
            assert_eq!(archive.len(), raw_bytes, "raw bytes at depth {depth}");
            assert_eq!(text.len(), text_bytes, "text bytes at depth {depth}");
            assert_eq!(
                strict_peer_text_archive("PKK2P.", &text).expect("payment round trip"),
                archive
            );
            assert_eq!(qr_frame_count(archive.len()), qr_frames);
            assert_eq!(nfc_data_chunk_count(text.len()), nfc_chunks);
            assert_eq!(nfc_data_chunk_count(text.len()) + 2, nfc_commands);
            assert!(archive.len() <= KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2);
            assert!(text.len() <= PEER_TEXT_LIMIT);
            if depth == 64 {
                deepest_payment = Some(payment);
            }
        }

        let payment = deepest_payment.expect("depth-64 payment");
        let payment_bundle = &payment.recipient_bundle;
        let acknowledgement_payload = KagemushaReceiverAcknowledgementPayloadV2 {
            operation_id: [0x45; 32],
            recipient_request_digest: request.digest().expect("request digest"),
            payment_bundle_digest: payment_bundle.digest().expect("payment bundle digest"),
            recipient_commitment: request.recipient_output.note_commitment,
            accepted_at_ms: request.issued_at_ms + 1,
            receiver_device_id: request.receiver_device_id.clone(),
            receiver_key_reference: request.recipient_key_reference,
            receiver_public_key: request.receiver_public_key.clone(),
        };
        let acknowledgement = KagemushaReceiverAcknowledgementV2 {
            signature: Signature::new(
                keypair.private_key(),
                &acknowledgement_payload
                    .signing_bytes()
                    .expect("acknowledgement signing bytes"),
            ),
            payload: acknowledgement_payload,
        };
        let acknowledgement_archive = acknowledgement
            .canonical_archive_for_payment(&request, payment_bundle)
            .expect("valid acknowledgement archive");
        let acknowledgement_text =
            canonical_peer_text("PKK2A.", &acknowledgement_archive).expect("acknowledgement text");
        assert_eq!(
            (acknowledgement_archive.len(), acknowledgement_text.len()),
            (471, 634)
        );
        assert_eq!(
            strict_peer_text_archive("PKK2A.", &acknowledgement_text)
                .expect("acknowledgement round trip"),
            acknowledgement_archive
        );
        assert_eq!(qr_frame_count(acknowledgement_archive.len()), 4);
        assert_eq!(nfc_data_chunk_count(acknowledgement_text.len()), 3);
        assert_eq!(nfc_data_chunk_count(acknowledgement_text.len()) + 2, 5);

        let maximum_archive = vec![0xA5; KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2];
        let maximum_text =
            canonical_peer_text("PKK2P.", &maximum_archive).expect("exact maximum text");
        assert_eq!(maximum_text.len(), PEER_TEXT_LIMIT);
        assert_eq!(
            strict_peer_text_archive("PKK2P.", &maximum_text).expect("maximum round trip"),
            maximum_archive
        );
        let oversized_archive = vec![0xA5; KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2 + 1];
        assert!(canonical_peer_text("PKK2P.", &oversized_archive).is_err());
        let oversized_text = format!("{maximum_text}A");
        assert_eq!(oversized_text.len(), PEER_TEXT_LIMIT + 1);
        assert!(strict_peer_text_archive("PKK2P.", &oversized_text).is_err());

        for noncanonical in [
            format!("{request_text}="),
            request_text.replacen("PKK2R.", "pkk2r.", 1),
            format!("PKK2R.+{}", &request_text[PEER_TEXT_PREFIX_BYTES + 1..]),
            format!("PKK2R./{}", &request_text[PEER_TEXT_PREFIX_BYTES + 1..]),
            format!("PKK2R.\n{}", &request_text[PEER_TEXT_PREFIX_BYTES..]),
            format!("PKK2R.{request_text}"),
        ] {
            assert!(
                strict_peer_text_archive("PKK2R.", &noncanonical).is_err(),
                "non-canonical peer framing must fail: {noncanonical}"
            );
        }
        assert!(strict_peer_text_archive("PKK2P.", &request_text).is_err());
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
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3
                    .to_owned(),
            transcript_profile:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V3
                    .to_owned(),
            generation: "release-generation-1".to_owned(),
            parity: iroha_data_model::offline::KagemushaPastaCycleParityV3::StepEq,
            circuit_id: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3
                .to_owned(),
            parameter_generation: "params-generation-1".to_owned(),
            ipa_k: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V3,
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
        wrong_parity.parity = iroha_data_model::offline::KagemushaPastaCycleParityV3::StepEp;
        assert!(wrong_parity.validate_header().is_err());

        let mut oversized = header;
        oversized.payload_size_bytes =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3 + 1;
        assert!(oversized.validate_header().is_err());
    }

    #[test]
    fn pasta_cycle_backend_stays_disabled_until_soundness_and_device_gates_pass() {
        let capabilities =
            iroha_data_model::offline::kagemusha_recursive_spend_native_capabilities_v3();
        capabilities.validate().expect("canonical capabilities");
        assert!(
            !iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE,
            "the proof backend must remain unavailable until every soundness and device gate passes"
        );
        for required in [
            "opposite_field_pasta_cycle_loader",
            "alternating_parity_deferred_verifier",
            "physical_device_performance_evidence",
        ] {
            assert!(
                capabilities
                    .missing_gates
                    .iter()
                    .any(|gate| gate == required),
                "fail-closed capabilities must retain blocker {required}"
            );
        }
    }
}
