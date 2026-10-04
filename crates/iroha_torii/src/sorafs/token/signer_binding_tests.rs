//! Move-only binding continuation, original lifetime and cleanup tests.

use super::*;
use std::cell::Cell;

struct Failure<'a> {
    graph: Vec<u8>,
    original: *const u8,
    deadline: Instant,
    original_deadline: Instant,
    remaining: usize,
    terminal: bool,
    attempts: &'a Cell<usize>,
    drops: &'a Cell<usize>,
}
impl CheckAttempt for Failure<'_> {
    fn deadline(&self) -> Instant {
        assert_eq!(self.deadline, self.original_deadline);
        self.deadline
    }
    fn retryable(&self) -> bool {
        !self.terminal
    }
}
impl BindingAttempt for Failure<'_> {
    type Output = usize;
    fn retry(mut self) -> Result<usize, Self> {
        assert_eq!(self.graph.as_ptr(), self.original);
        self.attempts.set(self.attempts.get() + 1);
        if self.remaining == 0 {
            Ok(self.graph.len())
        } else {
            self.remaining -= 1;
            Err(self)
        }
    }
}
impl Drop for Failure<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
fn failure<'a>(
    deadline: Instant,
    remaining: usize,
    terminal: bool,
    attempts: &'a Cell<usize>,
    drops: &'a Cell<usize>,
) -> Failure<'a> {
    let graph = vec![7; 256];
    Failure {
        original: graph.as_ptr(),
        graph,
        deadline,
        original_deadline: deadline,
        remaining,
        terminal,
        attempts,
        drops,
    }
}

#[test]
fn retries_move_the_same_graph_without_reentering_signing_or_renewing_time() {
    let start = Instant::now();
    let clock = Cell::new(start);
    let attempts = Cell::new(0);
    let drops = Cell::new(0);
    let signing_calls = Cell::new(0);
    let signed_result = {
        signing_calls.set(signing_calls.get() + 1);
        Err(failure(
            start + Duration::from_secs(1),
            3,
            false,
            &attempts,
            &drops,
        ))
    };
    assert_eq!(
        drive(
            signed_result,
            Failure::retry,
            || clock.get(),
            |delay| {
                assert!(!delay.is_zero() && delay <= Duration::from_millis(64));
                clock.set(clock.get() + delay);
            }
        )
        .unwrap(),
        256
    );
    assert_eq!(signing_calls.get(), 1);
    assert_eq!(attempts.get(), 4);
    assert_eq!(drops.get(), 1);
}

#[test]
fn terminal_and_expired_owners_never_retry_or_wait() {
    let now = Instant::now();
    for (deadline, terminal) in [(now, false), (now + Duration::from_secs(1), true)] {
        let attempts = Cell::new(0);
        let drops = Cell::new(0);
        let result = drive(
            Err(failure(deadline, usize::MAX, terminal, &attempts, &drops)),
            Failure::retry,
            || now,
            |_| panic!("terminal owner must not wait"),
        );
        assert!(matches!(
            result,
            Err(StreamTokenIssuerError::SignerFinalityUnavailable)
        ));
        assert_eq!(attempts.get(), 0);
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn waiting_is_capped_at_original_expiry_and_does_not_retry_after_it() {
    let start = Instant::now();
    let deadline = start + Duration::from_millis(9);
    let clock = Cell::new(start);
    let attempts = Cell::new(0);
    let drops = Cell::new(0);
    let result = drive(
        Err(failure(deadline, usize::MAX, false, &attempts, &drops)),
        Failure::retry,
        || clock.get(),
        |delay| {
            assert!(delay <= deadline.duration_since(clock.get()));
            clock.set(clock.get() + delay);
        },
    );
    assert!(matches!(
        result,
        Err(StreamTokenIssuerError::SignerFinalityUnavailable)
    ));
    assert_eq!(clock.get(), deadline);
    assert_eq!(attempts.get(), 1);
    assert_eq!(drops.get(), 1);
}

#[test]
fn wait_unwind_retires_exactly_one_original_owner() {
    let now = Instant::now();
    let attempts = Cell::new(0);
    let drops = Cell::new(0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        drive(
            Err(failure(
                now + Duration::from_secs(1),
                2,
                false,
                &attempts,
                &drops,
            )),
            Failure::retry,
            || now,
            |_| panic!("service unwind fixture"),
        )
    }));
    assert!(result.is_err());
    assert_eq!(attempts.get(), 0);
    assert_eq!(drops.get(), 1);
}

#[test]
fn finalized_verification_retries_the_original_state_owned_check_without_resigning() {
    use iroha_core::{
        execution_attempt::ExecutionAttemptError,
        query::stream_token_authority::{
            observation::{
                StreamTokenCheckExpectedV1, begin_stream_token_check_v1,
                capture_stream_token_authority_v1,
            },
            test_fixture::StreamTokenRuntimeTestFixtureV1 as Fixture,
        },
    };
    use iroha_data_model::{
        account::AccountId,
        sorafs::stream_token_authority::{StreamTokenCheckPhaseV1, StreamTokenReviewedV1},
    };
    use sorafs_manifest::signer::{
        protocol::{SignerOperationActionV1, SignerOperationCustodyV1, SignerOperationIntentV1},
        stream_token::{SignerStreamTokenRequestV1, stream_token_binding_digest_v1},
    };
    // The fixture policy begins one hour before its reference time; keep that bound nonzero.
    const NOW: u64 = 4_000_000;
    let mut fixture = Fixture::new_at(NOW - 5_000);
    let current =
        capture_stream_token_authority_v1(&fixture.state.view(), &fixture.policy.binding, [31; 32])
            .unwrap();
    let request = SignerStreamTokenRequestV1 {
        operation_id: [31; 32],
        binding_digest: stream_token_binding_digest_v1(&fixture.policy.binding).unwrap(),
        original_custody: SignerOperationCustodyV1 {
            record_digest: current.control.active_head.unwrap().record_digest,
            control_state_digest: current.anchor.state_digest,
        },
        signing_payload_digest: [32; 32],
        signing_payload_size: 256,
        issued_at_unix_ms: NOW - 1_000,
        expires_at_unix_ms: NOW + 60_000,
    };
    let reviewed = StreamTokenReviewedV1 {
        request,
        intent: SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: request.operation_id,
            request_digest: request.digest().unwrap(),
            previous_audit: current.head.audit,
        },
    };
    let prepared = begin_stream_token_check_v1(
        fixture.state.clone(),
        StreamTokenCheckExpectedV1 {
            binding: fixture.policy.binding.clone(),
            observer: AccountId::new(Fixture::key(3).public_key().clone()),
            expected_operator: current.operator,
            control_revision: current.control_revision,
            control_digest: current.anchor.state_digest,
            reviewed,
            phase: StreamTokenCheckPhaseV1::Current(current.head.audit),
            floor: current.floor,
        },
        Duration::from_secs(60),
    )
    .unwrap();
    let original_instruction = prepared.instruction().clone();
    let signed = fixture.chain().sign(
        &Fixture::key(3),
        [iroha_data_model::isi::InstructionBox::from(
            prepared.instruction().clone(),
        )],
        NOW,
    );
    let pending = prepared.bind_signed_transaction(signed.clone()).unwrap();
    let deadline = pending.deadline();
    assert!(fixture.commit_signed(signed.clone(), NOW + 1));
    let pool = fixture.state.ivm_execution_budget();
    let limit = pool.limit_bytes();
    let retained = pool.reserved_bytes();
    let samples = Cell::new(0);
    let attempt = |pending: PendingStreamTokenCheckV1| {
        pending.verify_finalized(|| {
            samples.set(samples.get() + 1);
            Ok(StreamTokenEligibilityTimeIntervalV1 {
                earliest_unix_ms: NOW + 2,
                latest_unix_ms: NOW + 2,
            })
        })
    };
    pool.set_limit_bytes(0);
    let failure = attempt(pending)
        .err()
        .expect("State-funded replay must refuse");
    let ExecutionAttemptError::Deferred(original) = failure.error() else {
        panic!("original local error");
    };
    assert!(matches!(
        original.allocation_refusal(),
        Some(iroha_allocation::AllocationRefusal::ExceedsLimit { limit_bytes: 0, .. })
    ));
    assert_eq!(samples.get(), 0);
    assert_eq!(pool.reserved_bytes(), retained);
    let mut waits = 0;
    let verified = drive(
        Err(failure),
        |failure| {
            assert_eq!(failure.signed_transaction(), &signed);
            assert_eq!(failure.deadline(), deadline);
            attempt(failure.into_pending())
        },
        Instant::now,
        |delay| {
            waits += 1;
            assert_eq!(delay, Duration::from_millis(4));
            assert_eq!(pool.reserved_bytes(), retained);
            pool.set_limit_bytes(limit);
        },
    )
    .unwrap();
    assert_eq!(waits, 1);
    assert_eq!(samples.get(), 1);
    assert_eq!(verified.instruction(), &original_instruction);
    drop(verified);
    assert!(pool.reserved_bytes() < retained);
}
