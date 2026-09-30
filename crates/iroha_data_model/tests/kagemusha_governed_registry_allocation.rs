//! Allocation checks for the Kagemusha verifier authority validated at State commit.
// This isolated integration test uses GlobalAlloc solely to observe heap traffic.
#![allow(unsafe_code)]

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_RELEASE_ACTIVE_V1, KAGEMUSHA_RELEASE_STANDBY_V1, KAGEMUSHA_WIRE_VERSION_V1,
    KagemushaGovernedVerifierRegistryV1, KagemushaGovernedVerifierReleaseV1,
    KagemushaReleaseAuthorityPolicyV1,
};

thread_local! {
    static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn record_allocation() {
    let _ = TRACK_ALLOCATIONS.try_with(|tracking| {
        if tracking.get() {
            let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
    });
}

// SAFETY: every allocation operation delegates to System with its original layout.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_allocation();
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_allocation();
        }
        pointer
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            record_allocation();
        }
        result
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }
}

fn allocations_during(f: impl FnOnce()) -> usize {
    struct StopTracking;
    impl Drop for StopTracking {
        fn drop(&mut self) {
            TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
        }
    }
    ALLOCATION_COUNT.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
    let stop = StopTracking;
    f();
    drop(stop);
    ALLOCATION_COUNT.with(Cell::get)
}

#[test]
fn valid_multi_release_registry_validation_uses_no_heap() {
    let signer = KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519).unwrap();
    let policy = KagemushaReleaseAuthorityPolicyV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        authority_set_id: [1; 32],
        threshold: 1,
        authorized_signers: vec![signer.public_key().clone()],
    };
    let policy_digest = policy.canonical_digest().unwrap();
    let release = |id: u8, status| KagemushaGovernedVerifierReleaseV1 {
        release_id: [id; 32],
        status,
        profile_digest: [2; 32],
        artifact_manifest_digest: [3; 32],
        receipt_digest: [4; 32],
        attestation_digest: [5; 32],
        authority_policy_digest: policy_digest,
        hardware_policy_digest: [6; 32],
        native_profile_digest: [7; 32],
        provider_policy_root: [8; 32],
        suite_id: [9; 32],
        vk_set_digest: [10; 32],
    };
    let registry = KagemushaGovernedVerifierRegistryV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        authority_policy: Some(policy),
        active_release_id: Some([1; 32]),
        releases: vec![
            release(1, KAGEMUSHA_RELEASE_ACTIVE_V1),
            release(2, KAGEMUSHA_RELEASE_STANDBY_V1),
        ],
    };
    let frozen = registry.clone();
    assert_eq!(
        allocations_during(|| {
            assert_eq!(
                registry
                    .authority_policy
                    .as_ref()
                    .unwrap()
                    .canonical_digest(),
                Ok(policy_digest)
            );
            registry.validate().unwrap();
            assert_eq!(registry, frozen);
        }),
        0
    );
}
