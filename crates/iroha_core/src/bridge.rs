//! Helpers for bridge finality proofs built from commit certificates.

use std::{
    collections::BTreeSet,
    fmt,
    num::NonZeroUsize,
    sync::{Arc, Mutex, OnceLock},
};

use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{
    ChainId,
    block::{BlockHeader, SignedBlock},
    bridge::{
        BRIDGE_FINALITY_PROOF_VERSION_V1, BridgeCommitment, BridgeFinalityBundle,
        BridgeFinalityProof, SccpOutboundMessageKeyV1,
    },
    isi::InstructionBox,
    name::Name,
    transaction::{Executable, TransactionEntrypoint},
};
use iroha_sccp::{SccpHubCommitmentV1, SccpPayloadV1, TairaBridgeFinalityProofV1};
use thiserror::Error;

use crate::{
    mmr::BlockMmr,
    state::{State as CoreState, StateReadOnly, consensus_key_pop_for_public_key},
    tx::AcceptedTransaction,
};

/// Narrow read-only surface used by bridge finality proof builders.
///
/// This keeps bridge-proof construction independent from full `StateView` snapshots.
pub trait BridgeStateReadOnly {
    /// Chain identifier bound to the state snapshot.
    fn bridge_chain_id(&self) -> &ChainId;
    /// Load a committed block at `height`.
    fn bridge_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>>;
    /// Load the exact durable Sumeragi-v2 finality artifact for `height`.
    fn bridge_v2_finality_artifact(
        &self,
        height: u64,
    ) -> Result<Option<iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact>, String>;
    /// Resolve a validator consensus-key proof-of-possession by public key.
    fn bridge_validator_pop(&self, public_key: &PublicKey) -> Option<Vec<u8>>;
}

impl<T: StateReadOnly> BridgeStateReadOnly for T {
    fn bridge_chain_id(&self) -> &ChainId {
        self.chain_id()
    }

    fn bridge_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
        self.kura().get_block(height)
    }

    fn bridge_v2_finality_artifact(
        &self,
        height: u64,
    ) -> Result<Option<iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact>, String>
    {
        self.kura()
            .v2_finality_artifact(height)
            .map_err(|error| error.to_string())
    }

    fn bridge_validator_pop(&self, public_key: &PublicKey) -> Option<Vec<u8>> {
        consensus_key_pop_for_public_key(self.world(), public_key)
    }
}

impl BridgeStateReadOnly for CoreState {
    fn bridge_chain_id(&self) -> &ChainId {
        self.chain_id_ref()
    }

    fn bridge_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
        self.block_by_height(height)
    }

    fn bridge_v2_finality_artifact(
        &self,
        height: u64,
    ) -> Result<Option<iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact>, String>
    {
        self.kura()
            .v2_finality_artifact(height)
            .map_err(|error| error.to_string())
    }

    fn bridge_validator_pop(&self, public_key: &PublicKey) -> Option<Vec<u8>> {
        let world = self.world_view();
        consensus_key_pop_for_public_key(&world, public_key)
    }
}

struct MmrCache {
    mmr: BlockMmr,
    height: u64,
    /// Chain id used to detect cross-ledger reuse in-process.
    chain_id: Option<ChainId>,
    /// Cached hash for the tip at `height` to detect top-block rewrites.
    tip_hash: Option<HashOf<BlockHeader>>,
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
    iroha_sccp::verify_sccp_payload_structure(&payload).then_some(payload)
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
    let key = sccp_outbound_message_key(context.lane, &payload)
        .ok_or(RecordedSccpMessageValidationError::InvalidContext)?;
    let commitment = iroha_sccp::hub_commitment_from_sccp_payload(context, &payload)
        .ok_or(RecordedSccpMessageValidationError::InvalidContext)?;
    let durable = iroha_data_model::bridge::SccpOutboundMessageRecordV1 {
        destination_binding_hash: context.destination_binding_hash,
        route_configuration_hash: context.route_configuration_hash,
        payload_hash: commitment.payload_hash,
        recorded_at_height: 1,
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
    let payload = decode_recorded_sccp_payload_bytes(payload_bytes)
        .ok_or(RecordedSccpMessageValidationError::InvalidPayload)?;
    validate_recorded_sccp_payload(context, payload)
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
    /// The requested block height is zero or does not fit into the host pointer width.
    #[error("invalid block height {0}")]
    InvalidHeight(u64),
    /// The block at the requested height was not found.
    #[error("block at height {0} not found")]
    BlockNotFound(u64),
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
    /// Validator `PoP` missing for the validator set entry.
    #[error("validator PoP missing for index {index}")]
    MissingValidatorPop {
        /// Index into the validator set.
        index: usize,
    },
    /// The exact durable artifact failed v2 quorum or BLS verification.
    #[error("Sumeragi-v2 finality artifact for height {height} failed verification: {reason}")]
    InvalidFinalityArtifact {
        /// Height being proven.
        height: u64,
        /// Typed verifier diagnostic.
        reason: String,
    },
}

fn compute_block_mmr(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<BlockMmr, BridgeFinalityError> {
    static BLOCK_MMR_CACHE: OnceLock<Mutex<MmrCache>> = OnceLock::new();

    if height == 0 {
        return Err(BridgeFinalityError::InvalidHeight(height));
    }

    let cache = BLOCK_MMR_CACHE.get_or_init(|| {
        Mutex::new(MmrCache {
            mmr: BlockMmr::default(),
            height: 0,
            chain_id: None,
            tip_hash: None,
        })
    });

    let mut guard = cache.lock().expect("mmr cache mutex poisoned");
    let chain_id = state.bridge_chain_id().clone();

    let mut rebuild = height < guard.height || guard.chain_id.as_ref() != Some(&chain_id);
    if !rebuild && guard.height > 0 {
        let cached_tip = guard.tip_hash;
        let current_tip = block_hash_at(state, guard.height)?;
        if cached_tip != Some(current_tip) {
            rebuild = true;
        }
    }

    if rebuild {
        // Rebuild from genesis to requested height to avoid rollback complexity.
        let mut fresh = BlockMmr::default();
        let mut tip_hash = None;
        for h in 1..=height {
            let hash = block_hash_at(state, h)?;
            fresh.push(hash);
            tip_hash = Some(hash);
        }
        guard.mmr = fresh;
        guard.height = height;
        guard.chain_id = Some(chain_id);
        guard.tip_hash = tip_hash;
    } else {
        let mut tip_hash = guard.tip_hash;
        for h in (guard.height + 1)..=height {
            let hash = block_hash_at(state, h)?;
            guard.mmr.push(hash);
            guard.height = h;
            tip_hash = Some(hash);
        }
        guard.chain_id = Some(chain_id);
        guard.tip_hash = tip_hash;
    }

    Ok(guard.mmr.clone())
}

fn block_hash_at(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>, BridgeFinalityError> {
    let h_usize: usize = height
        .try_into()
        .map_err(|_| BridgeFinalityError::InvalidHeight(height))?;
    let nonzero = NonZeroUsize::new(h_usize).ok_or(BridgeFinalityError::InvalidHeight(height))?;
    let block = state
        .bridge_block_by_height(nonzero)
        .ok_or(BridgeFinalityError::BlockNotFound(height))?;
    Ok(block.hash())
}

/// Build a self-contained finality proof for the block at `height`.
///
/// The proof bundles the block header, Kura's exact immutable v2 finality
/// artifact, and BLS PoPs aligned with the frozen powered roster.
///
/// # Errors
///
/// Returns [`BridgeFinalityError`] when the height is invalid, the block or
/// durable artifact is missing/malformed, a validator PoP is unavailable, or
/// the exact v2 artifact fails cryptographic verification.
pub fn build_finality_proof(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<BridgeFinalityProof, BridgeFinalityError> {
    let height_usize: usize = height
        .try_into()
        .map_err(|_| BridgeFinalityError::InvalidHeight(height))?;
    let nonzero_height =
        NonZeroUsize::new(height_usize).ok_or(BridgeFinalityError::InvalidHeight(height))?;

    let block = state
        .bridge_block_by_height(nonzero_height)
        .ok_or(BridgeFinalityError::BlockNotFound(height))?;
    let block_header = block.header();
    let block_hash = block.hash();
    let finality_artifact = state
        .bridge_v2_finality_artifact(height)
        .map_err(|reason| BridgeFinalityError::FinalityArtifactRead { height, reason })?
        .ok_or(BridgeFinalityError::FinalityArtifactNotFound(height))?;
    if finality_artifact.height_context.chain_id != *state.bridge_chain_id()
        || finality_artifact
            .validate_for_block(height, block_hash)
            .is_err()
    {
        return Err(BridgeFinalityError::FinalityArtifactMismatch { height });
    }

    let mut validator_set_pops = Vec::with_capacity(finality_artifact.height_context.roster.len());
    for (index, entry) in finality_artifact.height_context.roster.iter().enumerate() {
        let Some(pop) = state.bridge_validator_pop(entry.validator.public_key()) else {
            return Err(BridgeFinalityError::MissingValidatorPop { index });
        };
        validator_set_pops.push(pop);
    }
    finality_artifact
        .verify_with_validator_pops(&validator_set_pops)
        .map_err(|error| BridgeFinalityError::InvalidFinalityArtifact {
            height,
            reason: error.to_string(),
        })?;

    Ok(BridgeFinalityProof {
        version: BRIDGE_FINALITY_PROOF_VERSION_V1,
        block_header,
        finality_artifact,
        validator_set_pops,
    })
}

/// Build an MMR commitment plus exact typed finality proof for `height`.
///
/// # Errors
///
/// Returns [`BridgeFinalityError`] when the underlying finality proof or block MMR
/// cannot be built for the requested height.
pub fn build_finality_bundle(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<BridgeFinalityBundle, BridgeFinalityError> {
    let proof = build_finality_proof(state, height)?;
    let mmr = compute_block_mmr(state, height)?;
    let mmr_root = mmr.root();
    let commitment = BridgeCommitment {
        chain_id: proof.finality_artifact.height_context.chain_id.clone(),
        height_context_id: proof.finality_artifact.context_id(),
        block_height: proof.finality_artifact.height,
        block_hash: proof.finality_artifact.block_hash,
        mmr_root,
        mmr_leaf_index: mmr.leaves().checked_sub(1),
        mmr_peaks: Some(mmr.peaks.iter().map(|p| p.hash).collect()),
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
/// artifact differs from authoritative local state or its one BLS aggregate
/// verification fails.
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
    let height_usize = usize::try_from(height)
        .map_err(|_| format!("SCCP finality proof height {height} exceeds the host range"))?;
    let height_index = NonZeroUsize::new(height_usize)
        .ok_or_else(|| "SCCP finality proof height must be nonzero".to_owned())?;
    if artifact.height_context.chain_id != *state.bridge_chain_id() {
        return Err("SCCP finality proof chain id does not match local state".to_owned());
    }

    let local_block = state
        .bridge_block_by_height(height_index)
        .ok_or_else(|| format!("SCCP finality proof block {height} is not available locally"))?;
    if local_block.header() != finality.block_header || local_block.hash() != artifact.block_hash {
        return Err(
            "SCCP finality proof block header does not match the local canonical block".to_owned(),
        );
    }
    let commitment_root = finality
        .block_header
        .sccp_commitment_root()
        .ok_or_else(|| "SCCP finality proof block has no SCCP commitment root".to_owned())?;
    validate_local_sccp_records_against_commitment_root(&local_block, commitment_root)?;

    let local_artifact = state
        .bridge_v2_finality_artifact(height)
        .map_err(|reason| {
            format!(
                "failed to load local Sumeragi-v2 finality artifact at height {height}: {reason}"
            )
        })?
        .ok_or_else(|| {
            format!("local Sumeragi-v2 finality artifact at height {height} is missing")
        })?;
    if local_artifact != *artifact {
        return Err(
            "SCCP finality proof artifact does not match the exact durable local artifact"
                .to_owned(),
        );
    }

    let mut local_pops = Vec::with_capacity(artifact.height_context.roster.len());
    for (index, validator) in artifact.height_context.roster.iter().enumerate() {
        let pop = state
            .bridge_validator_pop(validator.validator.public_key())
            .ok_or_else(|| {
                format!("local validator proof of possession is missing at roster index {index}")
            })?;
        local_pops.push(pop);
    }
    if local_pops != finality.validator_set_pops {
        return Err(
            "SCCP finality proof PoPs do not match the authoritative local validator records"
                .to_owned(),
        );
    }

    count_sccp_local_bls_verification_for_tests();
    iroha_data_model::bridge::verify_bridge_finality_proof(finality, state.bridge_chain_id())
        .map_err(|error| {
            format!("SCCP finality proof cryptographic verification failed: {error}")
        })?;
    Ok(finality.clone())
}

#[cfg(test)]
std::thread_local! {
    static SCCP_LOCAL_BLS_VERIFICATIONS: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
}

#[cfg(test)]
fn count_sccp_local_bls_verification_for_tests() {
    SCCP_LOCAL_BLS_VERIFICATIONS.with(|counter| counter.set(counter.get().saturating_add(1)));
}

#[cfg(not(test))]
fn count_sccp_local_bls_verification_for_tests() {}

#[cfg(test)]
pub(crate) fn reset_sccp_local_bls_verifications_for_tests() {
    SCCP_LOCAL_BLS_VERIFICATIONS.with(|counter| counter.set(0));
}

#[cfg(test)]
pub(crate) fn sccp_local_bls_verifications_for_tests() -> usize {
    SCCP_LOCAL_BLS_VERIFICATIONS.with(core::cell::Cell::get)
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, num::NonZeroU64, sync::Arc};

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
        block: Option<Arc<SignedBlock>>,
        artifact: Option<iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact>,
        validator_pops: Vec<(PublicKey, Vec<u8>)>,
        artifact_error: Option<String>,
    }

    impl BridgeStateReadOnly for TestSccpFinalityState {
        fn bridge_chain_id(&self) -> &ChainId {
            &self.chain_id
        }

        fn bridge_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
            self.block.as_ref().and_then(|block| {
                (u64::try_from(height.get()).ok() == Some(block.header().height().get()))
                    .then(|| Arc::clone(block))
            })
        }

        fn bridge_v2_finality_artifact(
            &self,
            height: u64,
        ) -> Result<
            Option<iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact>,
            String,
        > {
            if let Some(error) = &self.artifact_error {
                return Err(error.clone());
            }
            Ok(self
                .artifact
                .as_ref()
                .filter(|artifact| artifact.height == height)
                .cloned())
        }

        fn bridge_validator_pop(&self, public_key: &PublicKey) -> Option<Vec<u8>> {
            self.validator_pops
                .iter()
                .find_map(|(candidate, pop)| (candidate == public_key).then(|| pop.clone()))
        }
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
        (block, decoded_payloads)
    }

    fn persisted_state_for_exact_sccp_fixture(
        fixture: &iroha_sccp::SccpExactOutboundTestFixtureV1,
        finality: &TairaBridgeFinalityProofV1,
    ) -> TestSccpFinalityState {
        let payload = canonical_test_sccp_payload_bytes(&fixture.bundle.payload);
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(payload)),
        ]));
        let entry_hash = tx.hash_as_entrypoint();
        let block_signer = checked_keypair();
        let signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(block_signer.private_key(), finality.block_header.hash())
                .expect("fixture local block signature"),
        );
        let mut block = SignedBlock::presigned(signature, finality.block_header.clone(), vec![tx]);
        block
            .set_transaction_results(
                Vec::new(),
                &[entry_hash],
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("fixture local block results");
        assert_eq!(block.hash(), finality.finality_artifact.block_hash);

        let validator_pops = finality
            .finality_artifact
            .height_context
            .roster
            .iter()
            .zip(&finality.validator_set_pops)
            .map(|(validator, pop)| (validator.validator.public_key().clone(), pop.clone()))
            .collect();
        TestSccpFinalityState {
            chain_id: finality.finality_artifact.height_context.chain_id.clone(),
            block: Some(Arc::new(block)),
            artifact: Some(finality.finality_artifact.clone()),
            validator_pops,
            artifact_error: None,
        }
    }

    #[test]
    fn destination_context_uses_one_decode_pairing_and_local_bls() {
        let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
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
        let state = persisted_state_for_exact_sccp_fixture(&fixture, verified.finality());
        reset_sccp_local_bls_verifications_for_tests();

        verify_sccp_destination_context_against_local_state(&state, &verified)
            .expect("route-bound context must anchor to exact local v2 artifact");
        assert_eq!(sccp_local_bls_verifications_for_tests(), 1);
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
    fn sccp_finality_local_state_check_rejects_missing_block_before_bls() {
        let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
        let finality =
            iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
                .expect("exact fixture finality proof");
        let state = TestSccpFinalityState {
            chain_id: finality.finality_artifact.height_context.chain_id.clone(),
            block: None,
            artifact: None,
            validator_pops: Vec::new(),
            artifact_error: None,
        };
        reset_sccp_local_bls_verifications_for_tests();
        let err = verify_sccp_finality_proof_against_local_state(&state, &finality)
            .expect_err("unanchored SCCP finality must fail before local crypto");
        assert!(err.contains("block 1 is not available locally"), "{err}");
        assert_eq!(sccp_local_bls_verifications_for_tests(), 0);
    }

    #[test]
    fn sccp_local_anchor_rejects_artifact_pop_chain_and_record_substitution_before_bls() {
        let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
        let finality =
            iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
                .expect("exact fixture finality proof");
        let base = persisted_state_for_exact_sccp_fixture(&fixture, &finality);

        let assert_rejected_before_bls = |state: &TestSccpFinalityState, expected: &str| {
            reset_sccp_local_bls_verifications_for_tests();
            let error = verify_sccp_finality_proof_against_local_state(state, &finality)
                .expect_err("adversarial local substitution must fail");
            assert!(
                error.contains(expected),
                "expected {expected:?}, got {error:?}"
            );
            assert_eq!(sccp_local_bls_verifications_for_tests(), 0);
        };

        let mut attack = base.clone();
        attack.chain_id = "attacker-chain".into();
        assert_rejected_before_bls(&attack, "chain id");

        let mut attack = base.clone();
        attack.artifact = None;
        assert_rejected_before_bls(&attack, "artifact at height 1 is missing");

        let mut attack = base.clone();
        attack.artifact_error = Some("corrupt sidecar".to_owned());
        assert_rejected_before_bls(&attack, "corrupt sidecar");

        let mut attack = base.clone();
        attack
            .artifact
            .as_mut()
            .expect("base artifact")
            .commit_qc
            .aggregate_signature[0] ^= 1;
        assert_rejected_before_bls(&attack, "exact durable local artifact");

        let mut attack = base.clone();
        attack.validator_pops.pop();
        assert_rejected_before_bls(&attack, "missing at roster index");

        let mut attack = base.clone();
        attack.validator_pops[0].1[0] ^= 1;
        assert_rejected_before_bls(&attack, "authoritative local validator records");

        let hostile_payload =
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(999, [0x44; 20]));
        let hostile_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(crate::bridge::test_record_sccp_message(hostile_payload)),
        ]));
        let hostile_entry_hash = hostile_tx.hash_as_entrypoint();
        let signer = checked_keypair();
        let signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(signer.private_key(), finality.block_header.hash())
                .expect("hostile local block signature"),
        );
        let mut hostile_block =
            SignedBlock::presigned(signature, finality.block_header.clone(), vec![hostile_tx]);
        hostile_block
            .set_transaction_results(
                Vec::new(),
                &[hostile_entry_hash],
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("hostile local block results");
        let mut attack = base;
        attack.block = Some(Arc::new(hostile_block));
        assert_rejected_before_bls(&attack, "commitment root does not match local SCCP records");
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
        let SccpPayloadV1::Transfer(mut invalid_transfer) = invalid else {
            panic!("sample payload should be a transfer");
        };
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
        let SccpPayloadV1::Transfer(transfer) = &mut payload else {
            unreachable!("sample payload is a transfer");
        };
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
        let SccpPayloadV1::Transfer(transfer) = payload else {
            unreachable!("sample payload is a transfer");
        };
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
        let SccpPayloadV1::Transfer(transfer) = &mut payload else {
            unreachable!("sample payload is a transfer");
        };
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
