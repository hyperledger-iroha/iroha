//! A checkpoint writer moves the original resource refusal once and retains its image.

use super::*;
use crate::{
    snapshot::checkpoint_capture::CheckpointWriteError,
    sumeragi::{
        driver::traits::{PublicationDeferral, PublicationError},
        executor::publication::original_refusal,
    },
    test_allocations::allocations_during,
};
use iroha_allocation::AllocationRefusal;

#[test]
fn checkpoint_resource_refusal_moves_once_before_same_image_retry() {
    // A retired fixture version must not refund the deliberately occupied evidence pool.
    let _epoch = crossbeam_epoch::pin();
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let receiver = state
        .runtime_checkpoint_capture
        .start(NonZeroUsize::new(64 * 1024 * 1024).unwrap());
    let directory = tempfile::tempdir().unwrap();
    let writer = maker(Arc::clone(&state), directory.path());
    let proposal = chain.proposal(Some(HOUR_MS), Vec::new());
    let mut original = chain.begin_proposal(proposal, Default::default()).unwrap();
    let result = original.result();
    original.publish(Signers::Quorum).unwrap_err();
    let job = receiver.try_recv().unwrap();
    let image = job.capture.snapshot.json.as_ptr();
    let budget = state.ivm_execution_budget();
    let baseline = budget.reserved_bytes();
    let occupied = budget
        .try_reserve_bytes(budget.limit_bytes() - baseline)
        .unwrap();
    let failure = writer.persist_pending_checkpoint(&job.capture).unwrap_err();
    let CheckpointWriteError::Execution(owner) = &failure else {
        panic!("the actual certified-history reader must retain its resource cause: {failure:?}");
    };
    // This clone is only a test observation; production moves its sole error owner.
    let expected = owner.allocation_refusal().unwrap().clone();
    assert!(matches!(expected, AllocationRefusal::Capacity { .. }));
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    job.acknowledge(Err(failure));
    assert!(
        !job.needs_write(),
        "the writer cannot overwrite an unobserved failure"
    );
    assert!(!job.visible());
    assert_eq!(state.committed_height(), 1);
    assert!(entries(directory.path()).unwrap().is_empty());
    let mut returned = None;
    assert_eq!(
        allocations_during(|| {
            returned = Some(original_refusal(job.take_refusal().unwrap()));
        }),
        0
    );
    let PublicationError::Deferred(returned) = returned.unwrap() else {
        panic!("the original local refusal must remain a publication deferral");
    };
    assert_eq!(returned.allocation_refusal(), Some(&expected));
    assert!(returned.release_wait().is_some());
    assert!(
        job.take_refusal().is_none(),
        "the original cause crosses exactly once"
    );
    assert!(job.needs_write());
    assert_eq!(
        budget.reserved_bytes(),
        budget.limit_bytes(),
        "moving the cause refunds no retained owner"
    );
    assert_eq!(job.capture.snapshot.json.as_ptr(), image);

    drop(occupied);
    assert_eq!(budget.reserved_bytes(), baseline);
    job.acknowledge(writer.persist_pending_checkpoint(&job.capture));
    original.publish(Signers::Quorum).unwrap();
    assert_eq!(original.result(), result);
    assert_eq!(job.capture.snapshot.json.as_ptr(), image);
    assert!(job.visible());
    assert_eq!(returned.allocation_refusal(), Some(&expected));
    drop(original);
    assert_eq!(state.committed_height(), 2);
    writer.promote_visible_checkpoint(&job.capture).unwrap();
    assert!(entries(directory.path()).unwrap().is_empty());
    assert_eq!(
        checkpoint_store::entries(directory.path()).unwrap().len(),
        1
    );
    state.runtime_checkpoint_capture.stop();
}

#[test]
fn startup_checkpoint_pump_preserves_original_evidence_resource_refusal() {
    let _epoch = crossbeam_epoch::pin();
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let directory = tempfile::tempdir().unwrap();
    let mut writer = maker(Arc::clone(&state), directory.path());
    writer.checkpoints.receiver = Some(
        state
            .runtime_checkpoint_capture
            .arm(writer.max_payload_bytes),
    );
    writer.begin_checkpoint_replay(&state).unwrap();
    let proposal = chain.proposal(Some(HOUR_MS), Vec::new());
    let mut original = chain.begin_proposal(proposal, Default::default()).unwrap();
    let result = original.result();
    original.publish(Signers::Quorum).unwrap_err();
    let budget = state.ivm_execution_budget();
    let occupied = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    let failure = writer.checkpoint_replay_step().unwrap_err();
    let PublicationError::Deferred(failure) = original_refusal(failure) else {
        panic!("the synchronous writer must return the same typed local cause");
    };
    assert!(matches!(
        failure.allocation_refusal(),
        Some(AllocationRefusal::Capacity { .. })
    ));
    assert!(failure.release_wait().is_some());
    assert_eq!(state.committed_height(), 1);
    assert_eq!(state.exact_durable_block_count().unwrap(), 2);
    assert!(
        !directory
            .path()
            .join(crate::snapshot::SNAPSHOT_CURRENT_FILE_NAME)
            .exists()
    );
    drop(occupied);
    writer.checkpoint_replay_step().unwrap();
    original.publish(Signers::Quorum).unwrap();
    assert_eq!(original.result(), result);
    drop(original);
    writer.checkpoint_replay_step().unwrap();
    assert_eq!(state.committed_height(), 2);
    assert!(entries(directory.path()).unwrap().is_empty());
    assert_eq!(
        checkpoint_store::entries(directory.path()).unwrap().len(),
        1
    );
    assert!(
        !directory
            .path()
            .join(crate::snapshot::SNAPSHOT_CURRENT_FILE_NAME)
            .exists()
    );
}

#[test]
fn checkpoint_reader_failure_preserves_exact_cause_without_pool_wait() {
    let original =
        CheckpointWriteError::from(crate::snapshot::TryReadError::PayloadAllocatorFailure {
            requested_bytes: 123,
        });
    let mut result = None;
    assert_eq!(
        allocations_during(|| {
            result = Some(original_refusal(original.into_publication(7)));
        }),
        0
    );
    let PublicationError::Deferred(actual) = result.unwrap() else {
        panic!("original typed reader failure must survive publication");
    };
    assert!(matches!(
        &actual,
        PublicationDeferral::CheckpointRead(
            crate::snapshot::TryReadError::PayloadAllocatorFailure {
                requested_bytes: 123
            }
        )
    ));
    assert!(actual.release_wait().is_none());
    assert!(actual.allocation_refusal().is_none());
    let permanent = original_refusal(
        CheckpointWriteError::from(crate::snapshot::TryReadError::SnapshotGenerationInvalid {
            path: PathBuf::from("original-checkpoint"),
            reason: "invalid signed payload".to_owned(),
        })
        .into_publication(7),
    );
    assert!(permanent.is_recovery_required());
    assert!(
        matches!(permanent, PublicationError::RecoveryRequired(message)
        if message.contains("original-checkpoint") && message.contains("invalid signed payload"))
    );
}
