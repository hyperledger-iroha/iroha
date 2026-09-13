//! Retained observation attempts and independent signed-state fixtures for evidence tests.

use super::*;

pub(super) struct Evidence {
    pub(super) receipt: receipt_fixture::Fixture,
    pub(super) receipt_bytes: Vec<u8>,
    pub(super) observer: KeyPair,
    pub(super) trust: SignerStateObserverTrustV1,
    pub(super) phase: Phase,
    pub(super) request: SignerStreamTokenObservationRequestV1,
    pub(super) attempt: Option<SignerStreamTokenObservationExpectedV1>,
    pub(super) state: SignerStreamTokenStateObservationV1,
    pub(super) now: u64,
}

pub(super) enum VerifiedEvidence {
    Current(VerifiedStreamTokenSignerQualificationV1),
    Completed(VerifiedStreamTokenSignerCompletedObservationV1),
    Released(VerifiedStreamTokenSignerReceiptV1),
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

// The oracle spells out the protocol domains and never calls the production preimage helpers.
pub(super) fn observer_payload(body: &SignerStreamTokenStateObservationBodyV1) -> Vec<u8> {
    let mut payload = b"iroha.sorafs.stream-token.finalized-state.v1\0".to_vec();
    payload.extend(norito::encode_canonical(body).expect("independent observation frame"));
    payload
}

pub(super) fn request_digest(request: &SignerStreamTokenObservationRequestV1) -> [u8; 32] {
    receipt_fixture::oracle_canonical(b"iroha.sorafs.stream-token.observation-request.v1", request)
}

pub(super) fn receipt_digest(bytes: &[u8]) -> [u8; 32] {
    receipt_fixture::oracle_digest(b"iroha.sorafs.signer.stream-token.receipt.v1", &[bytes])
}

impl Evidence {
    pub(super) fn new(phase: Phase) -> Self {
        Self::with_receipt(phase, receipt_fixture::fixture())
    }

    pub(super) fn with_receipt(phase: Phase, receipt: receipt_fixture::Fixture) -> Self {
        let receipt_bytes = receipt
            .receipt
            .encode_canonical()
            .expect("original receipt");
        let now = receipt.current.now_unix_ms;
        let observed_at = receipt.current.anchor_observed_at_unix_ms;
        let attempt = if is_current(phase) {
            SignerStreamTokenObservationExpectedV1::current(
                &receipt.binding,
                phase,
                [0x91; 32],
                receipt.current.current_anchor,
                observed_at - 1_000,
            )
        } else {
            SignerStreamTokenObservationExpectedV1::completed(
                &receipt_bytes,
                &receipt.token,
                &receipt.expected,
                &receipt.binding,
                phase,
                [0x91; 32],
                receipt.current.current_anchor,
                observed_at - 1_000,
            )
        }
        .expect("independently prepared pending attempt");
        let request = attempt.request().clone();
        let observer = receipt_fixture::key(0x71);
        let trust = SignerStateObserverTrustV1 {
            authority: SignerCustodyAuthorityV1 {
                service_id: "stream-state-observer".into(),
                administrator_id: "stream-state-reviewer".into(),
                key_revision: 2,
                policy_revision: 4,
                policy_digest: [0x75; 32],
            },
            public_key: observer.public_key().clone(),
            active_from_unix_ms: 800_000,
            active_until_unix_ms: 2_000_000,
            max_state_age_ms: 10_000,
        };
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
                magic: *b"IRSTKS01",
                request_digest: request_digest(&request),
                phase,
                subject,
                authority: trust.authority.clone(),
                chain_id: receipt.binding.chain_id.clone(),
                network_id: receipt.binding.network_id,
                observed_at_unix_ms: observed_at,
                expires_at_unix_ms: observed_at + 5_000,
                current_anchor: receipt.current.current_anchor,
                active_head: receipt.current.active_head,
                signer_revoked: false,
                attester_revoked: false,
            },
            signature: [0; 64],
        };
        let mut evidence = Self {
            receipt,
            receipt_bytes,
            observer,
            trust,
            phase,
            request,
            attempt: Some(attempt),
            state,
            now,
        };
        evidence.resign();
        evidence
    }

    pub(super) fn resign(&mut self) {
        // Raw canonical encoding lets signed malformed claims reach their precise verifier.
        self.state.signature = Signature::try_new(
            self.observer.private_key(),
            &observer_payload(&self.state.body),
        )
        .expect("independent observer signature")
        .payload()
        .try_into()
        .expect("Ed25519 width");
    }

    pub(super) fn fresh_attempt(&mut self, challenge: [u8; 32], not_before: u64) {
        assert!(
            self.attempt.is_none(),
            "retire the previous pending attempt"
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
        .expect("fresh independently retained attempt");
        self.request = attempt.request().clone();
        self.attempt = Some(attempt);
        self.state.body.request_digest = request_digest(&self.request);
        self.resign();
    }

    pub(super) fn observation_bytes(&self) -> Vec<u8> {
        norito::encode_canonical(&self.state).expect("raw canonical observation candidate")
    }

    pub(super) fn completed_mut(&mut self) -> &mut SignerCompletedOperationV1 {
        let SignerStreamTokenStateSubjectV1::CompletedOperation {
            completed_operation,
            ..
        } = &mut self.state.body.subject
        else {
            panic!("completed-phase fixture")
        };
        completed_operation
    }

    pub(super) fn verify_bytes(&mut self, bytes: &[u8]) -> Result<VerifiedEvidence, EvidenceError> {
        let attempt = self
            .attempt
            .take()
            .expect("one retained verification attempt");
        match self.phase {
            Phase::Startup
            | Phase::BeforeAdmission
            | Phase::BeforeProvider
            | Phase::AfterProvider
            | Phase::BeforeCommit => verify_stream_token_signer_current_evidence_v1(
                &self.receipt.receipt.custody_record,
                bytes,
                &self.receipt.binding,
                &self.receipt.trust,
                &self.trust,
                attempt,
                self.now,
            )
            .map(VerifiedEvidence::Current),
            Phase::AfterCommit => verify_stream_token_signer_completed_observation_v1(
                &self.receipt_bytes,
                bytes,
                &self.receipt.token,
                &self.receipt.expected,
                &self.receipt.binding,
                &self.receipt.trust,
                &self.trust,
                attempt,
                self.now,
            )
            .map(VerifiedEvidence::Completed),
            Phase::BeforeRelease => verify_stream_token_signer_evidence_v1(
                &self.receipt_bytes,
                bytes,
                &self.receipt.token,
                &self.receipt.expected,
                &self.receipt.binding,
                &self.receipt.trust,
                &self.trust,
                attempt,
                self.now,
            )
            .map(VerifiedEvidence::Released),
        }
    }
}

pub(super) fn assert_positive(evidence: &mut Evidence) {
    let bytes = evidence.observation_bytes();
    let verified = evidence
        .verify_bytes(&bytes)
        .expect("independent signed observation");
    let (custody, observed_at) = match &verified {
        VerifiedEvidence::Current(value) => {
            assert!(is_current(evidence.phase));
            assert_eq!(value.phase(), evidence.phase);
            (value.custody(), value.observed_at_unix_ms())
        }
        VerifiedEvidence::Completed(value) => {
            assert_eq!(evidence.phase, Phase::AfterCommit);
            assert_eq!(value.completion(), &evidence.receipt.completion);
            (value.custody(), value.observed_at_unix_ms())
        }
        VerifiedEvidence::Released(value) => {
            assert_eq!(evidence.phase, Phase::BeforeRelease);
            assert_eq!(value.completion(), &evidence.receipt.completion);
            assert_eq!(
                value.signing_payload_digest(),
                evidence.receipt.expected.signing_payload_digest()
            );
            (value.custody(), value.observed_at_unix_ms())
        }
    };
    assert_eq!(
        custody.record_digest(),
        evidence.receipt.current.active_head.record_digest
    );
    assert_eq!(observed_at, evidence.state.body.observed_at_unix_ms);
    assert!(evidence.attempt.is_none(), "success consumes the attempt");
}

pub(super) fn checked_fixture(phase: Phase) -> Evidence {
    assert_positive(&mut Evidence::new(phase));
    Evidence::new(phase)
}

pub(super) fn assert_error(evidence: &mut Evidence, expected: EvidenceError) {
    let bytes = evidence.observation_bytes();
    assert_eq!(evidence.verify_bytes(&bytes).err(), Some(expected));
    assert!(evidence.attempt.is_none(), "failure consumes the attempt");
}

pub(super) fn fields(bytes: &[u8], count: usize) -> Vec<std::ops::Range<usize>> {
    let flags = norito::core::default_encode_flags();
    let mut offset = 0;
    let mut ranges = Vec::with_capacity(count);
    for _ in 0..count {
        let start = offset;
        let (length, prefix) =
            norito::core::read_len_from_slice_with_flags(&bytes[offset..], flags)
                .expect("actual canonical field length");
        offset += prefix + length;
        assert!(offset <= bytes.len(), "field within actual payload");
        ranges.push(start..offset);
    }
    assert_eq!(offset, bytes.len(), "all actual canonical fields consumed");
    ranges
}

pub(super) fn omit_field<T: norito::NoritoSerialize>(
    value: &T,
    count: usize,
    omitted: usize,
) -> Vec<u8> {
    assert!(omitted < count);
    let flags = norito::core::default_encode_flags();
    let _guard = norito::core::DecodeFlagsGuard::enter(flags);
    let bytes = value.encode();
    let mut shortened = Vec::new();
    for (index, range) in fields(&bytes, count).into_iter().enumerate() {
        if index != omitted {
            shortened.extend_from_slice(&bytes[range]);
        }
    }
    norito::core::frame_bare_with_header_flags::<T>(&shortened, flags)
        .expect("omission retains the current schema")
}
