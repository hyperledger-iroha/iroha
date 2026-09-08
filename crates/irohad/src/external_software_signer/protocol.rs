//! Canonical public identities and wire containers for the signer service.
use iroha_crypto::PublicKey;
use norito::codec::{Decode, Encode};
use std::fmt;
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
/// Provider implementation class carried by external-signer provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[repr(u8)]
pub enum ExternalSignerBackendV1 {
    /// Isolated software key service with an encrypted key envelope.
    Software = 1,
}
pub use sorafs_manifest::signer::protocol::{
    SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1, SignerValueParseErrorV1,
};
/// Convert a canonical role to the daemon-owned native transaction provider role.
pub(super) const fn native_role(
    role: SignerRoleV1,
) -> Option<iroha_torii::SorafsNativeTransactionSignerRoleV1> {
    match role {
        SignerRoleV1::ProofOutcome => {
            Some(iroha_torii::SorafsNativeTransactionSignerRoleV1::ProofOutcome)
        }
        SignerRoleV1::Repair => Some(iroha_torii::SorafsNativeTransactionSignerRoleV1::Repair),
        SignerRoleV1::Reserve => Some(iroha_torii::SorafsNativeTransactionSignerRoleV1::Reserve),
        SignerRoleV1::Orderbook => {
            Some(iroha_torii::SorafsNativeTransactionSignerRoleV1::Orderbook)
        }
        _ => None,
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
    pub role: SignerRoleV1,
    /// Exact public authority for the role's purpose-separated payloads.
    pub purpose_binding: SignerPurposeBindingV1,
    /// Exact role-specific signing domain.
    pub domain: String,
    /// Active signature algorithm.
    pub key_algorithm: SignerKeyAlgorithmV1,
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
        algorithm: SignerKeyAlgorithmV1,
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
pub(super) fn valid_software_signer_handle(role: SignerRoleV1, value: &str) -> bool {
    let (role_segment, instance_prefix) = match role {
        SignerRoleV1::ProofOutcome => ("proof-outcome", None),
        SignerRoleV1::Repair => ("repair", None),
        SignerRoleV1::Reserve => ("reserve", None),
        SignerRoleV1::Orderbook => ("orderbook", None),
        SignerRoleV1::Promotion => ("promotion", None),
        SignerRoleV1::GovernanceDag => ("governance-dag", None),
        SignerRoleV1::PotrGateway => ("potr", Some("gateway-")),
        SignerRoleV1::PotrProvider => ("potr", Some("provider-")),
        SignerRoleV1::BillingStatement => ("billing", None),
        SignerRoleV1::EvidenceViewer => ("evidence-viewer", None),
        SignerRoleV1::PopCredentials => ("pop-credentials", None),
        SignerRoleV1::ReleaseManifest | SignerRoleV1::StreamToken => return false,
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
pub(super) use crate::signer_operation::{digest_canonical, digest_parts};
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
