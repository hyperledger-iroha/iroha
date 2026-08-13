//! Purpose-separated signing payloads for non-transaction runtime roles.
use norito::codec::{Decode, Encode};
use super::protocol::{
    SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1, SIGNER_PROTOCOL_VERSION_V1, SoftwareSignerPublicBindingV1,
    SoftwareSignerPurposeBindingV1, SoftwareSignerRoleV1,
};
const TYPED_PAYLOAD_MAGIC_V1: [u8; 8] = *b"IRSGTP01";
/// Exact purpose carried by one non-transaction external-signer request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[repr(u8)]
pub enum SoftwareSignerPurposeV1 {
    /// Canonical governance log-node signing payload.
    GovernanceLogNode = 1,
    /// Canonical Governance DAG block signing payload.
    GovernanceDagBlock = 2,
    /// Canonical Governance DAG head signing payload.
    GovernanceDagHead = 3,
    /// Predecessor-bound Governance DAG key transition.
    GovernanceKeyTransition = 15,
    /// Immutable Governance DAG qualification archive.
    GovernanceQualificationArchive = 16,
    /// Unsigned canonical PoTR receipt signed by the gateway key.
    PotrGatewayReceipt = 4,
    /// Unsigned canonical PoTR receipt signed by the provider key.
    PotrProviderReceipt = 5,
    /// Domain-separated governed billing-statement digest.
    BillingStatement = 6,
    /// Domain-prefixed evidence-view receipt message.
    EvidenceReceipt = 7,
    /// Domain-separated evidence checkpoint-store-record digest.
    EvidenceCheckpointStoreRecord = 8,
    /// Domain-prefixed evidence checkpoint-anchor message.
    EvidenceCheckpointAnchor = 9,
    /// Domain-separated evidence compaction-archive digest.
    EvidenceCompactionArchive = 10,
    /// Canonical domain-prefixed stream-token body.
    StreamToken = 11,
    /// Domain-separated PoP credential digest.
    PopCredential = 12,
    /// Domain-separated PoP commitment-root digest.
    PopCommitmentRoot = 13,
    /// Domain-separated PoP revocation-list digest.
    PopRevocationList = 14,
}
impl SoftwareSignerPurposeV1 {
    pub(super) const fn wire_id(self) -> u8 {
        self as u8
    }
    pub(super) const fn role(self) -> SoftwareSignerRoleV1 {
        match self {
            Self::GovernanceLogNode
            | Self::GovernanceDagBlock
            | Self::GovernanceDagHead
            | Self::GovernanceKeyTransition
            | Self::GovernanceQualificationArchive => SoftwareSignerRoleV1::GovernanceDag,
            Self::PotrGatewayReceipt => SoftwareSignerRoleV1::PotrGateway,
            Self::PotrProviderReceipt => SoftwareSignerRoleV1::PotrProvider,
            Self::BillingStatement => SoftwareSignerRoleV1::BillingStatement,
            Self::EvidenceReceipt
            | Self::EvidenceCheckpointStoreRecord
            | Self::EvidenceCheckpointAnchor
            | Self::EvidenceCompactionArchive => SoftwareSignerRoleV1::EvidenceViewer,
            Self::StreamToken => SoftwareSignerRoleV1::StreamToken,
            Self::PopCredential | Self::PopCommitmentRoot | Self::PopRevocationList => {
                SoftwareSignerRoleV1::PopCredentials
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct SoftwareSignerTypedPayloadV1 {
    magic: [u8; 8],
    version: u16,
    purpose: SoftwareSignerPurposeV1,
    message: Vec<u8>,
}
impl Drop for SoftwareSignerTypedPayloadV1 {
    fn drop(&mut self) {
        self.message.fill(0);
        let _ = std::hint::black_box(&self.message);
    }
}
pub(super) fn encode_typed_signing_payload(
    role: SoftwareSignerRoleV1,
    purpose: SoftwareSignerPurposeV1,
    message: &[u8],
) -> Result<Vec<u8>, ()> {
    if purpose.role() != role
        || message.is_empty()
        || message.len() > SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1
    {
        return Err(());
    }
    norito::encode_canonical(&SoftwareSignerTypedPayloadV1 {
        magic: TYPED_PAYLOAD_MAGIC_V1,
        version: SIGNER_PROTOCOL_VERSION_V1,
        purpose,
        message: message.to_vec(),
    })
    .map_err(|_| ())
}
pub(super) fn validated_typed_signing_message(
    binding: &SoftwareSignerPublicBindingV1,
    payload: &[u8],
) -> Result<Vec<u8>, ()> {
    if payload.is_empty() || payload.len() > SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1 {
        return Err(());
    }
    let typed: SoftwareSignerTypedPayloadV1 = norito::decode_canonical(payload).map_err(|_| ())?;
    if typed.magic != TYPED_PAYLOAD_MAGIC_V1
        || typed.version != SIGNER_PROTOCOL_VERSION_V1
        || typed.purpose.role() != binding.role
        || typed.message.is_empty()
        || typed.message.len() > SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1
        || !validate_message(binding, typed.purpose, &typed.message)
    {
        return Err(());
    }
    Ok(typed.message.clone())
}
fn validate_message(
    binding: &SoftwareSignerPublicBindingV1,
    purpose: SoftwareSignerPurposeV1,
    message: &[u8],
) -> bool {
    match purpose {
        SoftwareSignerPurposeV1::GovernanceLogNode => {
            governance_publisher(binding).is_some_and(|peer| {
                sorafs_manifest::governance::
                validate_governance_log_node_signing_payload_for_publisher_v1(message, peer)
                .is_ok()
            })
        }
        SoftwareSignerPurposeV1::GovernanceDagBlock => {
            governance_publisher(binding).is_some_and(|peer| {
                binding
                    .public_key
                    .try_to_bytes()
                    .is_ok_and(|(algorithm, key)| {
                        algorithm == iroha_crypto::Algorithm::Ed25519
                            && <[u8; 32]>::try_from(key).is_ok_and(|key| {
                                sorafs_manifest::governance::
                            validate_governance_dag_block_signing_payload_for_publisher_v1(
                                message, peer, key,
                            )
                            .is_ok()
                            })
                    })
            })
        }
        SoftwareSignerPurposeV1::GovernanceDagHead => {
            governance_publisher(binding).is_some_and(|peer| {
                sorafs_manifest::governance::
                validate_governance_dag_head_signing_payload_for_publisher_v1(message, peer)
                .is_ok()
            })
        }
        SoftwareSignerPurposeV1::GovernanceKeyTransition
        | SoftwareSignerPurposeV1::GovernanceQualificationArchive => {
            let purpose = match purpose {
                SoftwareSignerPurposeV1::GovernanceKeyTransition => {
                    sorafs_node::GovernanceDagSigningPurposeV1::KeyTransition
                }
                _ => sorafs_node::GovernanceDagSigningPurposeV1::QualificationArchive,
            };
            governance_publisher(binding).is_some_and(|peer| {
                binding
                    .public_key
                    .try_to_bytes()
                    .is_ok_and(|(algorithm, key)| {
                        algorithm == iroha_crypto::Algorithm::Ed25519
                            && <[u8; 32]>::try_from(key).is_ok_and(|key| {
                                sorafs_node::validate_governance_dag_control_signing_payload_v1(
                                    purpose, message, peer, key,
                                )
                                .is_ok()
                            })
                    })
            })
        }
        SoftwareSignerPurposeV1::PotrGatewayReceipt => validate_potr_payload(message, None),
        SoftwareSignerPurposeV1::PotrProviderReceipt => match &binding.purpose_binding {
            SoftwareSignerPurposeBindingV1::PotrProvider { provider_id, .. } => {
                validate_potr_payload(message, Some(*provider_id))
            }
            _ => false,
        },
        SoftwareSignerPurposeV1::BillingStatement
        | SoftwareSignerPurposeV1::EvidenceCheckpointStoreRecord
        | SoftwareSignerPurposeV1::EvidenceCompactionArchive
        | SoftwareSignerPurposeV1::PopCredential
        | SoftwareSignerPurposeV1::PopCommitmentRoot
        | SoftwareSignerPurposeV1::PopRevocationList => exact_nonzero_digest(message),
        SoftwareSignerPurposeV1::EvidenceReceipt
        | SoftwareSignerPurposeV1::EvidenceCheckpointAnchor => {
            let purpose = match purpose {
                SoftwareSignerPurposeV1::EvidenceReceipt => {
                    sorafs_node::evidence_viewer::EvidenceViewerSigningPurposeV1::Receipt
                }
                SoftwareSignerPurposeV1::EvidenceCheckpointAnchor => {
                    sorafs_node::evidence_viewer::EvidenceViewerSigningPurposeV1::CheckpointAnchor
                }
                _ => unreachable!("matched exact evidence signing purposes"),
            };
            binding
                .public_key
                .try_to_bytes()
                .is_ok_and(|(algorithm, key)| {
                    algorithm == iroha_crypto::Algorithm::Ed25519
                    && sorafs_node::evidence_viewer::validate_evidence_viewer_signing_message_v1(
                        purpose,
                        message,
                        &binding.handle,
                        key.try_into().unwrap_or([0; 32]),
                    )
                    .is_ok()
                })
        }
        SoftwareSignerPurposeV1::StreamToken => validate_stream_token_payload(message),
    }
}
fn exact_nonzero_digest(bytes: &[u8]) -> bool {
    bytes.len() == 32 && bytes.iter().any(|byte| *byte != 0)
}
fn validate_stream_token_payload(payload: &[u8]) -> bool {
    let Some(body_bytes) = payload
        .strip_prefix(sorafs_manifest::token::STREAM_TOKEN_SIGNATURE_DOMAIN_V1)
        .filter(|bytes| !bytes.is_empty())
    else {
        return false;
    };
    let Ok(body) = norito::decode_canonical::<sorafs_manifest::StreamTokenBodyV1>(body_bytes)
    else {
        return false;
    };
    iroha_torii::sorafs::token::validate_token_body(&body).is_ok()
        && body
            .signing_payload_bytes()
            .is_ok_and(|bytes| bytes == payload)
}
fn governance_publisher(binding: &SoftwareSignerPublicBindingV1) -> Option<&[u8]> {
    match &binding.purpose_binding {
        SoftwareSignerPurposeBindingV1::GovernanceDag { publisher_peer_id } => {
            Some(publisher_peer_id)
        }
        _ => None,
    }
}
fn validate_potr_payload(payload: &[u8], expected_provider_id: Option<[u8; 32]>) -> bool {
    let Some(receipt_bytes) = payload
        .strip_prefix(sorafs_manifest::POTR_RECEIPT_SIGNATURE_DOMAIN_V1)
        .filter(|bytes| !bytes.is_empty())
    else {
        return false;
    };
    let Ok(receipt) = norito::decode_canonical::<sorafs_manifest::PotrReceiptV1>(receipt_bytes)
    else {
        return false;
    };
    expected_provider_id.is_none_or(|provider_id| receipt.provider_id == provider_id)
        && receipt.gateway_signature.is_none()
        && receipt.provider_signature.is_none()
        && receipt.validate_unsigned().is_ok()
        && receipt
            .signing_payload_bytes()
            .is_ok_and(|bytes| bytes == payload)
}
