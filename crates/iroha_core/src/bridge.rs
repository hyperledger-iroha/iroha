//! Helpers for bridge finality proofs built from commit certificates.

use std::{collections::BTreeSet, fmt};

use iroha_data_model::{
    ChainId,
    block::{
        BlockHeader, SignedBlock,
        consensus_v2::finality::{V2FinalityArtifact, V2QuorumCertificateVerificationError},
    },
    bridge::{
        BRIDGE_FINALITY_PROOF_VERSION_V1, BridgeCommitment, BridgeFinalityBundle,
        BridgeFinalityProof, SccpGovernedRouteV1, SccpOutboundMessageKeyV1,
    },
    isi::InstructionBox,
    name::Name,
    transaction::{Executable, TransactionEntrypoint},
};
use iroha_sccp::{
    SccpGroth16Bn254ProofRequestV1, SccpHubCommitmentV1, SccpPayloadV1, TairaBridgeFinalityProofV1,
    TairaSccpMessageProofV1,
};
use mv::storage::StorageReadOnly;
use thiserror::Error;

use crate::{
    state::{State as CoreState, StateReadOnly},
    tx::AcceptedTransaction,
};

/// A Sumeragi-v2 finality artifact whose structure, roster PoPs, and CommitQC
/// cryptography have already been verified.
///
/// The wrapper is intentionally not decodable and exposes no mutable access.
/// Untrusted implementations of [`BridgeStateReadOnly`] must call [`Self::verify_for_header`]
/// to mint it. Kura-backed implementations use the private constructor only
/// after Kura's cache-backed verification boundary succeeds, and attach the
/// header authenticated by Kura's private durable finality record.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct VerifiedV2FinalityArtifact {
    artifact: V2FinalityArtifact,
    retained_header: BlockHeader,
}

impl VerifiedV2FinalityArtifact {
    /// Fully verify an untrusted artifact against its exact retained header.
    ///
    /// # Errors
    ///
    /// Returns the canonical v2 verification error when structural, PoP, or
    /// CommitQC cryptographic validation fails.
    pub fn verify_for_header(
        retained_header: BlockHeader,
        artifact: V2FinalityArtifact,
    ) -> Result<Self, V2QuorumCertificateVerificationError> {
        artifact.verify()?;
        artifact
            .validate_for_header(&retained_header)
            .map_err(V2QuorumCertificateVerificationError::InvalidArtifact)?;
        Ok(Self {
            artifact,
            retained_header,
        })
    }

    /// Borrow the verified artifact without allowing mutation.
    #[must_use]
    pub const fn artifact(&self) -> &V2FinalityArtifact {
        &self.artifact
    }

    /// Consume the wrapper and return the verified artifact.
    #[must_use]
    pub fn into_artifact(self) -> V2FinalityArtifact {
        self.artifact
    }

    /// Borrow Kura's authenticated retained header when one accompanied the artifact.
    #[must_use]
    pub const fn retained_header(&self) -> &BlockHeader {
        &self.retained_header
    }

    fn from_kura_verified(block_header: BlockHeader, artifact: V2FinalityArtifact) -> Self {
        Self {
            artifact,
            retained_header: block_header,
        }
    }
}

/// Narrow read-only surface used by bridge finality proof builders.
///
/// This keeps bridge-proof construction independent from full `StateView` snapshots.
pub trait BridgeStateReadOnly {
    /// Chain identifier bound to the state snapshot.
    fn bridge_chain_id(&self) -> &ChainId;
    /// Load an exact durable Sumeragi-v2 finality artifact whose structure,
    /// roster PoPs, and CommitQC cryptography have already been verified by the
    /// storage boundary.
    fn bridge_verified_v2_finality_artifact(
        &self,
        height: u64,
    ) -> Result<Option<VerifiedV2FinalityArtifact>, String>;
    /// Load verified finality and Kura's immutable SCCP archive in one bounded pass.
    ///
    /// Implementations must authenticate the exact retained header and deterministic
    /// commitment-index order without falling back to block bodies or mutable WSV payloads.
    fn bridge_verified_v2_finality_with_sccp_archive(
        &self,
        height: u64,
    ) -> Result<
        Option<(
            VerifiedV2FinalityArtifact,
            Vec<ValidatedSccpOutboundMessageProjectionV1>,
        )>,
        String,
    >;
}

impl<T: StateReadOnly> BridgeStateReadOnly for T {
    fn bridge_chain_id(&self) -> &ChainId {
        self.chain_id()
    }

    fn bridge_verified_v2_finality_artifact(
        &self,
        height: u64,
    ) -> Result<Option<VerifiedV2FinalityArtifact>, String> {
        self.kura()
            .v2_finality_artifact_with_header(height)
            .map(|record| {
                record.map(|(header, artifact)| {
                    VerifiedV2FinalityArtifact::from_kura_verified(header, artifact)
                })
            })
            .map_err(|error| error.to_string())
    }

    fn bridge_verified_v2_finality_with_sccp_archive(
        &self,
        height: u64,
    ) -> Result<
        Option<(
            VerifiedV2FinalityArtifact,
            Vec<ValidatedSccpOutboundMessageProjectionV1>,
        )>,
        String,
    > {
        self.kura()
            .v2_finality_artifact_with_archive(height)
            .map(|record| {
                record.map(|(header, artifact, archive)| {
                    (
                        VerifiedV2FinalityArtifact::from_kura_verified(header, artifact),
                        archive,
                    )
                })
            })
            .map_err(|error| error.to_string())
    }
}

impl BridgeStateReadOnly for CoreState {
    fn bridge_chain_id(&self) -> &ChainId {
        self.chain_id_ref()
    }

    fn bridge_verified_v2_finality_artifact(
        &self,
        height: u64,
    ) -> Result<Option<VerifiedV2FinalityArtifact>, String> {
        self.kura()
            .v2_finality_artifact_with_header(height)
            .map(|record| {
                record.map(|(header, artifact)| {
                    VerifiedV2FinalityArtifact::from_kura_verified(header, artifact)
                })
            })
            .map_err(|error| error.to_string())
    }

    fn bridge_verified_v2_finality_with_sccp_archive(
        &self,
        height: u64,
    ) -> Result<
        Option<(
            VerifiedV2FinalityArtifact,
            Vec<ValidatedSccpOutboundMessageProjectionV1>,
        )>,
        String,
    > {
        self.kura()
            .v2_finality_artifact_with_archive(height)
            .map(|record| {
                record.map(|(header, artifact, archive)| {
                    (
                        VerifiedV2FinalityArtifact::from_kura_verified(header, artifact),
                        archive,
                    )
                })
            })
            .map_err(|error| error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Decoded SCCP message plus its location in a transaction stream.
pub struct RecordedSccpMessage {
    /// Zero-based index of the transaction that emitted the SCCP message.
    pub tx_index: usize,
    /// Zero-based index of the instruction within the transaction executable.
    pub instruction_index: usize,
    /// Exact governed lane and destination binding supplied by the instruction.
    pub context: iroha_data_model::bridge::SccpOutboundMessageContextV1,
    /// Canonically decoded SCCP payload recorded by the instruction.
    pub payload: SccpPayloadV1,
    /// Commitment derived from the decoded SCCP payload.
    pub commitment: SccpHubCommitmentV1,
}

/// Canonically validated outbound SCCP record data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedRecordedSccpMessage {
    /// Exact structurally validated outbound context.
    pub context: iroha_data_model::bridge::SccpOutboundMessageContextV1,
    /// Canonically decoded SCCP payload recorded by the instruction.
    pub payload: SccpPayloadV1,
    /// Exact lane-bound outbound replay key derived from the context and payload.
    pub key: SccpOutboundMessageKeyV1,
    /// Canonical SCCP hub commitment for the payload.
    pub commitment: SccpHubCommitmentV1,
}

/// Location-free, fully validated projection of one finalized SCCP outbox message.
///
/// The projection is safe for read APIs to render directly: Core has verified exact canonical
/// payload framing and semantics, the lane-bound message identifier, all structural context
/// roles, and the payload commitment against the supplied durable record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSccpOutboundMessageProjectionV1 {
    /// Zero-based position authenticated by the block's SCCP commitment root.
    pub commitment_index: u32,
    /// Exact outbound lane and governed binding context retained by the record.
    pub context: iroha_data_model::bridge::SccpOutboundMessageContextV1,
    /// Canonically decoded SCCP V1 application payload.
    pub payload: SccpPayloadV1,
    /// Recomputed lane-, context-, message-, and payload-bound hub commitment.
    pub commitment: SccpHubCommitmentV1,
}

impl ValidatedRecordedSccpMessage {
    /// Build the payload-bearing pending outbox record from this canonical validation result.
    pub(crate) fn outbound_record(
        &self,
        recorded_at_height: u64,
        commitment_index: u32,
    ) -> Option<iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1> {
        let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(&self.payload).ok()?;
        let record = iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1 {
            destination_binding_hash: self.context.destination_binding_hash,
            route_configuration_hash: self.context.route_configuration_hash,
            payload_hash: self.commitment.payload_hash,
            payload_bytes,
            recorded_at_height,
            commitment_index,
        };
        validate_sccp_outbound_message_record_internal(&self.key, &record)
            .is_ok()
            .then_some(record)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedSccpMessageCandidate {
    instruction_index: usize,
    validated: ValidatedRecordedSccpMessage,
}

/// Failure while validating a recorded outbound SCCP message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecordedSccpMessageValidationError {
    /// The supplied context is not an exact SORA-to-external lane with a nonzero binding.
    InvalidContext,
    /// Payload bytes are not exact canonical SCCP variant framing.
    InvalidPayload,
    /// Outbound records may only originate from SORA.
    NonSoraSource {
        /// Source domain found in the payload.
        source_domain: u32,
    },
    /// The payload destination domain does not match the context's exact target profile.
    TargetProfileMismatch {
        /// Exact target profile declared by the context.
        target: iroha_data_model::bridge::SccpNetworkV1,
        /// SCCP destination domain encoded by the payload.
        payload_target_domain: u32,
    },
    /// The destination binding, lane-bound message id, and payload hash are not distinct.
    HashRoleCollision,
    /// Payload route fields are not bound to the deterministic outbound route.
    RouteBinding {
        /// Route binding validation error.
        error: SccpOutboundRouteValidationError,
    },
}

pub(crate) fn decode_recorded_sccp_payload_bytes(payload_bytes: &[u8]) -> Option<SccpPayloadV1> {
    let payload = iroha_sccp::decode_canonical_sccp_payload_bytes(payload_bytes)?;
    if iroha_sccp::canonical_sccp_payload_bytes(&payload)
        .ok()?
        .as_slice()
        != payload_bytes
    {
        return None;
    }
    Some(payload)
}

#[cfg(test)]
pub(crate) fn test_sccp_outbound_context_for_payload_bytes(
    payload_bytes: &[u8],
) -> iroha_data_model::bridge::SccpOutboundMessageContextV1 {
    use iroha_data_model::bridge::{SccpLaneIdV1, SccpNetworkV1};

    let target_domain = decode_recorded_sccp_payload_bytes(payload_bytes)
        .map(|payload| iroha_sccp::sccp_message_target_domain(&payload))
        .filter(|domain| *domain != iroha_sccp::SCCP_DOMAIN_SORA)
        .unwrap_or(iroha_sccp::SCCP_DOMAIN_ETH);
    let target = match target_domain {
        iroha_sccp::SCCP_DOMAIN_ETH => Some(SccpNetworkV1::EthereumMainnet),
        iroha_sccp::SCCP_DOMAIN_BSC => Some(SccpNetworkV1::BscMainnet),
        iroha_sccp::SCCP_DOMAIN_TRON => Some(SccpNetworkV1::TronMainnet),
        _ => None,
    }
    .unwrap_or(SccpNetworkV1::EthereumMainnet);
    let (destination_binding_hash, route_configuration_hash) = if matches!(
        target_domain,
        iroha_sccp::SCCP_DOMAIN_ETH | iroha_sccp::SCCP_DOMAIN_BSC
    ) {
        let route = iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
            target,
            iroha_data_model::bridge::SccpRouteActivationV1::Staged,
        );
        (
            route
                .destination_binding_hash()
                .expect("exact test EVM destination binding"),
            route
                .route_configuration_hash()
                .expect("exact test EVM route configuration"),
        )
    } else {
        ([0x36; 32], [0x37; 32])
    };
    iroha_data_model::bridge::SccpOutboundMessageContextV1::new(
        SccpLaneIdV1 {
            source: SccpNetworkV1::SoraTaira,
            target,
        },
        destination_binding_hash,
        route_configuration_hash,
    )
    .expect("test SCCP outbound context must be valid")
}

#[cfg(test)]
pub(crate) fn test_record_sccp_message(
    payload_bytes: Vec<u8>,
) -> iroha_data_model::isi::bridge::RecordSccpMessage {
    let context = test_sccp_outbound_context_for_payload_bytes(&payload_bytes);
    iroha_data_model::isi::bridge::RecordSccpMessage::new(context, payload_bytes)
}

#[cfg(test)]
pub(crate) fn test_sccp_outbound_message_key(payload: &SccpPayloadV1) -> SccpOutboundMessageKeyV1 {
    let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(payload)
        .expect("valid SCCP outbound-key fixture payload encodes");
    let context = test_sccp_outbound_context_for_payload_bytes(&payload_bytes);
    sccp_outbound_message_key(context.lane, payload).expect("test SCCP outbound key must be valid")
}

#[cfg(test)]
pub(crate) fn test_sccp_hub_commitment(payload: &SccpPayloadV1) -> SccpHubCommitmentV1 {
    let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(payload)
        .expect("valid SCCP commitment fixture payload encodes");
    let context = test_sccp_outbound_context_for_payload_bytes(&payload_bytes);
    iroha_sccp::hub_commitment_from_sccp_payload(context, payload)
        .expect("test SCCP hub commitment must be valid")
}

fn validate_recorded_sccp_payload(
    context: iroha_data_model::bridge::SccpOutboundMessageContextV1,
    payload: SccpPayloadV1,
) -> Result<ValidatedRecordedSccpMessage, RecordedSccpMessageValidationError> {
    if !context.is_well_formed() {
        return Err(RecordedSccpMessageValidationError::InvalidContext);
    }
    let source_domain = iroha_sccp::sccp_message_source_domain(&payload);
    if source_domain != iroha_sccp::SCCP_DOMAIN_SORA {
        return Err(RecordedSccpMessageValidationError::NonSoraSource { source_domain });
    }
    let payload_target_domain = iroha_sccp::sccp_message_target_domain(&payload);
    if payload_target_domain != context.lane.target.domain_id() {
        return Err(RecordedSccpMessageValidationError::TargetProfileMismatch {
            target: context.lane.target,
            payload_target_domain,
        });
    }
    validate_sora_outbound_sccp_payload_route(&payload)
        .map_err(|error| RecordedSccpMessageValidationError::RouteBinding { error })?;
    if !iroha_sccp::verify_sccp_payload_structure(&payload) {
        return Err(RecordedSccpMessageValidationError::InvalidPayload);
    }
    let key = sccp_outbound_message_key(context.lane, &payload)
        .ok_or(RecordedSccpMessageValidationError::InvalidContext)?;
    let commitment = iroha_sccp::hub_commitment_from_sccp_payload(context, &payload)
        .ok_or(RecordedSccpMessageValidationError::InvalidContext)?;
    let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(&payload)
        .map_err(|_| RecordedSccpMessageValidationError::InvalidPayload)?;
    let durable = iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1 {
        destination_binding_hash: context.destination_binding_hash,
        route_configuration_hash: context.route_configuration_hash,
        payload_hash: commitment.payload_hash,
        payload_bytes,
        recorded_at_height: 1,
        commitment_index: 0,
    };
    if !durable.is_well_formed_for_key(&key) {
        return Err(RecordedSccpMessageValidationError::HashRoleCollision);
    }
    Ok(ValidatedRecordedSccpMessage {
        context,
        key,
        commitment,
        payload,
    })
}

pub(crate) fn validate_recorded_sccp_message_payload_bytes(
    context: iroha_data_model::bridge::SccpOutboundMessageContextV1,
    payload_bytes: &[u8],
) -> Result<ValidatedRecordedSccpMessage, RecordedSccpMessageValidationError> {
    if payload_bytes.is_empty()
        || payload_bytes.len()
            > iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1
    {
        return Err(RecordedSccpMessageValidationError::InvalidPayload);
    }
    let payload = decode_recorded_sccp_payload_bytes(payload_bytes)
        .ok_or(RecordedSccpMessageValidationError::InvalidPayload)?;
    validate_recorded_sccp_payload(context, payload)
}

/// Fully revalidate one payload-bearing pending SCCP outbox record against its replay key.
///
/// This is intentionally stronger than the data-model structural predicate: it decodes and
/// re-encodes the retained payload, verifies SCCP V1 semantics, recomputes the lane-bound message
/// identifier and payload commitment, and binds both governed context hashes to the record.
fn validate_sccp_outbound_message_record_internal(
    key: &SccpOutboundMessageKeyV1,
    record: &iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1,
) -> Result<ValidatedRecordedSccpMessage, RecordedSccpMessageValidationError> {
    if !record.is_well_formed_for_key(key) {
        return Err(RecordedSccpMessageValidationError::InvalidContext);
    }
    let context = iroha_data_model::bridge::SccpOutboundMessageContextV1 {
        lane: key.lane,
        destination_binding_hash: record.destination_binding_hash,
        route_configuration_hash: record.route_configuration_hash,
    };
    let validated = validate_recorded_sccp_message_payload_bytes(context, &record.payload_bytes)?;
    if validated.key != *key || validated.commitment.payload_hash != record.payload_hash {
        return Err(RecordedSccpMessageValidationError::InvalidPayload);
    }
    Ok(validated)
}

/// Validate and project one payload-bearing pending SCCP outbox record.
///
/// Returns `None` unless the retained bytes are bounded, exact canonical SCCP V1 framing; decode
/// to a structurally and semantically valid SORA-origin payload for the supplied lane; recompute
/// the supplied lane-bound message identifier; and match the record's payload hash and governed
/// context roles. Registry hydration separately proves those context hashes name one retained
/// governed route before the state becomes observable.
#[must_use]
pub fn validate_sccp_outbound_message_record_v1(
    key: &SccpOutboundMessageKeyV1,
    record: &iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1,
) -> Option<ValidatedSccpOutboundMessageProjectionV1> {
    let validated = validate_sccp_outbound_message_record_internal(key, record).ok()?;
    Some(ValidatedSccpOutboundMessageProjectionV1 {
        commitment_index: record.commitment_index,
        context: validated.context,
        payload: validated.payload,
        commitment: validated.commitment,
    })
}

/// Return the next dense commitment index for one block height.
///
/// `None` means the height already contains the exact first-release maximum. The range includes
/// writes staged by the current state transaction, so dropping a failed transaction also releases
/// every index it tentatively allocated.
pub(crate) fn next_sccp_outbound_commitment_index(
    ordered: &impl StorageReadOnly<iroha_data_model::bridge::SccpOutboundMessageIndexKeyV1, ()>,
    height: u64,
) -> Result<Option<u32>, String> {
    if height == 0 {
        return Err("SCCP outbound commitment height must be nonzero".to_owned());
    }
    let max = iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1;
    let start =
        iroha_data_model::bridge::SccpOutboundMessageIndexKeyV1::range_start_at_or_before(height);
    let mut expected = 0_u32;
    for (index, ()) in ordered.range(start..) {
        if index.recorded_at_height != height {
            break;
        }
        if expected >= max {
            return Err(format!(
                "SCCP outbound index at height {height} exceeds the {max}-message block bound"
            ));
        }
        if !index.is_well_formed() || index.commitment_index != expected {
            return Err(format!(
                "SCCP outbound index at height {height} is not dense: expected {expected}, found {}",
                index.commitment_index
            ));
        }
        expected += 1;
    }
    Ok((expected < max).then_some(expected))
}

fn validate_recorded_sccp_message_payload_bytes_for_block_collection(
    context: iroha_data_model::bridge::SccpOutboundMessageContextV1,
    payload_bytes: &[u8],
) -> Result<ValidatedRecordedSccpMessage, RecordedSccpMessageValidationError> {
    validate_recorded_sccp_message_payload_bytes(context, payload_bytes)
}

pub(crate) fn sccp_outbound_message_key(
    lane: iroha_data_model::bridge::SccpLaneIdV1,
    payload: &SccpPayloadV1,
) -> Option<SccpOutboundMessageKeyV1> {
    SccpOutboundMessageKeyV1::new(lane, iroha_sccp::sccp_message_id(lane, payload)?)
}

fn recorded_sccp_message_instruction(
    instruction: &InstructionBox,
) -> Option<&iroha_data_model::isi::bridge::RecordSccpMessage> {
    instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::bridge::RecordSccpMessage>()
}

fn validate_recorded_sccp_message_instruction(
    instruction: &InstructionBox,
) -> Result<Option<ValidatedRecordedSccpMessage>, RecordedSccpMessageValidationError> {
    let Some(record) = recorded_sccp_message_instruction(instruction) else {
        return Ok(None);
    };
    validate_recorded_sccp_message_payload_bytes(record.context, &record.payload_bytes).map(Some)
}

fn signed_transaction_from_sccp_entrypoint(
    entrypoint: &TransactionEntrypoint,
) -> Option<&iroha_data_model::transaction::SignedTransaction> {
    match entrypoint {
        TransactionEntrypoint::External(transaction) => Some(transaction),
        TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction()),
        TransactionEntrypoint::SealedCommitment(_)
        | TransactionEntrypoint::PrivateKaigi(_)
        | TransactionEntrypoint::Time(_) => None,
    }
}

fn entrypoint_has_successful_or_pending_result(
    block: &SignedBlock,
    entrypoint_index: usize,
) -> bool {
    if !block.has_results() {
        return true;
    }

    block
        .results()
        .nth(entrypoint_index)
        .is_some_and(|result| result.as_ref().is_ok())
}

/// Invalid route binding for a SORA-origin outbound SCCP payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SccpOutboundRouteValidationError {
    /// Route id is not encoded as SCCP `canonical_text`.
    NonTextRouteId,
    /// Asset id is not encoded as SCCP `canonical_text`.
    NonTextAssetId,
    /// Route id bytes are not valid UTF-8.
    InvalidRouteIdUtf8,
    /// Text route id is empty.
    EmptyRouteId,
    /// Asset id bytes are not valid UTF-8.
    InvalidAssetIdUtf8,
    /// Text asset id has no route-local asset key.
    EmptyAssetKey,
    /// Text asset id route-local key is not an Iroha `Name`.
    InvalidAssetKey,
    /// Text asset id contains `#` without a scope suffix.
    EmptyAssetScope,
    /// Text asset id contains more than one `#` scope separator.
    AmbiguousAssetScope,
    /// Text asset id uses a scope suffix instead of the canonical route-local key.
    AssetScopeAlias {
        /// Route-local asset key extracted from the scoped spelling.
        asset_key: String,
        /// Scope suffix found in the payload.
        scope: String,
    },
    /// Asset home domain is not SORA in the first-release lock/release model.
    InvalidAssetHomeDomain {
        /// Asset home domain in the payload.
        asset_home_domain: u32,
        /// Destination domain in the payload.
        dest_domain: u32,
    },
}

impl SccpOutboundRouteValidationError {
    pub(crate) fn reason(&self) -> &'static str {
        match self {
            Self::NonTextRouteId => "RecordSccpMessage payload route_id is not canonical_text",
            Self::NonTextAssetId => "RecordSccpMessage payload asset_id is not canonical_text",
            Self::InvalidRouteIdUtf8 => "RecordSccpMessage payload route_id is invalid UTF-8",
            Self::EmptyRouteId => "RecordSccpMessage payload route_id is empty",
            Self::InvalidAssetIdUtf8 => "RecordSccpMessage payload asset_id is invalid UTF-8",
            Self::EmptyAssetKey => "RecordSccpMessage payload asset key is empty",
            Self::InvalidAssetKey => "RecordSccpMessage payload asset key is not a valid Name",
            Self::EmptyAssetScope => "RecordSccpMessage payload asset scope is empty",
            Self::AmbiguousAssetScope => {
                "RecordSccpMessage payload asset_id has multiple scope separators"
            }
            Self::AssetScopeAlias { .. } => {
                "RecordSccpMessage payload asset_id must be the canonical route-local asset key"
            }
            Self::InvalidAssetHomeDomain { .. } => {
                "RecordSccpMessage payload asset home domain is not SORA"
            }
        }
    }
}

impl fmt::Display for SccpOutboundRouteValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonTextRouteId => write!(f, "route_id must use canonical_text codec"),
            Self::NonTextAssetId => write!(f, "asset_id must use canonical_text codec"),
            Self::InvalidRouteIdUtf8 => write!(f, "route_id must be valid UTF-8"),
            Self::EmptyRouteId => write!(f, "route_id must not be empty"),
            Self::InvalidAssetIdUtf8 => write!(f, "asset_id must be valid UTF-8"),
            Self::EmptyAssetKey => write!(f, "asset key must not be empty"),
            Self::InvalidAssetKey => write!(f, "asset key must be a valid Iroha Name"),
            Self::EmptyAssetScope => write!(f, "asset scope must not be empty after `#`"),
            Self::AmbiguousAssetScope => {
                write!(f, "asset_id must contain at most one `#` scope separator")
            }
            Self::AssetScopeAlias { asset_key, scope } => write!(
                f,
                "asset_id must be canonical route-local key `{asset_key}`, not scoped alias `{asset_key}#{scope}`"
            ),
            Self::InvalidAssetHomeDomain {
                asset_home_domain,
                dest_domain: _,
            } => write!(f, "asset home domain {asset_home_domain} must be SORA"),
        }
    }
}

fn sccp_text_route_field<'a>(
    codec: u8,
    bytes: &'a [u8],
    non_text: SccpOutboundRouteValidationError,
    invalid_utf8: SccpOutboundRouteValidationError,
) -> Result<&'a str, SccpOutboundRouteValidationError> {
    if codec != iroha_sccp::SCCP_CODEC_CANONICAL_TEXT {
        return Err(non_text);
    }
    core::str::from_utf8(bytes).map_err(|_| invalid_utf8)
}

fn sccp_route_asset_key(asset_id: &str) -> Result<&str, SccpOutboundRouteValidationError> {
    let mut parts = asset_id.split('#');
    let asset_key = parts.next().unwrap_or_default();
    if asset_key.is_empty() {
        return Err(SccpOutboundRouteValidationError::EmptyAssetKey);
    }
    if asset_key.parse::<Name>().is_err() {
        return Err(SccpOutboundRouteValidationError::InvalidAssetKey);
    }
    if let Some(scope) = parts.next() {
        if scope.is_empty() {
            return Err(SccpOutboundRouteValidationError::EmptyAssetScope);
        }
        if parts.next().is_some() {
            return Err(SccpOutboundRouteValidationError::AmbiguousAssetScope);
        }
        return Err(SccpOutboundRouteValidationError::AssetScopeAlias {
            asset_key: asset_key.to_owned(),
            scope: scope.to_owned(),
        });
    }
    Ok(asset_key)
}

fn validate_sora_outbound_transfer_route(
    transfer: &iroha_sccp::TransferPayloadV1,
) -> Result<(), SccpOutboundRouteValidationError> {
    let route_id = sccp_text_route_field(
        transfer.route_id_codec,
        transfer.route_id.as_slice(),
        SccpOutboundRouteValidationError::NonTextRouteId,
        SccpOutboundRouteValidationError::InvalidRouteIdUtf8,
    )?;
    let asset_id = sccp_text_route_field(
        transfer.asset_id_codec,
        transfer.asset_id.as_slice(),
        SccpOutboundRouteValidationError::NonTextAssetId,
        SccpOutboundRouteValidationError::InvalidAssetIdUtf8,
    )?;
    if route_id.is_empty() {
        return Err(SccpOutboundRouteValidationError::EmptyRouteId);
    }
    sccp_route_asset_key(asset_id)?;
    if transfer.asset_home_domain != iroha_sccp::SCCP_DOMAIN_SORA {
        return Err(SccpOutboundRouteValidationError::InvalidAssetHomeDomain {
            asset_home_domain: transfer.asset_home_domain,
            dest_domain: transfer.dest_domain,
        });
    }
    Ok(())
}

/// Validate deterministic route binding for SORA-origin outbound SCCP records.
pub(crate) fn validate_sora_outbound_sccp_payload_route(
    payload: &SccpPayloadV1,
) -> Result<(), SccpOutboundRouteValidationError> {
    let SccpPayloadV1::Transfer(transfer) = payload;
    validate_sora_outbound_transfer_route(transfer)
}

fn collect_sccp_messages_from_executable<F>(
    tx_index: usize,
    executable: &Executable,
    seen: &mut BTreeSet<SccpOutboundMessageKeyV1>,
    is_already_recorded: &F,
    deduplicate: bool,
    out: &mut Vec<RecordedSccpMessage>,
) where
    F: Fn(&SccpOutboundMessageKeyV1) -> bool,
{
    let mut push_instruction = |instruction_index: usize, instruction: &InstructionBox| {
        let Ok(Some(validated)) = validate_recorded_sccp_message_instruction(instruction) else {
            return;
        };
        let key = validated.key.clone();
        if is_already_recorded(&key) {
            return;
        }
        if deduplicate {
            if seen.contains(&key) {
                return;
            }
            seen.insert(key);
        }
        out.push(RecordedSccpMessage {
            tx_index,
            instruction_index,
            context: validated.context,
            commitment: validated.commitment,
            payload: validated.payload,
        });
    };

    match executable {
        Executable::Instructions(instructions) => {
            for (instruction_index, instruction) in instructions.iter().enumerate() {
                push_instruction(instruction_index, instruction);
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::IvmProved(proved) => {
            for (instruction_index, instruction) in proved.overlay.iter().enumerate() {
                push_instruction(instruction_index, instruction);
            }
        }
    }
}

fn sccp_message_candidates_from_executable(
    executable: &Executable,
) -> Vec<RecordedSccpMessageCandidate> {
    let instructions = match executable {
        Executable::Instructions(instructions) => instructions.as_ref(),
        Executable::IvmProved(proved) => proved.overlay.as_ref(),
        Executable::ContractCall(_) | Executable::Ivm(_) => return Vec::new(),
    };

    instructions
        .iter()
        .enumerate()
        .filter_map(|(instruction_index, instruction)| {
            let record = recorded_sccp_message_instruction(instruction)?;
            let Ok(validated) = validate_recorded_sccp_message_payload_bytes_for_block_collection(
                record.context,
                &record.payload_bytes,
            ) else {
                return None;
            };
            Some(RecordedSccpMessageCandidate {
                instruction_index,
                validated,
            })
        })
        .collect()
}

/// Extract all SCCP message records from accepted signed entrypoints.
pub fn collect_sccp_messages_from_accepted_transactions(
    transactions: &[AcceptedTransaction<'_>],
) -> Vec<RecordedSccpMessage> {
    collect_new_sccp_messages_from_accepted_transactions(transactions, |_| false)
}

/// Extract newly recordable SCCP message records from accepted signed entrypoints.
///
/// Existing outbox keys are excluded so proposal headers do not commit messages
/// that execution will reject as outbound replays.
pub fn collect_new_sccp_messages_from_accepted_transactions<F>(
    transactions: &[AcceptedTransaction<'_>],
    is_already_recorded: F,
) -> Vec<RecordedSccpMessage>
where
    F: Fn(&SccpOutboundMessageKeyV1) -> bool,
{
    collect_new_sccp_messages_from_accepted_transactions_where(
        transactions,
        |_| true,
        is_already_recorded,
    )
}

/// Extract newly recordable SCCP message records from selected accepted signed entrypoints.
///
/// The transaction-index filter preserves canonical block entrypoint indices in
/// the returned messages while letting proposal assembly exclude transactions
/// whose refreshed routing context cannot execute outbound SCCP records.
pub(crate) fn collect_new_sccp_messages_from_accepted_transactions_where<F, G>(
    transactions: &[AcceptedTransaction<'_>],
    include_transaction_index: F,
    is_already_recorded: G,
) -> Vec<RecordedSccpMessage>
where
    F: Fn(usize) -> bool,
    G: Fn(&SccpOutboundMessageKeyV1) -> bool,
{
    let mut messages = Vec::new();
    let mut seen = BTreeSet::new();
    for (tx_index, transaction) in transactions.iter().enumerate() {
        if !include_transaction_index(tx_index) {
            continue;
        }
        if let Some(signed) = signed_transaction_from_sccp_entrypoint(transaction.entrypoint()) {
            collect_sccp_messages_from_executable(
                tx_index,
                signed.instructions(),
                &mut seen,
                &is_already_recorded,
                true,
                &mut messages,
            );
        }
    }
    messages
}

/// Extract SCCP message records from one accepted signed entrypoint without deduplicating them.
#[cfg(test)]
pub(crate) fn collect_sccp_messages_from_accepted_transaction(
    tx_index: usize,
    transaction: &AcceptedTransaction<'_>,
) -> Vec<RecordedSccpMessage> {
    let mut messages = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(signed) = signed_transaction_from_sccp_entrypoint(transaction.entrypoint()) {
        collect_sccp_messages_from_executable(
            tx_index,
            signed.instructions(),
            &mut seen,
            &|_| false,
            false,
            &mut messages,
        );
    }
    messages
}

fn collect_sccp_messages_from_signed_block_with_deduplication(
    block: &SignedBlock,
    deduplicate: bool,
) -> Vec<RecordedSccpMessage> {
    let mut messages = Vec::new();
    let mut seen = BTreeSet::new();
    for (entrypoint_index, entrypoint) in block.external_entrypoints_cloned().enumerate() {
        let transaction = match entrypoint {
            TransactionEntrypoint::External(transaction) => transaction,
            TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction().clone(),
            TransactionEntrypoint::SealedCommitment(_)
            | TransactionEntrypoint::PrivateKaigi(_)
            | TransactionEntrypoint::Time(_) => continue,
        };
        if !entrypoint_has_successful_or_pending_result(block, entrypoint_index) {
            continue;
        }
        let candidates = sccp_message_candidates_from_executable(transaction.instructions());
        for candidate in candidates {
            let key = candidate.validated.key.clone();
            if deduplicate {
                if seen.contains(&key) {
                    continue;
                }
                seen.insert(key);
            }
            messages.push(RecordedSccpMessage {
                tx_index: entrypoint_index,
                instruction_index: candidate.instruction_index,
                context: candidate.validated.context,
                commitment: candidate.validated.commitment,
                payload: candidate.validated.payload,
            });
        }
    }
    messages
}

/// Malformed committed SCCP record instruction found in a successful or pending entrypoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SccpRecordInstructionValidationError {
    /// `RecordSccpMessage` payload bytes could not be decoded as a valid SCCP payload.
    InvalidPayload {
        /// External entrypoint index in the block payload.
        tx_index: usize,
        /// Instruction index inside the IVM overlay.
        instruction_index: usize,
    },
    /// `RecordSccpMessage` context is malformed or does not describe an outbound lane.
    InvalidContext {
        /// External entrypoint index in the block payload.
        tx_index: usize,
        /// Instruction index inside the IVM overlay.
        instruction_index: usize,
    },
    /// `RecordSccpMessage` payload decoded, but its source domain is not SORA.
    NonSoraSource {
        /// External entrypoint index in the block payload.
        tx_index: usize,
        /// Instruction index inside the IVM overlay.
        instruction_index: usize,
        /// Source domain encoded in the SCCP payload.
        source_domain: u32,
    },
    /// Payload destination domain does not match the exact target profile.
    TargetProfileMismatch {
        /// External entrypoint index in the block payload.
        tx_index: usize,
        /// Instruction index inside the IVM overlay.
        instruction_index: usize,
        /// Exact target profile declared by the context.
        target: iroha_data_model::bridge::SccpNetworkV1,
        /// SCCP destination domain encoded by the payload.
        payload_target_domain: u32,
    },
    /// Context binding, message id, and payload hash collide across semantic roles.
    HashRoleCollision {
        /// External entrypoint index in the block payload.
        tx_index: usize,
        /// Instruction index inside the IVM overlay.
        instruction_index: usize,
    },
    /// `RecordSccpMessage` payload decoded, but outbound route binding is invalid.
    RouteBinding {
        /// External entrypoint index in the block payload.
        tx_index: usize,
        /// Instruction index inside the IVM overlay.
        instruction_index: usize,
        /// Route-binding validation error.
        error: SccpOutboundRouteValidationError,
    },
}

impl SccpRecordInstructionValidationError {
    pub(crate) fn tx_index(&self) -> usize {
        match self {
            Self::InvalidPayload { tx_index, .. }
            | Self::InvalidContext { tx_index, .. }
            | Self::NonSoraSource { tx_index, .. }
            | Self::TargetProfileMismatch { tx_index, .. }
            | Self::HashRoleCollision { tx_index, .. }
            | Self::RouteBinding { tx_index, .. } => *tx_index,
        }
    }

    pub(crate) fn instruction_index(&self) -> usize {
        match self {
            Self::InvalidPayload {
                instruction_index, ..
            }
            | Self::InvalidContext {
                instruction_index, ..
            }
            | Self::NonSoraSource {
                instruction_index, ..
            }
            | Self::TargetProfileMismatch {
                instruction_index, ..
            }
            | Self::HashRoleCollision {
                instruction_index, ..
            }
            | Self::RouteBinding {
                instruction_index, ..
            } => *instruction_index,
        }
    }

    pub(crate) fn reason(&self) -> &'static str {
        match self {
            Self::InvalidPayload { .. } => "RecordSccpMessage payload is invalid",
            Self::InvalidContext { .. } => "RecordSccpMessage context is invalid",
            Self::NonSoraSource { .. } => "RecordSccpMessage payload source domain is not SORA",
            Self::TargetProfileMismatch { .. } => {
                "RecordSccpMessage payload destination domain does not match its exact target profile"
            }
            Self::HashRoleCollision { .. } => {
                "RecordSccpMessage destination binding, message id, and payload hash must be distinct"
            }
            Self::RouteBinding { error, .. } => error.reason(),
        }
    }
}

fn invalid_sccp_record_instruction_in_executable(
    tx_index: usize,
    executable: &Executable,
) -> Option<SccpRecordInstructionValidationError> {
    let instructions = match executable {
        Executable::Instructions(instructions) => instructions.as_ref(),
        Executable::IvmProved(proved) => proved.overlay.as_ref(),
        Executable::ContractCall(_) | Executable::Ivm(_) => return None,
    };
    instructions
        .iter()
        .enumerate()
        .find_map(|(instruction_index, instruction)| {
            let record = recorded_sccp_message_instruction(instruction)?;
            match validate_recorded_sccp_message_payload_bytes(
                record.context,
                &record.payload_bytes,
            ) {
                Ok(_) => None,
                Err(RecordedSccpMessageValidationError::InvalidPayload) => {
                    Some(SccpRecordInstructionValidationError::InvalidPayload {
                        tx_index,
                        instruction_index,
                    })
                }
                Err(RecordedSccpMessageValidationError::InvalidContext) => {
                    Some(SccpRecordInstructionValidationError::InvalidContext {
                        tx_index,
                        instruction_index,
                    })
                }
                Err(RecordedSccpMessageValidationError::NonSoraSource { source_domain }) => {
                    Some(SccpRecordInstructionValidationError::NonSoraSource {
                        tx_index,
                        instruction_index,
                        source_domain,
                    })
                }
                Err(RecordedSccpMessageValidationError::TargetProfileMismatch {
                    target,
                    payload_target_domain,
                }) => Some(
                    SccpRecordInstructionValidationError::TargetProfileMismatch {
                        tx_index,
                        instruction_index,
                        target,
                        payload_target_domain,
                    },
                ),
                Err(RecordedSccpMessageValidationError::HashRoleCollision) => {
                    Some(SccpRecordInstructionValidationError::HashRoleCollision {
                        tx_index,
                        instruction_index,
                    })
                }
                Err(RecordedSccpMessageValidationError::RouteBinding { error }) => {
                    Some(SccpRecordInstructionValidationError::RouteBinding {
                        tx_index,
                        instruction_index,
                        error,
                    })
                }
            }
        })
}

fn invalid_sccp_record_instruction_in_signed_block(
    block: &SignedBlock,
) -> Option<SccpRecordInstructionValidationError> {
    for (entrypoint_index, entrypoint) in block.external_entrypoints_cloned().enumerate() {
        let transaction = match entrypoint {
            TransactionEntrypoint::External(transaction) => transaction,
            TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction().clone(),
            TransactionEntrypoint::SealedCommitment(_)
            | TransactionEntrypoint::PrivateKaigi(_)
            | TransactionEntrypoint::Time(_) => continue,
        };
        if !entrypoint_has_successful_or_pending_result(block, entrypoint_index) {
            continue;
        }
        if let Some(error) = invalid_sccp_record_instruction_in_executable(
            entrypoint_index,
            transaction.instructions(),
        ) {
            return Some(error);
        }
    }
    None
}

/// Extract all non-replayed SCCP message records from the external transactions in a signed block.
pub fn collect_sccp_messages_from_signed_block(block: &SignedBlock) -> Vec<RecordedSccpMessage> {
    collect_sccp_messages_from_signed_block_with_deduplication(block, true)
}

/// Return the first duplicate successful SCCP outbound key in a signed block, if any.
pub(crate) fn duplicate_sccp_outbound_message_key_in_signed_block(
    block: &SignedBlock,
) -> Option<SccpOutboundMessageKeyV1> {
    let mut seen = BTreeSet::new();
    for message in collect_sccp_messages_from_signed_block_with_deduplication(block, false) {
        let key =
            SccpOutboundMessageKeyV1::new(message.context.lane, message.commitment.message_id)?;
        if !seen.insert(key) {
            return Some(key);
        }
    }
    None
}

/// Validation error for committed SCCP records reconstructed from a signed block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SccpCommittedBlockValidationError {
    /// The block advertises an SCCP root but has no committed transaction results.
    MissingTransactionResults {
        /// Root advertised in the block header.
        actual: [u8; 32],
    },
    /// The block has fewer committed transaction results than external entrypoints.
    TransactionResultCountMismatch {
        /// Number of external entrypoints in the block payload.
        external_entrypoints: usize,
        /// Number of committed transaction results attached to the block.
        results: usize,
    },
    /// A successful or pending entrypoint contains a malformed SCCP record instruction.
    InvalidRecordInstruction(SccpRecordInstructionValidationError),
    /// The block contains more than one successful outbound message with the same replay key.
    DuplicateOutboundMessage(SccpOutboundMessageKeyV1),
    /// The block contains more successful outbound messages than the fixed first-release bound.
    TooManyOutboundMessages {
        /// Number of successful outbound messages reconstructed from the block.
        actual: usize,
        /// Maximum successful outbound messages admitted per block.
        max: usize,
    },
    /// The block header commitment root does not match reconstructed SCCP records.
    CommitmentRootMismatch {
        /// Root recomputed from committed SCCP message records.
        expected: Option<[u8; 32]>,
        /// Root advertised in the block header.
        actual: Option<[u8; 32]>,
    },
}

fn sccp_transaction_result_count_mismatch(block: &SignedBlock) -> Option<(usize, usize)> {
    if !block.has_results() {
        return None;
    }
    let external_entrypoints = block.external_entrypoint_count();
    let results = block.results().len();
    (results < external_entrypoints).then_some((external_entrypoints, results))
}

/// Validate committed SCCP records against the signed block header.
///
/// This check is intentionally fail-closed for duplicate successful outbound
/// keys before comparing roots, so a malformed block cannot be accepted by
/// signing a root over a deduplicated message list.
pub(crate) fn validate_sccp_commitment_root_for_signed_block(
    block: &SignedBlock,
) -> Result<(), SccpCommittedBlockValidationError> {
    if let Some(actual) = block.header().sccp_commitment_root()
        && !block.has_results()
    {
        return Err(SccpCommittedBlockValidationError::MissingTransactionResults { actual });
    }

    if let Some((external_entrypoints, results)) = sccp_transaction_result_count_mismatch(block) {
        return Err(
            SccpCommittedBlockValidationError::TransactionResultCountMismatch {
                external_entrypoints,
                results,
            },
        );
    }

    if let Some(error) = invalid_sccp_record_instruction_in_signed_block(block) {
        return Err(SccpCommittedBlockValidationError::InvalidRecordInstruction(
            error,
        ));
    }

    if let Some(key) = duplicate_sccp_outbound_message_key_in_signed_block(block) {
        return Err(SccpCommittedBlockValidationError::DuplicateOutboundMessage(
            key,
        ));
    }

    let messages = collect_sccp_messages_from_signed_block(block);
    let max = usize::try_from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1)
        .expect("SCCP block bound fits usize");
    if messages.len() > max {
        return Err(SccpCommittedBlockValidationError::TooManyOutboundMessages {
            actual: messages.len(),
            max,
        });
    }
    let expected = sccp_commitment_root_from_messages(&messages);
    let actual = block.header().sccp_commitment_root();
    if actual == expected {
        Ok(())
    } else {
        Err(SccpCommittedBlockValidationError::CommitmentRootMismatch { expected, actual })
    }
}

/// Compute the SCCP commitment Merkle root for a set of recorded messages.
pub fn sccp_commitment_root_from_messages(messages: &[RecordedSccpMessage]) -> Option<[u8; 32]> {
    let commitments: Vec<_> = messages
        .iter()
        .map(|message| message.commitment.clone())
        .collect();
    iroha_sccp::commitment_merkle_root(&commitments)
}

/// Errors returned when constructing a bridge finality proof.
#[allow(variant_size_differences)]
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BridgeFinalityError {
    /// The requested block height is zero.
    #[error("invalid block height {0}")]
    InvalidHeight(u64),
    /// No durable Sumeragi-v2 finality artifact exists for the requested height.
    #[error("Sumeragi-v2 finality artifact for height {0} not found")]
    FinalityArtifactNotFound(u64),
    /// Kura could not decode or validate the durable artifact.
    #[error("failed to load Sumeragi-v2 finality artifact for height {height}: {reason}")]
    FinalityArtifactRead {
        /// Height being proven.
        height: u64,
        /// Bounded Kura validation diagnostic.
        reason: String,
    },
    /// The durable artifact does not match the selected block header or chain.
    #[error("Sumeragi-v2 finality artifact for height {height} does not match the selected block")]
    FinalityArtifactMismatch {
        /// Height being proven.
        height: u64,
    },
}

/// Build a self-contained finality proof for the block at `height`.
///
/// The proof bundles the block header and Kura's exact immutable v2 finality
/// artifact. The artifact owns BLS PoPs aligned with its frozen powered roster,
/// so historical verification never consults mutable validator state.
///
/// # Errors
///
/// Returns [`BridgeFinalityError`] when the height is zero, the durable retained-header artifact
/// is missing/malformed, or the exact v2 artifact fails
/// cryptographic verification.
pub fn build_finality_proof(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<BridgeFinalityProof, BridgeFinalityError> {
    if height == 0 {
        return Err(BridgeFinalityError::InvalidHeight(height));
    }

    let verified_finality = state
        .bridge_verified_v2_finality_artifact(height)
        .map_err(|reason| BridgeFinalityError::FinalityArtifactRead { height, reason })?
        .ok_or(BridgeFinalityError::FinalityArtifactNotFound(height))?;
    build_finality_proof_from_verified(state.bridge_chain_id(), height, &verified_finality)
}

fn build_finality_proof_from_verified(
    chain_id: &ChainId,
    height: u64,
    verified_finality: &VerifiedV2FinalityArtifact,
) -> Result<BridgeFinalityProof, BridgeFinalityError> {
    let block_header = verified_finality.retained_header().clone();
    let finality_artifact = verified_finality.artifact().clone();
    if finality_artifact.height != height
        || block_header.height().get() != height
        || finality_artifact.height_context.chain_id != *chain_id
        || finality_artifact
            .validate_for_header(&block_header)
            .is_err()
    {
        return Err(BridgeFinalityError::FinalityArtifactMismatch { height });
    }

    Ok(BridgeFinalityProof {
        version: BRIDGE_FINALITY_PROOF_VERSION_V1,
        block_header,
        finality_artifact,
    })
}

/// Build an SCCP Groth16 request from a bundle bound to one already verified local artifact.
///
/// The marker is the trust boundary: Kura mints it after cache-backed verification, while
/// untrusted [`BridgeStateReadOnly`] providers must mint it with
/// [`VerifiedV2FinalityArtifact::verify_for_header`]. This function requires the bundle's
/// canonical finality proof to equal the marker's exact retained header and artifact before
/// delegating to SCCP's structural request assembler, so it never repeats BLS verification.
#[must_use]
pub fn build_sccp_groth16_bn254_proof_request_from_verified_finality_v1(
    verified_finality: &VerifiedV2FinalityArtifact,
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> Option<SccpGroth16Bn254ProofRequestV1> {
    let finality = TairaBridgeFinalityProofV1 {
        version: BRIDGE_FINALITY_PROOF_VERSION_V1,
        block_header: verified_finality.retained_header().clone(),
        finality_artifact: verified_finality.artifact().clone(),
    };
    iroha_sccp::build_sccp_groth16_bn254_proof_request_from_structurally_bound_finality_v1(
        bundle,
        governed_route,
        &finality,
    )
}

/// Fully authenticated finalized SCCP outbox projection for one exact block height.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSccpFinalizedMessagesV1 {
    verified_finality: VerifiedV2FinalityArtifact,
    /// Exact retained-header finality proof used to authenticate the projection.
    pub finality_proof: BridgeFinalityProof,
    /// Merkle root committed by the retained block header.
    pub commitment_root: [u8; 32],
    /// Canonical messages in zero-based commitment-index order.
    pub messages: Vec<ValidatedSccpOutboundMessageProjectionV1>,
}

impl ValidatedSccpFinalizedMessagesV1 {
    /// Borrow the exact cache-backed finality marker used to authenticate this projection.
    #[must_use]
    pub const fn verified_finality(&self) -> &VerifiedV2FinalityArtifact {
        &self.verified_finality
    }
}

/// Reconstruct and authenticate all finalized SCCP messages at one exact height.
///
/// The scan is bounded by [`iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1`]
/// and uses Kura's immutable, root-authenticated payload archive plus its exact retained-header
/// finality record. Historical block bodies and mutable WSV payloads are deliberately outside this
/// proof-serving boundary.
///
/// # Errors
///
/// Returns a bounded diagnostic when finality or the immutable archive is absent or malformed,
/// the projection is not dense and canonical, or its reconstructed root differs from the
/// finalized header.
pub fn validated_sccp_finalized_messages_at_height(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<Option<ValidatedSccpFinalizedMessagesV1>, String> {
    if height == 0 {
        return Err(BridgeFinalityError::InvalidHeight(height).to_string());
    }
    let (verified_finality, messages) = state
        .bridge_verified_v2_finality_with_sccp_archive(height)?
        .ok_or_else(|| BridgeFinalityError::FinalityArtifactNotFound(height).to_string())?;
    let finality_proof =
        build_finality_proof_from_verified(state.bridge_chain_id(), height, &verified_finality)
            .map_err(|error| error.to_string())?;
    let Some((commitment_root, messages)) = validate_sccp_outbound_projection_against_root(
        height,
        finality_proof.block_header.sccp_commitment_root(),
        messages,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(ValidatedSccpFinalizedMessagesV1 {
        verified_finality,
        finality_proof,
        commitment_root,
        messages,
    }))
}

fn validate_sccp_outbound_projection_against_root(
    height: u64,
    anchored_root: Option<[u8; 32]>,
    messages: Vec<ValidatedSccpOutboundMessageProjectionV1>,
) -> Result<Option<([u8; 32], Vec<ValidatedSccpOutboundMessageProjectionV1>)>, String> {
    let max = usize::try_from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1)
        .expect("SCCP block bound fits usize");
    if messages.len() > max {
        return Err(format!(
            "SCCP outbox projection at height {height} contains {} messages, exceeding the fixed {max}-message bound",
            messages.len()
        ));
    }
    let mut message_ids = BTreeSet::new();
    for (index, message) in messages.iter().enumerate() {
        let expected_index = u32::try_from(index).expect("bounded SCCP index fits u32");
        if message.commitment_index != expected_index {
            return Err(format!(
                "SCCP outbox projection at height {height} is not in dense commitment order: expected {expected_index}, found {}",
                message.commitment_index
            ));
        }
        if !message.context.is_well_formed()
            || iroha_sccp::hub_commitment_from_sccp_payload(message.context, &message.payload)
                .as_ref()
                != Some(&message.commitment)
        {
            return Err(format!(
                "SCCP outbox projection at height {height} contains a substituted context, payload, or commitment at index {expected_index}"
            ));
        }
        if !message_ids.insert(message.commitment.message_id) {
            return Err(format!(
                "SCCP outbox projection at height {height} repeats message identifier {}",
                hex::encode(message.commitment.message_id)
            ));
        }
    }
    if messages.is_empty() {
        return if anchored_root.is_none() {
            Ok(None)
        } else {
            Err(format!(
                "finalized SCCP header at height {height} commits a root but the immutable outbox archive is empty"
            ))
        };
    }
    let commitment_root = anchored_root.ok_or_else(|| {
        format!(
            "immutable SCCP outbox archive exists at height {height} but the retained finalized header has no commitment root"
        )
    })?;
    let commitments = messages
        .iter()
        .map(|message| message.commitment.clone())
        .collect::<Vec<_>>();
    let reconstructed = iroha_sccp::commitment_merkle_root(&commitments).ok_or_else(|| {
        format!("failed to reconstruct the bounded SCCP commitment root at height {height}")
    })?;
    if reconstructed != commitment_root {
        return Err(format!(
            "immutable SCCP outbox archive at height {height} reconstructs root 0x{}, expected finalized root 0x{}",
            hex::encode(reconstructed),
            hex::encode(commitment_root)
        ));
    }
    Ok(Some((commitment_root, messages)))
}

/// Build a compact commitment plus exact typed finality proof for `height`.
///
/// # Errors
///
/// Returns [`BridgeFinalityError`] when the underlying finality proof cannot be
/// built for the requested height.
pub fn build_finality_bundle(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<BridgeFinalityBundle, BridgeFinalityError> {
    let proof = build_finality_proof(state, height)?;
    let commitment = BridgeCommitment {
        chain_id: proof.finality_artifact.height_context.chain_id.clone(),
        height_context_id: proof.finality_artifact.context_id(),
        block_height: proof.finality_artifact.height,
        block_hash: proof.finality_artifact.block_hash,
    };
    Ok(BridgeFinalityBundle {
        commitment,
        finality_proof: proof,
    })
}

/// Verification errors raised when checking a BridgeFinalityProof.
#[allow(variant_size_differences)]
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BridgeFinalityVerificationError {
    /// The caller expected a different finalized height.
    #[error("finality proof height mismatch: expected {expected}, actual {actual}")]
    HeightMismatch {
        /// Height requested by the caller.
        expected: u64,
        /// Height carried by the exact v2 artifact.
        actual: u64,
    },
    /// Exact proof verification failed.
    #[error(transparent)]
    Verification(#[from] iroha_data_model::bridge::BridgeFinalityVerifyError),
}

/// Verification knobs for verify_finality_proof.
#[derive(Debug, Clone)]
pub struct FinalityProofVerificationConfig<'a> {
    /// Chain identifier expected by the verifier.
    pub expected_chain_id: &'a ChainId,
    /// Optional expected height to bind the proof to a specific block.
    pub expected_height: Option<u64>,
    /// Trusted context id for the exact height being verified.
    pub trusted_context_id: iroha_data_model::block::consensus_v2::HeightContextId,
}

/// Verify a BridgeFinalityProof against chain, height, context, powered quorum,
/// PoP, and aggregate-signature expectations.
///
/// # Errors
///
/// Returns BridgeFinalityVerificationError when the expected height differs or
/// the exact typed Sumeragi-v2 proof fails verification.
pub fn verify_finality_proof(
    proof: &BridgeFinalityProof,
    config: &FinalityProofVerificationConfig<'_>,
) -> Result<(), BridgeFinalityVerificationError> {
    if let Some(expected_height) = config.expected_height {
        let actual = proof.finality_artifact.height;
        if actual != expected_height {
            return Err(BridgeFinalityVerificationError::HeightMismatch {
                expected: expected_height,
                actual,
            });
        }
    }

    let mut verifier = iroha_data_model::bridge::BridgeFinalityVerifier::with_context(
        config.expected_chain_id.clone(),
        config.trusted_context_id,
    );
    verifier.verify(proof)?;
    Ok(())
}

#[cfg(test)]
fn validate_local_sccp_records_against_commitment_root(
    local_block: &SignedBlock,
    commitment_root: [u8; 32],
) -> Result<(), String> {
    if !local_block.has_results() {
        return Err(
            "SCCP finality proof local block is missing committed transaction results".to_owned(),
        );
    }
    if let Some((external_entrypoints, results)) =
        sccp_transaction_result_count_mismatch(local_block)
    {
        return Err(format!(
            "SCCP finality proof local block result count mismatch: external_entrypoints={external_entrypoints} results={results}"
        ));
    }

    if let Some(error) = invalid_sccp_record_instruction_in_signed_block(local_block) {
        return Err(format!(
            "SCCP finality proof local block contains invalid outbound SCCP record: tx_index={} instruction_index={} reason={}",
            error.tx_index(),
            error.instruction_index(),
            error.reason()
        ));
    }

    if let Some(key) = duplicate_sccp_outbound_message_key_in_signed_block(local_block) {
        return Err(format!(
            "SCCP finality proof local block contains duplicate outbound message source_profile={} target_profile={} message_id={}",
            key.lane.source.profile_key(),
            key.lane.target.profile_key(),
            hex::encode(key.message_id)
        ));
    }

    let messages = collect_sccp_messages_from_signed_block(local_block);
    if sccp_commitment_root_from_messages(&messages) != Some(commitment_root) {
        return Err(
            "SCCP finality proof commitment root does not match local SCCP records".to_owned(),
        );
    }

    Ok(())
}

/// Verify an SCCP finality proof against local committed block and v2 artifact data.
///
/// This intentionally rejects proofs when the local node cannot load the committed block or
/// exact durable Sumeragi-v2 artifact for the referenced height.
///
/// # Errors
/// Returns a human-readable rejection reason when the SCCP proof is not anchored to local state
/// or when the trusted local artifact fails full finality verification.
pub fn verify_sccp_finality_proof_against_local_state(
    state: &impl BridgeStateReadOnly,
    finality: &TairaBridgeFinalityProofV1,
) -> Result<BridgeFinalityProof, String> {
    if !iroha_sccp::verify_taira_bridge_finality_proof_structure(finality) {
        return Err("SCCP finality proof failed structural verification".to_owned());
    }
    verify_structural_sccp_finality_proof_against_local_state(state, finality)
}

/// Bind an opaque route/Groth16-verified destination context to local committed
/// block and durable v2 artifact state without repeating proof-controlled parsing.
///
/// # Errors
/// Returns a human-readable rejection reason when the context's finality
/// artifact differs from authoritative local state or the proof-typed storage
/// lookup rejects that local artifact.
pub fn verify_sccp_destination_context_against_local_state(
    state: &impl BridgeStateReadOnly,
    context: &iroha_sccp::SccpVerifiedDestinationContextV1,
) -> Result<BridgeFinalityProof, String> {
    verify_structural_sccp_finality_proof_against_local_state(state, context.finality())
}

fn verify_structural_sccp_finality_proof_against_local_state(
    state: &impl BridgeStateReadOnly,
    finality: &TairaBridgeFinalityProofV1,
) -> Result<BridgeFinalityProof, String> {
    let artifact = &finality.finality_artifact;
    let height = artifact.height;
    if artifact.height_context.chain_id != *state.bridge_chain_id() {
        return Err("SCCP finality proof chain id does not match local state".to_owned());
    }

    let local = validated_sccp_finalized_messages_at_height(state, height)?
        .ok_or_else(|| format!("local finalized block at height {height} has no SCCP messages"))?;
    if local.finality_proof.block_header != finality.block_header {
        return Err(
            "SCCP finality proof block header does not match the retained local canonical header"
                .to_owned(),
        );
    }
    if local.finality_proof.finality_artifact != *artifact {
        return Err(
            "SCCP finality proof artifact does not match the exact durable local artifact"
                .to_owned(),
        );
    }

    Ok(finality.clone())
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, num::NonZeroU64};

    use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        block::{BlockSignature, SignedBlock},
        isi::InstructionBox,
        nexus::DataSpaceId,
        prelude::TransactionBuilder,
        smart_contract::{CHAIN_DISCRIMINANT_MAINNET, ContractAddress},
        transaction::{
            DataTriggerSequence, Executable, ExecutionStep, IvmBytecode, IvmProved,
            SignedTransaction, TransactionEntrypoint, TransactionResultInner,
            executable::ContractInvocation,
        },
        trigger::{TimeTriggerEntrypoint, TriggerId},
    };

    use super::*;
    use crate::tx::AcceptedTransaction;

    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("bridge fixture key generation should succeed")
    }

    fn checked_bls_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("bridge BLS fixture key generation should succeed")
    }

    fn canonical_test_sccp_payload_bytes(payload: &SccpPayloadV1) -> Vec<u8> {
        iroha_sccp::canonical_sccp_payload_bytes(payload)
            .expect("valid SCCP bridge fixture payload encodes")
    }

    fn canonical_test_transfer_payload_bytes(payload: &iroha_sccp::TransferPayloadV1) -> Vec<u8> {
        iroha_sccp::canonical_transfer_payload_bytes(payload)
            .expect("valid SCCP transfer fixture payload encodes")
    }

    #[test]
    fn checked_keypair_helpers_preserve_requested_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
        assert_eq!(checked_bls_keypair().algorithm(), Algorithm::BlsNormal);
    }

    #[derive(Clone)]
    struct TestSccpFinalityState {
        chain_id: ChainId,
        retained_header: Option<BlockHeader>,
        messages: Vec<ValidatedSccpOutboundMessageProjectionV1>,
        artifact: Option<V2FinalityArtifact>,
        artifact_error: Option<String>,
    }

    impl BridgeStateReadOnly for TestSccpFinalityState {
        fn bridge_chain_id(&self) -> &ChainId {
            &self.chain_id
        }

        fn bridge_verified_v2_finality_artifact(
            &self,
            height: u64,
        ) -> Result<Option<VerifiedV2FinalityArtifact>, String> {
            if let Some(error) = &self.artifact_error {
                return Err(error.clone());
            }
            self.artifact
                .as_ref()
                .filter(|artifact| artifact.height == height)
                .cloned()
                .zip(self.retained_header.clone())
                .map(|(artifact, header)| {
                    VerifiedV2FinalityArtifact::verify_for_header(header, artifact)
                })
                .transpose()
                .map_err(|error| error.to_string())
        }

        fn bridge_verified_v2_finality_with_sccp_archive(
            &self,
            height: u64,
        ) -> Result<
            Option<(
                VerifiedV2FinalityArtifact,
                Vec<ValidatedSccpOutboundMessageProjectionV1>,
            )>,
            String,
        > {
            Ok(self
                .bridge_verified_v2_finality_artifact(height)?
                .map(|verified| (verified, self.messages.clone())))
        }
    }

    fn test_sccp_projections_from_block(
        block: &SignedBlock,
    ) -> Vec<ValidatedSccpOutboundMessageProjectionV1> {
        collect_sccp_messages_from_signed_block(block)
            .into_iter()
            .enumerate()
            .map(
                |(index, message)| ValidatedSccpOutboundMessageProjectionV1 {
                    commitment_index: u32::try_from(index).expect("test SCCP index fits u32"),
                    context: message.context,
                    payload: message.payload,
                    commitment: message.commitment,
                },
            )
            .collect()
    }

    fn sample_sccp_projection_set(count: u64) -> Vec<ValidatedSccpOutboundMessageProjectionV1> {
        let payloads = (0..count)
            .map(|nonce| {
                canonical_test_sccp_payload_bytes(&sample_transfer_payload(nonce + 1, [0x22; 20]))
            })
            .collect::<Vec<_>>();
        let (block, _) = signed_block_with_sccp_payloads(&payloads, 1);
        test_sccp_projections_from_block(&block)
    }

    fn projection_root(messages: &[ValidatedSccpOutboundMessageProjectionV1]) -> [u8; 32] {
        iroha_sccp::commitment_merkle_root(
            &messages
                .iter()
                .map(|message| message.commitment.clone())
                .collect::<Vec<_>>(),
        )
        .expect("nonempty test projection has a Merkle root")
    }

    #[test]
    fn finalized_projection_accepts_only_canonical_commitment_order() {
        let messages = sample_sccp_projection_set(3);
        let root = projection_root(&messages);

        let (validated_root, validated) =
            validate_sccp_outbound_projection_against_root(1, Some(root), messages.clone())
                .expect("canonical projection validates")
                .expect("nonempty projection is returned");

        assert_eq!(validated_root, root);
        assert_eq!(validated, messages);
    }

    #[test]
    fn finalized_projection_rejects_reorder_gap_and_coordinated_index_swap() {
        let messages = sample_sccp_projection_set(3);
        let root = projection_root(&messages);

        let mut reordered = messages.clone();
        reordered.reverse();
        let error = validate_sccp_outbound_projection_against_root(1, Some(root), reordered)
            .expect_err("retained indices must reject reordered storage output");
        assert!(error.contains("dense commitment order"), "{error}");

        let mut gap = messages.clone();
        gap[1].commitment_index = 2;
        let error = validate_sccp_outbound_projection_against_root(1, Some(root), gap)
            .expect_err("a commitment-index gap must fail closed");
        assert!(error.contains("expected 1, found 2"), "{error}");

        let mut coordinated_swap = messages;
        coordinated_swap.swap(0, 2);
        for (index, message) in coordinated_swap.iter_mut().enumerate() {
            message.commitment_index = u32::try_from(index).expect("small test index");
        }
        let error = validate_sccp_outbound_projection_against_root(1, Some(root), coordinated_swap)
            .expect_err("rewriting indices cannot rewrite the finalized Merkle order");
        assert!(error.contains("reconstructs root"), "{error}");
    }

    #[test]
    fn finalized_projection_rejects_duplicate_substituted_omitted_and_extra_messages() {
        let messages = sample_sccp_projection_set(3);
        let root = projection_root(&messages);

        let mut duplicate = messages.clone();
        duplicate[1] = duplicate[0].clone();
        duplicate[1].commitment_index = 1;
        let duplicate_root = projection_root(&duplicate);
        let error =
            validate_sccp_outbound_projection_against_root(1, Some(duplicate_root), duplicate)
                .expect_err("duplicate message identifiers must fail before root acceptance");
        assert!(error.contains("repeats message identifier"), "{error}");

        let mut substituted = messages.clone();
        substituted[1].commitment.message_id[0] ^= 1;
        let error = validate_sccp_outbound_projection_against_root(1, Some(root), substituted)
            .expect_err("payload-independent commitment substitution must fail closed");
        assert!(
            error.contains("substituted context, payload, or commitment"),
            "{error}"
        );

        let mut omitted = messages.clone();
        omitted.pop();
        let error = validate_sccp_outbound_projection_against_root(1, Some(root), omitted)
            .expect_err("omitting a finalized message must change the root");
        assert!(error.contains("reconstructs root"), "{error}");

        let extra = sample_sccp_projection_set(4);
        let error = validate_sccp_outbound_projection_against_root(1, Some(root), extra)
            .expect_err("appending a message must change the finalized root");
        assert!(error.contains("reconstructs root"), "{error}");
    }

    #[test]
    fn finalized_projection_enforces_empty_root_equivalence_and_fixed_bound() {
        assert_eq!(
            validate_sccp_outbound_projection_against_root(8, None, Vec::new())
                .expect("empty rootless block validates"),
            None
        );

        let error = validate_sccp_outbound_projection_against_root(8, Some([0xAA; 32]), Vec::new())
            .expect_err("a rooted finalized header cannot have an empty projection");
        assert!(error.contains("commits a root"), "{error}");

        let one = sample_sccp_projection_set(1);
        let error = validate_sccp_outbound_projection_against_root(8, None, one.clone())
            .expect_err("a rootless finalized header cannot have an outbox record");
        assert!(error.contains("has no commitment root"), "{error}");

        let over_limit = vec![
            one[0].clone();
            usize::try_from(
                iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 + 1,
            )
            .expect("protocol bound fits usize")
        ];
        let error = validate_sccp_outbound_projection_against_root(8, Some([0xBB; 32]), over_limit)
            .expect_err("the validator must reject before processing an oversized vector");
        assert!(
            error.contains("exceeding the fixed 512-message bound"),
            "{error}"
        );
    }

    fn sample_transfer_payload(nonce: u64, recipient: [u8; 20]) -> SccpPayloadV1 {
        SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
            version: 1,
            source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            nonce,
            route_revision: 1,
            asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            asset_id: b"xor".to_vec(),
            amount: 77,
            sender_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
            recipient: recipient.to_vec(),
            route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            route_id: iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1
                .as_bytes()
                .to_vec(),
        })
    }

    fn non_sora_source_transfer_payload(nonce: u64) -> SccpPayloadV1 {
        SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
            version: 1,
            source_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            dest_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            nonce,
            route_revision: 1,
            asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            asset_id: b"xor".to_vec(),
            amount: 77,
            sender_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
            sender: [0x22; 20].to_vec(),
            recipient_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            recipient: b"sora:recipient".to_vec(),
            route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            route_id: iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1
                .as_bytes()
                .to_vec(),
        })
    }

    #[test]
    fn durable_outbound_record_retains_and_revalidates_exact_canonical_payload() {
        let payload = sample_transfer_payload(41, [0x31; 20]);
        let payload_bytes = canonical_test_sccp_payload_bytes(&payload);
        let context = test_sccp_outbound_context_for_payload_bytes(&payload_bytes);
        let validated = validate_recorded_sccp_message_payload_bytes(context, &payload_bytes)
            .expect("exact outbound payload validates");
        let record = validated
            .outbound_record(9, 3)
            .expect("validated payload forms a durable record");

        assert_eq!(record.payload_bytes, payload_bytes);
        assert_eq!(
            record.payload_hash,
            iroha_sccp::payload_hash(&payload_bytes)
        );
        let projection = validate_sccp_outbound_message_record_v1(&validated.key, &record)
            .expect("durable record fully revalidates");
        assert_eq!(projection.context, validated.context);
        assert_eq!(projection.commitment_index, 3);
        assert_eq!(projection.payload, validated.payload);
        assert_eq!(projection.commitment, validated.commitment);
        assert!(validated.outbound_record(0, 0).is_none());
        assert!(
            validated
                .outbound_record(
                    9,
                    iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1,
                )
                .is_none()
        );
    }

    #[test]
    fn durable_outbound_record_rejects_payload_malleability_amplification_and_identity_drift() {
        let payload = sample_transfer_payload(42, [0x32; 20]);
        let payload_bytes = canonical_test_sccp_payload_bytes(&payload);
        let context = test_sccp_outbound_context_for_payload_bytes(&payload_bytes);
        let validated = validate_recorded_sccp_message_payload_bytes(context, &payload_bytes)
            .expect("exact outbound payload validates");
        let record = validated
            .outbound_record(10, 0)
            .expect("validated payload forms a durable record");

        let mut malformed = record.clone();
        malformed.payload_bytes[0] ^= 0x7f;
        let mut trailing_alias = record.clone();
        trailing_alias.payload_bytes.push(0);
        let mut oversized = record.clone();
        oversized.payload_bytes =
            vec![0xA5; iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1 + 1];
        let mut wrong_hash = record.clone();
        wrong_hash.payload_hash = [0xA6; 32];
        let wrong_key = SccpOutboundMessageKeyV1 {
            message_id: [0xA7; 32],
            ..validated.key
        };
        let wrong_lane_key = SccpOutboundMessageKeyV1 {
            lane: iroha_data_model::bridge::SccpLaneIdV1 {
                source: iroha_data_model::bridge::SccpNetworkV1::SoraTaira,
                target: iroha_data_model::bridge::SccpNetworkV1::EthereumSepolia,
            },
            ..validated.key
        };
        let mut aliased_asset_payload = payload;
        let SccpPayloadV1::Transfer(transfer) = &mut aliased_asset_payload;
        transfer.asset_id = b"xor#scope".to_vec();
        let aliased_asset_bytes = canonical_test_sccp_payload_bytes(&aliased_asset_payload);
        let aliased_asset_key = SccpOutboundMessageKeyV1::new(
            context.lane,
            iroha_sccp::sccp_message_id(context.lane, &aliased_asset_payload)
                .expect("scoped-asset payload remains structurally lane-bound"),
        )
        .expect("scoped-asset payload forms a structural key");
        let aliased_asset_record = iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1 {
            destination_binding_hash: context.destination_binding_hash,
            route_configuration_hash: context.route_configuration_hash,
            payload_hash: iroha_sccp::payload_hash(&aliased_asset_bytes),
            payload_bytes: aliased_asset_bytes,
            recorded_at_height: 10,
            commitment_index: 0,
        };
        assert!(aliased_asset_record.is_well_formed_for_key(&aliased_asset_key));

        for (key, hostile) in [
            (validated.key, malformed),
            (validated.key, trailing_alias),
            (validated.key, oversized),
            (validated.key, wrong_hash),
            (wrong_key, record.clone()),
            (wrong_lane_key, record),
            (aliased_asset_key, aliased_asset_record),
        ] {
            assert!(
                validate_sccp_outbound_message_record_v1(&key, &hostile).is_none(),
                "hostile durable evidence unexpectedly validated: {hostile:?}"
            );
        }
    }

    #[test]
    fn outbound_commitment_index_allocation_is_dense_bounded_and_rollback_safe() {
        use mv::storage::{Storage, StorageReadOnly};

        let index_key = |index: u32, id: u32| {
            let mut message_id = [0_u8; 32];
            message_id[..4].copy_from_slice(&id.to_le_bytes());
            iroha_data_model::bridge::SccpOutboundMessageIndexKeyV1 {
                recorded_at_height: 9,
                commitment_index: index,
                lane: iroha_data_model::bridge::SccpLaneIdV1 {
                    source: iroha_data_model::bridge::SccpNetworkV1::SoraTaira,
                    target: iroha_data_model::bridge::SccpNetworkV1::EthereumMainnet,
                },
                message_id,
            }
        };
        let storage = Storage::new();
        let mut block = storage.block();
        assert_eq!(
            next_sccp_outbound_commitment_index(&block, 9).expect("empty dense index"),
            Some(0)
        );
        {
            let mut transaction = block.transaction();
            transaction.insert(index_key(0, 1), ());
            assert_eq!(
                next_sccp_outbound_commitment_index(&transaction, 9)
                    .expect("transaction sees its staged index"),
                Some(1)
            );
        }
        assert!(
            block.is_empty(),
            "dropped transaction must revert its index"
        );
        assert_eq!(
            next_sccp_outbound_commitment_index(&block, 9).expect("reverted index is reusable"),
            Some(0)
        );

        for index in 0..iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 {
            block.insert(index_key(index, index + 1), ());
        }
        assert_eq!(
            next_sccp_outbound_commitment_index(&block, 9).expect("exactly full dense index"),
            None
        );
        block.insert(
            index_key(
                iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1,
                513,
            ),
            (),
        );
        assert!(next_sccp_outbound_commitment_index(&block, 9).is_err());

        let gap = Storage::new();
        let mut gap_block = gap.block();
        gap_block.insert(index_key(1, 1), ());
        assert!(next_sccp_outbound_commitment_index(&gap_block, 9).is_err());

        let duplicate = Storage::new();
        let mut duplicate_block = duplicate.block();
        duplicate_block.insert(index_key(0, 1), ());
        duplicate_block.insert(index_key(0, 2), ());
        assert!(next_sccp_outbound_commitment_index(&duplicate_block, 9).is_err());
    }

    fn signed_transaction_with_executable(executable: Executable) -> SignedTransaction {
        let keypair = checked_keypair();
        let chain: ChainId = "bridge-sccp-tests".parse().expect("chain id");
        let authority = AccountId::new(keypair.public_key().clone());
        TransactionBuilder::new(chain, authority)
            .with_executable(executable)
            .sign(keypair.private_key())
    }

    fn accepted_transaction_with_sccp_payload(payload: Vec<u8>) -> AcceptedTransaction<'static> {
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload)),
        ]));
        AcceptedTransaction::new_unchecked(Cow::Owned(tx))
    }

    fn sealed_commitment_entrypoint() -> TransactionEntrypoint {
        let keypair = checked_keypair();
        let chain_id: ChainId = "bridge-sccp-sealed-index".parse().expect("chain id");
        let authority = AccountId::new(keypair.public_key().clone());
        let inner_tx = TransactionBuilder::new(chain_id.clone(), authority.clone())
            .sign(keypair.private_key());
        let commitment =
            iroha_data_model::transaction::signed::compute_sealed_transaction_commitment(
                &chain_id, &inner_tx, [0x57; 32], 5,
            );
        let payload = iroha_data_model::transaction::signed::SealedTransactionCommitmentPayload {
            chain_id,
            authority,
            commitment,
            reveal_after_height: 2,
            reveal_deadline_height: 5,
            nonce: None,
        };
        TransactionEntrypoint::SealedCommitment(
            iroha_data_model::transaction::signed::SignedSealedTransactionCommitment::sign(
                payload,
                keypair.private_key(),
            ),
        )
    }

    fn sealed_sccp_record_entrypoints(payload: Vec<u8>) -> [TransactionEntrypoint; 2] {
        let keypair = checked_keypair();
        let chain_id: ChainId = "bridge-sccp-sealed-record".parse().expect("chain id");
        let authority = AccountId::new(keypair.public_key().clone());
        let signed = TransactionBuilder::new(chain_id.clone(), authority.clone())
            .with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
                crate::bridge::test_record_sccp_message(payload),
            )]))
            .sign(keypair.private_key());
        let salt = [0x58; 32];
        let reveal_deadline_height = 8;
        let commitment =
            iroha_data_model::transaction::signed::compute_sealed_transaction_commitment(
                &chain_id,
                &signed,
                salt,
                reveal_deadline_height,
            );
        let commitment_payload =
            iroha_data_model::transaction::signed::SealedTransactionCommitmentPayload {
                chain_id,
                authority,
                commitment,
                reveal_after_height: 4,
                reveal_deadline_height,
                nonce: None,
            };
        let signed_commitment =
            iroha_data_model::transaction::signed::SignedSealedTransactionCommitment::sign(
                commitment_payload,
                keypair.private_key(),
            );
        let reveal = iroha_data_model::transaction::signed::SealedTransactionReveal::new(
            commitment, signed, salt,
        );

        [
            TransactionEntrypoint::SealedCommitment(signed_commitment),
            TransactionEntrypoint::SealedReveal(reveal),
        ]
    }

    fn ivm_proved_with_overlay(instructions: Vec<InstructionBox>) -> Executable {
        Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03]),
            overlay: instructions.into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        })
    }

    fn replace_finalized_test_block_signature(block: &mut SignedBlock, signer: &KeyPair) {
        let signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(signer.private_key(), block.hash())
                .expect("sign finalized test block header"),
        );
        block
            .replace_signatures([signature].into_iter().collect())
            .expect("replace provisional test block signature");
        block
            .signatures()
            .next()
            .expect("finalized test block signature")
            .signature()
            .verify_hash(signer.public_key(), block.hash())
            .expect("finalized test block signature verifies");
    }

    fn signed_block_with_transactions(
        transactions: Vec<SignedTransaction>,
        height: u64,
    ) -> SignedBlock {
        let keypair = checked_keypair();
        let entry_hashes: Vec<_> = transactions
            .iter()
            .map(SignedTransaction::hash_as_entrypoint)
            .collect();
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(keypair.private_key(), header.hash())
                .expect("test block signing should succeed"),
        );
        let mut block = SignedBlock::presigned(signature, header, transactions);
        let results =
            std::iter::repeat_with(|| TransactionResultInner::Ok(DataTriggerSequence::default()))
                .take(entry_hashes.len())
                .collect();
        block
            .set_transaction_results(Vec::new(), &entry_hashes, results)
            .expect("test block entrypoint hashes should match payload");
        replace_finalized_test_block_signature(&mut block, &keypair);
        block
    }

    fn signed_block_without_results(
        transactions: Vec<SignedTransaction>,
        height: u64,
    ) -> SignedBlock {
        let keypair = checked_keypair();
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(keypair.private_key(), header.hash())
                .expect("test block signing should succeed"),
        );
        SignedBlock::presigned(signature, header, transactions)
    }

    fn signed_block_with_sccp_payloads(
        payloads: &[Vec<u8>],
        height: u64,
    ) -> (SignedBlock, Vec<SccpPayloadV1>) {
        let keypair = checked_keypair();
        let chain: ChainId = "bridge-sccp-tests".parse().expect("chain id");
        let authority = AccountId::new(keypair.public_key().clone());
        let decoded_payloads: Vec<_> = payloads
            .iter()
            .filter_map(|payload| iroha_sccp::decode_canonical_sccp_payload_bytes(payload))
            .collect();
        let instructions: Vec<InstructionBox> = payloads
            .iter()
            .cloned()
            .map(crate::bridge::test_record_sccp_message)
            .map(InstructionBox::from)
            .collect();
        let tx = TransactionBuilder::new(chain, authority)
            .with_executable(ivm_proved_with_overlay(instructions))
            .sign(keypair.private_key());
        let entry_hash = tx.hash_as_entrypoint();
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(keypair.private_key(), header.hash())
                .expect("test block signing should succeed"),
        );
        let mut block = SignedBlock::presigned(signature, header, vec![tx]);
        let entry_hashes = [entry_hash];
        block
            .set_transaction_results(
                Vec::new(),
                &entry_hashes,
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("test block entrypoint hashes should match payload");
        replace_finalized_test_block_signature(&mut block, &keypair);
        (block, decoded_payloads)
    }

    fn persisted_state_for_exact_sccp_fixture(
        fixture: &iroha_sccp::SccpExactOutboundTestFixtureV1,
    ) -> (
        iroha_sccp::SccpExactOutboundTestFixtureV1,
        TestSccpFinalityState,
    ) {
        let provisional_finality =
            iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
                .expect("exact provisional SCCP finality proof");
        let payload = canonical_test_sccp_payload_bytes(&fixture.bundle.payload);
        let instruction = crate::bridge::test_record_sccp_message(payload);
        assert_eq!(
            instruction.context, fixture.bundle.commitment.context,
            "exact local block instruction must preserve the bundle context"
        );
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(instruction),
        ]));
        let entry_hash = tx.hash_as_entrypoint();
        let block_signer = checked_keypair();
        let template_header = provisional_finality.block_header;
        let mut provisional_header = BlockHeader::new(
            template_header.height(),
            template_header.prev_block_hash(),
            None,
            None,
            u64::try_from(template_header.creation_time().as_millis())
                .expect("fixture creation time fits u64"),
            template_header.view_change_index(),
        );
        provisional_header.set_sccp_commitment_root(template_header.sccp_commitment_root());
        let signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(block_signer.private_key(), provisional_header.hash())
                .expect("fixture provisional local block signature"),
        );
        let mut block = SignedBlock::presigned(signature, provisional_header, vec![tx]);
        block
            .set_transaction_results(
                Vec::new(),
                &[entry_hash],
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("fixture local block results");
        assert!(
            provisional_finality
                .finality_artifact
                .validate_for_header(&block.header())
                .is_err(),
            "a pre-finalization artifact must not authenticate the completed local block"
        );
        replace_finalized_test_block_signature(&mut block, &block_signer);
        validate_sccp_commitment_root_for_signed_block(&block)
            .expect("completed local block authenticates its exact SCCP message");

        let fixture = fixture.with_finalized_block(&block, None);
        let finality =
            iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
                .expect("exact completed SCCP finality proof");
        assert_eq!(block.header(), finality.block_header);
        assert_eq!(block.hash(), finality.finality_artifact.block_hash);
        assert_eq!(
            fixture.request.public_inputs.finality_block_hash,
            <[u8; 32]>::from(Hash::from(block.hash()))
        );
        finality
            .finality_artifact
            .validate_for_header(&block.header())
            .expect("completed local finality artifact binds the exact block header");
        finality
            .finality_artifact
            .verify()
            .expect("completed local finality artifact is cryptographically valid");

        let messages = test_sccp_projections_from_block(&block);
        let state = TestSccpFinalityState {
            chain_id: finality.finality_artifact.height_context.chain_id.clone(),
            retained_header: Some(block.header()),
            messages,
            artifact: Some(finality.finality_artifact),
            artifact_error: None,
        };
        (fixture, state)
    }

    #[test]
    fn destination_context_uses_one_decode_pairing_and_verified_local_artifact() {
        let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
        let (fixture, state) = persisted_state_for_exact_sccp_fixture(&fixture);
        iroha_sccp::reset_sccp_destination_proof_work_counters_v1();
        let parsed = iroha_sccp::parse_sccp_destination_proof_v1(&fixture.bridge_proof)
            .expect("exact destination proof parses");
        let verified = iroha_sccp::verify_parsed_sccp_destination_proof_v1(parsed, &fixture.route)
            .expect("exact destination proof verifies against governed route");
        assert_eq!(
            iroha_sccp::sccp_destination_proof_work_counters_v1(),
            iroha_sccp::SccpDestinationProofWorkCountersV1 {
                artifact_framing_decodes: 1,
                bundle_decodes: 1,
                groth16_pairings: 1,
                bls_verifications: 0,
            }
        );
        verify_sccp_destination_context_against_local_state(&state, &verified)
            .expect("route-bound context must anchor to exact local v2 artifact");
        assert_eq!(
            iroha_sccp::sccp_destination_proof_work_counters_v1(),
            iroha_sccp::SccpDestinationProofWorkCountersV1 {
                artifact_framing_decodes: 1,
                bundle_decodes: 1,
                groth16_pairings: 1,
                bls_verifications: 0,
            },
            "local anchoring must not re-enter proof-controlled SCCP crypto"
        );
    }

    #[test]
    fn sccp_finality_local_state_check_rejects_missing_retained_finality_record() {
        let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
        let finality =
            iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
                .expect("exact fixture finality proof");
        let state = TestSccpFinalityState {
            chain_id: finality.finality_artifact.height_context.chain_id.clone(),
            retained_header: None,
            messages: Vec::new(),
            artifact: None,
            artifact_error: None,
        };
        let err = verify_sccp_finality_proof_against_local_state(&state, &finality)
            .expect_err("unanchored SCCP finality must fail before local crypto");
        assert!(err.contains("artifact for height 1 not found"), "{err}");
    }

    #[test]
    fn finality_builder_never_substitutes_an_adjacent_retained_height() {
        let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
        let (_, state) = persisted_state_for_exact_sccp_fixture(&fixture);

        assert_eq!(
            build_finality_proof(&state, 2),
            Err(BridgeFinalityError::FinalityArtifactNotFound(2))
        );
        let error = validated_sccp_finalized_messages_at_height(&state, 2)
            .expect_err("an adjacent request must not reuse height-one finality/archive data");
        assert!(error.contains("artifact for height 2 not found"), "{error}");
    }

    #[test]
    fn sccp_local_anchor_rejects_artifact_chain_and_record_substitution() {
        let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
        let (fixture, base) = persisted_state_for_exact_sccp_fixture(&fixture);
        let finality =
            iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
                .expect("exact completed fixture finality proof");

        let assert_rejected = |state: &TestSccpFinalityState, expected: &str| {
            let error = verify_sccp_finality_proof_against_local_state(state, &finality)
                .expect_err("adversarial local substitution must fail");
            assert!(
                error.contains(expected),
                "expected {expected:?}, got {error:?}"
            );
        };

        let mut attack = base.clone();
        attack.chain_id = "attacker-chain".into();
        assert_rejected(&attack, "chain id");

        let mut attack = base.clone();
        attack.artifact = None;
        assert_rejected(
            &attack,
            "Sumeragi-v2 finality artifact for height 1 not found",
        );

        let mut attack = base.clone();
        attack.artifact_error = Some("corrupt sidecar".to_owned());
        assert_rejected(&attack, "corrupt sidecar");

        let mut attack = base.clone();
        attack
            .artifact
            .as_mut()
            .expect("base artifact")
            .commit_qc
            .aggregate_signature[0] ^= 1;
        assert_rejected(
            &attack,
            "invalid Sumeragi-v2 quorum-certificate aggregate signature",
        );

        let mut attack = base.clone();
        attack
            .artifact
            .as_mut()
            .expect("base artifact")
            .validator_set_pops[0][0] ^= 1;
        assert_rejected(
            &attack,
            "invalid Sumeragi-v2 proof of possession at roster index 0",
        );

        let hostile_payload =
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(999, [0x44; 20]));
        let (hostile_block, _) = signed_block_with_sccp_payloads(&[hostile_payload], 1);
        let mut attack = base;
        attack.messages = test_sccp_projections_from_block(&hostile_block);
        assert_rejected(&attack, "reconstructs root");
    }

    #[test]
    fn sccp_commitment_root_is_none_for_empty_messages() {
        assert_eq!(sccp_commitment_root_from_messages(&[]), None);
    }

    #[test]
    fn sccp_commitment_root_matches_direct_merkle_root() {
        let payloads = vec![
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(1, [0x22; 20])),
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(2, [0x22; 20])),
        ];
        let (block, _) = signed_block_with_sccp_payloads(&payloads, 1);
        let messages = collect_sccp_messages_from_signed_block(&block);
        let commitments: Vec<_> = messages
            .iter()
            .map(|message| message.commitment.clone())
            .collect();

        assert_eq!(
            sccp_commitment_root_from_messages(&messages),
            iroha_sccp::commitment_merkle_root(&commitments)
        );
    }

    #[test]
    fn committed_block_rejects_513_self_consistent_outbound_messages() {
        let payloads = (0_u64
            ..=u64::from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1))
            .map(|nonce| {
                canonical_test_sccp_payload_bytes(&sample_transfer_payload(nonce + 1, [0x22; 20]))
            })
            .collect::<Vec<_>>();
        let (mut block, _) = signed_block_with_sccp_payloads(&payloads, 1);
        let messages = collect_sccp_messages_from_signed_block(&block);
        let root = sccp_commitment_root_from_messages(&messages).expect("nonempty SCCP root");
        block.set_sccp_commitment_root(Some(root));

        assert_eq!(
            validate_sccp_commitment_root_for_signed_block(&block),
            Err(SccpCommittedBlockValidationError::TooManyOutboundMessages {
                actual: 513,
                max: 512,
            })
        );
    }

    #[test]
    fn collect_sccp_messages_from_block_without_results_keeps_preexecution_records() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(15, [0x22; 20]));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload.clone())),
        ]));
        let block = signed_block_without_results(vec![tx], 13);

        assert!(!block.has_results());
        let messages = collect_sccp_messages_from_signed_block(&block);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tx_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
        );
    }

    #[test]
    fn collect_sccp_messages_from_empty_accepted_transactions_is_empty() {
        assert!(collect_sccp_messages_from_accepted_transactions(&[]).is_empty());
    }

    #[test]
    fn collect_sccp_messages_from_plain_instruction_executable() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(12, [0x22; 20]));
        let tx = signed_transaction_with_executable(Executable::Instructions(
            vec![InstructionBox::from(
                crate::bridge::test_record_sccp_message(payload.clone()),
            )]
            .into(),
        ));
        let block = signed_block_with_transactions(vec![tx], 1);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload)
                .expect("direct record payload decodes")
        );
    }

    #[test]
    fn collect_sccp_messages_from_block_preserves_payload_order() {
        let payloads = vec![
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(1, [0x22; 20])),
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(2, [0x22; 20])),
        ];
        let (block, decoded_payloads) = signed_block_with_sccp_payloads(&payloads, 1);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(
            messages
                .iter()
                .map(|message| &message.payload)
                .collect::<Vec<_>>(),
            decoded_payloads.iter().collect::<Vec<_>>()
        );

        let commitments: Vec<_> = messages
            .iter()
            .map(|message| message.commitment.clone())
            .collect();
        let root = sccp_commitment_root_from_messages(&messages).expect("commitment root");
        let proof = iroha_sccp::commitment_merkle_proof(&commitments, 1).expect("proof");
        assert_eq!(
            iroha_sccp::merkle_root_from_commitment(&messages[1].commitment, &proof),
            root
        );
    }

    #[test]
    fn commitment_paths_bind_first_middle_last_execution_indices_not_key_order() {
        let candidates = (1..=5)
            .map(|nonce| {
                canonical_test_sccp_payload_bytes(&sample_transfer_payload(nonce, [0x22; 20]))
            })
            .collect::<Vec<_>>();
        let (candidate_block, _) = signed_block_with_sccp_payloads(&candidates, 1);
        let candidate_messages = collect_sccp_messages_from_signed_block(&candidate_block);
        let mut ordered = candidates
            .into_iter()
            .zip(candidate_messages)
            .collect::<Vec<_>>();
        ordered.sort_by(|(_, left), (_, right)| {
            right.commitment.message_id.cmp(&left.commitment.message_id)
        });
        let payloads = ordered
            .into_iter()
            .map(|(payload, _)| payload)
            .collect::<Vec<_>>();
        let (block, _) = signed_block_with_sccp_payloads(&payloads, 1);
        let messages = collect_sccp_messages_from_signed_block(&block);
        assert!(
            messages
                .windows(2)
                .all(|pair| pair[0].commitment.message_id > pair[1].commitment.message_id),
            "fixture execution order must deliberately oppose ascending replay-key order"
        );
        let commitments = messages
            .iter()
            .map(|message| message.commitment.clone())
            .collect::<Vec<_>>();
        let root = iroha_sccp::commitment_merkle_root(&commitments).expect("five-message root");
        for index in [0, 2, 4] {
            let proof = iroha_sccp::commitment_merkle_proof(&commitments, index)
                .expect("first/middle/last path exists");
            assert_eq!(
                iroha_sccp::merkle_root_from_commitment(&messages[index].commitment, &proof),
                root,
                "execution-index path {index} must reconstruct the finalized root"
            );
        }
    }

    #[test]
    fn collect_sccp_messages_rejects_unprefixed_ascii_hex_record_payload_bytes() {
        let expected_payload = sample_transfer_payload(6, [0x22; 20]);
        let payload = canonical_test_sccp_payload_bytes(&expected_payload);
        let encoded_payload = hex::encode(&payload).into_bytes();
        let (block, _) = signed_block_with_sccp_payloads(&[encoded_payload], 4);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert!(
            messages.is_empty(),
            "ASCII hex payload aliases must not be collected as SCCP records"
        );
    }

    #[test]
    fn collect_sccp_messages_rejects_prefixed_ascii_hex_record_payload_bytes() {
        let expected_payload = sample_transfer_payload(7, [0x22; 20]);
        let payload = canonical_test_sccp_payload_bytes(&expected_payload);
        let encoded_payload = format!("0x{}", hex::encode(&payload)).into_bytes();
        let (block, _) = signed_block_with_sccp_payloads(&[encoded_payload], 4);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert!(
            messages.is_empty(),
            "prefixed ASCII hex payload aliases must not be collected as SCCP records"
        );
    }

    #[test]
    fn collect_sccp_messages_rejects_ascii_hex_record_payload_aliases() {
        let expected_payload = sample_transfer_payload(8, [0x22; 20]);
        let payload = canonical_test_sccp_payload_bytes(&expected_payload);
        let lowercase_hex = hex::encode(&payload);
        let uppercase_hex = lowercase_hex.to_ascii_uppercase();
        let cases = [
            lowercase_hex.as_bytes().to_vec(),
            format!("0x{lowercase_hex}").into_bytes(),
            uppercase_hex.as_bytes().to_vec(),
            format!("0X{lowercase_hex}").into_bytes(),
            format!(" {lowercase_hex}").into_bytes(),
            format!("{lowercase_hex}\n").into_bytes(),
            format!("{lowercase_hex}0").into_bytes(),
            b"not-hex".to_vec(),
        ];

        for encoded_payload in cases {
            let (block, _) = signed_block_with_sccp_payloads(&[encoded_payload], 5);
            assert!(
                collect_sccp_messages_from_signed_block(&block).is_empty(),
                "SCCP hex record payload aliases must be ignored"
            );
        }
    }

    #[test]
    fn collect_sccp_messages_ignores_ascii_hex_aliases_for_commitment_root() {
        let accepted_payload = sample_transfer_payload(9, [0x22; 20]);
        let accepted_bytes = canonical_test_sccp_payload_bytes(&accepted_payload);

        let rejected_payload = sample_transfer_payload(10, [0x22; 20]);
        let rejected_bytes = canonical_test_sccp_payload_bytes(&rejected_payload);
        let rejected_hex = hex::encode(&rejected_bytes);
        let uppercase_alias = rejected_hex.to_ascii_uppercase().into_bytes();
        let prefixed_alias = format!("0x{rejected_hex}").into_bytes();
        let padded_alias = format!("{rejected_hex}\n").into_bytes();
        let (block, _) = signed_block_with_sccp_payloads(
            &[
                uppercase_alias,
                accepted_bytes,
                prefixed_alias,
                padded_alias,
            ],
            6,
        );

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].payload, accepted_payload);

        let expected_commitment = test_sccp_hub_commitment(&accepted_payload);
        assert_eq!(
            sccp_commitment_root_from_messages(&messages),
            iroha_sccp::commitment_merkle_root(&[expected_commitment])
        );
    }

    #[test]
    fn collect_sccp_messages_skips_undecodable_payloads() {
        let payloads = vec![
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(3, [0x22; 20])),
            vec![0xff, 0x00, 0x01],
        ];
        let (block, decoded_payloads) = signed_block_with_sccp_payloads(&payloads, 2);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].payload, decoded_payloads[0]);
    }

    #[test]
    fn collect_sccp_messages_skips_non_sora_origin_payloads() {
        let inbound = SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
            version: 1,
            source_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            dest_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            nonce: 11,
            route_revision: 1,
            asset_home_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            asset_id: b"weth#eth".to_vec(),
            amount: 10,
            sender_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
            sender: [0x22; 20].to_vec(),
            recipient_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            recipient: b"alice@universal".to_vec(),
            route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            route_id: b"eth:sora:weth".to_vec(),
        });
        let (block, _) =
            signed_block_with_sccp_payloads(&[canonical_test_sccp_payload_bytes(&inbound)], 2);

        assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
    }

    #[test]
    fn collect_sccp_messages_skips_decodable_but_invalid_payloads() {
        let invalid = sample_transfer_payload(4, [0x22; 20]);
        let SccpPayloadV1::Transfer(mut invalid_transfer) = invalid;
        invalid_transfer.amount = 0;
        let invalid_payload = SccpPayloadV1::Transfer(invalid_transfer);
        assert!(
            iroha_sccp::decode_canonical_sccp_payload_bytes(&canonical_test_sccp_payload_bytes(
                &invalid_payload
            ))
            .is_some()
        );
        assert!(!iroha_sccp::verify_sccp_payload_structure(&invalid_payload));
        let valid_payload = sample_transfer_payload(5, [0x22; 20]);
        let payloads = vec![
            canonical_test_sccp_payload_bytes(&invalid_payload),
            canonical_test_sccp_payload_bytes(&valid_payload),
        ];
        let (block, _) = signed_block_with_sccp_payloads(&payloads, 3);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].payload, valid_payload);
        assert_eq!(messages[0].instruction_index, 1);
    }

    #[test]
    fn collect_sccp_messages_from_plain_ivm_executable_is_empty() {
        let tx =
            signed_transaction_with_executable(Executable::Ivm(IvmBytecode::from_compiled(vec![
                0x01, 0x02, 0x03,
            ])));
        let block = signed_block_with_transactions(vec![tx], 3);

        assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
    }

    #[test]
    fn collect_sccp_messages_from_contract_call_executable_is_empty() {
        let keypair = checked_keypair();
        let authority = AccountId::new(keypair.public_key().clone());
        let contract_address = ContractAddress::derive(
            CHAIN_DISCRIMINANT_MAINNET,
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let tx = signed_transaction_with_executable(Executable::ContractCall(ContractInvocation {
            contract_address,
            expected_code_hash: iroha_crypto::Hash::new(b"bridge-contract-code"),
            entrypoint: "bridge".to_string(),
            arguments: None,
        }));
        let block = signed_block_with_transactions(vec![tx], 4);

        assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
    }

    #[test]
    fn collect_sccp_messages_preserves_instruction_indices_after_skips() {
        let payloads = vec![
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(4, [0x22; 20])),
            vec![0x00, 0x01, 0xff],
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(5, [0x22; 20])),
        ];
        let (block, decoded_payloads) = signed_block_with_sccp_payloads(&payloads, 3);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages
                .iter()
                .map(|message| (message.tx_index, message.instruction_index))
                .collect::<Vec<_>>(),
            vec![(0, 0), (0, 2)]
        );
        assert_eq!(messages[0].payload, decoded_payloads[0]);
        assert_eq!(messages[1].payload, decoded_payloads[1]);
    }

    #[test]
    fn collect_sccp_messages_preserves_transaction_indices_across_block() {
        let first_payload =
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(6, [0x22; 20]));
        let second_payload =
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(7, [0x22; 20]));
        let ignored_tx =
            signed_transaction_with_executable(Executable::Ivm(IvmBytecode::from_compiled(vec![
                0xAA,
            ])));
        let first_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(first_payload)),
        ]));
        let second_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(second_payload)),
        ]));
        let block = signed_block_with_transactions(vec![ignored_tx, first_tx, second_tx], 5);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(
            messages
                .iter()
                .map(|message| (message.tx_index, message.instruction_index))
                .collect::<Vec<_>>(),
            vec![(1, 0), (2, 0)]
        );
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_skips_non_external_entrypoints() {
        let keypair = checked_keypair();
        let authority = AccountId::new(keypair.public_key().clone());
        let time_entry = TimeTriggerEntrypoint {
            id: "bridge_tick".parse::<TriggerId>().expect("trigger id"),
            instructions: ExecutionStep(Vec::<InstructionBox>::new().into()),
            authority,
        };
        let internal_tx = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(
            TransactionEntrypoint::Time(time_entry),
        ));

        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(6, [0x22; 20]));
        let external_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload.clone())),
        ]));
        let external_tx = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(
            TransactionEntrypoint::External(external_tx),
        ));

        let messages =
            collect_sccp_messages_from_accepted_transactions(&[internal_tx, external_tx]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tx_index, 1);
        assert_eq!(messages[0].instruction_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
        );
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_includes_sealed_reveals() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(7, [0x22; 20]));
        let [commitment, reveal] = sealed_sccp_record_entrypoints(payload.clone());
        let accepted_commitment =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment));
        let accepted_reveal = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(reveal));

        let messages = collect_sccp_messages_from_accepted_transactions(&[
            accepted_commitment,
            accepted_reveal,
        ]);

        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].tx_index, 1,
            "sealed commitment entrypoints must be counted when preserving canonical indices"
        );
        assert_eq!(messages[0].instruction_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
        );
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_deduplicates_outbound_keys() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(8, [0x22; 20]));
        let first = accepted_transaction_with_sccp_payload(payload.clone());
        let second = accepted_transaction_with_sccp_payload(payload.clone());

        let messages = collect_sccp_messages_from_accepted_transactions(&[first, second]);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tx_index, 0);
        assert_eq!(messages[0].instruction_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
        );
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_ignores_hex_aliases() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(8, [0x22; 20]));
        let hex_alias = format!("0x{}", hex::encode(&payload)).into_bytes();

        for (first, second, expected_tx_index) in [
            (payload.clone(), hex_alias.clone(), 0),
            (hex_alias.clone(), payload.clone(), 1),
        ] {
            let first = accepted_transaction_with_sccp_payload(first);
            let second = accepted_transaction_with_sccp_payload(second);

            let messages = collect_sccp_messages_from_accepted_transactions(&[first, second]);

            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0].tx_index, expected_tx_index);
            assert_eq!(messages[0].instruction_index, 0);
            assert_eq!(
                messages[0].payload,
                iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
            );
        }
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_filter_preserves_entry_indices() {
        let skipped_payload =
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(8, [0x22; 20]));
        let included_payload =
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(9, [0x22; 20]));
        let skipped = accepted_transaction_with_sccp_payload(skipped_payload);
        let included = accepted_transaction_with_sccp_payload(included_payload.clone());

        let messages = collect_new_sccp_messages_from_accepted_transactions_where(
            &[skipped, included],
            |tx_index| tx_index == 1,
            |_| false,
        );

        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].tx_index, 1,
            "route filtering must not renumber canonical transaction indices"
        );
        assert_eq!(messages[0].instruction_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&included_payload)
                .expect("payload decodes")
        );
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_skips_empty_outbound_route() {
        let mut payload = sample_transfer_payload(12, [0x22; 20]);
        let SccpPayloadV1::Transfer(transfer) = &mut payload;
        transfer.route_id.clear();
        let accepted =
            accepted_transaction_with_sccp_payload(canonical_test_sccp_payload_bytes(&payload));

        let messages = collect_sccp_messages_from_accepted_transactions(&[accepted]);

        assert!(
            messages.is_empty(),
            "proposal SCCP roots must not include records with empty route identifiers"
        );
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_skips_malformed_outbound_asset_scope() {
        let mut payload = sample_transfer_payload(23, [0x22; 20]);
        let SccpPayloadV1::Transfer(transfer) = &mut payload;
        transfer.asset_id = b"xor#".to_vec();
        transfer.route_id = iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1
            .as_bytes()
            .to_vec();
        let accepted =
            accepted_transaction_with_sccp_payload(canonical_test_sccp_payload_bytes(&payload));

        let messages = collect_sccp_messages_from_accepted_transactions(&[accepted]);

        assert!(
            messages.is_empty(),
            "proposal SCCP roots must not include asset-id aliases with empty scopes"
        );
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_skips_scoped_outbound_asset_alias() {
        let mut payload = sample_transfer_payload(25, [0x22; 20]);
        let SccpPayloadV1::Transfer(transfer) = &mut payload;
        transfer.asset_id = b"xor#universal".to_vec();
        transfer.route_id = iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1
            .as_bytes()
            .to_vec();
        let accepted =
            accepted_transaction_with_sccp_payload(canonical_test_sccp_payload_bytes(&payload));

        let messages = collect_sccp_messages_from_accepted_transactions(&[accepted]);

        assert!(
            messages.is_empty(),
            "proposal SCCP roots must not include scoped asset-id aliases"
        );
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_deduplicates_same_overlay_key() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(8, [0x22; 20]));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload.clone())),
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload.clone())),
        ]));
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

        let messages = collect_sccp_messages_from_accepted_transactions(&[accepted]);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tx_index, 0);
        assert_eq!(messages[0].instruction_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
        );
    }

    #[test]
    fn collect_new_sccp_messages_from_accepted_transactions_skips_existing_outbox_keys() {
        let payload = sample_transfer_payload(9, [0x22; 20]);
        let key = test_sccp_outbound_message_key(&payload);
        let accepted =
            accepted_transaction_with_sccp_payload(canonical_test_sccp_payload_bytes(&payload));

        let messages =
            collect_new_sccp_messages_from_accepted_transactions(&[accepted], |candidate| {
                candidate == &key
            });

        assert!(messages.is_empty());
    }

    #[test]
    fn collect_sccp_messages_from_block_deduplicates_successful_duplicate_outbound_keys() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(10, [0x22; 20]));
        let first_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload.clone())),
        ]));
        let second_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload.clone())),
        ]));
        let block = signed_block_with_transactions(vec![first_tx, second_tx], 9);

        let messages = collect_sccp_messages_from_signed_block(&block);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tx_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
        );
        let expected_commitment = test_sccp_hub_commitment(&messages[0].payload);
        assert_eq!(
            sccp_commitment_root_from_messages(&messages),
            iroha_sccp::commitment_merkle_root(&[expected_commitment])
        );
    }

    #[test]
    fn collect_sccp_messages_from_block_ignores_hex_aliases() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(11, [0x22; 20]));
        let encoded_payload = format!("0x{}", hex::encode(&payload)).into_bytes();
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(encoded_payload)),
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload.clone())),
        ]));
        let block = signed_block_with_transactions(vec![tx], 9);

        let messages = collect_sccp_messages_from_signed_block(&block);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tx_index, 0);
        assert_eq!(messages[0].instruction_index, 1);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
        );
    }

    #[test]
    fn local_sccp_finality_records_reject_duplicate_successful_outbound_keys() {
        let payload = sample_transfer_payload(12, [0x22; 20]);
        let payload_bytes = canonical_test_sccp_payload_bytes(&payload);
        let first_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(
                payload_bytes.clone(),
            )),
        ]));
        let second_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload_bytes)),
        ]));
        let block = signed_block_with_transactions(vec![first_tx, second_tx], 9);
        let messages = collect_sccp_messages_from_signed_block(&block);
        let deduped_root =
            sccp_commitment_root_from_messages(&messages).expect("deduped commitment root");

        let err = validate_local_sccp_records_against_commitment_root(&block, deduped_root)
            .expect_err("duplicate successful SCCP records must reject before root acceptance");

        assert!(err.contains("duplicate outbound message"));
        assert!(err.contains(&hex::encode(
            test_sccp_outbound_message_key(&payload).message_id
        )));
    }

    #[test]
    fn local_sccp_finality_records_reject_hex_alias_payload() {
        let payload = sample_transfer_payload(13, [0x22; 20]);
        let payload_bytes = canonical_test_sccp_payload_bytes(&payload);
        let encoded_payload = format!("0x{}", hex::encode(&payload_bytes)).into_bytes();
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(encoded_payload)),
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload_bytes)),
        ]));
        let block = signed_block_with_transactions(vec![tx], 9);
        let messages = collect_sccp_messages_from_signed_block(&block);
        let deduped_root =
            sccp_commitment_root_from_messages(&messages).expect("deduped commitment root");

        let err = validate_local_sccp_records_against_commitment_root(&block, deduped_root)
            .expect_err(
                "SCCP finality local record validation must reject encoded payload aliases",
            );

        assert!(err.contains("invalid outbound SCCP record"));
        assert!(err.contains("tx_index=0"));
        assert!(err.contains("instruction_index=0"));
        assert!(err.contains("payload is invalid"));
    }

    #[test]
    fn validate_sccp_commitment_root_for_signed_block_rejects_resultless_sccp_root() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(14, [0x22; 20]));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload)),
        ]));
        let mut block = signed_block_without_results(vec![tx], 9);
        let messages = collect_sccp_messages_from_signed_block(&block);
        let root = sccp_commitment_root_from_messages(&messages).expect("pre-execution SCCP root");
        block.set_sccp_commitment_root(Some(root));

        let err = validate_sccp_commitment_root_for_signed_block(&block)
            .expect_err("committed SCCP root validation must require committed results");

        assert_eq!(
            err,
            SccpCommittedBlockValidationError::MissingTransactionResults { actual: root }
        );
    }

    #[test]
    fn validate_sccp_commitment_root_for_signed_block_rejects_short_result_vector() {
        let plain_tx = signed_transaction_with_executable(Executable::Instructions(
            Vec::<InstructionBox>::new().into(),
        ));
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(16, [0x22; 20]));
        let sccp_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload)),
        ]));
        let mut block = signed_block_with_transactions(vec![plain_tx.clone()], 9);
        block.set_external_entrypoints(vec![
            TransactionEntrypoint::External(plain_tx),
            TransactionEntrypoint::External(sccp_tx),
        ]);

        let err = validate_sccp_commitment_root_for_signed_block(&block).expect_err(
            "committed SCCP validation must reject external entrypoints without committed results",
        );

        assert_eq!(
            err,
            SccpCommittedBlockValidationError::TransactionResultCountMismatch {
                external_entrypoints: 2,
                results: 1,
            }
        );
    }

    #[test]
    fn validate_sccp_commitment_root_for_signed_block_rejects_invalid_record_payload() {
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(
                b"not a canonical SCCP payload".to_vec(),
            )),
        ]));
        let block = signed_block_with_transactions(vec![tx], 9);

        let err = validate_sccp_commitment_root_for_signed_block(&block)
            .expect_err("successful invalid SCCP record payload must reject");

        assert_eq!(
            err,
            SccpCommittedBlockValidationError::InvalidRecordInstruction(
                SccpRecordInstructionValidationError::InvalidPayload {
                    tx_index: 0,
                    instruction_index: 0,
                }
            )
        );
    }

    #[test]
    fn validate_sccp_commitment_root_for_signed_block_rejects_hex_alias_payload() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(15, [0x22; 20]));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(
                format!("0x{}", hex::encode(&payload)).into_bytes(),
            )),
        ]));
        let block = signed_block_with_transactions(vec![tx], 9);

        let err = validate_sccp_commitment_root_for_signed_block(&block)
            .expect_err("successful hex-aliased SCCP record payload must reject");

        assert_eq!(
            err,
            SccpCommittedBlockValidationError::InvalidRecordInstruction(
                SccpRecordInstructionValidationError::InvalidPayload {
                    tx_index: 0,
                    instruction_index: 0,
                }
            )
        );
    }

    #[test]
    fn validate_sccp_commitment_root_for_signed_block_rejects_bare_transfer_payload() {
        let payload = sample_transfer_payload(20, [0x22; 20]);
        let SccpPayloadV1::Transfer(transfer) = payload;
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(
                canonical_test_transfer_payload_bytes(&transfer),
            )),
        ]));
        let block = signed_block_with_transactions(vec![tx], 9);

        let err = validate_sccp_commitment_root_for_signed_block(&block)
            .expect_err("successful bare transfer SCCP record payload must reject");

        assert_eq!(
            err,
            SccpCommittedBlockValidationError::InvalidRecordInstruction(
                SccpRecordInstructionValidationError::InvalidPayload {
                    tx_index: 0,
                    instruction_index: 0,
                }
            )
        );
    }

    #[test]
    fn validate_sccp_commitment_root_for_signed_block_rejects_non_sora_record_payload() {
        let payload = canonical_test_sccp_payload_bytes(&non_sora_source_transfer_payload(18));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload)),
        ]));
        let block = signed_block_with_transactions(vec![tx], 9);

        let err = validate_sccp_commitment_root_for_signed_block(&block)
            .expect_err("successful non-SORA SCCP record payload must reject");

        assert_eq!(
            err,
            SccpCommittedBlockValidationError::InvalidRecordInstruction(
                SccpRecordInstructionValidationError::NonSoraSource {
                    tx_index: 0,
                    instruction_index: 0,
                    source_domain: iroha_sccp::SCCP_DOMAIN_ETH,
                }
            )
        );
    }

    #[test]
    fn validate_sccp_commitment_root_for_signed_block_rejects_empty_outbound_route() {
        let mut payload = sample_transfer_payload(17, [0x22; 20]);
        let SccpPayloadV1::Transfer(transfer) = &mut payload;
        transfer.route_id.clear();
        let payload = canonical_test_sccp_payload_bytes(&payload);
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload)),
        ]));
        let block = signed_block_with_transactions(vec![tx], 9);

        let err = validate_sccp_commitment_root_for_signed_block(&block)
            .expect_err("successful empty outbound SCCP route must reject");

        assert_eq!(
            err,
            SccpCommittedBlockValidationError::InvalidRecordInstruction(
                SccpRecordInstructionValidationError::RouteBinding {
                    tx_index: 0,
                    instruction_index: 0,
                    error: SccpOutboundRouteValidationError::EmptyRouteId,
                }
            )
        );
    }

    #[test]
    fn recorded_sccp_route_validation_preserves_typed_errors_before_generic_structure() {
        #[derive(Clone, Copy)]
        enum Case {
            NonTextRoute,
            InvalidRouteUtf8,
            EmptyRoute,
            NonTextAsset,
            InvalidAssetUtf8,
            EmptyAsset,
            InvalidAsset,
            EmptyScope,
            AmbiguousScope,
            ScopedAlias,
            ForeignAssetHome,
        }

        let valid = sample_transfer_payload(170, [0x22; 20]);
        let valid_bytes = canonical_test_sccp_payload_bytes(&valid);
        let context = test_sccp_outbound_context_for_payload_bytes(&valid_bytes);
        for case in [
            Case::NonTextRoute,
            Case::InvalidRouteUtf8,
            Case::EmptyRoute,
            Case::NonTextAsset,
            Case::InvalidAssetUtf8,
            Case::EmptyAsset,
            Case::InvalidAsset,
            Case::EmptyScope,
            Case::AmbiguousScope,
            Case::ScopedAlias,
            Case::ForeignAssetHome,
        ] {
            let mut payload = valid.clone();
            let SccpPayloadV1::Transfer(transfer) = &mut payload;
            let expected = match case {
                Case::NonTextRoute => {
                    transfer.route_id_codec = iroha_sccp::SCCP_CODEC_EVM_ADDRESS20;
                    SccpOutboundRouteValidationError::NonTextRouteId
                }
                Case::InvalidRouteUtf8 => {
                    transfer.route_id = vec![0xFF];
                    SccpOutboundRouteValidationError::InvalidRouteIdUtf8
                }
                Case::EmptyRoute => {
                    transfer.route_id.clear();
                    SccpOutboundRouteValidationError::EmptyRouteId
                }
                Case::NonTextAsset => {
                    transfer.asset_id_codec = iroha_sccp::SCCP_CODEC_EVM_ADDRESS20;
                    SccpOutboundRouteValidationError::NonTextAssetId
                }
                Case::InvalidAssetUtf8 => {
                    transfer.asset_id = vec![0xFF];
                    SccpOutboundRouteValidationError::InvalidAssetIdUtf8
                }
                Case::EmptyAsset => {
                    transfer.asset_id.clear();
                    SccpOutboundRouteValidationError::EmptyAssetKey
                }
                Case::InvalidAsset => {
                    transfer.asset_id = b"bad name".to_vec();
                    SccpOutboundRouteValidationError::InvalidAssetKey
                }
                Case::EmptyScope => {
                    transfer.asset_id = b"xor#".to_vec();
                    SccpOutboundRouteValidationError::EmptyAssetScope
                }
                Case::AmbiguousScope => {
                    transfer.asset_id = b"xor#universal#shadow".to_vec();
                    SccpOutboundRouteValidationError::AmbiguousAssetScope
                }
                Case::ScopedAlias => {
                    transfer.asset_id = b"xor#universal".to_vec();
                    SccpOutboundRouteValidationError::AssetScopeAlias {
                        asset_key: "xor".to_owned(),
                        scope: "universal".to_owned(),
                    }
                }
                Case::ForeignAssetHome => {
                    transfer.asset_home_domain = iroha_sccp::SCCP_DOMAIN_ETH;
                    SccpOutboundRouteValidationError::InvalidAssetHomeDomain {
                        asset_home_domain: iroha_sccp::SCCP_DOMAIN_ETH,
                        dest_domain: transfer.dest_domain,
                    }
                }
            };
            let payload_bytes = canonical_test_sccp_payload_bytes(&payload);
            assert_eq!(
                validate_recorded_sccp_message_payload_bytes(context, &payload_bytes),
                Err(RecordedSccpMessageValidationError::RouteBinding { error: expected })
            );
        }

        let mut structurally_invalid = valid;
        let SccpPayloadV1::Transfer(transfer) = &mut structurally_invalid;
        transfer.amount = 0;
        let payload_bytes = canonical_test_sccp_payload_bytes(&structurally_invalid);
        assert_eq!(
            validate_recorded_sccp_message_payload_bytes(context, &payload_bytes),
            Err(RecordedSccpMessageValidationError::InvalidPayload)
        );
    }

    #[test]
    fn validate_sccp_commitment_root_for_signed_block_rejects_ambiguous_asset_scope() {
        let mut payload = sample_transfer_payload(24, [0x22; 20]);
        let SccpPayloadV1::Transfer(transfer) = &mut payload;
        transfer.asset_id = b"xor#universal#shadow".to_vec();
        let payload = canonical_test_sccp_payload_bytes(&payload);
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload)),
        ]));
        let block = signed_block_with_transactions(vec![tx], 9);

        let err = validate_sccp_commitment_root_for_signed_block(&block)
            .expect_err("successful ambiguous outbound SCCP asset scope must reject");

        assert_eq!(
            err,
            SccpCommittedBlockValidationError::InvalidRecordInstruction(
                SccpRecordInstructionValidationError::RouteBinding {
                    tx_index: 0,
                    instruction_index: 0,
                    error: SccpOutboundRouteValidationError::AmbiguousAssetScope,
                }
            )
        );
    }

    #[test]
    fn validate_sccp_commitment_root_for_signed_block_rejects_scoped_asset_alias() {
        let mut payload = sample_transfer_payload(25, [0x22; 20]);
        let SccpPayloadV1::Transfer(transfer) = &mut payload;
        transfer.asset_id = b"xor#universal".to_vec();
        let payload = canonical_test_sccp_payload_bytes(&payload);
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload)),
        ]));
        let block = signed_block_with_transactions(vec![tx], 9);

        let err = validate_sccp_commitment_root_for_signed_block(&block)
            .expect_err("successful scoped outbound SCCP asset alias must reject");

        assert_eq!(
            err,
            SccpCommittedBlockValidationError::InvalidRecordInstruction(
                SccpRecordInstructionValidationError::RouteBinding {
                    tx_index: 0,
                    instruction_index: 0,
                    error: SccpOutboundRouteValidationError::AssetScopeAlias {
                        asset_key: "xor".to_owned(),
                        scope: "universal".to_owned(),
                    },
                }
            )
        );
    }

    #[test]
    fn validate_sccp_commitment_root_for_signed_block_accepts_direct_record_instruction() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(19, [0x22; 20]));
        let tx = signed_transaction_with_executable(Executable::Instructions(
            vec![InstructionBox::from(
                crate::bridge::test_record_sccp_message(payload),
            )]
            .into(),
        ));
        let mut block = signed_block_with_transactions(vec![tx], 9);
        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(messages.len(), 1);
        let root = sccp_commitment_root_from_messages(&messages).expect("direct record root");
        block.set_sccp_commitment_root(Some(root));

        validate_sccp_commitment_root_for_signed_block(&block)
            .expect("successful direct SCCP record instruction must validate");
    }

    #[test]
    fn local_sccp_finality_records_reject_invalid_record_payload() {
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(
                b"not a canonical SCCP payload".to_vec(),
            )),
        ]));
        let block = signed_block_with_transactions(vec![tx], 9);

        let err = validate_local_sccp_records_against_commitment_root(&block, [0xAA; 32])
            .expect_err("local SCCP finality validation must reject invalid record payloads");

        assert!(err.contains("invalid outbound SCCP record"));
        assert!(err.contains("tx_index=0"));
        assert!(err.contains("instruction_index=0"));
        assert!(err.contains("payload is invalid"));
    }

    #[test]
    fn local_sccp_finality_records_reject_short_result_vector() {
        let plain_tx = signed_transaction_with_executable(Executable::Instructions(
            Vec::<InstructionBox>::new().into(),
        ));
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(17, [0x22; 20]));
        let sccp_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload)),
        ]));
        let mut block = signed_block_with_transactions(vec![plain_tx.clone()], 9);
        block.set_external_entrypoints(vec![
            TransactionEntrypoint::External(plain_tx),
            TransactionEntrypoint::External(sccp_tx),
        ]);

        let err = validate_local_sccp_records_against_commitment_root(&block, [0xAA; 32])
            .expect_err("local SCCP finality validation must reject short result vectors");

        assert!(err.contains("result count mismatch"));
        assert!(err.contains("external_entrypoints=2"));
        assert!(err.contains("results=1"));
    }

    #[test]
    fn local_sccp_finality_records_reject_resultless_matching_root() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(15, [0x22; 20]));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload)),
        ]));
        let block = signed_block_without_results(vec![tx], 9);
        let messages = collect_sccp_messages_from_signed_block(&block);
        let root = sccp_commitment_root_from_messages(&messages).expect("pre-execution SCCP root");

        let err = validate_local_sccp_records_against_commitment_root(&block, root)
            .expect_err("local SCCP finality validation must require committed results");

        assert!(err.contains("missing committed transaction results"));
    }

    #[test]
    fn collect_sccp_messages_from_block_skips_failed_transactions() {
        let first_payload =
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(10, [0x22; 20]));
        let second_payload =
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(11, [0x22; 20]));
        let first_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(
                first_payload.clone(),
            )),
        ]));
        let second_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(second_payload)),
        ]));
        let entry_hashes = vec![
            first_tx.hash_as_entrypoint(),
            second_tx.hash_as_entrypoint(),
        ];
        let mut block = signed_block_with_transactions(vec![first_tx, second_tx], 9);
        block
            .set_transaction_results(
                Vec::new(),
                &entry_hashes,
                vec![
                    TransactionResultInner::Ok(DataTriggerSequence::default()),
                    TransactionResultInner::Err(
                        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                            iroha_data_model::ValidationFail::NotPermitted(
                                "failed SCCP transaction fixture".to_owned(),
                            ),
                        ),
                    ),
                ],
            )
            .expect("test block entrypoint hashes should match payload");

        let messages = collect_sccp_messages_from_signed_block(&block);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tx_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&first_payload)
                .expect("payload decodes")
        );
    }

    #[test]
    fn collect_sccp_messages_from_block_skips_failed_external_with_time_trigger_result() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(12, [0x22; 20]));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload)),
        ]));
        let entry_hash = tx.hash_as_entrypoint();
        let keypair = checked_keypair();
        let authority = AccountId::new(keypair.public_key().clone());
        let time_entry = TimeTriggerEntrypoint {
            id: "bridge_tick_after_failure"
                .parse::<TriggerId>()
                .expect("trigger id"),
            instructions: ExecutionStep(Vec::<InstructionBox>::new().into()),
            authority,
        };
        let time_hash = time_entry.hash_as_entrypoint();
        let mut block = signed_block_with_transactions(vec![tx], 10);
        block
            .set_transaction_results(
                vec![time_entry],
                &[entry_hash, time_hash],
                vec![
                    TransactionResultInner::Err(
                        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                            iroha_data_model::ValidationFail::NotPermitted(
                                "failed SCCP transaction fixture".to_owned(),
                            ),
                        ),
                    ),
                    TransactionResultInner::Ok(DataTriggerSequence::default()),
                ],
            )
            .expect("test block entrypoint hashes should match payload");

        assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
    }

    #[test]
    fn collect_sccp_messages_from_block_uses_entrypoint_index_after_sealed_commitment() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(13, [0x22; 20]));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload)),
        ]));
        let entry_hash = tx.hash_as_entrypoint();
        let sealed_entrypoint = sealed_commitment_entrypoint();
        let sealed_hash = sealed_entrypoint.hash();
        let mut block = signed_block_with_transactions(vec![tx.clone()], 11);
        block
            .set_external_entrypoints(vec![sealed_entrypoint, TransactionEntrypoint::External(tx)]);
        block
            .set_transaction_results(
                Vec::new(),
                &[sealed_hash, entry_hash],
                vec![
                    TransactionResultInner::Ok(DataTriggerSequence::default()),
                    TransactionResultInner::Err(
                        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                            iroha_data_model::ValidationFail::NotPermitted(
                                "failed SCCP transaction fixture".to_owned(),
                            ),
                        ),
                    ),
                ],
            )
            .expect("test block entrypoint hashes should match payload");

        assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
    }

    #[test]
    fn collect_sccp_messages_from_block_includes_successful_sealed_reveal() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(14, [0x22; 20]));
        let [commitment, reveal] = sealed_sccp_record_entrypoints(payload.clone());
        let mut block = signed_block_with_transactions(Vec::new(), 12);
        block.set_external_entrypoints(vec![commitment, reveal]);
        let entry_hashes = block
            .external_entrypoints_cloned()
            .map(|entrypoint| entrypoint.hash())
            .collect::<Vec<_>>();
        block
            .set_transaction_results(
                Vec::new(),
                &entry_hashes,
                vec![
                    TransactionResultInner::Ok(DataTriggerSequence::default()),
                    TransactionResultInner::Ok(DataTriggerSequence::default()),
                ],
            )
            .expect("test block entrypoint hashes should match payload");

        let messages = collect_sccp_messages_from_signed_block(&block);

        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].tx_index, 1,
            "SCCP reveal record must keep the reveal entrypoint index"
        );
        assert_eq!(messages[0].instruction_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
        );
    }

    #[test]
    fn collect_sccp_messages_from_ivm_proved_overlay() {
        let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(7, [0x22; 20]));
        let executable = Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03]),
            overlay: vec![InstructionBox::from(
                crate::bridge::test_record_sccp_message(payload.clone()),
            )]
            .into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        });
        let tx = signed_transaction_with_executable(executable);
        let block = signed_block_with_transactions(vec![tx], 4);

        let messages = collect_sccp_messages_from_signed_block(&block);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tx_index, 0);
        assert_eq!(messages[0].instruction_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
        );
    }
}
