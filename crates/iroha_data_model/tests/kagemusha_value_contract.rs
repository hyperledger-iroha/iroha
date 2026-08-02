//! Exact-value and branch-safety tests for the first-release Kagemusha lifecycle.

use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    asset::AssetDefinitionId,
    domain::DomainId,
    offline::{
        KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2, KagemushaRecursiveSpendArtifactBindingV4,
        KagemushaRecursiveSpendBranchClaimV2, KagemushaRecursiveSpendBranchV2,
        KagemushaRecursiveSpendInputBranchV2, KagemushaRecursiveSpendRedemptionIntentV4,
        KagemushaRecursiveSpendSplitIntentV4, KagemushaRecursiveSpendStateBoundaryV5,
        KagemushaRecursiveSpendTopUpAnchorRefV2, KagemushaScaledAmountV2,
        KagemushaSpendableNoteDescriptorV2, KagemushaUnshieldPublicInputsBindingV2,
        KagemushaValidationError, kagemusha_confidential_amount_encoding_v2,
    },
    prelude::{Numeric, Quantity},
};

#[test]
fn pasta_state_boundary_roundtrips_every_limb_without_field_reduction() {
    let mut limbs = (0..iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5)
        .map(|index| u32::try_from(index).expect("bounded state limb"))
        .collect::<Vec<_>>();
    limbs[0] = iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5;
    let boundary =
        KagemushaRecursiveSpendStateBoundaryV5::new(limbs.clone()).expect("valid exact boundary");
    assert_eq!(boundary.exact_state().expect("recover exact state"), limbs);

    for malformed_len in [0, limbs.len() - 1, limbs.len() + 1] {
        assert!(KagemushaRecursiveSpendStateBoundaryV5::new(vec![1; malformed_len]).is_err());
    }
    let mut wrong_layout_limb = limbs.clone();
    wrong_layout_limb[0] =
        iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5
            .saturating_add(1);
    assert!(KagemushaRecursiveSpendStateBoundaryV5::new(wrong_layout_limb).is_err());

    let mut wrong_version = boundary;
    wrong_version.layout_version = wrong_version.layout_version.saturating_add(1);
    assert!(wrong_version.exact_state().is_err());
}

const SCALE: u32 = 9;
const TOTAL: u128 = 10_750_000_000;
const TRANSFER: u128 = 6_250_000_000;
const CHANGE: u128 = 4_500_000_000;

fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
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

fn artifact_binding_v4() -> KagemushaRecursiveSpendArtifactBindingV4 {
    KagemushaRecursiveSpendArtifactBindingV4 {
        version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: "kagemusha-release-v4-1".to_owned(),
        manifest_sha256: [0x14; 32],
    }
}

fn split_intent_v4() -> KagemushaRecursiveSpendSplitIntentV4 {
    let chain_id = ChainId::from("kagemusha-value-contract");
    let asset = asset();
    let anchor = anchor_ref();
    KagemushaRecursiveSpendSplitIntentV4 {
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        inputs: vec![KagemushaRecursiveSpendInputBranchV2 {
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
        topup_anchor_refs: vec![anchor],
        asset_scale: SCALE,
        output_artifact_binding: artifact_binding_v4(),
        transfer_amount: KagemushaScaledAmountV2::new(TRANSFER, SCALE).expect("transfer amount"),
        recipient_output: note(&chain_id, &asset, TRANSFER, 0x31, 0x32),
        change_output: Some(note(&chain_id, &asset, CHANGE, 0x33, 0x34)),
        recipient_request_digest: [0x35; 32],
        operation_id: [0x36; 32],
    }
}

fn redemption_intent_v4(
    public_atomic_units: u128,
    change_atomic_units: Option<u128>,
) -> KagemushaRecursiveSpendRedemptionIntentV4 {
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
    KagemushaRecursiveSpendRedemptionIntentV4 {
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
        change_artifact_binding: change_output.as_ref().map(|_| artifact_binding_v4()),
        change_output,
        unshield_public_inputs,
        unshield_public_inputs_digest,
        operation_id: [0x49; 32],
    }
}

#[test]
fn scaled_amount_converts_fractional_values_exactly_at_asset_scale() {
    let decimal: Quantity = "10.75".parse().expect("valid quantity");
    let amount = KagemushaScaledAmountV2::from_public_quantity(&decimal, SCALE)
        .expect("exact scale conversion");
    assert_eq!(amount.atomic_units, TOTAL);
    assert_eq!(amount.scale, SCALE);
    assert_eq!(amount.public_quantity(), decimal);

    let minimum: Quantity = "0.000000001".parse().expect("minimum atomic quantity");
    assert_eq!(
        KagemushaScaledAmountV2::from_public_quantity(&minimum, SCALE)
            .expect("minimum atomic amount"),
        KagemushaScaledAmountV2::new(1, SCALE).expect("minimum atomic amount")
    );

    let maximum = Quantity::try_from_numeric(Numeric::new(u128::MAX, SCALE))
        .expect("maximum non-negative quantity");
    assert_eq!(
        KagemushaScaledAmountV2::from_public_quantity(&maximum, SCALE)
            .expect("maximum u128 amount")
            .atomic_units,
        u128::MAX
    );
}

#[test]
fn scaled_amount_rejects_rounding_nonpositive_values_and_overflow() {
    let excess_precision: Quantity = "0.0000000001".parse().expect("valid quantity");
    assert!(matches!(
        KagemushaScaledAmountV2::from_public_quantity(&excess_precision, SCALE),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "amount.scale"
        })
    ));
    assert!(matches!(
        KagemushaScaledAmountV2::from_public_quantity(&Quantity::zero(), SCALE),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "amount.atomic_units"
        })
    ));
    assert!("-1".parse::<Quantity>().is_err());
    assert!(matches!(
        KagemushaScaledAmountV2::from_public_quantity(&Quantity::from(u128::MAX), 1),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "amount.atomic_units"
        })
    ));
    assert!(KagemushaScaledAmountV2::new(0, SCALE).is_err());
    assert!(KagemushaScaledAmountV2::new(1, 29).is_err());
}

#[test]
fn split_conserves_fractional_value_and_produces_disjoint_siblings() {
    let split = split_intent_v4();
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
}

#[test]
fn abi21_split_uses_v4_digest_and_rejects_nonconservation() {
    let split = split_intent_v4();
    split.validate_public_binding().expect("valid ABI-21 split");
    assert_eq!(
        split.input_amount().expect("validated V4 input total"),
        KagemushaScaledAmountV2::new(TOTAL, SCALE).expect("total amount")
    );
    assert_ne!(split.binding_digest().expect("V4 split digest"), [0; 32]);

    let recipient_claims = split
        .output_branch_claims(KagemushaRecursiveSpendBranchV2::Recipient)
        .expect("V4 recipient claims");
    let change_claims = split
        .output_branch_claims(KagemushaRecursiveSpendBranchV2::Change)
        .expect("V4 change claims");
    assert!(
        !recipient_claims[0]
            .path
            .conflicts_with(change_claims[0].path)
    );

    let mut nonconserving = split;
    nonconserving.change_output.as_mut().expect("change").amount =
        KagemushaScaledAmountV2::new(CHANGE + 1, SCALE).expect("wrong change amount");
    assert!(matches!(
        nonconserving.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "split.v4.conservation"
        })
    ));
}

#[test]
fn abi21_redemption_binds_change_claims_and_rejects_artifact_omission() {
    let intent = redemption_intent_v4(TRANSFER, Some(CHANGE));
    intent
        .validate_public_binding()
        .expect("valid ABI-21 partial redemption");
    let claims = intent
        .change_branch_claims()
        .expect("proof-bound V4 change claims");
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].path.depth, 1);

    let mut missing_binding = intent;
    missing_binding.change_artifact_binding = None;
    assert!(matches!(
        missing_binding.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "redemption.v4.change_output"
        })
    ));
}

#[test]
fn split_rejects_nonconservation_duplicate_material_and_overlapping_claims() {
    let mut wrong_change = split_intent_v4();
    wrong_change.change_output.as_mut().expect("change").amount =
        KagemushaScaledAmountV2::new(CHANGE + 1, SCALE).expect("wrong change amount");
    assert!(matches!(
        wrong_change.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "split.v4.conservation"
        })
    ));

    let mut duplicate_output = split_intent_v4();
    duplicate_output
        .change_output
        .as_mut()
        .expect("change")
        .note_commitment = duplicate_output.recipient_output.spend_nullifier;
    assert!(matches!(
        duplicate_output.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "split.v4.output_material"
        })
    ));

    let mut overlaps_consumed_material = split_intent_v4();
    overlaps_consumed_material.recipient_output.note_commitment = overlaps_consumed_material.inputs
        [0]
    .input_note
    .note_commitment;
    assert!(matches!(
        overlaps_consumed_material.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "split.v4.output_material"
        })
    ));

    let mut overlapping_claims = split_intent_v4();
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
fn branch_claim_conflicts_bind_paths_and_exact_transition_history() {
    let root =
        KagemushaRecursiveSpendBranchClaimV2::root(anchor_ref().anchor_digest).expect("root claim");
    let recipient = root
        .child(KagemushaRecursiveSpendBranchV2::Recipient, [0x41; 32])
        .expect("recipient claim");
    let same_transition_change = root
        .child(KagemushaRecursiveSpendBranchV2::Change, [0x41; 32])
        .expect("same-transition change claim");
    let alternative_transition_change = root
        .child(KagemushaRecursiveSpendBranchV2::Change, [0x42; 32])
        .expect("alternative-transition change claim");

    assert!(root.conflicts_with(&recipient).expect("root conflict"));
    assert!(
        recipient
            .conflicts_with(&root)
            .expect("root conflict symmetry")
    );
    assert!(
        !recipient
            .conflicts_with(&same_transition_change)
            .expect("siblings from one transition")
    );
    assert!(
        recipient
            .conflicts_with(&alternative_transition_change)
            .expect("alternative transition conflict")
    );
    assert!(
        alternative_transition_change
            .conflicts_with(&recipient)
            .expect("alternative transition conflict symmetry")
    );

    let other_root =
        KagemushaRecursiveSpendBranchClaimV2::root([0x77; 32]).expect("independent root");
    assert!(
        !recipient
            .conflicts_with(&other_root)
            .expect("independent lineage")
    );
}

#[test]
fn peer_hop_limit_is_eight_and_independent_of_branch_depth() {
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

    let mut last_permitted_parent = split_intent_v4();
    let maximum_hops = KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2;
    last_permitted_parent.inputs[0].branch_claims = vec![claim_at_depth(
        u8::try_from(maximum_hops - 1).expect("hop bound fits u8"),
    )];
    last_permitted_parent.inputs[0].peer_hop_count = maximum_hops - 1;
    last_permitted_parent
        .validate_public_binding()
        .expect("a seventh-hop parent may produce the eighth peer hop");

    let mut exhausted_parent = split_intent_v4();
    exhausted_parent.inputs[0].branch_claims = vec![claim_at_depth(
        u8::try_from(maximum_hops).expect("hop bound fits u8"),
    )];
    exhausted_parent.inputs[0].peer_hop_count = maximum_hops;
    assert!(matches!(
        exhausted_parent.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "split.v4.inputs"
        })
    ));

    let mut terminal_redemption = redemption_intent_v4(TOTAL, None);
    terminal_redemption.parent_branch_claims = vec![claim_at_depth(
        u8::try_from(maximum_hops).expect("hop bound fits u8"),
    )];
    terminal_redemption.parent_peer_hop_count = maximum_hops;
    terminal_redemption
        .validate_public_binding()
        .expect("redemption does not add a peer hop at the eight-hop boundary");

    terminal_redemption.parent_peer_hop_count = maximum_hops + 1;
    assert!(matches!(
        terminal_redemption.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "redemption.v4"
        })
    ));
}

#[test]
fn redemption_supports_exact_full_and_partial_value_conservation() {
    let full = redemption_intent_v4(TOTAL, None);
    full.validate_public_binding().expect("full redemption");
    assert_eq!(
        full.unshield_public_inputs.public_amount,
        kagemusha_confidential_amount_encoding_v2(TOTAL)
    );

    let partial = redemption_intent_v4(TRANSFER, Some(CHANGE));
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
    let mut wrong_change = redemption_intent_v4(TRANSFER, Some(CHANGE + 1));
    assert!(matches!(
        wrong_change.validate_public_binding(),
        Err(KagemushaValidationError::InvalidRecursiveSpendNote {
            field: "redemption.v4.change_output"
        })
    ));

    let mut duplicate_material = redemption_intent_v4(TRANSFER, Some(CHANGE));
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
            field: "redemption.v4.change_output"
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
