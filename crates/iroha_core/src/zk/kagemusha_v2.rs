//! Selector-free ABI-20/V4 Kagemusha recursive-spend backend and retained V2 primitives.
//!
//! V4 reuses the unchanged V2 amounts, note openings, authorization,
//! membership, and finality relations. Its fixed Eq/Ep recursive circuits keep
//! the recursive IPA verifier separate from the compact transition relation,
//! so initialization, one-parent, and two-parent transitions use identical
//! keys, layouts, and proof-size limits.

use halo2_proofs::halo2curves::pasta::Fp as Scalar;
use iroha_data_model::offline::{
    KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4, KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_CLAIMS_V2,
    KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2, KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2,
    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2, KagemushaRecursiveSpendArtifactManifestV4,
    KagemushaRecursiveSpendBundleV4, KagemushaRecursiveSpendPublicStatementV4,
    KagemushaRecursiveSpendStateBoundaryV2, KagemushaRecursiveSpendTransitionV4,
};
use norito::codec::Encode;

pub use super::kagemusha_recursion_adapter::{
    KagemushaGeneratedParityArtifactsV4, KagemushaGeneratedPastaCycleArtifactsV4,
    generate_kagemusha_pasta_cycle_artifacts_v4, validate_kagemusha_proof_pair_measurement_v4,
    validate_kagemusha_step_bootstrap_payload_v4,
};
pub use super::kagemusha_step_transition::{
    KagemushaStepOperationVectorV4, KagemushaStepTransferPublicV4,
};

/// Version of the exact field-neutral state vector carried across the Pasta cycle.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2: u32 = 2;
/// Number of canonical `u32` limbs in one recursive continuing-state vector.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2: usize = 890;
/// Number of `u32` limbs in one exact 32-byte value.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_DIGEST_LIMBS_V2: usize = 8;
/// Number of `u32` limbs in one exact 192-bit transition-choice tag.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V2: usize =
    KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_TAG_BYTES_V2 / 4;
/// Number of padded transition-tag limbs retained by one branch claim.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_HISTORY_LIMBS_V2: usize =
    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V2
        * KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2 as usize;
/// Number of fixed limbs occupied by one canonical branch claim.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V2: usize =
    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_DIGEST_LIMBS_V2
        + 1
        + 2
        + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_HISTORY_LIMBS_V2;

pub(crate) const S_VERSION: usize = 0;
pub(crate) const S_CHAIN_TAG: usize = S_VERSION + 1;
pub(crate) const S_ASSET_TAG: usize = S_CHAIN_TAG + 8;
pub(crate) const S_ASSET_SCALE: usize = S_ASSET_TAG + 8;
pub(crate) const S_FINAL_ROOT: usize = S_ASSET_SCALE + 1;
pub(crate) const S_NEXT_ZERO_LEAF_INDEX: usize = S_FINAL_ROOT + 8;
pub(crate) const S_TOPUP_ANCHOR_COUNT: usize = S_NEXT_ZERO_LEAF_INDEX + 1;
pub(crate) const S_TOPUP_ANCHORS: usize = S_TOPUP_ANCHOR_COUNT + 1;
pub(crate) const S_PROOF_STEP_COUNT: usize =
    S_TOPUP_ANCHORS + 16 * KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2;
pub(crate) const S_PEER_HOP_COUNT: usize = S_PROOF_STEP_COUNT + 1;
pub(crate) const S_CURRENT_COMMITMENT: usize = S_PEER_HOP_COUNT + 1;
pub(crate) const S_CURRENT_NULLIFIER: usize = S_CURRENT_COMMITMENT + 8;
pub(crate) const S_CURRENT_AMOUNT: usize = S_CURRENT_NULLIFIER + 8;
pub(crate) const S_CURRENT_SCALE: usize = S_CURRENT_AMOUNT + 4;
pub(crate) const S_BRANCH_CLAIM_COUNT: usize = S_CURRENT_SCALE + 1;
pub(crate) const S_BRANCH_CLAIMS: usize = S_BRANCH_CLAIM_COUNT + 1;
pub(crate) const S_ARTIFACT_MANIFEST_SHA256: usize = S_BRANCH_CLAIMS
    + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V2
        * KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_CLAIMS_V2;
pub(crate) const S_VERIFIER_KEY_ID: usize = S_ARTIFACT_MANIFEST_SHA256 + 8;
const S_END: usize = S_VERIFIER_KEY_ID + 8;

const _: [(); KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2] = [(); S_END];

/// Compile-time layout table for the exact recursive continuing-state vector.
///
/// The tuple values are `(field, first_limb, limb_count)`. Variable-count
/// collections always occupy their complete padded allocation; their count
/// limb and zero padding are part of the circuit relation.
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_V2: &[(&str, usize, usize)] = &[
    ("layout_version", S_VERSION, 1),
    ("chain_tag", S_CHAIN_TAG, 8),
    ("asset_tag", S_ASSET_TAG, 8),
    ("asset_scale", S_ASSET_SCALE, 1),
    ("final_root", S_FINAL_ROOT, 8),
    ("next_zero_leaf_index", S_NEXT_ZERO_LEAF_INDEX, 1),
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
        KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V2
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
pub const KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_COVERAGE_V2: &[(&str, &str)] = &[
    ("statement.chain_id", "chain_tag"),
    ("statement.asset", "asset_tag"),
    ("statement.asset_scale", "asset_scale"),
    ("statement.final_root", "final_root"),
    ("statement.next_zero_leaf_index", "next_zero_leaf_index"),
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
pub struct KagemushaRecursiveSpendStateVectorV2 {
    /// Fixed continuing-state limbs in [`KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_V2`] order.
    pub limbs: [u32; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2],
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
pub(crate) const I_LAYOUT_VERSION: usize = 0;
pub(crate) const I_APPEND_PROFILE: usize = I_LAYOUT_VERSION + 1;
pub(crate) const I_BRANCH_CHANGE: usize = I_APPEND_PROFILE + 1;
pub(crate) const I_HAS_CHANGE: usize = I_BRANCH_CHANGE + 1;
pub(crate) const I_RECORD_OUTPUT_SWAP: usize = I_HAS_CHANGE + 1;
pub(crate) const I_TRANSFER_OUTPUT_SWAP: usize = I_RECORD_OUTPUT_SWAP + 1;
pub(crate) const I_PROOF_STEP_COUNT: usize = I_TRANSFER_OUTPUT_SWAP + 1;
pub(crate) const I_PEER_HOP_COUNT: usize = I_PROOF_STEP_COUNT + 1;
pub(crate) const I_PREVIOUS_PROOF_STEP_COUNT: usize = I_PEER_HOP_COUNT + 1;
pub(crate) const I_PREVIOUS_PEER_HOP_COUNT: usize = I_PREVIOUS_PROOF_STEP_COUNT + 1;
pub(crate) const I_BRANCH_DEPTH: usize = I_PREVIOUS_PEER_HOP_COUNT + 1;
pub(crate) const I_PARENT_BRANCH_DEPTH: usize = I_BRANCH_DEPTH + 1;
pub(crate) const I_ASSET_SCALE: usize = I_PARENT_BRANCH_DEPTH + 1;
pub(crate) const I_INPUT_SCALE: usize = I_ASSET_SCALE + 1;
pub(crate) const I_TRANSFER_SCALE: usize = I_INPUT_SCALE + 1;
pub(crate) const I_RECIPIENT_SCALE: usize = I_TRANSFER_SCALE + 1;
pub(crate) const I_CHANGE_SCALE: usize = I_RECIPIENT_SCALE + 1;
pub(crate) const I_CURRENT_SCALE: usize = I_CHANGE_SCALE + 1;
pub(crate) const I_RECORD_INPUT_COUNT: usize = I_CURRENT_SCALE + 1;
pub(crate) const I_RECORD_OUTPUT_COUNT: usize = I_RECORD_INPUT_COUNT + 1;
pub(crate) const I_TRANSFER_INPUT_COUNT: usize = I_RECORD_OUTPUT_COUNT + 1;
pub(crate) const I_TRANSFER_OUTPUT_COUNT: usize = I_TRANSFER_INPUT_COUNT + 1;
pub(crate) const I_CURRENT_AMOUNT_LO: usize = I_TRANSFER_OUTPUT_COUNT + 1;
pub(crate) const I_CURRENT_AMOUNT_HI: usize = I_CURRENT_AMOUNT_LO + 1;
pub(crate) const I_INPUT_AMOUNT_LO: usize = I_CURRENT_AMOUNT_HI + 1;
pub(crate) const I_INPUT_AMOUNT_HI: usize = I_INPUT_AMOUNT_LO + 1;
pub(crate) const I_TRANSFER_AMOUNT_LO: usize = I_INPUT_AMOUNT_HI + 1;
pub(crate) const I_TRANSFER_AMOUNT_HI: usize = I_TRANSFER_AMOUNT_LO + 1;
pub(crate) const I_RECIPIENT_AMOUNT_LO: usize = I_TRANSFER_AMOUNT_HI + 1;
pub(crate) const I_RECIPIENT_AMOUNT_HI: usize = I_RECIPIENT_AMOUNT_LO + 1;
pub(crate) const I_CHANGE_AMOUNT_LO: usize = I_RECIPIENT_AMOUNT_HI + 1;
pub(crate) const I_CHANGE_AMOUNT_HI: usize = I_CHANGE_AMOUNT_LO + 1;
pub(crate) const I_BRANCH_PATH_BITS: usize = I_CHANGE_AMOUNT_HI + 1;
pub(crate) const I_PARENT_BRANCH_PATH_BITS: usize = I_BRANCH_PATH_BITS + 1;
pub(crate) const I_INITIAL_ROOT: usize = I_PARENT_BRANCH_PATH_BITS + 1;
pub(crate) const I_FINAL_ROOT: usize = I_INITIAL_ROOT + 1;
pub(crate) const I_RECORD_ROOT_BEFORE: usize = I_FINAL_ROOT + 1;
pub(crate) const I_RECORD_ROOT_AFTER: usize = I_RECORD_ROOT_BEFORE + 1;
pub(crate) const I_TRANSFER_ROOT: usize = I_RECORD_ROOT_AFTER + 1;
pub(crate) const I_CURRENT_COMMITMENT: usize = I_TRANSFER_ROOT + 1;
pub(crate) const I_CURRENT_NULLIFIER: usize = I_CURRENT_COMMITMENT + 1;
pub(crate) const I_INPUT_COMMITMENT: usize = I_CURRENT_NULLIFIER + 1;
pub(crate) const I_INPUT_NULLIFIER: usize = I_INPUT_COMMITMENT + 1;
pub(crate) const I_RECIPIENT_COMMITMENT: usize = I_INPUT_NULLIFIER + 1;
pub(crate) const I_RECIPIENT_NULLIFIER: usize = I_RECIPIENT_COMMITMENT + 1;
pub(crate) const I_CHANGE_COMMITMENT: usize = I_RECIPIENT_NULLIFIER + 1;
pub(crate) const I_CHANGE_NULLIFIER: usize = I_CHANGE_COMMITMENT + 1;
pub(crate) const I_RECORD_INPUT_NULLIFIER_0: usize = I_CHANGE_NULLIFIER + 1;
pub(crate) const I_RECORD_INPUT_NULLIFIER_1: usize = I_RECORD_INPUT_NULLIFIER_0 + 1;
pub(crate) const I_RECORD_OUTPUT_0: usize = I_RECORD_INPUT_NULLIFIER_1 + 1;
pub(crate) const I_RECORD_OUTPUT_1: usize = I_RECORD_OUTPUT_0 + 1;
pub(crate) const I_TRANSFER_INPUT_COMMITMENT_0: usize = I_RECORD_OUTPUT_1 + 1;
pub(crate) const I_TRANSFER_INPUT_COMMITMENT_1: usize = I_TRANSFER_INPUT_COMMITMENT_0 + 1;
pub(crate) const I_TRANSFER_NULLIFIER_0: usize = I_TRANSFER_INPUT_COMMITMENT_1 + 1;
pub(crate) const I_TRANSFER_NULLIFIER_1: usize = I_TRANSFER_NULLIFIER_0 + 1;
pub(crate) const I_TRANSFER_OUTPUT_0: usize = I_TRANSFER_NULLIFIER_1 + 1;
pub(crate) const I_TRANSFER_OUTPUT_1: usize = I_TRANSFER_OUTPUT_0 + 1;
pub(crate) const I_ASSET_TAG: usize = I_TRANSFER_OUTPUT_1 + 1;
pub(crate) const I_CHAIN_TAG: usize = I_ASSET_TAG + 1;
pub(crate) const I_STATEMENT_DIGEST: usize = I_CHAIN_TAG + 1;
pub(crate) const I_SPLIT_DIGEST: usize = I_STATEMENT_DIGEST + 4;
pub(crate) const I_RECIPIENT_REQUEST_DIGEST: usize = I_SPLIT_DIGEST + 4;
pub(crate) const I_OPERATION_ID: usize = I_RECIPIENT_REQUEST_DIGEST + 4;
pub(crate) const I_PARENT_BUNDLE_DIGEST: usize = I_OPERATION_ID + 4;
pub(crate) const I_BRANCH_LINEAGE_ROOT: usize = I_PARENT_BUNDLE_DIGEST + 4;
pub(crate) const I_PARENT_BRANCH_LINEAGE_ROOT: usize = I_BRANCH_LINEAGE_ROOT + 4;
pub(crate) const I_CHAIN_ID_DIGEST: usize = I_PARENT_BRANCH_LINEAGE_ROOT + 4;
pub(crate) const I_ASSET_ID_DIGEST: usize = I_CHAIN_ID_DIGEST + 4;
pub(crate) const I_TOPUP_OPERATION_ID: usize = I_ASSET_ID_DIGEST + 4;
pub(crate) const I_ARTIFACT_MANIFEST_SHA256: usize = I_TOPUP_OPERATION_ID + 4;
pub(crate) const I_CURRENT_HOP_DOMAIN_TAG: usize = I_ARTIFACT_MANIFEST_SHA256 + 4;
pub(crate) const I_TOPUP_RECEIPT_DIGEST: usize = I_CURRENT_HOP_DOMAIN_TAG + 4;
pub(crate) const I_PARENT_TOPUP_RECEIPT_DIGEST: usize = I_TOPUP_RECEIPT_DIGEST + 4;
pub(crate) const I_TOPUP_ANCHOR_DIGEST: usize = I_PARENT_TOPUP_RECEIPT_DIGEST + 4;
pub(crate) const I_TOPUP_ANCHOR_COUNT: usize = I_TOPUP_ANCHOR_DIGEST + 4;
pub(crate) const I_VERIFIER_KEY_ID_DIGEST: usize = I_TOPUP_ANCHOR_COUNT + 1;
pub(crate) const I_REDEMPTION_PROFILE: usize = I_VERIFIER_KEY_ID_DIGEST + 4;
pub(crate) const I_PARENT_FINAL_ROOT: usize = I_REDEMPTION_PROFILE + 1;
pub(crate) const I_REDEMPTION_RECIPIENT_DIGEST: usize = I_PARENT_FINAL_ROOT + 1;
pub(crate) const I_UNSHIELD_PUBLIC_INPUTS_DIGEST: usize = I_REDEMPTION_RECIPIENT_DIGEST + 4;
pub(crate) const I_UNSHIELD_PUBLIC_AMOUNT: usize = I_UNSHIELD_PUBLIC_INPUTS_DIGEST + 4;

/// Public-input contract for the secure Kagemusha output-membership relation.
///
/// Paths remain witness values, but every path direction is constrained to the
/// corresponding public leaf index and every recomputed path root is constrained
/// to these public roots and commitments. The relation reuses the unchanged
/// Axiom Poseidon specification and leaf/node domains of the confidential-note
/// primitive.
pub const KAGEMUSHA_OUTPUT_MEMBERSHIP_PUBLIC_INPUTS_SCHEMA_V4: &[u8] = br#"{"schema":"kagemusha_output_membership_v4","hash":"axiom_poseidon_t3_r2_rf8_rp57_mds0","merkle_leaf_domain":"cfleaf03","merkle_node_domain":"cfnode03","public_inputs":["is_init","is_split","is_redemption_change","has_change","initial_root","final_root","recipient_commitment","recipient_leaf_index","change_commitment","change_leaf_index","dummy_leaf_index"]}"#;

/// IPA domain exponent used by the fixed output-membership relation.
pub const KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V4: u32 = 12;
/// Number of one-row instance columns exposed by the output-membership relation.
pub const KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V4: usize = 11;

/// Operation profile selected by one output-membership proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaOutputMembershipOperationV4 {
    /// Initial note inserted by a finalized top-up.
    Init,
    /// Offline split with one recipient output and optional sender change.
    Split,
    /// Partial redemption with one confidential change output.
    RedemptionChange,
}

/// One proof-bound output and both paths needed to authenticate its creation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaOutputMembershipLeafV4 {
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
pub struct KagemushaOutputMembershipWitnessV4 {
    /// Operation whose exact output-presence rules are enforced in-circuit.
    pub operation: KagemushaOutputMembershipOperationV4,
    /// Commitment-tree root before inserting this operation's outputs.
    pub initial_root: [u8; 32],
    /// Commitment-tree root after inserting every present output.
    pub final_root: [u8; 32],
    /// Recipient output. Required by init/split and absent for redemption change.
    pub recipient: Option<KagemushaOutputMembershipLeafV4>,
    /// Sender change. Optional for split, required for redemption change, absent for init.
    pub change: Option<KagemushaOutputMembershipLeafV4>,
    /// First empty leaf after every output inserted by this operation.
    pub dummy_leaf_index: u32,
    /// Empty-leaf membership path under `final_root` for `dummy_leaf_index`.
    pub dummy_path: iroha_data_model::offline::KagemushaConfidentialMerklePathV2,
}

/// Eq/Fp secure Poseidon relation proving output insertion and final membership.
///
/// This circuit is intentionally separate from the symmetric transition-only
/// relation: confidential-tree roots are Pallas scalar-field elements and must
/// not be re-hashed natively in the reciprocal Fq step.  A production StepEq
/// composition consumes this relation together with the field-neutral state
/// boundary. Production readiness remains false until the complete
/// authenticated runtime, review, and device-evidence conjunction succeeds.
#[derive(Clone, Debug, Default)]
pub struct KagemushaOutputMembershipCircuitV4 {
    witness: Option<KagemushaOutputMembershipWitnessV4>,
}

impl KagemushaOutputMembershipCircuitV4 {
    /// Construct the circuit after checking fixed path sizes and scalar encodings.
    pub fn new(witness: KagemushaOutputMembershipWitnessV4) -> Result<Self, String> {
        output_membership_v4::validate_witness_shape(&witness)?;
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
        output_membership_v4::public_instances(witness)
    }
}

pub(in crate::zk) mod output_membership_v4 {
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
        KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V4, KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V4,
        KagemushaOutputMembershipCircuitV4, KagemushaOutputMembershipLeafV4,
        KagemushaOutputMembershipOperationV4, KagemushaOutputMembershipWitnessV4, Scalar,
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
        leaf: &KagemushaOutputMembershipLeafV4,
        label: &str,
    ) -> Result<(), String> {
        scalar_from_repr(leaf.commitment)
            .ok_or_else(|| format!("{label} commitment is not a canonical Pallas scalar"))?;
        validate_path_encoding(&leaf.update_path, &format!("{label} update path"))?;
        validate_path_encoding(&leaf.membership_path, &format!("{label} membership path"))
    }

    pub(super) fn validate_witness_encoding(
        witness: &KagemushaOutputMembershipWitnessV4,
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

    pub(super) fn validate_witness_shape(
        witness: &KagemushaOutputMembershipWitnessV4,
    ) -> Result<(), String> {
        validate_witness_encoding(witness)?;
        if witness.initial_root == [0; 32]
            || witness.final_root == [0; 32]
            || witness.initial_root == witness.final_root
            || witness.dummy_path.root != witness.final_root
            || witness
                .dummy_path
                .leaf_index()
                .map_err(|error| error.to_string())?
                != witness.dummy_leaf_index
        {
            return Err("Kagemusha output-membership roots or dummy path mismatch".to_owned());
        }
        for (label, leaf) in [
            ("recipient", witness.recipient.as_ref()),
            ("change", witness.change.as_ref()),
        ] {
            if let Some(leaf) = leaf {
                if leaf.commitment == [0; 32]
                    || leaf
                        .update_path
                        .leaf_index()
                        .map_err(|error| error.to_string())?
                        != leaf.leaf_index
                    || leaf
                        .membership_path
                        .leaf_index()
                        .map_err(|error| error.to_string())?
                        != leaf.leaf_index
                    || leaf.membership_path.root != witness.final_root
                {
                    return Err(format!("Kagemusha output-membership {label} path mismatch"));
                }
            }
        }
        let (first, last) = match (
            witness.operation,
            witness.recipient.as_ref(),
            witness.change.as_ref(),
        ) {
            (KagemushaOutputMembershipOperationV4::Init, Some(recipient), None) => {
                (recipient, recipient)
            }
            (KagemushaOutputMembershipOperationV4::Split, Some(recipient), None) => {
                (recipient, recipient)
            }
            (KagemushaOutputMembershipOperationV4::Split, Some(recipient), Some(change))
                if recipient.leaf_index.checked_add(1) == Some(change.leaf_index) =>
            {
                (recipient, change)
            }
            (KagemushaOutputMembershipOperationV4::RedemptionChange, None, Some(change)) => {
                (change, change)
            }
            _ => return Err("Kagemusha output-membership operation shape mismatch".to_owned()),
        };
        if first.update_path.root != witness.initial_root
            || last.leaf_index.checked_add(1) != Some(witness.dummy_leaf_index)
        {
            return Err("Kagemusha output-membership frontier is not consecutive".to_owned());
        }
        Ok(())
    }

    fn scalar(bytes: [u8; 32], label: &str) -> Result<Scalar, String> {
        scalar_from_repr(bytes).ok_or_else(|| format!("{label} is not a canonical Pallas scalar"))
    }

    fn profile_flags(operation: KagemushaOutputMembershipOperationV4) -> [Scalar; 3] {
        match operation {
            KagemushaOutputMembershipOperationV4::Init => [Scalar::ONE, Scalar::ZERO, Scalar::ZERO],
            KagemushaOutputMembershipOperationV4::Split => {
                [Scalar::ZERO, Scalar::ONE, Scalar::ZERO]
            }
            KagemushaOutputMembershipOperationV4::RedemptionChange => {
                [Scalar::ZERO, Scalar::ZERO, Scalar::ONE]
            }
        }
    }

    pub(super) fn public_instances(
        witness: &KagemushaOutputMembershipWitnessV4,
    ) -> Result<Vec<Vec<Scalar>>, String> {
        validate_witness_shape(witness)?;
        public_instances_unchecked(witness)
    }

    pub(super) fn public_instances_unchecked(
        witness: &KagemushaOutputMembershipWitnessV4,
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
            KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V4
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

    /// Assign the full Poseidon update/membership relation into an existing builder context.
    ///
    /// Returned cells follow
    /// [`KAGEMUSHA_OUTPUT_MEMBERSHIP_PUBLIC_INPUTS_SCHEMA_V4`](super::KAGEMUSHA_OUTPUT_MEMBERSHIP_PUBLIC_INPUTS_SCHEMA_V4)
    /// exactly, so callers can either expose them as instance columns or copy-constrain
    /// them to the corresponding transition and exact-state cells.
    pub(crate) fn assign_kagemusha_output_membership_v4(
        ctx: &mut Context<Scalar>,
        range: &halo2_base::gates::RangeChip<Scalar>,
        witness: Option<&KagemushaOutputMembershipWitnessV4>,
    ) -> Result<[AssignedValue<Scalar>; KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V4], String>
    {
        if let Some(witness) = witness {
            validate_witness_encoding(witness)?;
        }
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
        assert_present_exactly(ctx, range, recipient_commitment, recipient_present);
        assert_present_exactly(ctx, range, change_commitment, has_change);

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
        let poseidon = ConfidentialPoseidonChipV3::new(ctx, range);

        let recipient_update_path = recipient.map(|leaf| &leaf.update_path);
        let (recipient_before, recipient_after) = update_roots(
            ctx,
            range,
            &poseidon,
            recipient_commitment,
            recipient_update_path,
            &recipient_index_bits,
            recipient_present,
        );
        assert_equal_if(
            ctx,
            range,
            recipient_present,
            recipient_before,
            initial_root,
        );
        let root_after_recipient =
            gate.select(ctx, recipient_after, initial_root, recipient_present);

        let change_update_path = change.map(|leaf| &leaf.update_path);
        let (change_before, change_after) = update_roots(
            ctx,
            range,
            &poseidon,
            change_commitment,
            change_update_path,
            &change_index_bits,
            has_change,
        );
        assert_equal_if(ctx, range, has_change, change_before, root_after_recipient);
        let computed_final = gate.select(ctx, change_after, root_after_recipient, has_change);
        assert_equal(ctx, range, computed_final, final_root);

        let split_with_change = gate.mul(ctx, is_split, has_change);
        let one = ctx.load_constant(Scalar::ONE);
        let next_recipient_index = gate.add(ctx, recipient_index, one);
        assert_equal_if(
            ctx,
            range,
            split_with_change,
            change_index,
            next_recipient_index,
        );
        let last_output_index = gate.select(ctx, change_index, recipient_index, has_change);
        let next_output_index = gate.add(ctx, last_output_index, one);
        assert_equal(ctx, range, dummy_index, next_output_index);
        let same_output_index = gate.is_equal(ctx, recipient_index, change_index);
        let both_outputs = gate.mul(ctx, recipient_present, has_change);
        let duplicate_output_index = gate.mul(ctx, both_outputs, same_output_index);
        gate.assert_is_const(ctx, &duplicate_output_index, &Scalar::ZERO);
        let same_commitment = gate.is_equal(ctx, recipient_commitment, change_commitment);
        let duplicate_commitment = gate.mul(ctx, both_outputs, same_commitment);
        gate.assert_is_const(ctx, &duplicate_commitment, &Scalar::ZERO);

        membership_root(
            ctx,
            range,
            &poseidon,
            recipient_commitment,
            recipient.map(|leaf| &leaf.membership_path),
            &recipient_index_bits,
            recipient_present,
            final_root,
        );
        membership_root(
            ctx,
            range,
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
            range,
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

        Ok([
            is_init,
            is_split,
            is_redemption,
            has_change,
            initial_root,
            final_root,
            recipient_commitment,
            recipient_index,
            change_commitment,
            change_index,
            dummy_index,
        ])
    }

    fn builder(
        witness: Option<&KagemushaOutputMembershipWitnessV4>,
    ) -> Result<BaseCircuitBuilder<Scalar>, String> {
        let mut builder = BaseCircuitBuilder::new(false)
            .use_k(
                usize::try_from(KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V4)
                    .expect("Kagemusha membership IPA k fits usize"),
            )
            .use_lookup_bits(
                usize::try_from(KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V4 - 1)
                    .expect("Kagemusha membership lookup bits fit usize"),
            )
            .use_instance_columns(KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V4);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let bindings = assign_kagemusha_output_membership_v4(ctx, &range, witness)?;
        builder.assigned_instances = bindings.into_iter().map(|cell| vec![cell]).collect();
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        Ok(builder)
    }

    impl Circuit<Scalar> for KagemushaOutputMembershipCircuitV4 {
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

fn canonical_poseidon_digest<T: Encode>(value: &T) -> Result<[u8; 32], String> {
    let bytes = norito::to_bytes(value)
        .map_err(|err| format!("failed to encode Kagemusha V4 binding value: {err}"))?;
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
        statement: &KagemushaRecursiveSpendPublicStatementV4,
    ) -> Result<Self, String> {
        statement
            .validate_public_binding()
            .map_err(|err| err.to_string())?;
        Ok(match statement.transition.as_ref() {
            None => Self::Init,
            Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(_)) => Self::Append,
            Some(KagemushaRecursiveSpendTransitionV4::RedemptionChange(_)) => {
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

impl KagemushaRecursiveSpendStateVectorV2 {
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

    /// Reconstruct the complete continuing-state vector directly from an
    /// ABI-20 statement without projecting it through a legacy carrier.
    pub fn from_statement_v4(
        statement: &KagemushaRecursiveSpendPublicStatementV4,
    ) -> Result<Self, String> {
        statement
            .validate_public_binding()
            .map_err(|err| err.to_string())?;
        let vector = Self::from_statement_v4_inner(statement)?;
        vector.validate_against_statement_v4(statement)?;
        Ok(vector)
    }

    fn from_statement_v4_inner(
        statement: &KagemushaRecursiveSpendPublicStatementV4,
    ) -> Result<Self, String> {
        let mut limbs = [0_u32; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2];
        limbs[S_VERSION] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2;

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
        limbs[S_NEXT_ZERO_LEAF_INDEX] = statement.next_zero_leaf_index;

        limbs[S_TOPUP_ANCHOR_COUNT] = u32::try_from(statement.topup_anchor_refs.len())
            .map_err(|_| "Kagemusha V4 top-up anchor count does not fit u32".to_owned())?;
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
            .map_err(|_| "Kagemusha V4 branch-claim count does not fit u32".to_owned())?;
        for (index, claim) in statement.branch_claims.iter().enumerate() {
            let start =
                S_BRANCH_CLAIMS + index * KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V2;
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
                    + tag_index * KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V2;
                for (limb, chunk) in limbs[tag_start
                    ..tag_start + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_TRANSITION_TAG_LIMBS_V2]
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

    /// Validate every exact ABI-20 state limb against its canonical statement.
    pub fn validate_against_statement_v4(
        &self,
        statement: &KagemushaRecursiveSpendPublicStatementV4,
    ) -> Result<(), String> {
        statement
            .validate_public_binding()
            .map_err(|err| err.to_string())?;
        if self.limbs[S_VERSION] != KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2 {
            return Err("Kagemusha V4 recursive state-vector version mismatch".to_owned());
        }
        if self != &Self::from_statement_v4_inner(statement)? {
            return Err(
                "Kagemusha V4 recursive state vector does not exactly match the statement"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

fn encode_kagemusha_u32_scalar_v4(value: u32) -> [u8; 32] {
    let mut encoded = [0_u8; 32];
    encoded[..4].copy_from_slice(&value.to_le_bytes());
    encoded
}

fn kagemusha_public_inputs_for_statement_v4(
    statement: &KagemushaRecursiveSpendPublicStatementV4,
    operation: KagemushaStepOperationVectorV4,
    expected_manifest_sha256: [u8; 32],
    step_eq_compiled_protocol_sha256: [u8; 32],
    step_ep_compiled_protocol_sha256: [u8; 32],
) -> Result<super::kagemusha_recursion_adapter::KagemushaPastaCyclePublicInputsV4, String> {
    use super::kagemusha_recursion_adapter::{
        KAGEMUSHA_PASTA_PARENT_SLOTS_V1, KagemushaPastaCyclePublicInputsV4,
        kagemusha_sha256_public_words,
    };

    statement
        .validate_public_binding()
        .map_err(|error| error.to_string())?;
    if statement.artifact_binding.manifest_sha256 != expected_manifest_sha256 {
        return Err(
            "Kagemusha V4 statement selects a different authenticated artifact release".to_owned(),
        );
    }
    let statement_digest = statement.digest().map_err(|error| error.to_string())?;
    let state = KagemushaRecursiveSpendStateVectorV2::from_statement_v4(statement)?;
    Ok(KagemushaPastaCyclePublicInputsV4 {
        public_statement_digest: bytes_to_exact_u32_limbs(&statement_digest),
        operation,
        // The prover derives these fields from terminally verified opaque
        // parents. Callers never supply a state or lineage accumulator.
        parent_count: 0,
        parent_states: std::array::from_fn(|_| {
            vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2]
        }),
        result_state: state.limbs.to_vec(),
        manifest_sha256: bytes_to_exact_u32_limbs(&expected_manifest_sha256),
        step_eq_compiled_protocol_sha256: kagemusha_sha256_public_words(
            step_eq_compiled_protocol_sha256,
        ),
        step_ep_compiled_protocol_sha256: kagemusha_sha256_public_words(
            step_ep_compiled_protocol_sha256,
        ),
        parent_eq_lineage_accumulator: None,
        parent_ep_lineage_accumulator: None,
        parent_eq_deferred_sha256: [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
        parent_ep_deferred_sha256: [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
        live_selector: KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4,
    })
}

/// Opaque ABI-20 prover facade around the private Pasta recursion adapter.
///
/// Lifecycle callers select a semantic operation and provide its confidential
/// openings, but never receive or construct fold transcripts, accumulator
/// wires, bootstrap slots, or raw recursion public inputs.
pub struct KagemushaPastaCycleOpaqueProverV4 {
    inner: super::kagemusha_recursion_adapter::KagemushaPastaCycleProverV4,
    manifest_sha256: [u8; 32],
}

impl KagemushaPastaCycleOpaqueProverV4 {
    /// Parse and cross-check the complete authenticated ABI-20 prover set.
    pub fn from_authenticated_artifacts(
        artifacts: &super::kagemusha_artifact_v4::KagemushaPastaCycleProverArtifactsV4,
    ) -> Result<Self, String> {
        Ok(Self {
            inner: super::kagemusha_recursion_adapter::KagemushaPastaCycleProverV4::from_authenticated_artifacts(
                artifacts,
            )?,
            manifest_sha256: artifacts.manifest_sha256(),
        })
    }

    /// Prove the first V4 state directly from the finalized top-up opening and
    /// insertion path, returning only canonical opaque pair bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn prove_init_v4(
        &self,
        request: &iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV4,
        statement: &KagemushaRecursiveSpendPublicStatementV4,
        spend_key: &[u8],
        rho: [u8; 32],
        diversifier: [u8; 32],
        zero_path: &super::confidential_v2::ConfidentialMerklePathV2,
        output_membership: &KagemushaOutputMembershipWitnessV4,
    ) -> Result<Vec<u8>, String> {
        use iroha_data_model::offline::kagemusha_confidential_amount_encoding_v2;

        let anchor = &request.topup_anchor;
        let topup = super::confidential_v2::KagemushaTopUpShieldPublicInputsV2 {
            output_commitment: anchor.current_note.note_commitment,
            spend_nullifier: anchor.current_note.spend_nullifier,
            initial_root: anchor.initial_root,
            finalized_root: anchor.finalized_root,
            atomic_amount: kagemusha_confidential_amount_encoding_v2(anchor.amount.atomic_units),
            asset_scale: encode_kagemusha_u32_scalar_v4(anchor.asset_scale),
            leaf_index: encode_kagemusha_u32_scalar_v4(anchor.shield_leaf_index),
            asset_tag: super::confidential_v2::derive_confidential_asset_tag_v3(
                &anchor.asset.definition().to_string(),
            )?,
            chain_tag: super::confidential_v2::derive_confidential_chain_tag_v3(
                anchor.chain_id.as_str(),
            )?,
            payer_tag: super::confidential_v2::derive_kagemusha_topup_payer_tag_v3(
                &anchor.payer.to_string(),
            )?,
            operation_tag: super::confidential_v2::derive_kagemusha_topup_operation_tag_v3(
                &anchor.topup_operation_id,
            )?,
        };
        let operation = KagemushaStepOperationVectorV4::from_init_v4(
            request,
            statement,
            &topup,
            output_membership,
        )?;
        let secure = super::confidential_v2::prepare_kagemusha_step_topup_witness_v3(
            &anchor.chain_id,
            &anchor.asset.definition().to_string(),
            &anchor.payer.to_string(),
            anchor.topup_operation_id,
            anchor.amount.atomic_units,
            anchor.asset_scale,
            spend_key,
            rho,
            diversifier,
            anchor.shield_leaf_index,
            zero_path,
        )?;
        let public_inputs = kagemusha_public_inputs_for_statement_v4(
            statement,
            operation,
            self.manifest_sha256,
            self.inner.step_eq_compiled_protocol_sha256(),
            self.inner.step_ep_compiled_protocol_sha256(),
        )?;
        self.inner.prove_operation_encoded_v4(
            public_inputs,
            statement.proof_step_count,
            &[],
            &secure,
            output_membership,
        )
    }

    /// Prove one V4 recipient or change append branch from the exact secure
    /// transfer witness and terminally verified opaque parent pairs.
    #[allow(clippy::too_many_arguments)]
    pub fn prove_append_v4(
        &self,
        split: &iroha_data_model::offline::KagemushaRecursiveSpendSplitIntentV4,
        statement: &KagemushaRecursiveSpendPublicStatementV4,
        transfer: &KagemushaStepTransferPublicV4,
        spend_key: &[u8],
        input_paths: &[super::confidential_v2::ConfidentialMerklePathV2],
        inputs: &[super::confidential_v2::ConfidentialTransferInputV2],
        outputs: &[super::confidential_v2::ConfidentialTransferOutputV2],
        output_membership: &KagemushaOutputMembershipWitnessV4,
        parent_pair_bytes: &[&[u8]],
    ) -> Result<Vec<u8>, String> {
        if parent_pair_bytes.len() != split.inputs.len() {
            return Err("Kagemusha V4 append parent proof count mismatch".to_owned());
        }
        let operation = KagemushaStepOperationVectorV4::from_append_v4(
            split,
            statement,
            transfer,
            output_membership,
        )?;
        let secure = super::confidential_v2::prepare_kagemusha_step_transfer_witness_v3_with_paths(
            &split.chain_id,
            &split.asset.to_string(),
            spend_key,
            input_paths,
            inputs,
            outputs,
            split.inputs[0].input_root,
        )?;
        let public_inputs = kagemusha_public_inputs_for_statement_v4(
            statement,
            operation,
            self.manifest_sha256,
            self.inner.step_eq_compiled_protocol_sha256(),
            self.inner.step_ep_compiled_protocol_sha256(),
        )?;
        self.inner.prove_operation_encoded_v4(
            public_inputs,
            statement.proof_step_count,
            parent_pair_bytes,
            &secure,
            output_membership,
        )
    }

    /// Prove a partial-redemption V4 change child. Full redemption deliberately
    /// has no child proof and is handled only by terminal parent verification.
    #[allow(clippy::too_many_arguments)]
    pub fn prove_redemption_change_v4(
        &self,
        redemption: &iroha_data_model::offline::KagemushaRecursiveSpendRedemptionIntentV4,
        statement: &KagemushaRecursiveSpendPublicStatementV4,
        spend_key: &[u8],
        input_paths: &[super::confidential_v2::ConfidentialMerklePathV2],
        inputs: &[super::confidential_v2::ConfidentialUnshieldInputV2],
        outputs: &[super::confidential_v2::ConfidentialUnshieldOutputV3],
        output_membership: &KagemushaOutputMembershipWitnessV4,
        parent_pair_bytes: &[&[u8]],
    ) -> Result<Vec<u8>, String> {
        if parent_pair_bytes.len() != 1 {
            return Err(
                "Kagemusha V4 redemption change requires exactly one parent proof".to_owned(),
            );
        }
        let operation = KagemushaStepOperationVectorV4::from_redemption_change_v4(
            redemption,
            statement,
            output_membership,
        )?;
        let secure =
            super::confidential_v2::prepare_kagemusha_step_unshield_change_witness_v4_with_paths(
                &redemption.chain_id,
                &redemption.asset.to_string(),
                spend_key,
                input_paths,
                inputs,
                outputs,
                redemption.public_amount.atomic_units,
                redemption.input_root,
            )?;
        let public_inputs = kagemusha_public_inputs_for_statement_v4(
            statement,
            operation,
            self.manifest_sha256,
            self.inner.step_eq_compiled_protocol_sha256(),
            self.inner.step_ep_compiled_protocol_sha256(),
        )?;
        self.inner.prove_operation_encoded_v4(
            public_inputs,
            statement.proof_step_count,
            parent_pair_bytes,
            &secure,
            output_membership,
        )
    }
}

/// Opaque ABI-20 terminal-verifier facade. Public verification accepts only a
/// complete V4 bundle; recursion-specific pair internals remain private.
pub struct KagemushaPastaCycleOpaqueVerifierV4 {
    inner: super::kagemusha_recursion_adapter::KagemushaPastaCycleTerminalVerifierV4,
    manifest: KagemushaRecursiveSpendArtifactManifestV4,
    manifest_sha256: [u8; 32],
    candidate_evidence_lab: bool,
}

fn canonical_bundle_operation_v4(
    bundle: &KagemushaRecursiveSpendBundleV4,
) -> Result<KagemushaStepOperationVectorV4, String> {
    let operation = KagemushaStepOperationVectorV4::from(&bundle.operation);
    operation.to_fields()?;
    Ok(operation)
}

fn ensure_bundle_operation_v4(
    bundle: &KagemushaRecursiveSpendBundleV4,
    expected_operation: &KagemushaStepOperationVectorV4,
) -> Result<(), String> {
    let carried_operation = canonical_bundle_operation_v4(bundle)?;
    if &carried_operation != expected_operation {
        return Err(
            "Kagemusha V4 bundle operation does not match the submitted lifecycle operation"
                .to_owned(),
        );
    }
    Ok(())
}

impl KagemushaPastaCycleOpaqueVerifierV4 {
    /// Parse and cross-check the complete authenticated ABI-20 verifier set.
    pub fn from_authenticated_artifacts(
        artifacts: &super::kagemusha_artifact_v4::KagemushaPastaCycleVerifierArtifactsV4,
    ) -> Result<Self, String> {
        let inner = super::kagemusha_recursion_adapter::KagemushaPastaCycleTerminalVerifierV4::from_authenticated_artifacts(
            artifacts,
        )?;
        Ok(Self {
            inner,
            manifest: artifacts.manifest().clone(),
            manifest_sha256: artifacts.manifest_sha256(),
            candidate_evidence_lab: artifacts.is_candidate_evidence_lab(),
        })
    }

    /// Verify a complete V4 bundle against its authenticated release and
    /// canonical public statement.
    pub fn verify_bundle_v4(&self, bundle: &KagemushaRecursiveSpendBundleV4) -> Result<(), String> {
        let operation = canonical_bundle_operation_v4(bundle)?;
        self.verify_bundle_binding_v4(bundle, &operation)
    }

    /// Verify a complete V4 bundle and the exact lifecycle operation that
    /// produced it.
    pub fn verify_bundle_operation_v4(
        &self,
        bundle: &KagemushaRecursiveSpendBundleV4,
        expected_operation: &KagemushaStepOperationVectorV4,
    ) -> Result<(), String> {
        ensure_bundle_operation_v4(bundle, expected_operation)?;
        self.verify_bundle_binding_v4(bundle, expected_operation)
    }

    fn verify_bundle_binding_v4(
        &self,
        bundle: &KagemushaRecursiveSpendBundleV4,
        expected_operation: &KagemushaStepOperationVectorV4,
    ) -> Result<(), String> {
        ensure_kagemusha_recursive_spend_v4_proof_envelope_binding(
            bundle,
            &self.manifest,
            self.candidate_evidence_lab,
        )?;
        self.verify_binding_v4(
            &bundle.statement,
            expected_operation,
            &bundle.recursive_proof.proof_envelope.proof.bytes,
        )
    }

    fn verify_binding_v4(
        &self,
        statement: &KagemushaRecursiveSpendPublicStatementV4,
        expected_operation: &KagemushaStepOperationVectorV4,
        encoded_pair: &[u8],
    ) -> Result<(), String> {
        statement
            .validate_public_binding()
            .map_err(|error| error.to_string())?;
        if statement.artifact_binding.manifest_sha256 != self.manifest_sha256 {
            return Err(
                "Kagemusha V4 statement selects a different authenticated verifier release"
                    .to_owned(),
            );
        }
        let statement_digest = statement.digest().map_err(|error| error.to_string())?;
        let state = KagemushaRecursiveSpendStateVectorV2::from_statement_v4(statement)?;
        self.inner.verify_encoded_pair_binding(
            encoded_pair,
            statement,
            expected_operation,
            bytes_to_exact_u32_limbs(&statement_digest),
            &state.limbs,
            statement.proof_step_count,
            bytes_to_exact_u32_limbs(&self.manifest_sha256),
        )
    }
}

/// Require a complete ABI-20 bundle to bind its opaque pair envelope to the
/// authenticated release and to the exact statement-derived state boundary.
pub(crate) fn ensure_kagemusha_recursive_spend_v4_proof_envelope_binding(
    bundle: &KagemushaRecursiveSpendBundleV4,
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    candidate_evidence_lab: bool,
) -> Result<(), String> {
    bundle
        .validate_public_binding()
        .map_err(|error| error.to_string())?;
    let envelope = &bundle.recursive_proof.proof_envelope;
    if candidate_evidence_lab {
        #[cfg(feature = "kagemusha-candidate-evidence-lab")]
        envelope
            .validate_against_candidate_manifest(manifest)
            .map_err(|error| error.to_string())?;
        #[cfg(not(feature = "kagemusha-candidate-evidence-lab"))]
        return Err("Kagemusha candidate evidence lab is not compiled".to_owned());
    } else {
        envelope
            .validate_against_manifest(manifest)
            .map_err(|error| error.to_string())?;
    }
    ensure_kagemusha_recursive_spend_v4_state_boundary_binding(bundle)
}

fn ensure_kagemusha_recursive_spend_v4_state_boundary_binding(
    bundle: &KagemushaRecursiveSpendBundleV4,
) -> Result<(), String> {
    let envelope = &bundle.recursive_proof.proof_envelope;
    let state = KagemushaRecursiveSpendStateVectorV2::from_statement_v4(&bundle.statement)?;
    let expected_state_boundary = KagemushaRecursiveSpendStateBoundaryV2::new(state.limbs.to_vec())
        .map_err(|error| error.to_string())?;
    if envelope.state_boundary != expected_state_boundary {
        return Err(
            "Kagemusha V4 proof envelope state boundary does not match the canonical public statement"
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{output_membership_v4::assign_kagemusha_output_membership_v4, *};
    use ff::{Field as _, PrimeField as _};

    fn scalar_bytes(value: u64) -> [u8; 32] {
        let repr = Scalar::from(value).to_repr();
        let mut bytes = [0; 32];
        bytes.copy_from_slice(repr.as_ref());
        bytes
    }

    fn init_statement() -> KagemushaRecursiveSpendPublicStatementV4 {
        use iroha_data_model::{
            ChainId,
            asset::AssetDefinitionId,
            domain::DomainId,
            offline::{
                KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4, KagemushaPastaCycleParityV1,
                KagemushaRecursiveSpendArtifactBindingV4, KagemushaRecursiveSpendBranchClaimV2,
                KagemushaRecursiveSpendBranchPathV2, KagemushaRecursiveSpendTopUpAnchorRefV2,
                KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2,
                kagemusha_recursive_spend_verifier_key_id_v4,
            },
        };

        let chain_id = ChainId::from("kagemusha-v4-statement-binding");
        let asset = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("asset domain"),
            "rose".parse().expect("asset name"),
        );
        let anchor = KagemushaRecursiveSpendTopUpAnchorRefV2 {
            topup_operation_id: [0x41; 32],
            anchor_digest: [0x42; 32],
        };
        let manifest_sha256 = [0x44; 32];
        KagemushaRecursiveSpendPublicStatementV4 {
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            asset_scale: 9,
            final_root: scalar_bytes(12),
            next_zero_leaf_index: 8,
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
            artifact_binding: KagemushaRecursiveSpendArtifactBindingV4 {
                version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
                generation: "v4-envelope-binding-test".to_owned(),
                manifest_sha256,
            },
            verifier_key_id: kagemusha_recursive_spend_verifier_key_id_v4(
                KagemushaPastaCycleParityV1::StepEq,
                manifest_sha256,
            ),
        }
    }

    fn v4_bound_init_bundle_for_envelope_test() -> KagemushaRecursiveSpendBundleV4 {
        use iroha_data_model::{
            offline::{
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
                KagemushaPastaCycleProofEnvelopeV4, KagemushaRecursiveSpendOperationVectorV4,
                KagemushaRecursiveSpendProofV4,
            },
            proof::ProofBox,
        };

        let statement = init_statement();
        let generation = statement.artifact_binding.generation.clone();
        let manifest_sha256 = statement.artifact_binding.manifest_sha256;
        statement
            .validate_public_binding()
            .expect("canonical V4 statement");
        let state = KagemushaRecursiveSpendStateVectorV2::from_statement_v4(&statement)
            .expect("canonical V4 state");
        let statement_digest = statement.digest().expect("canonical V4 statement digest");
        let proof_envelope = KagemushaPastaCycleProofEnvelopeV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
            step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
            step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4.to_owned(),
            artifact_generation: generation,
            manifest_sha256,
            step_eq_parameter_generation: "v4-envelope-eq-params".to_owned(),
            step_ep_parameter_generation: "v4-envelope-ep-params".to_owned(),
            step_eq_circuit_params_sha256: [0x51; 32],
            step_ep_circuit_params_sha256: [0x52; 32],
            step_eq_verifier_key_sha256: [0x61; 32],
            step_ep_verifier_key_sha256: [0x62; 32],
            state_boundary: KagemushaRecursiveSpendStateBoundaryV2::new(state.limbs.to_vec())
                .expect("canonical V4 state boundary"),
            proof: ProofBox::new(
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
                    .parse()
                    .expect("V4 proof backend"),
                vec![0xA5],
            ),
        };
        let mut operation_limbs =
            [0_u32; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4];
        operation_limbs[0] = 1;
        let bundle = KagemushaRecursiveSpendBundleV4 {
            operation: KagemushaRecursiveSpendOperationVectorV4 {
                limbs: operation_limbs,
            },
            recursive_proof: KagemushaRecursiveSpendProofV4 {
                verifier_key_id: statement.verifier_key_id.clone(),
                public_statement_digest: statement_digest,
                proof_envelope,
            },
            statement,
        };
        bundle
            .validate_public_binding()
            .expect("structurally valid V4 bundle");
        bundle
    }

    #[test]
    fn v4_envelope_state_boundary_is_bound_beyond_structural_bundle_validation() {
        let bundle = v4_bound_init_bundle_for_envelope_test();
        ensure_kagemusha_recursive_spend_v4_state_boundary_binding(&bundle)
            .expect("canonical V4 state boundary");
        let original_digest = bundle.digest().expect("canonical bundle digest");

        let mut substituted = bundle;
        substituted
            .recursive_proof
            .proof_envelope
            .state_boundary
            .state_limbs[1] ^= 1;
        substituted
            .validate_public_binding()
            .expect("structural validation does not derive the statement state");
        assert_ne!(
            substituted.digest().expect("substituted bundle digest"),
            original_digest,
            "the unbound envelope field changes bundle identity"
        );
        assert!(
            ensure_kagemusha_recursive_spend_v4_state_boundary_binding(&substituted).is_err(),
            "bundle-level verification must reject a substituted state boundary"
        );
    }

    #[test]
    fn v4_bundle_operation_carrier_is_canonical_and_substitution_bound() {
        let bundle = v4_bound_init_bundle_for_envelope_test();
        let carried = canonical_bundle_operation_v4(&bundle).expect("canonical carried operation");
        ensure_bundle_operation_v4(&bundle, &carried).expect("exact operation matches");
        let original_digest = bundle.digest().expect("canonical bundle digest");

        let mut substituted = bundle.clone();
        substituted.operation.limbs[8] ^= 1;
        assert!(
            ensure_bundle_operation_v4(&substituted, &carried).is_err(),
            "a substituted public operation must reject before proof verification"
        );
        assert_ne!(
            substituted.digest().expect("substituted bundle digest"),
            original_digest,
            "the carried operation must be part of bundle identity"
        );

        let mut noncanonical = bundle;
        noncanonical.operation.limbs[..8].fill(u32::MAX);
        assert!(
            canonical_bundle_operation_v4(&noncanonical).is_err(),
            "non-canonical Pallas limbs must reject before proof decoding"
        );
    }

    #[test]
    fn recursive_state_vector_layout_is_contiguous_and_exact() {
        use std::collections::BTreeSet;

        let mut next = 0;
        let mut layout_fields = BTreeSet::new();
        for &(field, start, len) in KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_V2 {
            assert_ne!(field, "");
            assert!(
                layout_fields.insert(field),
                "duplicate layout field {field}"
            );
            assert_eq!(start, next, "state-vector field {field} must be contiguous");
            assert!(len > 0, "state-vector field {field} must not be empty");
            next += len;
        }
        assert_eq!(next, KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2);
        assert_eq!(next, 890);
        assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V2, 395);
        let mut statement_fields = BTreeSet::new();
        for &(statement_field, vector_field) in KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_COVERAGE_V2 {
            assert!(statement_fields.insert(statement_field));
            assert!(
                layout_fields.contains(vector_field),
                "continuing field {statement_field} maps to absent slot {vector_field}"
            );
        }
        assert_eq!(statement_fields.len(), 15);
    }

    #[test]
    fn recursive_state_vector_is_exact_and_zero_padded() {
        let statement = init_statement();
        let vector = KagemushaRecursiveSpendStateVectorV2::from_statement_v4(&statement)
            .expect("canonical init state vector");
        assert_eq!(
            vector.limbs[S_VERSION],
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2
        );
        assert_eq!(
            vector.limbs[S_NEXT_ZERO_LEAF_INDEX],
            statement.next_zero_leaf_index
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
                ..S_BRANCH_CLAIMS + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V2]
                .iter()
                .all(|limb| *limb == 0)
        );
        let second_claim = S_BRANCH_CLAIMS + KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_CLAIM_LIMBS_V2;
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
            S_NEXT_ZERO_LEAF_INDEX,
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
                substituted
                    .validate_against_statement_v4(&statement)
                    .is_err(),
                "state-vector substitution at limb {index} must reject"
            );
        }
    }

    #[test]
    fn recursive_state_vector_reference_encoding_is_deterministic() {
        use sha2::{Digest as _, Sha256};

        let vector = KagemushaRecursiveSpendStateVectorV2::from_statement_v4(&init_statement())
            .expect("canonical init state vector");
        let bytes = vector
            .limbs
            .iter()
            .flat_map(|limb| limb.to_le_bytes())
            .collect::<Vec<_>>();
        let actual: [u8; 32] = Sha256::digest(bytes).into();
        let repeated = KagemushaRecursiveSpendStateVectorV2::from_statement_v4(&init_statement())
            .expect("repeated canonical init state vector")
            .limbs
            .iter()
            .flat_map(|limb| limb.to_le_bytes())
            .collect::<Vec<_>>();
        assert_ne!(actual, [0; 32]);
        assert_eq!(actual.as_slice(), Sha256::digest(repeated).as_slice());
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

    fn sparse_output_membership_path(
        commitments: &[Option<[u8; 32]>],
        leaf_index: usize,
    ) -> iroha_data_model::offline::KagemushaConfidentialMerklePathV2 {
        let path = super::super::confidential_v2::compute_confidential_sparse_fixture_path_v3(
            commitments,
            leaf_index,
        )
        .expect("canonical sparse output-membership fixture path");
        if let Some(commitment) = commitments.get(leaf_index).copied().flatten() {
            super::super::confidential_v2::validate_confidential_membership_path_v3(
                commitment, leaf_index, &path,
            )
            .expect("valid sparse fixture membership path");
        } else {
            super::super::confidential_v2::validate_confidential_next_zero_path_v3(
                leaf_index, &path,
            )
            .expect("valid sparse fixture empty-leaf path");
        }
        let (siblings, directions, _witness_nodes, root) = path.into_parts();
        iroha_data_model::offline::KagemushaConfidentialMerklePathV2 {
            siblings,
            directions,
            root,
        }
    }

    fn output_membership_fixture(
        operation: KagemushaOutputMembershipOperationV4,
        include_change: bool,
    ) -> KagemushaOutputMembershipWitnessV4 {
        let mut commitments = match operation {
            KagemushaOutputMembershipOperationV4::Init => Vec::new(),
            KagemushaOutputMembershipOperationV4::Split
            | KagemushaOutputMembershipOperationV4::RedemptionChange => {
                vec![scalar_bytes(700)]
            }
        };
        let initial_root =
            super::super::confidential_v2::compute_confidential_root_v3(&commitments)
                .expect("initial confidential root");

        let mut recipient = None;
        if !matches!(
            operation,
            KagemushaOutputMembershipOperationV4::RedemptionChange
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
            KagemushaOutputMembershipLeafV4 {
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
                |(commitment, leaf_index, update_path)| KagemushaOutputMembershipLeafV4 {
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
        KagemushaOutputMembershipWitnessV4 {
            operation,
            initial_root,
            final_root,
            recipient,
            change,
            dummy_leaf_index,
            dummy_path,
        }
    }

    fn assert_output_membership_satisfied(witness: KagemushaOutputMembershipWitnessV4) {
        let circuit =
            KagemushaOutputMembershipCircuitV4::new(witness).expect("encoded membership witness");
        let instances = circuit.public_instances().expect("membership instances");
        let prover = halo2_proofs::dev::MockProver::run(
            KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V4,
            &circuit,
            instances,
        )
        .expect("output-membership mock prover");
        prover.assert_satisfied();
    }

    fn assert_output_membership_rejected(witness: KagemushaOutputMembershipWitnessV4) {
        let instances = output_membership_v4::public_instances_unchecked(&witness)
            .expect("well-encoded adversarial membership instances");
        let circuit = KagemushaOutputMembershipCircuitV4 {
            witness: Some(witness),
        };
        let prover = halo2_proofs::dev::MockProver::run(
            KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V4,
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
    fn output_membership_assignment_gadget_embeds_in_existing_builder() {
        use halo2_base::gates::circuit::builder::BaseCircuitBuilder;

        let witness = output_membership_fixture(KagemushaOutputMembershipOperationV4::Split, true);
        let public = KagemushaOutputMembershipCircuitV4::new(witness.clone())
            .expect("encoded membership witness")
            .public_instances()
            .expect("membership instances");
        let canonical_final_root =
            super::super::confidential_v2::scalar_from_repr(witness.final_root)
                .expect("canonical fixture root");
        let embedded = |external_final_root: Scalar| {
            let mut builder = BaseCircuitBuilder::<Scalar>::new(false)
                .use_k(
                    usize::try_from(KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V4)
                        .expect("membership k fits usize"),
                )
                .use_lookup_bits(
                    usize::try_from(KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V4 - 1)
                        .expect("membership lookup bits fit usize"),
                )
                .use_instance_columns(KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V4);
            let range = builder.range_chip();
            let ctx = builder.main(0);
            let bindings = assign_kagemusha_output_membership_v4(ctx, &range, Some(&witness))
                .expect("embedded membership assignment");
            let external_final_root = ctx.load_witness(external_final_root);
            ctx.constrain_equal(&bindings[5], &external_final_root);
            builder.assigned_instances = bindings.into_iter().map(|cell| vec![cell]).collect();
            builder.calculate_params(Some(9));
            builder
        };

        let accepted = embedded(canonical_final_root);
        halo2_proofs::dev::MockProver::run(
            KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V4,
            &accepted,
            public.clone(),
        )
        .expect("embedded output-membership mock prover")
        .assert_satisfied();

        let rejected = embedded(canonical_final_root + Scalar::ONE);
        assert!(
            halo2_proofs::dev::MockProver::run(
                KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V4,
                &rejected,
                public,
            )
            .expect("well-shaped mismatched external binding")
            .verify()
            .is_err(),
            "the reusable gadget's returned final-root cell must enforce embedding equality"
        );
    }

    #[test]
    fn output_membership_poseidon_update_accepts_every_operation_shape() {
        for witness in [
            output_membership_fixture(KagemushaOutputMembershipOperationV4::Init, false),
            output_membership_fixture(KagemushaOutputMembershipOperationV4::Split, false),
            output_membership_fixture(KagemushaOutputMembershipOperationV4::Split, true),
            output_membership_fixture(KagemushaOutputMembershipOperationV4::RedemptionChange, true),
        ] {
            assert_output_membership_satisfied(witness);
        }
    }

    #[test]
    fn output_membership_poseidon_update_rejects_public_substitution() {
        let witness = output_membership_fixture(KagemushaOutputMembershipOperationV4::Split, true);
        let circuit = KagemushaOutputMembershipCircuitV4::new(witness)
            .expect("encoded two-output membership witness");
        let canonical = circuit.public_instances().expect("membership instances");
        for column in 0..KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V4 {
            let mut substituted = canonical.clone();
            substituted[column][0] += Scalar::ONE;
            let prover = halo2_proofs::dev::MockProver::run(
                KAGEMUSHA_OUTPUT_MEMBERSHIP_IPA_K_V4,
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
        type Mutation = fn(&mut KagemushaOutputMembershipWitnessV4);
        let mutations: [(&str, Mutation); 22] = [
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
            ("recipient membership direction", |witness| {
                let path = &mut witness
                    .recipient
                    .as_mut()
                    .expect("recipient")
                    .membership_path;
                path.directions[0] ^= 1;
            }),
            ("recipient membership root", |witness| {
                let path = &mut witness
                    .recipient
                    .as_mut()
                    .expect("recipient")
                    .membership_path;
                path.root = bump_scalar_bytes(path.root);
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
            ("change update direction", |witness| {
                let path = &mut witness.change.as_mut().expect("change").update_path;
                path.directions[0] ^= 1;
            }),
            ("change update root", |witness| {
                let path = &mut witness.change.as_mut().expect("change").update_path;
                path.root = bump_scalar_bytes(path.root);
            }),
            ("change membership sibling", |witness| {
                let path = &mut witness.change.as_mut().expect("change").membership_path;
                path.siblings[1] = bump_scalar_bytes(path.siblings[1]);
            }),
            ("change membership direction", |witness| {
                let path = &mut witness.change.as_mut().expect("change").membership_path;
                path.directions[0] ^= 1;
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
            ("dummy direction", |witness| {
                witness.dummy_path.directions[0] ^= 1;
            }),
            ("dummy root", |witness| {
                witness.dummy_path.root = bump_scalar_bytes(witness.dummy_path.root);
            }),
        ];
        for (name, mutate) in mutations {
            let mut witness =
                output_membership_fixture(KagemushaOutputMembershipOperationV4::Split, true);
            mutate(&mut witness);
            assert_output_membership_rejected(witness);
            let _ = name;
        }
    }

    #[test]
    fn output_membership_rejects_a_valid_empty_path_that_skips_the_frontier() {
        let mut witness =
            output_membership_fixture(KagemushaOutputMembershipOperationV4::Split, true);
        let commitments = [scalar_bytes(700), scalar_bytes(701), scalar_bytes(702)];
        let skipped = witness
            .dummy_leaf_index
            .checked_add(1)
            .expect("fixture frontier increment");
        witness.dummy_leaf_index = skipped;
        witness.dummy_path = output_membership_path(
            &commitments,
            usize::try_from(skipped).expect("skipped index fits usize"),
        );
        assert_eq!(witness.dummy_path.root, witness.final_root);
        assert_output_membership_rejected(witness);
    }

    #[test]
    fn output_membership_rejects_nonconsecutive_change_with_valid_merkle_paths() {
        let initial_commitments = [scalar_bytes(700)];
        let recipient_commitment = scalar_bytes(701);
        let change_commitment = scalar_bytes(702);
        let after_recipient = [initial_commitments[0], recipient_commitment];
        let final_commitments = [
            Some(initial_commitments[0]),
            Some(recipient_commitment),
            None,
            Some(change_commitment),
        ];
        let initial_root =
            super::super::confidential_v2::compute_confidential_root_v3(&initial_commitments)
                .expect("initial root");
        let after_recipient_root =
            super::super::confidential_v2::compute_confidential_root_v3(&after_recipient)
                .expect("root after recipient insertion");
        let recipient_update_path = output_membership_path(&initial_commitments, 1);
        let recipient_membership_path = sparse_output_membership_path(&final_commitments, 1);
        let change_update_path = output_membership_path(&after_recipient, 3);
        let change_membership_path = sparse_output_membership_path(&final_commitments, 3);
        let dummy_path = sparse_output_membership_path(&final_commitments, 4);
        let final_root = dummy_path.root;

        assert_eq!(recipient_update_path.root, initial_root);
        assert_eq!(change_update_path.root, after_recipient_root);
        assert_eq!(recipient_membership_path.root, final_root);
        assert_eq!(change_membership_path.root, final_root);
        assert_eq!(change_update_path.siblings, change_membership_path.siblings);
        assert_eq!(
            change_update_path.directions,
            change_membership_path.directions
        );
        let witness = KagemushaOutputMembershipWitnessV4 {
            operation: KagemushaOutputMembershipOperationV4::Split,
            initial_root,
            final_root,
            recipient: Some(KagemushaOutputMembershipLeafV4 {
                commitment: recipient_commitment,
                leaf_index: 1,
                update_path: recipient_update_path,
                membership_path: recipient_membership_path,
            }),
            change: Some(KagemushaOutputMembershipLeafV4 {
                commitment: change_commitment,
                leaf_index: 3,
                update_path: change_update_path,
                membership_path: change_membership_path,
            }),
            dummy_leaf_index: 4,
            dummy_path,
        };
        assert_output_membership_rejected(witness);
    }

    #[test]
    fn output_membership_poseidon_update_rejects_profile_presence_smuggling() {
        let mut init_with_change =
            output_membership_fixture(KagemushaOutputMembershipOperationV4::Split, true);
        init_with_change.operation = KagemushaOutputMembershipOperationV4::Init;
        assert_output_membership_rejected(init_with_change);

        let mut redemption_without_change =
            output_membership_fixture(KagemushaOutputMembershipOperationV4::RedemptionChange, true);
        redemption_without_change.change = None;
        assert_output_membership_rejected(redemption_without_change);

        let mut redemption_with_recipient =
            output_membership_fixture(KagemushaOutputMembershipOperationV4::Split, true);
        redemption_with_recipient.operation =
            KagemushaOutputMembershipOperationV4::RedemptionChange;
        assert_output_membership_rejected(redemption_with_recipient);

        let mut split_without_recipient =
            output_membership_fixture(KagemushaOutputMembershipOperationV4::Split, false);
        split_without_recipient.recipient = None;
        assert_output_membership_rejected(split_without_recipient);
    }
}
