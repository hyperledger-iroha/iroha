//! Pure transition/subject regressions; no fixture constructs a Native cash owner.
use super::*;
use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;

fn outgoing() -> (
    KagemushaStateV1,
    KagemushaStateV1,
    TransitionProofStatementV1,
) {
    let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let enrollment = fixture.verify(300).unwrap();
    let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
        &enrollment,
        Arc::clone(&fixture.release),
    )
    .unwrap();
    let (_, bootstrap) = derive_preview(
        &floor,
        [43; 32],
        KagemushaDurableCapacityV1 {
            inbox_bytes: KagemushaDurableCapacityV1::MINIMUM_INBOX_BYTES,
            outbox_bytes: KagemushaDurableCapacityV1::MINIMUM_OUTBOX_BYTES,
        },
    )
    .unwrap();
    let base = bootstrap.state;
    let before = KagemushaStateV1::build(
        base.context(),
        base.liability_pool_id,
        base.lane.clone(),
        100,
        (1_u128 << 80) + 5,
        (1_u128 << 90) + 7,
        base.hardware_epoch,
        base.device_policy_binding,
        [44; 32],
        base.consumed_credit_root,
    )
    .unwrap();
    let after = KagemushaStateV1::build(
        before.context(),
        before.liability_pool_id,
        before.lane.clone(),
        75,
        before.logical_sequence + 1,
        before.secure_index + 1,
        before.hardware_epoch,
        before.device_policy_binding,
        [45; 32],
        before.consumed_credit_root,
    )
    .unwrap();
    let statement = TransitionProofStatementV1 {
        version: 1,
        protocol_version: 1,
        predecessor_suite_id: before.suite_id,
        predecessor_vk_digest: before.vk_digest,
        successor_suite_id: after.suite_id,
        successor_vk_digest: after.vk_digest,
        kind: KagemushaTransitionKindV1::SendSplit,
        amount: 25,
        mint_finality_semantic_digest: [0; 32],
        mint_finality_proof_binding_digest: [0; 32],
        peer_credit_id: [46; 32],
        recipient_encryption_key_binding: [47; 32],
        lifecycle_binding_digest: [48; 32],
        prepared_transition_binding_digest: [49; 32],
        receive_credit_binding_digest: [0; 32],
        predecessor_release_id: before.release_id,
        release_id: after.release_id,
        asset_incarnation: before.asset_incarnation,
        liability_pool_id: before.liability_pool_id,
        hardware_profile_id: before.hardware_profile_id,
        policy_epoch: before.policy_epoch,
        lane: before.lane.clone(),
        predecessor_commitment: before.state_commitment,
        successor_commitment: after.state_commitment,
        predecessor_sequence: before.logical_sequence,
        successor_sequence: after.logical_sequence,
        predecessor_epoch: before.hardware_epoch,
        successor_epoch: after.hardware_epoch,
        predecessor_device_policy_binding: before.device_policy_binding,
        successor_device_policy_binding: after.device_policy_binding,
        predecessor_state_nonce_commitment: before.state_nonce_commitment,
        successor_state_nonce_commitment: after.state_nonce_commitment,
        journal_revision_before: 19,
        journal_revision_after: 20,
        effect_digest: [50; 32],
    };
    (before, after, statement)
}

#[test]
fn native_cash_rederives_subtraction_and_independent_full_indexes() {
    let (before, after, statement) = outgoing();
    require_outgoing(&before, &after, &statement, 19).unwrap();
    assert!(before.secure_index > u128::from(u64::MAX));
    assert!(before.logical_sequence > u128::from(u64::MAX));
    assert_ne!(before.logical_sequence, before.secure_index);
    assert!(require_outgoing(&before, &after, &statement, 18).is_err());
    let mut changed = after.clone();
    changed.secure_index = changed.logical_sequence;
    assert!(require_outgoing(&before, &changed, &statement, 19).is_err());
    let mut changed = after.clone();
    changed.balance += 1;
    assert!(require_outgoing(&before, &changed, &statement, 19).is_err());
    let mut changed = statement.clone();
    changed.amount = before.balance + 1;
    assert!(matches!(
        require_outgoing(&before, &after, &changed, 19),
        Err(KagemushaStateErrorV1::InsufficientBalance)
    ));
    let mut changed = statement.clone();
    changed.successor_sequence = changed.journal_revision_after;
    assert!(require_outgoing(&before, &after, &changed, 19).is_err());
}

#[test]
fn native_cash_rejects_substituted_complete_financial_statement_scope() {
    let (before, after, statement) = outgoing();
    for index in 0..12 {
        let mut changed = statement.clone();
        match index {
            0 => changed.predecessor_commitment[0] ^= 1,
            1 => changed.successor_commitment[0] ^= 1,
            2 => changed.predecessor_suite_id[0] ^= 1,
            3 => changed.successor_vk_digest[0] ^= 1,
            4 => changed.release_id[0] ^= 1,
            5 => changed.liability_pool_id[0] ^= 1,
            6 => changed.hardware_profile_id[0] ^= 1,
            7 => changed.policy_epoch += 1,
            8 => changed.lane.device_lane_id[0] ^= 1,
            9 => changed.successor_epoch.epoch_id[0] ^= 1,
            10 => changed.predecessor_state_nonce_commitment[0] ^= 1,
            _ => changed.journal_revision_after += 1,
        }
        assert!(
            require_outgoing(&before, &after, &changed, 19).is_err(),
            "field {index}"
        );
    }
}

#[test]
fn native_cash_preparation_subject_keeps_real_secure_index_and_full_state_sha() {
    let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let enrollment = fixture.verify(300).unwrap();
    let (before, after, statement) = outgoing();
    let subject =
        preparation_subject(&before, &after, &statement, enrollment.app_credential()).unwrap();
    assert_eq!(subject.secure_index_before, before.secure_index);
    assert_eq!(subject.secure_index_after, after.secure_index);
    assert_eq!(
        subject.transition_statement_digest,
        statement.digest().unwrap()
    );
    assert_eq!(subject.candidate_envelope_digest, [0; 32]);
    assert_eq!(subject.terminal_body_commitment, [0; 32]);
    subject.canonical_prepare_signing_bytes().unwrap();
    assert!(subject.canonical_signing_bytes().is_err());
    let mut changed = statement;
    changed.prepared_transition_binding_digest[0] ^= 1;
    assert_ne!(
        subject.transition_statement_digest,
        changed.digest().unwrap()
    );
}
