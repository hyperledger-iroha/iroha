//! Pure mathematical preview tests. No source/proof/Native receipt or effect capability is forged.
use super::*;
use iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1;

fn data() -> (
    KagemushaStateV1,
    KagemushaOrdinaryIncomingReservationV1,
    SourceFacts,
    super::super::sparse_merkle::ExactConsumedCreditIndex,
    KagemushaOrdinaryCashClockContextV1,
) {
    let fixture = kagemusha_ordinary_mint_codec_fixture_v1();
    let context = &fixture.request.authorization.statement.context;
    let enrolled = fixture.enrollment_fixture.verify(1000).unwrap();
    let c = enrolled.app_credential().subject();
    let tree = super::super::sparse_merkle::ExactConsumedCreditIndex::empty();
    let owner = &context.lineage.owner;
    let lane = KagemushaLaneIdV1 {
        network_id: owner.runtime.network_id,
        device_lane_id: owner.lane_id,
        asset: owner.runtime.asset.clone(),
        scale: owner.runtime.scale,
    };
    let state = KagemushaStateV1::build(
        KagemushaStateContextV1 {
            protocol_version: 1,
            suite_id: c.suite_id,
            vk_digest: context.vk_digest,
            release_id: c.release_id,
            asset_incarnation: owner.runtime.asset_incarnation,
            hardware_profile_id: c.hardware_profile_id,
            policy_epoch: c.policy_epoch,
        },
        derive_liability_pool_id(&lane, owner.runtime.asset_incarnation).unwrap(),
        lane,
        9,
        (1_u128 << 100) + 7,
        (1_u128 << 100) + 11,
        HardwareEpochV1 {
            generation: u128::from(c.hardware_epoch),
            epoch_id: context.lineage.financial_epoch_id,
        },
        DevicePolicyBindingV1 {
            device_key_reference: c.app_key_reference,
            hardware_policy_id: [42; 32],
        },
        [90; 32],
        tree.root(),
    )
    .unwrap();
    state.validate().unwrap();
    let public_data = [27; 80];
    let selection = iroha_data_model::kagemusha::KagemushaOrdinaryIncomingSelectionV1 {
        version: 1,
        lineage: context.lineage.clone(),
        operation_id: context.operation_id,
        predecessor: iroha_data_model::kagemusha::KagemushaOrdinaryFinancialHeadV1 {
            state_commitment: state.state_commitment,
            logical_sequence: state.logical_sequence,
            state_original_sha256: Sha256::digest(public_data).into(),
        },
        source: KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
            topup_request_original_sha256: Sha256::digest(
                fixture.request.canonical_bytes().unwrap(),
            )
            .into(),
        },
        credit_id: fixture.request.authorization.statement.credit_id,
        amount: context.amount,
        scale: owner.runtime.scale,
        recipient_app_credential_digest: enrolled.app_credential().digest(),
        financial_control_original_sha256: context.financial_control_original_sha256,
        clock_context_digest: context.clock_context.binding_digest().unwrap(),
    };
    let reservation = KagemushaOrdinaryIncomingReservationV1 {
        selection,
        source_proof_original_sha256: [62; 32],
        finalized_source_original_sha256: [25; 32],
        source_semantic_digest: [26; 32],
    };
    reservation.validate_shape().unwrap();
    // Plain semantic fixture data is confined to this mathematical test; production derives
    // these private facts only by real MintAuthority/closed received-output admission.
    let facts = SourceFacts {
        kind: KagemushaTransitionKindV1::MintFold,
        credit_id: reservation.selection.credit_id,
        amount: reservation.selection.amount,
        semantic_digest: reservation.source_semantic_digest,
        mint_proof_binding: [28; 32],
        lifecycle_binding: [29; 32],
    };
    let clock = KagemushaOrdinaryCashClockContextV1 {
        version: 1,
        request_nonce: [30; 32],
        signed_observations_original_digest: [31; 32],
        lower_at_ms: 1500,
        upper_at_ms: 1501,
    };
    (state, reservation, facts, tree, clock)
}
fn artifacts(state: &KagemushaStateV1) -> KagemushaRecursionArtifactsV1 {
    let mut a = crate::kagemusha_v1_recursion::tests::artifacts();
    a.release_id = state.release_id;
    a
}
#[test]
fn ordinary_incoming_mathematical_preview_conserves_balance_full_indices_and_replay_roots() {
    let (before, reservation, facts, mut tree, clock) = data();
    let envelope = reservation.digest().unwrap();
    let key = CreditIdV1(facts.credit_id);
    let replay = tree.preview_insert_witness(key, envelope).unwrap();
    let a = artifacts(&before);
    let preview = derive_math_preview(
        &before,
        &reservation,
        &facts,
        &replay,
        [91; 32],
        17,
        [92; 32],
        &clock,
        [93; 32],
        a,
    )
    .unwrap();
    preview.successor.validate().unwrap();
    assert_eq!(preview.successor.balance, before.balance + facts.amount);
    assert_eq!(
        preview.successor.logical_sequence,
        before.logical_sequence + 1
    );
    assert_eq!(preview.successor.secure_index, before.secure_index + 1);
    assert_eq!(
        preview.successor.consumed_credit_root,
        replay.successor_root
    );
    assert_eq!(preview.statement.effect_digest, envelope);
    assert_eq!(
        preview.preparation.financial_index_before,
        before.secure_index
    );
    assert_eq!(preview.preparation.logical_journal_sequence_before, 17);
    assert_eq!(
        preview.normalized.transition_intent_digest,
        preview.preparation.binding_digest().unwrap()
    );
    assert_eq!(
        preview.normalized.recovery_record_digest,
        preview.preparation.recovery_binding_digest().unwrap()
    );
    assert_eq!(preview.normalized.terminal_commit_binding_digest, [0; 32]);
    assert_eq!(
        preview.normalized.sender_one_time_authorization_digest,
        [0; 32]
    );
    assert_eq!(
        preview.normalized.durable_outbox_effect_digest,
        a.canonical_empty_effect_digest
    );
    let fresh = derive_math_preview(
        &before,
        &reservation,
        &facts,
        &replay,
        [91; 32],
        17,
        [94; 32],
        &clock,
        [93; 32],
        a,
    )
    .unwrap();
    assert_eq!(fresh.successor, preview.successor);
    assert_ne!(
        fresh.normalized.transition_intent_digest,
        preview.normalized.transition_intent_digest
    );
    tree.insert_with_witness(&replay).unwrap();
    let mut retry = reservation.clone();
    retry.selection.operation_id[0] ^= 1;
    retry.selection.financial_control_original_sha256[0] ^= 1;
    retry.selection.clock_context_digest[0] ^= 1;
    assert_ne!(retry.digest().unwrap(), envelope);
    assert!(
        tree.preview_insert_witness(key, retry.digest().unwrap())
            .is_err()
    );
}
#[test]
fn ordinary_incoming_mathematical_preview_rejects_wrong_leaf_scope_nonce_and_overflow() {
    let (before, reservation, facts, tree, clock) = data();
    let replay = tree
        .preview_insert_witness(CreditIdV1(facts.credit_id), reservation.digest().unwrap())
        .unwrap();
    let a = artifacts(&before);
    for i in 0..3 {
        let mut wrong = replay.clone();
        match i {
            0 => wrong.envelope_digest[0] ^= 1,
            1 => wrong.credit_id.0[0] ^= 1,
            _ => wrong.siblings_root_to_leaf[0].eq[0] ^= 1,
        }
        assert!(
            derive_math_preview(
                &before,
                &reservation,
                &facts,
                &wrong,
                [91; 32],
                17,
                [92; 32],
                &clock,
                [93; 32],
                a
            )
            .is_err()
        );
    }
    assert!(
        derive_math_preview(
            &before,
            &reservation,
            &facts,
            &replay,
            before.state_nonce_commitment,
            17,
            [92; 32],
            &clock,
            [93; 32],
            a
        )
        .is_err()
    );
    assert!(
        derive_math_preview(
            &before,
            &reservation,
            &facts,
            &replay,
            [91; 32],
            u64::MAX,
            [92; 32],
            &clock,
            [93; 32],
            a
        )
        .is_err()
    );
    let mut max = before.clone();
    max.balance = u128::MAX;
    assert!(matches!(
        derive_math_preview(
            &max,
            &reservation,
            &facts,
            &replay,
            [91; 32],
            17,
            [92; 32],
            &clock,
            [93; 32],
            a
        ),
        Err(KagemushaStateErrorV1::ArithmeticOverflow)
    ));
    let fixture = kagemusha_ordinary_mint_codec_fixture_v1();
    let enrollment = fixture.enrollment_fixture.verify(1000).unwrap();
    assert!(
        require_scope(
            &before,
            &[27; 80],
            enrollment.app_credential(),
            &reservation,
            before.release_id,
            [42; 32]
        )
        .is_ok()
    );
    let mut wrong = reservation.clone();
    wrong.selection.lineage.financial_authority_commitment[0] ^= 1;
    assert!(
        require_scope(
            &before,
            &[27; 80],
            enrollment.app_credential(),
            &wrong,
            before.release_id,
            [42; 32]
        )
        .is_err()
    );
    let mut wrong = reservation.clone();
    wrong.selection.predecessor.state_original_sha256[0] ^= 1;
    assert!(
        require_scope(
            &before,
            &[27; 80],
            enrollment.app_credential(),
            &wrong,
            before.release_id,
            [42; 32]
        )
        .is_err()
    );
    let mut wrong = reservation.clone();
    wrong.selection.predecessor.logical_sequence = (before.logical_sequence as u64).into();
    assert!(
        require_scope(
            &before,
            &[27; 80],
            enrollment.app_credential(),
            &wrong,
            before.release_id,
            [42; 32]
        )
        .is_err()
    );
}
