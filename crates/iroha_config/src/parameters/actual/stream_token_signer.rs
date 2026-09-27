//! Concrete public trust pins for the stream-token signing path.

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

/// Independent signer attester trust; authenticated custody is verified at runtime.
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
    /// Credential-free production observer handle; not an authorization claim.
    pub runtime_handle: String,
    /// Independently configured observer authority.
    pub authority: SorafsStreamTokenAuthorityConfig,
    /// Maximum observation age and lifetime, within 300,000 milliseconds.
    pub max_state_age_ms: u64,
}

/// Complete public binding for one provider's stream-token signing service.
///
/// The local provider comes from storage configuration, while chain and network are supplied by
/// the node context. Neither is duplicated here. No private key material or default trust is provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorafsStreamTokenSignerConfig {
    /// Closed UTC eligibility uncertainty on each side of the host's fresh clock sample.
    pub clock_uncertainty_ms: u64,
    /// Explicit native software runtime; absent selects externally supplied adapters.
    pub native: Option<SorafsStreamTokenNativeConfig>,
    /// Public opaque software or hardware runtime handle.
    pub runtime_handle: String,
    /// Public opaque key-generation handle for the configured provider.
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
    /// Independent signer-custody attester trust.
    pub attester: SorafsStreamTokenAttesterConfig,
    /// Independent finalized-state observer trust.
    pub observer: SorafsStreamTokenObserverConfig,
}

/// Explicit local software custody for the native State/queue stream-token runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorafsStreamTokenNativeConfig {
    /// Owner-only role-11 private-key credential, in the canonical runtime credential format.
    pub signer_credential: std::path::PathBuf,
    /// Bounded signed custody record; this file is material, never state authority.
    pub custody_record: std::path::PathBuf,
    /// Existing owner-only private completed-receipt directory.
    pub receipt_journal: std::path::PathBuf,
    /// Exact registered provider-owner transaction account.
    pub operator: iroha_data_model::account::AccountId,
    /// Independent owner-only operator transaction key credential.
    pub operator_credential: std::path::PathBuf,
    /// Independent owner-only observer key credential, matching public observer trust.
    pub observer_credential: std::path::PathBuf,
    /// Explicit fee approval used for each native operator and Check transaction.
    pub fee_payment: iroha_data_model::transaction::FeePaymentIntent,
    /// Complete bounded native transaction observation deadline.
    pub timeout_ms: u64,
}
