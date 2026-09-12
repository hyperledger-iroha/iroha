//! Separate-key observer fixtures around real independently attested stream receipt simulations.

use super::*;

pub(super) enum VerifiedEvidence {
    Current(VerifiedStreamTokenSignerQualificationV1),
    AfterCommit(VerifiedStreamTokenSignerCompletedObservationV1),
    Released(VerifiedStreamTokenSignerReceiptV1),
}

pub(super) struct Evidence {
    pub(super) receipt: receipt_fixture::Fixture,
    pub(super) observer: KeyPair,
    pub(super) trust: SignerStateObserverTrustV1,
    pub(super) state: SignerStreamTokenStateObservationV1,
    pub(super) request: SignerStreamTokenObservationRequestV1,
    pub(super) attempt: Option<SignerStreamTokenObservationExpectedV1>,
    pub(super) phase: Phase,
    pub(super) now: u64,
    pub(super) receipt_bytes: Vec<u8>,
}

pub(super) fn is_current(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::Startup
            | Phase::BeforeAdmission
            | Phase::BeforeProvider
            | Phase::AfterProvider
            | Phase::BeforeCommit
    )
}

pub(super) fn phases() -> [Phase; 7] {
    [
        Phase::Startup,
        Phase::BeforeAdmission,
        Phase::BeforeProvider,
        Phase::AfterProvider,
        Phase::BeforeCommit,
        Phase::AfterCommit,
        Phase::BeforeRelease,
    ]
}

pub(super) fn observer_payload(body: &SignerStreamTokenStateObservationBodyV1) -> Vec<u8> {
    let mut payload = b"iroha.sorafs.stream-token.finalized-state.v1\0".to_vec();
    payload.extend(norito::encode_canonical(body).expect("independent state-body oracle"));
    payload
}

pub(super) fn request_digest(request: &SignerStreamTokenObservationRequestV1) -> [u8; 32] {
    receipt_fixture::oracle_canonical(b"iroha.sorafs.stream-token.observation-request.v1", request)
}

pub(super) fn receipt_digest(receipt: &[u8]) -> [u8; 32] {
    receipt_fixture::oracle_digest(b"iroha.sorafs.signer.stream-token.receipt.v1", &[receipt])
}

impl Evidence {
    pub(super) fn new(phase: Phase) -> Self {
        Self::with_receipt(phase, receipt_fixture::fixture())
    }

    pub(super) fn with_receipt(phase: Phase, receipt: receipt_fixture::Fixture) -> Self {
        receipt_fixture::assert_positive(&receipt);
        let observer = receipt_fixture::key(0x73);
        // Independently configured before any observation is assembled or signed.
        let trust = SignerStateObserverTrustV1 {
            authority: SignerCustodyAuthorityV1 {
                service_id: "state-observer-primary".into(),
                administrator_id: "state-security-primary".into(),
                key_revision: 2,
                policy_revision: 4,
                policy_digest: [0x74; 32],
            },
            public_key: observer.public_key().clone(),
            active_from_unix_ms: 800_000,
            active_until_unix_ms: 2_000_000,
            max_state_age_ms: 10_000,
        };
        let now = receipt.current.now_unix_ms;
        let receipt_bytes = receipt
            .receipt
            .encode_canonical()
            .expect("retained exact receipt");
        let challenge = [0x91; 32]; // Deterministic test input, not a production entropy source.
        let minimum_anchor = receipt.current.current_anchor;
        let not_before = receipt.current.anchor_observed_at_unix_ms - 1_000;
        let attempt = if is_current(phase) {
            SignerStreamTokenObservationExpectedV1::current(
                &receipt.binding,
                phase,
                challenge,
                minimum_anchor,
                not_before,
            )
        } else {
            SignerStreamTokenObservationExpectedV1::completed(
                &receipt_bytes,
                &receipt.token,
                &receipt.expected,
                &receipt.binding,
                phase,
                challenge,
                minimum_anchor,
                not_before,
            )
        }
        .expect("caller prepares request before candidate observer response");
        let request = attempt.request().clone();
        let subject = if is_current(phase) {
            SignerStreamTokenStateSubjectV1::CurrentCustody {
                binding_digest: receipt.expected.binding_digest(),
            }
        } else {
            SignerStreamTokenStateSubjectV1::CompletedOperation {
                binding_digest: receipt.expected.binding_digest(),
                operation_id: receipt.expected.operation_id(),
                signing_payload_digest: receipt.expected.signing_payload_digest(),
                signing_payload_size: receipt.expected.signing_payload_size(),
                completed_operation: receipt.completion,
            }
        };
        let state = SignerStreamTokenStateObservationV1 {
            body: SignerStreamTokenStateObservationBodyV1 {
                magic: SignerStreamTokenStateObservationBodyV1::magic(),
                request_digest: request_digest(&request),
                phase,
                subject,
                authority: trust.authority.clone(),
                chain_id: receipt.binding.chain_id.clone(),
                network_id: receipt.binding.network_id,
                observed_at_unix_ms: receipt.current.anchor_observed_at_unix_ms,
                expires_at_unix_ms: now + 5_000,
                current_anchor: receipt.current.current_anchor,
                active_head: receipt.current.active_head,
                signer_revoked: false,
                attester_revoked: false,
            },
            signature: [0; 64],
        };
        let mut evidence = Self {
            receipt,
            observer,
            trust,
            state,
            request,
            attempt: Some(attempt),
            phase,
            now,
            receipt_bytes,
        };
        evidence.resign();
        evidence
    }

    // Deliberately signs even invalid claims so negatives reach consumer policy checks.
    pub(super) fn resign(&mut self) {
        self.state.signature = Signature::try_new(
            self.observer.private_key(),
            &observer_payload(&self.state.body),
        )
        .expect("independent observer signature")
        .payload()
        .try_into()
        .expect("Ed25519 width");
    }

    pub(super) fn observation_bytes(&self) -> Vec<u8> {
        // Safe producer bounds have their own tests; malicious public claims still need framing.
        norito::encode_canonical(&self.state).expect("canonical candidate observation")
    }

    pub(super) fn verify_bytes(&mut self, bytes: &[u8]) -> Result<VerifiedEvidence, EvidenceError> {
        let attempt = self
            .attempt
            .take()
            .expect("the test runtime owns exactly one pending attempt");
        verify_attempt(self, bytes, attempt)
    }

    pub(super) fn verify(&mut self) -> Result<VerifiedEvidence, EvidenceError> {
        self.verify_bytes(&self.observation_bytes())
    }

    pub(super) fn completed_mut(&mut self) -> &mut SignerCompletedOperationV1 {
        let SignerStreamTokenStateSubjectV1::CompletedOperation {
            completed_operation,
            ..
        } = &mut self.state.body.subject
        else {
            panic!("completed fixture")
        };
        completed_operation
    }

    pub(super) fn fresh_attempt(&mut self, challenge: [u8; 32], not_before: u64) {
        assert!(
            self.attempt.is_none(),
            "cannot replace an in-flight caller attempt"
        );
        let attempt = if is_current(self.phase) {
            SignerStreamTokenObservationExpectedV1::current(
                &self.receipt.binding,
                self.phase,
                challenge,
                self.request.minimum_anchor,
                not_before,
            )
        } else {
            SignerStreamTokenObservationExpectedV1::completed(
                &self.receipt_bytes,
                &self.receipt.token,
                &self.receipt.expected,
                &self.receipt.binding,
                self.phase,
                challenge,
                self.request.minimum_anchor,
                not_before,
            )
        }
        .expect("fresh independently prepared fixture request");
        self.request = attempt.request().clone();
        self.state.body.request_digest = request_digest(&self.request);
        self.attempt = Some(attempt);
        self.resign();
    }
}

pub(super) fn verify_attempt(
    e: &Evidence,
    bytes: &[u8],
    attempt: SignerStreamTokenObservationExpectedV1,
) -> Result<VerifiedEvidence, EvidenceError> {
    if is_current(e.phase) {
        verify_stream_token_signer_current_evidence_v1(
            &e.receipt.receipt.custody_record,
            bytes,
            &e.receipt.binding,
            &e.receipt.trust,
            &e.trust,
            attempt,
            e.now,
        )
        .map(VerifiedEvidence::Current)
    } else if e.phase == Phase::AfterCommit {
        verify_stream_token_signer_completed_observation_v1(
            &e.receipt_bytes,
            bytes,
            &e.receipt.token,
            &e.receipt.expected,
            &e.receipt.binding,
            &e.receipt.trust,
            &e.trust,
            attempt,
            e.now,
        )
        .map(VerifiedEvidence::AfterCommit)
    } else {
        verify_stream_token_signer_evidence_v1(
            &e.receipt_bytes,
            bytes,
            &e.receipt.token,
            &e.receipt.expected,
            &e.receipt.binding,
            &e.receipt.trust,
            &e.trust,
            attempt,
            e.now,
        )
        .map(VerifiedEvidence::Released)
    }
}

pub(super) fn assert_positive(e: &mut Evidence) {
    let verified = e
        .verify()
        .unwrap_or_else(|error| panic!("signed evidence positive: {error:?}"));
    match verified {
        VerifiedEvidence::Current(value) => {
            assert!(is_current(e.phase));
            assert_eq!(value.phase(), e.phase);
            assert_eq!(
                value.observed_at_unix_ms(),
                e.state.body.observed_at_unix_ms
            );
            assert_eq!(
                value.custody().record_digest(),
                e.receipt.current.active_head.record_digest
            );
            // Current wire subjects bind only the configured signer. For an operation phase,
            // this bounded harness separately retains the in-flight operation/original custody;
            // a same-binding qualification is not pooled or reused across operations.
            if e.phase != Phase::Startup {
                assert_eq!(
                    e.receipt.receipt.request.operation_id,
                    e.receipt.expected.operation_id()
                );
                assert_eq!(
                    SignerOperationCustodyV1::from_verified(value.custody()),
                    e.receipt.receipt.request.original_custody
                );
            }
        }
        VerifiedEvidence::AfterCommit(value) => {
            assert_eq!(e.phase, Phase::AfterCommit);
            assert_eq!(
                value.observed_at_unix_ms(),
                e.state.body.observed_at_unix_ms
            );
            assert_eq!(value.completion(), &e.receipt.completion);
            assert_eq!(
                value.custody().record_digest(),
                e.receipt.current.active_head.record_digest
            );
        }
        VerifiedEvidence::Released(value) => {
            assert_eq!(e.phase, Phase::BeforeRelease);
            assert_eq!(
                value.observed_at_unix_ms(),
                e.state.body.observed_at_unix_ms
            );
            assert_eq!(value.completion(), &e.receipt.completion);
            assert_eq!(
                value.signing_payload_digest(),
                e.receipt.expected.signing_payload_digest()
            );
            assert_eq!(
                value.custody().record_digest(),
                e.receipt.current.active_head.record_digest
            );
        }
    }
    assert!(e.attempt.is_none());
}

// Two independent deterministic test scenarios: verify the exact baseline before mutating its
// separately owned copy. This does not model production retry or a global challenge-reuse cache.
pub(super) fn checked_fixture(phase: Phase) -> Evidence {
    let mut baseline = Evidence::new(phase);
    assert_positive(&mut baseline);
    Evidence::new(phase)
}

pub(super) fn assert_error(e: &mut Evidence, expected: EvidenceError) {
    assert_eq!(e.verify().err(), Some(expected));
    assert!(
        e.attempt.is_none(),
        "failure also consumes the pending attempt"
    );
}

pub(super) fn fields(payload: &[u8], count: usize) -> Vec<std::ops::Range<usize>> {
    let flags = norito::core::default_encode_flags();
    let mut offset = 0;
    let result = (0..count)
        .map(|_| {
            let start = offset;
            let (len, prefix) =
                norito::core::read_len_from_slice_with_flags(&payload[offset..], flags)
                    .expect("actual sequential schema field");
            offset += prefix + len;
            start..offset
        })
        .collect();
    assert_eq!(offset, payload.len(), "exact current field count");
    result
}

pub(super) fn omit_field<T: norito::NoritoSerialize>(
    value: &T,
    count: usize,
    omitted: usize,
) -> Vec<u8> {
    let payload = value.encode();
    let mut changed = Vec::new();
    for (index, range) in fields(&payload, count).into_iter().enumerate() {
        if index != omitted {
            changed.extend_from_slice(&payload[range]);
        }
    }
    norito::core::frame_bare_with_header_flags::<T>(&changed, norito::core::default_encode_flags())
        .expect("actual schema with omitted required field")
}
