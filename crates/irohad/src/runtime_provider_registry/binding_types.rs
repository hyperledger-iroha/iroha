//! Exact public metadata carried by specialized runtime-provider bindings.

/// Public WebAuthn inputs accepted by the evidence-viewer provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceViewerWebAuthnBindingV1 {
    /// Canonical relying-party identifier.
    pub rp_id: String,
    /// Exact ordered canonical HTTPS origins accepted by the service.
    pub allowed_origins: Vec<String>,
    /// Maximum lifetime admitted for one issued challenge.
    pub challenge_ttl_ms: u64,
}

/// Exact public inputs accepted by the deployment-owned PoP provider registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PopCredentialRuntimeBindingV1 {
    /// Exact active finalized issuer-policy digest.
    pub issuer_policy_digest: [u8; 32],
    /// Exact governed issuer identity.
    pub issuer_id: String,
    /// Exact non-secret issuer HSM key handle.
    pub issuer_hsm_key_id: String,
    /// Exact governed issuer verification key.
    pub issuer_public_key: [u8; 32],
    /// Exact non-secret encrypted-enrollment recipient handle.
    pub enrollment_recipient_key_id: String,
    /// Exact digest of the hybrid enrollment-recipient public key.
    pub enrollment_recipient_public_key_digest: [u8; 32],
    /// Exact non-secret wallet-recipient protected-key handle.
    pub wallet_recipient_key_id: String,
    /// Exact digest of the hybrid wallet-recipient public key.
    pub wallet_recipient_public_key_digest: [u8; 32],
    /// Exact non-secret wallet wrapping-key handle.
    pub wallet_wrapping_key_id: String,
}
