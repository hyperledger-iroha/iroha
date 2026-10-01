//! Explicit software custody for the consensus-owned gateway admission runtime.

/// Native gateway transaction credentials and independent UTC eligibility policy.
///
/// These are configuration pins, never current-state or finality evidence. Credential files are
/// opened through the owner-only runtime credential reader; private key bytes are not configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorafsStreamTokenGatewayNativeConfig {
    /// Registered direct Ed25519 policy operator with the scoped operation permission.
    pub operator: iroha_data_model::account::AccountId,
    /// Owner-only canonical operator private-key credential.
    pub operator_credential: std::path::PathBuf,
    /// Independent registered direct Ed25519 observer with the scoped Check permission.
    pub observer: iroha_data_model::account::AccountId,
    /// Distinct owner-only canonical observer private-key credential.
    pub observer_credential: std::path::PathBuf,
    /// Independent registered direct Ed25519 recorder for governed reputation Append intents.
    pub reputation_recorder: iroha_data_model::account::AccountId,
    /// Distinct owner-only canonical recorder private-key credential.
    pub reputation_recorder_credential: std::path::PathBuf,
    /// Explicit fee approval for operator and observer transactions; Append fees are governed.
    pub fee_payment: iroha_data_model::transaction::FeePaymentIntent,
    /// Closed UTC eligibility uncertainty on either side of each fresh host-clock sample.
    pub clock_uncertainty_ms: u64,
}
