//! SoraDNS resolver attestation data structures.
//!
//! This module defines the Resolver Attestation Document (RAD) schema along
//! with the directory records and pub/sub event payloads outlined in the
//! SoraDNS roadmap. These types are shared between governance tooling, Torii
//! APIs, SDKs, and the resolver implementation so all components agree on the
//! canonical Norito encoding.

use iroha_crypto::{PublicKey, Signature};
use iroha_primitives::soradns::GatewayHostBindings;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{account::AccountId, ipfs::IpfsPath};

/// Current RAD schema version identifier.
pub const RAD_VERSION_V1: u8 = 1;
/// Current directory record schema version identifier.
pub const DIRECTORY_RECORD_VERSION_V1: u16 = 1;

/// Canonical resolver identifier (BLAKE3 hash of resolver public key + FQDN).
pub type ResolverId = [u8; 32];
/// Canonical resolver directory identifier (Merkle root of the RAD set).
pub type DirectoryId = [u8; 32];

/// Deterministic host bindings derived from a resolver's canonical FQDN.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct GatewayHostSet {
    /// Base32-encoded BLAKE3 label used for canonical hosts.
    pub canonical_label: String,
    /// Canonical host (`<hash>.gw.sora.id`).
    pub canonical_host: String,
    /// Wildcard pattern authorised for the canonical host.
    pub canonical_wildcard: String,
    /// Pretty host (`<fqdn>.gw.sora.name`).
    pub pretty_host: String,
}

impl GatewayHostSet {
    /// Returns `true` if the supplied hostname matches the canonical or pretty host.
    #[must_use]
    pub fn matches(&self, host: &str) -> bool {
        let candidate = host.trim().to_ascii_lowercase();
        candidate == self.canonical_host || candidate == self.pretty_host
    }
}

impl From<&GatewayHostBindings> for GatewayHostSet {
    fn from(bindings: &GatewayHostBindings) -> Self {
        Self {
            canonical_label: bindings.canonical_label().to_string(),
            canonical_host: bindings.canonical_host().to_string(),
            canonical_wildcard: GatewayHostBindings::canonical_wildcard().to_string(),
            pretty_host: bindings.pretty_host().to_string(),
        }
    }
}

/// Transport capabilities and endpoints exposed by a resolver.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ResolverTransportBundle {
    /// HTTPS `DoH` endpoint (RFC8484).
    pub doh: Option<HttpTransportV1>,
    /// TLS tunnel endpoint for `DoT` (RFC7858).
    pub dot: Option<TlsTransportV1>,
    /// QUIC-powered `DoQ` endpoint (draft-ietf-dprive-dnsoquic).
    pub doq: Option<QuicTransportV1>,
    /// Optional Oblivious `DoH` relay preview configuration.
    pub odoh_relay: Option<OdohRelayV1>,
    /// Optional `SoraNet` proxy bridge allowing anonymous requests.
    pub soranet_bridge: Option<SoranetBridgeConfigV1>,
    /// Whether QNAME minimisation is enforced.
    pub qname_minimisation: bool,
    /// Padding configuration advertised to clients.
    pub padding_policy: PaddingPolicyV1,
}

/// HTTP-based DNS transport metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct HttpTransportV1 {
    /// Fully-qualified HTTPS endpoint.
    pub endpoint: String,
    /// Support for GET queries (RFC8484 §4.1.1).
    pub supports_get: bool,
    /// Support for POST queries (RFC8484 §4.1.2).
    pub supports_post: bool,
    /// Maximum response size (bytes) advertised for padding heuristics.
    pub max_response_bytes: u32,
}

/// TLS-based DNS transport metadata (`DoT`/`DoH3`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TlsTransportV1 {
    /// Endpoint (e.g., `tls://resolver.sora.net:853`).
    pub endpoint: String,
    /// Supported ALPN identifiers.
    pub alpn_protocols: Vec<String>,
    /// Supported cipher suites (IANA names).
    pub cipher_suites: Vec<String>,
}

/// QUIC-based DNS transport metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct QuicTransportV1 {
    /// Endpoint (e.g., `quic://resolver.sora.net:784`).
    pub endpoint: String,
    /// Whether the resolver enforces retry tokens.
    pub supports_retry: bool,
    /// Optional congestion-control profile identifier.
    pub congestion_profile: Option<String>,
}

/// Oblivious `DoH` relay preview metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OdohRelayV1 {
    /// Relay endpoint.
    pub endpoint: String,
    /// Key identifier advertised alongside the `ODoH` public key.
    pub key_id: String,
    /// `ODoH` public key material (HPKE).
    pub public_key: Vec<u8>,
}

/// Configuration for bridging requests over `SoraNet` circuits.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SoranetBridgeConfigV1 {
    /// Multiaddr entry point (e.g., `/dns4/resolver.sora.net/tcp/7000/quic`).
    pub multiaddr: String,
    /// Policy label describing required guard/pinning behaviour.
    pub circuit_policy: String,
}

/// Padding configuration applied to DNS responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PaddingPolicyV1 {
    /// Minimum padding bytes appended to each response.
    pub min_bytes: u16,
    /// Maximum padding bytes appended to each response.
    pub max_bytes: u16,
    /// Alignment boundary applied after padding.
    pub pad_to_block: u16,
}

/// TLS provisioning material associated with a resolver.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ResolverTlsBundle {
    /// ACME/issuance profiles authorised for the resolver.
    pub provisioning_profiles: Vec<TlsProvisioningProfile>,
    /// SHA-256 fingerprints of the deployed certificate chain.
    pub certificate_fingerprints: Vec<String>,
    /// Wildcard host patterns covered by the certificate chain.
    pub wildcard_hosts: Vec<String>,
    /// Timestamp (unix seconds) when the certificate chain expires.
    pub not_after_unix: u64,
}

/// Supported TLS provisioning strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "profile", content = "payload")]
pub enum TlsProvisioningProfile {
    /// ACME DNS-01 issuance.
    Dns01,
    /// ACME TLS-ALPN-01 issuance.
    TlsAlpn01,
    /// Custom provisioning (manually issued certificates).
    Custom,
}

/// Rotation policy attached to a resolver attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RotationPolicyV1 {
    /// Maximum allowed lifetime for a RAD (days).
    pub max_lifetime_days: u16,
    /// Required overlap window (seconds) between staged replacements.
    pub required_overlap_seconds: u32,
    /// Whether dual signatures (operator + governance) are mandatory.
    pub require_dual_signatures: bool,
}

/// Resolver Attestation Document (RAD) signed by the operator and governance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ResolverAttestationDocumentV1 {
    /// RAD schema version (currently `RAD_VERSION_V1`).
    pub version: u8,
    /// Deterministic resolver identifier (`blake3(public_key || fqdn)`).
    pub resolver_id: [u8; 32],
    /// Fully-qualified domain name (punycode-normalised).
    pub fqdn: String,
    /// Deterministic gateway hosts.
    pub canonical_hosts: GatewayHostSet,
    /// Transport metadata.
    pub transport: ResolverTransportBundle,
    /// TLS provisioning metadata.
    pub tls: ResolverTlsBundle,
    /// Hash of the resolver deployment manifest (BLAKE3).
    pub resolver_manifest_hash: [u8; 32],
    /// Hash of the GAR manifest used for gateways (BLAKE3).
    pub gar_manifest_hash: [u8; 32],
    /// Issuance timestamp (unix seconds).
    pub issued_at_unix: u64,
    /// Start of the validity window.
    pub valid_from_unix: u64,
    /// End of the validity window.
    pub valid_until_unix: u64,
    /// Resolver operator account ID (ledger identity).
    pub operator_account: AccountId,
    /// Operator signature over the RAD payload.
    pub operator_signature: Signature,
    /// Governance council signature over the RAD payload.
    pub governance_signature: Signature,
    /// Rotation policy instructions.
    pub rotation_policy: RotationPolicyV1,
    /// Optional telemetry endpoint for health reporting.
    pub telemetry_endpoint: Option<String>,
}

/// Directory record anchoring the active RAD set in the ledger.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ResolverDirectoryRecordV1 {
    /// Merkle root hash of the active RAD set.
    pub root_hash: DirectoryId,
    /// Record schema version (maps to RAD schema).
    pub record_version: u16,
    /// Timestamp (milliseconds since Unix epoch) when the bundle was created.
    pub created_at_ms: u64,
    /// Total number of RAD entries included in this directory.
    pub rad_count: u32,
    /// SHA-256 digest of the canonical `directory.json` artifact.
    pub directory_json_sha256: [u8; 32],
    /// Optional previous directory root for audit history.
    #[norito(default)]
    pub previous_root: Option<[u8; 32]>,
    /// Block height when the record was published.
    pub published_at_block: u64,
    /// Publication timestamp (unix seconds).
    pub published_at_unix: u64,
    /// `SoraFS` manifest CID containing the RAD bundle + proofs.
    pub proof_manifest_cid: IpfsPath,
    /// Release engineer public key that produced the builder signature.
    pub builder_public_key: PublicKey,
    /// Release engineer signature over the canonical record payload.
    pub builder_signature: Signature,
}

/// Pending resolver directory draft awaiting council publication.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PendingDirectoryDraftV1 {
    /// Directory record submitted for approval.
    pub record: ResolverDirectoryRecordV1,
    /// CID referencing the CAR bundle with RAD artifacts.
    pub car_cid: IpfsPath,
    /// SHA-256 digest of the canonical directory.json artifact.
    pub directory_json_sha256: [u8; 32],
    /// Builder key included in the submission.
    pub builder_public_key: PublicKey,
    /// Builder signature covering the record payload.
    pub builder_signature: Signature,
    /// Submission timestamp recorded by the host (unix milliseconds).
    pub submitted_at_ms: u64,
}

/// Record describing a resolver revocation hotfix entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ResolverRevocationRecordV1 {
    /// Resolver identifier that was revoked.
    pub resolver_id: ResolverId,
    /// Reason recorded for the revocation.
    pub reason: RadRevokeReason,
    /// Unix timestamp in milliseconds when the revocation was recorded.
    pub revoked_at_ms: u64,
}

/// Rotation policy enforced for directory publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DirectoryRotationPolicyV1 {
    /// Minimum interval (milliseconds) between two publishes.
    pub min_interval_ms: u64,
    /// Maximum tolerated clock skew when comparing `created_at_ms`.
    pub max_skew_ms: u64,
    /// Whether each publish must change the Merkle root.
    pub require_change: bool,
    /// Minimum number of council signatures required to publish.
    pub council_threshold: u16,
}

impl Default for DirectoryRotationPolicyV1 {
    fn default() -> Self {
        Self {
            min_interval_ms: 21_600_000, // 6 hours
            max_skew_ms: 300_000,        // 5 minutes
            require_change: true,
            council_threshold: 3,
        }
    }
}

/// Pub/sub events emitted whenever the resolver directory changes.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "event", content = "payload")]
pub enum ResolverDirectoryEventV1 {
    /// Draft submitted by an authorized release signer.
    DraftSubmitted(DirectoryDraftSubmittedEventV1),
    /// Directory record published on-chain.
    Published(DirectoryPublishedEventV1),
    /// Resolver revoked via hotfix.
    Revoked(DirectoryRevokedEventV1),
    /// Resolver revocation lifted.
    Unrevoked(DirectoryUnrevokedEventV1),
    /// Release signer added to the allowlist.
    ReleaseSignerAdded(DirectoryReleaseSignerEventV1),
    /// Release signer removed from the allowlist.
    ReleaseSignerRemoved(DirectoryReleaseSignerEventV1),
    /// Rotation policy updated by governance.
    PolicyUpdated(DirectoryPolicyUpdatedEventV1),
}

/// Reasons for revoking a RAD entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "reason", content = "payload")]
pub enum RadRevokeReason {
    /// Operator-requested revocation (e.g., compromise).
    OperatorRequest,
    /// Governance-forced revocation (policy or compliance breach).
    GovernanceAction,
    /// Certificate or manifest mismatch detected.
    IntegrityViolation,
}

/// Payload for `ResolverDirectoryEventV1::DraftSubmitted`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DirectoryDraftSubmittedEventV1 {
    /// Directory identifier (Merkle root) referenced by the draft.
    pub directory_id: DirectoryId,
    /// CID pointing at the submitted CAR bundle.
    pub car_cid: IpfsPath,
    /// Builder public key that authored the draft.
    pub builder_public_key: PublicKey,
}

/// Payload for `ResolverDirectoryEventV1::Published`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DirectoryPublishedEventV1 {
    /// Directory identifier (Merkle root) that became active.
    pub directory_id: DirectoryId,
    /// Previous directory identifier, if any.
    pub previous_directory_id: Option<DirectoryId>,
    /// SHA-256 digest of the canonical directory.json artifact.
    pub directory_json_sha256: [u8; 32],
    /// CID pointing at the CAR bundle.
    pub car_cid: IpfsPath,
    /// Block height in which the publish occurred.
    pub block_height: u64,
}

/// Payload for `ResolverDirectoryEventV1::Revoked`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DirectoryRevokedEventV1 {
    /// Resolver identifier that was revoked.
    pub resolver_id: ResolverId,
    /// Reason supplied for the revocation.
    pub reason: RadRevokeReason,
    /// Block height in which the revocation was recorded.
    pub block_height: u64,
}

/// Payload for `ResolverDirectoryEventV1::Unrevoked`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DirectoryUnrevokedEventV1 {
    /// Resolver identifier that was restored.
    pub resolver_id: ResolverId,
    /// Block height in which the unrevocation was recorded.
    pub block_height: u64,
}

/// Payload for release signer modifications.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DirectoryReleaseSignerEventV1 {
    /// Public key that was added or removed.
    pub public_key: PublicKey,
}

/// Payload for `ResolverDirectoryEventV1::PolicyUpdated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DirectoryPolicyUpdatedEventV1 {
    /// Updated directory rotation policy.
    pub policy: DirectoryRotationPolicyV1,
}
/// Re-export frequently used `SoraDNS` attestation types.
pub mod prelude {
    pub use super::{
        DirectoryDraftSubmittedEventV1, DirectoryId, DirectoryPolicyUpdatedEventV1,
        DirectoryPublishedEventV1, DirectoryReleaseSignerEventV1, DirectoryRevokedEventV1,
        DirectoryRotationPolicyV1, DirectoryUnrevokedEventV1, GatewayHostSet, HttpTransportV1,
        OdohRelayV1, PaddingPolicyV1, PendingDirectoryDraftV1, QuicTransportV1, RAD_VERSION_V1,
        RadRevokeReason, ResolverAttestationDocumentV1, ResolverDirectoryEventV1,
        ResolverDirectoryRecordV1, ResolverId, ResolverRevocationRecordV1, ResolverTlsBundle,
        ResolverTransportBundle, RotationPolicyV1, SoranetBridgeConfigV1, TlsProvisioningProfile,
        TlsTransportV1,
    };
}

#[cfg(test)]
mod tests {
    use iroha_primitives::soradns::derive_gateway_hosts;

    use super::*;

    #[test]
    fn gateway_host_set_matches_bindings() {
        let bindings = derive_gateway_hosts("docs.sora").expect("derive hosts");
        let host_set = GatewayHostSet::from(&bindings);
        assert_eq!(host_set.canonical_host, bindings.canonical_host());
        assert_eq!(host_set.pretty_host, bindings.pretty_host());
        assert!(host_set.matches(bindings.canonical_host()));
        assert!(host_set.matches(bindings.pretty_host().to_uppercase().as_str()));
        assert!(!host_set.matches("example.com"));
    }
}
