//! Exact replay, crash recovery, and lease-boundary tests.
use super::*;
use iroha_data_model::sorafs::reputation::{
    PorTerminalOutcomeV1, ReputationJournalEventIdV1, StreamTokenRequestRouteV1,
    StreamTokenValidationBindingV1,
};
use sorafs_node::reputation::runtime::{ReputationJournalEnqueueOutcomeV1, ReputationRuntimeError};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
const VALIDATED_AT_MS: u64 = 1_800_000_000_000;
const HANDLE: &str = "sealed://sorafs/stream-admission/eu-1";
fn qualification() -> StreamTokenGatewayAdmissionQualificationV1 {
    StreamTokenGatewayAdmissionQualificationV1 {
        gateway_id: [0x31; 32],
        revision: 7,
        policy_digest: [0x32; 32],
        max_pending: 64,
        max_tracked_tokens: 64,
        lease_ttl_ms: 120_000,
    }
}
fn request(
    nonce: &str,
    validated_at_unix_ms: u64,
    expires_at_epoch: u64,
    max_streams: u16,
) -> StreamTokenGatewayAdmissionRequestV1 {
    StreamTokenGatewayAdmissionRequestV1 {
        context: StreamTokenValidationRequestContextV1::try_new(
            ProviderId::new([0x41; 32]),
            [0x42; 32],
            sorafs_manifest::canonical_manifest_root_cid([0x43; 32]),
            "sorafs.sf1@1.0.0".to_owned(),
            nonce,
            Some(b"Q2Fub25pY2FsVG9rZW4="),
            StreamTokenRequestRouteV1::car_range(64, 1_023).expect("canonical route"),
        )
        .expect("canonical request context"),
        token_body_digest: Some([0x44; 32]),
        token_key_version: Some(3),
        validated_at_unix_ms,
        status: StreamTokenValidationStatusV1::Accepted,
        quota: Some(StreamTokenGatewayQuotaRequestV1 {
            token_id: "11".repeat(16),
            max_streams,
            requests_per_minute: 120,
            rate_limit_bytes: 1_048_576,
            requested_bytes: 960,
            expires_at_epoch,
            observed_at_epoch: validated_at_unix_ms / 1_000,
        }),
    }
}
fn record_for_request(
    request: &StreamTokenGatewayAdmissionRequestV1,
    sequence: u64,
    status: StreamTokenValidationStatusV1,
) -> StreamTokenGatewayAdmissionRecordV1 {
    let admitted = status == StreamTokenValidationStatusV1::Accepted;
    let token_expiry = admitted.then(|| {
        request
            .quota
            .as_ref()
            .expect("accepted request quota")
            .expires_at_epoch
    });
    StreamTokenGatewayAdmissionRecordV1 {
        provider_id: request.context.provider_id(),
        outcome: StreamTokenValidationOutcomeV1 {
            binding: StreamTokenValidationBindingV1 {
                gateway_id: qualification().gateway_id,
                gateway_sequence: sequence,
                request_context_digest: request.context.digest().expect("request digest"),
            },
            token_body_digest: request.token_body_digest,
            token_key_version: request.token_key_version,
            validated_at_unix_ms: request.validated_at_unix_ms,
            status,
        },
        retry_after_secs: None,
        lease_id: admitted.then(|| [u8::try_from(sequence).expect("test sequence"); 32]),
        lease_expires_at_unix_ms: token_expiry.map(|expires| {
            exact_lease_expiry_unix_ms(
                request.validated_at_unix_ms,
                expires,
                qualification().lease_ttl_ms,
            )
            .expect("canonical lease expiry")
        }),
        lease_token_expires_at_epoch: token_expiry,
    }
}
#[derive(Debug, Default)]
struct ReputationProbe {
    calls: Mutex<Vec<(ProviderId, StreamTokenValidationOutcomeV1)>>,
    fail_next: AtomicUsize,
}
impl ReputationProbe {
    fn fail_once(&self) {
        self.fail_next.store(1, Ordering::Release);
    }
    fn calls(&self) -> Vec<(ProviderId, StreamTokenValidationOutcomeV1)> {
        self.calls.lock().expect("reputation calls").clone()
    }
}
impl ReputationNativeOutcomeAdmissionApiV1 for ReputationProbe {
    fn activation_state(
        &self,
    ) -> Result<ReputationNativeOutcomeAdmissionStateV1, ReputationRuntimeError> {
        Ok(ReputationNativeOutcomeAdmissionStateV1::Active)
    }
    fn record_por_terminal(
        &self,
        _provider_id: ProviderId,
        _outcome: PorTerminalOutcomeV1,
    ) -> Result<ReputationJournalEnqueueOutcomeV1, ReputationRuntimeError> {
        Err(ReputationRuntimeError::InvalidRuntimePolicy)
    }
    fn record_authenticated_stream_token_validation(
        &self,
        provider_id: ProviderId,
        outcome: StreamTokenValidationOutcomeV1,
    ) -> Result<StreamTokenReputationAdmissionOutcomeV1, ReputationRuntimeError> {
        if self
            .fail_next
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok()
        {
            return Err(ReputationRuntimeError::RuntimeBindingChanged);
        }
        let mut calls = self.calls.lock().expect("reputation calls");
        let replay = calls
            .iter()
            .any(|(seen_provider, seen)| *seen_provider == provider_id && *seen == outcome);
        calls.push((provider_id, outcome));
        if !outcome.status.counts_for_provider() {
            return Ok(StreamTokenReputationAdmissionOutcomeV1::NotCounted);
        }
        let event_id = ReputationJournalEventIdV1(outcome.binding.validation_id());
        Ok(StreamTokenReputationAdmissionOutcomeV1::Enqueued(
            if replay {
                ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id }
            } else {
                ReputationJournalEnqueueOutcomeV1::Inserted { event_id }
            },
        ))
    }
}
#[derive(Debug, Default)]
struct DurableProviderState {
    requests: Vec<(
        StreamTokenGatewayAdmissionRequestV1,
        StreamTokenGatewayAdmissionRecordV1,
    )>,
    records: Vec<StreamTokenGatewayAdmissionRecordV1>,
    acknowledged_through: u64,
    active_leases: Vec<(String, [u8; 32], u64)>,
    released_leases: Vec<[u8; 32]>,
    pending_script: VecDeque<Option<StreamTokenGatewayAdmissionReadbackV1>>,
}
#[derive(Debug)]
struct DurableProvider {
    state: Mutex<DurableProviderState>,
}
impl DurableProvider {
    fn new() -> Self {
        Self {
            state: Mutex::new(DurableProviderState::default()),
        }
    }
    fn script_pending(
        &self,
        script: impl IntoIterator<Item = Option<StreamTokenGatewayAdmissionReadbackV1>>,
    ) {
        self.state
            .lock()
            .expect("provider state")
            .pending_script
            .extend(script);
    }
    fn acknowledged_through(&self) -> u64 {
        self.state
            .lock()
            .expect("provider state")
            .acknowledged_through
    }
}
impl StreamTokenGatewayAdmissionProviderV1 for DurableProvider {
    fn handle(&self) -> &str {
        HANDLE
    }
    fn qualification(
        &self,
    ) -> Result<StreamTokenGatewayAdmissionQualificationV1, StreamTokenGatewayAdmissionErrorV1>
    {
        Ok(qualification())
    }
    fn admit(
        &self,
        request: &StreamTokenGatewayAdmissionRequestV1,
    ) -> Result<StreamTokenGatewayAdmissionResultV1, StreamTokenGatewayAdmissionErrorV1> {
        request.validate()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::Unavailable)?;
        if let Some((stored_request, record)) = state
            .requests
            .iter()
            .find(|(stored, _)| stored.context.digest() == request.context.digest())
        {
            if stored_request != request {
                return Err(StreamTokenGatewayAdmissionErrorV1::Conflict);
            }
            let sequence = record.outcome.binding.gateway_sequence;
            let delivery_state = if sequence <= state.acknowledged_through {
                StreamTokenGatewayAdmissionDeliveryStateV1::AcknowledgedExactReplay {
                    acknowledged_through_sequence: state.acknowledged_through,
                }
            } else {
                StreamTokenGatewayAdmissionDeliveryStateV1::Pending {
                    predecessor_sequence: sequence - 1,
                }
            };
            return Ok(StreamTokenGatewayAdmissionResultV1 {
                record: *record,
                delivery_state,
            });
        }
        let sequence = u64::try_from(state.records.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(StreamTokenGatewayAdmissionErrorV1::Unavailable)?;
        let mut status = request.status;
        if status == StreamTokenValidationStatusV1::Accepted {
            let quota = request
                .quota
                .as_ref()
                .ok_or(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)?;
            state
                .active_leases
                .retain(|(_, _, expires)| *expires > request.validated_at_unix_ms);
            let active = state
                .active_leases
                .iter()
                .filter(|(token_id, _, _)| token_id == &quota.token_id)
                .count();
            if active >= usize::from(quota.max_streams) {
                status = StreamTokenValidationStatusV1::ProviderViolation(
                    StreamTokenViolationKindV1::ConcurrencyLimitExceeded,
                );
            }
        }
        let record = record_for_request(request, sequence, status);
        if let (Some(quota), Some(lease_id), Some(expires)) = (
            request.quota.as_ref(),
            record.lease_id,
            record.lease_expires_at_unix_ms,
        ) {
            state
                .active_leases
                .push((quota.token_id.clone(), lease_id, expires));
        }
        state.requests.push((request.clone(), record));
        state.records.push(record);
        Ok(StreamTokenGatewayAdmissionResultV1 {
            record,
            delivery_state: StreamTokenGatewayAdmissionDeliveryStateV1::Pending {
                predecessor_sequence: sequence - 1,
            },
        })
    }
    fn pending(
        &self,
        max_items: u32,
    ) -> Result<StreamTokenGatewayAdmissionReadbackV1, StreamTokenGatewayAdmissionErrorV1> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::Unavailable)?;
        if let Some(scripted) = state.pending_script.pop_front() {
            if let Some(readback) = scripted {
                return Ok(readback);
            }
        }
        let acknowledged = usize::try_from(state.acknowledged_through)
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::Unavailable)?;
        Ok(StreamTokenGatewayAdmissionReadbackV1 {
            acknowledged_through_sequence: state.acknowledged_through,
            high_water_sequence: u64::try_from(state.records.len())
                .map_err(|_| StreamTokenGatewayAdmissionErrorV1::Unavailable)?,
            records: state
                .records
                .iter()
                .skip(acknowledged)
                .take(max_items as usize)
                .copied()
                .collect(),
        })
    }
    fn acknowledge(
        &self,
        record: StreamTokenGatewayAdmissionRecordV1,
    ) -> Result<StreamTokenGatewayAdmissionAckV1, StreamTokenGatewayAdmissionErrorV1> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::Unavailable)?;
        let sequence = record.outcome.binding.gateway_sequence;
        let index = usize::try_from(sequence - 1)
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::Conflict)?;
        if state.records.get(index) != Some(&record) {
            return Err(StreamTokenGatewayAdmissionErrorV1::Conflict);
        }
        if sequence <= state.acknowledged_through {
            return Ok(StreamTokenGatewayAdmissionAckV1::ExactReplay);
        }
        if state.acknowledged_through.checked_add(1) != Some(sequence) {
            return Err(StreamTokenGatewayAdmissionErrorV1::Conflict);
        }
        state.acknowledged_through = sequence;
        Ok(StreamTokenGatewayAdmissionAckV1::Acknowledged)
    }
    fn release_lease(
        &self,
        record: StreamTokenGatewayAdmissionRecordV1,
    ) -> Result<StreamTokenGatewayAdmissionAckV1, StreamTokenGatewayAdmissionErrorV1> {
        let lease_id = record
            .lease_id
            .ok_or(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| StreamTokenGatewayAdmissionErrorV1::Unavailable)?;
        if state.released_leases.contains(&lease_id) {
            return Ok(StreamTokenGatewayAdmissionAckV1::ExactReplay);
        }
        let position = state
            .active_leases
            .iter()
            .position(|(_, active_id, _)| *active_id == lease_id)
            .ok_or(StreamTokenGatewayAdmissionErrorV1::Conflict)?;
        state.active_leases.remove(position);
        state.released_leases.push(lease_id);
        Ok(StreamTokenGatewayAdmissionAckV1::Acknowledged)
    }
}
fn capture(
    provider: Arc<DurableProvider>,
    reputation: Arc<ReputationProbe>,
    reconcile_max_items: u32,
) -> StreamTokenAdmissionCaptureV1 {
    StreamTokenAdmissionCaptureV1::try_new(
        HANDLE,
        qualification(),
        reconcile_max_items,
        provider,
        reputation,
    )
    .expect("qualified capture")
}
#[test]
fn acknowledged_admit_replay_replays_reputation_and_requires_exact_ack_readback() {
    let provider = Arc::new(DurableProvider::new());
    let reputation = Arc::new(ReputationProbe::default());
    let first = capture(Arc::clone(&provider), Arc::clone(&reputation), 8);
    let request = request(
        "nonce-replay",
        VALIDATED_AT_MS,
        VALIDATED_AT_MS / 1_000 + 600,
        2,
    );
    let inserted = first.admit(&request).expect("first admission");
    assert_eq!(provider.acknowledged_through(), 1);
    let replica = capture(Arc::clone(&provider), Arc::clone(&reputation), 8);
    assert_eq!(
        replica.admit(&request).expect("acknowledged replay"),
        inserted
    );
    assert_eq!(provider.acknowledged_through(), 1);
    assert_eq!(reputation.calls().len(), 2);
}
#[test]
fn omitted_required_row_is_rejected_before_any_callback() {
    let provider = Arc::new(DurableProvider::new());
    provider.script_pending([
        None,
        Some(StreamTokenGatewayAdmissionReadbackV1 {
            acknowledged_through_sequence: 0,
            high_water_sequence: 1,
            records: Vec::new(),
        }),
    ]);
    let reputation = Arc::new(ReputationProbe::default());
    let capture = capture(Arc::clone(&provider), Arc::clone(&reputation), 8);
    let error = capture
        .admit(&request(
            "nonce-omitted",
            VALIDATED_AT_MS,
            VALIDATED_AT_MS / 1_000 + 600,
            2,
        ))
        .expect_err("omitted required row must fail");
    assert_eq!(
        error,
        StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome
    );
    assert!(reputation.calls().is_empty());
    assert_eq!(provider.acknowledged_through(), 0);
}
#[test]
fn later_sequence_cannot_substitute_for_required_row() {
    let provider = Arc::new(DurableProvider::new());
    let required = request(
        "nonce-required",
        VALIDATED_AT_MS,
        VALIDATED_AT_MS / 1_000 + 600,
        2,
    );
    let later = request(
        "nonce-later",
        VALIDATED_AT_MS + 1,
        VALIDATED_AT_MS / 1_000 + 600,
        2,
    );
    provider.script_pending([
        None,
        Some(StreamTokenGatewayAdmissionReadbackV1 {
            acknowledged_through_sequence: 0,
            high_water_sequence: 2,
            records: vec![record_for_request(
                &later,
                2,
                StreamTokenValidationStatusV1::Accepted,
            )],
        }),
    ]);
    let reputation = Arc::new(ReputationProbe::default());
    let capture = capture(Arc::clone(&provider), Arc::clone(&reputation), 8);
    assert_eq!(
        capture.admit(&required),
        Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
    );
    assert!(reputation.calls().is_empty());
    assert_eq!(provider.acknowledged_through(), 0);
}
#[test]
fn complete_batch_is_validated_before_first_callback_or_ack() {
    let provider = Arc::new(DurableProvider::new());
    let first = request(
        "nonce-batch-a",
        VALIDATED_AT_MS,
        VALIDATED_AT_MS / 1_000 + 600,
        2,
    );
    let second = request(
        "nonce-batch-b",
        VALIDATED_AT_MS + 1,
        VALIDATED_AT_MS / 1_000 + 600,
        2,
    );
    let first_record = provider.admit(&first).expect("stage first").record;
    let mut substituted = provider.admit(&second).expect("stage second").record;
    substituted.lease_expires_at_unix_ms = substituted
        .lease_expires_at_unix_ms
        .and_then(|expires| expires.checked_add(1));
    provider.script_pending([Some(StreamTokenGatewayAdmissionReadbackV1 {
        acknowledged_through_sequence: 0,
        high_water_sequence: 2,
        records: vec![first_record, substituted],
    })]);
    let reputation = Arc::new(ReputationProbe::default());
    let capture = capture(Arc::clone(&provider), Arc::clone(&reputation), 8);
    assert_eq!(
        capture.reconcile_pending(),
        Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
    );
    assert!(reputation.calls().is_empty());
    assert_eq!(provider.acknowledged_through(), 0);
}
#[test]
fn callback_crash_retains_row_for_exact_restart_replay() {
    let provider = Arc::new(DurableProvider::new());
    let reputation = Arc::new(ReputationProbe::default());
    reputation.fail_once();
    let request = request(
        "nonce-crash",
        VALIDATED_AT_MS,
        VALIDATED_AT_MS / 1_000 + 600,
        2,
    );
    assert_eq!(
        capture(Arc::clone(&provider), Arc::clone(&reputation), 8).admit(&request),
        Err(StreamTokenGatewayAdmissionErrorV1::ReputationCallback)
    );
    assert_eq!(provider.acknowledged_through(), 0);
    let restarted = capture(Arc::clone(&provider), Arc::clone(&reputation), 8);
    assert_eq!(restarted.reconcile_pending().expect("restart replay"), 1);
    assert_eq!(provider.acknowledged_through(), 1);
    assert_eq!(reputation.calls().len(), 1);
}
#[test]
fn shared_provider_owns_concurrency_and_release_across_replicas() {
    let provider = Arc::new(DurableProvider::new());
    let reputation = Arc::new(ReputationProbe::default());
    let first_replica = capture(Arc::clone(&provider), Arc::clone(&reputation), 8);
    let second_replica = capture(Arc::clone(&provider), Arc::clone(&reputation), 8);
    let first = first_replica
        .admit(&request(
            "nonce-stream-a",
            VALIDATED_AT_MS,
            VALIDATED_AT_MS / 1_000 + 600,
            1,
        ))
        .expect("first lease");
    let blocked = second_replica
        .admit(&request(
            "nonce-stream-b",
            VALIDATED_AT_MS + 1,
            VALIDATED_AT_MS / 1_000 + 600,
            1,
        ))
        .expect("authenticated concurrency terminal");
    assert_eq!(
        blocked.outcome.status,
        StreamTokenValidationStatusV1::ProviderViolation(
            StreamTokenViolationKindV1::ConcurrencyLimitExceeded
        )
    );
    assert!(blocked.lease_id.is_none());
    assert_eq!(
        first_replica.release_lease(first),
        Ok(StreamTokenGatewayAdmissionAckV1::Acknowledged)
    );
    assert_eq!(
        second_replica
            .admit(&request(
                "nonce-stream-c",
                VALIDATED_AT_MS + 2,
                VALIDATED_AT_MS / 1_000 + 600,
                1,
            ))
            .expect("lease after release")
            .outcome
            .status,
        StreamTokenValidationStatusV1::Accepted
    );
}
#[test]
fn crashed_lease_expires_at_exact_authenticated_deadline() {
    let provider = Arc::new(DurableProvider::new());
    let reputation = Arc::new(ReputationProbe::default());
    let capture = capture(Arc::clone(&provider), reputation, 8);
    let first = capture
        .admit(&request(
            "nonce-expiring-a",
            VALIDATED_AT_MS,
            VALIDATED_AT_MS / 1_000 + 600,
            1,
        ))
        .expect("first lease");
    let deadline = first
        .lease_expires_at_unix_ms
        .expect("authenticated lease deadline");
    assert_eq!(
        capture
            .admit(&request(
                "nonce-expiring-b",
                deadline,
                deadline / 1_000 + 600,
                1,
            ))
            .expect("lease at exact prior expiry")
            .outcome
            .status,
        StreamTokenValidationStatusV1::Accepted
    );
}
#[test]
fn lease_deadline_is_exact_and_rejects_early_late_expired_and_overflow_values() {
    let provider = DurableProvider::new();
    let short = request(
        "nonce-lease-boundary",
        VALIDATED_AT_MS,
        VALIDATED_AT_MS / 1_000 + 30,
        2,
    );
    let admission = provider.admit(&short).expect("short token lease");
    let expected = VALIDATED_AT_MS + 30_000;
    assert_eq!(admission.record.lease_expires_at_unix_ms, Some(expected));
    admission
        .validate_for_request(&short, qualification())
        .expect("exact lease deadline");
    let mut early = admission;
    early.record.lease_expires_at_unix_ms = Some(expected - 1);
    assert_eq!(
        early.validate_for_request(&short, qualification()),
        Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
    );
    let mut late = admission;
    late.record.lease_expires_at_unix_ms = Some(expected + 1);
    assert_eq!(
        late.validate_for_request(&short, qualification()),
        Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
    );
    let mut expired = admission;
    expired.record.lease_token_expires_at_epoch = Some(VALIDATED_AT_MS / 1_000);
    expired.record.lease_expires_at_unix_ms = Some(VALIDATED_AT_MS);
    assert_eq!(
        expired.validate_for_request(&short, qualification()),
        Err(StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome)
    );
    let mut overflowing_token = short.clone();
    overflowing_token
        .quota
        .as_mut()
        .expect("quota")
        .expires_at_epoch = u64::MAX / 1_000 + 1;
    assert_eq!(
        overflowing_token.validate(),
        Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)
    );
    let mut mismatched_bytes = short.clone();
    mismatched_bytes
        .quota
        .as_mut()
        .expect("quota")
        .requested_bytes = 959;
    assert_eq!(
        mismatched_bytes.validate(),
        Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)
    );
    assert_eq!(
        exact_lease_expiry_unix_ms(u64::MAX - 5, u64::MAX / 1_000, 10),
        Err(StreamTokenGatewayAdmissionErrorV1::InvalidRequest)
    );
}
