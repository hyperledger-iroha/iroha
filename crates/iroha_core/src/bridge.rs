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
        BridgeAuthoritySet, BridgeCommitment, BridgeCommitmentJustification, BridgeFinalityBundle,
        BridgeFinalityProof, SccpOutboundMessageKey,
    },
    consensus::{Qc, VALIDATOR_SET_HASH_VERSION_V1},
    isi::InstructionBox,
    name::Name,
    peer::PeerId,
    transaction::{Executable, TransactionEntrypoint},
};
use iroha_sccp::{
    NexusBridgeFinalityProofV1, NexusConsensusPhaseV1, NexusQcRefV1, SccpHubCommitmentV1,
    SccpPayloadV1,
};
use thiserror::Error;

use crate::{
    mmr::BlockMmr,
    state::{
        State as CoreState, StateReadOnly, StateTransaction, consensus_key_pop_for_public_key,
    },
    sumeragi,
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
    /// Load the commit certificate persisted for `height`/`block_hash`.
    fn bridge_commit_qc_for_block(
        &self,
        height: u64,
        block_hash: HashOf<BlockHeader>,
    ) -> Option<Qc>;
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

    fn bridge_commit_qc_for_block(
        &self,
        height: u64,
        block_hash: HashOf<BlockHeader>,
    ) -> Option<Qc> {
        let sidecar = self.kura().read_roster_metadata(height)?;
        let commit_qc = sidecar.commit_qc?;
        (commit_qc.height == height && commit_qc.subject_block_hash == block_hash)
            .then_some(commit_qc)
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

    fn bridge_commit_qc_for_block(
        &self,
        height: u64,
        block_hash: HashOf<BlockHeader>,
    ) -> Option<Qc> {
        self.commit_roster_snapshot_for_block(height, block_hash)
            .map(|snapshot| snapshot.commit_qc)
            .or_else(|| self.commit_qc_for_block(height, block_hash))
    }

    fn bridge_validator_pop(&self, public_key: &PublicKey) -> Option<Vec<u8>> {
        let world = self.world_view();
        consensus_key_pop_for_public_key(&world, public_key)
    }
}

/// Narrow read-only surface used to validate SCCP finality proofs against local state.
pub trait SccpFinalityStateReadOnly {
    /// Chain identifier bound to the local committed chain.
    fn sccp_chain_id(&self) -> &ChainId;
    /// Load the locally committed block at `height`.
    fn sccp_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>>;
    /// Load the locally trusted commit QC for `height` and `block_hash`.
    fn sccp_commit_qc_for_block(&self, height: u64, block_hash: HashOf<BlockHeader>) -> Option<Qc>;
}

impl SccpFinalityStateReadOnly for CoreState {
    fn sccp_chain_id(&self) -> &ChainId {
        self.chain_id_ref()
    }

    fn sccp_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
        self.block_by_height(height)
    }

    fn sccp_commit_qc_for_block(&self, height: u64, block_hash: HashOf<BlockHeader>) -> Option<Qc> {
        self.commit_roster_snapshot_for_block(height, block_hash)
            .map(|snapshot| snapshot.commit_qc)
    }
}

impl SccpFinalityStateReadOnly for StateTransaction<'_, '_> {
    fn sccp_chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    fn sccp_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
        self.block_by_height(height)
    }

    fn sccp_commit_qc_for_block(&self, height: u64, block_hash: HashOf<BlockHeader>) -> Option<Qc> {
        self.commit_qc_for_block(height, block_hash)
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
    /// Canonically decoded SCCP payload recorded by the instruction.
    pub payload: SccpPayloadV1,
    /// Commitment derived from the decoded SCCP payload.
    pub commitment: SccpHubCommitmentV1,
}

/// Canonically validated outbound SCCP record data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedRecordedSccpMessage {
    /// Canonically decoded SCCP payload recorded by the instruction.
    pub payload: SccpPayloadV1,
    /// Lane-independent outbound replay key derived from the payload.
    pub key: SccpOutboundMessageKey,
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
    /// Payload bytes are not exact canonical SCCP variant framing.
    InvalidPayload,
    /// Outbound records may only originate from SORA.
    NonSoraSource {
        /// Source domain found in the payload.
        source_domain: u32,
    },
    /// Payload route fields are not bound to the deterministic outbound route.
    RouteBinding {
        /// Route binding validation error.
        error: SccpOutboundRouteValidationError,
    },
}

pub(crate) fn decode_recorded_sccp_payload_bytes(payload_bytes: &[u8]) -> Option<SccpPayloadV1> {
    let payload = iroha_sccp::decode_canonical_sccp_payload_bytes(payload_bytes)?;
    if iroha_sccp::canonical_sccp_payload_bytes(&payload).as_slice() != payload_bytes {
        return None;
    }
    iroha_sccp::verify_sccp_payload_structure(&payload).then_some(payload)
}

pub(crate) fn decode_lowercase_hex_sccp_payload_alias(
    payload_bytes: &[u8],
) -> Option<SccpPayloadV1> {
    let payload_text = std::str::from_utf8(payload_bytes).ok()?;
    let hex_text = payload_text.strip_prefix("0x").unwrap_or(payload_text);
    if hex_text.is_empty() || !hex_text.len().is_multiple_of(2) {
        return None;
    }
    if !hex_text
        .as_bytes()
        .iter()
        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return None;
    }
    let payload = hex::decode(hex_text).ok()?;
    decode_recorded_sccp_payload_bytes(&payload)
}

fn validate_recorded_sccp_payload(
    payload: SccpPayloadV1,
) -> Result<ValidatedRecordedSccpMessage, RecordedSccpMessageValidationError> {
    let source_domain = iroha_sccp::sccp_message_source_domain(&payload);
    if source_domain != iroha_sccp::SCCP_DOMAIN_SORA {
        return Err(RecordedSccpMessageValidationError::NonSoraSource { source_domain });
    }
    validate_sora_outbound_sccp_payload_route(&payload)
        .map_err(|error| RecordedSccpMessageValidationError::RouteBinding { error })?;
    Ok(ValidatedRecordedSccpMessage {
        key: sccp_outbound_message_key(&payload),
        commitment: iroha_sccp::hub_commitment_from_sccp_payload(&payload),
        payload,
    })
}

pub(crate) fn validate_recorded_sccp_message_payload_bytes(
    payload_bytes: &[u8],
) -> Result<ValidatedRecordedSccpMessage, RecordedSccpMessageValidationError> {
    let payload = decode_recorded_sccp_payload_bytes(payload_bytes)
        .ok_or(RecordedSccpMessageValidationError::InvalidPayload)?;
    validate_recorded_sccp_payload(payload)
}

fn validate_recorded_sccp_message_payload_bytes_for_block_collection(
    payload_bytes: &[u8],
) -> Result<ValidatedRecordedSccpMessage, RecordedSccpMessageValidationError> {
    validate_recorded_sccp_message_payload_bytes(payload_bytes)
}

pub(crate) fn sccp_outbound_message_key(payload: &SccpPayloadV1) -> SccpOutboundMessageKey {
    SccpOutboundMessageKey {
        source_domain: iroha_sccp::sccp_message_source_domain(payload),
        target_domain: iroha_sccp::sccp_message_target_domain(payload),
        message_id: iroha_sccp::sccp_message_id(payload),
    }
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
    validate_recorded_sccp_message_payload_bytes(&record.payload_bytes).map(Some)
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
    /// Route id is not encoded as SCCP `text_utf8`.
    NonTextRouteId,
    /// Asset id is not encoded as SCCP `text_utf8`.
    NonTextAssetId,
    /// Route id bytes are not valid UTF-8.
    InvalidRouteIdUtf8,
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
    /// Destination domain does not have a canonical route slug.
    UnknownDestinationDomain(u32),
    /// Asset home domain is neither SORA nor the destination domain.
    InvalidAssetHomeDomain {
        /// Asset home domain in the payload.
        asset_home_domain: u32,
        /// Destination domain in the payload.
        dest_domain: u32,
    },
    /// Route id does not match the asset and domain binding.
    RouteIdMismatch {
        /// Actual route id from the payload.
        actual: String,
        /// Expected deterministic route id for the payload binding.
        expected: String,
    },
}

impl SccpOutboundRouteValidationError {
    pub(crate) fn reason(&self) -> &'static str {
        match self {
            Self::NonTextRouteId => "RecordSccpMessage payload route_id is not text_utf8",
            Self::NonTextAssetId => "RecordSccpMessage payload asset_id is not text_utf8",
            Self::InvalidRouteIdUtf8 => "RecordSccpMessage payload route_id is invalid UTF-8",
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
            Self::UnknownDestinationDomain(_) => {
                "RecordSccpMessage payload destination domain has no route slug"
            }
            Self::InvalidAssetHomeDomain { .. } => {
                "RecordSccpMessage payload asset home domain is not bound to SORA or destination"
            }
            Self::RouteIdMismatch { .. } => {
                "RecordSccpMessage payload route_id does not match asset/domain binding"
            }
        }
    }
}

impl fmt::Display for SccpOutboundRouteValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonTextRouteId => write!(f, "route_id must use text_utf8 codec"),
            Self::NonTextAssetId => write!(f, "asset_id must use text_utf8 codec"),
            Self::InvalidRouteIdUtf8 => write!(f, "route_id must be valid UTF-8"),
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
            Self::UnknownDestinationDomain(domain) => {
                write!(f, "destination domain {domain} has no route slug")
            }
            Self::InvalidAssetHomeDomain {
                asset_home_domain,
                dest_domain,
            } => write!(
                f,
                "asset home domain {asset_home_domain} must be SORA or the destination domain {dest_domain}"
            ),
            Self::RouteIdMismatch { actual, expected } => write!(
                f,
                "route_id `{actual}` does not match asset/domain binding `{expected}`"
            ),
        }
    }
}

fn sccp_domain_route_slug(domain: u32) -> Option<&'static str> {
    match domain {
        iroha_sccp::SCCP_DOMAIN_SORA => Some("sora"),
        iroha_sccp::SCCP_DOMAIN_ETH => Some("eth"),
        iroha_sccp::SCCP_DOMAIN_BSC => Some("bsc"),
        iroha_sccp::SCCP_DOMAIN_SOL => Some("sol"),
        iroha_sccp::SCCP_DOMAIN_TON => Some("ton"),
        iroha_sccp::SCCP_DOMAIN_TRON => Some("tron"),
        _ => None,
    }
}

fn sccp_text_route_field<'a>(
    codec: u8,
    bytes: &'a [u8],
    non_text: SccpOutboundRouteValidationError,
    invalid_utf8: SccpOutboundRouteValidationError,
) -> Result<&'a str, SccpOutboundRouteValidationError> {
    if codec != iroha_sccp::SCCP_CODEC_TEXT_UTF8 {
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

fn is_known_taira_xor_outbound_route_fields(
    route_id: &str,
    asset_id: &str,
    target_domain: u32,
    asset_home_domain: Option<u32>,
) -> bool {
    let asset_home_is_sora = match asset_home_domain {
        Some(domain) => domain == iroha_sccp::SCCP_DOMAIN_SORA,
        None => true,
    };
    asset_home_is_sora
        && asset_id == iroha_sccp::SCCP_TAIRA_XOR_ASSET_KEY_V1
        && matches!(
            (route_id, target_domain),
            (
                iroha_sccp::SCCP_TAIRA_TRON_XOR_ROUTE_ID_V1,
                iroha_sccp::SCCP_DOMAIN_TRON
            ) | (
                iroha_sccp::SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1,
                iroha_sccp::SCCP_DOMAIN_BSC
            ) | ("taira_ton_xor", iroha_sccp::SCCP_DOMAIN_TON)
        )
}

fn expected_sora_home_outbound_route_id(
    target_domain: u32,
    asset_key: &str,
) -> Result<String, SccpOutboundRouteValidationError> {
    let target_slug = sccp_domain_route_slug(target_domain).ok_or(
        SccpOutboundRouteValidationError::UnknownDestinationDomain(target_domain),
    )?;
    Ok(format!("nexus:{target_slug}:{asset_key}"))
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
    if is_known_taira_xor_outbound_route_fields(
        route_id,
        asset_id,
        transfer.dest_domain,
        Some(transfer.asset_home_domain),
    ) {
        return Ok(());
    }

    let asset_key = sccp_route_asset_key(asset_id)?;
    let dest_slug = sccp_domain_route_slug(transfer.dest_domain).ok_or(
        SccpOutboundRouteValidationError::UnknownDestinationDomain(transfer.dest_domain),
    )?;
    let expected_route_id = if transfer.asset_home_domain == iroha_sccp::SCCP_DOMAIN_SORA {
        expected_sora_home_outbound_route_id(transfer.dest_domain, asset_key)?
    } else if transfer.asset_home_domain == transfer.dest_domain {
        format!("{dest_slug}:sora:{asset_key}")
    } else {
        return Err(SccpOutboundRouteValidationError::InvalidAssetHomeDomain {
            asset_home_domain: transfer.asset_home_domain,
            dest_domain: transfer.dest_domain,
        });
    };
    if route_id != expected_route_id {
        return Err(SccpOutboundRouteValidationError::RouteIdMismatch {
            actual: route_id.to_owned(),
            expected: expected_route_id,
        });
    }
    Ok(())
}

fn validate_sora_outbound_route_activation(
    activation: &iroha_sccp::RouteActivatePayloadV1,
) -> Result<(), SccpOutboundRouteValidationError> {
    let route_id = sccp_text_route_field(
        activation.route_id_codec,
        activation.route_id.as_slice(),
        SccpOutboundRouteValidationError::NonTextRouteId,
        SccpOutboundRouteValidationError::InvalidRouteIdUtf8,
    )?;
    let asset_id = sccp_text_route_field(
        activation.asset_id_codec,
        activation.asset_id.as_slice(),
        SccpOutboundRouteValidationError::NonTextAssetId,
        SccpOutboundRouteValidationError::InvalidAssetIdUtf8,
    )?;
    if is_known_taira_xor_outbound_route_fields(route_id, asset_id, activation.target_domain, None)
    {
        return Ok(());
    }

    let asset_key = sccp_route_asset_key(asset_id)?;
    let expected_route_id =
        expected_sora_home_outbound_route_id(activation.target_domain, asset_key)?;
    if route_id != expected_route_id {
        return Err(SccpOutboundRouteValidationError::RouteIdMismatch {
            actual: route_id.to_owned(),
            expected: expected_route_id,
        });
    }
    Ok(())
}

/// Validate deterministic route binding for SORA-origin outbound SCCP records.
pub(crate) fn validate_sora_outbound_sccp_payload_route(
    payload: &SccpPayloadV1,
) -> Result<(), SccpOutboundRouteValidationError> {
    match payload {
        SccpPayloadV1::RouteActivate(activation) => {
            validate_sora_outbound_route_activation(activation)?
        }
        SccpPayloadV1::Transfer(transfer) => validate_sora_outbound_transfer_route(transfer)?,
        SccpPayloadV1::AssetRegister(_)
        | SccpPayloadV1::TokenAdd(_)
        | SccpPayloadV1::TokenPause(_)
        | SccpPayloadV1::TokenResume(_) => {}
    }
    Ok(())
}

fn collect_sccp_messages_from_executable<F>(
    tx_index: usize,
    executable: &Executable,
    seen: &mut BTreeSet<SccpOutboundMessageKey>,
    is_already_recorded: &F,
    deduplicate: bool,
    out: &mut Vec<RecordedSccpMessage>,
) where
    F: Fn(&SccpOutboundMessageKey) -> bool,
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
            commitment: validated.commitment,
            payload: validated.payload,
        });
    };

    match executable {
        Executable::Instructions(_) | Executable::ContractCall(_) | Executable::Ivm(_) => {}
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
    let Executable::IvmProved(proved) = executable else {
        return Vec::new();
    };

    proved
        .overlay
        .iter()
        .enumerate()
        .filter_map(|(instruction_index, instruction)| {
            let record = recorded_sccp_message_instruction(instruction)?;
            let Ok(validated) = validate_recorded_sccp_message_payload_bytes_for_block_collection(
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
    F: Fn(&SccpOutboundMessageKey) -> bool,
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
    G: Fn(&SccpOutboundMessageKey) -> bool,
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
    /// `RecordSccpMessage` appeared outside a verified `IvmProved` overlay.
    UnsupportedExecutable {
        /// External entrypoint index in the block payload.
        tx_index: usize,
        /// Instruction index inside the executable instruction list.
        instruction_index: usize,
    },
    /// `RecordSccpMessage` payload bytes could not be decoded as a valid SCCP payload.
    InvalidPayload {
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
            Self::UnsupportedExecutable { tx_index, .. }
            | Self::InvalidPayload { tx_index, .. }
            | Self::NonSoraSource { tx_index, .. }
            | Self::RouteBinding { tx_index, .. } => *tx_index,
        }
    }

    pub(crate) fn instruction_index(&self) -> usize {
        match self {
            Self::UnsupportedExecutable {
                instruction_index, ..
            }
            | Self::InvalidPayload {
                instruction_index, ..
            }
            | Self::NonSoraSource {
                instruction_index, ..
            }
            | Self::RouteBinding {
                instruction_index, ..
            } => *instruction_index,
        }
    }

    pub(crate) fn reason(&self) -> &'static str {
        match self {
            Self::UnsupportedExecutable { .. } => {
                "RecordSccpMessage requires a verified IvmProved overlay"
            }
            Self::InvalidPayload { .. } => "RecordSccpMessage payload is invalid",
            Self::NonSoraSource { .. } => "RecordSccpMessage payload source domain is not SORA",
            Self::RouteBinding { error, .. } => error.reason(),
        }
    }
}

fn invalid_sccp_record_instruction_in_executable(
    tx_index: usize,
    executable: &Executable,
) -> Option<SccpRecordInstructionValidationError> {
    match executable {
        Executable::Instructions(instructions) => {
            instructions
                .iter()
                .enumerate()
                .find_map(|(instruction_index, instruction)| {
                    recorded_sccp_message_instruction(instruction).map(|_| {
                        SccpRecordInstructionValidationError::UnsupportedExecutable {
                            tx_index,
                            instruction_index,
                        }
                    })
                })
        }
        Executable::IvmProved(proved) => {
            proved
                .overlay
                .iter()
                .enumerate()
                .find_map(|(instruction_index, instruction)| {
                    let record = recorded_sccp_message_instruction(instruction)?;
                    match validate_recorded_sccp_message_payload_bytes(&record.payload_bytes) {
                        Ok(_) => None,
                        Err(RecordedSccpMessageValidationError::InvalidPayload) => {
                            Some(SccpRecordInstructionValidationError::InvalidPayload {
                                tx_index,
                                instruction_index,
                            })
                        }
                        Err(RecordedSccpMessageValidationError::NonSoraSource {
                            source_domain,
                        }) => Some(SccpRecordInstructionValidationError::NonSoraSource {
                            tx_index,
                            instruction_index,
                            source_domain,
                        }),
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
        Executable::ContractCall(_) | Executable::Ivm(_) => None,
    }
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
) -> Option<SccpOutboundMessageKey> {
    let mut seen = BTreeSet::new();
    for message in collect_sccp_messages_from_signed_block_with_deduplication(block, false) {
        let key = sccp_outbound_message_key(&message.payload);
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
    DuplicateOutboundMessage(SccpOutboundMessageKey),
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
#[derive(Debug, Error, Copy, Clone)]
pub enum BridgeFinalityError {
    /// The requested block height is zero or does not fit into the host pointer width.
    #[error("invalid block height {0}")]
    InvalidHeight(u64),
    /// The block at the requested height was not found.
    #[error("block at height {0} not found")]
    BlockNotFound(u64),
    /// No commit certificate was found for the requested height.
    #[error("commit certificate for height {0} not found")]
    QcNotFound(u64),
    /// The commit certificate references a different block hash than the stored block.
    #[error(
        "commit certificate hash {cert_hash:?} does not match block hash {block_hash:?} at height {height}"
    )]
    QcHashMismatch {
        /// Height being proven.
        height: u64,
        /// Hash recorded inside the commit certificate.
        cert_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
        /// Hash of the stored block header.
        block_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
    },
    /// Validator `PoP` missing for the validator set entry.
    #[error("validator PoP missing for index {index}")]
    MissingValidatorPop {
        /// Index into the validator set.
        index: usize,
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
/// The proof bundles the block header, its hash, and the commit certificate
/// collected for that block. Verifiers recompute the block hash from the header
/// and validate the commit certificate signatures against the provided
/// validator set.
///
/// # Errors
///
/// Returns [`BridgeFinalityError`] when the height is invalid, the block or commit
/// certificate is missing, or their hashes do not match.
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

    let mut cert_candidates: Vec<_> = sumeragi::status::commit_qc_history()
        .into_iter()
        .filter(|entry| entry.height == height)
        .collect();
    let cert = if let Some(cert) = cert_candidates
        .iter()
        .find(|candidate| candidate.subject_block_hash == block_hash)
    {
        cert.clone()
    } else if let Some(cert) = state.bridge_commit_qc_for_block(height, block_hash) {
        cert
    } else if let Some(cert) = cert_candidates.pop() {
        return Err(BridgeFinalityError::QcHashMismatch {
            height,
            cert_hash: cert.subject_block_hash,
            block_hash,
        });
    } else {
        return Err(BridgeFinalityError::QcNotFound(height));
    };

    let mut validator_set_pops = Vec::with_capacity(cert.validator_set.len());
    for (index, peer) in cert.validator_set.iter().enumerate() {
        let Some(pop) = state.bridge_validator_pop(peer.public_key()) else {
            return Err(BridgeFinalityError::MissingValidatorPop { index });
        };
        validator_set_pops.push(pop);
    }

    Ok(BridgeFinalityProof {
        height,
        chain_id: state.bridge_chain_id().clone(),
        block_header,
        block_hash,
        commit_qc: cert,
        validator_set_pops,
    })
}

/// Build a commitment + justification bundle for the block at `height`.
///
/// The bundle relies on the commit certificate aggregate signature for
/// justification; the historical signature list is left empty.
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
    let authority_set = BridgeAuthoritySet {
        id: height, // simple monotonically increasing id derived from height; future revisions can carry explicit ids
        validator_set: proof.commit_qc.validator_set.clone(),
        validator_set_hash: proof.commit_qc.validator_set_hash,
        validator_set_hash_version: proof.commit_qc.validator_set_hash_version,
    };
    let commitment = BridgeCommitment {
        chain_id: proof.chain_id.clone(),
        authority_set: authority_set.clone(),
        block_height: proof.height,
        block_hash: proof.block_hash,
        mmr_root,
        mmr_leaf_index: mmr.leaves().checked_sub(1),
        mmr_peaks: Some(mmr.peaks.iter().map(|p| p.hash).collect()),
        next_authority_set: None,
    };
    let justification = BridgeCommitmentJustification {
        signatures: Vec::new(),
    };
    Ok(BridgeFinalityBundle {
        commitment,
        justification,
        block_header: proof.block_header,
        commit_qc: proof.commit_qc,
    })
}

/// Verification errors raised when checking a [`BridgeFinalityProof`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BridgeFinalityVerificationError {
    /// The proof carries a different chain id than expected.
    #[error("finality proof chain_id mismatch: expected {expected}, actual {actual}")]
    ChainIdMismatch {
        /// Chain id the verifier expects.
        expected: ChainId,
        /// Chain id carried inside the proof.
        actual: ChainId,
    },
    /// The caller expected a different height than the proof advertises.
    #[error("finality proof height mismatch: expected {expected}, proof {actual}")]
    HeightMismatch {
        /// Height the verifier expected.
        expected: u64,
        /// Height carried in the proof.
        actual: u64,
    },
    /// The block header height disagrees with the proof height.
    #[error("block header height {header_height} does not match proof height {proof_height}")]
    BlockHeaderHeightMismatch {
        /// Height in the proof.
        proof_height: u64,
        /// Height carried in the block header.
        header_height: u64,
    },
    /// The commit certificate height disagrees with the proof height.
    #[error("commit certificate height {cert_height} does not match proof height {proof_height}")]
    QcHeightMismatch {
        /// Height in the proof.
        proof_height: u64,
        /// Height carried in the commit certificate.
        cert_height: u64,
    },
    /// Commit certificate phase is not `Commit`.
    #[error("unexpected commit certificate phase {actual:?}")]
    UnexpectedCertificatePhase {
        /// Phase carried in the commit certificate.
        actual: sumeragi::consensus::Phase,
    },
    /// Recomputed block hash does not match the proof/certificate payloads.
    #[error(
        "block hash mismatch: header {header_hash:?}, proof {proof_hash:?}, certificate {certificate_hash:?}"
    )]
    BlockHashMismatch {
        /// Hash recomputed from the block header.
        header_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
        /// Hash carried in the proof.
        proof_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
        /// Hash advertised inside the commit certificate.
        certificate_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
    },
    /// Validator set hash version advertised by the certificate is unsupported.
    #[error("unsupported validator_set_hash_version {version}")]
    UnsupportedValidatorSetHashVersion {
        /// Unsupported version encountered.
        version: u16,
    },
    /// Recomputed validator set hash does not match the certificate payload.
    #[error("validator_set_hash mismatch: computed {computed:?}, advertised {advertised:?}")]
    ValidatorSetHashMismatch {
        /// Hash recomputed from the validator set.
        computed: HashOf<Vec<PeerId>>,
        /// Hash advertised in the commit certificate.
        advertised: HashOf<Vec<PeerId>>,
    },
    /// The verifier pinned a validator set hash that does not match the proof.
    #[error("trusted validator_set_hash mismatch: trusted {trusted:?}, certificate {advertised:?}")]
    TrustedValidatorSetHashMismatch {
        /// Trusted validator set hash supplied by the verifier.
        trusted: HashOf<Vec<PeerId>>,
        /// Hash recomputed from the proof validator set.
        advertised: HashOf<Vec<PeerId>>,
    },
    /// Validator set is empty, so no quorum can be reached.
    #[error("validator set is empty")]
    EmptyValidatorSet,
    /// Validator-set `PoP` length does not match the validator-set length.
    #[error("validator set pop length mismatch: expected {expected}, got {actual}")]
    ValidatorSetPopLengthMismatch {
        /// Expected `PoP` count.
        expected: usize,
        /// Actual `PoP` count.
        actual: usize,
    },
    /// Signer bitmap length does not match the validator set size.
    #[error("signer bitmap length mismatch: expected {expected}, got {actual}")]
    SignerBitmapLengthMismatch {
        /// Expected bitmap length in bytes.
        expected: usize,
        /// Actual bitmap length in bytes.
        actual: usize,
    },
    /// A signer index falls outside the validator set bounds.
    #[error("signer index {signer} is out of bounds for roster length {roster_len}")]
    SignerOutOfBounds {
        /// Offending signer index.
        signer: u64,
        /// Length of the validator set.
        roster_len: usize,
    },
    /// Duplicate signer index detected inside the commit certificate.
    #[error("duplicate signer index {signer} in commit certificate signatures")]
    DuplicateSigner {
        /// Signer index that appears multiple times.
        signer: u64,
    },
    /// Quorum was not met when counting unique signatures.
    #[error("insufficient signatures: collected {collected}, required {required}")]
    InsufficientSignatures {
        /// Unique signatures collected.
        collected: usize,
        /// Required quorum.
        required: usize,
    },
    /// Commit certificate carries no aggregate signature.
    #[error("commit certificate aggregate signature is missing")]
    AggregateSignatureMissing,
    /// Commit certificate aggregate signature failed verification.
    #[error("commit certificate aggregate signature is invalid")]
    AggregateSignatureInvalid,
}

/// Verification knobs for [`verify_finality_proof`].
#[derive(Debug, Clone)]
pub struct FinalityProofVerificationConfig<'a> {
    /// Chain identifier expected by the verifier.
    pub expected_chain_id: &'a ChainId,
    /// Optional expected height to bind the proof to a specific block.
    pub expected_height: Option<u64>,
    /// Optional trusted validator set hash anchor to guard against roster replays.
    pub trusted_validator_set_hash: Option<HashOf<Vec<PeerId>>>,
}

const fn min_votes_for_len(len: usize) -> usize {
    if len > 3 {
        ((len.saturating_sub(1)) / 3) * 2 + 1
    } else {
        len
    }
}

/// Verify a [`BridgeFinalityProof`] against chain/height/validator set expectations.
///
/// Callers supply the expected chain id and may optionally bind the proof to a specific
/// height and validator set hash. Verification recomputes the block hash, enforces
/// validator set hashing rules, and checks signatures for quorum and validity.
///
/// # Errors
/// Returns [`BridgeFinalityVerificationError`] when the proof fails chain/height checks,
/// validator set hashing/anchors, or signature validation.
#[allow(clippy::too_many_lines)]
pub fn verify_finality_proof(
    proof: &BridgeFinalityProof,
    config: &FinalityProofVerificationConfig<'_>,
) -> Result<(), BridgeFinalityVerificationError> {
    if proof.chain_id != *config.expected_chain_id {
        return Err(BridgeFinalityVerificationError::ChainIdMismatch {
            expected: config.expected_chain_id.clone(),
            actual: proof.chain_id.clone(),
        });
    }

    if let Some(expected_height) = config.expected_height {
        if proof.height != expected_height {
            return Err(BridgeFinalityVerificationError::HeightMismatch {
                expected: expected_height,
                actual: proof.height,
            });
        }
    }

    let header_height = proof.block_header.height().get();
    if header_height != proof.height {
        return Err(BridgeFinalityVerificationError::BlockHeaderHeightMismatch {
            proof_height: proof.height,
            header_height,
        });
    }

    let certificate = &proof.commit_qc;
    if certificate.height != proof.height {
        return Err(BridgeFinalityVerificationError::QcHeightMismatch {
            proof_height: proof.height,
            cert_height: certificate.height,
        });
    }

    if certificate.phase != sumeragi::consensus::Phase::Commit {
        return Err(
            BridgeFinalityVerificationError::UnexpectedCertificatePhase {
                actual: certificate.phase,
            },
        );
    }

    let header_hash = proof.block_header.hash();
    if header_hash != proof.block_hash || header_hash != certificate.subject_block_hash {
        return Err(BridgeFinalityVerificationError::BlockHashMismatch {
            header_hash,
            proof_hash: proof.block_hash,
            certificate_hash: certificate.subject_block_hash,
        });
    }

    if certificate.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1 {
        return Err(
            BridgeFinalityVerificationError::UnsupportedValidatorSetHashVersion {
                version: certificate.validator_set_hash_version,
            },
        );
    }

    let computed_set_hash = HashOf::new(&certificate.validator_set);
    if computed_set_hash != certificate.validator_set_hash {
        return Err(BridgeFinalityVerificationError::ValidatorSetHashMismatch {
            computed: computed_set_hash,
            advertised: certificate.validator_set_hash,
        });
    }

    if let Some(trusted) = config.trusted_validator_set_hash {
        if trusted != computed_set_hash {
            return Err(
                BridgeFinalityVerificationError::TrustedValidatorSetHashMismatch {
                    trusted,
                    advertised: computed_set_hash,
                },
            );
        }
    }

    let roster_len = certificate.validator_set.len();
    if roster_len == 0 {
        return Err(BridgeFinalityVerificationError::EmptyValidatorSet);
    }
    if proof.validator_set_pops.len() != roster_len {
        return Err(
            BridgeFinalityVerificationError::ValidatorSetPopLengthMismatch {
                expected: roster_len,
                actual: proof.validator_set_pops.len(),
            },
        );
    }
    let expected_bitmap_len = roster_len.div_ceil(8);
    if certificate.aggregate.signers_bitmap.len() != expected_bitmap_len {
        return Err(
            BridgeFinalityVerificationError::SignerBitmapLengthMismatch {
                expected: expected_bitmap_len,
                actual: certificate.aggregate.signers_bitmap.len(),
            },
        );
    }
    let required = min_votes_for_len(roster_len);
    let mut seen = BTreeSet::new();
    for (byte_idx, byte) in certificate.aggregate.signers_bitmap.iter().enumerate() {
        if *byte == 0 {
            continue;
        }
        for bit in 0..8 {
            if (byte >> bit) & 1 == 0 {
                continue;
            }
            let idx = byte_idx * 8 + bit;
            let signer = u64::try_from(idx).unwrap_or(u64::MAX);
            if idx >= roster_len {
                return Err(BridgeFinalityVerificationError::SignerOutOfBounds {
                    signer,
                    roster_len,
                });
            }
            if !seen.insert(signer) {
                return Err(BridgeFinalityVerificationError::DuplicateSigner { signer });
            }
        }
    }

    if certificate.aggregate.bls_aggregate_signature.is_empty() {
        return Err(BridgeFinalityVerificationError::AggregateSignatureMissing);
    }

    let collected = seen.len();
    if collected < required {
        return Err(BridgeFinalityVerificationError::InsufficientSignatures {
            collected,
            required,
        });
    }

    let vote = sumeragi::consensus::Vote {
        phase: certificate.phase,
        block_hash: certificate.subject_block_hash,
        parent_state_root: certificate.parent_state_root,
        post_state_root: certificate.post_state_root,
        height: certificate.height,
        view: certificate.view,
        epoch: certificate.epoch,
        chain_order_hash: certificate.chain_order_hash,
        rechain_seq: certificate.rechain_seq,
        highest_qc: None,
        signer: 0,
        bls_sig: Vec::new(),
    };
    let preimage =
        sumeragi::consensus::vote_preimage(config.expected_chain_id, &certificate.mode_tag, &vote);
    let mut public_keys: Vec<&iroha_crypto::PublicKey> = Vec::with_capacity(seen.len());
    let mut pops: Vec<&[u8]> = Vec::with_capacity(seen.len());
    for signer in &seen {
        let idx = usize::try_from(*signer).map_err(|_| {
            BridgeFinalityVerificationError::SignerOutOfBounds {
                signer: *signer,
                roster_len,
            }
        })?;
        let Some(peer) = certificate.validator_set.get(idx) else {
            return Err(BridgeFinalityVerificationError::SignerOutOfBounds {
                signer: *signer,
                roster_len,
            });
        };
        public_keys.push(peer.public_key());
        pops.push(proof.validator_set_pops[idx].as_slice());
    }
    if iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &certificate.aggregate.bls_aggregate_signature,
        &public_keys,
        &pops,
    )
    .is_err()
    {
        return Err(BridgeFinalityVerificationError::AggregateSignatureInvalid);
    }

    Ok(())
}

fn sccp_block_hash_from_h256(hash: [u8; 32]) -> HashOf<BlockHeader> {
    HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(hash))
}

fn sccp_block_hash_to_h256(hash: &HashOf<BlockHeader>) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(hash.as_ref().as_ref());
    out
}

fn sccp_hash_to_h256(hash: &Hash) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(hash.as_ref());
    out
}

fn sccp_consensus_phase(phase: sumeragi::consensus::Phase) -> NexusConsensusPhaseV1 {
    match phase {
        sumeragi::consensus::Phase::Prepare => NexusConsensusPhaseV1::Prepare,
        sumeragi::consensus::Phase::Commit => NexusConsensusPhaseV1::Commit,
        sumeragi::consensus::Phase::NewView => NexusConsensusPhaseV1::NewView,
    }
}

fn sccp_qc_ref(reference: &iroha_data_model::block::consensus::QcRef) -> NexusQcRefV1 {
    NexusQcRefV1 {
        height: reference.height,
        view: reference.view,
        epoch: reference.epoch,
        subject_block_hash: sccp_block_hash_to_h256(&reference.subject_block_hash),
        phase: sccp_consensus_phase(reference.phase),
    }
}

fn sccp_qc_projection_matches_local(
    finality: &NexusBridgeFinalityProofV1,
    trusted_qc: &Qc,
) -> bool {
    let qc = &finality.commit_qc;
    qc.version == 1
        && qc.phase == NexusConsensusPhaseV1::Commit
        && trusted_qc.phase == sumeragi::consensus::Phase::Commit
        && qc.height == trusted_qc.height
        && qc.view == trusted_qc.view
        && qc.epoch == trusted_qc.epoch
        && qc.mode_tag == trusted_qc.mode_tag
        && qc.subject_block_hash == sccp_block_hash_to_h256(&trusted_qc.subject_block_hash)
        && qc.parent_state_root == sccp_hash_to_h256(&trusted_qc.parent_state_root)
        && qc.post_state_root == sccp_hash_to_h256(&trusted_qc.post_state_root)
        && qc.chain_order_hash == sccp_hash_to_h256(&trusted_qc.chain_order_hash)
        && qc.rechain_seq == trusted_qc.rechain_seq
        && qc.highest_qc == trusted_qc.highest_qc.as_ref().map(sccp_qc_ref)
        && qc.validator_set_hash_version == trusted_qc.validator_set_hash_version
        && qc.validator_public_keys
            == trusted_qc
                .validator_set
                .iter()
                .map(|peer| peer.public_key().to_string())
                .collect::<Vec<_>>()
        && qc.validator_set_pops.len() == trusted_qc.validator_set.len()
        && qc.signers_bitmap == trusted_qc.aggregate.signers_bitmap
        && qc.bls_aggregate_signature == trusted_qc.aggregate.bls_aggregate_signature
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
            "SCCP finality proof local block contains duplicate outbound message source_domain={} target_domain={} message_id={}",
            key.source_domain,
            key.target_domain,
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

/// Verify an SCCP finality proof against local committed blocks and trusted commit-roster data.
///
/// This intentionally rejects proofs when the local node cannot load the committed block or
/// trusted commit QC for the referenced height.
///
/// # Errors
/// Returns a human-readable rejection reason when the SCCP proof is not anchored to local state
/// or when the trusted local QC fails full finality verification.
#[allow(clippy::too_many_lines)]
pub fn verify_sccp_finality_proof_against_local_state(
    state: &impl SccpFinalityStateReadOnly,
    finality: &NexusBridgeFinalityProofV1,
) -> Result<BridgeFinalityProof, String> {
    if !iroha_sccp::verify_nexus_bridge_finality_proof_structure(finality) {
        return Err("SCCP finality proof failed structural verification".to_owned());
    }
    if !iroha_sccp::verify_nexus_bridge_finality_proof_cryptographic(finality) {
        return Err("SCCP finality proof failed cryptographic verification".to_owned());
    }
    if finality.chain_id != state.sccp_chain_id().as_str() {
        return Err(format!(
            "SCCP finality proof chain_id mismatch: expected {}, actual {}",
            state.sccp_chain_id(),
            finality.chain_id
        ));
    }

    let height_usize: usize = finality
        .height
        .try_into()
        .map_err(|_| format!("invalid SCCP finality height {}", finality.height))?;
    let nonzero_height = NonZeroUsize::new(height_usize)
        .ok_or_else(|| format!("invalid SCCP finality height {}", finality.height))?;
    let local_block = state
        .sccp_block_by_height(nonzero_height)
        .ok_or_else(|| format!("local committed block {} not found", finality.height))?;
    let local_header = local_block.header();
    let local_hash = local_block.hash();
    let proof_hash = sccp_block_hash_from_h256(finality.block_hash);
    if proof_hash != local_hash {
        return Err(
            "SCCP finality proof block hash does not match local committed block".to_owned(),
        );
    }

    let local_header_bytes = norito::to_bytes(&local_header)
        .map_err(|err| format!("failed to encode local block header: {err}"))?;
    if local_header_bytes != finality.block_header_bytes {
        return Err(
            "SCCP finality proof block header bytes do not match local committed block".to_owned(),
        );
    }
    if local_header.sccp_commitment_root() != Some(finality.commitment_root) {
        return Err(
            "SCCP finality proof commitment root does not match local block header".to_owned(),
        );
    }

    validate_local_sccp_records_against_commitment_root(
        local_block.as_ref(),
        finality.commitment_root,
    )?;

    let trusted_qc = state
        .sccp_commit_qc_for_block(finality.height, local_hash)
        .ok_or_else(|| {
            format!(
                "trusted local commit QC for SCCP proof height {} is missing",
                finality.height
            )
        })?;
    if !sccp_qc_projection_matches_local(finality, &trusted_qc) {
        return Err(
            "SCCP finality QC projection does not match the trusted local commit QC".to_owned(),
        );
    }

    let proof = BridgeFinalityProof {
        height: finality.height,
        chain_id: state.sccp_chain_id().clone(),
        block_header: local_header,
        block_hash: local_hash,
        commit_qc: trusted_qc,
        validator_set_pops: finality.commit_qc.validator_set_pops.clone(),
    };
    verify_finality_proof(
        &proof,
        &FinalityProofVerificationConfig {
            expected_chain_id: state.sccp_chain_id(),
            expected_height: Some(finality.height),
            trusted_validator_set_hash: Some(proof.commit_qc.validator_set_hash),
        },
    )
    .map_err(|err| format!("trusted local finality QC failed verification: {err}"))?;
    Ok(proof)
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, num::NonZeroU64, sync::Arc};

    use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        block::{BlockSignature, SignedBlock},
        consensus::{CertPhase, QcAggregate, default_chain_order_hash},
        isi::InstructionBox,
        nexus::DataSpaceId,
        peer::PeerId,
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

    #[test]
    fn checked_keypair_helpers_preserve_requested_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
        assert_eq!(checked_bls_keypair().algorithm(), Algorithm::BlsNormal);
    }

    struct EmptySccpFinalityState {
        chain_id: ChainId,
    }

    struct PersistedQcBridgeState {
        chain_id: ChainId,
        block: Arc<SignedBlock>,
        commit_qc: Qc,
        validator_public_key: PublicKey,
        validator_pop: Vec<u8>,
    }

    impl BridgeStateReadOnly for PersistedQcBridgeState {
        fn bridge_chain_id(&self) -> &ChainId {
            &self.chain_id
        }

        fn bridge_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
            (u64::try_from(height.get()).ok() == Some(self.block.header().height().get()))
                .then(|| self.block.clone())
        }

        fn bridge_commit_qc_for_block(
            &self,
            height: u64,
            block_hash: HashOf<BlockHeader>,
        ) -> Option<Qc> {
            (self.commit_qc.height == height && self.commit_qc.subject_block_hash == block_hash)
                .then(|| self.commit_qc.clone())
        }

        fn bridge_validator_pop(&self, public_key: &PublicKey) -> Option<Vec<u8>> {
            (public_key == &self.validator_public_key).then(|| self.validator_pop.clone())
        }
    }

    impl SccpFinalityStateReadOnly for EmptySccpFinalityState {
        fn sccp_chain_id(&self) -> &ChainId {
            &self.chain_id
        }

        fn sccp_block_by_height(&self, _height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
            None
        }

        fn sccp_commit_qc_for_block(
            &self,
            _height: u64,
            _block_hash: HashOf<BlockHeader>,
        ) -> Option<Qc> {
            None
        }
    }

    fn sample_transfer_payload(nonce: u64, recipient: &[u8]) -> SccpPayloadV1 {
        SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
            version: 1,
            source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            nonce,
            asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            asset_id_codec: 1,
            asset_id: b"xor".to_vec(),
            amount: 77,
            sender_codec: 1,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: iroha_sccp::SCCP_CODEC_EVM_HEX,
            recipient: recipient.to_vec(),
            route_id_codec: 1,
            route_id: b"nexus:eth:xor".to_vec(),
        })
    }

    fn sample_route_activate_payload(nonce: u64, route_id: &[u8]) -> SccpPayloadV1 {
        SccpPayloadV1::RouteActivate(iroha_sccp::RouteActivatePayloadV1 {
            version: 1,
            source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            target_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            nonce,
            asset_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor".to_vec(),
            route_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
            route_id: route_id.to_vec(),
        })
    }

    fn non_sora_source_transfer_payload(nonce: u64) -> SccpPayloadV1 {
        SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
            version: 1,
            source_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            dest_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            nonce,
            asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            asset_id_codec: 1,
            asset_id: b"xor".to_vec(),
            amount: 77,
            sender_codec: iroha_sccp::SCCP_CODEC_EVM_HEX,
            sender: b"0x0000000000000000000000000000000000000555".to_vec(),
            recipient_codec: 1,
            recipient: b"sora:recipient".to_vec(),
            route_id_codec: 1,
            route_id: b"nexus:eth:xor".to_vec(),
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
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload,
            )),
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
                iroha_data_model::isi::bridge::RecordSccpMessage::new(payload),
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

    #[test]
    fn build_finality_proof_uses_persisted_qc_when_status_history_misses_height() {
        let height = 987_654;
        let chain_id: ChainId = "bridge-sccp-persisted-qc".parse().expect("chain id");
        let block = Arc::new(signed_block_with_transactions(Vec::new(), height));
        let block_hash = block.hash();
        let validator_keypair = checked_bls_keypair();
        let validator_public_key = validator_keypair.public_key().clone();
        let validator_set = vec![PeerId::new(validator_public_key.clone())];
        let commit_qc = Qc {
            phase: CertPhase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: Hash::new(b"persisted-qc-parent"),
            post_state_root: Hash::new(b"persisted-qc-post"),
            height,
            view: 3,
            epoch: 0,
            chain_order_hash: default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: iroha_data_model::block::consensus::PERMISSIONED_TAG.to_owned(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![0x01],
                bls_aggregate_signature: vec![0xAA; 96],
            },
        };
        let validator_pop = vec![0x42; 48];
        let state = PersistedQcBridgeState {
            chain_id,
            block,
            commit_qc: commit_qc.clone(),
            validator_public_key,
            validator_pop: validator_pop.clone(),
        };

        let proof = build_finality_proof(&state, height)
            .expect("persisted commit QC should satisfy finality proof build");

        assert_eq!(proof.commit_qc, commit_qc);
        assert_eq!(proof.validator_set_pops, vec![validator_pop]);
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
            .map(iroha_data_model::isi::bridge::RecordSccpMessage::new)
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

    #[test]
    fn sccp_finality_local_state_check_rejects_unsigned_qc_before_state_lookup() {
        let chain_id: ChainId = iroha_sccp::SCCP_NEXUS_FINALITY_CHAIN_ID_V1
            .parse()
            .expect("chain id");
        let validator_keypair = checked_bls_keypair();
        let validator_public_keys = vec![validator_keypair.public_key().to_string()];
        let validator_set = vec![PeerId::new(validator_keypair.public_key().clone())];
        let validator_set_hash = HashOf::new(&validator_set);
        let mut validator_set_hash_bytes = [0u8; 32];
        validator_set_hash_bytes.copy_from_slice(validator_set_hash.as_ref().as_ref());
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            7,
            b"0x0000000000000000000000000000000000000007",
        ));
        let (block, _) = signed_block_with_sccp_payloads(&[payload], 7);
        let messages = collect_sccp_messages_from_signed_block(&block);
        let commitment_root =
            sccp_commitment_root_from_messages(&messages).expect("commitment root");
        let mut block_header = block.header().clone();
        block_header.set_sccp_commitment_root(Some(commitment_root));
        let block_hash = sccp_block_hash_to_h256(&block_header.hash());
        let block_header_bytes = norito::to_bytes(&block_header).expect("encode block header");
        let finality = NexusBridgeFinalityProofV1 {
            version: 1,
            chain_id: chain_id.to_string(),
            height: 7,
            block_hash,
            commitment_root,
            block_header_bytes,
            commit_qc: iroha_sccp::NexusCommitQcV1 {
                version: 1,
                phase: NexusConsensusPhaseV1::Commit,
                height: 7,
                view: 0,
                epoch: 0,
                mode_tag: "iroha2-consensus::permissioned-sumeragi@v2".to_owned(),
                subject_block_hash: block_hash,
                parent_state_root: [1; 32],
                post_state_root: [2; 32],
                chain_order_hash: [3; 32],
                rechain_seq: 0,
                highest_qc: None,
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: validator_set_hash_bytes,
                validator_public_keys,
                validator_set_pops: vec![vec![1; 48]],
                signers_bitmap: vec![0b0000_0001],
                bls_aggregate_signature: vec![2; 96],
            },
        };
        assert!(iroha_sccp::verify_nexus_bridge_finality_proof_structure(
            &finality
        ));
        assert!(!iroha_sccp::verify_nexus_bridge_finality_proof_cryptographic(&finality));

        let state = EmptySccpFinalityState { chain_id };
        let err = verify_sccp_finality_proof_against_local_state(&state, &finality)
            .expect_err("unsigned SCCP finality must be rejected before local anchoring");
        assert_eq!(err, "SCCP finality proof failed cryptographic verification");
    }

    #[test]
    fn sccp_commitment_root_is_none_for_empty_messages() {
        assert_eq!(sccp_commitment_root_from_messages(&[]), None);
    }

    #[test]
    fn sccp_commitment_root_matches_direct_merkle_root() {
        let payloads = vec![
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                1,
                b"0x0000000000000000000000000000000000000001",
            )),
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                2,
                b"0x0000000000000000000000000000000000000002",
            )),
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
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            15,
            b"0x0000000000000000000000000000000000000999",
        ));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload.clone(),
            )),
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
    fn collect_sccp_messages_from_plain_instruction_executable_is_empty() {
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            12,
            b"0x0000000000000000000000000000000000000012",
        ));
        let tx = signed_transaction_with_executable(Executable::Instructions(
            vec![InstructionBox::from(
                iroha_data_model::isi::bridge::RecordSccpMessage::new(payload),
            )]
            .into(),
        ));
        let block = signed_block_with_transactions(vec![tx], 1);

        assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
    }

    #[test]
    fn collect_sccp_messages_from_block_preserves_payload_order() {
        let payloads = vec![
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                1,
                b"0x0000000000000000000000000000000000000001",
            )),
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                2,
                b"0x0000000000000000000000000000000000000002",
            )),
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
        let expected_payload =
            sample_transfer_payload(6, b"0x0000000000000000000000000000000000000006");
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&expected_payload);
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
        let expected_payload =
            sample_transfer_payload(7, b"0x0000000000000000000000000000000000000007");
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&expected_payload);
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
        let expected_payload =
            sample_transfer_payload(8, b"0x0000000000000000000000000000000000000008");
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&expected_payload);
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
        let accepted_payload =
            sample_transfer_payload(9, b"0x0000000000000000000000000000000000000009");
        let accepted_bytes = iroha_sccp::canonical_sccp_payload_bytes(&accepted_payload);

        let rejected_payload =
            sample_transfer_payload(10, b"0x0000000000000000000000000000000000000010");
        let rejected_bytes = iroha_sccp::canonical_sccp_payload_bytes(&rejected_payload);
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

        let expected_commitment = iroha_sccp::hub_commitment_from_sccp_payload(&accepted_payload);
        assert_eq!(
            sccp_commitment_root_from_messages(&messages),
            iroha_sccp::commitment_merkle_root(&[expected_commitment])
        );
    }

    #[test]
    fn collect_sccp_messages_skips_undecodable_payloads() {
        let payloads = vec![
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                3,
                b"0x0000000000000000000000000000000000000003",
            )),
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
            asset_home_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            asset_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
            asset_id: b"weth#eth".to_vec(),
            amount: 10,
            sender_codec: iroha_sccp::SCCP_CODEC_EVM_HEX,
            sender: b"0x1111111111111111111111111111111111111111".to_vec(),
            recipient_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
            recipient: b"alice@universal".to_vec(),
            route_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
            route_id: b"eth:sora:weth".to_vec(),
        });
        let (block, _) = signed_block_with_sccp_payloads(
            &[iroha_sccp::canonical_sccp_payload_bytes(&inbound)],
            2,
        );

        assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
    }

    #[test]
    fn collect_sccp_messages_skips_decodable_but_invalid_payloads() {
        let invalid = sample_transfer_payload(4, b"0x0000000000000000000000000000000000000004");
        let SccpPayloadV1::Transfer(mut invalid_transfer) = invalid else {
            panic!("sample payload should be a transfer");
        };
        invalid_transfer.amount = 0;
        let invalid_payload = SccpPayloadV1::Transfer(invalid_transfer);
        assert!(
            iroha_sccp::decode_canonical_sccp_payload_bytes(
                &iroha_sccp::canonical_sccp_payload_bytes(&invalid_payload)
            )
            .is_some()
        );
        assert!(!iroha_sccp::verify_sccp_payload_structure(&invalid_payload));
        let valid_payload =
            sample_transfer_payload(5, b"0x0000000000000000000000000000000000000005");
        let payloads = vec![
            iroha_sccp::canonical_sccp_payload_bytes(&invalid_payload),
            iroha_sccp::canonical_sccp_payload_bytes(&valid_payload),
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
            payload: None,
        }));
        let block = signed_block_with_transactions(vec![tx], 4);

        assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
    }

    #[test]
    fn collect_sccp_messages_preserves_instruction_indices_after_skips() {
        let payloads = vec![
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                4,
                b"0x0000000000000000000000000000000000000004",
            )),
            vec![0x00, 0x01, 0xff],
            iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
                5,
                b"0x0000000000000000000000000000000000000005",
            )),
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
        let first_payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            6,
            b"0x0000000000000000000000000000000000000006",
        ));
        let second_payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            7,
            b"0x0000000000000000000000000000000000000007",
        ));
        let ignored_tx =
            signed_transaction_with_executable(Executable::Ivm(IvmBytecode::from_compiled(vec![
                0xAA,
            ])));
        let first_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                first_payload,
            )),
        ]));
        let second_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                second_payload,
            )),
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

        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            6,
            b"0x0000000000000000000000000000000000000008",
        ));
        let external_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload.clone(),
            )),
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
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            7,
            b"0x0000000000000000000000000000000000000991",
        ));
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
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            8,
            b"0x0000000000000000000000000000000000000222",
        ));
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
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            8,
            b"0x0000000000000000000000000000000000000224",
        ));
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
        let skipped_payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            8,
            b"0x0000000000000000000000000000000000000225",
        ));
        let included_payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            9,
            b"0x0000000000000000000000000000000000000226",
        ));
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
    fn collect_sccp_messages_from_accepted_transactions_skips_unbound_outbound_route() {
        let mut payload =
            sample_transfer_payload(12, b"0x0000000000000000000000000000000000000227");
        let SccpPayloadV1::Transfer(transfer) = &mut payload else {
            unreachable!("sample payload is a transfer");
        };
        transfer.route_id = b"nexus:bsc:xor".to_vec();
        let accepted = accepted_transaction_with_sccp_payload(
            iroha_sccp::canonical_sccp_payload_bytes(&payload),
        );

        let messages = collect_sccp_messages_from_accepted_transactions(&[accepted]);

        assert!(
            messages.is_empty(),
            "proposal SCCP roots must not include records that execution will reject"
        );
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_skips_unbound_route_activation() {
        let payload = sample_route_activate_payload(21, b"nexus:bsc:xor");
        let accepted = accepted_transaction_with_sccp_payload(
            iroha_sccp::canonical_sccp_payload_bytes(&payload),
        );

        let messages = collect_sccp_messages_from_accepted_transactions(&[accepted]);

        assert!(
            messages.is_empty(),
            "proposal SCCP roots must not include route activations that execution will reject"
        );
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_skips_malformed_outbound_asset_scope() {
        let mut payload =
            sample_transfer_payload(23, b"0x0000000000000000000000000000000000000229");
        let SccpPayloadV1::Transfer(transfer) = &mut payload else {
            unreachable!("sample payload is a transfer");
        };
        transfer.asset_id = b"xor#".to_vec();
        transfer.route_id = b"nexus:eth:xor".to_vec();
        let accepted = accepted_transaction_with_sccp_payload(
            iroha_sccp::canonical_sccp_payload_bytes(&payload),
        );

        let messages = collect_sccp_messages_from_accepted_transactions(&[accepted]);

        assert!(
            messages.is_empty(),
            "proposal SCCP roots must not include asset-id aliases with empty scopes"
        );
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_skips_scoped_outbound_asset_alias() {
        let mut payload =
            sample_transfer_payload(25, b"0x0000000000000000000000000000000000000230");
        let SccpPayloadV1::Transfer(transfer) = &mut payload else {
            unreachable!("sample payload is a transfer");
        };
        transfer.asset_id = b"xor#universal".to_vec();
        transfer.route_id = b"nexus:eth:xor".to_vec();
        let accepted = accepted_transaction_with_sccp_payload(
            iroha_sccp::canonical_sccp_payload_bytes(&payload),
        );

        let messages = collect_sccp_messages_from_accepted_transactions(&[accepted]);

        assert!(
            messages.is_empty(),
            "proposal SCCP roots must not include scoped asset-id aliases"
        );
    }

    #[test]
    fn collect_sccp_messages_from_accepted_transactions_deduplicates_same_overlay_key() {
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            8,
            b"0x0000000000000000000000000000000000000223",
        ));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload.clone(),
            )),
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload.clone(),
            )),
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
        let payload = sample_transfer_payload(9, b"0x0000000000000000000000000000000000000333");
        let key = sccp_outbound_message_key(&payload);
        let accepted = accepted_transaction_with_sccp_payload(
            iroha_sccp::canonical_sccp_payload_bytes(&payload),
        );

        let messages =
            collect_new_sccp_messages_from_accepted_transactions(&[accepted], |candidate| {
                candidate == &key
            });

        assert!(messages.is_empty());
    }

    #[test]
    fn collect_sccp_messages_from_block_deduplicates_successful_duplicate_outbound_keys() {
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            10,
            b"0x0000000000000000000000000000000000000443",
        ));
        let first_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload.clone(),
            )),
        ]));
        let second_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload.clone(),
            )),
        ]));
        let block = signed_block_with_transactions(vec![first_tx, second_tx], 9);

        let messages = collect_sccp_messages_from_signed_block(&block);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tx_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
        );
        let expected_commitment =
            iroha_sccp::hub_commitment_from_sccp_payload(&messages[0].payload);
        assert_eq!(
            sccp_commitment_root_from_messages(&messages),
            iroha_sccp::commitment_merkle_root(&[expected_commitment])
        );
    }

    #[test]
    fn collect_sccp_messages_from_block_ignores_hex_aliases() {
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            11,
            b"0x0000000000000000000000000000000000000445",
        ));
        let encoded_payload = format!("0x{}", hex::encode(&payload)).into_bytes();
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                encoded_payload,
            )),
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload.clone(),
            )),
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
        let payload = sample_transfer_payload(12, b"0x0000000000000000000000000000000000000446");
        let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(&payload);
        let first_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload_bytes.clone(),
            )),
        ]));
        let second_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload_bytes,
            )),
        ]));
        let block = signed_block_with_transactions(vec![first_tx, second_tx], 9);
        let messages = collect_sccp_messages_from_signed_block(&block);
        let deduped_root =
            sccp_commitment_root_from_messages(&messages).expect("deduped commitment root");

        let err = validate_local_sccp_records_against_commitment_root(&block, deduped_root)
            .expect_err("duplicate successful SCCP records must reject before root acceptance");

        assert!(err.contains("duplicate outbound message"));
        assert!(err.contains(&hex::encode(sccp_outbound_message_key(&payload).message_id)));
    }

    #[test]
    fn local_sccp_finality_records_reject_hex_alias_payload() {
        let payload = sample_transfer_payload(13, b"0x0000000000000000000000000000000000000447");
        let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(&payload);
        let encoded_payload = format!("0x{}", hex::encode(&payload_bytes)).into_bytes();
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                encoded_payload,
            )),
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload_bytes,
            )),
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
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            14,
            b"0x0000000000000000000000000000000000000448",
        ));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload,
            )),
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
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            16,
            b"0x0000000000000000000000000000000000000450",
        ));
        let sccp_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload,
            )),
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
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
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
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            15,
            b"0x0000000000000000000000000000000000000449",
        ));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
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
        let payload = sample_transfer_payload(20, b"0x0000000000000000000000000000000000000453");
        let SccpPayloadV1::Transfer(transfer) = payload else {
            unreachable!("sample payload is a transfer");
        };
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                iroha_sccp::canonical_transfer_payload_bytes(&transfer),
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
        let payload =
            iroha_sccp::canonical_sccp_payload_bytes(&non_sora_source_transfer_payload(18));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload,
            )),
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
    fn validate_sccp_commitment_root_for_signed_block_rejects_unbound_outbound_route() {
        let mut payload =
            sample_transfer_payload(17, b"0x0000000000000000000000000000000000000451");
        let SccpPayloadV1::Transfer(transfer) = &mut payload else {
            unreachable!("sample payload is a transfer");
        };
        transfer.route_id = b"nexus:bsc:xor".to_vec();
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&payload);
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload,
            )),
        ]));
        let block = signed_block_with_transactions(vec![tx], 9);

        let err = validate_sccp_commitment_root_for_signed_block(&block)
            .expect_err("successful invalid outbound SCCP route must reject");

        assert_eq!(
            err,
            SccpCommittedBlockValidationError::InvalidRecordInstruction(
                SccpRecordInstructionValidationError::RouteBinding {
                    tx_index: 0,
                    instruction_index: 0,
                    error: SccpOutboundRouteValidationError::RouteIdMismatch {
                        actual: "nexus:bsc:xor".to_owned(),
                        expected: "nexus:eth:xor".to_owned(),
                    },
                }
            )
        );
    }

    #[test]
    fn validate_sccp_commitment_root_for_signed_block_rejects_unbound_route_activation() {
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_route_activate_payload(
            22,
            b"nexus:bsc:xor",
        ));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload,
            )),
        ]));
        let block = signed_block_with_transactions(vec![tx], 9);

        let err = validate_sccp_commitment_root_for_signed_block(&block)
            .expect_err("successful invalid outbound SCCP route activation must reject");

        assert_eq!(
            err,
            SccpCommittedBlockValidationError::InvalidRecordInstruction(
                SccpRecordInstructionValidationError::RouteBinding {
                    tx_index: 0,
                    instruction_index: 0,
                    error: SccpOutboundRouteValidationError::RouteIdMismatch {
                        actual: "nexus:bsc:xor".to_owned(),
                        expected: "nexus:eth:xor".to_owned(),
                    },
                }
            )
        );
    }

    #[test]
    fn validate_sccp_commitment_root_for_signed_block_rejects_ambiguous_asset_scope() {
        let mut payload = sample_route_activate_payload(24, b"nexus:eth:xor");
        let SccpPayloadV1::RouteActivate(activation) = &mut payload else {
            unreachable!("sample payload is a route activation");
        };
        activation.asset_id = b"xor#universal#shadow".to_vec();
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&payload);
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload,
            )),
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
        let mut payload = sample_route_activate_payload(25, b"nexus:eth:xor");
        let SccpPayloadV1::RouteActivate(activation) = &mut payload else {
            unreachable!("sample payload is a route activation");
        };
        activation.asset_id = b"xor#universal".to_vec();
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&payload);
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload,
            )),
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
    fn validate_sccp_commitment_root_for_signed_block_rejects_direct_record_instruction() {
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            19,
            b"0x0000000000000000000000000000000000000452",
        ));
        let tx = signed_transaction_with_executable(Executable::Instructions(
            vec![InstructionBox::from(
                iroha_data_model::isi::bridge::RecordSccpMessage::new(payload),
            )]
            .into(),
        ));
        let block = signed_block_with_transactions(vec![tx], 9);

        let err = validate_sccp_commitment_root_for_signed_block(&block)
            .expect_err("successful direct SCCP record instruction must reject");

        assert_eq!(
            err,
            SccpCommittedBlockValidationError::InvalidRecordInstruction(
                SccpRecordInstructionValidationError::UnsupportedExecutable {
                    tx_index: 0,
                    instruction_index: 0,
                }
            )
        );
    }

    #[test]
    fn local_sccp_finality_records_reject_invalid_record_payload() {
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
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
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            17,
            b"0x0000000000000000000000000000000000000451",
        ));
        let sccp_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload,
            )),
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
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            15,
            b"0x0000000000000000000000000000000000000449",
        ));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload,
            )),
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
        let first_payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            10,
            b"0x0000000000000000000000000000000000000444",
        ));
        let second_payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            11,
            b"0x0000000000000000000000000000000000000555",
        ));
        let first_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                first_payload.clone(),
            )),
        ]));
        let second_tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                second_payload,
            )),
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
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            12,
            b"0x0000000000000000000000000000000000000666",
        ));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload,
            )),
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
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            13,
            b"0x0000000000000000000000000000000000000777",
        ));
        let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
            InstructionBox::from(iroha_data_model::isi::bridge::RecordSccpMessage::new(
                payload,
            )),
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
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            14,
            b"0x0000000000000000000000000000000000000888",
        ));
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
        let payload = iroha_sccp::canonical_sccp_payload_bytes(&sample_transfer_payload(
            7,
            b"0x0000000000000000000000000000000000000111",
        ));
        let executable = Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03]),
            overlay: vec![InstructionBox::from(
                iroha_data_model::isi::bridge::RecordSccpMessage::new(payload.clone()),
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
