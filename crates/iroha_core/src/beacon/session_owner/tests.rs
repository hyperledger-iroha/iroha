//! Complete nested graph admission, original-source retry and physical refusal controls.

use super::*;
use crate::{
    beacon::{fixtures::*, global_threshold_beacon_roster_hash_v1},
    test_allocations::{allocations_during, refuse_one_layout_during},
};
use iroha_allocation::{
    ChargedBufferError, ChargedBufferFromChargeError, release::ReleaseRegistration,
};
use std::task::{Context, Waker};

fn fixture(seats: u16) -> AdaptiveBeaconFixture {
    let mut session = adaptive_dkg_session_fixture();
    session.committee_size = seats;
    session.threshold = (seats - 1) / 3 + 1;
    let keys = adaptive_fixture_signing_keys(seats);
    session.roster_hash = global_threshold_beacon_roster_hash_v1(
        &keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>(),
    );
    adaptive_beacon_fixture_for_session_and_keys(session, &keys, &fixture_budget())
}

fn distinct<T>(source: &[T], captured: &[T]) {
    assert_eq!(source.len(), captured.len());
    if !source.is_empty() {
        assert_ne!(source.as_ptr(), captured.as_ptr());
    }
}
pub(super) fn check_allocations(
    source: &GlobalThresholdBeaconKeySessionV1,
    captured: &GlobalThresholdBeaconKeySessionV1,
) {
    assert_eq!(source, captured);
    distinct(&source.public_shares, &captured.public_shares);
    let source = &source.adaptive_dkg;
    let captured = &captured.adaptive_dkg;
    distinct(&source.dealer_commitments, &captured.dealer_commitments);
    for (a, b) in source
        .dealer_commitments
        .iter()
        .zip(&captured.dealer_commitments)
    {
        distinct(&a.coefficient_commitments, &b.coefficient_commitments);
        distinct(a.signature.payload(), b.signature.payload());
    }
    distinct(&source.recipient_keys, &captured.recipient_keys);
    for (a, b) in source.recipient_keys.iter().zip(&captured.recipient_keys) {
        distinct(
            a.validator.public_key().try_to_bytes().unwrap().1,
            b.validator.public_key().try_to_bytes().unwrap().1,
        );
        distinct(&a.mlkem768_public_key, &b.mlkem768_public_key);
        distinct(a.signature.payload(), b.signature.payload());
    }
    distinct(&source.encrypted_shares, &captured.encrypted_shares);
    for (a, b) in source
        .encrypted_shares
        .iter()
        .zip(&captured.encrypted_shares)
    {
        distinct(&a.mlkem768_ciphertext, &b.mlkem768_ciphertext);
        distinct(&a.encrypted_share, &b.encrypted_share);
        distinct(a.signature.payload(), b.signature.payload());
    }
    distinct(&source.share_acceptances, &captured.share_acceptances);
    for (a, b) in source
        .share_acceptances
        .iter()
        .zip(&captured.share_acceptances)
    {
        distinct(a.signature.payload(), b.signature.payload());
    }
    distinct(&source.qualified_dealers, &captured.qualified_dealers);
}

#[test]
fn real_minimum_and_maximum_session_graphs_use_only_exact_original_allocations() {
    for seats in [4, 31] {
        let fixture = fixture(seats);
        let source = fixture.session.record();
        let demand = Demand::for_session(source).unwrap();
        let n = usize::from(seats);
        assert_eq!(demand.charges, 6 + 5 * n + 4 * n * n);
        let bytes = demand.total_bytes().unwrap();
        let budget = AllocationBudget::new(bytes);
        let before = norito::to_bytes(source).unwrap();
        let mut captured = None;
        let allocations = allocations_during(|| {
            captured = Some(retain_canonical_session(source, &budget).unwrap());
        });
        assert_eq!(
            allocations,
            demand.charges + 1,
            "every live nested allocation plus one exact ledger; no ordinary clone or scratch allocation"
        );
        let captured = captured.unwrap();
        assert!(captured.belongs_to(&budget));
        assert_eq!(budget.reserved_bytes(), bytes);
        check_allocations(source, captured.get());
        assert_eq!(norito::to_bytes(captured.get()).unwrap(), before);
        assert_eq!(norito::to_bytes(source).unwrap(), before);
        drop(captured);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn complete_graph_capacity_refusal_keeps_exact_original_release_and_retry() {
    let fixture = fixture(4);
    let source = fixture.session.record();
    let bytes = Demand::for_session(source).unwrap().total_bytes().unwrap();
    let floor = ReleaseRegistration::allocation_layout().size();
    let budget = AllocationBudget::new(bytes + floor);
    let mut registration = crate::unit_test_support::release_registration(&budget);
    let blocker = budget.try_reserve_bytes(1).unwrap();
    let expected = budget.try_reserve_bytes(bytes).unwrap_err();
    let mut result = None;
    assert_eq!(
        allocations_during(|| {
            result = Some(retain_canonical_session(source, &budget));
        }),
        0
    );
    let error = result.unwrap().err().expect("real finite pool is occupied");
    let SessionGraphError::Admission(actual) = error else {
        panic!("must retain original typed admission");
    };
    assert_eq!(actual, expected);
    let AllocationRefusal::Capacity {
        release,
        requested_bytes,
        ..
    } = actual
    else {
        panic!("actual outstanding owner has a release source");
    };
    assert_eq!(requested_bytes, bytes);
    assert_eq!(budget.reserved_bytes(), floor + 1);
    let mut context = Context::from_waker(Waker::noop());
    assert!(registration.poll_wait(&release, &mut context).is_pending());
    drop(blocker);
    assert!(registration.poll_wait(&release, &mut context).is_ready());
    registration.cancel();
    let captured = retain_canonical_session(source, &budget).unwrap();
    check_allocations(source, captured.get());
    assert_eq!(budget.reserved_bytes(), floor + bytes);
    drop(captured);
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);

    let short = AllocationBudget::new(bytes - 1);
    let error = retain_canonical_session(source, &short)
        .err()
        .expect("one byte below complete demand");
    assert!(
        matches!(error, SessionGraphError::Admission(AllocationRefusal::ExceedsLimit { requested_bytes, limit_bytes }) if requested_bytes == bytes && limit_bytes == bytes - 1)
    );
    assert_eq!(short.reserved_bytes(), 0);
}

#[test]
fn physical_refusal_at_every_nested_storage_kind_retires_partial_graph_before_retry() {
    let fixture = fixture(4);
    let source = fixture.session.record();
    let dkg = &source.adaptive_dkg;
    let demand = Demand::for_session(source).unwrap();
    let layouts = [
        Layout::array::<AllocationCharge>(demand.charges).unwrap(),
        Layout::array::<GlobalThresholdBeaconPublicShareV1>(source.public_shares.len()).unwrap(),
        Layout::array::<GlobalThresholdBeaconDkgDealerCommitmentV1>(dkg.dealer_commitments.len())
            .unwrap(),
        Layout::array::<[u8; 96]>(dkg.dealer_commitments[0].coefficient_commitments.len()).unwrap(),
        dkg.dealer_commitments[0]
            .signature
            .retained_allocation_layout(),
        Layout::array::<GlobalThresholdBeaconDkgRecipientKeyV1>(dkg.recipient_keys.len()).unwrap(),
        dkg.recipient_keys[0]
            .validator
            .public_key()
            .retained_allocation_layout(),
        Layout::array::<u8>(dkg.recipient_keys[0].mlkem768_public_key.len()).unwrap(),
        Layout::array::<GlobalThresholdBeaconDkgEncryptedShareV1>(dkg.encrypted_shares.len())
            .unwrap(),
        Layout::array::<u8>(dkg.encrypted_shares[0].mlkem768_ciphertext.len()).unwrap(),
        Layout::array::<u8>(dkg.encrypted_shares[0].encrypted_share.len()).unwrap(),
        Layout::array::<GlobalThresholdBeaconDkgShareAcceptanceV1>(dkg.share_acceptances.len())
            .unwrap(),
        Layout::array::<u16>(dkg.qualified_dealers.len()).unwrap(),
    ];
    let original = norito::to_bytes(source).unwrap();
    for layout in layouts {
        let budget = AllocationBudget::new(demand.total_bytes().unwrap());
        let (result, refused) =
            refuse_one_layout_during(layout, || retain_canonical_session(source, &budget));
        assert!(
            refused,
            "selected actual layout must be reached: {layout:?}"
        );
        let error = result.err().expect("physical allocation must fail closed");
        let actual_bytes = match error {
            SessionGraphError::Buffer(PrepaidBufferError::Allocation(
                ChargedBufferError::Allocator { requested_bytes },
            )) => requested_bytes,
            SessionGraphError::PublicKey(PublicKeyAllocationError::Allocation(
                ChargedBufferFromChargeError::Allocator { layout },
            ))
            | SessionGraphError::Signature(SignatureAllocationError::Allocation(
                ChargedBufferFromChargeError::Allocator { layout },
            )) => layout.size(),
            other => panic!("physical allocator failure must remain concrete: {other}"),
        };
        assert_eq!(actual_bytes, layout.size());
        assert_eq!(
            budget.reserved_bytes(),
            0,
            "all incomplete destination allocations retired before their credits"
        );
        assert_eq!(norito::to_bytes(source).unwrap(), original);
        let captured = retain_canonical_session(source, &budget).unwrap();
        check_allocations(source, captured.get());
        drop(captured);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn graph_custody_preserves_empty_invalid_bytes_without_claiming_validation() {
    let fixture = fixture(4);
    let mut source = fixture.session.record().clone();
    source.version = 0;
    source.adaptive_dkg.dealer_commitments[0]
        .coefficient_commitments
        .clear();
    source.adaptive_dkg.dealer_commitments[0].signature = Signature::from_bytes(&[]);
    source.adaptive_dkg.recipient_keys[0]
        .mlkem768_public_key
        .clear();
    source.adaptive_dkg.recipient_keys[0].signature = Signature::from_bytes(&[0; 96]);
    source.adaptive_dkg.encrypted_shares[0]
        .mlkem768_ciphertext
        .clear();
    source.adaptive_dkg.encrypted_shares[0]
        .encrypted_share
        .clear();
    source.adaptive_dkg.qualified_dealers.clear();
    let demand = Demand::for_session(&source).unwrap();
    let budget = AllocationBudget::new(demand.total_bytes().unwrap());
    let captured = retain_canonical_session(&source, &budget).unwrap();
    check_allocations(&source, captured.get());
    assert_eq!(
        norito::to_bytes(&source).unwrap(),
        norito::to_bytes(captured.get()).unwrap()
    );
    assert!(matches!(
        crate::beacon::validate_global_threshold_beacon_session_v1(
            captured.get(),
            &fixture.binding,
            &budget,
        ),
        Err(crate::beacon::GlobalThresholdBeaconSessionError::Invalid(
            crate::beacon::GlobalThresholdBeaconError::UnsupportedVersion { actual: 0 }
        ))
    ));
    drop(captured);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn completed_canonical_graph_unwind_retires_all_original_credits_without_touching_source() {
    let fixture = fixture(4);
    let source = fixture.session.record();
    let before = norito::to_bytes(source).unwrap();
    let bytes = Demand::for_session(source).unwrap().total_bytes().unwrap();
    let budget = AllocationBudget::new(bytes);
    let captured = retain_canonical_session(source, &budget).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _original_graph = captured;
        panic!("exercise completed original session graph cleanup");
    }));
    assert!(result.is_err());
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(norito::to_bytes(source).unwrap(), before);
}
