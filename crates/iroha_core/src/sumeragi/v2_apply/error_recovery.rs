/// Fail-closed application or recovery failure.
#[derive(Debug, Error)]
pub(crate) enum V2ApplyError {
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
    /// Kura already contains a different block at the decided height.
    #[error("Kura contains a conflicting block at the Sumeragi v2 decision height")]
    KuraConflict,
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
            Self::CommittedRecoveryRequired { .. } => true,
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
    fn missing_certified_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
        match self {
            Self::MissingCertifiedMergeSidecar { reference } => Some(reference),
            _ => None,
        }
    }
}
