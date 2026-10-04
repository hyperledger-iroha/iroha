//! Real phase-frame decoding, retained refusal, exact graph moves and physical failure.

use super::*;
use crate::{
    beacon::{
        GlobalThresholdBeaconDkgSnapshotV1, PreparedLocalGlobalThresholdBeaconDkgSeatV1,
        fixtures::*, global_threshold_beacon_roster_hash_v1,
    },
    test_allocations::{allocations_during, refuse_one_layout_during},
};
use norito::core::{DecodeAttemptErrorKind, PreparedDecodeError};

fn fixture(
    seats: u16,
) -> (
    AdaptiveBeaconFixture,
    Vec<iroha_crypto::KeyPair>,
    Vec<PeerId>,
) {
    let keys = adaptive_fixture_signing_keys(seats);
    let roster = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let mut session = adaptive_dkg_session_fixture();
    session.committee_size = seats;
    session.threshold = (seats - 1) / 3 + 1;
    session.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
    let fixture = adaptive_beacon_fixture_for_session_and_keys(session, &keys, &fixture_budget());
    (fixture, keys, roster)
}
fn phases(fixture: &AdaptiveBeaconFixture) -> [Vec<u8>; 3] {
    let transcript = &fixture.session.record().adaptive_dkg;
    let mut snapshot = GlobalThresholdBeaconDkgSnapshotV1 {
        session: transcript.session,
        generator_h: transcript.generator_h,
        generator_v: transcript.generator_v,
        recipient_keys: transcript.recipient_keys.clone(),
        dealer_commitments: transcript.dealer_commitments.clone(),
        encrypted_shares: Vec::new(),
        share_acceptances: Vec::new(),
        last_updated_height: transcript.session.start_height,
    };
    let commitments = norito::encode_canonical(&snapshot).unwrap();
    snapshot.encrypted_shares = transcript.encrypted_shares.clone();
    snapshot.last_updated_height = transcript.session.commitments_end_height;
    let deliveries = norito::encode_canonical(&snapshot).unwrap();
    [
        commitments,
        deliveries,
        norito::encode_canonical(fixture.session.record()).unwrap(),
    ]
}

#[test]
fn prepared_four_and_thirty_one_phase_banks_decode_and_seal_without_late_allocation() {
    for seats in [4, 31] {
        let (fixture, keys, roster) = fixture(seats);
        let frames = phases(&fixture);
        let session = fixture.session.adaptive_dkg.session;
        let pool = fixture_budget();
        let local =
            PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(session, &roster, 1, &keys[0], &pool)
                .unwrap();
        assert_eq!(
            local.input_frame_bounds(),
            frames.each_ref().map(|frame| frame.len())
        );
        let verifier = local.prepare_final_session_verifier().unwrap();
        drop(local);
        let mut inputs =
            PreparedGlobalThresholdBeaconDkgInputsV1::new(session, &roster, &pool).unwrap();
        assert!(inputs.belongs_to(&pool));
        assert!(!inputs.belongs_to(&fixture_budget()));
        let blocker = pool
            .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
            .unwrap();
        let mut result = None;
        assert_eq!(
            allocations_during(|| {
                result = Some((|| {
                    inputs.decode_commitments(
                        &frames[0],
                        norito::canonical_decode_limits(frames[0].len()),
                    )?;
                    inputs.decode_deliveries(
                        &frames[1],
                        norito::canonical_decode_limits(frames[1].len()),
                    )?;
                    inputs.decode_final_session(
                        &frames[2],
                        norito::canonical_decode_limits(frames[2].len()),
                    )?;
                    inputs.take_final_session()
                })());
            }),
            0,
            "every actual phase field uses its original prepared allocation"
        );
        let graph = result.unwrap().unwrap();
        assert_eq!(
            inputs.commitments().unwrap().recipient_keys,
            fixture.session.adaptive_dkg.recipient_keys
        );
        assert_eq!(
            inputs.deliveries().unwrap().encrypted_shares,
            fixture.session.adaptive_dkg.encrypted_shares
        );
        assert_eq!(graph.get(), fixture.session.record());
        let original_shares = graph.get().public_shares.as_ptr();
        let original_ciphertext = graph.get().adaptive_dkg.encrypted_shares[0]
            .encrypted_share
            .as_ptr();
        let mut result = None;
        assert_eq!(
            allocations_during(|| result = Some(verifier.seal(graph, &fixture.binding))),
            0
        );
        let sealed = result
            .unwrap()
            .unwrap_or_else(|(_, _, error)| panic!("original final graph: {error}"));
        assert_eq!(sealed.record().public_shares.as_ptr(), original_shares);
        assert_eq!(
            sealed.record().adaptive_dkg.encrypted_shares[0]
                .encrypted_share
                .as_ptr(),
            original_ciphertext
        );
        assert!(matches!(
            inputs.take_final_session(),
            Err(GlobalThresholdBeaconInputErrorV1::Phase)
        ));
        drop(inputs);
        drop(blocker);
        drop(sealed);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn prepared_input_keeps_original_complete_source_and_scope_on_real_enclosing_refusal() {
    let (fixture, _, roster) = fixture(4);
    let frames = phases(&fixture);
    let pool = fixture_budget();
    let mut inputs = PreparedGlobalThresholdBeaconDkgInputsV1::new(
        fixture.session.adaptive_dkg.session,
        &roster,
        &pool,
    )
    .unwrap();
    let before = pool.reserved_bytes();
    let limit = norito::DecodeLimits::new(0, 0, 0, 0, 0);
    let error = norito::core::with_decode_limits_scope(limit, || {
        inputs.decode_commitments(&frames[0], norito::canonical_decode_limits(frames[0].len()))
    })
    .unwrap_err();
    let GlobalThresholdBeaconInputErrorV1::Decode(PreparedDecodeError::Codec(original)) = error
    else {
        panic!("original canonical scope cause");
    };
    assert_eq!(original.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    assert_eq!(pool.reserved_bytes(), before);
    assert!(inputs.commitments().is_none());
    let foreign = frames[0].clone();
    assert!(matches!(
        inputs.decode_commitments(&foreign, norito::canonical_decode_limits(foreign.len())),
        Err(GlobalThresholdBeaconInputErrorV1::SourceChanged)
    ));
    let occupied = pool
        .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
        .unwrap();
    let mut result = None;
    assert_eq!(
        allocations_during(|| result = Some(
            inputs.decode_commitments(&frames[0], norito::canonical_decode_limits(frames[0].len()))
        )),
        0
    );
    result.unwrap().unwrap();
    let first = inputs.commitments().unwrap().recipient_keys.as_ptr();
    inputs
        .decode_commitments(&frames[0], norito::canonical_decode_limits(frames[0].len()))
        .unwrap();
    assert_eq!(inputs.commitments().unwrap().recipient_keys.as_ptr(), first);
    drop(occupied);
    drop(original);
    drop(inputs);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn prepared_input_rejects_out_of_order_and_rebound_attempt_without_consuming_destination() {
    let (fixture, _, roster) = fixture(4);
    let frames = phases(&fixture);
    let pool = fixture_budget();
    let mut inputs = PreparedGlobalThresholdBeaconDkgInputsV1::new(
        fixture.session.adaptive_dkg.session,
        &roster,
        &pool,
    )
    .unwrap();
    let before = pool.reserved_bytes();
    assert!(matches!(
        inputs.decode_deliveries(&frames[1], norito::canonical_decode_limits(frames[1].len())),
        Err(GlobalThresholdBeaconInputErrorV1::Phase)
    ));
    assert!(matches!(
        inputs.decode_final_session(&frames[2], norito::canonical_decode_limits(frames[2].len())),
        Err(GlobalThresholdBeaconInputErrorV1::Phase)
    ));
    assert_eq!(pool.reserved_bytes(), before);
    let mut changed: GlobalThresholdBeaconDkgSnapshotV1 = norito::decode_canonical_with_limits(
        &frames[0],
        norito::canonical_decode_limits(frames[0].len()),
    )
    .unwrap();
    changed.session.attempt_id[0] ^= 1;
    let changed = norito::encode_canonical(&changed).unwrap();
    assert!(matches!(
        inputs.decode_commitments(&changed, norito::canonical_decode_limits(changed.len())),
        Err(GlobalThresholdBeaconInputErrorV1::Binding)
    ));
    assert!(inputs.commitments().is_none());
    assert!(matches!(
        inputs.decode_commitments(&frames[0], norito::canonical_decode_limits(frames[0].len())),
        Err(GlobalThresholdBeaconInputErrorV1::SourceChanged)
    ));
    drop(inputs);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn prepared_bank_rejects_truncated_oversized_and_duplicate_geometry_without_late_backing() {
    let (fixture, _, roster) = fixture(4);
    let frames = phases(&fixture);
    let mut wrong: GlobalThresholdBeaconDkgSnapshotV1 = norito::decode_canonical_with_limits(
        &frames[0],
        norito::canonical_decode_limits(frames[0].len()),
    )
    .unwrap();
    wrong.recipient_keys.push(wrong.recipient_keys[0].clone());
    let oversized = norito::encode_canonical(&wrong).unwrap();
    wrong.recipient_keys.pop();
    wrong.dealer_commitments[0]
        .coefficient_commitments
        .push([0; 96]);
    let coefficients = norito::encode_canonical(&wrong).unwrap();
    for frame in [
        &frames[0][..frames[0].len() - 1],
        oversized.as_slice(),
        coefficients.as_slice(),
    ] {
        let pool = fixture_budget();
        let mut inputs = PreparedGlobalThresholdBeaconDkgInputsV1::new(
            fixture.session.adaptive_dkg.session,
            &roster,
            &pool,
        )
        .unwrap();
        let before = pool.reserved_bytes();
        let mut result = None;
        assert_eq!(
            allocations_during(|| result = Some(
                inputs.decode_commitments(frame, norito::canonical_decode_limits(frame.len()))
            )),
            0
        );
        assert!(matches!(
            result.unwrap(),
            Err(GlobalThresholdBeaconInputErrorV1::Decode(
                PreparedDecodeError::Codec(_)
            ))
        ));
        assert_eq!(pool.reserved_bytes(), before);
        assert!(inputs.commitments().is_none());
        drop(inputs);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn prepared_banks_refund_real_nested_allocator_failure_and_keep_original_capacity_refusal() {
    let (fixture, _, roster) = fixture(4);
    let session = fixture.session.adaptive_dkg.session;
    let pool = fixture_budget();
    for width in [
        soranet_pq::MlKemSuite::MlKem768.public_key_len(),
        soranet_pq::MlKemSuite::MlKem768.ciphertext_len(),
        iroha_crypto::Algorithm::BlsNormal.signature_payload_len(),
    ] {
        let (result, refused) =
            refuse_one_layout_during(Layout::array::<u8>(width).unwrap(), || {
                PreparedGlobalThresholdBeaconDkgInputsV1::new(session, &roster, &pool)
            });
        assert!(refused, "a real nested initialized backing was attempted");
        assert!(result.is_err());
        assert_eq!(pool.reserved_bytes(), 0);
    }
    let probe = PreparedGlobalThresholdBeaconDkgInputsV1::new(session, &roster, &pool).unwrap();
    let required = pool.reserved_bytes();
    drop(probe);
    assert_eq!(pool.reserved_bytes(), 0);
    let short = AllocationBudget::new(required - 1);
    assert!(matches!(
        PreparedGlobalThresholdBeaconDkgInputsV1::new(session, &roster, &short),
        Err(GlobalThresholdBeaconInputErrorV1::Preparation(
            crate::beacon::GlobalThresholdBeaconSessionError::Admission(
                AllocationRefusal::Capacity { .. }
            )
        ))
    ));
    assert_eq!(short.reserved_bytes(), 0);
    let exact = AllocationBudget::new(required);
    let prepared = PreparedGlobalThresholdBeaconDkgInputsV1::new(session, &roster, &exact).unwrap();
    assert_eq!(exact.reserved_bytes(), required);
    drop(prepared);
    assert_eq!(exact.reserved_bytes(), 0);
}
