//! Real binding graph ownership, retained prefixes and canonical parity.
use super::*;
use crate::{
    account::{MultisigMember, MultisigPolicy},
    amx_prepare_streaming_allocations::{PhysicalObservation, with_nth_exact_allocation_denial},
};
use iroha_allocation::AllocationRefusal;
use iroha_crypto::{Algorithm, KeyPair};

fn fixture(multisig: bool) -> AssetUidBindingV1 {
    let key = |seed| {
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
            .public_key()
            .clone()
    };
    let account = if multisig {
        AccountId::new_multisig(
            MultisigPolicy::new(
                2,
                vec![
                    MultisigMember::new(key(11), 1).unwrap(),
                    MultisigMember::new(key(22), 2).unwrap(),
                ],
            )
            .unwrap(),
        )
    } else {
        AccountId::new(key(11))
    };
    AssetUidBindingV1 {
        asset_definition_id: crate::sccp::escrow::sccp_taira_xor_asset_definition_id(),
        account_id: account,
        uid: [7; 32],
    }
}
fn decode(
    bytes: &[u8],
    pool: &AllocationBudget,
) -> Result<AllocatedAssetUidBindingV1, OwnedPreparedDecodeError<Error>> {
    decode_asset_uid_binding_with_owned_controls(
        bytes,
        norito::canonical_decode_limits(bytes.len()),
        pool,
    )
}
fn controls() -> usize {
    PreparedDecodeWorkspace::allocation_layouts()
        .iter()
        .map(std::alloc::Layout::size)
        .sum()
}

#[test]
fn binding_graph_matches_ordinary_and_retains_only_its_actual_original_allocations() {
    for multi in [false, true] {
        let expected = fixture(multi);
        let bytes = norito::encode_canonical(&expected).unwrap();
        assert_eq!(
            norito::decode_canonical::<AssetUidBindingV1>(&bytes).unwrap(),
            expected
        );
        let pool = AllocationBudget::new(1 << 20);
        let other = AllocationBudget::new(1 << 20);
        let complete = decode(&bytes, &pool).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(complete.canonical(), &expected);
        assert!(complete.belongs_to(&pool));
        assert!(!complete.belongs_to(&other));
        assert_eq!(
            norito::encode_canonical(complete.canonical()).unwrap(),
            bytes
        );
        assert_eq!(pool.reserved_bytes(), complete.allocation_bytes().unwrap());
        assert!(pool.reserved_bytes() > 0);
        drop(complete);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn binding_header_precedes_exhausted_pool_and_malformed_frames_match_ordinary() {
    let frame = norito::encode_canonical(&fixture(false)).unwrap();
    let mut bad_magic = frame.clone();
    bad_magic[0] = 0;
    let empty = AllocationBudget::new(0);
    assert!(
        matches!(decode(&bad_magic, &empty), Err(OwnedPreparedDecodeError::Decode(PreparedDecodeError::Codec(error))) if matches!(error.error(), norito::Error::InvalidMagic))
    );
    assert!(matches!(
        decode(&frame, &empty),
        Err(OwnedPreparedDecodeError::Admission(_))
    ));
    let mut trailing = frame.clone();
    trailing.push(0);
    let mut changed = frame.clone();
    *changed.last_mut().unwrap() ^= 1;
    for invalid in [&frame[..4], &frame[..frame.len() - 1], &trailing, &changed] {
        assert!(norito::decode_canonical::<AssetUidBindingV1>(invalid).is_err());
        let pool = AllocationBudget::new(1 << 20);
        assert!(decode(invalid, &pool).is_err());
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn binding_capacity_after_both_controls_preserves_exact_key_refusal() {
    let expected = fixture(false);
    let frame = norito::encode_canonical(&expected).unwrap();
    let key = expected
        .account_id
        .try_signatory()
        .unwrap()
        .retained_allocation_layout();
    let pool = AllocationBudget::new(controls() + key.size());
    let blocker = pool.try_reserve_bytes(1).unwrap();
    let error = match decode(&frame, &pool) {
        Err(error) => error,
        Ok(_) => panic!("key backing must refuse"),
    };
    assert!(
        matches!(error, OwnedPreparedDecodeError::Decode(PreparedDecodeError::Destination(Error::Allocation(ChargedBufferError::Admission(AllocationRefusal::Capacity {requested_bytes, reserved_bytes, ..})))) if requested_bytes == key.size() && reserved_bytes == controls() + 1)
    );
    assert_eq!(pool.reserved_bytes(), 1);
    drop(blocker);
    assert_eq!(decode(&frame, &pool).unwrap().canonical(), &expected);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn binding_first_key_physical_failure_is_original_and_refunds_actual_controls() {
    let expected = fixture(false);
    let frame = norito::encode_canonical(&expected).unwrap();
    let layout = expected
        .account_id
        .try_signatory()
        .unwrap()
        .retained_allocation_layout();
    let pool = AllocationBudget::new(1 << 20);
    let (result, refused) = with_nth_exact_allocation_denial(layout, 0, || decode(&frame, &pool));
    assert!(
        refused,
        "actual first account key allocation must be reached"
    );
    assert!(
        matches!(result, Err(OwnedPreparedDecodeError::Decode(PreparedDecodeError::Destination(Error::Allocation(ChargedBufferError::Allocator {requested_bytes})))) if requested_bytes == layout.size())
    );
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(decode(&frame, &pool).unwrap().canonical(), &expected);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn binding_late_multisig_key_refusal_keeps_original_prefix_and_retries_same_source() {
    let expected = fixture(true);
    let frame = norito::encode_canonical(&expected).unwrap();
    let crate::account::AccountController::Multisig(policy) = expected.account_id.controller()
    else {
        unreachable!()
    };
    let layout = policy.members()[0]
        .public_key()
        .retained_allocation_layout();
    assert!(
        policy
            .members()
            .iter()
            .all(|member| member.public_key().retained_allocation_layout() == layout)
    );
    let pool = AllocationBudget::new(1 << 20);
    let mut pending = PendingAssetUidBindingDecodeV1::new(&pool);
    let observation = PhysicalObservation::start();
    let (result, refused) = with_nth_exact_allocation_denial(layout, 1, || {
        pending.decode(&frame, norito::canonical_decode_limits(frame.len()))
    });
    assert!(
        refused,
        "actual second multisig key allocation must be reached"
    );
    assert!(
        matches!(result, Err(OwnedPreparedDecodeError::Decode(PreparedDecodeError::Destination(Error::Allocation(ChargedBufferError::Allocator {requested_bytes})))) if requested_bytes == layout.size())
    );
    let members = pending
        .destination
        .account
        .policy
        .members
        .members
        .as_ref()
        .expect("real retained member backing");
    assert_eq!(members.as_slice().len(), 1);
    let members_pointer = members.as_slice().as_ptr();
    let key_pointer = observation
        .snapshot()
        .single_live_pointer(layout)
        .expect("one actual retained key");
    assert!(pending.belongs_to(&pool));
    assert!(pending.finish().is_err());
    let retained = pool.reserved_bytes();
    assert!(retained > layout.size());
    let changed = frame.clone();
    assert!(
        pending
            .decode(&changed, norito::canonical_decode_limits(changed.len()))
            .is_err()
    );
    assert_eq!(pool.reserved_bytes(), retained);
    pending
        .decode(&frame, norito::canonical_decode_limits(frame.len()))
        .unwrap();
    let complete = pending.finish().unwrap();
    assert_eq!(complete.canonical(), &expected);
    // Existing member Vec and first key remain the actual backing through completion.
    let crate::account::AccountController::Multisig(policy) =
        complete.canonical().account_id.controller()
    else {
        unreachable!()
    };
    assert_eq!(policy.members().as_ptr(), members_pointer);
    assert_eq!(observation.snapshot().retirements(key_pointer), 0);
    assert_eq!(pool.reserved_bytes(), complete.allocation_bytes().unwrap());
    assert!(
        pending
            .decode(&frame, norito::canonical_decode_limits(frame.len()))
            .is_err()
    );
    drop(complete);
    assert_eq!(observation.snapshot().retirements(key_pointer), 1);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn binding_owned_prepared_entry_retains_both_physical_control_failures() {
    let frame = norito::encode_canonical(&fixture(false)).unwrap();
    let layout = PreparedDecodeWorkspace::allocation_layouts()[0];
    for index in 0..2 {
        let pool = AllocationBudget::new(1 << 20);
        let (result, refused) =
            with_nth_exact_allocation_denial(layout, index, || decode(&frame, &pool));
        assert!(refused);
        assert!(
            matches!(result, Err(OwnedPreparedDecodeError::Decode(PreparedDecodeError::Scope(ncore::PreparedDecodeScopeError::Allocation(iroha_allocation::PrepaidSharedError::Allocator {requested_bytes})))) if requested_bytes == layout.size())
        );
        assert_eq!(pool.reserved_bytes(), 0);
        assert!(decode(&frame, &pool).is_ok());
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn binding_late_capacity_retains_multisig_prefix_and_refunds_on_drop() {
    let expected = fixture(true);
    let frame = norito::encode_canonical(&expected).unwrap();
    let probe = AllocationBudget::new(1 << 20);
    drop(decode(&frame, &probe).unwrap());
    let demand = probe.peak_reserved_bytes();
    assert_eq!(probe.reserved_bytes(), 0);
    let pool = AllocationBudget::new(demand);
    let held = pool.try_reserve_bytes(1).unwrap();
    let mut pending = PendingAssetUidBindingDecodeV1::new(&pool);
    let result = pending.decode(&frame, norito::canonical_decode_limits(frame.len()));
    assert!(matches!(
        result,
        Err(OwnedPreparedDecodeError::Decode(
            PreparedDecodeError::Destination(Error::Allocation(ChargedBufferError::Admission(
                AllocationRefusal::Capacity { .. }
            )))
        ))
    ));
    let members = pending
        .destination
        .account
        .policy
        .members
        .members
        .as_ref()
        .expect("real retained member backing");
    assert_eq!(members.as_slice().len(), 1);
    let pointer = members.as_slice().as_ptr();
    assert!(pending.finish().is_err());
    assert!(pending.belongs_to(&pool));
    drop(held);
    pending
        .decode(&frame, norito::canonical_decode_limits(frame.len()))
        .unwrap();
    let complete = pending.finish().unwrap();
    let crate::account::AccountController::Multisig(policy) =
        complete.canonical().account_id.controller()
    else {
        unreachable!()
    };
    assert_eq!(policy.members().as_ptr(), pointer);
    assert_eq!(complete.canonical(), &expected);
    drop(complete);
    drop(pending);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn binding_late_capacity_wait_cannot_self_wake_or_replace_its_original_controls() {
    use iroha_allocation::release::ReleaseRegistration;
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, Waker},
    };
    let frame = norito::encode_canonical(&fixture(true)).unwrap();
    let probe = AllocationBudget::new(1 << 20);
    drop(decode(&frame, &probe).unwrap());
    let pool = AllocationBudget::new(probe.peak_reserved_bytes());
    let control_layout = PreparedDecodeWorkspace::allocation_layouts()[0];
    let wait_pool = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let mut wait_credit = wait_pool
        .try_reserve(ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut wait_credit).unwrap();
    registration.bind_thread(std::thread::current());
    let held = pool.try_reserve_bytes(1).unwrap();
    let mut pending = PendingAssetUidBindingDecodeV1::new(&pool);
    let error = pending
        .decode(&frame, norito::canonical_decode_limits(frame.len()))
        .unwrap_err();
    let OwnedPreparedDecodeError::Decode(PreparedDecodeError::Destination(Error::Allocation(
        ChargedBufferError::Admission(AllocationRefusal::Capacity { release, .. }),
    ))) = error
    else {
        panic!("late original graph capacity refusal")
    };
    assert!(pending.workspace.as_ref().unwrap().belongs_to(&pool));
    assert_eq!(
        pending
            .destination
            .account
            .policy
            .members
            .members
            .as_ref()
            .unwrap()
            .as_slice()
            .len(),
        1
    );
    let retained = pool.reserved_bytes();
    let mut wait = release.wait_for_release(&mut registration);
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(
        Pin::new(&mut wait).poll(&mut context),
        Poll::Pending,
        "late refusal must retain its own two counter controls without returning credit"
    );
    let foreign = AllocationBudget::new(1);
    let foreign_held = foreign.try_reserve_bytes(1).unwrap();
    drop(foreign_held);
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Pending);
    let (repeated, replaced) = with_nth_exact_allocation_denial(control_layout, 0, || {
        pending.decode(&frame, norito::canonical_decode_limits(frame.len()))
    });
    assert!(
        !replaced,
        "a refused retry cannot replace either original control"
    );
    assert!(matches!(
        repeated,
        Err(OwnedPreparedDecodeError::Decode(
            PreparedDecodeError::Destination(Error::Allocation(ChargedBufferError::Admission(_)))
        ))
    ));
    assert_eq!(pool.reserved_bytes(), retained);
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Pending);
    drop(held);
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Ready(()));
    drop(wait);
    let (retry, replaced) = with_nth_exact_allocation_denial(control_layout, 0, || {
        pending.decode(&frame, norito::canonical_decode_limits(frame.len()))
    });
    assert!(
        !replaced,
        "a successful retry cannot replace either original control"
    );
    retry.unwrap();
    let complete = pending.finish().unwrap();
    assert!(pending.workspace.is_none());
    assert_eq!(pool.reserved_bytes(), complete.allocation_bytes().unwrap());
    drop(complete);
    assert_eq!(pool.reserved_bytes(), 0);

    // Cancellation retires the same controls, spans and partial graph, returning
    // their original credits without releasing the unrelated external holder.
    let held = pool.try_reserve_bytes(1).unwrap();
    let mut cancelled = PendingAssetUidBindingDecodeV1::new(&pool);
    let error = cancelled
        .decode(&frame, norito::canonical_decode_limits(frame.len()))
        .unwrap_err();
    let OwnedPreparedDecodeError::Decode(PreparedDecodeError::Destination(Error::Allocation(
        ChargedBufferError::Admission(AllocationRefusal::Capacity { release, .. }),
    ))) = error
    else {
        panic!("same late original graph refusal")
    };
    let mut wait = release.wait_for_release(&mut registration);
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Pending);
    drop(cancelled);
    assert_eq!(pool.reserved_bytes(), 1);
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Ready(()));
    drop(wait);
    drop(held);
    assert_eq!(pool.reserved_bytes(), 0);
}
