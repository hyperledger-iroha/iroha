//! Pure State derivation controls; these fixtures construct no Native cash owner or money proof.
use super::*;
use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;
fn state(balance: u128, logical: u128, secure: u128) -> KagemushaStateV1 {
    let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let enrollment = fixture.verify(300).unwrap();
    let floor = KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(
        &enrollment,
        Arc::clone(&fixture.release),
    )
    .unwrap();
    let (_, preview) = derive_preview(
        &floor,
        [43; 32],
        KagemushaDurableCapacityV1 {
            inbox_bytes: KagemushaDurableCapacityV1::MINIMUM_INBOX_BYTES,
            outbox_bytes: KagemushaDurableCapacityV1::MINIMUM_OUTBOX_BYTES,
        },
    )
    .unwrap();
    let base = preview.state;
    KagemushaStateV1::build(
        base.context(),
        base.liability_pool_id,
        base.lane.clone(),
        balance,
        logical,
        secure,
        base.hardware_epoch,
        base.device_policy_binding,
        base.state_nonce_commitment,
        base.consumed_credit_root,
    )
    .unwrap()
}
#[test]
fn successor_uses_reserved_native_nonce_and_preserves_full_independent_indexes() {
    let before = state(100, (1_u128 << 80) + 5, (1_u128 << 90) + 7);
    let after = derive_successor(&before, [51; 32], [52; 32], 25).unwrap();
    assert_eq!(after.balance, 75);
    assert_eq!(after.logical_sequence, before.logical_sequence + 1);
    assert_eq!(after.secure_index, before.secure_index + 1);
    assert_ne!(after.logical_sequence, after.secure_index);
    assert_eq!(after.consumed_credit_root, before.consumed_credit_root);
    assert_eq!(after.device_policy_binding, before.device_policy_binding);
    assert_eq!(
        after,
        derive_successor(&before, [51; 32], [52; 32], 25).unwrap()
    );
    assert_ne!(
        after,
        derive_successor(&before, [53; 32], [52; 32], 25).unwrap()
    );
    assert_ne!(
        after,
        derive_successor(&before, [51; 32], [54; 32], 25).unwrap()
    );
}
#[test]
fn no_unproved_mint_or_zero_amount_can_enter_outgoing_preparation() {
    let empty = state(0, 0, 0);
    assert!(matches!(
        derive_successor(&empty, [51; 32], [52; 32], 1),
        Err(KagemushaStateErrorV1::InsufficientBalance)
    ));
    assert!(derive_successor(&empty, [51; 32], [52; 32], 0).is_err());
    let before = state(100, 5, 7);
    assert!(derive_successor(&before, [0; 32], [52; 32], 25).is_err());
    assert!(derive_successor(&before, [51; 32], [0; 32], 25).is_err());
    assert!(derive_successor(&before, [51; 32], [52; 32], 101).is_err());
}
#[test]
fn successor_never_truncates_exact_next_u128_or_uses_a_financial_index_as_apple_counter() {
    let before = state(100, u128::MAX - 1, u128::MAX - 1);
    let after = derive_successor(&before, [51; 32], [52; 32], 25).unwrap();
    assert_eq!(after.logical_sequence, u128::MAX);
    assert_eq!(after.secure_index, u128::MAX);
    assert!(matches!(
        derive_successor(&after, [53; 32], [54; 32], 25),
        Err(KagemushaStateErrorV1::SequenceOverflow)
    ));
}
#[test]
fn maintained_prepared_record_binds_native_operation_and_actual_reserved_commitment() {
    let value = KagemushaOrdinaryPreparedTransitionV1 {
        version: 1,
        operation: 4,
        lifecycle_digest: [1; 32],
        request_digest: [0; 32],
        predecessor_state: [2; 32],
        successor_state: [3; 32],
        amount: (1_u128 << 80) + 1,
        reservation_digest: [4; 32],
        native_preparation_operation_id: [5; 32],
    };
    let original = value.canonical_bytes().unwrap();
    assert_eq!(
        value,
        KagemushaOrdinaryPreparedTransitionV1::decode_canonical_exact(&original).unwrap()
    );
    let digest = value.binding_digest().unwrap();
    for index in 0..3 {
        let mut changed = value;
        match index {
            0 => changed.native_preparation_operation_id[0] ^= 1,
            1 => changed.reservation_digest[0] ^= 1,
            _ => changed.amount += 1,
        }
        assert_ne!(digest, changed.binding_digest().unwrap());
    }
    let mut truncated = original;
    truncated.pop();
    assert!(KagemushaOrdinaryPreparedTransitionV1::decode_canonical_exact(&truncated).is_err());
    assert_eq!(ORDINARY_PREPARATION_LIFETIME_MS, 10_000);
}

#[test]
fn original_preparation_window_requires_outgoing_purpose_and_exact_ten_second_cap() {
    let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let c = &fixture.selection.preparation.challenge;
    let credential = &fixture.selection.issuance.credential;
    let subject = KagemushaHardwareTransitionSelectionV1 {
        version: 1,
        release_id: c.release_id,
        provider_policy_root: [21; 32],
        app_policy_digest: [22; 32],
        credential_id: credential.canonical_digest().unwrap(),
        network_id: fixture.release.network_id(),
        lane_commitment: c.lane_id,
        hardware_profile_id: c.hardware_profile_id,
        policy_epoch: c.policy_epoch,
        hardware_epoch_id: [24; 32],
        hardware_epoch_generation: c.hardware_epoch,
        operation_kind: KagemushaOperationKindV1::SendSplit,
        transition_statement_digest: [25; 32],
        candidate_envelope_digest: [0; 32],
        terminal_body_commitment: [0; 32],
        secure_index_before: (1_u128 << 80) + 2,
        secure_index_after: (1_u128 << 80) + 3,
    };
    assert_eq!(subject.network_id.as_bytes(), &c.network_id);
    let mut challenge = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::PrepareTransition,
        operation_id: [26; 32],
        nonce: [27; 32],
        account_binding: c.account_binding,
        authority_policy_digest: c.app_authority_policy_digest,
        attested_key_id: credential.subject.attested_key_id,
        enrollment_digest: credential.canonical_digest().unwrap(),
        subject_signing_digest: Sha256::digest(subject.canonical_prepare_signing_bytes().unwrap())
            .into(),
        normalized_guard_digest: [28; 32],
        issued_at_ms: 300,
        expires_at_ms: 10_300,
        subject,
    };
    require_preparation_challenge_window(&challenge).unwrap();
    let original_w = challenge.canonical_signing_bytes().unwrap();
    let expected_w_digest: DigestV1 = Sha256::digest(&original_w).into();
    assert_eq!(
        preparation_message_digest(&challenge).unwrap(),
        expected_w_digest
    );
    assert_ne!(
        preparation_message_digest(&challenge).unwrap(),
        challenge.enrollment_digest
    );
    let mut other_nonce = challenge;
    other_nonce.nonce[0] ^= 1;
    assert_ne!(
        preparation_message_digest(&other_nonce).unwrap(),
        expected_w_digest
    );
    challenge.expires_at_ms = 10_301;
    assert!(require_preparation_challenge_window(&challenge).is_err());
    challenge.expires_at_ms = 300;
    assert!(require_preparation_challenge_window(&challenge).is_err());
    challenge.expires_at_ms = 10_300;
    challenge.purpose = KagemushaAppOperationApprovalPurposeV1::MonetaryTransition;
    assert!(require_preparation_challenge_window(&challenge).is_err());
    challenge.purpose = KagemushaAppOperationApprovalPurposeV1::PrepareTransition;
    challenge.subject.operation_kind = KagemushaOperationKindV1::RedeemSplit;
    challenge.subject_signing_digest =
        Sha256::digest(challenge.subject.canonical_prepare_signing_bytes().unwrap()).into();
    require_preparation_challenge_window(&challenge).unwrap();
    challenge.subject_signing_digest[0] ^= 1;
    assert!(require_preparation_challenge_window(&challenge).is_err());
}
