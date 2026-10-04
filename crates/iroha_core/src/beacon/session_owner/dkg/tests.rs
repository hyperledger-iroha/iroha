//! Actual public-row backing, unchanged-height refusal, snapshot and final-owner controls.

use super::*;
use crate::beacon::{
    AdaptiveGlobalThresholdBeaconDkgCryptoV1, GlobalThresholdBeaconDkgStateV1, fixtures::*,
};
use crate::test_allocations::refuse_one_layout_during;
use std::task::{Context, Waker};

#[test]
fn public_dkg_rows_keep_original_admission_and_unchanged_height_until_retry() {
    let fixture = adaptive_beacon_fixture();
    let transcript = &fixture.session.record().adaptive_dkg;
    let budget = fixture_budget();
    let mut registration = crate::unit_test_support::release_registration(&budget);
    let mut state = GlobalThresholdBeaconDkgStateV1::new(
        transcript.session,
        &AdaptiveGlobalThresholdBeaconDkgCryptoV1,
        &budget,
    )
    .unwrap();
    let original_height = state.last_updated_height;
    let original = transcript.recipient_keys[0].encode();
    let baseline = budget.reserved_bytes();
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - baseline)
        .unwrap();
    let error = state
        .record_recipient_key(
            transcript.session.start_height,
            &transcript.recipient_keys[0],
        )
        .err()
        .unwrap();
    let GlobalThresholdBeaconSessionError::Admission(AllocationRefusal::Capacity {
        release, ..
    }) = error
    else {
        panic!("exact original source")
    };
    assert_eq!(state.last_updated_height, original_height);
    assert_eq!(state.recipient_keys.len(), 0);
    let mut cx = Context::from_waker(Waker::noop());
    assert!(registration.poll_wait(&release, &mut cx).is_pending());
    let foreign = AllocationBudget::new(128);
    drop(foreign.try_reserve_bytes(128).unwrap());
    assert!(registration.poll_wait(&release, &mut cx).is_pending());
    drop(blocker);
    assert!(registration.poll_wait(&release, &mut cx).is_ready());
    registration.cancel();
    state
        .record_recipient_key(
            transcript.session.start_height,
            &transcript.recipient_keys[0],
        )
        .unwrap();
    assert_eq!(state.last_updated_height, transcript.session.start_height);
    assert_eq!(state.recipient_keys.len(), 1);
    let retained = state.recipient_keys.get(&1).unwrap();
    assert_eq!(retained, &transcript.recipient_keys[0]);
    assert_ne!(
        retained.mlkem768_public_key.as_ptr(),
        transcript.recipient_keys[0].mlkem768_public_key.as_ptr()
    );
    assert_ne!(
        retained.signature.payload().as_ptr(),
        transcript.recipient_keys[0].signature.payload().as_ptr()
    );
    assert!(state.recipient_keys.belongs_to(&budget));
    assert_eq!(transcript.recipient_keys[0].encode(), original);
    let occupied = budget.reserved_bytes();
    state
        .record_recipient_key(
            transcript.session.start_height,
            &transcript.recipient_keys[0],
        )
        .unwrap();
    assert_eq!(budget.reserved_bytes(), occupied);
    drop(state);
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn public_dkg_row_allocator_refusal_drops_actual_partial_backing_before_retry() {
    let fixture = adaptive_beacon_fixture();
    let row = &fixture.session.record().adaptive_dkg.encrypted_shares[0];
    let budget = fixture_budget();
    let mut reservation = budget
        .try_reserve(DkgRows::<GlobalThresholdBeaconDkgEncryptedShareV1>::layout(1).unwrap())
        .unwrap();
    let mut rows = DkgRows::from_reservation(1, &mut reservation).unwrap();
    let baseline = budget.reserved_bytes();
    for size in [1088, 124, 96] {
        let (result, refused) =
            refuse_one_layout_during(Layout::array::<u8>(size).unwrap(), || {
                rows.insert(row, &budget)
            });
        assert!(refused);
        assert!(result.is_err());
        assert_eq!(rows.len(), 0);
        assert_eq!(budget.reserved_bytes(), baseline);
    }
    rows.insert(row, &budget).unwrap();
    assert_eq!(
        rows.get(&(row.dealer_index, row.recipient_index)),
        Some(row)
    );
    assert!(rows.belongs_to(&budget));
    drop(rows);
    drop(reservation);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn public_dkg_snapshot_and_finalization_refuse_without_cancelling_and_move_exact_owner() {
    let fixture = adaptive_beacon_fixture();
    let dkg = &fixture.session.record().adaptive_dkg;
    let budget = fixture_budget();
    let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
    let mut state = GlobalThresholdBeaconDkgStateV1::new(dkg.session, &crypto, &budget).unwrap();
    for row in &dkg.recipient_keys {
        state
            .record_recipient_key(dkg.session.start_height, row)
            .unwrap();
    }
    for row in &dkg.dealer_commitments {
        state
            .record_dealer_commitment(dkg.session.start_height, row, &crypto)
            .unwrap();
    }
    for row in &dkg.encrypted_shares {
        state
            .record_encrypted_share(dkg.session.commitments_end_height, row)
            .unwrap();
    }
    for row in &dkg.share_acceptances {
        state
            .record_share_acceptance(dkg.session.deliveries_end_height, row)
            .unwrap();
    }
    let height = state.last_updated_height;
    let retained = budget.reserved_bytes();
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - retained)
        .unwrap();
    assert!(matches!(
        state.public_snapshot(),
        Err(GlobalThresholdBeaconSessionError::Admission(
            AllocationRefusal::Capacity { .. }
        ))
    ));
    assert!(matches!(
        state.finalize(dkg.finalized_at_height, &crypto),
        Err(GlobalThresholdBeaconSessionError::Admission(
            AllocationRefusal::Capacity { .. }
        ))
    ));
    assert!(!state.aborted);
    assert!(state.finalized.is_none());
    assert_eq!(state.last_updated_height, height);
    assert_eq!(
        state.phase_at(dkg.finalized_at_height),
        crate::beacon::GlobalThresholdBeaconDkgPhaseV1::Finalizable
    );
    drop(blocker);
    let snapshot = state.public_snapshot().unwrap();
    assert!(snapshot.belongs_to(&budget));
    let raw = snapshot.record();
    assert_eq!(&raw.recipient_keys, &dkg.recipient_keys);
    assert_eq!(&raw.encrypted_shares, &dkg.encrypted_shares);
    assert_eq!(
        norito::encode_canonical(&snapshot).unwrap(),
        norito::encode_canonical(raw).unwrap()
    );
    drop(snapshot);
    let record = state.finalize(dkg.finalized_at_height, &crypto).unwrap();
    assert_eq!(record, fixture.session.record());
    let pointer = record.adaptive_dkg.encrypted_shares[0]
        .encrypted_share
        .as_ptr();
    let wire = norito::encode_canonical(record).unwrap();
    let final_owner = state.into_finalized().unwrap();
    assert_eq!(
        final_owner.adaptive_dkg.encrypted_shares[0]
            .encrypted_share
            .as_ptr(),
        pointer
    );
    assert!(final_owner.belongs_to(&budget));
    assert_eq!(norito::encode_canonical(&final_owner).unwrap(), wire);
    drop(final_owner);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn charged_byte_writer_retains_exact_backing_and_rejects_partial_append() {
    use std::io::Write as _;
    let budget = AllocationBudget::new(5);
    let mut bytes = ChargedBuffer::new(5, &budget).unwrap();
    let original = bytes.as_slice().as_ptr();
    let mut writer = ChargedBytesWriter(&mut bytes);
    writer.write_all(&[1, 2, 3]).unwrap();
    assert_eq!(
        writer.write_all(&[4, 5, 6]).unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(bytes.as_slice(), &[1, 2, 3]);
    assert_eq!(bytes.as_slice().as_ptr(), original);
    assert_eq!(bytes.capacity(), 5);
    assert!(bytes.belongs_to(&budget));
    assert_eq!(budget.reserved_bytes(), 5);
    ChargedBytesWriter(&mut bytes).write_all(&[4, 5]).unwrap();
    assert_eq!(bytes.as_slice(), &[1, 2, 3, 4, 5]);
    assert_eq!(bytes.as_slice().as_ptr(), original);
    drop(bytes);
    assert_eq!(budget.reserved_bytes(), 0);
}
