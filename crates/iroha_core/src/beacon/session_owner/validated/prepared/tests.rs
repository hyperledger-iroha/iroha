//! Same-graph sealing, complete preclaim admission and exact returned retry custody.

use super::*;
use crate::{
    beacon::{
        PreparedLocalGlobalThresholdBeaconDkgSeatV1, fixtures::*,
        global_threshold_beacon_roster_hash_v1, session_owner::retain_canonical_session,
    },
    test_allocations::{allocations_during, refuse_one_layout_during},
};

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
fn prepared(
    fixture: &AdaptiveBeaconFixture,
    keys: &[iroha_crypto::KeyPair],
    roster: &[PeerId],
    budget: &AllocationBudget,
) -> PreparedGlobalThresholdBeaconSessionVerificationV1 {
    let local = PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(
        fixture.session.adaptive_dkg.session,
        roster,
        1,
        &keys[0],
        budget,
    )
    .unwrap();
    local.prepare_final_session_verifier().unwrap()
}

#[test]
fn prepared_seal_moves_original_four_and_thirty_one_graphs_without_late_allocation() {
    for seats in [4, 31] {
        let (fixture, keys, roster) = fixture(seats);
        let budget = fixture_budget();
        let prepared = prepared(&fixture, &keys, &roster, &budget);
        let source = retain_canonical_session(fixture.session.record(), &budget).unwrap();
        let graph_bytes = source.allocation_bytes().unwrap();
        let shares = source.get().public_shares.as_ptr();
        let recipients = source.get().adaptive_dkg.recipient_keys.as_ptr();
        let encrypted = source.get().adaptive_dkg.encrypted_shares[0]
            .encrypted_share
            .as_ptr();
        let retained =
            graph_bytes + ChargedShared::<RetainedPayload<Payload>>::allocation_layout().size();
        let occupied = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        let occupied_bytes = occupied.remaining_bytes();
        let mut outcome = None;
        assert_eq!(
            allocations_during(|| outcome = Some(prepared.seal(source, &fixture.binding))),
            0
        );
        let sealed = outcome
            .unwrap()
            .unwrap_or_else(|(_, _, error)| panic!("original graph sealing: {error}"));
        assert_eq!(sealed.record(), fixture.session.record());
        assert_eq!(sealed.record().public_shares.as_ptr(), shares);
        assert_eq!(
            sealed.record().adaptive_dkg.recipient_keys.as_ptr(),
            recipients
        );
        assert_eq!(
            sealed.record().adaptive_dkg.encrypted_shares[0]
                .encrypted_share
                .as_ptr(),
            encrypted
        );
        assert_eq!(sealed.retained_allocation_bytes(), retained);
        assert_eq!(budget.reserved_bytes(), retained + occupied_bytes);
        assert!(sealed.belongs_to(&budget));
        drop(occupied);
        drop(sealed);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn prepared_seal_refuses_foreign_graph_and_invalid_transcript_without_consuming_either_owner() {
    let (fixture, keys, roster) = fixture(4);
    let budget = fixture_budget();
    let foreign_budget = fixture_budget();
    let prepared = prepared(&fixture, &keys, &roster, &budget);
    let source = retain_canonical_session(fixture.session.record(), &foreign_budget).unwrap();
    let original = source.get().public_shares.as_ptr();
    let before = (budget.reserved_bytes(), foreign_budget.reserved_bytes());
    let (prepared, source, error) = match prepared.seal(source, &fixture.binding) {
        Err(parts) => parts,
        Ok(_) => panic!("foreign graph cannot borrow this shell"),
    };
    assert!(matches!(
        error,
        GlobalThresholdBeaconSessionError::ForeignReservation
    ));
    assert_eq!(source.get().public_shares.as_ptr(), original);
    assert_eq!(
        (budget.reserved_bytes(), foreign_budget.reserved_bytes()),
        before
    );
    drop(source);
    assert_eq!(foreign_budget.reserved_bytes(), 0);
    let mut bad = fixture.session.record().clone();
    bad.transcript_hash[0] ^= 1;
    let mut bad_binding = fixture.binding.clone();
    bad_binding.transcript_hash = bad.transcript_hash;
    let bad = retain_canonical_session(&bad, &budget).unwrap();
    let bad_pointer = bad.get().public_shares.as_ptr();
    let valid = retain_canonical_session(fixture.session.record(), &budget).unwrap();
    let valid_pointer = valid.get().public_shares.as_ptr();
    let occupied = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    let mut outcome = None;
    assert_eq!(
        allocations_during(|| outcome = Some(prepared.seal(bad, &bad_binding))),
        0
    );
    let (prepared, bad, error) = match outcome.unwrap() {
        Err(parts) => parts,
        Ok(_) => panic!("incorrect reconstructed transcript must remain invalid"),
    };
    assert!(matches!(
        error,
        GlobalThresholdBeaconSessionError::Invalid(GlobalThresholdBeaconError::TranscriptMismatch)
    ));
    assert_eq!(bad.get().public_shares.as_ptr(), bad_pointer);
    assert!(prepared.workspace.dealers.as_slice().is_empty());
    let mut outcome = None;
    assert_eq!(
        allocations_during(|| outcome = Some(prepared.seal(valid, &fixture.binding))),
        0
    );
    let sealed = outcome
        .unwrap()
        .unwrap_or_else(|(_, _, error)| panic!("same prepared verifier retry: {error}"));
    assert_eq!(sealed.record().public_shares.as_ptr(), valid_pointer);
    drop(occupied);
    drop(bad);
    drop(sealed);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn prepared_seal_original_capacity_and_shell_refusal_leave_local_attempt_unchanged() {
    let (fixture, keys, roster) = fixture(4);
    let budget = fixture_budget();
    let local = PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(
        fixture.session.adaptive_dkg.session,
        &roster,
        1,
        &keys[0],
        &budget,
    )
    .unwrap();
    let floor = budget.reserved_bytes();
    let occupied = budget
        .try_reserve_bytes(budget.limit_bytes() - floor)
        .unwrap();
    let error = match local.prepare_final_session_verifier() {
        Err(error) => error,
        Ok(_) => panic!("occupied original pool cannot create verifier backing"),
    };
    let GlobalThresholdBeaconSessionError::Admission(actual) = error else {
        panic!("exact original pool refusal");
    };
    let AllocationRefusal::Capacity {
        requested_bytes, ..
    } = &actual
    else {
        panic!("original capacity pressure");
    };
    assert_eq!(
        actual,
        budget.try_reserve_bytes(*requested_bytes).unwrap_err()
    );
    drop(occupied);
    assert_eq!(budget.reserved_bytes(), floor);
    let layout = ChargedShared::<RetainedPayload<Payload>>::allocation_layout();
    let mut failure = None;
    let ((), refused) = refuse_one_layout_during(layout, || {
        failure = Some(local.prepare_final_session_verifier())
    });
    assert!(refused, "the original physical shared shell was reached");
    assert!(
        matches!(failure.unwrap(), Err(GlobalThresholdBeaconSessionError::Shared(PrepaidSharedError::Allocator { requested_bytes })) if requested_bytes == layout.size())
    );
    assert_eq!(budget.reserved_bytes(), floor);
    let prepared = local.prepare_final_session_verifier().unwrap();
    assert!(prepared.belongs_to(&budget));
    drop(prepared);
    assert_eq!(budget.reserved_bytes(), floor);
    drop(local);
    assert_eq!(budget.reserved_bytes(), 0);
}
