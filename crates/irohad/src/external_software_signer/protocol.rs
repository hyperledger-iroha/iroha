//! Canonical public identities and wire containers for the signer service.
use iroha_crypto::{Algorithm, PublicKey};
use norito::codec::{Decode, Encode};
use std::{fmt, str::FromStr};
pub(super) const SIGNER_PROTOCOL_MAGIC_V1: [u8; 8] = *b"IRSGNR01";
pub(super) const SIGNER_PROTOCOL_VERSION_V1: u16 = 1;
pub(super) const SIGNER_KEY_MAGIC_V1: [u8; 8] = *b"IRSGKY01";
pub(super) const SIGNER_AUDIT_MAGIC_V1: [u8; 8] = *b"IRSGAU01";
pub(super) const SIGNER_PUBLIC_BINDING_MAGIC_V1: [u8; 8] = *b"IRSGPB01";
pub(super) const SIGNER_FRAME_QUALIFY_REQUEST_V1: u8 = 1;
pub(super) const SIGNER_FRAME_QUALIFY_RESPONSE_V1: u8 = 2;
pub(super) const SIGNER_FRAME_SIGN_REQUEST_V1: u8 = 3;
pub(super) const SIGNER_FRAME_SIGN_RESPONSE_V1: u8 = 4;
pub(super) const SIGNER_FRAME_ADMIN_REQUEST_V1: u8 = 5;
pub(super) const SIGNER_FRAME_ADMIN_RESPONSE_V1: u8 = 6;
pub(super) const SIGNER_MAX_FRAME_BYTES_V1: usize = 34 * 1024 * 1024;
pub(super) const SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1: usize = 32 * 1024 * 1024;
pub(super) const SIGNER_MAX_ID_BYTES_V1: usize = 128;
pub(super) const SIGNER_MAX_DOMAIN_BYTES_V1: usize = 128;
pub(super) const SIGNER_MAX_SIGNATURE_BYTES_V1: usize = 4 * 1024;
pub(super) const SIGNER_MAX_PRIVATE_KEY_BYTES_V1: usize = 8 * 1024;
const PUBLIC_KEY_DIGEST_DOMAIN_V1: &[u8] = b"iroha.external-signer.public-key.v1";
const PUBLIC_BINDING_DIGEST_DOMAIN_V1: &[u8] = b"iroha.external-signer.binding.v1";
const REQUEST_DIGEST_DOMAIN_V1: &[u8] = b"iroha.external-signer.request.v1";
const RESPONSE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.external-signer.response.v1";
/// Exact prefix of the `SoraFS` V1 foundational-promotion signing payload.
///
/// The promotion key signs the complete byte string beginning with this prefix; the signer never
/// hashes, decodes, or reserializes the JSON suffix in place of those reviewed bytes.
pub const SORAFS_FOUNDATIONAL_PROMOTION_DOMAIN_V1: &[u8] =
    b"iroha:sorafs:production-readiness:foundational-prerequisites:v1\0";
/// Exact prefix signed only by an isolated Taira release-authority key.
pub const TAIRA_RELEASE_AUTHORITY_SIGNING_DOMAIN_V1: &[u8] = b"iroha:taira:release-authority:v1\0";
/// Provider implementation class carried by external-signer provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[repr(u8)]
pub enum ExternalSignerBackendV1 {
    /// Isolated software key service with an encrypted key envelope.
    Software = 1,
}
/// Signature algorithms admitted by the external signer V1 protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[repr(u8)]
pub enum SoftwareSignerKeyAlgorithmV1 {
    /// Ed25519.
    Ed25519 = 1,
    /// FIPS 204 ML-DSA-65.
    MlDsa = 2,
}
/// Error returned when a signer algorithm or role label is not canonical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoftwareSignerValueParseErrorV1;
impl fmt::Display for SoftwareSignerValueParseErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid external software signer value")
    }
}
impl std::error::Error for SoftwareSignerValueParseErrorV1 {}
impl SoftwareSignerKeyAlgorithmV1 {
    /// Convert to the workspace cryptography algorithm.
    #[must_use]
    pub const fn algorithm(self) -> Algorithm {
        match self {
            Self::Ed25519 => Algorithm::Ed25519,
            Self::MlDsa => Algorithm::MlDsa,
        }
    }
}
impl TryFrom<Algorithm> for SoftwareSignerKeyAlgorithmV1 {
    type Error = ();
    fn try_from(value: Algorithm) -> Result<Self, Self::Error> {
        match value {
            Algorithm::Ed25519 => Ok(Self::Ed25519),
            Algorithm::MlDsa => Ok(Self::MlDsa),
            _ => Err(()),
        }
    }
}
impl FromStr for SoftwareSignerKeyAlgorithmV1 {
    type Err = SoftwareSignerValueParseErrorV1;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "ed25519" => Ok(Self::Ed25519),
            "ml-dsa-65" => Ok(Self::MlDsa),
            _ => Err(SoftwareSignerValueParseErrorV1),
        }
    }
}
impl fmt::Display for SoftwareSignerKeyAlgorithmV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Ed25519 => "ed25519",
            Self::MlDsa => "ml-dsa-65",
        })
    }
}
/// Least-privilege signing domains served by the first software-signer slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[repr(u8)]
pub enum SoftwareSignerRoleV1 {
    /// Finalized proof-outcome transaction signing.
    ProofOutcome = 1,
    /// Native repair transaction signing.
    Repair = 2,
    /// Native reserve/rent transaction signing.
    Reserve = 3,
    /// Native orderbook transaction signing.
    Orderbook = 4,
    /// `SoraFS` V1 foundational promotion-envelope signing.
    Promotion = 5,
    /// Governance DAG publisher signing.
    GovernanceDag = 6,
    /// `PoTR` gateway receipt signing.
    PotrGateway = 7,
    /// `PoTR` provider receipt signing.
    PotrProvider = 8,
    /// Governed billing-statement digest signing.
    BillingStatement = 9,
    /// Evidence-viewer receipt, checkpoint, and archive signing.
    EvidenceViewer = 10,
    /// Stream-token issuance signing.
    StreamToken = 11,
    /// `PoP` credential, commitment-root, and revocation signing.
    PopCredentials = 12,
    /// Purpose-separated signing owned by one Taira release-authority role.
    TairaAuthority = 13,
}
impl SoftwareSignerRoleV1 {
    /// Stable role label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProofOutcome => "proof_outcome",
            Self::Repair => "repair",
            Self::Reserve => "reserve",
            Self::Orderbook => "orderbook",
            Self::Promotion => "promotion",
            Self::GovernanceDag => "governance_dag",
            Self::PotrGateway => "potr_gateway",
            Self::PotrProvider => "potr_provider",
            Self::BillingStatement => "billing_statement",
            Self::EvidenceViewer => "evidence_viewer",
            Self::StreamToken => "stream_token",
            Self::PopCredentials => "pop_credentials",
            Self::TairaAuthority => "taira_authority",
        }
    }
    /// Exact signing domain enforced before any key operation.
    #[must_use]
    pub const fn domain(self) -> &'static str {
        match self {
            Self::ProofOutcome => "sorafs.native-transaction.proof-outcome.v1",
            Self::Repair => "sorafs.native-transaction.repair.v1",
            Self::Reserve => "sorafs.native-transaction.reserve-rent.v1",
            Self::Orderbook => "sorafs.native-transaction.orderbook.v1",
            Self::Promotion => "sorafs.production-readiness.foundational-prerequisites.v1",
            Self::GovernanceDag => "sorafs.governance-dag.publisher.v1",
            Self::PotrGateway => "sorafs.potr.gateway-receipt.v1",
            Self::PotrProvider => "sorafs.potr.provider-receipt.v1",
            Self::BillingStatement => "sorafs.billing.statement-signature.v1",
            Self::EvidenceViewer => "sorafs.evidence-viewer.signing.v1",
            Self::StreamToken => "sorafs.stream-token.signature.v1",
            Self::PopCredentials => "sorafs.pop.issuer-signature.v1",
            Self::TairaAuthority => "iroha.taira.release-authority.v1",
        }
    }
    /// Whether this isolated role admits the requested key algorithm.
    #[must_use]
    pub const fn allows_algorithm(self, algorithm: SoftwareSignerKeyAlgorithmV1) -> bool {
        match self {
            Self::ProofOutcome | Self::Repair | Self::Reserve | Self::Orderbook => true,
            Self::PotrProvider => matches!(algorithm, SoftwareSignerKeyAlgorithmV1::MlDsa),
            Self::Promotion
            | Self::GovernanceDag
            | Self::PotrGateway
            | Self::BillingStatement
            | Self::EvidenceViewer
            | Self::StreamToken
            | Self::PopCredentials
            | Self::TairaAuthority => {
                matches!(algorithm, SoftwareSignerKeyAlgorithmV1::Ed25519)
            }
        }
    }
    /// Convert a native-transaction role to the existing Torii runtime role.
    #[must_use]
    pub const fn native_role(self) -> Option<iroha_torii::SorafsNativeTransactionSignerRoleV1> {
        match self {
            Self::ProofOutcome => {
                Some(iroha_torii::SorafsNativeTransactionSignerRoleV1::ProofOutcome)
            }
            Self::Repair => Some(iroha_torii::SorafsNativeTransactionSignerRoleV1::Repair),
            Self::Reserve => Some(iroha_torii::SorafsNativeTransactionSignerRoleV1::Reserve),
            Self::Orderbook => Some(iroha_torii::SorafsNativeTransactionSignerRoleV1::Orderbook),
            Self::Promotion
            | Self::GovernanceDag
            | Self::PotrGateway
            | Self::PotrProvider
            | Self::BillingStatement
            | Self::EvidenceViewer
            | Self::StreamToken
            | Self::PopCredentials
            | Self::TairaAuthority => None,
        }
    }
}
impl From<iroha_torii::SorafsNativeTransactionSignerRoleV1> for SoftwareSignerRoleV1 {
    fn from(value: iroha_torii::SorafsNativeTransactionSignerRoleV1) -> Self {
        match value {
            iroha_torii::SorafsNativeTransactionSignerRoleV1::ProofOutcome => Self::ProofOutcome,
            iroha_torii::SorafsNativeTransactionSignerRoleV1::Repair => Self::Repair,
            iroha_torii::SorafsNativeTransactionSignerRoleV1::Reserve => Self::Reserve,
            iroha_torii::SorafsNativeTransactionSignerRoleV1::Orderbook => Self::Orderbook,
        }
    }
}
impl FromStr for SoftwareSignerRoleV1 {
    type Err = SoftwareSignerValueParseErrorV1;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "proof_outcome" => Ok(Self::ProofOutcome),
            "repair" => Ok(Self::Repair),
            "reserve" => Ok(Self::Reserve),
            "orderbook" => Ok(Self::Orderbook),
            "promotion" => Ok(Self::Promotion),
            "governance_dag" => Ok(Self::GovernanceDag),
            "potr_gateway" => Ok(Self::PotrGateway),
            "potr_provider" => Ok(Self::PotrProvider),
            "billing_statement" => Ok(Self::BillingStatement),
            "evidence_viewer" => Ok(Self::EvidenceViewer),
            "stream_token" => Ok(Self::StreamToken),
            "pop_credentials" => Ok(Self::PopCredentials),
            "taira_authority" => Ok(Self::TairaAuthority),
            _ => Err(SoftwareSignerValueParseErrorV1),
        }
    }
}
impl fmt::Display for SoftwareSignerRoleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
/// Public role-specific authority pinned into the encrypted key envelope.
///
/// The signer service validates this value itself, so an authenticated client
/// cannot bypass the deployment adapter by submitting a structurally valid
/// payload for a substituted publisher, provider, or issuer identity.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub enum SoftwareSignerPurposeBindingV1 {
    /// Native transaction and promotion roles carry their authority in the
    /// signed payload or the public key itself.
    NativeOrPromotion,
    /// Exact Governance DAG publisher peer identity.
    GovernanceDag {
        /// Canonical publisher peer identifier bytes.
        publisher_peer_id: Vec<u8>,
    },
    /// Exact independently administered `PoTR` gateway signer identity.
    PotrGateway {
        /// Public gateway signer identifier.
        signer_id: [u8; 32],
    },
    /// Exact independently administered `PoTR` provider signer and provider.
    PotrProvider {
        /// Public provider-side signer identifier.
        signer_id: [u8; 32],
        /// Provider identifier authorized for signed receipts.
        provider_id: [u8; 32],
    },
    /// Exact governed billing statement signer identity.
    BillingStatement {
        /// Stable public billing signer identity.
        signer_id: String,
    },
    /// Evidence-viewer authority is the binding handle and Ed25519 key.
    EvidenceViewer,
    /// Stream-token authority is the binding handle and Ed25519 key.
    StreamToken,
    /// Exact governed `PoP` issuer identity.
    PopCredentials {
        /// Stable public `PoP` credential issuer identity.
        issuer_id: String,
    },
    /// Exact Taira release-authority role owned by this isolated key.
    TairaAuthority {
        /// Stable kebab-case role label from the closed eight-role registry.
        role: String,
    },
}
impl SoftwareSignerPurposeBindingV1 {
    pub(super) fn validates_role(&self, role: SoftwareSignerRoleV1) -> bool {
        match (role, self) {
            (
                SoftwareSignerRoleV1::ProofOutcome
                | SoftwareSignerRoleV1::Repair
                | SoftwareSignerRoleV1::Reserve
                | SoftwareSignerRoleV1::Orderbook
                | SoftwareSignerRoleV1::Promotion,
                Self::NativeOrPromotion,
            )
            | (SoftwareSignerRoleV1::EvidenceViewer, Self::EvidenceViewer)
            | (SoftwareSignerRoleV1::StreamToken, Self::StreamToken) => true,
            (SoftwareSignerRoleV1::GovernanceDag, Self::GovernanceDag { publisher_peer_id }) => {
                !publisher_peer_id.is_empty()
                    && publisher_peer_id.len()
                        <= sorafs_manifest::GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1
            }
            (SoftwareSignerRoleV1::PotrGateway, Self::PotrGateway { signer_id }) => {
                *signer_id != [0; 32]
            }
            (
                SoftwareSignerRoleV1::PotrProvider,
                Self::PotrProvider {
                    signer_id,
                    provider_id,
                },
            ) => *signer_id != [0; 32] && *provider_id != [0; 32] && signer_id != provider_id,
            (SoftwareSignerRoleV1::BillingStatement, Self::BillingStatement { signer_id }) => {
                valid_identity(signer_id)
            }
            (SoftwareSignerRoleV1::PopCredentials, Self::PopCredentials { issuer_id }) => {
                valid_identity(issuer_id)
            }
            (SoftwareSignerRoleV1::TairaAuthority, Self::TairaAuthority { role }) => {
                valid_taira_authority_role_label(role)
            }
            _ => false,
        }
    }
}
/// Immutable public identity expected from one software signer service.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SoftwareSignerPublicBindingV1 {
    /// Exact binding format marker.
    pub magic: [u8; 8],
    /// Exact binding version.
    pub version: u16,
    /// Backend classification. V1 production software services require `Software`.
    pub backend: ExternalSignerBackendV1,
    /// Stable opaque runtime-provider handle.
    pub handle: String,
    /// Stable service identity managed by the signer operator.
    pub service_id: String,
    /// Stable independent administrator identity.
    pub administrator_id: String,
    /// Exact operating-system UID of the signer service.
    pub service_uid: u32,
    /// Exact operating-system UID allowed to request signatures.
    pub client_uid: u32,
    /// Exact operating-system UID allowed to administer the service.
    pub administrator_uid: u32,
    /// Isolated signing role.
    pub role: SoftwareSignerRoleV1,
    /// Exact public authority for the role's purpose-separated payloads.
    pub purpose_binding: SoftwareSignerPurposeBindingV1,
    /// Exact role-specific signing domain.
    pub domain: String,
    /// Active signature algorithm.
    pub key_algorithm: SoftwareSignerKeyAlgorithmV1,
    /// Monotonic key generation.
    pub key_revision: u64,
    /// Monotonic public-policy generation.
    pub policy_revision: u64,
    /// Digest of the public signing policy.
    pub policy_digest: [u8; 32],
    /// Exact active public key.
    pub public_key: PublicKey,
    /// Domain-separated digest of `public_key`.
    pub public_key_digest: [u8; 32],
    /// First audit-chain record digest.
    pub audit_genesis_digest: [u8; 32],
    /// Maximum canonical transaction-payload bytes accepted by the service.
    pub max_request_bytes: u32,
}
impl SoftwareSignerPublicBindingV1 {
    /// Validate every fail-closed public binding invariant.
    ///
    /// # Errors
    ///
    /// Returns `()` for malformed, substituted, test-marked, or non-software bindings.
    #[expect(
        clippy::result_unit_err,
        reason = "the public binding exposes only valid versus invalid and carries no attacker-controlled detail"
    )]
    pub fn validate(&self) -> Result<(), ()> {
        if self.magic != SIGNER_PUBLIC_BINDING_MAGIC_V1
            || self.version != SIGNER_PROTOCOL_VERSION_V1
            || self.backend != ExternalSignerBackendV1::Software
            || !valid_identity(&self.service_id)
            || !valid_identity(&self.administrator_id)
            || self.service_id == self.administrator_id
            || self.service_uid == self.client_uid
            || self.service_uid == self.administrator_uid
            || self.client_uid == self.administrator_uid
            || !self.purpose_binding.validates_role(self.role)
            || self.domain != self.role.domain()
            || self.domain.len() > SIGNER_MAX_DOMAIN_BYTES_V1
            || self.key_revision == 0
            || self.policy_revision == 0
            || self.policy_digest == [0; 32]
            || self.audit_genesis_digest == [0; 32]
            || self.max_request_bytes == 0
            || usize::try_from(self.max_request_bytes).map_err(|_| ())?
                > SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1
            || self.public_key.try_algorithm().map_err(|_| ())? != self.key_algorithm.algorithm()
            || !self.role.allows_algorithm(self.key_algorithm)
            || public_key_digest(&self.public_key)? != self.public_key_digest
            || !valid_software_signer_handle(self.role, &self.handle)
        {
            return Err(());
        }
        Ok(())
    }
    /// Return the canonical domain-separated binding digest.
    ///
    /// # Errors
    ///
    /// Returns `()` when the binding is invalid or cannot be encoded.
    #[expect(
        clippy::result_unit_err,
        reason = "the canonical digest contract intentionally exposes only success versus invalid binding"
    )]
    pub fn digest(&self) -> Result<[u8; 32], ()> {
        self.validate()?;
        digest_canonical(PUBLIC_BINDING_DIGEST_DOMAIN_V1, self)
    }
}
/// Live software-signer provenance returned by qualification and signing.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SoftwareSignerLiveProvenanceV1 {
    /// Immutable public service binding.
    pub binding: SoftwareSignerPublicBindingV1,
    /// Last durably committed audit sequence.
    pub audit_sequence: u64,
    /// Last durably committed audit record digest.
    pub audit_head: [u8; 32],
    /// Whether the active generation has been irreversibly revoked.
    pub revoked: bool,
    /// Active key signature over the canonical provenance body.
    pub attestation: Vec<u8>,
}
impl SoftwareSignerLiveProvenanceV1 {
    /// Compare every signed live-state field while excluding the potentially
    /// randomized signature encoding itself.
    pub(super) fn has_same_stable_state(&self, other: &Self) -> bool {
        self.binding == other.binding
            && self.audit_sequence == other.audit_sequence
            && self.audit_head == other.audit_head
            && self.revoked == other.revoked
    }
}
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub(super) struct SoftwareSignerFrameV1 {
    pub magic: [u8; 8],
    pub version: u16,
    pub kind: u8,
    pub body: Vec<u8>,
}
impl fmt::Debug for SoftwareSignerFrameV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SoftwareSignerFrameV1")
            .field("version", &self.version)
            .field("kind", &self.kind)
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}
impl Drop for SoftwareSignerFrameV1 {
    fn drop(&mut self) {
        scrub(&mut self.body);
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct QualifyRequestV1 {
    pub binding_digest: [u8; 32],
    pub client_nonce: [u8; 32],
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct QualifyResponseV1 {
    pub client_nonce: [u8; 32],
    pub server_nonce: [u8; 32],
    pub provenance: SoftwareSignerLiveProvenanceV1,
    pub response_digest: [u8; 32],
    pub response_attestation: Vec<u8>,
}
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub(super) struct SignRequestV1 {
    pub binding_digest: [u8; 32],
    pub operation_id: [u8; 32],
    pub expected_key_revision: u64,
    pub expected_policy_revision: u64,
    pub expected_policy_digest: [u8; 32],
    pub payload_digest: [u8; 32],
    pub payload: Vec<u8>,
    pub request_digest: [u8; 32],
}
impl fmt::Debug for SignRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignRequestV1")
            .field("operation_id", &hex::encode(self.operation_id))
            .field("payload_digest", &hex::encode(self.payload_digest))
            .field("payload_len", &self.payload.len())
            .finish_non_exhaustive()
    }
}
impl Drop for SignRequestV1 {
    fn drop(&mut self) {
        scrub(&mut self.payload);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[repr(u8)]
pub(super) enum SignStatusV1 {
    Ok = 0,
    Replayed = 1,
    Rejected = 2,
    Equivocation = 3,
    StaleOrRevoked = 4,
    Unavailable = 5,
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct SignResponseV1 {
    pub operation_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub payload_digest: [u8; 32],
    pub status: SignStatusV1,
    pub signature: Vec<u8>,
    pub commit_sequence: u64,
    pub commit_audit_head: [u8; 32],
    pub provenance: SoftwareSignerLiveProvenanceV1,
    pub response_digest: [u8; 32],
    pub response_attestation: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) enum AdminCommandV1 {
    Status,
    Rotate {
        operation_id: [u8; 32],
        expected_audit_head: [u8; 32],
        expected_key_revision: u64,
        new_key_revision: u64,
        new_policy_revision: u64,
        new_policy_digest: [u8; 32],
        algorithm: SoftwareSignerKeyAlgorithmV1,
    },
    Revoke {
        operation_id: [u8; 32],
        expected_audit_head: [u8; 32],
        expected_key_revision: u64,
        reason_digest: [u8; 32],
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct AdminRequestV1 {
    pub binding_digest: [u8; 32],
    pub command: AdminCommandV1,
    pub request_digest: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[repr(u8)]
pub(super) enum AdminStatusV1 {
    Ok = 0,
    Replayed = 1,
    Rejected = 2,
    Conflict = 3,
    Unavailable = 4,
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct AdminResponseV1 {
    pub request_digest: [u8; 32],
    pub status: AdminStatusV1,
    pub provenance: SoftwareSignerLiveProvenanceV1,
    pub response_digest: [u8; 32],
    pub response_attestation: Vec<u8>,
}
pub(super) fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= SIGNER_MAX_ID_BYTES_V1
        && !value.as_bytes().contains(&0)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
        && !value.to_ascii_lowercase().contains("test")
}
pub(super) fn valid_taira_authority_role_label(value: &str) -> bool {
    matches!(
        value,
        "native-evidence"
            | "privacy-protocol-origin"
            | "privacy-governance"
            | "qualification"
            | "deploy-issuance"
            | "rollout-observation"
            | "public-soak-observation"
            | "public-soak-replay-admission"
    )
}
pub(super) fn valid_software_signer_handle(role: SoftwareSignerRoleV1, value: &str) -> bool {
    let (role_segment, instance_prefix) = match role {
        SoftwareSignerRoleV1::ProofOutcome => ("proof-outcome", None),
        SoftwareSignerRoleV1::Repair => ("repair", None),
        SoftwareSignerRoleV1::Reserve => ("reserve", None),
        SoftwareSignerRoleV1::Orderbook => ("orderbook", None),
        SoftwareSignerRoleV1::Promotion => ("promotion", None),
        SoftwareSignerRoleV1::GovernanceDag => ("governance-dag", None),
        SoftwareSignerRoleV1::PotrGateway => ("potr", Some("gateway-")),
        SoftwareSignerRoleV1::PotrProvider => ("potr", Some("provider-")),
        SoftwareSignerRoleV1::BillingStatement => ("billing", None),
        SoftwareSignerRoleV1::EvidenceViewer => ("evidence-viewer", None),
        SoftwareSignerRoleV1::StreamToken => ("stream-token", None),
        SoftwareSignerRoleV1::PopCredentials => ("pop-credentials", None),
        SoftwareSignerRoleV1::TairaAuthority => ("taira-authority", None),
    };
    let prefix = format!("software://sorafs/{role_segment}/");
    iroha_config::parameters::validate_production_runtime_handle(value).is_ok()
        && value.strip_prefix(&prefix).is_some_and(|instance| {
            valid_identity(instance)
                && !instance.contains('/')
                && instance_prefix.is_none_or(|prefix| instance.starts_with(prefix))
        })
}
pub(super) fn public_key_digest(public_key: &PublicKey) -> Result<[u8; 32], ()> {
    let (algorithm, payload) = public_key.try_to_bytes().map_err(|_| ())?;
    Ok(digest_parts(
        PUBLIC_KEY_DIGEST_DOMAIN_V1,
        &[&[algorithm as u8], payload],
    ))
}
pub(super) fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(&u64::try_from(part.len()).unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}
pub(super) fn digest_canonical<T: norito::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], ()> {
    let bytes = norito::encode_canonical(value).map_err(|_| ())?;
    Ok(digest_parts(domain, &[&bytes]))
}
pub(super) fn payload_digest(payload: &[u8]) -> [u8; 32] {
    digest_parts(b"iroha.external-signer.payload.v1", &[payload])
}
pub(super) fn sign_request_digest(request: &SignRequestV1) -> Result<[u8; 32], ()> {
    digest_canonical(
        REQUEST_DIGEST_DOMAIN_V1,
        &(
            request.binding_digest,
            request.operation_id,
            request.expected_key_revision,
            request.expected_policy_revision,
            request.expected_policy_digest,
            request.payload_digest,
            u64::try_from(request.payload.len()).map_err(|_| ())?,
        ),
    )
}
pub(super) fn sign_response_digest(response: &SignResponseV1) -> Result<[u8; 32], ()> {
    digest_canonical(
        RESPONSE_DIGEST_DOMAIN_V1,
        &(
            response.operation_id,
            response.request_digest,
            response.payload_digest,
            response.status,
            response.signature.clone(),
            response.commit_sequence,
            response.commit_audit_head,
            response.provenance.clone(),
        ),
    )
}
pub(super) fn qualify_response_digest(response: &QualifyResponseV1) -> Result<[u8; 32], ()> {
    digest_canonical(
        RESPONSE_DIGEST_DOMAIN_V1,
        &(
            response.client_nonce,
            response.server_nonce,
            response.provenance.clone(),
        ),
    )
}
pub(super) fn admin_request_digest(
    binding_digest: [u8; 32],
    command: &AdminCommandV1,
) -> Result<[u8; 32], ()> {
    digest_canonical(REQUEST_DIGEST_DOMAIN_V1, &(binding_digest, command.clone()))
}
pub(super) fn admin_response_digest(response: &AdminResponseV1) -> Result<[u8; 32], ()> {
    digest_canonical(
        RESPONSE_DIGEST_DOMAIN_V1,
        &(
            response.request_digest,
            response.status,
            response.provenance.clone(),
        ),
    )
}
pub(super) fn scrub(bytes: &mut [u8]) {
    bytes.fill(0);
    let _ = std::hint::black_box(bytes);
}
