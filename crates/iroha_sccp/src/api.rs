//! Canonical SCCP capability and recent-message wire records shared by Torii and clients.
//!
//! Torii, SDK, and CLI consumers import these declarations directly. The fixed
//! protocol identities name the producer's original declarations independently
//! of this Rust module. Separate SDK declarations are retired, and their frame
//! names are not accepted variants of these records.

use iroha_data_model::prelude::Quantity;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
/// Fixed SCCP V1 route-registry capacity limits.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::routing::SccpRegistryLimitsDto")]
pub struct SccpRegistryLimits {
    /// Maximum governed lanes retained by the registry.
    #[norito(rename = "max_governed_lanes")]
    pub governed_lanes: u32,
    /// Maximum nonterminal route revisions across all lanes.
    #[norito(rename = "max_live_governed_routes")]
    pub live_governed_routes: u32,
    /// Maximum nonterminal route revisions in one lane.
    #[norito(rename = "max_live_routes_per_lane")]
    pub live_routes_per_lane: u32,
    /// Maximum retained route revisions in one lane, including retired revisions.
    #[norito(rename = "max_retained_routes_per_lane")]
    pub retained_routes_per_lane: u32,
    /// Maximum retained native trust anchors in one lane.
    #[norito(rename = "max_retained_native_trust_anchors_per_lane")]
    pub retained_native_trust_anchors_per_lane: u32,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
/// Consensus-critical SCCP proof and verifier-work limits.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::routing::SccpResourceLimitsDto")]
pub struct SccpResourceLimits {
    /// Maximum successful outbound SCCP messages committed by one block.
    #[norito(rename = "max_outbound_messages_per_block")]
    pub outbound_messages_per_block: u32,
    /// Maximum canonical bytes retained for one outbound SCCP payload.
    #[norito(rename = "max_outbound_message_payload_bytes")]
    pub outbound_message_payload_bytes: u64,
    /// Maximum payload-bearing outbound messages awaiting destination proof acceptance.
    #[norito(rename = "max_pending_outbound_messages")]
    pub pending_outbound_messages: u64,
    /// Maximum canonical outbound payload bytes awaiting destination proof acceptance.
    #[norito(rename = "max_pending_outbound_payload_bytes")]
    pub pending_outbound_payload_bytes: u64,
    /// Maximum closed SCCP proofs in one transaction.
    #[norito(rename = "max_proofs_per_transaction")]
    pub proofs_per_transaction: u32,
    /// Maximum closed SCCP proofs committed in one block.
    #[norito(rename = "max_proofs_per_block")]
    pub proofs_per_block: u32,
    /// Maximum canonical bytes retained for one closed SCCP proof.
    #[norito(rename = "max_proof_bytes_per_proof")]
    pub proof_bytes_per_proof: u64,
    /// Maximum aggregate SCCP proof bytes in one transaction.
    #[norito(rename = "max_proof_bytes_per_transaction")]
    pub proof_bytes_per_transaction: u64,
    /// Maximum aggregate SCCP proof bytes committed in one block.
    #[norito(rename = "max_proof_bytes_per_block")]
    pub proof_bytes_per_block: u64,
    /// Maximum native-finality continuation headers in one transaction.
    #[norito(rename = "max_native_headers_per_transaction")]
    pub native_headers_per_transaction: u32,
    /// Maximum native-finality continuation headers committed in one block.
    #[norito(rename = "max_native_headers_per_block")]
    pub native_headers_per_block: u32,
    /// Maximum Ethereum light-client updates in one transaction.
    #[norito(rename = "max_ethereum_light_client_updates_per_transaction")]
    pub ethereum_light_client_updates_per_transaction: u32,
    /// Maximum Ethereum light-client updates committed in one block.
    #[norito(rename = "max_ethereum_light_client_updates_per_block")]
    pub ethereum_light_client_updates_per_block: u32,
    /// Maximum framed native-finality header bytes in one transaction.
    #[norito(rename = "max_native_header_bytes_per_transaction")]
    pub native_header_bytes_per_transaction: u64,
    /// Maximum framed native-finality header bytes committed in one block.
    #[norito(rename = "max_native_header_bytes_per_block")]
    pub native_header_bytes_per_block: u64,
    /// Maximum secp256k1 recoveries in one transaction.
    #[norito(rename = "max_secp256k1_recoveries_per_transaction")]
    pub secp256k1_recoveries_per_transaction: u32,
    /// Maximum secp256k1 recoveries committed in one block.
    #[norito(rename = "max_secp256k1_recoveries_per_block")]
    pub secp256k1_recoveries_per_block: u32,
    /// Maximum BLS aggregate-signature checks in one transaction.
    #[norito(rename = "max_bls_aggregate_checks_per_transaction")]
    pub bls_aggregate_checks_per_transaction: u32,
    /// Maximum BLS aggregate-signature checks committed in one block.
    #[norito(rename = "max_bls_aggregate_checks_per_block")]
    pub bls_aggregate_checks_per_block: u32,
    /// Maximum BLS key-validation and signer-contribution work in one transaction.
    #[norito(rename = "max_bls_signer_contributions_per_transaction")]
    pub bls_signer_contributions_per_transaction: u32,
    /// Maximum BLS key-validation and signer-contribution work committed in one block.
    #[norito(rename = "max_bls_signer_contributions_per_block")]
    pub bls_signer_contributions_per_block: u32,
    /// Maximum Ed25519 signature checks in one transaction.
    #[norito(rename = "max_ed25519_signature_checks_per_transaction")]
    pub ed25519_signature_checks_per_transaction: u32,
    /// Maximum Ed25519 signature checks committed in one block.
    #[norito(rename = "max_ed25519_signature_checks_per_block")]
    pub ed25519_signature_checks_per_block: u32,
    /// Maximum TON Ed25519 validator-key checks in one transaction.
    #[norito(rename = "max_ed25519_validator_key_checks_per_transaction")]
    pub ed25519_validator_key_checks_per_transaction: u32,
    /// Maximum TON Ed25519 validator-key checks committed in one block.
    #[norito(rename = "max_ed25519_validator_key_checks_per_block")]
    pub ed25519_validator_key_checks_per_block: u32,
    /// Maximum BN254 pairing-product checks in one transaction.
    #[norito(rename = "max_bn254_pairing_checks_per_transaction")]
    pub bn254_pairing_checks_per_transaction: u32,
    /// Maximum BN254 pairing-product checks committed in one block.
    #[norito(rename = "max_bn254_pairing_checks_per_block")]
    pub bn254_pairing_checks_per_block: u32,
    /// Maximum BLS12-381 pairing-product checks in one transaction.
    #[norito(rename = "max_bls12_381_pairing_checks_per_transaction")]
    pub bls12_381_pairing_checks_per_transaction: u32,
    /// Maximum BLS12-381 pairing-product checks committed in one block.
    #[norito(rename = "max_bls12_381_pairing_checks_per_block")]
    pub bls12_381_pairing_checks_per_block: u32,
}

#[derive(
    Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
/// Public SCCP capability snapshot advertised by the node.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::routing::SccpCapabilitiesDto")]
pub struct SccpCapabilities {
    /// Capability schema version. First release is exactly `1`.
    pub version: u8,
    /// Hex-encoded digest of the authoritative typed route registry.
    pub registry_revision: String,
    /// Authoritative typed route-registry endpoint.
    pub registry_path: String,
    /// Finalized SORA message-bundle endpoint template.
    pub message_bundle_path: String,
    /// Query-free state-derived Groth16 request endpoint template.
    pub proof_request_path: String,
    /// Newest-first indexed outbound-message endpoint.
    pub recent_messages_path: String,
    /// Route-scoped SORA outbound contract-material endpoint template.
    pub sora_outbound_material_path: String,
    /// Fixed SCCP V1 route-registry capacity limits.
    pub registry_limits: SccpRegistryLimits,
    /// Consensus-critical proof and deterministic verifier-work limits.
    pub resource_limits: SccpResourceLimits,
    /// Closed destination-proof submission endpoint when the application API is enabled.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub proof_submit_path: Option<String>,
    /// Protocol-native inbound proof endpoint when the application API is enabled.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub native_message_submit_path: Option<String>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
/// Canonical readback and proof-request links for a recent SCCP message.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::routing::SccpRecentMessageLinksDto")]
pub struct SccpRecentMessageLinks {
    /// Canonical SCCP bundle lookup path.
    pub bundle_path: String,
    /// Query-free canonical Groth16 request lookup path.
    pub proof_request_path: String,
}

#[derive(
    Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
/// Compact newest-first SCCP outbound message discovery record.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::routing::SccpRecentMessageDto")]
pub struct SccpRecentMessage {
    /// Height of the finalized SORA block that anchored the message.
    pub height: u64,
    /// Zero-based position in the finalized SCCP commitment tree.
    pub commitment_index: u32,
    /// Hex-encoded canonical lane-bound SCCP message id.
    pub message_id_hex: String,
    /// Stable logical SCCP payload kind.
    pub kind: String,
    /// Exact SORA source profile committed by the message.
    pub source_profile: String,
    /// Exact external destination profile committed by the message.
    pub target_profile: String,
    /// Hex-encoded destination binding committed when the message was recorded.
    pub destination_binding_hash: String,
    /// Hex-encoded immutable governed route configuration.
    pub route_configuration_hash: String,
    /// Numeric SCCP target domain.
    pub target_domain: u32,
    /// Decoded asset id when representable as text.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    /// Decoded route id when representable as text.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub route_id: Option<String>,
    /// Decoded recipient when representable as text.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    /// Exact non-negative transfer quantity projected from the fixed SCCP scalar.
    pub amount: Quantity,
    /// Required normalized decoded payload projection.
    pub payload_projection: crate::SccpPayloadProjectionV1,
    /// Canonical bundle and proof-request links.
    pub links: SccpRecentMessageLinks,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
/// Compound continuation returned by recent SCCP discovery.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::routing::SccpRecentCursorDto")]
pub struct SccpRecentCursor {
    /// Height of the last returned item.
    pub from: u64,
    /// Commitment index of the last returned item.
    pub after_index: u32,
}

#[derive(
    Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
/// Newest-first committed SCCP message discovery response.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::routing::SccpRecentMessagesDto")]
pub struct SccpRecentMessages {
    /// Newest-first committed outbound SCCP messages.
    pub items: Vec<SccpRecentMessage>,
    /// Continuation for the next page when additional entries exist.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub next: Option<SccpRecentCursor>,
}
