//! Concrete public trust pins for the sole hardware stream-token signing path.

/// Independently administered Ed25519 authority and its configured eligibility interval.
///
/// This is public configuration, not an authenticated current-state observation or attestation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorafsStreamTokenAuthorityConfig {
    /// Exact public service identity, distinct across signer, attester and observer.
    pub service_id: String,
    /// Exact independent administrator identity.
    pub administrator_id: String,
    /// Canonical strong Ed25519 public key.
    pub public_key: [u8; 32],
    /// Nonzero authority key generation.
    pub key_revision: u64,
    /// Nonzero authority policy generation.
    pub policy_revision: u64,
    /// Exact nonzero authority policy digest.
    pub policy_digest: [u8; 32],
    /// Inclusive authority eligibility start in Unix milliseconds.
    pub active_from_unix_ms: u64,
    /// Exclusive authority eligibility end in Unix milliseconds.
    pub active_until_unix_ms: u64,
}

/// Independent hardware attester trust; actual device qualification occurs at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorafsStreamTokenAttesterConfig {
    /// Independently configured attestation authority.
    pub authority: SorafsStreamTokenAuthorityConfig,
    /// Maximum attested custody lifetime, within one day in milliseconds.
    pub max_validity_ms: u64,
    /// Maximum age of the independently observed custody anchor, within one day.
    pub max_anchor_age_ms: u64,
}

/// Independent finalized-state observer trust and public runtime routing handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorafsStreamTokenObserverConfig {
    /// Credential-free production observer handle; not a claim of hardware custody.
    pub runtime_handle: String,
    /// Independently configured observer authority.
    pub authority: SorafsStreamTokenAuthorityConfig,
    /// Maximum observation age and lifetime, within 300,000 milliseconds.
    pub max_state_age_ms: u64,
}

/// Complete public binding for one provider's hardware stream-token signing service.
///
/// The local provider comes from storage configuration, while chain and network are supplied by
/// the node context. Neither is duplicated here. No private key material or default trust is provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorafsStreamTokenHardwareConfig {
    /// Public opaque hardware runtime handle.
    pub runtime_handle: String,
    /// Public opaque nonexportable hardware key-generation handle.
    pub key_handle: String,
    /// Exact public signer service identity.
    pub service_id: String,
    /// Exact independent signer administrator identity.
    pub administrator_id: String,
    /// Canonical strong Ed25519 stream-token public key.
    pub public_key: [u8; 32],
    /// Sole key generation, within `1..=u32::MAX`; converted checked by the token issuer.
    pub key_revision: u64,
    /// Nonzero signer policy generation.
    pub policy_revision: u64,
    /// Exact nonzero signer policy digest.
    pub policy_digest: [u8; 32],
    /// Independent hardware-custody attester trust.
    pub attester: SorafsStreamTokenAttesterConfig,
    /// Independent finalized-state observer trust.
    pub observer: SorafsStreamTokenObserverConfig,
}
