//! Public remote-attachment requests and status, distinct from local validator readiness.

use super::{ManagedAttachmentFailure, ManagedStatus};
use iroha_data_model::private_dataspace::PrivateDataspaceCursor;
use norito::json::{JsonDeserialize, JsonSerialize};
use std::time::Duration;

/// One installed-network private dataspace operation under a finite total budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataspaceRequest {
    /// Store-local context name.
    pub name: String,
    /// Independently installed network profile name.
    pub network: String,
    /// Exact canonical SNS dataspace alias.
    pub alias: String,
    /// Exact canonical owner label, leased as `label@alias` in the same paid request.
    pub account_alias: String,
    /// Total bootstrap, private startup and attachment budget.
    pub timeout: Duration,
}

/// Progress of the outbound parent workflow; local readiness is reported separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagedAttachmentPhase {
    /// Authenticating the installed parent and current native quorum.
    Connecting,
    /// Recovering the exact signed funding operation.
    Funding,
    /// Recovering the exact paid namespace lease.
    Namespace,
    /// Recovering the parent registration and its independent receipt.
    Registering,
    /// Observing or relaying certificates for an already registered private root.
    Anchoring,
    /// A native parent receipt has been independently verified and retained.
    Attached,
    /// Current parent work could not complete; retained receipts remain historical evidence.
    Unavailable,
}

impl ManagedAttachmentPhase {
    /// Stable public spelling used by CLI and desktop status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Funding => "funding",
            Self::Namespace => "namespace",
            Self::Registering => "registering",
            Self::Anchoring => "anchoring",
            Self::Attached => "attached",
            Self::Unavailable => "unavailable",
        }
    }
}
impl JsonSerialize for ManagedAttachmentPhase {
    fn json_serialize(&self, output: &mut String) {
        self.as_str().json_serialize(output);
    }
}
impl std::fmt::Display for ManagedAttachmentPhase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
impl JsonDeserialize for ManagedAttachmentPhase {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        match parser.parse_string()?.as_str() {
            "connecting" => Ok(Self::Connecting),
            "funding" => Ok(Self::Funding),
            "namespace" => Ok(Self::Namespace),
            "registering" => Ok(Self::Registering),
            "anchoring" => Ok(Self::Anchoring),
            "attached" => Ok(Self::Attached),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(norito::json::Error::Message(
                "invalid managed attachment phase".into(),
            )),
        }
    }
}

/// Historical independently verified parent inclusion, never a current-liveness claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ManagedConfirmedAnchor {
    /// Original certified parent carrier height.
    pub parent_height: u64,
    /// Exact certified child decision included by that carrier.
    pub child: PrivateDataspaceCursor,
}

/// Public parent workflow progress without owner credentials or private block bodies.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ManagedAttachmentStatus {
    /// Independently installed parent profile name.
    pub network: String,
    /// Current operation stage, preserved when the latest attempt fails.
    pub stage: ManagedAttachmentPhase,
    /// Exact native wallet operation status, if an operation was observed.
    pub wallet_status: Option<String>,
    /// Last observed native child successor; this does not establish parent inclusion.
    pub local_successor: Option<PrivateDataspaceCursor>,
    /// Last independently verified original parent receipt, retained across unavailable turns.
    pub parent_confirmed: Option<ManagedConfirmedAnchor>,
    /// Closed failure classification with static public text and no private content.
    pub failure: Option<ManagedAttachmentFailure>,
}

/// Local native process observation and independently tracked outbound parent attachment.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ManagedDataspaceStatus {
    /// Authenticated local worker observation.
    pub local: ManagedStatus,
    /// Parent workflow evidence; historical cursors never imply fresh parent readiness.
    pub attachment: ManagedAttachmentStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_status_roundtrips_closed_phases_without_inventing_a_receipt() {
        for stage in [
            ManagedAttachmentPhase::Connecting,
            ManagedAttachmentPhase::Funding,
            ManagedAttachmentPhase::Namespace,
            ManagedAttachmentPhase::Registering,
            ManagedAttachmentPhase::Anchoring,
            ManagedAttachmentPhase::Attached,
            ManagedAttachmentPhase::Unavailable,
        ] {
            let status = ManagedAttachmentStatus {
                network: "installed".into(),
                stage,
                wallet_status: None,
                local_successor: None,
                parent_confirmed: None,
                failure: None,
            };
            let bytes = norito::json::to_vec(&status).unwrap();
            assert_eq!(
                norito::json::from_slice::<ManagedAttachmentStatus>(&bytes).unwrap(),
                status
            );
            assert_eq!(
                norito::json::from_str::<ManagedAttachmentPhase>(&format!(
                    "\"{}\"",
                    stage.as_str()
                ))
                .unwrap(),
                stage
            );
        }
        assert!(norito::json::from_str::<ManagedAttachmentPhase>("\"ready\"").is_err());
        assert!(norito::json::from_str::<ManagedAttachmentStatus>(r#"{"network":"installed","stage":"connecting","wallet_status":null,"local_successor":null,"parent_confirmed":null,"failure":null,"trusted":true}"#).is_err());
    }
}
