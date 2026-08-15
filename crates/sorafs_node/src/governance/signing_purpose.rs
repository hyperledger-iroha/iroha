/// Exact Governance DAG payload class presented to the runtime signer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GovernanceDagSigningPurposeV1 {
    /// Canonical governance log-node signature payload.
    LogNode = 1,
    /// Canonical Governance DAG block signature payload.
    DagBlock = 2,
    /// Canonical Governance DAG head signature payload.
    DagHead = 3,
    /// Predecessor-bound signer/store qualification transition.
    KeyTransition = 4,
    /// Signed immutable qualification-history archive.
    QualificationArchive = 5,
}
impl GovernanceDagSigningPurposeV1 {
    /// Immutable V1 wire identifier.
    #[must_use]
    pub const fn wire_id(self) -> u8 {
        self as u8
    }
    /// Decode one immutable V1 wire identifier without aliases.
    #[must_use]
    pub const fn try_from_wire_id(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::LogNode),
            2 => Some(Self::DagBlock),
            3 => Some(Self::DagHead),
            4 => Some(Self::KeyTransition),
            5 => Some(Self::QualificationArchive),
            _ => None,
        }
    }
}
/// Validate one exact purpose-separated Governance DAG control payload.
pub fn validate_governance_dag_control_signing_payload_v1(
    purpose: GovernanceDagSigningPurposeV1,
    payload: &[u8],
    publisher_peer_id: &[u8],
    publisher_public_key: [u8; 32],
) -> Result<(), GovernancePublishError> {
    match purpose {
        GovernanceDagSigningPurposeV1::KeyTransition => {
            let bytes = payload
                .strip_prefix(b"sorafs.governance-dag.key-transition.v1\0")
                .ok_or_else(|| {
                    GovernancePublishError::other("invalid governance key transition")
                })?;
            let decoded: RuntimeDagKeyTransitionSigningPayloadV1 = norito::decode_canonical(bytes)
                .map_err(|_| GovernancePublishError::other("invalid governance key transition"))?;
            if decoded.version != GOVERNANCE_RUNTIME_DAG_KEY_TRANSITION_VERSION_V1
                || decoded.outgoing_segment_revision == 0
                || decoded.incoming_segment_revision
                    != decoded
                        .outgoing_segment_revision
                        .checked_add(1)
                        .unwrap_or(0)
                || decoded.transition_body_digest == [0; 32]
            {
                return Err(GovernancePublishError::other(
                    "invalid governance key transition",
                ));
            }
        }
        GovernanceDagSigningPurposeV1::QualificationArchive => {
            let bytes = payload
                .strip_prefix(b"sorafs.governance-dag.qualification-archive.v1\0")
                .ok_or_else(|| {
                    GovernancePublishError::other("invalid governance qualification archive")
                })?;
            let body: RuntimeDagQualificationArchiveBodyV1 = norito::decode_canonical(bytes)
                .map_err(|_| {
                    GovernancePublishError::other("invalid governance qualification archive")
                })?;
            validate_runtime_dag_provider_binding(&body.signer)?;
            if body.version != GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_VERSION_V1
                || body.root_digest == [0; 32]
                || body.archive_generation == 0
                || body.archive_generation
                    > GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_CHAIN_MAX_V1
                || body.transitions.is_empty()
                || body.transitions.len()
                    > GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVE_MAX_TRANSITIONS_V1
                || body.signer.publisher_peer_id != publisher_peer_id
                || body.signer.publisher_public_key != publisher_public_key
            {
                return Err(GovernancePublishError::other(
                    "invalid governance qualification archive",
                ));
            }
            let mut expected = body.first_transition_generation;
            for transition in &body.transitions {
                validate_runtime_dag_qualification_transition(transition, body.root_digest)?;
                if transition.body.generation != expected {
                    return Err(GovernancePublishError::other(
                        "invalid governance qualification archive",
                    ));
                }
                expected = expected.checked_add(1).ok_or_else(|| {
                    GovernancePublishError::other("invalid governance qualification archive")
                })?;
            }
            if expected.checked_sub(1) != Some(body.last_transition_generation)
                || runtime_dag_transition_digest(
                    body.transitions
                        .last()
                        .expect("non-empty archive transitions"),
                )? != body.tail_transition_digest
            {
                return Err(GovernancePublishError::other(
                    "invalid governance qualification archive",
                ));
            }
        }
        _ => {
            return Err(GovernancePublishError::other(
                "invalid governance control purpose",
            ));
        }
    }
    Ok(())
}
/// Runtime-only signing boundary for the local Governance DAG publisher.
///
/// Production implementations delegate to an authenticated external signer.
/// Private key bytes must never be returned to the caller, persisted below the
/// publisher root, or sourced from [`iroha_config`](iroha_config).
pub trait GovernanceDagRuntimeSigner: Send + Sync + fmt::Debug {
    /// Opaque, non-secret deployment handle for this signer.
    fn handle(&self) -> &str;
    /// Qualify the active adapter and its public policy revision.
    ///
    /// Implementations must fail when the external-signer adapter is unavailable, revoked, stale,
    /// test-marked, or otherwise not production-ready. Provider diagnostics can contain secrets and
    /// are therefore always redacted by the caller.
    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String>;
    /// Governed publisher peer identity bound to this signer.
    fn publisher_peer_id(&self) -> &[u8];
    /// Raw Ed25519 public key bound to the opaque handle.
    fn public_key(&self) -> [u8; 32];
    /// Sign one exact canonical Governance DAG payload.
    ///
    /// Implementations must not include credentials or provider diagnostics in the returned error.
    /// This crate nevertheless redacts every provider error at the trust boundary.
    fn sign(
        &self,
        purpose: GovernanceDagSigningPurposeV1,
        payload: &[u8],
    ) -> Result<[u8; 64], String>;
}
