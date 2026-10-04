//! UNLINKED original-State controls; stage as a child of the existing tests module.
//!
//! TODO: Link these only with the reviewed writer/Core producer. They reuse its actual
//! CertifiedTestChain Fixture; no supplied State hash, synthetic pending stage or new pool can
//! stand in for the live original owner. Full producer allocation census remains a separate gate.

use super::*;
use crate::query::musubi_pin_outbox::exact_wire::{
    MusubiPinOutboxWireCaptureErrorV1, OriginalWireDestination,
};
use iroha_allocation::{
    AllocationBudget, AllocationRefusal, ChargedBuffer, release::ReleaseRegistration,
};
use std::{
    future::Future as _,
    io::Write as _,
    pin::Pin,
    task::{Context, Waker},
};

fn instruction_backing(pending: &PendingMusubiPinOutboxCheckV1) -> *const InstructionBox {
    let iroha_data_model::transaction::Executable::Instructions(instructions) =
        pending.signed_transaction().instructions()
    else {
        panic!("one native Check")
    };
    instructions.as_ptr()
}

#[test]
fn exact_wire_capture_keeps_original_state_signed_backing_challenge_and_deadline() {
    let fixture = Fixture::new();
    let pending = fixture.pending();
    let original = pending.signed_transaction().encode_wire_v1().unwrap();
    let backing = instruction_backing(&pending);
    let challenge = pending.prepared.instruction.challenge;
    let deadline = pending.deadline();
    let budget = pending.prepared.state.ivm_execution_budget();
    let baseline = budget.reserved_bytes();
    let captured = pending.capture_exact_wire_v1().unwrap();
    assert_eq!(captured.exact_wire(), original);
    assert_eq!(captured.deadline(), deadline);
    assert_eq!(budget.reserved_bytes(), baseline + original.len());
    let (pending, bytes) = captured.into_parts();
    assert!(bytes.belongs_to(&budget));
    assert!(Arc::ptr_eq(&pending.prepared.state, fixture.chain.state()));
    assert_eq!(instruction_backing(&pending), backing);
    assert_eq!(pending.prepared.instruction.challenge, challenge);
    assert_eq!(pending.deadline(), deadline);
    assert_eq!(
        pending.signed_transaction().encode_wire_v1().unwrap(),
        original
    );
    drop(bytes);
    assert_eq!(budget.reserved_bytes(), baseline);
}

#[test]
fn capture_capacity_refusal_returns_original_pending_and_release_owner() {
    let fixture = Fixture::new();
    let pending = fixture.pending();
    let exact = pending.signed_transaction().encode_wire_v1().unwrap();
    let deadline = pending.deadline();
    let challenge = pending.prepared.instruction.challenge;
    let backing = instruction_backing(&pending);
    let budget = pending.prepared.state.ivm_execution_budget();
    let mut prepaid = budget
        .try_reserve(ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut prepaid).unwrap();
    drop(prepaid);
    let held = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    let failure = match pending.capture_exact_wire_v1() {
        Ok(_) => panic!("original pool is exhausted"),
        Err(failure) => failure,
    };
    let (pending, error) = failure.into_parts();
    assert!(Arc::ptr_eq(&pending.prepared.state, fixture.chain.state()));
    assert_eq!(instruction_backing(&pending), backing);
    assert_eq!(pending.prepared.instruction.challenge, challenge);
    assert_eq!(pending.deadline(), deadline);
    assert_eq!(
        pending.signed_transaction().encode_wire_v1().unwrap(),
        exact
    );
    let MusubiPinOutboxWireCaptureErrorV1::Deferred(original) = error else {
        panic!("typed original admission refusal lost");
    };
    let Some(AllocationRefusal::Capacity { release, .. }) = original.allocation_refusal() else {
        panic!("original release owner lost");
    };
    let mut wait = release.clone().wait_for_release(&mut registration);
    let mut context = Context::from_waker(Waker::noop());
    assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
    let other = AllocationBudget::new(1); // A negative control, never the producer's pool.
    drop(other.try_reserve_bytes(1).unwrap());
    assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
    drop(held);
    assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
    drop(wait);
    let captured = pending.capture_exact_wire_v1().unwrap();
    assert_eq!(captured.exact_wire(), exact);
    assert_eq!(captured.deadline(), deadline);
    let (pending, _) = captured.into_parts();
    assert_eq!(instruction_backing(&pending), backing);
    assert_eq!(pending.prepared.instruction.challenge, challenge);
}

#[test]
fn expired_original_round_is_retained_without_recreating_pending_or_extending_deadline() {
    let fixture = Fixture::new();
    let mut pending = fixture.pending();
    // Use the existing original-round expiry seam; no sleep or replacement round is involved.
    let original_deadline = pending.deadline();
    let backing = instruction_backing(&pending);
    let challenge = pending.prepared.instruction.challenge;
    pending.prepared.round.expire_for_test();
    let expired = pending.deadline();
    let failure = match pending.capture_exact_wire_v1() {
        Ok(_) => panic!("an expired paid Check cannot be recaptured"),
        Err(failure) => failure,
    };
    assert!(matches!(
        failure.error(),
        MusubiPinOutboxWireCaptureErrorV1::Check(Error::Expired)
    ));
    let pending = failure.into_pending();
    assert_eq!(instruction_backing(&pending), backing);
    assert_eq!(pending.prepared.instruction.challenge, challenge);
    assert!(pending.deadline() < original_deadline);
    assert_eq!(pending.deadline(), expired);
}

#[test]
fn admitted_destination_refuses_growth_and_reclaims_only_its_actual_backing() {
    let fixture = Fixture::new();
    let budget = fixture.chain.state().ivm_execution_budget();
    let baseline = budget.reserved_bytes();
    let mut destination = OriginalWireDestination(ChargedBuffer::new(1, &budget).unwrap());
    destination.write_all(&[1]).unwrap();
    assert!(destination.write_all(&[2]).is_err());
    destination.flush().unwrap();
    assert_eq!(budget.reserved_bytes(), baseline + 1);
    drop(destination);
    assert_eq!(budget.reserved_bytes(), baseline);
}
