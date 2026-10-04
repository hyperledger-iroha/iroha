//! Public key-session lifecycle and release verification regressions.
use super::test_fixtures::*;
use super::*;
use iroha_crypto::threshold_bls::AdaptiveThresholdBlsSecretShare;
use norito::codec::{DecodeAll as _, Encode as _};
use rand::{SeedableRng as _, rngs::StdRng};
#[test]
fn key_session_lifecycle_roundtrips_and_enforces_inclusive_bounds() {
    let key_session_id = TleKeySessionId::new(binding(0xA7));
    let mut lifecycle = TleKeySessionLifecycleV1::new(key_session_id, 10, 3, 2)
        .expect("construct bounded lifecycle");
    assert!(!lifecycle.permits_fresh_ballot_at(9));
    assert!(lifecycle.permits_fresh_ballot_at(10));
    assert!(lifecycle.permits_fresh_ballot_at(12));
    assert!(!lifecycle.permits_fresh_ballot_at(13));

    lifecycle
        .consume_fresh_ballot(12)
        .expect("inclusive expiry height remains selectable");
    lifecycle
        .consume_fresh_ballot(10)
        .expect("second configured use remains selectable");
    assert!(!lifecycle.permits_fresh_ballot_at(10));
    assert_eq!(
        lifecycle.consume_fresh_ballot(10),
        Err(TleKeySessionLifecycleValidationErrorV1::FreshBallotBudgetExceeded)
    );

    crate::frame_test_support::assert_owner_frame_v1(
        &lifecycle,
        "iroha_core::tle_release::TleKeySessionLifecycleV1",
    );
    let encoded = norito::encode_canonical(&lifecycle).expect("encode lifecycle");
    let decoded =
        norito::decode_canonical::<TleKeySessionLifecycleV1>(&encoded).expect("decode lifecycle");
    assert_eq!(decoded, lifecycle);
}

#[test]
fn key_session_lifecycle_cutover_is_inclusive_and_cannot_precede_activation() {
    let key_session_id = TleKeySessionId::new(binding(0xA8));
    let mut lifecycle = TleKeySessionLifecycleV1::new(key_session_id, 20, 10, 1)
        .expect("construct bounded lifecycle");
    assert_eq!(
        lifecycle.cut_over_after(19),
        Err(TleKeySessionLifecycleValidationErrorV1::InvalidHeightBounds)
    );
    lifecycle
        .cut_over_after(24)
        .expect("cut over after an active height");
    assert_eq!(lifecycle.selection_closed_at_height, Some(24));
    assert!(lifecycle.selection_is_closed());
    assert!(lifecycle.permits_fresh_ballot_at(24));
    assert!(!lifecycle.permits_fresh_ballot_at(25));
    assert_eq!(
        lifecycle.cut_over_after(25),
        Err(TleKeySessionLifecycleValidationErrorV1::SelectionAlreadyClosed)
    );

    let mut missing_marker = lifecycle;
    missing_marker.selection_closed_at_height = None;
    assert_eq!(
        missing_marker.validate(),
        Err(TleKeySessionLifecycleValidationErrorV1::InvalidHeightBounds)
    );

    let mut closed_after_natural_expiry = TleKeySessionLifecycleV1::new(key_session_id, 20, 10, 1)
        .expect("construct second bounded lifecycle");
    closed_after_natural_expiry
        .cut_over_after(40)
        .expect("closure after natural expiry is still explicit");
    assert_eq!(closed_after_natural_expiry.selectable_through_height, 29);
    assert_eq!(
        closed_after_natural_expiry.selection_closed_at_height,
        Some(40)
    );
    closed_after_natural_expiry
        .validate()
        .expect("explicit post-expiry closure is reconstructible");
}

#[test]
fn public_state_roundtrips_and_revalidates_every_proof() {
    let fixture = fixture();
    crate::frame_test_support::assert_owner_frame_v1(
        fixture.validated.public_state(),
        "iroha_core::tle_release::TleKeySessionPublicStateV1",
    );
    let framed = norito::encode_canonical(fixture.validated.public_state())
        .expect("public-state owner frame");
    let restored_frame: TleKeySessionPublicStateV1 =
        norito::decode_canonical(&framed).expect("public-state frame roundtrip");
    assert_eq!(
        restored_frame
            .validate()
            .expect("revalidate all decoded proofs")
            .public_state(),
        fixture.validated.public_state()
    );
    let encoded = fixture.validated.public_state().encode();
    let decoded = TleKeySessionPublicStateV1::decode_all(&mut encoded.as_slice())
        .expect("decode public state");
    let restored = decoded.validate().expect("revalidate public state");
    assert_eq!(restored.public_state(), fixture.validated.public_state());

    let mut tampered = fixture.validated.public_state().clone();
    tampered.qualified_dealer_commitments[0].constant_pok_response[31] ^= 1;
    assert!(tampered.validate().is_err());
}

#[test]
fn inline_dealer_validation_rejects_excess_without_omitting_proof_checks() {
    let fixture = fixture();
    let mut state = fixture.validated.public_state().clone();
    let dealer = state.qualified_dealer_commitments.last().unwrap().clone();
    while state.qualified_dealers.len() <= usize::from(THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1) {
        state.qualified_dealers.push(dealer.dealer_index);
        state.qualified_dealer_commitments.push(dealer.clone());
    }
    assert_eq!(
        state.clone().validate().unwrap_err(),
        TleReleaseAdapterError::Threshold(ThresholdBlsError::NonCanonicalQualifiedSet)
    );
    state
        .qualified_dealer_commitments
        .last_mut()
        .unwrap()
        .constant_pok_response = [0xFF; 32];
    assert_eq!(
        state.validate().unwrap_err(),
        TleReleaseAdapterError::Threshold(ThresholdBlsError::InvalidScalar)
    );
}

#[test]
fn reconstructed_public_share_iterator_checks_every_value_and_exact_length() {
    let fixture = fixture();
    for mutation in 0..4 {
        let mut state = fixture.validated.public_state().clone();
        match mutation {
            0 => {
                state.public_shares.pop();
            }
            1 => state.public_shares.push(state.public_shares[0]),
            2 => state.public_shares.reverse(),
            3 => state.public_shares[0].participant_hash[0] ^= 1,
            _ => unreachable!(),
        }
        assert_eq!(
            state.validate().unwrap_err(),
            TleReleaseAdapterError::TranscriptMismatch
        );
    }
}

#[test]
fn exact_identity_partials_combine_without_a_subset_bitmap() {
    let fixture = fixture();
    let identity = identity(fixture.session);
    let parameters = *fixture.validated.transcript().parameters();
    let mut rng = StdRng::from_seed([8; 32]);
    let mut records = Vec::new();
    for recipient in 1_u16..=2 {
        let private_shares = fixture
            .dealer_secrets
            .iter()
            .zip(&fixture.dealers)
            .map(|(secret, dealer)| {
                secret
                    .private_share(&parameters, dealer, recipient)
                    .expect("private contribution")
            })
            .collect::<Vec<_>>();
        let signing_share = AdaptiveThresholdBlsSecretShare::from_dealer_shares(
            fixture.validated.transcript(),
            &private_shares,
        )
        .expect("signing share");
        let partial = signing_share
            .sign_payload_with_rng(
                fixture.validated.transcript(),
                &identity.payload_bytes(),
                &mut rng,
            )
            .expect("partial");
        records.push(
            fixture
                .validated
                .encode_partial_release(&identity, 100, &partial)
                .expect("public partial"),
        );
    }

    let final_release = fixture
        .validated
        .combine_partial_releases(&identity, 100, &records)
        .expect("final release");
    assert_eq!(
        fixture
            .validated
            .verify_final_release(&identity, 100, &final_release),
        Ok(())
    );
    let _release_key = fixture
        .validated
        .release_key_for_opening(&identity, 100, &final_release)
        .expect("zeroizing release key");
    assert_eq!(
        fixture
            .validated
            .verify_final_release(&identity, 99, &final_release),
        Err(TleReleaseAdapterError::ReleaseHeightNotReached)
    );

    let wrong_identity = TleReleaseIdentityV1::new(
        fixture.session,
        binding(10),
        binding(11),
        binding(99),
        binding(13),
        binding(14),
        100,
        binding(15),
    )
    .expect("wrong identity");
    assert_eq!(
        fixture
            .validated
            .verify_final_release(&wrong_identity, 100, &final_release),
        Err(TleReleaseAdapterError::ReleaseBindingMismatch)
    );
}
