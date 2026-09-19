/// Fail-closed application or recovery failure.
#[derive(Debug, Error)]
pub(crate) enum V2ApplyError {
    /// A complete committed snapshot identity could not be acquired.
    #[error(transparent)]
    SnapshotCapture(#[from] crate::snapshot::SnapshotCaptureError),
    /// Frozen wire input is malformed.
    #[error(transparent)]
    Wire(#[from] wire::ValidationError),
    /// Finality artifact is malformed.
    #[error(transparent)]
    Finality(#[from] wire::finality::V2FinalityValidationError),
    /// Frozen PoPs or the exact CommitQC failed cryptographic verification.
    #[error("invalid Sumeragi v2 durable finality cryptography: {0}")]
    FinalityCryptography(wire::finality::V2QuorumCertificateVerificationError),
    /// Exact-body loading or marker verification failed.
    #[error(transparent)]
    Body(#[from] super::v2_body_store::V2BodyStoreError),
    /// Kura persistence or canonical association failed.
    #[error(transparent)]
    Kura(#[from] crate::kura::Error),
    /// A canonical storage read failed before it could authenticate local evidence.
    #[error("Sumeragi v2 canonical storage read requires restart recovery: {0}")]
    CanonicalStorageRead(#[source] crate::kura::Error),
    /// A committed local projection could not be reconstructed/authenticated.
    /// This is not a deterministic rejection of the incoming candidate.
    #[error("Sumeragi v2 canonical State recovery failed: {0}")]
    LocalCanonicalState(String),
    /// Actual local lock contention, with an original release observation.
    #[error(transparent)]
    LocalValidationBusy(super::v2_body_store::BodyValidationBusy),
    /// Local Queue promises still need a canonical retirement/drain outcome.
    /// Their presence cannot establish that the agreed proposal is invalid.
    #[error(
        "local Queue ownership blocks retirement of lane {lane_id:?}, dataspace {dataspace_id:?}, incarnation {lane_incarnation}"
    )]
    LocalRetirementPending {
        /// Exact retiring lane.
        lane_id: LaneId,
        /// Exact retiring dataspace.
        dataspace_id: DataSpaceId,
        /// Exact retiring incarnation.
        lane_incarnation: Hash,
    },
    /// The configured local stable evidence ceiling cannot hold the exact pair.
    /// This is a configuration deficit, not current occupancy or a body verdict.
    #[error(
        "local Native AMX evidence capacity is {configured_bytes} bytes; exact pair requires {required_bytes} bytes"
    )]
    LocalEvidenceCapacity {
        /// Exact authenticated framed pair bytes.
        required_bytes: u64,
        /// This service's configured stable byte ceiling.
        configured_bytes: u64,
    },
    /// Apply task and frozen context do not identify one exact decision.
    #[error("Sumeragi v2 Apply task differs from its frozen context or body")]
    TaskMismatch,
    /// Height cannot be represented by local storage indexes.
    #[error("Sumeragi v2 decision height is not representable")]
    HeightOverflow,
    /// WSV is unexpectedly ahead of the decision.
    #[error("WSV height {state_height} is ahead of v2 decision height {decision_height}")]
    StateAhead {
        /// Current WSV height.
        state_height: usize,
        /// Decided height.
        decision_height: usize,
    },
    /// More than one unapplied height separates WSV and the decision.
    #[error("WSV height {state_height} has a gap before v2 decision height {decision_height}")]
    StateGap {
        /// Current WSV height.
        state_height: usize,
        /// Decided height.
        decision_height: usize,
    },
    /// WSV reports application but Kura has no canonical block.
    #[error("WSV is ahead of Kura while completing a Sumeragi v2 decision")]
    StateAheadOfKura,
    /// Deterministic validation rejected the exact durable body.
    #[error("Sumeragi v2 application validation failed: {0}")]
    Validation(String),
    /// Proposal ingress carried execution results or a result-root commitment.
    #[error("Sumeragi v2 proposal body must be resultless")]
    ResultBearingProposal,
    /// Deterministic validation did not produce the StateBlock execution witness.
    #[error("Sumeragi v2 validation produced no execution witness")]
    ExecutionCommitmentUnavailable,
    /// Execution-witness projection itself was malformed.
    #[error("invalid Sumeragi v2 execution commitment: {0}")]
    ExecutionCommitment(String),
    /// A proposal or executed block could not be encoded canonically.
    #[error("invalid canonical Sumeragi v2 block: {0}")]
    CanonicalBlock(String),
    /// The signed or persisted execution result differs from deterministic replay.
    #[error("Sumeragi v2 execution commitment differs from deterministic validation")]
    ExecutionCommitmentMismatch,
    /// The exact certified merge sidecar has not reached durable local storage yet.
    #[error("certified merge sidecar `{}` is not available locally yet", reference.entry_hash)]
    MissingCertifiedMergeSidecar {
        /// Compact, certificate-bound reference used for bounded recovery.
        reference: CertifiedMergeLedgerReference,
    },
    /// Certificate-aware block commit conversion failed.
    #[error("Sumeragi v2 block commit conversion failed: {0}")]
    Commit(String),
    /// Kura or WSV crossed the canonical commit point but the complete durable transition failed.
    #[error("Sumeragi v2 committed transition requires restart recovery at {stage}: {detail}")]
    CommittedRecoveryRequired {
        /// Post-commit stage that could not be completed.
        stage: &'static str,
        /// Underlying persistence diagnostic.
        detail: String,
    },
    /// Test-only crash boundary after Kura commits and before WSV publication.
    #[cfg(test)]
    #[error("injected crash after Kura store and before WSV commit")]
    InjectedCrashAfterKuraStore,
    /// Test-only crash boundary between staged WSV checkpoint and State publication.
    #[cfg(test)]
    #[error("injected crash after staged WSV checkpoint and before WSV commit")]
    InjectedCrashAfterWsvCheckpoint,
    /// Test-only crash boundary between provider-ingest archive and State publication.
    #[cfg(test)]
    #[error("injected crash after provider-ingest archive capture and before WSV commit")]
    InjectedCrashAfterProviderIngestArchiveCapture,
    /// Test-only crash boundary between reputation archive and State publication.
    #[cfg(test)]
    #[error("injected crash after reputation archive capture and before WSV commit")]
    InjectedCrashAfterReputationArchiveCapture,
}
impl V2ApplyError {
    fn committed_recovery_required(stage: &'static str, error: &impl std::fmt::Display) -> Self {
        Self::CommittedRecoveryRequired {
            stage,
            detail: error.to_string(),
        }
    }
    /// Return whether the live consensus process must stop producing output until restart.
    #[must_use]
    pub(crate) const fn requires_restart_recovery(&self) -> bool {
        match self {
            Self::Kura(error) => error.requires_restart_recovery(),
            Self::CommittedRecoveryRequired { .. }
            | Self::CanonicalStorageRead(_)
            | Self::LocalCanonicalState(_)
            | Self::LocalRetirementPending { .. }
            | Self::LocalEvidenceCapacity { .. }
            | Self::StateAheadOfKura => true,
            #[cfg(test)]
            Self::InjectedCrashAfterKuraStore
            | Self::InjectedCrashAfterWsvCheckpoint
            | Self::InjectedCrashAfterProviderIngestArchiveCapture
            | Self::InjectedCrashAfterReputationArchiveCapture => true,
            _ => false,
        }
    }
}
impl BodyValidationError for V2ApplyError {
    fn rejection_identity(&self) -> Option<super::v2_body_store::BodyValidationRejectionIdentity> {
        use super::v2_body_store::BodyValidationRejectionIdentity;
        // Keep this exhaustive: new service errors cannot silently acquire a
        // durable negative vote. In particular Kura/State/receipt failures say
        // nothing about the semantic validity of the authenticated body.
        match self {
            Self::Validation(_)
            | Self::ResultBearingProposal
            | Self::ExecutionCommitmentMismatch => Some(BodyValidationRejectionIdentity::Rejected),
            Self::SnapshotCapture(_)
            | Self::Wire(_)
            | Self::Finality(_)
            | Self::FinalityCryptography(_)
            | Self::Body(_)
            | Self::Kura(_)
            | Self::CanonicalStorageRead(_)
            | Self::LocalCanonicalState(_)
            | Self::LocalValidationBusy(_)
            | Self::LocalRetirementPending { .. }
            | Self::LocalEvidenceCapacity { .. }
            | Self::TaskMismatch
            | Self::HeightOverflow
            | Self::StateAhead { .. }
            | Self::StateGap { .. }
            | Self::StateAheadOfKura
            | Self::ExecutionCommitmentUnavailable
            | Self::ExecutionCommitment(_)
            | Self::CanonicalBlock(_)
            | Self::MissingCertifiedMergeSidecar { .. }
            | Self::Commit(_)
            | Self::CommittedRecoveryRequired { .. } => None,
            #[cfg(test)]
            Self::InjectedCrashAfterKuraStore
            | Self::InjectedCrashAfterWsvCheckpoint
            | Self::InjectedCrashAfterProviderIngestArchiveCapture
            | Self::InjectedCrashAfterReputationArchiveCapture => None,
        }
    }

    fn missing_certified_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
        match self {
            Self::MissingCertifiedMergeSidecar { reference } => Some(reference),
            _ => None,
        }
    }

    fn local_busy(&self) -> Option<&super::v2_body_store::BodyValidationBusy> {
        match self {
            Self::LocalValidationBusy(busy) => Some(busy),
            _ => None,
        }
    }
}

impl From<super::lane_planner::V2LanePayloadPlanError> for V2ApplyError {
    fn from(error: super::lane_planner::V2LanePayloadPlanError) -> Self {
        if error.is_storage_error() {
            Self::LocalCanonicalState(error.to_string())
        } else {
            Self::Validation(error.to_string())
        }
    }
}
