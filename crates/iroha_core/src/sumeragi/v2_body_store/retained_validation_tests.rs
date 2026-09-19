// Marker failures retain the same affine payload; these fixtures do not admit State.

use super::*;
use crate::sumeragi::{
    v2_apply::validation_custody::{
        CarrierCustodyError, CarrierValidator, RetainedBodyValidationService,
        test_support::TrackedOwner,
    },
    v2_body_store::retained_validation::{
        fail_next_marker_directory_sync, fail_next_marker_file_sync,
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Validator {
    commitment: wire::ExecutionCommitment,
    calls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}
impl CarrierValidator for Validator {
    type Owner = TrackedOwner;
    type Error = String;
    fn prepare(
        &mut self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<Self::Owner, Self::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(TrackedOwner::new(
            context,
            body,
            self.commitment,
            Arc::clone(&self.drops),
        ))
    }
}
fn validator(
    receipt: &super::super::DurableBodyReceipt,
) -> (Validator, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    (
        Validator {
            commitment: ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment(),
            calls: Arc::clone(&calls),
            drops: Arc::clone(&drops),
        },
        calls,
        drops,
    )
}

#[test]
fn retained_marker_file_sync_refusal_keeps_owner_through_retry_abort_and_consume() {
    let directory = TempDir::new().unwrap();
    let (context, keys) = context_and_keys();
    let (body, manifest) = body_and_manifest(&context, &keys, None);
    let mut store = V2BodyStore::open(directory.path(), context).unwrap();
    let durable = store.store(manifest, body).unwrap();
    let (producer, calls, drops) = validator(&durable);
    let mut service = store.retained_validation_service(producer).unwrap();
    fail_next_marker_file_sync();
    assert!(matches!(
        store.execute_retained_durable_validation(
            durable.clone(),
            durable.manifest_hash(),
            &mut service
        ),
        Err(V2BodyStoreError::Io { .. })
    ));
    let allocation = service
        .owner_for_test(durable.subject())
        .unwrap()
        .allocation();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(service.marker_counts_for_test(), (1, 0));
    assert!(store.validated.is_empty());
    assert!(store.rejected.is_empty());
    assert!(
        !store
            .validated_path_for(durable.round(), durable.subject())
            .exists()
    );
    let outcome = store
        .execute_retained_durable_validation(durable.clone(), durable.manifest_hash(), &mut service)
        .unwrap();
    let receipt = outcome.validated_receipt().unwrap().clone();
    assert_eq!(service.marker_counts_for_test(), (0, 1));
    assert_eq!(
        service
            .owner_for_test(durable.subject())
            .unwrap()
            .allocation(),
        allocation
    );
    store
        .execute_retained_durable_validation(durable.clone(), durable.manifest_hash(), &mut service)
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop(service.select(&receipt).unwrap());
    assert_eq!(
        service
            .owner_for_test(durable.subject())
            .unwrap()
            .allocation(),
        allocation
    );
    let result = service.select(&receipt).unwrap().try_consume(|owner| {
        assert_eq!(owner.allocation(), allocation);
        Err::<(), _>((owner, "local publication refusal"))
    });
    assert_eq!(result, Err("local publication refusal"));
    assert_eq!(
        service
            .owner_for_test(durable.subject())
            .unwrap()
            .allocation(),
        allocation
    );
    service
        .select(&receipt)
        .unwrap()
        .try_consume(|owner| {
            assert_eq!(owner.allocation(), allocation);
            drop(owner);
            Ok::<_, (TrackedOwner, ())>(())
        })
        .unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(matches!(
        service.select(&receipt),
        Err(CarrierCustodyError::Unconfirmed)
    ));
    assert!(matches!(
        store.execute_retained_durable_validation(
            durable.clone(),
            durable.manifest_hash(),
            &mut service
        ),
        Err(V2BodyStoreError::CarrierCustody(
            CarrierCustodyError::MissingOwner
        ))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn retained_reproposal_directory_sync_refusal_preserves_prior_confirmed_receipt() {
    let directory = TempDir::new().unwrap();
    let (context, keys) = context_and_keys();
    let (body, manifest) = body_and_manifest(&context, &keys, None);
    let mut store = V2BodyStore::open(directory.path(), context.clone()).unwrap();
    let durable = store.store(manifest.clone(), body.clone()).unwrap();
    let (producer, calls, drops) = validator(&durable);
    let mut service = store.retained_validation_service(producer).unwrap();
    let original = store
        .execute_retained_durable_validation(durable.clone(), durable.manifest_hash(), &mut service)
        .unwrap()
        .validated_receipt()
        .unwrap()
        .clone();
    let allocation = service
        .owner_for_test(durable.subject())
        .unwrap()
        .allocation();
    let round = wire::ConsensusRound {
        view: 7,
        ..durable.round()
    };
    let manifest = encode_payload(&context, round, manifest.subject, &body)
        .unwrap()
        .manifest()
        .clone();
    let later = store.store(manifest, body).unwrap();
    fail_next_marker_directory_sync();
    assert!(matches!(
        store.execute_retained_durable_validation(
            later.clone(),
            later.manifest_hash(),
            &mut service
        ),
        Err(V2BodyStoreError::Io { .. })
    ));
    assert!(
        store
            .validated_path_for(later.round(), later.subject())
            .exists(),
        "rename completed before directory fsync refusal"
    );
    assert_eq!(service.marker_counts_for_test(), (1, 1));
    assert_eq!(store.validated.len(), 1);
    drop(service.select(&original).unwrap());
    let unconfirmed = ValidatedBodyReceipt {
        durable: later.clone(),
        execution_commitment: original.execution_commitment(),
    };
    assert!(matches!(
        service.select(&unconfirmed),
        Err(CarrierCustodyError::Unconfirmed)
    ));
    let later_receipt = store
        .execute_retained_durable_validation(later.clone(), later.manifest_hash(), &mut service)
        .unwrap()
        .validated_receipt()
        .unwrap()
        .clone();
    assert_eq!(service.marker_counts_for_test(), (0, 2));
    assert_eq!(
        service
            .owner_for_test(durable.subject())
            .unwrap()
            .allocation(),
        allocation
    );
    drop(service.select(&original).unwrap());
    drop(service.select(&later_receipt).unwrap());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
}

#[test]
fn retained_consumption_tombstone_rejects_delayed_earlier_round_without_execution() {
    let directory = TempDir::new().unwrap();
    let (context, keys) = context_and_keys();
    let (body, origin) = body_and_manifest(&context, &keys, None);
    let mut store = V2BodyStore::open(directory.path(), context.clone()).unwrap();
    let manifest = encode_payload(
        &context,
        wire::ConsensusRound {
            view: 5,
            ..origin.round
        },
        origin.subject,
        &body,
    )
    .unwrap()
    .manifest()
    .clone();
    let current = store.store(manifest, body.clone()).unwrap();
    let (producer, calls, drops) = validator(&current);
    let mut service = store.retained_validation_service(producer).unwrap();
    let receipt = store
        .execute_retained_durable_validation(current.clone(), current.manifest_hash(), &mut service)
        .unwrap()
        .validated_receipt()
        .unwrap()
        .clone();
    service
        .select(&receipt)
        .unwrap()
        .try_consume(|owner| {
            drop(owner);
            Ok::<_, (TrackedOwner, ())>(())
        })
        .unwrap();
    let manifest = encode_payload(
        &context,
        wire::ConsensusRound {
            view: 4,
            ..origin.round
        },
        origin.subject,
        &body,
    )
    .unwrap()
    .manifest()
    .clone();
    let earlier = store.store(manifest, body).unwrap();
    assert!(matches!(
        store.execute_retained_durable_validation(
            earlier.clone(),
            earlier.manifest_hash(),
            &mut service
        ),
        Err(V2BodyStoreError::CarrierCustody(
            CarrierCustodyError::MissingOwner
        ))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn retained_validation_requires_exact_store_and_existing_cached_owner() {
    let directory = TempDir::new().unwrap();
    let (context, keys) = context_and_keys();
    let (body, manifest) = body_and_manifest(&context, &keys, None);
    let mut store = V2BodyStore::open(directory.path(), context.clone()).unwrap();
    let durable = store.store(manifest, body).unwrap();
    let (producer, calls, _) = validator(&durable);
    let mut service = store.retained_validation_service(producer).unwrap();
    assert!(matches!(
        store.execute_retained_durable_validation(
            durable.clone(),
            HashOf::from_untyped_unchecked(Hash::new(b"wrong manifest")),
            &mut service
        ),
        Err(V2BodyStoreError::ReceiptMismatch)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    store
        .execute_retained_durable_validation(durable.clone(), durable.manifest_hash(), &mut service)
        .unwrap();
    let (replacement, replacement_calls, _) = validator(&durable);
    let mut missing = store.retained_validation_service(replacement).unwrap();
    assert!(matches!(
        store.execute_retained_durable_validation(
            durable.clone(),
            durable.manifest_hash(),
            &mut missing
        ),
        Err(V2BodyStoreError::CarrierCustody(
            CarrierCustodyError::MissingOwner
        ))
    ));
    assert_eq!(replacement_calls.load(Ordering::SeqCst), 0);
    // A process incarnation must close its exclusive directory ownership before
    // reopening. The old service still retains its distinct instance identity.
    drop(store);
    let mut reopened = V2BodyStore::open(directory.path(), context).unwrap();
    assert!(matches!(
        reopened.execute_retained_durable_validation(
            durable.clone(),
            durable.manifest_hash(),
            &mut service
        ),
        Err(V2BodyStoreError::CarrierCustody(
            CarrierCustodyError::Identity
        ))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn retained_descriptor_capacity_refuses_before_execution_or_marker_write() {
    let directory = TempDir::new().unwrap();
    let (context, keys) = context_and_keys();
    let mut store = V2BodyStore::open(directory.path(), context.clone()).unwrap();
    let (body, manifest) = body_and_manifest_for_view(&context, &keys, 0);
    let first = store.store(manifest, body).unwrap();
    let (body, manifest) = body_and_manifest_for_view(&context, &keys, 1);
    let second = store.store(manifest, body).unwrap();
    assert_ne!(first.subject(), second.subject());
    let (producer, calls, drops) = validator(&first);
    // Retain two actual body frames, but constrain the private descriptor table
    // to one entry so each pre-execution refusal can be exercised independently.
    let mut service =
        RetainedBodyValidationService::new(producer, store.instance_identity(), 1).unwrap();
    let confirmed = store
        .execute_retained_durable_validation(first.clone(), first.manifest_hash(), &mut service)
        .unwrap()
        .validated_receipt()
        .unwrap()
        .clone();
    let allocation = service
        .owner_for_test(first.subject())
        .unwrap()
        .allocation();
    assert!(matches!(
        store.execute_retained_durable_validation(
            second.clone(),
            second.manifest_hash(),
            &mut service
        ),
        Err(V2BodyStoreError::CarrierCustody(
            CarrierCustodyError::Capacity
        ))
    ));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "full marker table must refuse before preparing another candidate"
    );
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        service
            .owner_for_test(first.subject())
            .unwrap()
            .allocation(),
        allocation
    );
    assert_eq!(service.marker_counts_for_test(), (0, 1));
    assert!(service.owner_for_test(second.subject()).is_none());
    assert!(
        !store
            .validated
            .contains_key(&(second.round(), second.subject()))
    );
    assert!(store.rejected.is_empty());
    assert!(
        !store
            .validated_path_for(second.round(), second.subject())
            .exists()
    );
    service
        .select(&confirmed)
        .unwrap()
        .try_consume(|owner| {
            drop(owner);
            Ok::<_, (TrackedOwner, ())>(())
        })
        .unwrap();
    assert_eq!(service.marker_counts_for_test(), (0, 0));
    assert!(matches!(
        store.execute_retained_durable_validation(
            second.clone(),
            second.manifest_hash(),
            &mut service
        ),
        Err(V2BodyStoreError::CarrierCustody(
            CarrierCustodyError::Capacity
        ))
    ));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "consumed subject tombstone must still occupy its bounded candidate descriptor"
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(
        !store
            .validated
            .contains_key(&(second.round(), second.subject()))
    );
    assert!(store.rejected.is_empty());
    assert!(
        !store
            .validated_path_for(second.round(), second.subject())
            .exists()
    );
}
