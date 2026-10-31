//! Counter, identity and lifecycle checks for the canonical source resource pair.
use super::*;
use crate::vega::zk_ams::mkhe::rns_native_profile::{
    ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1, ZK_AMS_MKHE_RNS_NATIVE_SPOOL_MAX_BYTES_V1,
    ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1, zk_ams_mkhe_rns_native_profile_v1,
    zk_ams_mkhe_rns_native_topology_v1,
};

fn layout() -> Layout {
    Layout::new(
        zk_ams_mkhe_rns_native_profile_v1().unwrap().profile_digest,
        zk_ams_mkhe_rns_native_topology_v1()
            .unwrap()
            .topology_digest,
        [1; 32],
        [2; 32],
        [3; 32],
    )
    .unwrap()
}

fn finish_schedule(source: &mut ZkAmsMkheRnsNativeSourceReservationV1) {
    for arena in [Arena::Main, Arena::Nonce] {
        for slot in 0..arena.slot_count() {
            source.charge_write_v1(arena, slot).unwrap();
        }
        source.charge_seal_v1(arena).unwrap();
    }
    source.require_sealed_v1().unwrap();
}

#[test]
fn source_admission_retains_preexisting_work_memory_and_original_storage_pair() {
    let mut resources = ZkAmsMkheRnsNativeSourceResourcesV1::new_v1();
    resources.proof.charge(71).unwrap();
    let previous_memory = resources.proof.reserve_workspace_v1(91, 0).unwrap();
    let previous_file = resources.storage.reserve_files_v1(123).unwrap();
    let mut source = resources.admit_source_v1(layout(), 61, 29).unwrap();
    assert_eq!(source.resources.proof.consumed().unwrap(), 71);
    assert_eq!(source.resources.proof.live_bytes().unwrap(), 91 + 61 + 29);
    let storage = source.resources.storage.usage_v1().unwrap();
    assert_eq!(
        storage.live_spool_bytes,
        123 + ZK_AMS_MKHE_RNS_NATIVE_SOURCE_TOTAL_FILE_BYTES_V1
    );
    assert_eq!(storage.reserved_io_bytes, 2 * storage.live_spool_bytes);
    source.release_construction_scratch_v1();
    assert_eq!(source.resources.proof.live_bytes().unwrap(), 91 + 61);
    drop(previous_memory);
    drop(previous_file);
    assert_eq!(source.resources.proof.live_bytes().unwrap(), 61);
    assert_eq!(
        source
            .resources
            .storage
            .usage_v1()
            .unwrap()
            .live_spool_bytes,
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_TOTAL_FILE_BYTES_V1
    );
}

#[test]
fn earlier_original_workspace_charge_prevents_source_admission() {
    let mut resources = ZkAmsMkheRnsNativeSourceResourcesV1::new_v1();
    let previous = resources
        .proof
        .reserve_workspace_v1(ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1 - 10, 0)
        .unwrap();
    assert!(matches!(
        resources.admit_source_v1(layout(), 11, 0),
        Err(Error::ResourceCeilingExceeded)
    ));
    // The original live allocation is still charged after the issuer is consumed.
    assert!(matches!(
        previous.reserve_child_workspace_v1(11, 0),
        Err(RnsNativeResourceErrorV1::WorkspaceLimit)
    ));
    assert!(previous.reserve_child_workspace_v1(10, 0).is_ok());
}

#[test]
fn earlier_ordered_spool_and_io_reservations_constrain_same_source_session() {
    for (spool_limit, io_limit) in [
        (
            ZK_AMS_MKHE_RNS_NATIVE_SOURCE_TOTAL_FILE_BYTES_V1 + 7,
            ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1,
        ),
        (
            ZK_AMS_MKHE_RNS_NATIVE_SPOOL_MAX_BYTES_V1,
            2 * ZK_AMS_MKHE_RNS_NATIVE_SOURCE_TOTAL_FILE_BYTES_V1 + 14,
        ),
    ] {
        let mut resources = ZkAmsMkheRnsNativeSourceResourcesV1 {
            proof: RnsNativeProofResourceBudgetV1::default(),
            storage: OrderedStorageSessionBudgetV1::with_test_limits_v1(spool_limit, io_limit),
        };
        let previous = resources.storage.reserve_files_v1(8).unwrap();
        let observer = resources.proof.reserve_workspace_v1(0, 0).unwrap();
        assert!(matches!(
            resources.admit_source_v1(layout(), 100, 10),
            Err(Error::ResourceCeilingExceeded)
        ));
        // Failed second admission releases only its temporary memory reservation.
        assert!(
            observer
                .reserve_child_workspace_v1(ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1, 0)
                .is_ok()
        );
        drop(previous);
    }
}

#[test]
fn attempted_io_before_source_begin_is_never_refunded_by_failed_admission() {
    let limit = 2 * ZK_AMS_MKHE_RNS_NATIVE_SOURCE_TOTAL_FILE_BYTES_V1 + 16;
    let mut resources = ZkAmsMkheRnsNativeSourceResourcesV1 {
        proof: RnsNativeProofResourceBudgetV1::default(),
        storage: OrderedStorageSessionBudgetV1::with_test_limits_v1(
            ZK_AMS_MKHE_RNS_NATIVE_SPOOL_MAX_BYTES_V1,
            limit,
        ),
    };
    let mut earlier = resources.storage.reserve_files_v1(8).unwrap();
    earlier.charge_reserved_io_v1(16).unwrap();
    earlier.charge_read_io_v1(1).unwrap();
    assert_eq!(resources.storage.usage_v1().unwrap().attempted_io_bytes, 17);
    assert!(matches!(
        resources.admit_source_v1(layout(), 0, 0),
        Err(Error::ResourceCeilingExceeded)
    ));
    // The earlier reservation keeps the same original ledger alive after the
    // failed owner is consumed. Its prior attempted bytes were not reset/refunded.
    earlier.charge_read_io_v1(limit - 17).unwrap();
    assert_eq!(
        earlier.charge_read_io_v1(1),
        Err(StorageBudgetErrorV1::IoLimit)
    );
}

#[test]
fn exact_write_seal_schedule_and_invalid_preflights_preserve_counters() {
    let mut source = ZkAmsMkheRnsNativeSourceResourcesV1::new_v1()
        .admit_source_v1(layout(), 0, 0)
        .unwrap();
    let before = source.resources.storage.usage_v1().unwrap();
    assert_eq!(
        source.charge_write_v1(Arena::Main, 1),
        Err(Error::UnexpectedWrite)
    );
    assert_eq!(source.charge_seal_v1(Arena::Main), Err(Error::Incomplete));
    assert_eq!(
        source.charge_read_v1(Arena::Main, 0),
        Err(Error::Incomplete)
    );
    assert_eq!(source.resources.storage.usage_v1().unwrap(), before);
    finish_schedule(&mut source);
    let complete = source.resources.storage.usage_v1().unwrap();
    assert_eq!(complete.attempted_io_bytes, 632_479_776);
    assert_eq!(complete.reserved_io_bytes, 0);
    assert_eq!(
        source.charge_write_v1(Arena::Main, 0),
        Err(Error::UnexpectedWrite)
    );
    assert_eq!(source.charge_seal_v1(Arena::Nonce), Err(Error::Incomplete));
    assert_eq!(
        source.charge_read_v1(Arena::Main, Arena::Main.slot_count()),
        Err(Error::UnexpectedWrite)
    );
    assert_eq!(source.resources.storage.usage_v1().unwrap(), complete);
    source.charge_read_v1(Arena::Main, 0).unwrap();
    source.charge_read_v1(Arena::Main, 0).unwrap();
    source.charge_read_v1(Arena::Nonce, 1).unwrap();
    assert_eq!(
        source
            .resources
            .storage
            .usage_v1()
            .unwrap()
            .attempted_io_bytes,
        632_479_776 + 2 * 8_208 + 48
    );
}

#[test]
fn chunks_retain_original_identity_and_live_bytes_after_parent_drop() {
    let source = ZkAmsMkheRnsNativeSourceResourcesV1::new_v1()
        .admit_source_v1(layout(), 64, 0)
        .unwrap();
    let other = ZkAmsMkheRnsNativeSourceResourcesV1::new_v1()
        .admit_source_v1(layout(), 64, 0)
        .unwrap();
    let mut chunk = source.reserve_workspace_v1(8_300, 99).unwrap();
    assert!(source.owns_workspace_v1(&chunk));
    assert!(!other.owns_workspace_v1(&chunk));
    assert_eq!(
        source.resources.proof.live_bytes().unwrap(),
        64 + 8_300 + 99
    );
    chunk.release_scratch_v1();
    let observer = source.memory.reserve_child_workspace_v1(0, 0).unwrap();
    drop(source);
    assert!(matches!(
        observer
            .reserve_child_workspace_v1(ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1 - 8_299, 0),
        Err(RnsNativeResourceErrorV1::WorkspaceLimit)
    ));
    assert!(
        observer
            .reserve_child_workspace_v1(ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1 - 8_300, 0)
            .is_ok()
    );
    drop(chunk);
    assert!(
        observer
            .reserve_child_workspace_v1(ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1, 0)
            .is_ok()
    );
}

#[test]
fn earlier_original_work_and_attempted_io_survive_workspace_and_io_refusals() {
    let mut resources = ZkAmsMkheRnsNativeSourceResourcesV1::new_v1();
    resources.proof.charge(917).unwrap();
    resources.storage = OrderedStorageSessionBudgetV1::with_test_limits_v1(
        ZK_AMS_MKHE_RNS_NATIVE_SPOOL_MAX_BYTES_V1,
        632_479_776 + 8_208,
    );
    let mut source = resources.admit_source_v1(layout(), 100, 0).unwrap();
    finish_schedule(&mut source);
    let holding = source
        .reserve_workspace_v1(ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1 - 100, 0)
        .unwrap();
    let before = source.resources.storage.usage_v1().unwrap();
    assert!(matches!(
        source.reserve_workspace_v1(1, 0),
        Err(Error::ResourceCeilingExceeded)
    ));
    assert_eq!(source.resources.storage.usage_v1().unwrap(), before);
    assert_eq!(source.resources.proof.consumed().unwrap(), 917);
    drop(holding);
    source.charge_read_v1(Arena::Main, 0).unwrap();
    let attempted = source.resources.storage.usage_v1().unwrap();
    assert_eq!(
        source.charge_read_v1(Arena::Main, 0),
        Err(Error::ResourceCeilingExceeded)
    );
    assert_eq!(source.resources.storage.usage_v1().unwrap(), attempted);
    assert_eq!(source.resources.proof.consumed().unwrap(), 917);
}

#[test]
fn ordered_sibling_contention_uses_the_same_storage_arc() {
    let mut source = ZkAmsMkheRnsNativeSourceResourcesV1::new_v1()
        .admit_source_v1(layout(), 0, 0)
        .unwrap();
    finish_schedule(&mut source);
    let sibling = source.files.reserve_sibling_file_v1(1234).unwrap();
    let usage = source.resources.storage.usage_v1().unwrap();
    assert_eq!(
        usage.live_spool_bytes,
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_TOTAL_FILE_BYTES_V1 + 1234
    );
    assert_eq!(usage.reserved_io_bytes, 2468);
    source.charge_read_v1(Arena::Nonce, 0).unwrap();
    drop(sibling);
    assert_eq!(
        source
            .resources
            .storage
            .usage_v1()
            .unwrap()
            .live_spool_bytes,
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_TOTAL_FILE_BYTES_V1
    );
    assert_eq!(
        source
            .resources
            .storage
            .usage_v1()
            .unwrap()
            .attempted_io_bytes,
        632_479_776 + 48
    );
}

#[test]
fn unwind_releases_only_held_workspace_and_files_after_prior_attempts() {
    let mut source = ZkAmsMkheRnsNativeSourceResourcesV1::new_v1()
        .admit_source_v1(layout(), 0, 0)
        .unwrap();
    finish_schedule(&mut source);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _chunk = source.reserve_workspace_v1(99, 11).unwrap();
        source.charge_read_v1(Arena::Nonce, 0).unwrap();
        panic!("test source operation unwind");
    }));
    assert!(outcome.is_err());
    assert_eq!(source.resources.proof.live_bytes().unwrap(), 0);
    assert_eq!(
        source
            .resources
            .storage
            .usage_v1()
            .unwrap()
            .attempted_io_bytes,
        632_479_776 + 48
    );
}

#[test]
fn error_mapping_is_path_free_and_preserves_poison_distinction() {
    for error in [
        RnsNativeResourceErrorV1::ArithmeticOverflow,
        RnsNativeResourceErrorV1::WorkLimit,
        RnsNativeResourceErrorV1::WorkspaceLimit,
    ] {
        assert_eq!(map_memory_error_v1(error), Error::ResourceCeilingExceeded);
    }
    assert_eq!(
        map_memory_error_v1(RnsNativeResourceErrorV1::LedgerPoisoned),
        Error::Poisoned
    );
    for error in [
        StorageBudgetErrorV1::Overflow,
        StorageBudgetErrorV1::SpoolLimit,
        StorageBudgetErrorV1::IoLimit,
    ] {
        assert_eq!(map_storage_error_v1(error), Error::ResourceCeilingExceeded);
    }
    assert_eq!(
        map_storage_error_v1(StorageBudgetErrorV1::Poisoned),
        Error::Poisoned
    );
    assert_eq!(
        map_storage_error_v1(StorageBudgetErrorV1::Order),
        Error::Incomplete
    );
}
