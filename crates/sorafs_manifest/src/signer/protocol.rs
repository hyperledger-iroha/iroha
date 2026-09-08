//! Canonical public signer roles, purpose bindings and deterministic framing.

use iroha_crypto::Algorithm;
use norito::codec::{Decode, Encode};
use std::{fmt, str::FromStr};
const SIGNER_MAX_ID_BYTES_V1: usize = 128;

/// Signature algorithms admitted by the external signer V1 protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[repr(u8)]
pub enum SignerKeyAlgorithmV1 {
    /// Ed25519.
    Ed25519 = 1,
    /// FIPS 204 ML-DSA-65.
    MlDsa = 2,
}
/// Error returned when a signer algorithm or role label is not canonical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignerValueParseErrorV1;
impl fmt::Display for SignerValueParseErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid external signer value")
    }
}
impl std::error::Error for SignerValueParseErrorV1 {}
impl SignerKeyAlgorithmV1 {
    /// Convert to the workspace cryptography algorithm.
    #[must_use]
    pub const fn algorithm(self) -> Algorithm {
        match self {
            Self::Ed25519 => Algorithm::Ed25519,
            Self::MlDsa => Algorithm::MlDsa,
        }
    }
}
impl TryFrom<Algorithm> for SignerKeyAlgorithmV1 {
    type Error = ();
    fn try_from(value: Algorithm) -> Result<Self, Self::Error> {
        match value {
            Algorithm::Ed25519 => Ok(Self::Ed25519),
            Algorithm::MlDsa => Ok(Self::MlDsa),
            _ => Err(()),
        }
    }
}
impl FromStr for SignerKeyAlgorithmV1 {
    type Err = SignerValueParseErrorV1;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "ed25519" => Ok(Self::Ed25519),
            "ml-dsa-65" => Ok(Self::MlDsa),
            _ => Err(SignerValueParseErrorV1),
        }
    }
}
impl fmt::Display for SignerKeyAlgorithmV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Ed25519 => "ed25519",
            Self::MlDsa => "ml-dsa-65",
        })
    }
}
/// Least-privilege signing domains served by the canonical hardware signer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[repr(u8)]
pub enum SignerRoleV1 {
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
    /// Exact aggregate release-manifest bytes, separate from foundational promotion.
    ReleaseManifest = 13,
}
impl SignerRoleV1 {
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
            Self::ReleaseManifest => "release_manifest",
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
            Self::ReleaseManifest => "sorafs.release-manifest.signature.v1",
        }
    }
    /// Whether this isolated role admits the requested key algorithm.
    #[must_use]
    pub const fn allows_algorithm(self, algorithm: SignerKeyAlgorithmV1) -> bool {
        match self {
            Self::ProofOutcome | Self::Repair | Self::Reserve | Self::Orderbook => true,
            Self::PotrProvider => matches!(algorithm, SignerKeyAlgorithmV1::MlDsa),
            Self::Promotion
            | Self::GovernanceDag
            | Self::PotrGateway
            | Self::BillingStatement
            | Self::EvidenceViewer
            | Self::StreamToken
            | Self::PopCredentials
            | Self::ReleaseManifest => {
                matches!(algorithm, SignerKeyAlgorithmV1::Ed25519)
            }
        }
    }
}
impl FromStr for SignerRoleV1 {
    type Err = SignerValueParseErrorV1;
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
            "release_manifest" => Ok(Self::ReleaseManifest),
            _ => Err(SignerValueParseErrorV1),
        }
    }
}
impl fmt::Display for SignerRoleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
/// Public role-specific authority pinned into independently qualified custody.
///
/// The signer service validates this value itself, so an authenticated client
/// cannot bypass the deployment adapter by submitting a structurally valid
/// payload for a substituted publisher, provider, or issuer identity.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub enum SignerPurposeBindingV1 {
    /// Native transaction and promotion roles carry their authority in the
    /// signed payload or the public key itself.
    NativeOrPromotion,
    /// Exact aggregate release deployment and reviewed release-policy subject.
    ReleaseManifest {
        /// Canonical deployment identity governed by the signing policy.
        deployment_id: String,
    },
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
}
impl SignerPurposeBindingV1 {
    /// Whether this authority is well formed for the exact signing role.
    #[must_use]
    pub fn validates_role(&self, role: SignerRoleV1) -> bool {
        match (role, self) {
            (
                SignerRoleV1::ProofOutcome
                | SignerRoleV1::Repair
                | SignerRoleV1::Reserve
                | SignerRoleV1::Orderbook
                | SignerRoleV1::Promotion,
                Self::NativeOrPromotion,
            )
            | (SignerRoleV1::EvidenceViewer, Self::EvidenceViewer)
            | (SignerRoleV1::StreamToken, Self::StreamToken) => true,
            (SignerRoleV1::ReleaseManifest, Self::ReleaseManifest { deployment_id }) => {
                valid_identity(deployment_id)
            }
            (SignerRoleV1::GovernanceDag, Self::GovernanceDag { publisher_peer_id }) => {
                !publisher_peer_id.is_empty()
                    && publisher_peer_id.len()
                        <= crate::GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1
            }
            (SignerRoleV1::PotrGateway, Self::PotrGateway { signer_id }) => *signer_id != [0; 32],
            (
                SignerRoleV1::PotrProvider,
                Self::PotrProvider {
                    signer_id,
                    provider_id,
                },
            ) => *signer_id != [0; 32] && *provider_id != [0; 32] && signer_id != provider_id,
            (SignerRoleV1::BillingStatement, Self::BillingStatement { signer_id }) => {
                valid_identity(signer_id)
            }
            (SignerRoleV1::PopCredentials, Self::PopCredentials { issuer_id }) => {
                valid_identity(issuer_id)
            }
            _ => false,
        }
    }
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

use super::custody::VerifiedSignerCustodyV1;
const INTENT_DOMAIN: &[u8] = b"iroha.sorafs.signer.operation.intent.v1";
/// Ordinary service actions admitted by the operation boundary.
///
/// Terminal administrative transitions are deliberately not ordinary active-key operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub enum SignerOperationActionV1 {
    /// One role-authorized payload signature and its audit/provenance/response.
    Sign,
    /// An authenticated qualification observation with its own audit entry.
    Qualify,
    /// An authenticated administrative status observation with its own audit entry.
    Status,
    /// Finalize an old-generation audit before activating an independently qualified successor.
    ActivateCustody,
    /// Finalize an old-generation audit before terminal revocation takes effect.
    RevokeCustody,
}

/// Exactly one ordered sub-signature within an aggregate reserved action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub enum SignerKeyOperationPurposeV1 {
    /// Validated exact role-specific signing bytes.
    RolePayload,
    /// Canonical predecessor-bound audit-record signing bytes.
    AuditRecord,
    /// Canonical live provenance signing bytes, including independent custody evidence.
    Provenance,
    /// Canonical final response signing bytes.
    Response,
}

/// Independently expected audit predecessor or successor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SignerOperationAuditHeadV1 {
    /// Monotonic audit record sequence; zero only before genesis.
    pub sequence: u64,
    /// Exact canonical audit record digest; zero only before genesis.
    pub digest: [u8; 32],
}
impl SignerOperationAuditHeadV1 {
    /// Canonical audit signing message, binding both digest and monotonic sequence.
    #[must_use]
    pub fn signing_message(&self) -> [u8; 32] {
        digest_parts(
            b"iroha.external-signer.audit-attestation.v1",
            &[&self.sequence.to_be_bytes(), &self.digest],
        )
    }
}

/// Exact service action admitted before any provider I/O.
#[derive(Clone, Copy, PartialEq, Eq, Decode, Encode)]
pub struct SignerOperationIntentV1 {
    /// Exact aggregate action, determining the required sub-signature sequence.
    pub action: SignerOperationActionV1,
    /// Nonzero operation id whose reservation remains durable even on failure.
    pub operation_id: [u8; 32],
    /// Digest of the validated canonical service request, including its expected public binding.
    pub request_digest: [u8; 32],
    /// Independently expected authoritative journal predecessor.
    pub previous_audit: SignerOperationAuditHeadV1,
}
impl SignerOperationIntentV1 {
    /// Canonical intent commitment; rejects inert or incoherent request coordinates.
    ///
    /// # Errors
    /// Returns an error for invalid coordinates or canonical encoding failure.
    pub fn digest(&self) -> Result<[u8; 32], ()> {
        if self.operation_id == [0; 32]
            || self.request_digest == [0; 32]
            || self.previous_audit.sequence == u64::MAX
            || (self.previous_audit.sequence == 0) != (self.previous_audit.digest == [0; 32])
        {
            return Err(());
        }
        digest_canonical(INTENT_DOMAIN, self).map_err(|_| ())
    }
}
impl fmt::Debug for SignerOperationIntentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerOperationIntentV1")
            .field("action", &self.action)
            .finish_non_exhaustive()
    }
}

/// Public reservation coordinates returned only by the injected authoritative state source.
///
/// Possession of this value does not authorize signing: all provider requests are privately
/// constructed and the source must authenticate its exact ownership on every reserved observation.
#[derive(Clone, Copy, PartialEq, Eq, Decode, Encode)]
pub struct SignerOperationReservationV1 {
    /// Unique nonzero durable reservation id.
    pub reservation_id: [u8; 32],
    /// Positive monotonic fencing generation assigned by authoritative CAS.
    pub fence: u64,
    /// Exclusive expiry in the source's independently trusted Unix milliseconds.
    pub expires_at_unix_ms: u64,
}
impl fmt::Debug for SignerOperationReservationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerOperationReservationV1")
            .field("fence", &self.fence)
            .finish_non_exhaustive()
    }
}

/// Public commitments prepared internally by the service before authoritative completion CAS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SignerOperationCommitmentV1 {
    /// The exact next immutable audit record, persisted before completion can be acknowledged.
    pub audit: SignerOperationAuditHeadV1,
    /// Canonical response digest; recovery must reproduce this exact response.
    pub response_digest: [u8; 32],
}

/// Original custody identity that every durable completion must retain and authenticate.
///
/// The complete record digest commits chain, network, role, key, opaque handle and key/policy
/// generations. The control-state digest additionally fences changes that preserve the same
/// record. These are public claims until compared with an independently verified observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SignerOperationCustodyV1 {
    /// Exact independently qualified record used by the original operation.
    pub record_digest: [u8; 32],
    /// Exact per-role finalized custody control state used by the original operation.
    pub control_state_digest: [u8; 32],
}
impl SignerOperationCustodyV1 {
    /// Bind a completion only to an independently verified custody observation.
    #[must_use]
    pub fn from_verified(custody: &VerifiedSignerCustodyV1) -> Self {
        Self {
            record_digest: custody.record_digest(),
            control_state_digest: custody.current_anchor().state_digest,
        }
    }
}
impl SignerOperationCommitmentV1 {
    /// Canonical signing message for the exact final response digest.
    #[must_use]
    pub fn response_signing_message(&self) -> [u8; 32] {
        digest_parts(
            b"iroha.external-signer.response-attestation.v1",
            &[&self.response_digest],
        )
    }
}

/// Maximum exact reviewed release-manifest payload, consistent with the release verifier.
pub const SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1: usize = 1024 * 1024;

/// Exact released sub-signature, bound to its ordered semantic purpose and message digest.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct SignerOperationSignatureV1 {
    /// Position-specific signing purpose.
    pub purpose: SignerKeyOperationPurposeV1,
    /// Domain-separated commitment to exact signed bytes.
    pub message_digest: [u8; 32],
    /// Canonical signature bytes; never private key material.
    pub signature: Vec<u8>,
}
impl fmt::Debug for SignerOperationSignatureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerOperationSignatureV1")
            .field("purpose", &self.purpose)
            .finish_non_exhaustive()
    }
}

/// Commit exact signing bytes with the runtime operation domain.
#[must_use]
pub fn signer_operation_message_digest_v1(message: &[u8]) -> [u8; 32] {
    digest_parts(b"iroha.sorafs.signer.operation.message.v1", &[message])
}

/// Commit the exact ordered signatures using the runtime completion domain.
///
/// # Errors
/// Rejects empty/oversized signature collections or canonical serialization failures.
pub fn signer_operation_signatures_digest_v1(
    signatures: &[SignerOperationSignatureV1],
) -> Result<[u8; 32], ()> {
    if signatures.is_empty()
        || signatures.len() > 4
        || signatures
            .iter()
            .any(|signature| signature.signature.is_empty() || signature.signature.len() > 4096)
    {
        return Err(());
    }
    let manifest = signatures
        .iter()
        .map(|signature| {
            (
                signature.purpose,
                signature.message_digest,
                digest_parts(
                    b"iroha.sorafs.signer.operation.signature.v1",
                    &[signature.signature.as_slice()],
                ),
            )
        })
        .collect::<Vec<_>>();
    digest_canonical(b"iroha.sorafs.signer.operation.signatures.v1", &manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_role_labels_domains_and_release_purpose_are_distinct() {
        let roles = [
            SignerRoleV1::ProofOutcome,
            SignerRoleV1::Repair,
            SignerRoleV1::Reserve,
            SignerRoleV1::Orderbook,
            SignerRoleV1::Promotion,
            SignerRoleV1::GovernanceDag,
            SignerRoleV1::PotrGateway,
            SignerRoleV1::PotrProvider,
            SignerRoleV1::BillingStatement,
            SignerRoleV1::EvidenceViewer,
            SignerRoleV1::StreamToken,
            SignerRoleV1::PopCredentials,
            SignerRoleV1::ReleaseManifest,
        ];
        let mut domains = std::collections::BTreeSet::new();
        for role in roles {
            assert_eq!(role.as_str().parse::<SignerRoleV1>().unwrap(), role);
            assert_eq!(role.to_string(), role.as_str());
            assert!(domains.insert(role.domain()));
            let bytes = norito::encode_canonical(&role).unwrap();
            assert_eq!(
                norito::decode_canonical::<SignerRoleV1>(&bytes).unwrap(),
                role
            );
        }
        for invalid in [
            "ReleaseManifest",
            "release-manifest",
            "release_manifest ",
            "software",
            "",
        ] {
            assert!(invalid.parse::<SignerRoleV1>().is_err());
        }
        let release = SignerPurposeBindingV1::ReleaseManifest {
            deployment_id: "production-primary".into(),
        };
        assert!(release.validates_role(SignerRoleV1::ReleaseManifest));
        assert!(!release.validates_role(SignerRoleV1::Promotion));
        assert!(
            !SignerPurposeBindingV1::NativeOrPromotion
                .validates_role(SignerRoleV1::ReleaseManifest)
        );
        assert!(SignerRoleV1::ReleaseManifest.allows_algorithm(SignerKeyAlgorithmV1::Ed25519));
        assert!(!SignerRoleV1::ReleaseManifest.allows_algorithm(SignerKeyAlgorithmV1::MlDsa));
    }

    #[test]
    fn signatures_commit_exact_order_bytes_messages_and_enforce_input_bounds() {
        let signature = SignerOperationSignatureV1 {
            purpose: SignerKeyOperationPurposeV1::RolePayload,
            message_digest: [1; 32],
            signature: vec![2; 64],
        };
        let mut other = signature.clone();
        other.purpose = SignerKeyOperationPurposeV1::AuditRecord;
        let expected =
            signer_operation_signatures_digest_v1(&[signature.clone(), other.clone()]).unwrap();
        assert_ne!(
            signer_operation_signatures_digest_v1(&[other.clone(), signature.clone()]).unwrap(),
            expected
        );
        other.signature[0] ^= 1;
        assert_ne!(
            signer_operation_signatures_digest_v1(&[signature.clone(), other.clone()]).unwrap(),
            expected
        );
        assert!(signer_operation_signatures_digest_v1(&[]).is_err());
        assert!(signer_operation_signatures_digest_v1(&vec![signature.clone(); 5]).is_err());
        other.signature = vec![0; 4097];
        assert!(signer_operation_signatures_digest_v1(&[other]).is_err());
        assert_ne!(
            signer_operation_message_digest_v1(b"ab"),
            signer_operation_message_digest_v1(b"a\0b")
        );
    }
}
