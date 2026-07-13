//! Exact-value and branch-safety tests for the first-release Kagemusha lifecycle.

use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    asset::AssetDefinitionId,
    domain::DomainId,
    offline::{
        KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2, KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
        KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3, KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V3,
        KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V3, KagemushaConfidentialMerklePathV2,
        KagemushaNoteMembershipWitnessV2, KagemushaPastaCycleParityV3,
        KagemushaRecursiveSpendArtifactBindingV3, KagemushaRecursiveSpendBranchClaimV2,
        KagemushaRecursiveSpendBranchV2, KagemushaRecursiveSpendBundleV2,
        KagemushaRecursiveSpendInputBranchV2, KagemushaRecursiveSpendPeerSplitTransitionV2,
        KagemushaRecursiveSpendProofV2, KagemushaRecursiveSpendPublicStatementV2,
        KagemushaRecursiveSpendRedemptionIntentV2, KagemushaRecursiveSpendSplitIntentV2,
        KagemushaRecursiveSpendSplitResultV2, KagemushaRecursiveSpendTopUpAnchorRefV2,
        KagemushaRecursiveSpendTransitionV2, KagemushaScaledAmountV2,
        KagemushaSpendableNoteDescriptorV2, KagemushaUnshieldPublicInputsBindingV2,
        KagemushaValidationError, kagemusha_confidential_amount_encoding_v2,
        kagemusha_recursive_spend_step_circuit_id_v3, kagemusha_recursive_spend_step_parity_v3,
        kagemusha_recursive_spend_step_verifier_role_v3,
    },
    prelude::Numeric,
    proof::{ProofBox, VerifyingKeyId},
};

const SCALE: u32 = 9;
const TOTAL: u128 = 10_750_000_000;
const TRANSFER: u128 = 6_250_000_000;
const CHANGE: u128 = 4_500_000_000;

fn asset() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("sbp", "universal").expect("fixture domain"),
        "pkr".parse().expect("fixture asset name"),
    )
}

fn recipient() -> AccountId {
    let keypair = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
        .expect("deterministic fixture keypair");
    AccountId::new(keypair.public_key().clone())
}

fn note(
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    atomic_units: u128,
    commitment_byte: u8,
    nullifier_byte: u8,
) -> KagemushaSpendableNoteDescriptorV2 {
    KagemushaSpendableNoteDescriptorV2 {
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        note_commitment: [commitment_byte; 32],
        spend_nullifier: [nullifier_byte; 32],
        amount: KagemushaScaledAmountV2::new(atomic_units, SCALE).expect("fixture amount"),
    }
}

fn anchor_ref() -> KagemushaRecursiveSpendTopUpAnchorRefV2 {
    KagemushaRecursiveSpendTopUpAnchorRefV2 {
        topup_operation_id: [0x11; 32],
        anchor_digest: [0x12; 32],
    }
}

fn artifact_binding() -> KagemushaRecursiveSpendArtifactBindingV3 {
    KagemushaRecursiveSpendArtifactBindingV3 {
        generation: "kagemusha-release-1".to_owned(),
        manifest_sha256: [0x13; 32],
    }
}

fn split_intent() -> KagemushaRecursiveSpendSplitIntentV2 {
    let chain_id = ChainId::from("kagemusha-value-contract");
    let asset = asset();
    let anchor = anchor_ref();
    KagemushaRecursiveSpendSplitIntentV2::new(
        chain_id.clone(),
        asset.clone(),
        vec![KagemushaRecursiveSpendInputBranchV2 {
            bundle_digest: [0x21; 32],
            input_note: note(&chain_id, &asset, TOTAL, 0x22, 0x23),
            branch_claims: vec![
                KagemushaRecursiveSpendBranchClaimV2::root(anchor.anchor_digest)
                    .expect("root claim"),
            ],
            input_root: [0x24; 32],
            proof_step_count: 1,
            peer_hop_count: 0,
        }],
        vec![anchor],
        SCALE,
        artifact_binding(),
        KagemushaScaledAmountV2::new(TRANSFER, SCALE).expect("transfer amount"),
        note(&chain_id, &asset, TRANSFER, 0x31, 0x32),
        Some(note(&chain_id, &asset, CHANGE, 0x33, 0x34)),
        [0x35; 32],
        [0x36; 32],
    )
    .expect("valid exact split")
}

fn redemption_intent(
    public_atomic_units: u128,
    change_atomic_units: Option<u128>,
) -> KagemushaRecursiveSpendRedemptionIntentV2 {
    let chain_id = ChainId::from("kagemusha-value-contract");
    let asset = asset();
    let input_note = note(&chain_id, &asset, TOTAL, 0x41, 0x42);
    let input_root = [0x43; 32];
    let public_amount =
        KagemushaScaledAmountV2::new(public_atomic_units, SCALE).expect("public amount");
    let change_output =
        change_atomic_units.map(|amount| note(&chain_id, &asset, amount, 0x44, 0x45));
    let unshield_public_inputs = KagemushaUnshieldPublicInputsBindingV2 {
        input_commitment_0: input_note.note_commitment,
        input_commitment_1: [0; 32],
        nullifier_0: input_note.spend_nullifier,
        nullifier_1: [0; 32],
        change_output_commitment: change_output
            .as_ref()
            .map_or([0; 32], |change| change.note_commitment),
        root: input_root,
        public_amount: kagemusha_confidential_amount_encoding_v2(public_atomic_units),
        asset_tag: [0x46; 32],
        chain_tag: [0x47; 32],
    };
    let unshield_public_inputs_digest = unshield_public_inputs
        .digest()
        .expect("unshield public-input digest");
    KagemushaRecursiveSpendRedemptionIntentV2 {
        chain_id,
        asset,
        input_note,
        parent_branch_claims: vec![
            KagemushaRecursiveSpendBranchClaimV2::root(anchor_ref().anchor_digest)
                .expect("root claim"),
        ],
        parent_topup_anchor_refs: vec![anchor_ref()],
        parent_proof_step_count: 1,
        parent_peer_hop_count: 0,
        parent_bundle_digest: [0x48; 32],
        input_root,
        recipient: recipient(),
        public_amount,
        change_artifact_binding: change_output.as_ref().map(|_| artifact_binding()),
        change_output,
        unshield_public_inputs,
        unshield_public_inputs_digest,
        operation_id: [0x49; 32],
    }
}

#[test]
fn scaled_amount_converts_fractional_values_exactly_at_asset_scale() {
    let decimal: Numeric = "10.75".parse().expect("valid decimal");
    let amount = KagemushaScaledAmountV2::from_public_numeric(&decimal, SCALE)
        .expect("exact scale conversion");
    assert_eq!(amount.atomic_units, TOTAL);
    assert_eq!(amount.scale, SCALE);
    assert_eq!(amount.public_numeric(), decimal);

    let minimum: Numeric = "0.000000001".parse().expect("minimum atomic decimal");
    assert_eq!(
        KagemushaScaledAmountV2::from_public_numeric(&minimum, SCALE)
            .expect("minimum atomic amount"),
        KagemushaScaledAmountV2::new(1, SCALE).expect("minimum atomic amount")
    );

    let maximum = Numeric::new(u128::MAX, SCALE);
    assert_eq!(
        KagemushaScaledAmountV2::from_public_numeric(&maximum, SCALE)
            .expect("maximum u128 amount")
            .atomic_units,
        u128::MAX
    );
}

#[test]
fn scaled_amount_rejects_rounding_nonpositive_values_and_overflow() {
    let excess_precision: Numeric = "0.0000000001".parse().expect("valid decimal");
    assert!(matches!(
        KagemushaScaledAmountV2::from_public_numeric(&excess_precision, SCALE),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "amount.scale"
        })
    ));
    for invalid in [Numeric::new(0, 0), Numeric::new(-1_i128, 0)] {
        assert!(matches!(
            KagemushaScaledAmountV2::from_public_numeric(&invalid, SCALE),
            Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "amount.atomic_units"
            })
        ));
    }
    assert!(matches!(
        KagemushaScaledAmountV2::from_public_numeric(&Numeric::new(u128::MAX, 0), 1),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "amount.atomic_units"
        })
    ));
    assert!(KagemushaScaledAmountV2::new(0, SCALE).is_err());
    assert!(KagemushaScaledAmountV2::new(1, 29).is_err());
}

#[test]
fn step_parity_is_derived_exactly_from_the_proof_step_count() {
    assert_eq!(
        kagemusha_recursive_spend_step_parity_v3(1).expect("init parity"),
        KagemushaPastaCycleParityV3::StepEq
    );
    assert_eq!(
        kagemusha_recursive_spend_step_parity_v3(2).expect("first spend parity"),
        KagemushaPastaCycleParityV3::StepEp
    );
    assert_eq!(
        kagemusha_recursive_spend_step_circuit_id_v3(63).expect("odd parity circuit"),
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3
    );
    assert_eq!(
        kagemusha_recursive_spend_step_circuit_id_v3(64).expect("even parity circuit"),
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3
    );
    assert_eq!(
        kagemusha_recursive_spend_step_verifier_role_v3(1).expect("init verifier role"),
        KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V3
    );
    assert_eq!(
        kagemusha_recursive_spend_step_verifier_role_v3(2).expect("spend verifier role"),
        KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V3
    );
    assert!(kagemusha_recursive_spend_step_parity_v3(0).is_err());
    assert!(
        kagemusha_recursive_spend_step_parity_v3(KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2 + 1)
            .is_err()
    );
}

#[test]
fn membership_witness_rejects_root_direction_and_dummy_alias_tampering() {
    fn path(leaf_index: u32, root: [u8; 32]) -> KagemushaConfidentialMerklePathV2 {
        KagemushaConfidentialMerklePathV2 {
            siblings: vec![[0; 32]; 16],
            directions: (0..16)
                .map(|level| ((leaf_index >> level) & 1) as u8)
                .collect(),
            root,
        }
    }

    let witness = KagemushaNoteMembershipWitnessV2 {
        leaf_index: 7,
        input_path: path(7, [0xA5; 32]),
        dummy_input_path: path(8, [0xA5; 32]),
    };
    witness.validate_structure().expect("valid witness shape");

    let mut wrong_direction = witness.clone();
    wrong_direction.input_path.directions[0] ^= 1;
    assert!(wrong_direction.validate_structure().is_err());

    let mut truncated = witness.clone();
    truncated.dummy_input_path.siblings.pop();
    assert!(truncated.validate_structure().is_err());

    let mut different_root = witness.clone();
    different_root.dummy_input_path.root = [0xB6; 32];
    assert!(different_root.validate_structure().is_err());

    let aliased_dummy = KagemushaNoteMembershipWitnessV2 {
        dummy_input_path: path(7, [0xA5; 32]),
        ..witness
    };
    assert!(aliased_dummy.validate_structure().is_err());
}

#[test]
fn split_conserves_fractional_value_and_produces_disjoint_siblings() {
    let split = split_intent();
    assert_eq!(
        split.input_amount().expect("validated input total"),
        KagemushaScaledAmountV2::new(TOTAL, SCALE).expect("total amount")
    );

    let recipient_claims = split
        .output_branch_claims(KagemushaRecursiveSpendBranchV2::Recipient)
        .expect("recipient claims");
    let change_claims = split
        .output_branch_claims(KagemushaRecursiveSpendBranchV2::Change)
        .expect("change claims");
    assert_eq!(recipient_claims.len(), 1);
    assert_eq!(change_claims.len(), 1);
    assert!(
        !recipient_claims[0]
            .path
            .conflicts_with(change_claims[0].path)
    );
    assert!(
        !recipient_claims[0]
            .conflicts_with(&change_claims[0])
            .expect("consistent siblings")
    );
    assert!(
        split.inputs[0].branch_claims[0]
            .conflicts_with(&recipient_claims[0])
            .expect("ancestor conflict")
    );

    let other_root =
        KagemushaRecursiveSpendBranchClaimV2::root([0xF1; 32]).expect("other lineage root");
    assert!(
        !recipient_claims[0]
            .conflicts_with(&other_root)
            .expect("different lineage")
    );

    let root = &split.inputs[0].branch_claims[0];
    let left = root
        .child(KagemushaRecursiveSpendBranchV2::Recipient, [0xA1; 32])
        .expect("left first edge")
        .child(KagemushaRecursiveSpendBranchV2::Recipient, [0xA2; 32])
        .expect("left second edge");
    let right = root
        .child(KagemushaRecursiveSpendBranchV2::Recipient, [0xB1; 32])
        .expect("alternative first edge")
        .child(KagemushaRecursiveSpendBranchV2::Change, [0xB2; 32])
        .expect("right second edge");
    assert!(
        left.conflicts_with(&right)
            .expect("shared-prefix transition mismatch")
    );
}

#[test]
fn split_projects_one_shared_proof_into_independently_spendable_siblings() {
    fn path(leaf_index: u32, root: [u8; 32]) -> KagemushaConfidentialMerklePathV2 {
        KagemushaConfidentialMerklePathV2 {
            siblings: vec![[0x71; 32]; 16],
            directions: (0..16)
                .map(|level| ((leaf_index >> level) & 1) as u8)
                .collect(),
            root,
        }
    }

    let split = split_intent();
    let transition = KagemushaRecursiveSpendPeerSplitTransitionV2::from_intent(&split)
        .expect("shared split transition");
    let verifier_key_id = VerifyingKeyId::new(
        "halo2/ipa",
        kagemusha_recursive_spend_step_verifier_role_v3(2).expect("step role"),
    );
    let statement = KagemushaRecursiveSpendPublicStatementV2 {
        chain_id: split.chain_id.clone(),
        asset: split.asset.clone(),
        asset_scale: split.asset_scale,
        input_root: split.inputs[0].input_root,
        final_root: [0x72; 32],
        topup_anchor_refs: split.topup_anchor_refs.clone(),
        proof_step_count: 2,
        peer_hop_count: 1,
        transition: Some(KagemushaRecursiveSpendTransitionV2::PeerSplit(transition)),
        artifact_binding: split.output_artifact_binding.clone(),
        verifier_key_id: verifier_key_id.clone(),
    };
    let statement_digest = statement.digest().expect("shared statement digest");
    let recursive_proof = KagemushaRecursiveSpendProofV2 {
        verifier_key_id,
        public_statement_digest: statement_digest,
        proof: ProofBox::new(
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3
                .parse()
                .expect("proof backend"),
            vec![0x73; 64],
        ),
    };
    let recipient_bundle = KagemushaRecursiveSpendBundleV2 {
        statement: statement.clone(),
        recursive_proof: recursive_proof.clone(),
        branch: KagemushaRecursiveSpendBranchV2::Recipient,
        current_note: split.recipient_output.clone(),
        branch_claims: split
            .output_branch_claims(KagemushaRecursiveSpendBranchV2::Recipient)
            .expect("recipient claims"),
    };
    let change_bundle = KagemushaRecursiveSpendBundleV2 {
        statement,
        recursive_proof,
        branch: KagemushaRecursiveSpendBranchV2::Change,
        current_note: split.change_output.clone().expect("change output"),
        branch_claims: split
            .output_branch_claims(KagemushaRecursiveSpendBranchV2::Change)
            .expect("change claims"),
    };
    let dummy = path(2, [0x72; 32]);
    let result = KagemushaRecursiveSpendSplitResultV2::new(
        split.clone(),
        split.binding_digest().expect("split digest"),
        recipient_bundle,
        KagemushaNoteMembershipWitnessV2 {
            leaf_index: 0,
            input_path: path(0, [0x72; 32]),
            dummy_input_path: dummy.clone(),
        },
        Some(change_bundle),
        Some(KagemushaNoteMembershipWitnessV2 {
            leaf_index: 1,
            input_path: path(1, [0x72; 32]),
            dummy_input_path: dummy,
        }),
    )
    .expect("one shared proof with two output projections");
    let change = result.change_bundle.as_ref().expect("change bundle");
    assert_eq!(result.recipient_bundle.statement, change.statement);
    assert_eq!(
        result.recipient_bundle.recursive_proof,
        change.recursive_proof
    );

    let mut wrong_projection = result.clone();
    wrong_projection
        .change_bundle
        .as_mut()
        .expect("change bundle")
        .branch = KagemushaRecursiveSpendBranchV2::Recipient;
    assert!(wrong_projection.validate_public_binding().is_err());

    let mut substituted = result;
    substituted
        .change_bundle
        .as_mut()
        .expect("change bundle")
        .recursive_proof
        .proof
        .bytes[0] ^= 1;
    assert!(substituted.validate_public_binding().is_err());
}

#[test]
fn split_rejects_nonconservation_duplicate_material_and_overlapping_claims() {
    let mut wrong_change = split_intent();
    wrong_change.change_output.as_mut().expect("change").amount =
        KagemushaScaledAmountV2::new(CHANGE + 1, SCALE).expect("wrong change amount");
    assert!(matches!(
        wrong_change.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "split.conservation"
        })
    ));

    let mut duplicate_output = split_intent();
    duplicate_output
        .change_output
        .as_mut()
        .expect("change")
        .note_commitment = duplicate_output.recipient_output.spend_nullifier;
    assert!(matches!(
        duplicate_output.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "split.output_material"
        })
    ));

    let mut overlaps_consumed_material = split_intent();
    overlaps_consumed_material.recipient_output.note_commitment = overlaps_consumed_material.inputs
        [0]
    .input_note
    .note_commitment;
    assert!(matches!(
        overlaps_consumed_material.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "split.output_material"
        })
    ));

    let mut overlapping_claims = split_intent();
    let root = overlapping_claims.inputs[0].branch_claims[0].clone();
    let descendant = root
        .child(KagemushaRecursiveSpendBranchV2::Recipient, [0x55; 32])
        .expect("descendant claim");
    overlapping_claims.inputs[0].branch_claims = vec![root, descendant];
    assert!(matches!(
        overlapping_claims.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "branch_claims.conflict"
        })
    ));
}

#[test]
fn first_release_split_rejects_more_than_one_parent() {
    let mut split = split_intent();
    let mut second = split.inputs[0].clone();
    second.bundle_digest = [0x81; 32];
    second.input_note.note_commitment = [0x82; 32];
    second.input_note.spend_nullifier = [0x83; 32];
    split.inputs.push(second);

    assert!(matches!(
        split.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendProof { field: "split" })
    ));
}

#[test]
fn split_and_redemption_reject_lineage_counter_drift() {
    let mut split = split_intent();
    split.inputs[0].peer_hop_count = 1;
    assert!(matches!(
        split.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "split.inputs"
        })
    ));

    let mut redemption = redemption_intent(TOTAL, None);
    redemption.parent_proof_step_count = 2;
    assert!(matches!(
        redemption.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "redemption"
        })
    ));
}

#[test]
fn peer_hop_limit_is_sixty_four_and_redemption_does_not_add_a_peer_hop() {
    assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2, 64);
    assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2, 64);
    assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2, 65);

    fn claim_at_depth(depth: u8) -> KagemushaRecursiveSpendBranchClaimV2 {
        let mut claim = KagemushaRecursiveSpendBranchClaimV2::root(anchor_ref().anchor_digest)
            .expect("root claim");
        for step in 0..depth {
            let mut digest = [0x55; 32];
            digest[0] = step.saturating_add(1);
            claim = claim
                .child(KagemushaRecursiveSpendBranchV2::Recipient, digest)
                .expect("extend canonical branch claim");
        }
        claim
    }

    let mut last_permitted_parent = split_intent();
    let maximum_hops = KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2;
    last_permitted_parent.inputs[0].branch_claims = vec![claim_at_depth(
        KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2 - 1,
    )];
    last_permitted_parent.inputs[0].proof_step_count =
        u32::from(KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2);
    last_permitted_parent.inputs[0].peer_hop_count = maximum_hops - 1;
    last_permitted_parent
        .validate_public_binding()
        .expect("a 63-hop parent may produce the 64th peer hop at branch depth 64");

    let mut exhausted_parent = split_intent();
    exhausted_parent.inputs[0].branch_claims = vec![claim_at_depth(
        KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2,
    )];
    exhausted_parent.inputs[0].proof_step_count =
        u32::from(KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2) + 1;
    exhausted_parent.inputs[0].peer_hop_count = maximum_hops;
    assert!(matches!(
        exhausted_parent.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "split.inputs"
        })
    ));

    let mut terminal_redemption = redemption_intent(TOTAL, None);
    terminal_redemption.parent_branch_claims = vec![claim_at_depth(
        KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2,
    )];
    terminal_redemption.parent_proof_step_count =
        u32::from(KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2) + 1;
    terminal_redemption.parent_peer_hop_count = maximum_hops;
    terminal_redemption
        .validate_public_binding()
        .expect("redemption does not add a peer hop at the 64-hop boundary");

    let mut terminal_partial = redemption_intent(TRANSFER, Some(CHANGE));
    terminal_partial.parent_branch_claims = vec![claim_at_depth(
        KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2,
    )];
    terminal_partial.parent_proof_step_count =
        u32::from(KAGEMUSHA_RECURSIVE_SPEND_MAX_BRANCH_DEPTH_V2) + 1;
    terminal_partial.parent_peer_hop_count = maximum_hops;
    assert!(matches!(
        terminal_partial.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "redemption.change_output"
        })
    ));

    terminal_redemption.parent_peer_hop_count = maximum_hops + 1;
    assert!(matches!(
        terminal_redemption.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "redemption"
        })
    ));
}

#[test]
fn redemption_supports_exact_full_and_partial_value_conservation() {
    let full = redemption_intent(TOTAL, None);
    full.validate_public_binding().expect("full redemption");
    assert_eq!(
        full.unshield_public_inputs.public_amount,
        kagemusha_confidential_amount_encoding_v2(TOTAL)
    );

    let partial = redemption_intent(TRANSFER, Some(CHANGE));
    partial
        .validate_public_binding()
        .expect("partial redemption with change");
    assert_eq!(
        partial.public_amount.atomic_units.checked_add(
            partial
                .change_output
                .as_ref()
                .expect("partial redemption change")
                .amount
                .atomic_units
        ),
        Some(TOTAL)
    );
}

#[test]
fn redemption_rejects_nonconservation_and_reused_input_material() {
    let mut wrong_change = redemption_intent(TRANSFER, Some(CHANGE + 1));
    assert!(matches!(
        wrong_change.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "redemption.change_output"
        })
    ));

    let mut duplicate_material = redemption_intent(TRANSFER, Some(CHANGE));
    duplicate_material
        .change_output
        .as_mut()
        .expect("change")
        .note_commitment = duplicate_material.input_note.note_commitment;
    duplicate_material
        .unshield_public_inputs
        .change_output_commitment = duplicate_material.input_note.note_commitment;
    duplicate_material.unshield_public_inputs_digest = duplicate_material
        .unshield_public_inputs
        .digest()
        .expect("updated unshield digest");
    assert!(matches!(
        duplicate_material.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "redemption.change_output"
        })
    ));

    wrong_change.change_output = None;
    wrong_change.change_artifact_binding = None;
    wrong_change.unshield_public_inputs.change_output_commitment = [0; 32];
    wrong_change.unshield_public_inputs_digest = wrong_change
        .unshield_public_inputs
        .digest()
        .expect("updated unshield digest");
    assert!(wrong_change.validate_public_binding().is_err());
}
