//! Checkpoint durability stays a local retry of the original publication owner.

use super::*;
use crate::{snapshot::CheckpointDeferral, test_allocations::allocations_during};

#[test]
fn checkpoint_writer_observations_cross_publication_boundary_without_invented_pool_wake() {
    for original in [
        CheckpointDeferral::WorkerStarting { height: 7 },
        CheckpointDeferral::QueueFull { height: 7 },
        CheckpointDeferral::WorkerUnavailable { height: 7 },
        CheckpointDeferral::DurabilityPending { height: 7 },
        CheckpointDeferral::Persistence {
            height: 7,
            kind: std::io::ErrorKind::PermissionDenied,
        },
    ] {
        let mut outcome = None;
        assert_eq!(
            allocations_during(|| {
                outcome = Some(original_refusal(
                    TransactionsBlockError::CheckpointDeferred(original),
                ));
            }),
            0,
        );
        let PublicationError::Deferred(actual) = outcome.unwrap() else {
            panic!("checkpoint IO must retain the unfinished publication owner")
        };
        assert!(matches!(&actual, PublicationDeferral::Checkpoint(value) if *value == original));
        assert!(actual.release_wait().is_none());
        assert!(actual.allocation_refusal().is_none());
        assert!(actual.execution().is_none());
        assert!(actual.state_capture().is_none());
    }
}
