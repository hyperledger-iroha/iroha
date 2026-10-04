//! V1 subject vectors, caller work refusal, and physical allocation observations.
// This separate test binary observes the production derivation through GlobalAlloc.
#![allow(unsafe_code)]

use iroha_crypto::{Algorithm, Hash, PublicKey};
use iroha_data_model::smart_contract::ContractAddress;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    convert::Infallible,
    hint::black_box,
};

const KNOWN_ADDRESS: &str = "irohac1qyqqqqqqqqqqqqpze5aq5vfxha4qlvu4q80e0ff4yesw50c37z96q";
// Existing contract_address_subject_consensus_vector, independent of the shared helper.
const KNOWN_SUBJECT: &str = "c19d0326bf14cb44e4e11d5c561f5f69367c305e2bc3ee29086b49aa07df3a55";
const V1_TAG: &[u8] = b"iroha:contract-subject:hash-to-point:v1:";

thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static REQUESTS: Cell<usize> = const { Cell::new(0) };
}

struct TrackingAllocator;
#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn record_request() {
    if TRACKING.try_with(Cell::get).unwrap_or(false) {
        let _ = REQUESTS.try_with(|requests| requests.set(requests.get() + 1));
    }
}

// SAFETY: every operation forwards the original pointer and layout to System.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_request();
        // SAFETY: the caller's allocation contract is passed through unchanged.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_request();
        // SAFETY: the caller's allocation contract is passed through unchanged.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_request();
        // SAFETY: the live System allocation and requested size are forwarded.
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pointer and layout are from the matching System allocation.
        unsafe { System.dealloc(pointer, layout) }
    }
}

fn measured<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    struct StopTracking;
    impl Drop for StopTracking {
        fn drop(&mut self) {
            TRACKING.with(|tracking| tracking.set(false));
        }
    }
    assert!(!TRACKING.with(Cell::get), "nested measurement");
    REQUESTS.with(|requests| requests.set(0));
    TRACKING.with(|tracking| tracking.set(true));
    let stop = StopTracking;
    let result = operation();
    drop(stop);
    (result, REQUESTS.with(Cell::get))
}

fn pinned_address() -> ContractAddress {
    KNOWN_ADDRESS.parse().expect("existing V1 address vector")
}

// Independent control retaining the pre-refactor V1 loop and owning strict parser.
// This test oracle deliberately does not invoke the new candidate or bytes APIs.
fn reference_subject(address: &ContractAddress) -> ([u8; 32], u32) {
    let mut counter = 0_u32;
    loop {
        let counter_bytes = counter.to_be_bytes();
        let candidate =
            Hash::new_from_chunks(&[V1_TAG, address.as_str().as_bytes(), &counter_bytes]);
        if PublicKey::from_bytes(Algorithm::Ed25519, candidate.as_ref()).is_ok() {
            return (candidate.into(), counter);
        }
        counter = counter.checked_add(1).expect("V1 reference counter");
    }
}

fn derive(address: &ContractAddress) -> [u8; 32] {
    address
        .try_subject_key_bytes(|_| Ok::<(), Infallible>(()))
        .expect("infallible admission")
}

#[test]
fn bytes_and_owned_subject_match_existing_v1_vector_and_reference() {
    let address = pinned_address();
    let (reference, counter) = reference_subject(&address);
    assert_eq!(hex::encode(reference), KNOWN_SUBJECT);
    assert_eq!(counter, 8, "the existing vector rejects eight candidates");
    let expected_hash_bytes = V1_TAG.len() + KNOWN_ADDRESS.len() + 4;
    let mut attempts = 0;
    let actual = address
        .try_subject_key_bytes(|hash_bytes| {
            assert_eq!(hash_bytes, expected_hash_bytes);
            attempts += 1;
            Ok::<(), Infallible>(())
        })
        .expect("admitted vector");
    assert_eq!(attempts, 9);
    assert_eq!(actual, reference);
    assert_eq!(hex::encode(actual), KNOWN_SUBJECT);
    assert_eq!(
        address.subject_id().expect_single_signatory().to_bytes(),
        (Algorithm::Ed25519, actual.as_slice())
    );
}

#[derive(Debug, PartialEq, Eq)]
struct WorkRefusal {
    permitted_attempts: usize,
    requested_hash_bytes: usize,
}

#[test]
fn admission_refuses_first_and_later_attempts_and_same_input_retries() {
    let address = pinned_address();
    let expected = hex::decode(KNOWN_SUBJECT).expect("known bytes");
    for permitted_attempts in [0, 1, 8] {
        let mut admitted = 0;
        let mut calls = 0;
        let refusal = address
            .try_subject_key_bytes(|requested_hash_bytes| {
                calls += 1;
                if admitted == permitted_attempts {
                    return Err(WorkRefusal {
                        permitted_attempts,
                        requested_hash_bytes,
                    });
                }
                admitted += 1;
                Ok(())
            })
            .expect_err("local work refusal before the known successful candidate");
        assert_eq!(admitted, permitted_attempts);
        assert_eq!(calls, permitted_attempts + 1);
        assert_eq!(
            refusal,
            WorkRefusal {
                permitted_attempts,
                requested_hash_bytes: V1_TAG.len() + KNOWN_ADDRESS.len() + 4
            }
        );
        // A failed local attempt cannot change the address or the derivation's initial counter.
        let mut retry_attempts = 0;
        let retried = address
            .try_subject_key_bytes(|_| {
                retry_attempts += 1;
                if retry_attempts > 9 {
                    return Err("retry exceeded the known V1 work");
                }
                Ok(())
            })
            .expect("same original input succeeds with sufficient work");
        assert_eq!(retry_attempts, 9);
        assert_eq!(retried.as_slice(), expected.as_slice());
        assert_eq!(address.as_str(), KNOWN_ADDRESS);
    }
}

#[test]
fn candidate_sequence_matches_reference_for_distinct_valid_addresses() {
    let (hrp, mut payload) = bech32::decode(KNOWN_ADDRESS).expect("V1 address payload");
    for seed in 0_u8..32 {
        payload[9] = seed;
        let literal = bech32::encode::<bech32::Bech32m>(hrp, &payload).expect("V1 literal");
        let address: ContractAddress = literal.parse().expect("same V1 address format");
        assert_eq!(address.as_str(), literal);
        let (reference, counter) = reference_subject(&address);
        let mut attempts = 0_u32;
        let actual = address
            .try_subject_key_bytes(|hash_bytes| {
                assert_eq!(hash_bytes, V1_TAG.len() + literal.len() + 4);
                attempts += 1;
                Ok::<(), Infallible>(())
            })
            .expect("admitted corpus address");
        assert_eq!(attempts, counter + 1);
        assert_eq!(actual, reference);
        assert_eq!(
            address.subject_id().expect_single_signatory().to_bytes().1,
            actual.as_slice()
        );
    }
}

#[test]
fn cold_bytes_derivation_and_work_refusal_have_no_heap_allocation() {
    std::thread::spawn(|| {
        // Address ownership is acquired before observation. No public-key parser has run on
        // this thread; the crypto unit test separately observes actual cache consultation.
        let address = pinned_address();
        let (actual, count) = measured(|| derive(black_box(&address)));
        assert_eq!(
            count, 0,
            "cold bytes derivation must allocate no heap storage"
        );
        assert_eq!(hex::encode(actual), KNOWN_SUBJECT);
        for permitted_attempts in [0, 1, 8] {
            let mut remaining = permitted_attempts;
            let (result, count) = measured(|| {
                address.try_subject_key_bytes(|_| {
                    if remaining == 0 {
                        return Err(permitted_attempts);
                    }
                    remaining -= 1;
                    Ok(())
                })
            });
            assert_eq!(result, Err(permitted_attempts));
            assert_eq!(
                count, 0,
                "neither invalid candidates nor refusal may allocate"
            );
        }
        let (again, count) = measured(|| derive(black_box(&address)));
        assert_eq!(count, 0);
        assert_eq!(again, actual);
        // Final account ownership is a separate, real allocation, not part of the bytes claim.
        let (owned, cold_count) = measured(|| black_box(address.subject_id()));
        assert!(
            cold_count > 0,
            "the final owner must remain visible to the allocator"
        );
        assert_eq!(
            owned.expect_single_signatory().to_bytes().1,
            actual.as_slice()
        );
        let (warm_owned, count) = measured(|| black_box(address.subject_id()));
        assert!(
            count > 0,
            "even a populated parse cache cannot fund final ownership"
        );
        assert_eq!(warm_owned, owned);
        assert!(
            cold_count > count,
            "ordinary cached parsing must retain its first-call setup"
        );
    })
    .join()
    .expect("cold allocation observation thread");
}

#[test]
fn allocator_observer_detects_real_backing_and_retires_after_unwind() {
    let (backing, count) = measured(|| black_box(vec![0x5a_u8; 73]));
    assert!(count > 0, "physical allocation must be visible");
    drop(backing);
    let panic = std::panic::catch_unwind(|| measured(|| panic!("allocation observer unwind")));
    assert!(panic.is_err());
    assert!(!TRACKING.with(Cell::get));
    assert_eq!(measured(|| ()).1, 0);
}
