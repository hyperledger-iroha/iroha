//! Local lane storage failures retain their source and never authorize proposal rejection.

use std::{error::Error as _, io, sync::Arc};

use super::{BlockValidationError, event};
use crate::sumeragi::lanes::merge::MergeError;

#[derive(Debug)]
struct OriginalFailure(Arc<()>);
impl core::fmt::Display for OriginalFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("original lane read failed")
    }
}
impl std::error::Error for OriginalFailure {}

#[test]
fn merge_state_view_failures_preserve_original_source_without_rejection() {
    use crate::state::{MergeLedgerCommitError, StateViewError};
    use iroha_allocation::{AllocationBudget, AllocationCharge, release::ReleaseNotification};
    use std::{
        future::Future as _,
        task::{Context, Waker},
    };

    let layout = ReleaseNotification::allocation_layout::<AllocationCharge>();
    let pool = AllocationBudget::new(
        layout.size()
            + 2 * iroha_allocation::release::ReleaseRegistration::allocation_layout().size(),
    );
    let mut registrations = [
        crate::unit_test_support::release_registration(&pool),
        crate::unit_test_support::release_registration(&pool),
    ];
    let mut reservation = pool.try_reserve(layout).unwrap();
    let source =
        ReleaseNotification::try_new_charged(reservation.try_split(layout).unwrap()).unwrap();
    let mutex = std::sync::Mutex::new(());
    let held = source.guard(mutex.lock().unwrap());
    let original = source.observe();
    let mut waits = Vec::new();
    for (registration, error) in registrations.iter_mut().zip([
        BlockValidationError::from(MergeError::StateView(StateViewError::Busy(
            original.clone(),
        ))),
        BlockValidationError::from_certified_merge_stage_error(MergeLedgerCommitError::StateView(
            StateViewError::Busy(original.clone()),
        )),
    ]) {
        assert_eq!(event::map_block_err_to_reason(&error), None);
        let BlockValidationError::StateView(StateViewError::Busy(retained)) = &error else {
            panic!("State view source survives the original merge boundary")
        };
        assert_eq!(retained, &original);
        assert!(std::ptr::eq(
            error
                .source()
                .unwrap()
                .downcast_ref::<StateViewError>()
                .unwrap(),
            match &error {
                BlockValidationError::StateView(retained) => retained,
                _ => unreachable!(),
            }
        ));
        let mut wait = retained.clone().wait_for_release(registration);
        assert!(
            std::pin::Pin::new(&mut wait)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        waits.push(wait);
    }
    drop(held);
    for mut wait in waits {
        assert!(
            std::pin::Pin::new(&mut wait)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
    }
    for original in [StateViewError::Changed, StateViewError::Poisoned] {
        let error = BlockValidationError::from(MergeError::StateView(original));
        assert_eq!(event::map_block_err_to_reason(&error), None);
        assert!(matches!(error, BlockValidationError::StateView(_)));
    }
}

#[test]
fn lane_io_failure_retains_kind_and_original_source_without_rejection() {
    for kind in [
        io::ErrorKind::WouldBlock,
        io::ErrorKind::InvalidData,
        io::ErrorKind::PermissionDenied,
    ] {
        let identity = Arc::new(());
        let error = BlockValidationError::from(MergeError::Storage(
            io::Error::new(kind, OriginalFailure(identity.clone())).into(),
        ));
        assert_eq!(event::map_block_err_to_reason(&error), None);
        let BlockValidationError::LaneStorage(original) = &error else {
            panic!("local storage failure lost its owner: {error:?}");
        };
        assert_eq!(original.kind(), kind);
        let retained = error.source().unwrap().downcast_ref::<io::Error>().unwrap();
        assert!(std::ptr::eq(retained, original));
        let marker = retained
            .get_ref()
            .unwrap()
            .downcast_ref::<OriginalFailure>()
            .unwrap();
        assert!(Arc::ptr_eq(&identity, &marker.0));
    }
}

#[test]
fn missing_lane_height_retains_recovery_classification() {
    let error = BlockValidationError::from(MergeError::Pending("committed height absent".into()));
    assert_eq!(event::map_block_err_to_reason(&error), None);
    assert!(
        matches!(error, BlockValidationError::LocalStorageRecoveryRequired { reason }
        if reason == "committed height absent")
    );
}

#[test]
fn malformed_lane_reference_remains_deterministically_invalid() {
    let error = BlockValidationError::from(MergeError::Invalid("noncontiguous merge".into()));
    assert_eq!(
        event::map_block_err_to_reason(&error),
        Some(iroha_data_model::block::error::BlockRejectionReason::TransactionValidationFailed)
    );
    assert!(
        matches!(error, BlockValidationError::ExecutionContextInvalid(reason)
        if reason == "noncontiguous merge")
    );
}
