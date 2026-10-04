//! Actual pre-RNG output allocation, original-pool refusal and canonical borrowed-frame controls.

use super::*;
use crate::{
    beacon::{
        AdaptiveGlobalThresholdBeaconDkgCryptoV1, GlobalThresholdBeaconDkgStateV1,
        GlobalThresholdBeaconSessionBindingV1, validate_global_threshold_beacon_session_v1,
    },
    test_allocations::{allocations_during, refuse_one_layout_during},
};
use iroha_allocation::ChargedBufferError;
use std::task::{Context, Waker};

fn assert_canonical_bytes(actual: &[u8], expected: &[u8]) {
    assert!(
        actual == expected,
        "canonical frame mismatch: actual length {}, expected length {}, first differing offset {:?}",
        actual.len(),
        expected.len(),
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right)
            .or_else(
                || (actual.len() != expected.len()).then_some(actual.len().min(expected.len()))
            )
    );
}

#[test]
fn prepared_local_outputs_are_complete_before_randomness_at_four_and_thirty_one() {
    for n in [4, 31] {
        let (session, keys, roster) = super::tests::signed_session(n);
        let budget = crate::beacon::fixtures::fixture_budget();
        let mut prepared = None;
        let allocations = allocations_during(|| {
            prepared = Some(
                PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(
                    session, &roster, 1, &keys[0], &budget,
                )
                .unwrap(),
            )
        });
        assert_eq!(
            allocations,
            14 + 6 * usize::from(n),
            "only the exact nested ledgers/backing, five outer buffers and two reusable workspaces"
        );
        let prepared = prepared.unwrap();
        let original_frame = prepared.public_frame.backing();
        let retained = budget.reserved_bytes();
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - retained)
            .unwrap();
        let mut local = None;
        assert_eq!(
            allocations_during(|| local = Some(prepared.generate(&keys[0]).unwrap())),
            0,
            "a claimed attempt cannot require another allocation after RNG begins"
        );
        let mut local = local.unwrap();
        assert_eq!(local.public_frame.backing(), original_frame);
        assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
        let mut frame_len = 0;
        assert_eq!(
            allocations_during(|| frame_len = local.publication_frame().unwrap().len()),
            0
        );
        assert!(frame_len < original_frame.1);
        let raw: GlobalThresholdBeaconDkgSnapshotV1 =
            norito::decode_canonical(local.publication_frame().unwrap()).unwrap();
        let (key, commitment) = local.publication();
        let parameters = adaptive_beacon_parameters(&session).unwrap();
        let expected = GlobalThresholdBeaconDkgSnapshotV1 {
            session,
            generator_h: *parameters.h_bytes(),
            generator_v: *parameters.v_bytes(),
            recipient_keys: vec![key.clone()],
            dealer_commitments: vec![commitment.clone()],
            encrypted_shares: vec![],
            share_acceptances: vec![],
            last_updated_height: session.start_height,
        };
        assert_eq!(raw, expected);
        assert_canonical_bytes(
            local.publication_frame().unwrap(),
            &norito::encode_canonical(&expected).unwrap(),
        );
        drop(local);
        assert_eq!(budget.reserved_bytes(), budget.limit_bytes() - retained);
        drop(blocker);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn prepared_local_capacity_and_physical_refusals_preserve_original_attempt_and_refund() {
    let (session, keys, roster) = super::tests::signed_session(4);
    let budget = crate::beacon::fixtures::fixture_budget();
    let mut slot = crate::unit_test_support::release_registration(&budget);
    let floor = budget.reserved_bytes();
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - floor)
        .unwrap();
    let error =
        PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(session, &roster, 1, &keys[0], &budget)
            .err()
            .unwrap();
    let LocalGlobalThresholdBeaconDkgErrorV1::Session(
        GlobalThresholdBeaconSessionError::Admission(AllocationRefusal::Capacity {
            release, ..
        }),
    ) = error
    else {
        panic!("retain original admission source")
    };
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    let mut cx = Context::from_waker(Waker::noop());
    assert!(slot.poll_wait(&release, &mut cx).is_pending());
    let foreign = AllocationBudget::new(128);
    drop(foreign.try_reserve_bytes(128).unwrap());
    assert!(slot.poll_wait(&release, &mut cx).is_pending());
    drop(blocker);
    assert!(slot.poll_wait(&release, &mut cx).is_ready());
    slot.cancel();
    let ready =
        PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(session, &roster, 1, &keys[0], &budget)
            .unwrap();
    drop(ready);
    assert_eq!(budget.reserved_bytes(), floor);
    for layout in [
        Layout::array::<u8>(1184).unwrap(),
        Layout::array::<u8>(1088).unwrap(),
        Layout::array::<u8>(124).unwrap(),
        Layout::array::<u8>(96).unwrap(),
        Layout::array::<[u8; 96]>(2).unwrap(),
        Layout::array::<DasRenPrivateShare<BeaconPurpose>>(4).unwrap(),
    ] {
        let (result, refused) = refuse_one_layout_during(layout, || {
            PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(session, &roster, 1, &keys[0], &budget)
        });
        assert!(refused, "actual selected layout {layout:?}");
        assert!(matches!(
            result,
            Err(LocalGlobalThresholdBeaconDkgErrorV1::Session(
                GlobalThresholdBeaconSessionError::Buffer(
                    iroha_allocation::PrepaidBufferError::Allocation(
                        ChargedBufferError::Allocator { .. }
                    )
                )
            ))
        ));
        assert_eq!(
            budget.reserved_bytes(),
            floor,
            "every initialized field is dropped before its original refund"
        );
        drop(
            PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(
                session, &roster, 1, &keys[0], &budget,
            )
            .unwrap(),
        );
        assert_eq!(budget.reserved_bytes(), floor);
    }
    drop(slot);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn prepared_public_frame_maximum_matches_canonical_full_transcripts_at_four_and_thirty_one() {
    for n in [4, 31] {
        let (session, keys, roster) = super::tests::signed_session(n);
        let budget = crate::beacon::fixtures::fixture_budget();
        let fixture = crate::beacon::fixtures::adaptive_beacon_fixture_for_session_and_keys(
            session, &keys, &budget,
        );
        let prepared = PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(
            session, &roster, 1, &keys[0], &budget,
        )
        .unwrap();
        let dkg = &fixture.session.record().adaptive_dkg;
        let complete = GlobalThresholdBeaconDkgSnapshotV1 {
            session,
            generator_h: dkg.generator_h,
            generator_v: dkg.generator_v,
            recipient_keys: dkg.recipient_keys.clone(),
            dealer_commitments: dkg.dealer_commitments.clone(),
            encrypted_shares: dkg.encrypted_shares.clone(),
            share_acceptances: dkg.share_acceptances.clone(),
            last_updated_height: session.acceptances_end_height,
        };
        assert_eq!(
            prepared.public_frame.backing().1,
            norito::encode_canonical(&complete).unwrap().len(),
            "canonical max{n} geometry, including every actual signature"
        );
    }
}

#[test]
fn local_phase_frames_preserve_exact_input_and_prepaid_pointer_without_late_growth() {
    let (session, keys, roster) = super::tests::signed_session(4);
    let budget = crate::beacon::fixtures::fixture_budget();
    let mut local = keys
        .iter()
        .enumerate()
        .map(|(index, key)| {
            PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(
                session,
                &roster,
                u16::try_from(index + 1).unwrap(),
                key,
                &budget,
            )
            .unwrap()
            .generate(key)
            .unwrap()
        })
        .collect::<Vec<_>>();
    let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
    let mut public = GlobalThresholdBeaconDkgStateV1::new(session, &crypto, &budget).unwrap();
    for seat in &local {
        let (key, dealer) = seat.publication();
        public.record_recipient_key(1, key).unwrap();
        public.record_dealer_commitment(1, dealer, &crypto).unwrap();
    }
    let all_public = public.public_snapshot().unwrap();
    for (seat, key) in local.iter_mut().zip(&keys) {
        let backing = seat.public_frame.backing();
        let original_bytes = budget.reserved_bytes();
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - original_bytes)
            .unwrap();
        assert_eq!(
            allocations_during(|| {
                let _ = seat
                    .deliver(
                        &all_public.recipient_keys,
                        &all_public.dealer_commitments,
                        2,
                        key,
                    )
                    .unwrap();
            }),
            0
        );
        assert_eq!(
            allocations_during(|| {
                let _ = seat.delivery_frame(&all_public).unwrap();
            }),
            0
        );
        assert_eq!(seat.public_frame.backing(), backing);
        assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
        drop(blocker);
        let mut expected = all_public.record().clone();
        expected.encrypted_shares = seat
            .outputs
            .outgoing
            .as_slice()
            .iter()
            .map(|row| row.get().clone())
            .collect();
        expected.last_updated_height = 2;
        assert_canonical_bytes(
            seat.delivery_frame(&all_public).unwrap(),
            &norito::encode_canonical(&expected).unwrap(),
        );
        for edge in &expected.encrypted_shares {
            public.record_encrypted_share(2, edge).unwrap();
        }
        let mut foreign = all_public.record().clone();
        foreign.last_updated_height = 2;
        assert!(matches!(
            seat.delivery_frame(&foreign),
            Err(LocalGlobalThresholdBeaconDkgErrorV1::Invalid(
                GlobalThresholdBeaconError::InvalidDkgSession
            ))
        ));
    }
    let all_edges = public.public_snapshot().unwrap();
    for (seat, key) in local.iter_mut().zip(&keys) {
        let backing = seat.public_frame.backing();
        let before = budget.reserved_bytes();
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - before)
            .unwrap();
        assert_eq!(
            allocations_during(|| {
                let _ = seat.accept(&all_edges, 3, key).unwrap();
            }),
            0
        );
        assert_eq!(
            allocations_during(|| {
                let _ = seat.acceptance_frame(&all_edges).unwrap();
            }),
            0
        );
        assert_eq!(seat.public_frame.backing(), backing);
        assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
        drop(blocker);
        let mut expected = all_edges.record().clone();
        expected.share_acceptances = seat
            .outputs
            .acceptances
            .as_slice()
            .iter()
            .map(|row| row.get().clone())
            .collect();
        expected.last_updated_height = 3;
        assert_canonical_bytes(
            seat.acceptance_frame(&all_edges).unwrap(),
            &norito::encode_canonical(&expected).unwrap(),
        );
        for row in &expected.share_acceptances {
            public.record_share_acceptance(3, row).unwrap();
        }
        let mut changed = all_edges.record().clone();
        changed.encrypted_shares[0].encrypted_share[0] ^= 1;
        assert!(matches!(
            seat.acceptance_frame(&changed),
            Err(LocalGlobalThresholdBeaconDkgErrorV1::Invalid(
                GlobalThresholdBeaconError::InvalidDkgSession
            ))
        ));
    }
    let record = public.finalize(4, &crypto).unwrap();
    let binding = GlobalThresholdBeaconSessionBindingV1 {
        network_id: record.network_id,
        session_id: record.session_id,
        roster_hash: record.roster_hash,
        transcript_hash: record.transcript_hash,
    };
    let foreign_budget = crate::beacon::fixtures::fixture_budget();
    let foreign =
        validate_global_threshold_beacon_session_v1(record, &binding, &foreign_budget).unwrap();
    for seat in &mut local {
        assert!(matches!(
            seat.finalize_private_share(&foreign),
            Err(LocalGlobalThresholdBeaconDkgErrorV1::Session(
                GlobalThresholdBeaconSessionError::ForeignReservation
            ))
        ));
        assert!(
            !seat.extracted,
            "foreign owner cannot consume the original private shares"
        );
    }
    drop(foreign);
    assert_eq!(foreign_budget.reserved_bytes(), 0);
    let sealed = validate_global_threshold_beacon_session_v1(record, &binding, &budget).unwrap();
    for seat in &mut local {
        assert_eq!(
            allocations_during(|| {
                let _ = seat.finalize_private_share(&sealed).unwrap();
            }),
            0
        );
    }
}

#[test]
fn cancelling_or_unwinding_prepared_attempt_refunds_all_original_output_backing() {
    let (session, keys, roster) = super::tests::signed_session(4);
    let budget = crate::beacon::fixtures::fixture_budget();
    let prepared =
        PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(session, &roster, 1, &keys[0], &budget)
            .unwrap();
    assert!(budget.reserved_bytes() > 0);
    drop(prepared);
    assert_eq!(budget.reserved_bytes(), 0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _prepared = PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(
            session, &roster, 1, &keys[0], &budget,
        )
        .unwrap();
        assert!(budget.reserved_bytes() > 0);
        panic!("test-only caller unwind before its durable attempt claim");
    }));
    assert!(result.is_err());
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn original_dealer_entropy_failures_remain_local_threshold_causes() {
    struct Entropy {
        unavailable: bool,
        calls: usize,
    }
    impl rand::TryRngCore for Entropy {
        type Error = std::io::Error;
        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            let mut bytes = [0; 4];
            self.try_fill_bytes(&mut bytes)?;
            Ok(u32::from_le_bytes(bytes))
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            let mut bytes = [0; 8];
            self.try_fill_bytes(&mut bytes)?;
            Ok(u64::from_le_bytes(bytes))
        }
        fn try_fill_bytes(&mut self, bytes: &mut [u8]) -> Result<(), Self::Error> {
            self.calls += 1;
            if self.unavailable {
                return Err(std::io::ErrorKind::Other.into());
            }
            bytes.fill(0);
            Ok(())
        }
    }
    impl rand::TryCryptoRng for Entropy {}
    let (session, _, _) = super::tests::signed_session(4);
    let parameters = adaptive_beacon_parameters(&session).unwrap();
    for unavailable in [true, false] {
        let mut entropy = Entropy {
            unavailable,
            calls: 0,
        };
        let original = DasRenDealerSecret::generate_with_rng(&parameters, 1, &mut entropy)
            .err()
            .expect("real threshold producer rejects unavailable or inert entropy");
        let expected = if unavailable {
            iroha_crypto::threshold_bls::ThresholdBlsError::RandomnessUnavailable
        } else {
            iroha_crypto::threshold_bls::ThresholdBlsError::InertRandomness
        };
        assert_eq!(original, expected);
        let LocalGlobalThresholdBeaconDkgErrorV1::Threshold(retained) =
            LocalGlobalThresholdBeaconDkgErrorV1::from(original)
        else {
            panic!("a local entropy failure must never authenticate an invalid-input verdict")
        };
        assert_eq!(retained, expected);
        if unavailable {
            assert_eq!(entropy.calls, 1);
        } else {
            assert!(entropy.calls > 1);
        }
    }
}
