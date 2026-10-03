//! UNLINKED storage controls; fixture permits do not qualify a production owner or inventory.

use super::*;
use crate::musubi_publication_service::finality::tests::{ReaderFixture, reader_fixture};
use format::{ClaimDescriptorV1, ClaimFrameV1, decode_claim};
use iroha_allocation::{AllocationRefusal, release::ReleaseRegistration};
use iroha_core::state::StateReadOnly as _;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::AccountId,
    isi::musubi::AdvanceMusubiPinOutboxV1,
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use std::{
    future::Future as _,
    io::Write as _,
    os::unix::fs::PermissionsExt as _,
    pin::Pin,
    task::{Context, Waker},
    time::{Duration, Instant},
};

fn directory() -> (tempfile::TempDir, PrivateDirectory) {
    let temp = tempfile::tempdir().unwrap();
    let directory =
        PrivateDirectory::open_or_create(temp.path().join(CONTROL_DIRECTORY_V1)).unwrap();
    (temp, directory)
}

fn owner<'a>(
    fixture: &'a ReaderFixture,
    directory: &'a PrivateDirectory,
    reader: &'a MusubiPublicationPinOutboxHighWaterReaderV1,
) -> OriginalControlOwnerV1<'a> {
    // Test-only inert construction: the fixed marker here is not authenticated finality and
    // never issues a production permit. Only the original State allocation owner is exercised.
    OriginalControlOwnerV1 {
        directory,
        state: &fixture.state,
        reader,
        budget: fixture.state.query_view().execution_budget(),
        binding: ClaimBindingV1 {
            network_id: *fixture.query.network_id.as_bytes(),
            owner_marker_digest: [0x41; 32],
            session_id: [0x42; 32],
        },
        limits: ControlJournalLimitsV1 {
            max_records: 2,
            max_total_bytes: 2 * (MAX_WIRE_BYTES_V1 + MAX_CLAIM_BYTES_V1) as u64,
        },
    }
}

fn permit<'a>(owner: &'a OriginalControlOwnerV1<'a>) -> AdmittedControlSlotV1<'a> {
    let slot = ControlSlotV1::Advance {
        predecessor_revision: 0,
    };
    AdmittedControlSlotV1 {
        owner,
        slot,
        names: SlotNamesV1::new(slot).unwrap(),
        admitted_record_bytes: (MAX_WIRE_BYTES_V1 + MAX_CLAIM_BYTES_V1) as u64,
    }
}

fn reader(fixture: &ReaderFixture) -> MusubiPublicationPinOutboxHighWaterReaderV1 {
    MusubiPublicationPinOutboxHighWaterReaderV1::new(
        fixture.query.network_id,
        Arc::clone(&fixture.state),
    )
    .unwrap()
}

fn signed_wire(fixture: &ReaderFixture, binding: ClaimBindingV1) -> Vec<u8> {
    let key = KeyPair::from_seed(vec![0x53; 32], Algorithm::Ed25519);
    let authority = AccountId::new(key.public_key().clone());
    let instruction = AdvanceMusubiPinOutboxV1 {
        network_id: fixture.query.network_id,
        pin_authority: authority.clone(),
        session_id: binding.session_id,
        expected_revision: 0,
        expected_inventory_digest: [0; 32],
        inventory_digest: [0x54; 32],
    };
    let mut builder = TransactionBuilder::new(
        fixture.query.network_id,
        authority,
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction]);
    builder.set_creation_time(Duration::from_millis(42_001));
    // Tests obtain exact bytes through the canonical producer. Its ordinary Vec remains a
    // production funding blocker; the test does not relabel that producer as charged.
    builder.sign(key.private_key()).encode_wire_v1().unwrap()
}

fn captured<C>(
    permit: &AdmittedControlSlotV1<'_>,
    wire: &[u8],
    custody: C,
) -> CapturedExactControlWireV1<C> {
    let mut bytes = ChargedBuffer::new(wire.len(), &permit.owner.budget).unwrap();
    bytes.append(wire).unwrap();
    CapturedExactControlWireV1 {
        custody,
        bytes,
        binding: permit.owner.binding,
        slot: permit.slot,
    }
}

#[test]
fn exact_original_signed_wire_survives_retry_and_inert_reopen_without_replacement() {
    let fixture = reader_fixture();
    let (_temp, directory) = directory();
    let reader = reader(&fixture);
    let owner = owner(&fixture, &directory, &reader);
    let permit = permit(&owner);
    let wire = signed_wire(&fixture, owner.binding);
    let baseline = owner.budget.reserved_bytes();
    let custody = Arc::new(());
    let stored = permit
        .persist(captured(&permit, &wire, Arc::clone(&custody)))
        .unwrap_or_else(|failure| panic!("store: {:?}", failure.error));
    assert!(Arc::ptr_eq(&stored.original.custody, &custody));
    let stored = permit
        .persist(stored.original)
        .unwrap_or_else(|failure| panic!("exact retry: {:?}", failure.error));
    let recovered = permit.recover_inert().unwrap();
    assert_eq!(recovered.exact_wire(), wire);
    assert!(Arc::ptr_eq(recovered.state, &fixture.state));
    assert!(recovered.bytes.belongs_to(&owner.budget));
    assert_eq!(owner.budget.reserved_bytes(), baseline + 2 * wire.len());
    drop(recovered);
    assert_eq!(owner.budget.reserved_bytes(), baseline + wire.len());

    let mut changed_wire = wire.clone();
    changed_wire[1] ^= 1;
    let conflict = match permit.persist(captured(&permit, &changed_wire, Arc::clone(&custody))) {
        Ok(_) => panic!("same semantic slot cannot replace signed bytes"),
        Err(failure) => failure,
    };
    assert!(matches!(conflict.error, ControlJournalErrorV1::Conflict));
    assert!(Arc::ptr_eq(&conflict.original.custody, &custody));
    assert_eq!(conflict.original.bytes.as_slice(), changed_wire);
    assert_eq!(permit.recover_inert().unwrap().exact_wire(), wire);
    drop(conflict);
    drop(stored);
    assert_eq!(owner.budget.reserved_bytes(), baseline);
}

#[test]
fn reopening_after_original_directory_and_live_custody_drop_returns_only_inert_bytes() {
    let fixture = reader_fixture();
    let (temp, directory) = directory();
    let reader = reader(&fixture);
    let wire = {
        let owner = owner(&fixture, &directory, &reader);
        let permit = permit(&owner);
        let wire = signed_wire(&fixture, owner.binding);
        let live = Arc::new(());
        let weak = Arc::downgrade(&live);
        let stored = permit
            .persist(captured(&permit, &wire, live))
            .unwrap_or_else(|failure| panic!("store: {:?}", failure.error));
        drop(stored);
        assert!(weak.upgrade().is_none());
        wire
    };
    drop(directory);
    let reopened =
        PrivateDirectory::open_or_create(temp.path().join(CONTROL_DIRECTORY_V1)).unwrap();
    let owner = owner(&fixture, &reopened, &reader);
    let permit = permit(&owner);
    let baseline = owner.budget.reserved_bytes();
    let recovered = permit.recover_inert().unwrap();
    assert_eq!(recovered.exact_wire(), wire);
    assert!(Arc::ptr_eq(recovered.state, &fixture.state));
    assert_eq!(owner.budget.reserved_bytes(), baseline + wire.len());
    drop(recovered);
    assert_eq!(owner.budget.reserved_bytes(), baseline);
}

#[test]
fn interrupted_claim_partial_body_and_missing_pair_members_remain_occupied() {
    for complete_claim in [false, true] {
        let fixture = reader_fixture();
        let (_temp, directory) = directory();
        let reader = reader(&fixture);
        let owner = owner(&fixture, &directory, &reader);
        let permit = permit(&owner);
        let wire = signed_wire(&fixture, owner.binding);
        let descriptor = ClaimDescriptorV1::new(owner.binding, permit.slot, &wire).unwrap();
        let frame = ClaimFrameV1::encode(&descriptor, &owner.budget).unwrap();
        let mut claim = directory
            .create_borrowed_private(permit.names.claim(), MAX_CLAIM_BYTES_V1)
            .unwrap();
        if complete_claim {
            claim.write_all(frame.0.as_slice()).unwrap();
            drop(claim.seal_read_only().unwrap());
        } else {
            claim.write_all(&frame.0.as_slice()[..7]).unwrap();
            drop(claim);
        }
        assert!(permit.recover_inert().is_err());
        let failed = match permit.persist(captured(&permit, &wire, ())) {
            Ok(_) => panic!("an incomplete claim remains occupied"),
            Err(failure) => failure,
        };
        assert!(matches!(
            failed.error,
            ControlJournalErrorV1::OccupiedIncomplete
        ));
        assert!(!directory.path().join(permit.names.wire()).exists());
        assert_eq!(
            std::fs::metadata(directory.path().join(permit.names.claim()))
                .unwrap()
                .len(),
            if complete_claim {
                frame.0.as_slice().len() as u64
            } else {
                7
            }
        );
    }
}

#[test]
fn an_orphan_wire_or_partial_wire_after_a_sealed_claim_never_allows_replacement() {
    for orphan_wire in [false, true] {
        let fixture = reader_fixture();
        let (_temp, directory) = directory();
        let reader = reader(&fixture);
        let owner = owner(&fixture, &directory, &reader);
        let permit = permit(&owner);
        let wire = signed_wire(&fixture, owner.binding);
        if !orphan_wire {
            let descriptor = ClaimDescriptorV1::new(owner.binding, permit.slot, &wire).unwrap();
            let frame = ClaimFrameV1::encode(&descriptor, &owner.budget).unwrap();
            let mut claim = directory
                .create_borrowed_private(permit.names.claim(), MAX_CLAIM_BYTES_V1)
                .unwrap();
            claim.write_all(frame.0.as_slice()).unwrap();
            drop(claim.seal_read_only().unwrap());
        }
        let mut stored_wire = directory
            .create_borrowed_private(permit.names.wire(), MAX_WIRE_BYTES_V1)
            .unwrap();
        stored_wire.write_all(&wire[..7]).unwrap();
        if orphan_wire {
            drop(stored_wire.seal_read_only().unwrap());
        } else {
            drop(stored_wire);
        }
        assert!(permit.recover_inert().is_err());
        let original = captured(&permit, &wire, Arc::new(()));
        let weak = Arc::downgrade(&original.custody);
        let failed = match permit.persist(original) {
            Ok(_) => panic!("partial or orphan wire cannot be replaced"),
            Err(failure) => failure,
        };
        assert!(matches!(
            failed.error,
            ControlJournalErrorV1::OccupiedIncomplete
        ));
        assert!(weak.upgrade().is_some());
        assert_eq!(failed.original.bytes.as_slice(), wire);
        assert_eq!(
            std::fs::metadata(directory.path().join(permit.names.wire()))
                .unwrap()
                .len(),
            7
        );
        assert_eq!(
            directory.path().join(permit.names.claim()).exists(),
            !orphan_wire
        );
    }
}

#[test]
fn reopened_evidence_rejects_custody_changes_and_claim_or_wire_substitution() {
    let fixture = reader_fixture();
    let (_temp, directory) = directory();
    let reader = reader(&fixture);
    let owner = owner(&fixture, &directory, &reader);
    let permit = permit(&owner);
    let wire = signed_wire(&fixture, owner.binding);
    let _stored = permit
        .persist(captured(&permit, &wire, ()))
        .unwrap_or_else(|failure| panic!("{:?}", failure.error));
    let wire_path = directory.path().join(permit.names.wire());
    std::fs::set_permissions(&wire_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(permit.recover_inert().is_err());
    let mut changed = wire.clone();
    changed[1] ^= 1;
    std::fs::write(&wire_path, changed).unwrap();
    std::fs::set_permissions(&wire_path, std::fs::Permissions::from_mode(0o400)).unwrap();
    assert!(matches!(
        permit.recover_inert(),
        Err(ControlJournalErrorV1::Conflict)
    ));
    assert!(permit.persist(captured(&permit, &wire, ())).is_err());
}

#[test]
fn canonical_claim_frame_rejects_truncation_checksum_trailing_bytes_and_binding_changes() {
    let fixture = reader_fixture();
    let (_temp, directory) = directory();
    let reader = reader(&fixture);
    let owner = owner(&fixture, &directory, &reader);
    let permit = permit(&owner);
    let wire = signed_wire(&fixture, owner.binding);
    let descriptor = ClaimDescriptorV1::new(owner.binding, permit.slot, &wire).unwrap();
    let frame = ClaimFrameV1::encode(&descriptor, &owner.budget).unwrap();
    assert_eq!(decode_claim(frame.0.as_slice()).unwrap(), descriptor);
    for length in [0, 1, frame.0.as_slice().len() - 1] {
        assert!(decode_claim(&frame.0.as_slice()[..length]).is_err());
    }
    let mut changed = frame.0.as_slice().to_vec();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    assert!(decode_claim(&changed).is_err());
    let mut trailing = frame.0.as_slice().to_vec();
    trailing.push(0);
    assert!(decode_claim(&trailing).is_err());
    let mut wrong_binding = owner.binding;
    wrong_binding.session_id[0] ^= 1;
    let original = captured(&permit, &wire, ());
    let wrong = CapturedExactControlWireV1 {
        binding: wrong_binding,
        ..original
    };
    assert!(permit.persist(wrong).is_err());
    assert!(!directory.path().join(permit.names.claim()).exists());
}

#[test]
fn original_state_capacity_refusal_preserves_live_custody_and_release_owner_before_mutation() {
    let fixture = reader_fixture();
    let (_temp, directory) = directory();
    let reader = reader(&fixture);
    let owner = owner(&fixture, &directory, &reader);
    let permit = permit(&owner);
    let wire = signed_wire(&fixture, owner.binding);
    let deadline = Instant::now() + Duration::from_secs(60);
    let custody = Arc::new(());
    let original = captured(&permit, &wire, (Arc::clone(&custody), deadline));
    let mut prepaid = owner
        .budget
        .try_reserve(ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut prepaid).unwrap();
    drop(prepaid);
    let held = owner
        .budget
        .try_reserve_bytes(owner.budget.limit_bytes() - owner.budget.reserved_bytes())
        .unwrap();
    let failure = match permit.persist(original) {
        Ok(_) => panic!("the original State pool has no claim-buffer capacity"),
        Err(failure) => failure,
    };
    assert_eq!(failure.original.custody.1, deadline);
    assert!(Arc::ptr_eq(&failure.original.custody.0, &custody));
    assert_eq!(failure.original.bytes.as_slice(), wire);
    let ControlJournalErrorV1::Deferred(ref error) = failure.error else {
        panic!("original refusal lost");
    };
    let Some(AllocationRefusal::Capacity { release, .. }) = error.allocation_refusal() else {
        panic!("original release owner lost");
    };
    let mut wait = release.clone().wait_for_release(&mut registration);
    let mut context = Context::from_waker(Waker::noop());
    assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
    let unrelated = AllocationBudget::new(1);
    drop(unrelated.try_reserve_bytes(1).unwrap());
    assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
    assert!(!directory.path().join(permit.names.claim()).exists());
    drop(held);
    assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
    drop(wait);
    let stored = permit
        .persist(failure.original)
        .unwrap_or_else(|failure| panic!("retry: {:?}", failure.error));
    assert_eq!(stored.original.custody.1, deadline);
    assert!(Arc::ptr_eq(&stored.original.custody.0, &custody));
}

#[test]
fn foreign_backing_and_insufficient_admitted_extent_refuse_before_native_creation() {
    let fixture = reader_fixture();
    let (_temp, directory) = directory();
    let reader = reader(&fixture);
    let owner = owner(&fixture, &directory, &reader);
    let mut permit = permit(&owner);
    let wire = signed_wire(&fixture, owner.binding);
    let foreign = AllocationBudget::new(MAX_WIRE_BYTES_V1);
    let mut bytes = ChargedBuffer::new(wire.len(), &foreign).unwrap();
    bytes.append(&wire).unwrap();
    let original = CapturedExactControlWireV1 {
        custody: (),
        bytes,
        binding: owner.binding,
        slot: permit.slot,
    };
    assert!(permit.persist(original).is_err());
    permit.admitted_record_bytes = 1;
    assert!(permit.persist(captured(&permit, &wire, ())).is_err());
    assert!(!directory.path().join(permit.names.claim()).exists());
}

#[test]
fn slot_names_bind_semantic_occurrence_and_limits_do_not_grant_permits() {
    let first = SlotNamesV1::new(ControlSlotV1::Advance {
        predecessor_revision: 0,
    })
    .unwrap();
    let next = SlotNamesV1::new(ControlSlotV1::Advance {
        predecessor_revision: 1,
    })
    .unwrap();
    assert_ne!(first.claim(), next.claim());
    assert_eq!(first.claim(), "a0000000000000000.claim");
    assert_eq!(first.wire(), "a0000000000000000.wire");
    let challenge = SlotNamesV1::new(ControlSlotV1::Check { challenge: [1; 32] }).unwrap();
    assert_ne!(first.claim(), challenge.claim());
    assert!(SlotNamesV1::new(ControlSlotV1::Check { challenge: [0; 32] }).is_err());
    for limits in [
        ControlJournalLimitsV1 {
            max_records: 0,
            max_total_bytes: 1,
        },
        ControlJournalLimitsV1 {
            max_records: MAX_CONTROL_RECORDS_V1 + 1,
            max_total_bytes: 1,
        },
        ControlJournalLimitsV1 {
            max_records: 1,
            max_total_bytes: 0,
        },
        ControlJournalLimitsV1 {
            max_records: 1,
            max_total_bytes: MAX_CONTROL_BYTES_V1 + 1,
        },
    ] {
        assert!(limits.validate().is_err());
    }
    assert!(
        ControlJournalLimitsV1 {
            max_records: MAX_CONTROL_RECORDS_V1,
            max_total_bytes: MAX_CONTROL_BYTES_V1
        }
        .validate()
        .is_ok()
    );
}

// TODO: Before integration, add actual PendingMusubiPinOutboxCheckV1 failure/custody tests
// through the completed funded producer and authoritative permit issuer. This generic custody
// control does not claim those absent ownership boundaries or their physical allocation census.
