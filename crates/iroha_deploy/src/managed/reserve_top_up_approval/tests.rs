//! Public evidence separation and original generated approval custody controls.
use super::*;

#[test]
fn applied_report_and_copied_finality_cannot_create_private_top_up_approval() {
    let mut report = progress(OperationStatus::Applied, None, None);
    report.finalized = Some(ManagedTransactionFinality {
        transaction_hash: iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
            b"unproved decision",
        )),
        height: 99,
        block_hash: iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
            b"unproved decision carrier",
        )),
        block_time_ms: 1,
    });
    assert!(report.historical().is_none());
    assert!(report.current.is_none());
}

#[test]
fn approval_owner_holds_original_profile_lock_without_starting_runtime() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet(
        "approval-lock",
        &temporary.path().join("generation"),
        &ports,
    )
    .unwrap();
    let coordinator = ManagedReserveTopUpApproval::open(
        &prepared,
        crate::managed::native_operation::test_support::provider_id(&prepared, 0),
    )
    .unwrap();
    assert!(
        ManagedReserveTopUpApproval::open(
            &prepared,
            crate::managed::native_operation::test_support::provider_id(&prepared, 0)
        )
        .is_err()
    );
    let mut changed = prepared.clone();
    changed.context.dataspace_id = 7;
    assert!(
        ManagedReserveTopUpApproval::open(
            &changed,
            crate::managed::native_operation::test_support::provider_id(&prepared, 0)
        )
        .is_err()
    );
    changed = prepared.clone();
    changed.service_profile =
        crate::localnet::service_authorities::LocalnetServiceProfile::Standard;
    assert!(
        ManagedReserveTopUpApproval::open(
            &changed,
            crate::managed::native_operation::test_support::provider_id(&prepared, 0)
        )
        .is_err()
    );
    assert_eq!(
        coordinator.authority.directory.entries(2).unwrap(),
        [std::ffi::OsString::from("operation.lock")]
    );
    drop(coordinator);
    ManagedReserveTopUpApproval::open(
        &prepared,
        crate::managed::native_operation::test_support::provider_id(&prepared, 0),
    )
    .unwrap();
}

// Create a real committed request-only attempt through the sole wallet/epoch owner. This helper
// neither signs nor treats codec-only checkpoints as native evidence.
pub(super) fn retain_explicit_request(
    coordinator: &ManagedReserveTopUpApproval,
    directory: &PrivateDirectory,
    original: &Original,
    utc: u64,
    options: &BoundedTransactionOptions,
) -> Selected<Original> {
    journal::publish_intent(directory, original).unwrap();
    let account = AccountService::new(coordinator.authority.config.clone()).unwrap();
    journal::explicit(directory, original, utc, options, &account).unwrap();
    let selected = journal::required_original(directory).unwrap();
    assert_eq!(
        account
            .inspect_reserve_movement_decision_preparation(
                &selected.directory().path().join("transaction"),
                &selected.request(options.deadline),
            )
            .unwrap()
            .phase(),
        iroha_wallet::operations::NativePreparationPhase::RequestOnly,
    );
    selected
}
