//! Closed public attachment diagnostics; underlying errors never cross this boundary.

use super::Error;
use crate::{
    attachment::AttachmentError, bootstrap::BootstrapError, provisioning::ProvisioningError,
    verify::finality::FinalityError,
};
use norito::json::{JsonDeserialize, JsonSerialize};

/// Stable classification of incomplete parent work, never evidence of a committed operation.
/// Messages are fixed locally; response bodies, credentials and custody paths are excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ManagedAttachmentFailure {
    /// Request construction or preparation failed before completion could be observed.
    #[error("operation preparation failed; retry the exact retained request")]
    PreparationFailed,
    /// Submission or recovery failed; the journal still owns any potentially submitted request.
    #[error("operation recovery failed; retain and reconcile the original journal")]
    RecoveryFailed,
    /// The retained signed operation expired and cannot be extended or signed again.
    #[error("retained signed operation expired; its deadline cannot be extended")]
    OperationExpired,
    /// The exact retained operation has a canonical terminal rejection.
    #[error("retained signed operation was rejected; preserve its journal for diagnosis")]
    OperationRejected,
    /// A bounded namespace rent quote could not be obtained.
    #[error("namespace quote unavailable; retry the exact retained namespace")]
    QuoteUnavailable,
    /// A current SNS lease could not be independently authenticated.
    #[error("current namespace lease is unverified; retry from fresh parent quorum")]
    NamespaceUnverified,
    /// Exact retained identity, policy or authorization did not validate.
    #[error("retained attachment identity or policy was rejected; inspect the selected context")]
    ContextRejected,
    /// Native private custody could not be opened or published.
    #[error("private operation custody is unavailable; retain the existing context and journals")]
    CustodyUnavailable,
    /// An installed release could not be authenticated under its independent trust inputs.
    #[error("installed parent authentication failed; inspect the selected network profile")]
    ParentAuthenticationFailed,
    /// Transport or fresh quorum observation did not complete.
    #[error("fresh parent observation is unavailable; retry the retained context")]
    ParentUnavailable,
    /// A certified prefix advanced but a fresh independent parent observation is still required.
    #[error("parent finality catch-up is incomplete; retained certified progress will be resumed")]
    ParentCatchingUp,
    /// Native evidence verification rejected the supplied material.
    #[error("parent or child evidence was rejected; no new parent receipt is established")]
    EvidenceRejected,
    /// A finite native observation bound was reached.
    #[error("parent observation reached its resource bound; retry the retained context")]
    ObservationLimit,
    /// A relay operation failed without establishing completion.
    #[error("parent relay operation failed; reconcile the exact retained operation")]
    RelayIncomplete,
    /// Foreground time elapsed before the exact operation was independently confirmed.
    #[error("operation completion is not yet verified; inspect status and retry the same context")]
    AwaitingCompletion,
    /// The local supervisor stopped; retained receipts remain historical.
    #[error("local supervisor is stopped; parent receipts are historical")]
    SupervisorStopped,
    /// The outbound owner could not start or has stopped.
    #[error("parent attachment worker is unavailable; retry the retained context")]
    WorkerUnavailable,
}

impl ManagedAttachmentFailure {
    /// Stable JSON and automation code; its human explanation is available through `Display`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreparationFailed => "preparation_failed",
            Self::RecoveryFailed => "recovery_failed",
            Self::OperationExpired => "operation_expired",
            Self::OperationRejected => "operation_rejected",
            Self::QuoteUnavailable => "quote_unavailable",
            Self::NamespaceUnverified => "namespace_unverified",
            Self::ContextRejected => "context_rejected",
            Self::CustodyUnavailable => "custody_unavailable",
            Self::ParentAuthenticationFailed => "parent_authentication_failed",
            Self::ParentUnavailable => "parent_unavailable",
            Self::ParentCatchingUp => "parent_catching_up",
            Self::EvidenceRejected => "evidence_rejected",
            Self::ObservationLimit => "observation_limit",
            Self::RelayIncomplete => "relay_incomplete",
            Self::AwaitingCompletion => "awaiting_completion",
            Self::SupervisorStopped => "supervisor_stopped",
            Self::WorkerUnavailable => "worker_unavailable",
        }
    }

    /// Whether retrying the immutable signed operation cannot advance this workflow.
    pub(super) const fn is_terminal_operation(self) -> bool {
        matches!(self, Self::OperationExpired | Self::OperationRejected)
    }
}

impl JsonSerialize for ManagedAttachmentFailure {
    fn json_serialize(&self, output: &mut String) {
        self.as_str().json_serialize(output);
    }
}

impl JsonDeserialize for ManagedAttachmentFailure {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        match parser.parse_string()?.as_str() {
            "preparation_failed" => Ok(Self::PreparationFailed),
            "recovery_failed" => Ok(Self::RecoveryFailed),
            "operation_expired" => Ok(Self::OperationExpired),
            "operation_rejected" => Ok(Self::OperationRejected),
            "quote_unavailable" => Ok(Self::QuoteUnavailable),
            "namespace_unverified" => Ok(Self::NamespaceUnverified),
            "context_rejected" => Ok(Self::ContextRejected),
            "custody_unavailable" => Ok(Self::CustodyUnavailable),
            "parent_authentication_failed" => Ok(Self::ParentAuthenticationFailed),
            "parent_unavailable" => Ok(Self::ParentUnavailable),
            "parent_catching_up" => Ok(Self::ParentCatchingUp),
            "evidence_rejected" => Ok(Self::EvidenceRejected),
            "observation_limit" => Ok(Self::ObservationLimit),
            "relay_incomplete" => Ok(Self::RelayIncomplete),
            "awaiting_completion" => Ok(Self::AwaitingCompletion),
            "supervisor_stopped" => Ok(Self::SupervisorStopped),
            "worker_unavailable" => Ok(Self::WorkerUnavailable),
            _ => Err(norito::json::Error::Message(
                "invalid managed attachment failure code".into(),
            )),
        }
    }
}

impl From<ProvisioningError> for ManagedAttachmentFailure {
    fn from(error: ProvisioningError) -> Self {
        match error {
            ProvisioningError::FaucetPreparation | ProvisioningError::NamespacePreparation => {
                Self::PreparationFailed
            }
            ProvisioningError::FaucetRecovery | ProvisioningError::NamespaceRecovery => {
                Self::RecoveryFailed
            }
            ProvisioningError::NamespaceQuote => Self::QuoteUnavailable,
            ProvisioningError::NamespaceObservation => Self::NamespaceUnverified,
            ProvisioningError::Deadline => Self::AwaitingCompletion,
            ProvisioningError::Io(_) => Self::CustodyUnavailable,
            ProvisioningError::Invalid(_) => Self::ContextRejected,
            ProvisioningError::Bootstrap(error) => error.into(),
            ProvisioningError::Attachment(error) => error.into(),
        }
    }
}

impl From<BootstrapError> for ManagedAttachmentFailure {
    fn from(error: BootstrapError) -> Self {
        match error {
            BootstrapError::Io(_) | BootstrapError::Busy => Self::CustodyUnavailable,
            BootstrapError::Invalid(_) => Self::ParentAuthenticationFailed,
            BootstrapError::Finality(error) => error.into(),
        }
    }
}

impl From<AttachmentError> for ManagedAttachmentFailure {
    fn from(error: AttachmentError) -> Self {
        match error {
            AttachmentError::Io(_) => Self::CustodyUnavailable,
            AttachmentError::Invalid(_) => Self::ContextRejected,
            AttachmentError::Proof(_) => Self::EvidenceRejected,
            AttachmentError::Finality(error) => error.into(),
            AttachmentError::Bootstrap(error) => error.into(),
            AttachmentError::Operation(_) => Self::RelayIncomplete,
        }
    }
}

impl From<FinalityError> for ManagedAttachmentFailure {
    fn from(error: FinalityError) -> Self {
        match error {
            FinalityError::CatchingUp { .. } => Self::ParentCatchingUp,
            FinalityError::Source(_) | FinalityError::InsufficientAttestations(_) => {
                Self::ParentUnavailable
            }
            FinalityError::ResourceLimit(_) => Self::ObservationLimit,
            _ => Self::EvidenceRejected,
        }
    }
}

impl From<Error> for ManagedAttachmentFailure {
    fn from(error: Error) -> Self {
        match error {
            Error::Io(_) | Error::Busy(_) => Self::CustodyUnavailable,
            Error::ParentDeadline | Error::Timeout(_) => Self::AwaitingCompletion,
            Error::ParentProgressDeadline { failure, .. } => failure,
            // A retained service bootstrap that cannot advance under this authorization
            // invalidates the selected context; it is never a parent completion signal.
            Error::NoSelection | Error::Invalid(_) | Error::Bootstrap(_) => Self::ContextRejected,
        }
    }
}

impl From<std::io::Error> for ManagedAttachmentFailure {
    fn from(_: std::io::Error) -> Self {
        Self::CustodyUnavailable
    }
}

#[cfg(test)]
#[path = "remote_failure_tests.rs"]
mod tests;
