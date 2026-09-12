// Canonical derived types remain in the containing stream_token_evidence owner.

/// Independently selected point in the operation lifecycle, signed into every observer query.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub enum SignerStreamTokenObservationPhaseV1 {
    /// Startup qualifies current custody without creating a token operation.
    Startup,
    /// Fresh current custody for one new serving admission, without a completed-operation claim.
    BeforeAdmission,
    /// Fresh custody immediately before a provider operation.
    BeforeProvider,
    /// Fresh custody immediately after that provider operation.
    AfterProvider,
    /// Fresh custody before authoritative completion CAS.
    BeforeCommit,
    /// Authenticated exact completed row after CAS, without final release authority.
    AfterCommit,
    /// Final fresh exact completed observation immediately before token release.
    BeforeRelease,
}
impl SignerStreamTokenObservationPhaseV1 {
    fn is_current(self) -> bool {
        matches!(
            self,
            Self::Startup
                | Self::BeforeAdmission
                | Self::BeforeProvider
                | Self::AfterProvider
                | Self::BeforeCommit
        )
    }
}

/// Fixed-width independently requested subject; no optional or synthetic completion exists.
#[derive(Clone, Copy, PartialEq, Eq, Decode, Encode)]
pub enum SignerStreamTokenObservationRequestSubjectV1 {
    /// Current custody for the exact provider-scoped signer binding.
    CurrentCustody {
        /// Canonical independently pinned binding commitment.
        binding_digest: [u8; 32],
    },
    /// Exact prepared body, receipt and signatures for a completed operation.
    CompletedOperation {
        /// Canonical independently pinned binding commitment, including provider.
        binding_digest: [u8; 32],
        /// Independently prepared operation identity.
        operation_id: [u8; 32],
        /// Exact canonical domain-prefixed body commitment.
        signing_payload_digest: [u8; 32],
        /// Exact full role message byte count.
        signing_payload_size: u64,
        /// Commitment to the exact bounded canonical receipt received before this query.
        receipt_digest: [u8; 32],
        /// Existing canonical commitment to all four exact ordered signatures.
        signatures_digest: [u8; 32],
    },
}
impl SignerStreamTokenObservationRequestSubjectV1 {
    fn is_valid(self, phase: SignerStreamTokenObservationPhaseV1) -> bool {
        match self {
            Self::CurrentCustody { binding_digest } => {
                phase.is_current() && binding_digest != [0; 32]
            }
            Self::CompletedOperation {
                binding_digest,
                operation_id,
                signing_payload_digest,
                signing_payload_size,
                receipt_digest,
                signatures_digest,
            } => {
                !phase.is_current()
                    && binding_digest != [0; 32]
                    && operation_id != [0; 32]
                    && signing_payload_digest != [0; 32]
                    && signing_payload_size != 0
                    && signing_payload_size <= SIGNER_STREAM_TOKEN_MAX_PAYLOAD_BYTES_V1 as u64
                    && receipt_digest != [0; 32]
                    && signatures_digest != [0; 32]
            }
        }
    }
}

/// Canonical observer query; decoding this public claim does not create a retained expectation.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::signer::stream_token_evidence::SignerStreamTokenObservationRequestV1"
)]
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct SignerStreamTokenObservationRequestV1 {
    /// Sole marker, available from [`Self::magic`].
    pub magic: [u8; 8],
    /// Exact independent lifecycle phase.
    pub phase: SignerStreamTokenObservationPhaseV1,
    /// Exact phase-compatible subject.
    pub subject: SignerStreamTokenObservationRequestSubjectV1,
    /// Fresh unpredictable caller challenge; never chosen by the responder.
    pub challenge: [u8; 32],
    /// Independently known finalized lower bound; equal height requires full equality.
    pub minimum_anchor: SignerCustodyAnchorV1,
    /// Caller-selected trusted observation-time floor, including retained monotonic history.
    pub not_before_unix_ms: u64,
}
impl SignerStreamTokenObservationRequestV1 {
    /// Sole canonical request marker.
    pub const fn magic() -> [u8; 8] {
        REQUEST_MAGIC
    }
    fn validate(&self) -> Result<(), SignerStreamTokenEvidenceErrorV1> {
        if self.magic != REQUEST_MAGIC
            || self.challenge == [0; 32]
            || !valid_anchor(self.minimum_anchor)
            || self.not_before_unix_ms == 0
            || !self.subject.is_valid(self.phase)
        {
            return Err(SignerStreamTokenEvidenceErrorV1::SourceMismatch);
        }
        Ok(())
    }
    /// Encode the one bounded canonical query after fixed-field validation.
    ///
    /// # Errors
    /// Rejects malformed fields, wrong phase/subject or the complete request ceiling.
    pub fn encode_canonical(&self) -> Result<Vec<u8>, SignerStreamTokenEvidenceErrorV1> {
        self.validate()?;
        encode_document(self, SIGNER_STREAM_TOKEN_OBSERVATION_REQUEST_MAX_BYTES_V1)
    }
    /// Decode and validate a bounded canonical request claim, never a retained attempt.
    ///
    /// # Errors
    /// Rejects noncanonical frames, resource limits and invalid phase/subject/challenge/floors.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, SignerStreamTokenEvidenceErrorV1> {
        let value: Self =
            decode_document(bytes, SIGNER_STREAM_TOKEN_OBSERVATION_REQUEST_MAX_BYTES_V1)?;
        value.validate()?;
        Ok(value)
    }
    /// Commit the entire canonical phase, subject, challenge and independent time/finality floors.
    ///
    /// # Errors
    /// Rejects invalid fields or bounded canonical encoding failure.
    pub fn digest(&self) -> Result<[u8; 32], SignerStreamTokenEvidenceErrorV1> {
        let bytes = self.encode_canonical()?;
        Ok(digest_parts(REQUEST_DOMAIN, &[&bytes]))
    }
}

/// Exact signed state subject, with a completion row only in the completed phase.
#[derive(Clone, Copy, PartialEq, Eq, Decode, Encode)]
pub enum SignerStreamTokenStateSubjectV1 {
    /// Current custody has no synthetic token or completed operation.
    CurrentCustody {
        /// Independently requested provider-scoped binding commitment.
        binding_digest: [u8; 32],
    },
    /// Exact prepared operation and immutable authenticated completed row.
    CompletedOperation {
        /// Independently requested provider-scoped binding commitment.
        binding_digest: [u8; 32],
        /// Independently prepared operation identity.
        operation_id: [u8; 32],
        /// Exact domain-prefixed canonical body commitment.
        signing_payload_digest: [u8; 32],
        /// Full role-signing message size.
        signing_payload_size: u64,
        /// Immutable row authenticated by the observer under its finalized operation anchor.
        completed_operation: SignerCompletedOperationV1,
    },
}
impl SignerStreamTokenStateSubjectV1 {
    fn matches_request(&self, request: &SignerStreamTokenObservationRequestSubjectV1) -> bool {
        match (self, request) {
            (
                Self::CurrentCustody {
                    binding_digest: actual,
                },
                SignerStreamTokenObservationRequestSubjectV1::CurrentCustody { binding_digest },
            ) => actual == binding_digest,
            (
                Self::CompletedOperation {
                    binding_digest: actual_binding,
                    operation_id: actual_operation,
                    signing_payload_digest: actual_payload,
                    signing_payload_size: actual_size,
                    ..
                },
                SignerStreamTokenObservationRequestSubjectV1::CompletedOperation {
                    binding_digest,
                    operation_id,
                    signing_payload_digest,
                    signing_payload_size,
                    ..
                },
            ) => {
                actual_binding == binding_digest
                    && actual_operation == operation_id
                    && actual_payload == signing_payload_digest
                    && actual_size == signing_payload_size
            }
            _ => false,
        }
    }
    fn valid_phase(&self, phase: SignerStreamTokenObservationPhaseV1) -> bool {
        match self {
            Self::CurrentCustody { binding_digest } => {
                phase.is_current() && *binding_digest != [0; 32]
            }
            Self::CompletedOperation {
                binding_digest,
                operation_id,
                signing_payload_digest,
                signing_payload_size,
                ..
            } => {
                !phase.is_current()
                    && *binding_digest != [0; 32]
                    && *operation_id != [0; 32]
                    && *signing_payload_digest != [0; 32]
                    && *signing_payload_size != 0
                    && *signing_payload_size <= SIGNER_STREAM_TOKEN_MAX_PAYLOAD_BYTES_V1 as u64
            }
        }
    }
}

/// Exact current finalized state, signed by the independently configured observer.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::signer::stream_token_evidence::SignerStreamTokenStateObservationBodyV1"
)]
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct SignerStreamTokenStateObservationBodyV1 {
    /// Sole marker, available from [`Self::magic`].
    pub magic: [u8; 8],
    /// Exact digest of the independently retained canonical phase/query.
    pub request_digest: [u8; 32],
    /// Exact lifecycle phase; pre- and post-operation evidence are not interchangeable.
    pub phase: SignerStreamTokenObservationPhaseV1,
    /// Phase-compatible current or immutable completed operation subject.
    pub subject: SignerStreamTokenStateSubjectV1,
    /// Exact observer service, administrator and key/policy generation.
    pub authority: SignerCustodyAuthorityV1,
    /// Exact independently pinned chain label.
    pub chain_id: String,
    /// Exact genesis-derived network identity.
    pub network_id: [u8; 32],
    /// Time of the actual authoritative read after this query was received.
    pub observed_at_unix_ms: u64,
    /// Exclusive observation expiry under independently configured trust.
    pub expires_at_unix_ms: u64,
    /// Current finalized custody control-state anchor.
    pub current_anchor: SignerCustodyAnchorV1,
    /// Exact authoritative enrolled ACTIVE custody head.
    pub active_head: SignerCustodyActiveHeadV1,
    /// Current authoritative role-key revocation.
    pub signer_revoked: bool,
    /// Current authoritative attestation-key revocation.
    pub attester_revoked: bool,
}
impl SignerStreamTokenStateObservationBodyV1 {
    /// Sole canonical token-state marker.
    pub const fn magic() -> [u8; 8] {
        STATE_MAGIC
    }
    fn validate(&self) -> Result<(), SignerStreamTokenEvidenceErrorV1> {
        if self.magic != STATE_MAGIC
            || self.request_digest == [0; 32]
            || !self.subject.valid_phase(self.phase)
            || !valid_identity(&self.authority.service_id)
            || !valid_identity(&self.authority.administrator_id)
            || self.authority.key_revision == 0
            || self.authority.policy_revision == 0
            || self.authority.policy_digest == [0; 32]
            || iroha_primitives::chain_id::validate_chain_id(&self.chain_id).is_err()
            || self.network_id == [0; 32]
            || self.observed_at_unix_ms == 0
            || self.expires_at_unix_ms <= self.observed_at_unix_ms
            || self.expires_at_unix_ms - self.observed_at_unix_ms
                > SIGNER_STATE_OBSERVATION_MAX_AGE_MS_V1
        {
            return Err(SignerStreamTokenEvidenceErrorV1::InvalidState);
        }
        Ok(())
    }
    /// Exact domain-prefixed canonical state for the independent observer to sign.
    ///
    /// # Errors
    /// Rejects invalid variable leaves/times/subjects before output-frame counting and encoding.
    pub fn signing_payload(&self) -> Result<Vec<u8>, SignerStreamTokenEvidenceErrorV1> {
        self.validate()?;
        let frame = encode_document(
            self,
            SIGNER_STREAM_TOKEN_EVIDENCE_MAX_BYTES_V1 - STATE_DOMAIN.len(),
        )?;
        let mut payload = Vec::with_capacity(STATE_DOMAIN.len() + frame.len());
        payload.extend_from_slice(STATE_DOMAIN);
        payload.extend_from_slice(&frame);
        Ok(payload)
    }
    fn current(&self, now_unix_ms: u64) -> SignerCustodyUseContextV1 {
        SignerCustodyUseContextV1 {
            now_unix_ms,
            anchor_observed_at_unix_ms: self.observed_at_unix_ms,
            current_anchor: self.current_anchor,
            active_head: self.active_head,
            signer_revoked: self.signer_revoked,
            attester_revoked: self.attester_revoked,
        }
    }
}

/// Canonical signed token observation; its key and trust never come from this envelope.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::signer::stream_token_evidence::SignerStreamTokenStateObservationV1"
)]
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct SignerStreamTokenStateObservationV1 {
    /// Exact signed phase/query/current/completed state.
    pub body: SignerStreamTokenStateObservationBodyV1,
    /// Raw 64-byte Ed25519 observer signature.
    pub signature: [u8; 64],
}
impl SignerStreamTokenStateObservationV1 {
    /// Encode the complete signed frame after bounding every variable leaf.
    ///
    /// # Errors
    /// Rejects invalid body leaves/phase/times and oversized canonical output.
    pub fn encode_canonical(&self) -> Result<Vec<u8>, SignerStreamTokenEvidenceErrorV1> {
        self.body.validate()?;
        encode_document(self, SIGNER_STREAM_TOKEN_EVIDENCE_MAX_BYTES_V1)
    }
    /// Decode only the bounded canonical signed observation format.
    ///
    /// # Errors
    /// Rejects malformed/noncanonical/compressed/oversized frames and invalid body fields.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, SignerStreamTokenEvidenceErrorV1> {
        let value: Self = decode_document(bytes, SIGNER_STREAM_TOKEN_EVIDENCE_MAX_BYTES_V1)?;
        value.body.validate()?;
        Ok(value)
    }
}

macro_rules! redacted_evidence_debug {
    ($($name:ident),+ $(,)?) => { $(impl fmt::Debug for $name {
        fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
            out.debug_struct(stringify!($name)).finish_non_exhaustive()
        }
    })+ };
}
redacted_evidence_debug!(
    SignerStreamTokenObservationRequestSubjectV1,
    SignerStreamTokenObservationRequestV1,
    SignerStreamTokenStateSubjectV1,
    SignerStreamTokenStateObservationBodyV1,
    SignerStreamTokenStateObservationV1
);
