//! Actual codec refusal, unchanged one-use expectation and exact fallible destinations.

use super::*;
use norito::core::{
    DecodeAttemptErrorKind, DecodeLimits, DecodeResourceError, with_decode_limits_measured,
    with_decode_limits_scope,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

// There is no other global allocator in this crate. Only this thread's explicitly
// armed, exact output layout can fail once; other tests and native work use System.
// Callers below arm only known fallible output destinations, never a whole verifier
// with unrelated infallible receipt/custody allocations.
struct DestinationAllocator;
thread_local! {
    static REFUSE_LAYOUT: Cell<Option<usize>> = const { Cell::new(None) };
    static REFUSED: Cell<Option<usize>> = const { Cell::new(None) };
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}
fn refuse(bytes: usize) -> bool {
    let _ = ALLOCATIONS.try_with(|count| {
        if let Some(previous) = count.get() {
            count.set(Some(previous + 1));
        }
    });
    REFUSE_LAYOUT
        .try_with(|slot| {
            if slot.get() == Some(bytes) {
                slot.set(None);
                REFUSED.with(|observed| observed.set(Some(bytes)));
                true
            } else {
                false
            }
        })
        .unwrap_or(false)
}
// SAFETY: all accepted operations retain System's pointer/layout contract. A
// denied allocation returns null without taking ownership or changing an input.
unsafe impl GlobalAlloc for DestinationAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if refuse(layout.size()) {
            std::ptr::null_mut()
        } else {
            // SAFETY: the caller supplied the GlobalAlloc layout contract.
            unsafe { System.alloc(layout) }
        }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if refuse(layout.size()) {
            std::ptr::null_mut()
        } else {
            // SAFETY: the caller supplied the GlobalAlloc layout contract.
            unsafe { System.alloc_zeroed(layout) }
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: every non-null allocation was returned unchanged from System.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if refuse(new_size) {
            std::ptr::null_mut()
        } else {
            // SAFETY: the original pointer/layout and requested size come from GlobalAlloc.
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }
}
#[global_allocator]
static ALLOCATOR: DestinationAllocator = DestinationAllocator;

struct RefusalGuard;
impl Drop for RefusalGuard {
    fn drop(&mut self) {
        REFUSE_LAYOUT.with(|slot| slot.set(None));
    }
}
fn refuse_output<T>(bytes: usize, action: impl FnOnce() -> T) -> T {
    assert!(bytes > 0);
    REFUSED.with(|slot| slot.set(None));
    REFUSE_LAYOUT.with(|slot| {
        assert!(slot.get().is_none(), "no nested allocation probe");
        slot.set(Some(bytes));
    });
    let guard = RefusalGuard;
    let result = action();
    drop(guard);
    assert_eq!(
        REFUSED.with(Cell::get),
        Some(bytes),
        "actual output allocation refused"
    );
    result
}
fn allocation_limit(bytes: usize) -> DecodeLimits {
    DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
}

#[test]
fn bounded_output_and_domain_allocation_errors_keep_original_inputs_and_retry() {
    for phase in [Phase::Startup, Phase::AfterCommit, Phase::BeforeRelease] {
        let evidence = Evidence::new(phase);
        let expected_request = evidence.request.encode_canonical().unwrap();
        let expected_state = evidence.state.encode_canonical().unwrap();
        let expected_payload = observer_payload(&evidence.state.body);
        for (request, bytes) in [
            (true, expected_request.len()),
            (false, expected_state.len()),
        ] {
            let error = refuse_output(bytes, || {
                if request {
                    evidence.request.encode_canonical()
                } else {
                    evidence.state.encode_canonical()
                }
            })
            .unwrap_err();
            assert!(error.is_retryable());
            assert_eq!(error.rejection(), None);
            assert!(matches!(error,
                SignerStreamTokenEvidenceAdmissionErrorV1::Encoding(
                    norito::core::BoundedEncodeError::AllocationFailed { bytes: actual }
                ) if actual == bytes
            ));
        }
        let error = refuse_output(expected_payload.len(), || {
            evidence.state.body.signing_payload()
        })
        .unwrap_err();
        assert!(error.is_retryable());
        assert_eq!(error.rejection(), None);
        assert!(matches!(
            error,
            SignerStreamTokenEvidenceAdmissionErrorV1::Allocation(_)
        ));
        assert_eq!(
            evidence.request.encode_canonical().unwrap(),
            expected_request
        );
        assert_eq!(evidence.state.encode_canonical().unwrap(), expected_state);
        assert_eq!(
            evidence.state.body.signing_payload().unwrap(),
            expected_payload
        );
        assert_eq!(evidence.attempt.request(), &evidence.request);
    }
}

#[test]
fn actual_decoder_refusal_retains_the_same_expected_and_reply_after_scope_retires() {
    for phase in [Phase::Startup, Phase::AfterCommit, Phase::BeforeRelease] {
        let mut evidence = Evidence::new(phase);
        let observation = evidence.observation_bytes();
        let receipt = evidence.receipt_bytes.clone();
        let request = evidence.attempt.request_bytes().unwrap();
        let owner = std::ptr::from_ref(&evidence.attempt);
        // Completed verification first authenticates the original receipt shape.
        // Fund precisely that real prefix, so this test reaches the evidence
        // decoder instead of the separately open receipt codec projection.
        let (_, prefix) = with_decode_limits_measured(allocation_limit(usize::MAX), || {
            if !is_current(phase) {
                completed_request_subject(
                    &evidence.receipt_bytes,
                    &evidence.receipt.token,
                    &evidence.receipt.expected,
                    &evidence.receipt.binding,
                )
                .unwrap();
            }
        });
        let limit = prefix.total_allocated_bytes();
        let failure = with_decode_limits_scope(allocation_limit(limit), || {
            evidence.verify_bytes(&observation)
        })
        .err()
        .expect("original enclosing allowance refuses evidence decode");
        assert!(failure.is_retryable());
        assert_eq!(failure.rejection(), None);
        assert_eq!(evidence.attempt.request(), &evidence.request);
        assert_eq!(std::ptr::from_ref(&evidence.attempt), owner);
        let SignerStreamTokenEvidenceAdmissionErrorV1::Codec(original) = failure else {
            panic!("the real evidence decoder must own this refusal")
        };
        assert_eq!(original.kind(), DecodeAttemptErrorKind::EnclosingLimit);
        let source = original.into_error();
        assert!(matches!(source.decode_resource_error(),
            Some(DecodeResourceError::TotalAllocationExceeded { attempted, limit: actual })
                if actual == limit as u64 && attempted > actual
        ));
        assert_eq!(evidence.attempt.request_bytes().unwrap(), request);
        assert!(!evidence.attempt.is_retired());
        // No owner move, constructor, observer, new challenge or signature occurs on retry.
        evidence
            .verify_bytes(&observation)
            .expect("same original Expected and reply retry");
        assert!(evidence.attempt.is_retired());
        assert_eq!(std::ptr::from_ref(&evidence.attempt), owner);
        assert_eq!(evidence.receipt_bytes, receipt);
        assert_eq!(evidence.observation_bytes(), observation);
    }
}

#[test]
fn protocol_and_malformed_document_rejections_do_not_gain_local_provenance() {
    let evidence = Evidence::new(Phase::BeforeRelease);
    let observation = evidence.observation_bytes();
    let original = observation.clone();
    let error = with_decode_limits_scope(allocation_limit(0), || {
        decode_document::<SignerStreamTokenStateObservationV1>(&observation, observation.len() - 1)
    })
    .unwrap_err();
    assert!(!error.is_retryable());
    assert_eq!(error.rejection(), Some(EvidenceError::InvalidDocument));
    assert!(matches!(
        error,
        SignerStreamTokenEvidenceAdmissionErrorV1::Rejected(_)
    ));
    let mut malformed = observation.clone();
    *malformed.last_mut().unwrap() ^= 1;
    let error = with_decode_limits_scope(allocation_limit(0), || {
        SignerStreamTokenStateObservationV1::decode_canonical(&malformed)
    })
    .unwrap_err();
    assert!(!error.is_retryable());
    assert_eq!(error.rejection(), Some(EvidenceError::InvalidDocument));
    let SignerStreamTokenEvidenceAdmissionErrorV1::Codec(original_error) = error else {
        panic!("original malformed-frame verdict")
    };
    assert_eq!(original_error.kind(), DecodeAttemptErrorKind::Invalid);
    assert_eq!(observation, original);
}

#[test]
fn real_frame_ceiling_stays_semantic_and_ambient_layout_restores_on_refusal() {
    let evidence = Evidence::new(Phase::BeforeRelease);
    let canonical = evidence.state.encode_canonical().unwrap();
    for flags in receipt_fixture::layouts() {
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        let error = encode_document(&evidence.state, canonical.len() - 1).unwrap_err();
        assert!(!error.is_retryable());
        assert_eq!(error.rejection(), Some(EvidenceError::InvalidDocument));
        assert!(
            matches!(error, SignerStreamTokenEvidenceAdmissionErrorV1::Encoding(
            norito::core::BoundedEncodeError::FrameTooLarge { encoded_bytes, max_bytes }
        ) if encoded_bytes == canonical.len() && max_bytes + 1 == encoded_bytes)
        );
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));
        assert_eq!(evidence.state.encode_canonical().unwrap(), canonical);
    }
}

fn allocations_during<T>(action: impl FnOnce() -> T) -> (T, usize) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ALLOCATIONS.with(|count| count.set(None));
        }
    }
    ALLOCATIONS.with(|count| {
        assert!(count.get().is_none(), "no nested allocation measurement");
        count.set(Some(0));
    });
    let _reset = Reset;
    let output = action();
    let count = ALLOCATIONS.with(|count| count.get().unwrap());
    (output, count)
}

#[test]
fn public_success_and_completed_rejection_permanently_retire_each_phase_in_place() {
    for phase in phases() {
        for success in [false, true] {
            let mut evidence = Evidence::new(phase);
            let bytes = evidence.observation_bytes();
            let address = std::ptr::from_ref(&evidence.attempt);
            if success {
                evidence.verify_bytes(&bytes).unwrap();
            } else {
                let error = evidence.verify_bytes(&[0]).err().unwrap();
                assert!(!error.is_retryable());
            }
            assert!(evidence.attempt.is_retired());
            assert_eq!(std::ptr::from_ref(&evidence.attempt), address);
            // A retired owner is refused before any codec or output allocation,
            // even when the caller now offers the original valid signed reply.
            let (result, allocations) = allocations_during(|| evidence.verify_bytes(&bytes));
            assert_eq!(allocations, 0);
            assert_eq!(
                result.err().unwrap().rejection(),
                Some(EvidenceError::SourceMismatch)
            );
            assert!(evidence.attempt.is_retired());
            assert_eq!(std::ptr::from_ref(&evidence.attempt), address);
        }
    }
}

#[test]
fn original_local_refusal_and_terminal_transition_bookkeeping_allocate_nothing() {
    let mut evidence = Evidence::new(Phase::Startup);
    let bytes = evidence.observation_bytes();
    let original = with_decode_limits_scope(allocation_limit(0), || evidence.verify_bytes(&bytes))
        .err()
        .unwrap();
    assert!(original.is_retryable());
    assert!(!evidence.attempt.is_retired());
    let address = std::ptr::from_ref(&evidence.attempt);
    // Move the actual original codec cause through the private phase boundary;
    // constructing a wire-shaped error cannot produce this retry admission.
    let (refused, allocations) =
        allocations_during(|| evidence.attempt.verify::<()>(|_| Err(original)));
    assert_eq!(allocations, 0);
    assert!(refused.err().unwrap().is_retryable());
    assert!(!evidence.attempt.is_retired());
    assert_eq!(std::ptr::from_ref(&evidence.attempt), address);
    let (completed, allocations) = allocations_during(|| evidence.attempt.verify(|_| Ok(())));
    assert_eq!(allocations, 0);
    completed.unwrap();
    assert!(evidence.attempt.is_retired());
    assert_eq!(std::ptr::from_ref(&evidence.attempt), address);
}

#[test]
fn verification_unwind_retires_the_original_before_checker_entry() {
    for phase in phases() {
        let mut evidence = Evidence::new(phase);
        let bytes = evidence.observation_bytes();
        let request = evidence.attempt.request_bytes().unwrap();
        let address = std::ptr::from_ref(&evidence.attempt);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            evidence.attempt.verify::<()>(|original| {
                assert!(original.is_retired());
                panic!("verification abort after retiring the original attempt");
            })
        }));
        assert!(unwind.is_err());
        assert!(evidence.attempt.is_retired());
        assert_eq!(evidence.attempt.request_bytes().unwrap(), request);
        assert_eq!(std::ptr::from_ref(&evidence.attempt), address);
        let (result, allocations) = allocations_during(|| evidence.verify_bytes(&bytes));
        assert_eq!(allocations, 0);
        assert_eq!(
            result.err().unwrap().rejection(),
            Some(EvidenceError::SourceMismatch)
        );
    }
}
