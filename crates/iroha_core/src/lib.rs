//! Iroha — A simple, enterprise-grade decentralized ledger.
#![allow(unexpected_cfgs)]
// Nested `if` blocks remain intentional for readability/instrumentation; Clippy's
// `collapsible_if` lint would force let-chains that obscure the control flow.
#![allow(clippy::collapsible_if)]
#![allow(clippy::all)]
#![allow(clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(
    clippy::cast_lossless,
    clippy::cloned_instead_of_copied,
    clippy::clone_on_copy,
    clippy::collapsible_else_if,
    clippy::doc_markdown,
    clippy::explicit_iter_loop,
    clippy::identity_op,
    clippy::if_not_else,
    clippy::if_same_then_else,
    clippy::ignored_unit_patterns,
    clippy::iter_overeager_cloned,
    clippy::iter_with_drain,
    clippy::large_enum_variant,
    clippy::map_unwrap_or,
    clippy::match_same_arms,
    clippy::missing_const_for_thread_local,
    clippy::needless_borrows_for_generic_args,
    clippy::needless_continue,
    clippy::needless_pass_by_value,
    clippy::needless_return,
    clippy::option_if_let_else,
    clippy::ptr_arg,
    clippy::question_mark,
    clippy::redundant_closure_for_method_calls,
    clippy::redundant_pub_crate,
    clippy::result_large_err,
    clippy::return_self_not_must_use,
    clippy::single_match_else,
    clippy::struct_excessive_bools,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::unnecessary_wraps,
    clippy::unused_self,
    clippy::useless_conversion,
    clippy::useless_let_if_seq
)]
#![cfg_attr(test, allow(clippy::large_stack_arrays))]
#[cfg(all(feature = "kaigi_privacy_mocks", not(test)))]
compile_error!(
    "`kaigi_privacy_mocks` is a unit-test-only feature and cannot be enabled in a production library"
);
#[cfg(not(feature = "zk-halo2"))]
compile_error!(
    "Halo2 backends are mandatory; enable `zk-halo2` (default) when building iroha_core"
);
#[cfg(not(feature = "zk-halo2-ipa"))]
compile_error!(
    "Halo2 IPA backends are mandatory; enable `zk-halo2-ipa` (default) when building iroha_core"
);
#[cfg(not(feature = "zk-ipa-native"))]
compile_error!(
    "Native IPA helpers must remain enabled; `zk-ipa-native` is required for all builds"
);
/// Randomness beacon scaffolding using BLS‑VRF outputs.
pub mod alias;
/// Declarative alias setup classification and planning primitives.
pub mod alias_setup;
pub mod beacon;
/// Block types and helpers.
pub mod block;
/// Block synchronization protocol and messages.
/// Bridge finality proof helpers.
pub mod bridge;
/// Lane compliance policy evaluation.
pub mod compliance;
/// Data availability orchestration and ingest helpers.
pub mod da;
/// Runtime executor integration and helpers.
pub mod executor;
/// FASTPQ transcript helpers and host plumbing.
pub mod fastpq;
/// Unified settlement fee evidence structures.
pub mod fees;
/// Gas metering for non-VM ISI execution.
pub mod gas;
/// Gossip protocols for transactions and peers.
pub mod gossiper;
/// Governance helpers (parliament selection, etc.).
pub mod governance;
/// Cross-lane plumbing and privacy commitment registries.
pub mod interlane;
/// ISO bridge helpers (reference data ingestion, etc.).
pub mod iso_bridge;
/// Jurisdiction attestation/SDN enforcement helpers.
pub mod jurisdiction;
/// Kiso: storage primitives and data layout.
pub mod kiso;
/// Persistent block storage (Kura) backend.
pub mod kura;
/// Lane-local block vote validation and QC aggregation helpers.
pub mod lane_consensus;
mod lane_drain;
/// Merge-ledger reduction helpers.
pub mod merge;
/// Authenticated bounded transfer of certified merge-ledger sidecars.
pub mod merge_sidecar;
/// Rebuildable, non-consensus Musubi description and keyword search projection.
pub mod musubi_search;
/// Native AMX participant attestation control plane.
pub mod native_amx;
#[cfg(any(test, feature = "test-network-native-amx-fault-injection"))]
pub(crate) mod native_amx_fault_injection;
/// Nexus helpers (UAID portfolio aggregation, etc.).
pub mod nexus;
/// Oracle host helpers (admission/aggregation plumbing).
pub mod oracle;
/// Panic hook suppression helpers shared across crates.
pub mod panic_hook;
/// Peer discovery and gossip.
pub mod peers_gossiper;
/// Pipeline helpers (access-set derivation, scheduler glue)
pub mod pipeline;
/// First-release privacy protocol governance and admission budgets.
pub mod privacy;
/// Native transparent privacy protocol engines.
pub mod privacy_engines;
/// Deterministic compiled manifests for executable privacy engines.
pub mod privacy_profiles;
/// Native deterministic privacy release evidence, compiled only into explicit
/// release runners and opt-in integration gates.
#[cfg(feature = "privacy-release-evidence")]
pub mod privacy_release_evidence;
/// Durable records produced by verified first-release privacy actions.
pub mod privacy_state;
/// Exhaustive native proof verification and verified-effect derivation.
pub(crate) mod privacy_verifier;
/// Atomic private-settlement runtime helpers.
pub mod private_settlement;
/// Query API types and execution.
pub mod query;
/// Transaction queue and mempool logic.
pub mod queue;
pub(crate) mod receiver_snapshot;
mod secure_file_metadata;
/// Unified XOR settlement engine.
pub mod settlement;
/// Smart contracts and host ABI.
pub mod smartcontracts;
/// World state snapshots.
pub mod snapshot;
/// Ledger-backed SNS ownership helpers.
pub mod sns;
/// Shared Soracloud runtime snapshot types and traits.
pub mod soracloud_runtime;
/// SoraNet relay incentive calculator and treasury helpers.
pub mod soranet_incentives;
/// In-memory state and view types.
pub mod state;
/// Norito Streaming handshake/state helpers.
pub mod streaming;
/// Consensus protocol (Sumeragi).
pub mod sumeragi;
pub mod telemetry;
/// Network Time Service (scaffolding)
pub mod time;
/// Adaptive threshold-BLS timelock-release session and share verification.
pub mod tle_release;
/// Shared Torii helpers (query surfaces, filters).
pub mod torii;
/// Peer-to-peer Torii ingress proxy envelopes.
pub mod torii_proxy;
pub mod tx;
/// Validation-fee admission enforcement.
pub mod validation_fee;
/// Zero-knowledge verification helpers (backend dispatch + envelope validation).
pub mod zk;
/// Native STARK/FRI verifier under `zk-stark` (`stark/fri/*`).
#[cfg(feature = "zk-stark")]
pub mod zk_stark;
pub use block::InvalidGenesisError;
/// Encode one schema-bound public contract argument record using the canonical IVM ABI.
pub use ivm::encode_argument_record_from_json;
/// Pre-validate a genesis block against the expected genesis account prior to startup.
///
/// # Errors
///
/// Returns [`block::InvalidGenesisError`] when the provided block violates genesis invariants such
/// as signature, authority, or transaction structure requirements.
pub fn validate_genesis_block(
    block: &iroha_data_model::block::SignedBlock,
    genesis_account: &iroha_data_model::account::AccountId,
) -> Result<(), block::InvalidGenesisError> {
    block::check_genesis_block(block, genesis_account)
}
use core::time::Duration;
use gossiper::TransactionGossip;
use iroha_data_model::{events::EventBox, prelude::*};
use iroha_primitives::unique_vec::UniqueVec;
use norito::{
    codec::{Decode, Encode},
    streaming::ControlFrame,
};
use std::sync::Arc;
/// Re-export of Norito JSON derive macros for core crate internals.
pub mod json_macros {
    pub use norito::derive::{JsonDeserialize, JsonSerialize};
}
use crate::{
    merge_sidecar::CertifiedMergeSidecarMessage,
    peers_gossiper::{PeerTrustGossip, PeersGossip},
    sumeragi::message::{BlockMessage, BlockMessageWire},
};
use iroha_data_model::{merge::MergeCommitteeSignature, nexus::LaneRelayEnvelope};
use iroha_torii_shared::connect as connect_proto;
use tokio::sync::broadcast;
/// The interval at which sumeragi checks if there are tx in the `queue`.
pub const TX_RETRIEVAL_INTERVAL: Duration = Duration::from_millis(100);
/// Maximum encoded P2P frame size accepted for one lane-drain vote.
///
/// The cap covers the largest valid embedded lane committee and is enforced by
/// `irohad` before the vote reaches the Sumeragi actor queue.
pub const MAX_LANE_DRAIN_VOTE_WIRE_BYTES: usize = lane_consensus::MAX_LANE_DRAIN_VOTE_BYTES;
/// Maximum complete P2P frame admitted for one authenticated Kura replica advert.
///
/// The signed advert itself is capped at 16 KiB. The additional deterministic
/// headroom covers the nested `BlockMessageWire` and `NetworkMessage` Norito
/// frames without exposing the general network decoder to an attacker-sized
/// signature allocation.
pub const MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES: usize = 32 * 1024;
// Every live v2 message is nested inside `BlockMessageWire`, `NetworkMessage`,
// and the authenticated P2P relay/data envelopes. Keep one explicit allowance
// for those schema-bound layers while deriving attacker-controlled collection
// sizes from the consensus protocol constants below.
const SUMERAGI_V2_NETWORK_FRAME_OVERHEAD_BYTES: usize = 64 * 1024;
const SUMERAGI_V2_HASH_SEQUENCE_MAX_WIRE_BYTES: usize = core::mem::size_of::<u64>()
    + (iroha_data_model::block::consensus_v2::MAX_DA_CHUNK_COUNT as usize + 1)
        * core::mem::size_of::<u64>()
    + iroha_data_model::block::consensus_v2::MAX_DA_CHUNK_COUNT as usize
        * iroha_crypto::Hash::LENGTH;
const MAX_SUMERAGI_V2_CHUNK_NETWORK_FRAME_BYTES: usize =
    iroha_data_model::block::consensus_v2::MAX_DA_CHUNK_SIZE_BYTES as usize
        + iroha_data_model::block::consensus_v2::MAX_CONSENSUS_SIGNATURE_BYTES
        + 2 * core::mem::size_of::<u64>()
        + SUMERAGI_V2_NETWORK_FRAME_OVERHEAD_BYTES;
const MAX_SUMERAGI_V2_CONTROL_NETWORK_FRAME_BYTES: usize =
    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONTROL.get();
const MAX_SUMERAGI_V2_CERTIFIED_BODY_RESPONSE_NETWORK_FRAME_BYTES: usize =
    iroha_config::parameters::defaults::network::MAX_PLAINTEXT_FRAME_BYTES.get();
const MAX_SUMERAGI_V2_DECODE_DEPTH: usize = 64;
const _: () = assert!(
    MAX_SUMERAGI_V2_CHUNK_NETWORK_FRAME_BYTES < MAX_SUMERAGI_V2_CONTROL_NETWORK_FRAME_BYTES
);
const _: () = assert!(
    iroha_data_model::block::consensus_v2::MAX_DA_PAYLOAD_SIZE_BYTES as usize
        + SUMERAGI_V2_HASH_SEQUENCE_MAX_WIRE_BYTES
        + SUMERAGI_V2_NETWORK_FRAME_OVERHEAD_BYTES
        <= MAX_SUMERAGI_V2_CERTIFIED_BODY_RESPONSE_NETWORK_FRAME_BYTES
);
const NETWORK_MESSAGE_LANE_DRAIN_VOTE_TAG: u32 = 3;
const NETWORK_MESSAGE_TORII_PROXY_REQUEST_TAG: u32 = 13;
const NETWORK_MESSAGE_TORII_PROXY_RESPONSE_TAG: u32 = 14;
const NETWORK_MESSAGE_QUEUE_PLAN_ADMISSION_PUBLICATION_TAG: u32 = 16;
const NETWORK_MESSAGE_QUEUE_PLAN_ADMISSION_CERTIFICATE_TAG: u32 = 17;
/// Hard Norito frame bound for one QueuePlan admission-certificate handoff.
pub const MAX_QUEUE_PLAN_ADMISSION_CERTIFICATE_WIRE_BYTES: usize =
    iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES + 64 * 1024;
const MAX_LANE_DRAIN_VOTE_DECODE_ELEMENTS: usize = MAX_LANE_DRAIN_VOTE_WIRE_BYTES;
// A canonical 128-member BLS committee needs just over 256 KiB under Norito's
// conservative nested alignment-copy accounting. Keep deterministic headroom
// while the 16 KiB frame and exact 128-element sequence caps remain primary.
const MAX_LANE_DRAIN_VOTE_DECODE_ALLOCATED_BYTES: usize = 512 * 1024;
const MAX_LANE_DRAIN_VOTE_DECODE_DEPTH: usize = 64;
fn inbound_enum_parts(payload: &[u8]) -> Result<(u32, &[u8]), norito::core::Error> {
    let tag: [u8; core::mem::size_of::<u32>()] = payload
        .get(..core::mem::size_of::<u32>())
        .ok_or(norito::core::Error::LengthMismatch)?
        .try_into()
        .map_err(|_| norito::core::Error::LengthMismatch)?;
    let remaining = payload
        .get(core::mem::size_of::<u32>()..)
        .ok_or(norito::core::Error::LengthMismatch)?;
    Ok((u32::from_le_bytes(tag), remaining))
}
fn inbound_enum_field(remaining: &[u8], flags: u8) -> Result<&[u8], norito::core::Error> {
    let (field_len, prefix_len) = norito::core::read_len_from_slice_with_flags(remaining, flags)?;
    let field_end = prefix_len
        .checked_add(field_len)
        .ok_or(norito::core::Error::LengthMismatch)?;
    if field_end != remaining.len() {
        return Err(norito::core::Error::LengthMismatch);
    }
    remaining
        .get(prefix_len..field_end)
        .ok_or(norito::core::Error::LengthMismatch)
}
fn inbound_owned_enum_field(remaining: &[u8], flags: u8) -> Result<&[u8], norito::core::Error> {
    // Enum fields are length-delimited by the derive, while Box/Arc add a
    // second ownership prefix around their value. Nested raw classifiers must
    // inspect the value after both canonical boundaries.
    let owned = inbound_enum_field(remaining, flags)?;
    inbound_enum_field(owned, flags)
}
fn inbound_two_field_struct(
    payload: &[u8],
    flags: u8,
    first_field_width: usize,
) -> Result<(&[u8], &[u8]), norito::core::Error> {
    use norito::core::{Error, header_flags};
    if flags & header_flags::PACKED_STRUCT == 0 {
        let (first_len, first_prefix) =
            norito::core::read_len_from_slice_with_flags(payload, flags)?;
        let first_end = first_prefix
            .checked_add(first_len)
            .ok_or(Error::LengthMismatch)?;
        let first = payload
            .get(first_prefix..first_end)
            .ok_or(Error::LengthMismatch)?;
        let remaining = payload.get(first_end..).ok_or(Error::LengthMismatch)?;
        let second = inbound_enum_field(remaining, flags)?;
        return (first.len() == first_field_width)
            .then_some((first, second))
            .ok_or(Error::LengthMismatch);
    }
    if flags & header_flags::FIELD_BITSET == 0 {
        const FIELD_COUNT: usize = 2;
        let table_len = (FIELD_COUNT + 1)
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or(Error::LengthMismatch)?;
        let (offsets, fields) = payload
            .split_at_checked(table_len)
            .ok_or(Error::LengthMismatch)?;
        let read_offset = |index: usize| -> Result<usize, Error> {
            let start = index
                .checked_mul(core::mem::size_of::<u64>())
                .ok_or(Error::LengthMismatch)?;
            let end = start
                .checked_add(core::mem::size_of::<u64>())
                .ok_or(Error::LengthMismatch)?;
            let bytes: [u8; 8] = offsets
                .get(start..end)
                .ok_or(Error::LengthMismatch)?
                .try_into()
                .map_err(|_| Error::LengthMismatch)?;
            usize::try_from(u64::from_le_bytes(bytes)).map_err(|_| Error::LengthMismatch)
        };
        let start = read_offset(0)?;
        let middle = read_offset(1)?;
        let end = read_offset(2)?;
        if start != 0 || middle != first_field_width || middle > end || end != fields.len() {
            return Err(Error::LengthMismatch);
        }
        return Ok((
            fields.get(start..middle).ok_or(Error::LengthMismatch)?,
            fields.get(middle..end).ok_or(Error::LengthMismatch)?,
        ));
    }
    // `ConsensusMessageV2` has a fixed-width u16 followed by one dynamic enum.
    const EXPECTED_FIELD_BITSET: u8 = 0b0000_0010;
    let (&bitset, size_header) = payload.split_first().ok_or(Error::LengthMismatch)?;
    if bitset != EXPECTED_FIELD_BITSET {
        return Err(Error::LengthMismatch);
    }
    let (second_len, prefix_len) =
        norito::core::read_len_from_slice_with_flags(size_header, flags)?;
    let fields = size_header.get(prefix_len..).ok_or(Error::LengthMismatch)?;
    let expected_len = first_field_width
        .checked_add(second_len)
        .ok_or(Error::LengthMismatch)?;
    if fields.len() != expected_len {
        return Err(Error::LengthMismatch);
    }
    Ok((
        fields
            .get(..first_field_width)
            .ok_or(Error::LengthMismatch)?,
        fields
            .get(first_field_width..)
            .ok_or(Error::LengthMismatch)?,
    ))
}
#[derive(Clone, Copy)]
enum InboundStructField {
    Fixed(usize),
    Sized,
    ByteSequence,
    Sequence,
}
fn inbound_sequence_count(bytes: &[u8]) -> Result<(u64, usize), norito::core::Error> {
    let prefix = bytes
        .get(..core::mem::size_of::<u64>())
        .ok_or(norito::core::Error::LengthMismatch)?;
    let prefix: [u8; core::mem::size_of::<u64>()] = prefix
        .try_into()
        .map_err(|_| norito::core::Error::LengthMismatch)?;
    Ok((u64::from_le_bytes(prefix), core::mem::size_of::<u64>()))
}
fn inbound_byte_sequence_wire_len(bytes: &[u8]) -> Result<usize, norito::core::Error> {
    let (count, prefix_len) = inbound_sequence_count(bytes)?;
    let count = usize::try_from(count).map_err(|_| norito::core::Error::LengthMismatch)?;
    prefix_len
        .checked_add(count)
        .filter(|len| *len <= bytes.len())
        .ok_or(norito::core::Error::LengthMismatch)
}
fn inbound_struct_field<'a>(
    payload: &'a [u8],
    flags: u8,
    fields: &[InboundStructField],
    target: usize,
) -> Result<&'a [u8], norito::core::Error> {
    use norito::core::{Error, header_flags};
    if target >= fields.len() || fields.len() > u8::BITS as usize {
        return Err(Error::LengthMismatch);
    }
    if flags & header_flags::PACKED_STRUCT == 0 {
        let mut remaining = payload;
        let mut selected = None;
        for index in 0..fields.len() {
            let (field_len, prefix_len) =
                norito::core::read_len_from_slice_with_flags(remaining, flags)?;
            let field_end = prefix_len
                .checked_add(field_len)
                .ok_or(Error::LengthMismatch)?;
            let field = remaining
                .get(prefix_len..field_end)
                .ok_or(Error::LengthMismatch)?;
            if index == target {
                selected = Some(field);
            }
            remaining = remaining.get(field_end..).ok_or(Error::LengthMismatch)?;
        }
        if !remaining.is_empty() {
            return Err(Error::LengthMismatch);
        }
        return selected.ok_or(Error::LengthMismatch);
    }
    if flags & header_flags::FIELD_BITSET == 0 {
        let table_entries = fields.len().checked_add(1).ok_or(Error::LengthMismatch)?;
        let table_len = table_entries
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or(Error::LengthMismatch)?;
        let (offsets, data) = payload
            .split_at_checked(table_len)
            .ok_or(Error::LengthMismatch)?;
        let read_offset = |index: usize| -> Result<usize, Error> {
            let start = index
                .checked_mul(core::mem::size_of::<u64>())
                .ok_or(Error::LengthMismatch)?;
            let end = start
                .checked_add(core::mem::size_of::<u64>())
                .ok_or(Error::LengthMismatch)?;
            let encoded: [u8; core::mem::size_of::<u64>()] = offsets
                .get(start..end)
                .ok_or(Error::LengthMismatch)?
                .try_into()
                .map_err(|_| Error::LengthMismatch)?;
            usize::try_from(u64::from_le_bytes(encoded)).map_err(|_| Error::LengthMismatch)
        };
        let mut previous = 0;
        for index in 0..table_entries {
            let offset = read_offset(index)?;
            if (index == 0 && offset != 0) || offset < previous || offset > data.len() {
                return Err(Error::LengthMismatch);
            }
            previous = offset;
        }
        if previous != data.len() {
            return Err(Error::LengthMismatch);
        }
        let start = read_offset(target)?;
        let end = read_offset(target + 1)?;
        return data.get(start..end).ok_or(Error::LengthMismatch);
    }

    let (&bitset, mut size_headers) = payload.split_first().ok_or(Error::LengthMismatch)?;
    let expected_bitset = fields
        .iter()
        .enumerate()
        .fold(0_u8, |bits, (index, field)| {
            if matches!(field, InboundStructField::Sized) {
                bits | (1_u8 << index)
            } else {
                bits
            }
        });
    if bitset != expected_bitset {
        return Err(Error::LengthMismatch);
    }
    let mut sized_lengths = [0_usize; u8::BITS as usize];
    for (index, field) in fields.iter().enumerate() {
        if matches!(field, InboundStructField::Sized) {
            let (field_len, prefix_len) =
                norito::core::read_len_from_slice_with_flags(size_headers, flags)?;
            sized_lengths[index] = field_len;
            size_headers = size_headers
                .get(prefix_len..)
                .ok_or(Error::LengthMismatch)?;
        }
    }
    let data = size_headers;
    let mut offset = 0_usize;
    for (index, field) in fields.iter().enumerate() {
        if index == target && matches!(field, InboundStructField::Sequence) {
            return data.get(offset..).ok_or(Error::LengthMismatch);
        }
        let field_len = match field {
            InboundStructField::Fixed(width) => *width,
            InboundStructField::Sized => sized_lengths[index],
            InboundStructField::ByteSequence => {
                inbound_byte_sequence_wire_len(data.get(offset..).ok_or(Error::LengthMismatch)?)?
            }
            InboundStructField::Sequence => return Err(Error::LengthMismatch),
        };
        let end = offset.checked_add(field_len).ok_or(Error::LengthMismatch)?;
        let field = data.get(offset..end).ok_or(Error::LengthMismatch)?;
        if index == target {
            return Ok(field);
        }
        offset = end;
    }
    Err(Error::LengthMismatch)
}
fn enforce_inbound_sequence_limit(field: &[u8], limit: usize) -> Result<(), norito::core::Error> {
    let (length, _) = inbound_sequence_count(field)?;
    let limit = u64::try_from(limit).unwrap_or(u64::MAX);
    if length > limit {
        return Err(norito::core::Error::SequenceLengthExceeded { length, limit });
    }
    Ok(())
}
fn enforce_inbound_byte_sequence_limit(
    field: &[u8],
    limit: usize,
) -> Result<(), norito::core::Error> {
    enforce_inbound_sequence_limit(field, limit)?;
    let exact_len = inbound_byte_sequence_wire_len(field)?;
    if exact_len != field.len() {
        return Err(norito::core::Error::LengthMismatch);
    }
    Ok(())
}
fn enforce_inbound_manifest_limits(manifest: &[u8], flags: u8) -> Result<(), norito::core::Error> {
    const FIELDS: [InboundStructField; 6] = [
        InboundStructField::Sized,
        InboundStructField::Sized,
        InboundStructField::Fixed(core::mem::size_of::<u64>()),
        InboundStructField::Sized,
        InboundStructField::Sequence,
        InboundStructField::Sized,
    ];
    let hashes = inbound_struct_field(manifest, flags, &FIELDS, 4)?;
    enforce_inbound_sequence_limit(
        hashes,
        iroha_data_model::block::consensus_v2::MAX_DA_CHUNK_COUNT as usize,
    )
}
fn enforce_inbound_consensus_v2_payload_limits(
    tag: u32,
    payload: &[u8],
    flags: u8,
) -> Result<(), norito::core::Error> {
    use iroha_data_model::block::consensus_v2::{
        MAX_CONSENSUS_SIGNATURE_BYTES, MAX_DA_CHUNK_SIZE_BYTES, MAX_DA_PAYLOAD_SIZE_BYTES,
    };
    const PROPOSAL_FIELDS: [InboundStructField; 6] = [
        InboundStructField::Sized,
        // `ValidatorIndex` is a type alias, but the derive deliberately treats
        // arbitrary named field types as length-delimited in hybrid layouts.
        InboundStructField::Sized,
        InboundStructField::Sized,
        InboundStructField::Sized,
        InboundStructField::Sized,
        InboundStructField::ByteSequence,
    ];
    const CHUNK_FIELDS: [InboundStructField; 5] = [
        InboundStructField::Sized,
        InboundStructField::Fixed(core::mem::size_of::<u32>()),
        InboundStructField::ByteSequence,
        InboundStructField::Sized,
        InboundStructField::ByteSequence,
    ];
    const CERTIFIED_RESPONSE_FIELDS: [InboundStructField; 5] = [
        InboundStructField::Sized,
        InboundStructField::Sized,
        InboundStructField::ByteSequence,
        InboundStructField::Sized,
        InboundStructField::ByteSequence,
    ];
    match tag {
        0 => {
            enforce_inbound_manifest_limits(
                inbound_struct_field(payload, flags, &PROPOSAL_FIELDS, 3)?,
                flags,
            )?;
            enforce_inbound_byte_sequence_limit(
                inbound_struct_field(payload, flags, &PROPOSAL_FIELDS, 5)?,
                MAX_CONSENSUS_SIGNATURE_BYTES,
            )
        }
        5 => {
            enforce_inbound_byte_sequence_limit(
                inbound_struct_field(payload, flags, &CHUNK_FIELDS, 2)?,
                MAX_DA_CHUNK_SIZE_BYTES as usize,
            )?;
            enforce_inbound_byte_sequence_limit(
                inbound_struct_field(payload, flags, &CHUNK_FIELDS, 4)?,
                MAX_CONSENSUS_SIGNATURE_BYTES,
            )
        }
        7 => {
            enforce_inbound_manifest_limits(
                inbound_struct_field(payload, flags, &CERTIFIED_RESPONSE_FIELDS, 1)?,
                flags,
            )?;
            enforce_inbound_byte_sequence_limit(
                inbound_struct_field(payload, flags, &CERTIFIED_RESPONSE_FIELDS, 2)?,
                MAX_DA_PAYLOAD_SIZE_BYTES as usize,
            )?;
            enforce_inbound_byte_sequence_limit(
                inbound_struct_field(payload, flags, &CERTIFIED_RESPONSE_FIELDS, 4)?,
                MAX_CONSENSUS_SIGNATURE_BYTES,
            )
        }
        _ => Ok(()),
    }
}
fn inbound_consensus_v2_parts(
    payload: &[u8],
    flags: u8,
) -> Result<(u16, u32, &[u8]), norito::core::Error> {
    let (version, message) = inbound_two_field_struct(payload, flags, core::mem::size_of::<u16>())?;
    let version: [u8; core::mem::size_of::<u16>()] = version
        .try_into()
        .map_err(|_| norito::core::Error::LengthMismatch)?;
    let (tag, remaining) = inbound_enum_parts(message)?;
    let field = inbound_enum_field(remaining, flags)?;
    Ok((u16::from_le_bytes(version), tag, field))
}
fn inbound_consensus_v2_topic(
    payload: &[u8],
    flags: u8,
) -> Result<iroha_p2p::network::message::Topic, norito::core::Error> {
    use iroha_data_model::block::consensus_v2::PROTOCOL_VERSION;
    use iroha_p2p::network::message::Topic;
    let (version, tag, _) = inbound_consensus_v2_parts(payload, flags)?;
    if version != PROTOCOL_VERSION {
        return Ok(Topic::Other);
    }
    match tag {
        0..=4 | 9..=10 => Ok(Topic::ConsensusSafety),
        7 => Ok(Topic::ConsensusPayload),
        5 => Ok(Topic::ConsensusChunk),
        6 | 8 => Ok(Topic::Consensus),
        _ => Err(norito::core::Error::Message(
            "unknown Sumeragi v2 payload discriminant".to_owned(),
        )),
    }
}
fn inbound_consensus_v2_decode_limits(
    payload: &[u8],
    framed_len: usize,
    flags: u8,
) -> Result<Option<norito::DecodeLimits>, norito::core::Error> {
    use iroha_data_model::block::consensus_v2::{
        MAX_CONSENSUS_SIGNATURE_BYTES, MAX_DA_CHUNK_COUNT, MAX_DA_CHUNK_SIZE_BYTES,
        MAX_DA_PAYLOAD_SIZE_BYTES, PROTOCOL_VERSION,
    };
    let (version, tag, payload_field) = inbound_consensus_v2_parts(payload, flags)?;
    if version != PROTOCOL_VERSION {
        // The raw topic classifier routes another protocol revision through
        // the much smaller `Other` cap before this hook is reached.
        return Ok(None);
    }
    let default_sequence_limit = usize::try_from(MAX_DA_CHUNK_COUNT)
        .unwrap_or(usize::MAX)
        .max(MAX_CONSENSUS_SIGNATURE_BYTES);
    let (frame_limit, sequence_limit) = match tag {
        5 => (
            MAX_SUMERAGI_V2_CHUNK_NETWORK_FRAME_BYTES,
            usize::try_from(MAX_DA_CHUNK_SIZE_BYTES).unwrap_or(usize::MAX),
        ),
        7 => (
            MAX_SUMERAGI_V2_CERTIFIED_BODY_RESPONSE_NETWORK_FRAME_BYTES,
            usize::try_from(MAX_DA_PAYLOAD_SIZE_BYTES).unwrap_or(usize::MAX),
        ),
        0..=4 | 6 | 8..=10 => (
            MAX_SUMERAGI_V2_CONTROL_NETWORK_FRAME_BYTES,
            default_sequence_limit,
        ),
        _ => {
            return Err(norito::core::Error::Message(
                "unknown Sumeragi v2 payload discriminant".to_owned(),
            ));
        }
    };
    if framed_len > frame_limit {
        return Err(norito::core::Error::ArchiveLengthExceeded {
            length: u64::try_from(framed_len).unwrap_or(u64::MAX),
            limit: u64::try_from(frame_limit).unwrap_or(u64::MAX),
        });
    }
    enforce_inbound_consensus_v2_payload_limits(tag, payload_field, flags)?;
    let canonical = norito::canonical_decode_limits(frame_limit);
    Ok(Some(norito::DecodeLimits::new(
        sequence_limit,
        frame_limit,
        canonical.max_total_elements(),
        canonical.max_total_allocated_bytes(),
        MAX_SUMERAGI_V2_DECODE_DEPTH,
    )))
}
fn inbound_sumeragi_enum_field(framed: &[u8]) -> Result<(u32, &[u8], u8), norito::core::Error> {
    let view = norito::core::from_bytes_view(framed)?;
    if view.schema() != <BlockMessage as norito::NoritoSerialize>::schema_hash() {
        return Err(norito::core::Error::SchemaMismatch);
    }
    let align = norito::core::archived_payload_align::<BlockMessage>();
    let padding = if align <= 1 {
        0
    } else {
        let remainder = norito::core::Header::SIZE % align;
        if remainder == 0 { 0 } else { align - remainder }
    };
    let exact_len = norito::core::Header::SIZE
        .checked_add(padding)
        .and_then(|prefix| prefix.checked_add(view.as_bytes().len()))
        .ok_or(norito::core::Error::LengthMismatch)?;
    if exact_len != framed.len() {
        return Err(norito::core::Error::LengthMismatch);
    }
    let (tag, remaining) = inbound_enum_parts(view.as_bytes())?;
    let field = inbound_enum_field(remaining, view.flags())?;
    Ok((tag, field, view.flags()))
}
fn inbound_sumeragi_topic(
    framed: &[u8],
) -> Result<iroha_p2p::network::message::Topic, norito::core::Error> {
    use iroha_p2p::network::message::Topic;
    let (tag, field, flags) = inbound_sumeragi_enum_field(framed)?;
    match tag {
        // Keep these discriminants synchronized with `BlockMessage`. They are
        // inspected before allocating or decoding the nested consensus value.
        0 | 1 | 3..=8 => Ok(Topic::Consensus),
        2 | 9 => Ok(Topic::ConsensusPayload),
        10 => inbound_consensus_v2_topic(field, flags),
        _ => Err(norito::core::Error::Message(
            "unknown Sumeragi block discriminant".to_owned(),
        )),
    }
}
fn inbound_certified_merge_sidecar_topic(
    payload: &[u8],
    flags: u8,
) -> Result<iroha_p2p::network::message::Topic, norito::core::Error> {
    use iroha_p2p::network::message::Topic;
    let (tag, remaining) = inbound_enum_parts(payload)?;
    inbound_enum_field(remaining, flags)?;
    match tag {
        0..=3 => Ok(Topic::Consensus),
        4 => Ok(Topic::ConsensusChunk),
        _ => Err(norito::core::Error::Message(
            "unknown certified merge-sidecar discriminant".to_owned(),
        )),
    }
}
fn inbound_transaction_gossip_topic(
    payload: &[u8],
    flags: u8,
) -> Result<iroha_p2p::network::message::Topic, norito::core::Error> {
    use iroha_p2p::network::message::Topic;
    let mut remaining = payload;
    let mut plane = None;
    for index in 0..4 {
        let (field_len, prefix_len) =
            norito::core::read_len_from_slice_with_flags(remaining, flags)?;
        let field_end = prefix_len
            .checked_add(field_len)
            .ok_or(norito::core::Error::LengthMismatch)?;
        let field = remaining
            .get(prefix_len..field_end)
            .ok_or(norito::core::Error::LengthMismatch)?;
        remaining = remaining
            .get(field_end..)
            .ok_or(norito::core::Error::LengthMismatch)?;
        if index == 3 {
            plane = Some(field);
        }
    }
    if !remaining.is_empty() {
        return Err(norito::core::Error::LengthMismatch);
    }
    let (tag, trailing) = inbound_enum_parts(plane.ok_or(norito::core::Error::LengthMismatch)?)?;
    if !trailing.is_empty() {
        return Err(norito::core::Error::LengthMismatch);
    }
    match tag {
        0 => Ok(Topic::TxGossip),
        1 => Ok(Topic::TxGossipRestricted),
        _ => Err(norito::core::Error::Message(
            "unknown transaction-gossip plane discriminant".to_owned(),
        )),
    }
}
/// Specialized type of Iroha Network
pub type IrohaNetwork = iroha_p2p::NetworkHandle<NetworkMessage>;
/// Ids of peers.
pub type Peers = UniqueVec<PeerId>;
/// Type of `Sender<EventBox>` which should be used for channels of `Event` messages.
pub type EventsSender = broadcast::Sender<EventBox>;
/// Network message envelope exchanged between peers.
#[derive(Clone, Debug, Decode, Encode)]
pub enum NetworkMessage {
    /// Live Sumeragi v2, lane-local, or authenticated auxiliary consensus data.
    #[codec(index = 0)]
    SumeragiBlock(Arc<BlockMessageWire>),
    /// Lane settlement relay envelope (NX-4).
    #[codec(index = 1)]
    LaneRelay(Box<LaneRelayEnvelope>),
    /// Merge committee signature share for merge-ledger quorum certificates.
    #[codec(index = 2)]
    MergeCommitteeSignature(Arc<MergeCommitteeSignature>),
    /// Lane-committee signature share for an automatic drain certificate.
    #[codec(index = 3)]
    LaneDrainVote(Box<crate::lane_consensus::LaneDrainVoteV1>),
    /// Authenticated request/chunk traffic for a block-referenced certified merge sidecar.
    #[codec(index = 4)]
    CertifiedMergeSidecar(Arc<CertifiedMergeSidecarMessage>),
    /// Native AMX participant attestation control-plane message.
    #[codec(index = 5)]
    NativeAmx(Arc<native_amx::NativeAmxMessage>),
    /// Transaction gossiper message.
    #[codec(index = 6)]
    TransactionGossiper(Arc<TransactionGossip>),
    /// Peer address gossip message.
    #[codec(index = 7)]
    PeersGossiper(Box<PeersGossip>),
    /// Peer trust gossip message.
    #[codec(index = 8)]
    PeerTrustGossip(Box<PeerTrustGossip>),
    /// Health check message.
    #[codec(index = 9)]
    Health,
    /// Network Time Service: time synchronization ping.
    #[codec(index = 10)]
    TimePing(Box<crate::time::TimePing>),
    /// Network Time Service: time synchronization pong.
    #[codec(index = 11)]
    TimePong(Box<crate::time::TimePong>),
    /// Iroha Connect (WalletConnect-style) authenticated P2P control message.
    #[codec(index = 12)]
    Connect(Box<connect_proto::ConnectP2pMessage>),
    /// Torii proxy request routed across bounded Torii ingress proxy hops.
    #[codec(index = 13)]
    ToriiProxyRequest(Arc<torii_proxy::ToriiProxyRequestV1>),
    /// Torii proxy response returned to the ingress node.
    #[codec(index = 14)]
    ToriiProxyResponse(Box<torii_proxy::ToriiProxyResponseV1>),
    /// Norito Streaming control-plane frame.
    #[codec(index = 15)]
    StreamingControl(Box<ControlFrame>),
    /// Certified QueuePlan admission disseminated to every live authoritative validator.
    #[codec(index = 16)]
    QueuePlanAdmissionPublication(Arc<torii_proxy::QueuePlanAdmissionPublicationV1>),
    /// Exact Kura-durable QueuePlan admission certificate handed to the global leader.
    #[codec(index = 17)]
    QueuePlanAdmissionCertificate(Arc<Vec<u8>>),
}
impl NetworkMessage {
    /// Returns `true` when the message is handled by Torii's proxy-plane P2P
    /// subscribers instead of the generic `irohad` relay path.
    #[must_use]
    pub const fn is_torii_proxy_control_message(&self) -> bool {
        matches!(
            self,
            Self::ToriiProxyRequest(_)
                | Self::ToriiProxyResponse(_)
                | Self::QueuePlanAdmissionPublication(_)
        )
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for NetworkMessage {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        use std::borrow::Cow;
        let min_size = norito::core::archived_payload_size::<Self>();
        let decode_bytes: Cow<'a, [u8]> = if min_size > 0 && bytes.len() < min_size {
            let mut padded = Vec::with_capacity(min_size);
            padded.extend_from_slice(bytes);
            padded.resize(min_size, 0);
            Cow::Owned(padded)
        } else {
            Cow::Borrowed(bytes)
        };
        let archived = norito::core::archived_from_slice::<Self>(decode_bytes.as_ref())?;
        let _guard = norito::core::PayloadCtxGuard::enter_with_len(archived.bytes(), bytes.len());
        let value =
            <Self as norito::core::NoritoDeserialize>::try_deserialize(archived.archived())?;
        Ok((value, bytes.len()))
    }
}
// Encode/Decode are derived above for `NetworkMessage`.
// Classify core network messages into P2P topics for scheduling.
impl iroha_p2p::network::message::ClassifyTopic for NetworkMessage {
    const HAS_INBOUND_DECODE_LIMITS: bool = true;
    fn topic(&self) -> iroha_p2p::network::message::Topic {
        use iroha_p2p::network::message::Topic as T;
        match self {
            NetworkMessage::SumeragiBlock(msg) => match msg.as_ref().as_ref() {
                BlockMessage::V2(message) => {
                    use iroha_data_model::block::consensus_v2::{
                        ConsensusMessageV2Payload, PROTOCOL_VERSION,
                    };
                    if message.protocol_version != PROTOCOL_VERSION {
                        T::Other
                    } else {
                        match &message.payload {
                            ConsensusMessageV2Payload::PayloadChunk(_) => T::ConsensusChunk,
                            ConsensusMessageV2Payload::CertifiedBodyResponse(_) => {
                                T::ConsensusPayload
                            }
                            ConsensusMessageV2Payload::Proposal(_)
                            | ConsensusMessageV2Payload::Vote(_)
                            | ConsensusMessageV2Payload::QuorumCertificate(_)
                            | ConsensusMessageV2Payload::TimeoutVote(_)
                            | ConsensusMessageV2Payload::TimeoutCertificate(_)
                            | ConsensusMessageV2Payload::CommitCertificateResponse(_)
                            | ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => {
                                T::ConsensusSafety
                            }
                            ConsensusMessageV2Payload::CertifiedBodyRequest(_)
                            | ConsensusMessageV2Payload::CommitCertificateRequest(_) => {
                                T::Consensus
                            }
                        }
                    }
                }
                BlockMessage::LaneExecutablePayload(_)
                | BlockMessage::LaneHistoricalRecoveryResponse(_) => T::ConsensusPayload,
                BlockMessage::LaneBlockProposal(_)
                | BlockMessage::LaneBlockNewViewVote(_)
                | BlockMessage::LaneBlockNewViewCertificate(_)
                | BlockMessage::LaneBlockVote(_)
                | BlockMessage::LaneBlockQc(_)
                | BlockMessage::LaneBlockCertificate(_)
                | BlockMessage::LaneHistoricalRecoveryRequest(_) => T::Consensus,
                BlockMessage::KuraReplicaAdvert(_) => T::Consensus,
            },
            NetworkMessage::CertifiedMergeSidecar(message) => match message.as_ref() {
                CertifiedMergeSidecarMessage::Request(_)
                | CertifiedMergeSidecarMessage::Close(_)
                | CertifiedMergeSidecarMessage::CloseAck(_)
                | CertifiedMergeSidecarMessage::GenerationHint(_) => T::Consensus,
                CertifiedMergeSidecarMessage::Chunk(_) => T::ConsensusChunk,
            },
            NetworkMessage::LaneRelay(_)
            | NetworkMessage::MergeCommitteeSignature(_)
            | NetworkMessage::LaneDrainVote(_)
            | NetworkMessage::NativeAmx(_)
            | NetworkMessage::QueuePlanAdmissionCertificate(_) => T::Consensus,
            NetworkMessage::ToriiProxyRequest(_)
            | NetworkMessage::ToriiProxyResponse(_)
            | NetworkMessage::QueuePlanAdmissionPublication(_)
            | NetworkMessage::StreamingControl(_) => T::Control,
            NetworkMessage::TransactionGossiper(gossip) => match gossip.plane {
                gossiper::GossipPlane::Public => T::TxGossip,
                gossiper::GossipPlane::Restricted => T::TxGossipRestricted,
            },
            NetworkMessage::PeersGossiper(_) => T::PeerGossip,
            NetworkMessage::PeerTrustGossip(_) => T::TrustGossip,
            NetworkMessage::Health
            | NetworkMessage::TimePing(_)
            | NetworkMessage::TimePong(_)
            | NetworkMessage::Connect(_) => T::Health,
        }
    }
    fn subscriber_route(&self) -> iroha_p2p::network::message::SubscriberRoute {
        use iroha_p2p::network::message::SubscriberRoute;
        match self {
            Self::ToriiProxyRequest(_)
            | Self::ToriiProxyResponse(_)
            | Self::QueuePlanAdmissionPublication(_) => SubscriberRoute::ToriiProxy,
            Self::Connect(_) => SubscriberRoute::Connect,
            _ => SubscriberRoute::General,
        }
    }
    fn progress_reconstruction(&self) -> iroha_p2p::network::message::ProgressReconstruction {
        use iroha_p2p::network::message::ProgressReconstruction;
        match self {
            // Sumeragi and certified-sidecar workers retain exact pending work
            // and retry it through their bounded schedulers.
            Self::SumeragiBlock(_) | Self::CertifiedMergeSidecar(_) => {
                ProgressReconstruction::Retransmit
            }
            // Lane/merge producers rebuild their bounded handoff after
            // temporary actor pressure. Transport must keep the accepted
            // exact occurrence until writer flush; none of these payloads may
            // be retired merely because state synchronization might later
            // subsume it.
            Self::LaneRelay(_)
            | Self::MergeCommitteeSignature(_)
            | Self::LaneDrainVote(_)
            | Self::NativeAmx(_)
            | Self::QueuePlanAdmissionCertificate(_) => ProgressReconstruction::Retransmit,
            _ => ProgressReconstruction::Exact,
        }
    }
    fn inbound_topic(
        payload: &[u8],
        flags: u8,
    ) -> Result<Option<iroha_p2p::network::message::Topic>, norito::core::Error> {
        use iroha_p2p::network::message::Topic;
        let (tag, remaining) = inbound_enum_parts(payload)?;
        if tag == 9 {
            if !remaining.is_empty() {
                return Err(norito::core::Error::LengthMismatch);
            }
            return Ok(Some(Topic::Health));
        }
        let field = if matches!(tag, 0 | 4 | 6 | 16) {
            inbound_owned_enum_field(remaining, flags)?
        } else {
            inbound_enum_field(remaining, flags)?
        };
        let topic = match tag {
            0 => inbound_sumeragi_topic(field)?,
            1..=3 | 5 | 17 => Topic::Consensus,
            4 => inbound_certified_merge_sidecar_topic(field, flags)?,
            6 => inbound_transaction_gossip_topic(field, flags)?,
            7 => Topic::PeerGossip,
            8 => Topic::TrustGossip,
            10..=12 => Topic::Health,
            13..=16 => Topic::Control,
            _ => {
                return Err(norito::core::Error::Message(
                    "unknown core network-message discriminant".to_owned(),
                ));
            }
        };
        Ok(Some(topic))
    }
    fn inbound_decode_limits(
        payload: &[u8],
        framed_len: usize,
        flags: u8,
    ) -> Result<Option<norito::DecodeLimits>, norito::core::Error> {
        let discriminant = payload
            .get(..core::mem::size_of::<u32>())
            .ok_or(norito::core::Error::LengthMismatch)?;
        let mut discriminant_bytes = [0_u8; core::mem::size_of::<u32>()];
        discriminant_bytes.copy_from_slice(discriminant);
        match u32::from_le_bytes(discriminant_bytes) {
            0 => {
                let (_, remaining) = inbound_enum_parts(payload)?;
                let framed = inbound_owned_enum_field(remaining, flags)?;
                let (block_tag, block, block_flags) = inbound_sumeragi_enum_field(framed)?;
                if block_tag == 10 {
                    return inbound_consensus_v2_decode_limits(block, framed_len, block_flags);
                }
                if block_tag == 0 {
                    if framed_len > MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES {
                        return Err(norito::core::Error::ArchiveLengthExceeded {
                            length: u64::try_from(framed_len).unwrap_or(u64::MAX),
                            limit: u64::try_from(MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES)
                                .unwrap_or(u64::MAX),
                        });
                    }
                    return Ok(Some(norito::DecodeLimits::new(
                        MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES,
                        MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES,
                        MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES,
                        4 * MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES,
                        64,
                    )));
                }
                Ok(None)
            }
            NETWORK_MESSAGE_LANE_DRAIN_VOTE_TAG => {
                if framed_len > MAX_LANE_DRAIN_VOTE_WIRE_BYTES {
                    return Err(norito::core::Error::ArchiveLengthExceeded {
                        length: u64::try_from(framed_len).unwrap_or(u64::MAX),
                        limit: u64::try_from(MAX_LANE_DRAIN_VOTE_WIRE_BYTES).unwrap_or(u64::MAX),
                    });
                }
                Ok(Some(norito::DecodeLimits::new(
                    lane_consensus::MAX_LANE_BLOCK_VALIDATORS,
                    MAX_LANE_DRAIN_VOTE_WIRE_BYTES,
                    MAX_LANE_DRAIN_VOTE_DECODE_ELEMENTS,
                    MAX_LANE_DRAIN_VOTE_DECODE_ALLOCATED_BYTES,
                    MAX_LANE_DRAIN_VOTE_DECODE_DEPTH,
                )))
            }
            NETWORK_MESSAGE_TORII_PROXY_REQUEST_TAG => {
                use torii_proxy::{
                    TORII_PROXY_REQUEST_MAX_DECODE_ALLOCATED_BYTES_V1,
                    TORII_PROXY_REQUEST_MAX_FRAME_BYTES_V1,
                };
                if framed_len > TORII_PROXY_REQUEST_MAX_FRAME_BYTES_V1 {
                    return Err(norito::core::Error::ArchiveLengthExceeded {
                        length: u64::try_from(framed_len).unwrap_or(u64::MAX),
                        limit: u64::try_from(TORII_PROXY_REQUEST_MAX_FRAME_BYTES_V1)
                            .unwrap_or(u64::MAX),
                    });
                }
                Ok(Some(norito::DecodeLimits::new(
                    TORII_PROXY_REQUEST_MAX_FRAME_BYTES_V1,
                    TORII_PROXY_REQUEST_MAX_FRAME_BYTES_V1,
                    TORII_PROXY_REQUEST_MAX_FRAME_BYTES_V1,
                    TORII_PROXY_REQUEST_MAX_DECODE_ALLOCATED_BYTES_V1,
                    64,
                )))
            }
            NETWORK_MESSAGE_TORII_PROXY_RESPONSE_TAG => {
                use torii_proxy::{
                    TORII_PROXY_RESPONSE_MAX_DECODE_ALLOCATED_BYTES_V1,
                    TORII_PROXY_RESPONSE_MAX_FRAME_BYTES_V1,
                };
                if framed_len > TORII_PROXY_RESPONSE_MAX_FRAME_BYTES_V1 {
                    return Err(norito::core::Error::ArchiveLengthExceeded {
                        length: u64::try_from(framed_len).unwrap_or(u64::MAX),
                        limit: u64::try_from(TORII_PROXY_RESPONSE_MAX_FRAME_BYTES_V1)
                            .unwrap_or(u64::MAX),
                    });
                }
                Ok(Some(norito::DecodeLimits::new(
                    TORII_PROXY_RESPONSE_MAX_FRAME_BYTES_V1,
                    TORII_PROXY_RESPONSE_MAX_FRAME_BYTES_V1,
                    TORII_PROXY_RESPONSE_MAX_FRAME_BYTES_V1,
                    TORII_PROXY_RESPONSE_MAX_DECODE_ALLOCATED_BYTES_V1,
                    64,
                )))
            }
            NETWORK_MESSAGE_QUEUE_PLAN_ADMISSION_PUBLICATION_TAG => {
                const WIRE_OVERHEAD_BYTES: usize = 64 * 1024;
                const MAX_CERTIFICATE_BYTES: usize =
                    iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES;
                const MAX_WIRE_BYTES: usize = MAX_CERTIFICATE_BYTES + WIRE_OVERHEAD_BYTES;
                if framed_len > MAX_WIRE_BYTES {
                    return Err(norito::core::Error::ArchiveLengthExceeded {
                        length: u64::try_from(framed_len).unwrap_or(u64::MAX),
                        limit: u64::try_from(MAX_WIRE_BYTES).unwrap_or(u64::MAX),
                    });
                }
                Ok(Some(norito::DecodeLimits::new(
                    MAX_CERTIFICATE_BYTES,
                    MAX_CERTIFICATE_BYTES,
                    MAX_CERTIFICATE_BYTES,
                    MAX_WIRE_BYTES.saturating_mul(2),
                    16,
                )))
            }
            NETWORK_MESSAGE_QUEUE_PLAN_ADMISSION_CERTIFICATE_TAG => {
                let max_body = iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES;
                if framed_len > MAX_QUEUE_PLAN_ADMISSION_CERTIFICATE_WIRE_BYTES {
                    return Err(norito::core::Error::ArchiveLengthExceeded {
                        length: u64::try_from(framed_len).unwrap_or(u64::MAX),
                        limit: u64::try_from(MAX_QUEUE_PLAN_ADMISSION_CERTIFICATE_WIRE_BYTES)
                            .unwrap_or(u64::MAX),
                    });
                }
                Ok(Some(norito::DecodeLimits::new(
                    max_body,
                    MAX_QUEUE_PLAN_ADMISSION_CERTIFICATE_WIRE_BYTES,
                    MAX_QUEUE_PLAN_ADMISSION_CERTIFICATE_WIRE_BYTES,
                    8 * MAX_QUEUE_PLAN_ADMISSION_CERTIFICATE_WIRE_BYTES,
                    64,
                )))
            }
            _ => Ok(None),
        }
    }
    fn is_outbound_allowed(&self) -> bool {
        match self {
            Self::SumeragiBlock(message) => {
                message.as_ref().as_message().ensure_live_outbound().is_ok()
            }
            _ => true,
        }
    }
}
pub mod role {
    //! Module with extension for [`RoleId`] to be stored inside state.
    use super::*;
    use core::{fmt, str::FromStr};
    use derive_more::Constructor;
    use iroha_primitives::impl_as_dyn_key;
    use mv::json::JsonKeyCodec;
    use norito::json;
    /// [`RoleId`] with owner [`AccountId`] attached to it.
    #[derive(
        Debug,
        Clone,
        Constructor,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Decode,
        Encode,
        crate::json_macros::JsonDeserialize,
        crate::json_macros::JsonSerialize,
    )]
    pub struct RoleIdWithOwner {
        /// [`AccountId`] of the owner.
        pub account: AccountId,
        /// [`RoleId`]  of the given role.
        pub id: RoleId,
    }
    /// Reference to [`RoleIdWithOwner`].
    #[derive(Debug, Clone, Copy, Constructor, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct RoleIdWithOwnerRef<'role> {
        /// [`AccountId`] of the owner.
        pub account: &'role AccountId,
        /// [`RoleId`]  of the given role.
        pub role: &'role RoleId,
    }
    impl AsRoleIdWithOwnerRef for RoleIdWithOwner {
        fn as_key(&self) -> RoleIdWithOwnerRef<'_> {
            RoleIdWithOwnerRef {
                account: &self.account,
                role: &self.id,
            }
        }
    }
    impl_as_dyn_key! {
        target: RoleIdWithOwner,
        key: RoleIdWithOwnerRef<'_>,
        trait: AsRoleIdWithOwnerRef
    }
    impl fmt::Display for RoleIdWithOwner {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}|{}", self.account, self.id)
        }
    }
    impl FromStr for RoleIdWithOwner {
        type Err = iroha_data_model::ParseError;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            const SEPARATOR: char = '|';
            let (account_raw, role_raw) =
                s.split_once(SEPARATOR)
                    .ok_or(iroha_data_model::ParseError::new(
                        "RoleIdWithOwner must be formatted as `account|role`",
                    ))?;
            let account = AccountId::parse_encoded(account_raw).map_err(|_| {
                iroha_data_model::ParseError::new("Invalid account component in RoleIdWithOwner")
            })?;
            let id = role_raw.parse().map_err(|_| {
                iroha_data_model::ParseError::new("Invalid role component in RoleIdWithOwner")
            })?;
            Ok(RoleIdWithOwner { account, id })
        }
    }
    impl JsonKeyCodec for RoleIdWithOwner {
        fn encode_json_key(&self, out: &mut String) {
            json::write_json_string(&self.to_string(), out);
        }
        fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
            encoded
                .parse::<RoleIdWithOwner>()
                .map_err(|err| json::Error::Message(err.to_string()))
        }
    }
}
// RoleIdWithOwner derives codec implementations in the role module above.
pub mod prelude {
    //! Re-exports important traits and types. Meant to be glob imported when using `Iroha`.
    #[doc(inline)]
    pub use crate::{
        oracle::{ObservationAdmission, OracleAggregator, aggregate, validate_connector_request},
        smartcontracts::ValidSingularQuery,
        state::{StateReadOnly, StateView, World, WorldReadOnly},
        tx::AcceptedTransaction,
    };
    #[doc(inline)]
    pub use iroha_crypto::{Algorithm, Hash, KeyPair, PrivateKey, PublicKey};
}
// These synthetic-state regressions need deliberately nonshipping validation
// or state-apply fixtures. Compile them inside the library test harness so
// ordinary `cargo test` keeps exercising them without exporting fixture
// authority from production builds.
#[cfg(test)]
extern crate self as iroha_core;
#[cfg(test)]
#[path = "../tests/admission_batching.rs"]
mod admission_batching_tests;
#[cfg(test)]
#[path = "../tests/adversarial_block_rejections.rs"]
mod adversarial_block_rejections_tests;
#[cfg(test)]
#[path = "../tests/bls_batch_pop.rs"]
mod bls_batch_pop_tests;
#[cfg(test)]
#[path = "../tests/event_ordering.rs"]
mod event_ordering_tests;
#[cfg(test)]
#[path = "../tests/execute_trigger_events.rs"]
mod execute_trigger_events_tests;
#[cfg(test)]
#[path = "../tests/isi_gas_fees.rs"]
mod isi_gas_fees_tests;
#[cfg(test)]
#[path = "../tests/ivm_corehost_axt.rs"]
mod ivm_corehost_axt_tests;
#[cfg(test)]
#[path = "../tests/overlay_chunking.rs"]
mod overlay_chunking_tests;
#[cfg(test)]
#[path = "../tests/overlay_workers_parity.rs"]
mod overlay_workers_parity_tests;
#[cfg(test)]
#[path = "../tests/parallel_apply_knob.rs"]
mod parallel_apply_knob_tests;
#[cfg(test)]
#[path = "../tests/parallel_apply.rs"]
mod parallel_apply_tests;
#[cfg(test)]
#[path = "../tests/pipeline_warning_event.rs"]
mod pipeline_warning_event_tests;
#[cfg(test)]
#[path = "../tests/scheduler_gpu_key_bucket_parity.rs"]
mod scheduler_gpu_key_bucket_parity_tests;
#[cfg(test)]
#[path = "../tests/scheduler_ready_queue_heap_parity.rs"]
mod scheduler_ready_queue_heap_parity_tests;
#[cfg(test)]
#[path = "../tests/scheduler_telemetry.rs"]
mod scheduler_telemetry_tests;
#[cfg(test)]
#[path = "../tests/signature_batch_determinism.rs"]
mod signature_batch_determinism_tests;
#[cfg(test)]
#[path = "../tests/snapshots.rs"]
mod synthetic_state_snapshots;
#[cfg(test)]
#[path = "../tests/validation_fee_admission.rs"]
mod validation_fee_admission_tests;
#[cfg(test)]
mod tests {
    use crate::{
        MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES, MAX_LANE_DRAIN_VOTE_WIRE_BYTES,
        NetworkMessage, PeerTrustGossip, PeersGossip,
        gossiper::{GossipPlane, GossipRoute, GossipTransaction, TransactionGossip},
        queue::{RoutingDecision, RoutingPlan},
        role::RoleIdWithOwner,
        sumeragi::message::{
            BlockMessage, BlockMessageWire, KURA_REPLICA_ADVERT_VERSION_V1, KuraReplicaAdvertV1,
        },
        torii_proxy::{
            QUEUE_PLAN_ADMISSION_PUBLICATION_VERSION_V1, QueuePlanAdmissionPublicationV1,
            TORII_PROXY_NETWORK_MESSAGE_OVERHEAD_BYTES_V1,
            TORII_PROXY_REQUEST_MAX_DECODE_ALLOCATED_BYTES_V1,
            TORII_PROXY_REQUEST_MAX_ENCODED_BYTES_V1, TORII_PROXY_REQUEST_MAX_FRAME_BYTES_V1,
            TORII_PROXY_REQUEST_VERSION_V1, TORII_PROXY_RESPONSE_MAX_ENCODED_BYTES_V1,
            TORII_PROXY_RESPONSE_MAX_FRAME_BYTES_V1, TORII_PROXY_RESPONSE_VERSION_V1,
            ToriiProxyHttpResponseV1, ToriiProxyRequestKindV1, ToriiProxyRequestV1,
            ToriiProxyResponseFormatV1, ToriiProxyResponseV1, ToriiProxyTransactionAdmissionV1,
            ToriiReadEndpointV1, ToriiReadProxyRequestV1, ToriiRouteHintV1, ToriiRoutingPlanHintV1,
        },
    };
    use iroha_crypto::{Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::block::BlockHeader;
    use iroha_data_model::nexus::{DataSpaceId, LaneId};
    use iroha_data_model::peer::PeerId;
    use iroha_data_model::role::RoleId;
    use iroha_data_model::transaction::{TransactionBuilder, TransactionEntrypoint};
    use iroha_data_model::{Level, NetworkId, isi::Log};
    use iroha_p2p::{
        ClassifyTopic,
        network::message::{SubscriberRoute, Topic as NetworkTopic},
    };
    use iroha_test_samples::gen_account_in;
    use norito::{codec::Encode, core as ncore};
    use std::{cmp::Ordering, collections::BTreeMap, num::NonZeroU64, sync::Arc, time::Duration};
    fn test_network_id(label: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            label,
        )))
    }
    fn checked_topic_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked network topic keypair")
    }
    fn raw_network_topic(message: &NetworkMessage) -> NetworkTopic {
        let encoded = ncore::to_bytes(message).expect("encode raw-topic fixture");
        let view = ncore::from_bytes_view(&encoded).expect("inspect raw-topic fixture");
        <NetworkMessage as ClassifyTopic>::inbound_topic(view.as_bytes(), view.flags())
            .expect("classify well-formed raw network payload")
            .expect("core network messages have a total raw classifier")
    }
    fn raw_network_tag(message: &NetworkMessage) -> u32 {
        let encoded = ncore::to_bytes(message).expect("encode raw-tag fixture");
        let view = ncore::from_bytes_view(&encoded).expect("inspect raw-tag fixture");
        super::inbound_enum_parts(view.as_bytes())
            .expect("extract core network-message discriminant")
            .0
    }
    fn raw_sumeragi_topic_for_synthetic_tag(tag: u32) -> Result<NetworkTopic, ncore::Error> {
        use iroha_data_model::block::consensus_v2 as wire;
        let (mut payload, flags) = norito::codec::encode_with_header_flags(&BlockMessage::V2(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(
                wire::PayloadChunk {
                    manifest_hash: HashOf::from_untyped_unchecked(Hash::new(
                        b"synthetic-topic-manifest",
                    )),
                    index: 0,
                    bytes: vec![0xA5],
                    sender: 0,
                    signature: vec![0x5A],
                },
            )),
        ));
        payload
            .get_mut(..core::mem::size_of::<u32>())
            .ok_or(ncore::Error::LengthMismatch)?
            .copy_from_slice(&tag.to_le_bytes());
        let framed = ncore::frame_bare_with_header_flags::<BlockMessage>(&payload, flags)?;
        super::inbound_sumeragi_topic(&framed)
    }
    #[test]
    fn network_topic_fixture_uses_checked_ed25519_keypair() {
        let keypair = checked_topic_keypair();
        assert_eq!(
            keypair
                .public_key()
                .try_algorithm()
                .expect("checked topic fixture key algorithm"),
            iroha_crypto::Algorithm::Ed25519
        );
    }
    fn canonical_signed_transaction_payload(
        signed: &iroha_data_model::transaction::SignedTransaction,
    ) -> Arc<Vec<u8>> {
        Arc::new(
            ncore::to_bytes(
                &iroha_data_model::transaction::TransactionEntrypoint::External(signed.clone()),
            )
            .expect("encode signed transaction entrypoint"),
        )
    }
    #[test]
    fn trust_gossip_classifies_to_trust_topic() {
        let gossip = PeerTrustGossip {
            network_id: test_network_id(b"trust-gossip-topic"),
            trust: Vec::new(),
        };
        let msg = NetworkMessage::PeerTrustGossip(Box::new(gossip));
        assert!(matches!(
            msg.topic(),
            iroha_p2p::network::message::Topic::TrustGossip
        ));
    }
    #[test]
    fn first_release_network_tags_are_contiguous() {
        use iroha_primitives::unique_vec::UniqueVec;
        use iroha_torii_shared::connect::{ConnectP2pMessageV1, ConnectSessionTerminatedV1};
        use norito::streaming::{ControlErrorFrame, ErrorCode};
        let fixtures = vec![
            (
                NetworkMessage::PeersGossiper(Box::new(PeersGossip {
                    peers: UniqueVec::new(),
                    peer_capabilities: BTreeMap::new(),
                })),
                7,
                NetworkTopic::PeerGossip,
                SubscriberRoute::General,
            ),
            (
                NetworkMessage::PeerTrustGossip(Box::new(PeerTrustGossip {
                    network_id: test_network_id(b"trust-gossip-wire-tag"),
                    trust: Vec::new(),
                })),
                8,
                NetworkTopic::TrustGossip,
                SubscriberRoute::General,
            ),
            (
                NetworkMessage::Health,
                9,
                NetworkTopic::Health,
                SubscriberRoute::General,
            ),
            (
                NetworkMessage::TimePing(Box::new(crate::time::TimePing { id: 1, t1_ms: 2 })),
                10,
                NetworkTopic::Health,
                SubscriberRoute::General,
            ),
            (
                NetworkMessage::TimePong(Box::new(crate::time::TimePong {
                    id: 1,
                    t2_ms: 2,
                    t3_ms: 3,
                })),
                11,
                NetworkTopic::Health,
                SubscriberRoute::General,
            ),
            (
                NetworkMessage::Connect(Box::new(ConnectP2pMessageV1::SessionTerminated(
                    ConnectSessionTerminatedV1 {
                        sid: [0x14; 32],
                        reason: "closed".to_owned(),
                    },
                ))),
                12,
                NetworkTopic::Health,
                SubscriberRoute::Connect,
            ),
            (
                NetworkMessage::StreamingControl(Box::new(norito::streaming::ControlFrame::Error(
                    ControlErrorFrame {
                        code: ErrorCode::ProtocolViolation,
                        message: "invalid frame".to_owned(),
                    },
                ))),
                15,
                NetworkTopic::Control,
                SubscriberRoute::General,
            ),
        ];
        for (message, expected_tag, expected_topic, expected_route) in fixtures {
            assert_eq!(raw_network_tag(&message), expected_tag);
            assert_eq!(message.topic(), expected_topic);
            assert_eq!(raw_network_topic(&message), expected_topic);
            assert_eq!(message.subscriber_route(), expected_route);
        }
    }
    #[test]
    fn role_id_with_owner_parse_roundtrip() {
        let (account, _keypair) = gen_account_in("wonderland");
        let role: RoleId = "auditor".parse().expect("valid role id");
        let rid = RoleIdWithOwner {
            account: account.clone(),
            id: role.clone(),
        };
        let encoded = rid.to_string();
        let decoded: RoleIdWithOwner = encoded.parse().expect("roundtrip");
        assert_eq!(decoded.account.subject_id(), account.subject_id());
        assert_eq!(decoded.id, role);
    }
    #[test]
    fn network_message_decode_from_slice_roundtrip() {
        let message = NetworkMessage::Health;
        let bytes = norito::to_bytes(&message).expect("encode network message");
        let view = norito::core::from_bytes_view(&bytes).expect("archive view");
        let decoded: NetworkMessage = view.decode().expect("decode network message");
        assert!(matches!(decoded, NetworkMessage::Health));
        assert_eq!(raw_network_topic(&message), NetworkTopic::Health);
    }
    #[test]
    fn raw_network_topic_is_total_for_restricted_gossip_and_fails_closed_on_unknown_layouts() {
        #[derive(Encode)]
        enum SingleFieldNetworkMessage {
            Field(u8),
        }
        let restricted = NetworkMessage::TransactionGossiper(Arc::new(TransactionGossip {
            txs: Vec::new(),
            routes: Vec::new(),
            plans: Vec::new(),
            plane: GossipPlane::Restricted,
        }));
        assert_eq!(
            raw_network_topic(&restricted),
            NetworkTopic::TxGossipRestricted
        );
        for (tag, expected) in [
            (1_u32, NetworkTopic::Consensus),
            (2, NetworkTopic::Consensus),
            (3, NetworkTopic::Consensus),
            (5, NetworkTopic::Consensus),
            (7, NetworkTopic::PeerGossip),
            (8, NetworkTopic::TrustGossip),
            (10, NetworkTopic::Health),
            (11, NetworkTopic::Health),
            (12, NetworkTopic::Health),
            (13, NetworkTopic::Control),
            (14, NetworkTopic::Control),
            (15, NetworkTopic::Control),
            (16, NetworkTopic::Control),
            (17, NetworkTopic::Consensus),
        ] {
            let (mut payload, flags) =
                norito::codec::encode_with_header_flags(&SingleFieldNetworkMessage::Field(0));
            payload[..core::mem::size_of::<u32>()].copy_from_slice(&tag.to_le_bytes());
            let classified = <NetworkMessage as ClassifyTopic>::inbound_topic(&payload, flags)
                .expect("classify explicit first-release tag");
            assert_eq!(
                classified,
                Some(expected),
                "wire tag {tag} must retain its exact first-release transport class"
            );
        }
        let flags = ncore::default_encode_flags();
        assert!(
            <NetworkMessage as ClassifyTopic>::inbound_topic(&18_u32.to_le_bytes(), flags).is_err(),
            "the first tag after the compact first-release range must fail before typed decode"
        );
        assert!(
            <NetworkMessage as ClassifyTopic>::inbound_topic(&99_u32.to_le_bytes(), flags).is_err(),
            "unknown network-message tags must fail before typed decode"
        );
        let mut trailing_health = 9_u32.to_le_bytes().to_vec();
        trailing_health.push(0);
        assert!(
            <NetworkMessage as ClassifyTopic>::inbound_topic(&trailing_health, flags).is_err(),
            "the unit health tag must not hide a trailing dynamic payload"
        );
    }
    #[test]
    fn raw_block_message_tags_keep_exact_capacity_classes() {
        for tag in [0, 1, 3, 4, 5, 6, 7, 8] {
            assert_eq!(
                raw_sumeragi_topic_for_synthetic_tag(tag).expect("classify lane control tag"),
                NetworkTopic::Consensus,
                "block-message control discriminant {tag} must stay on reliable consensus transport"
            );
        }
        for tag in [2, 9] {
            assert_eq!(
                raw_sumeragi_topic_for_synthetic_tag(tag).expect("classify lane payload tag"),
                NetworkTopic::ConsensusPayload,
                "lane payload discriminant {tag} must use the bounded payload corridor"
            );
        }
        assert_eq!(
            raw_sumeragi_topic_for_synthetic_tag(10)
                .expect("classify canonical global-v2 safety message"),
            NetworkTopic::ConsensusSafety,
            "global-v2 discriminant must preserve its inner protocol topic"
        );
        assert!(
            raw_sumeragi_topic_for_synthetic_tag(11).is_err(),
            "the first tag after the compact block-message range must fail closed"
        );
    }
    #[test]
    fn raw_consensus_struct_parser_accepts_each_advertised_packed_layout() {
        #[derive(Encode)]
        struct TwoFieldFixture {
            version: u16,
            payload: PayloadFixture,
        }
        #[derive(Encode)]
        enum PayloadFixture {
            Safety(u8),
        }
        let fixture = TwoFieldFixture {
            version: 3,
            payload: PayloadFixture::Safety(7),
        };
        for requested in [
            0,
            ncore::header_flags::PACKED_STRUCT,
            ncore::header_flags::PACKED_STRUCT
                | ncore::header_flags::COMPACT_LEN
                | ncore::header_flags::FIELD_BITSET,
        ] {
            let (bare, flags) = {
                let _guard = ncore::DecodeFlagsGuard::enter(requested);
                norito::codec::encode_with_header_flags(&fixture)
            };
            assert_eq!(
                flags
                    & (ncore::header_flags::PACKED_STRUCT
                        | ncore::header_flags::COMPACT_LEN
                        | ncore::header_flags::FIELD_BITSET),
                requested,
                "fixture must advertise the layout under test"
            );
            let (version, payload) =
                super::inbound_two_field_struct(&bare, flags, core::mem::size_of::<u16>())
                    .expect("extract two-field consensus layout");
            assert_eq!(version, 3_u16.to_le_bytes());
            let (tag, remaining) = super::inbound_enum_parts(payload).expect("payload enum tag");
            assert_eq!(tag, 0);
            let field = if flags & ncore::header_flags::PACKED_STRUCT == 0 {
                super::inbound_enum_field(remaining, flags).expect("length-prefixed enum field")
            } else {
                remaining
            };
            assert_eq!(field, [7]);
        }
    }
    #[test]
    fn lane_drain_vote_network_message_roundtrips_on_control_topic() {
        use iroha_crypto::{Algorithm, HashOf};
        use iroha_data_model::{
            consensus::VALIDATOR_SET_HASH_VERSION_V1,
            merge::{LaneDrainCertificateBodyV1, LaneDrainIntentV1},
        };
        let keypair = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("generate lane-drain BLS fixture keypair");
        let signer = PeerId::new(keypair.public_key().clone());
        let validator_set = vec![signer.clone()];
        let body = LaneDrainCertificateBodyV1 {
            version: 1,
            intent: LaneDrainIntentV1 {
                version: 1,
                network_id: test_network_id(b"lane-drain-network-genesis"),
                lane_id: LaneId::new(3),
                dataspace_id: DataSpaceId::new(7),
                lane_incarnation: Hash::new(b"lane-drain-network-incarnation"),
                close_global_height: 12,
                initial_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                    LaneId::new(3),
                    DataSpaceId::new(7),
                    Hash::new(b"lane-drain-network-incarnation"),
                    4,
                    Some(Hash::new(b"lane-drain-network-initial")),
                ),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set,
                validator_count: 1,
                min_quorum: 1,
            },
            final_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                LaneId::new(3),
                DataSpaceId::new(7),
                Hash::new(b"lane-drain-network-incarnation"),
                5,
                Some(Hash::new(b"lane-drain-network-final")),
            ),
        };
        let vote =
            crate::lane_consensus::LaneDrainVoteV1::new_signed(body, signer, keypair.private_key())
                .expect("sign valid lane-drain vote");
        let message = NetworkMessage::LaneDrainVote(Box::new(vote.clone()));
        assert_eq!(
            message.topic(),
            NetworkTopic::Consensus,
            "lane-drain traffic must not share the authoritative v2 safety topic"
        );
        assert_eq!(raw_network_tag(&message), 3);
        let encoded = norito::to_bytes(&message).expect("encode lane-drain vote message");
        let decoded = norito::decode_from_bytes::<NetworkMessage>(&encoded)
            .expect("decode lane-drain vote message");
        let NetworkMessage::LaneDrainVote(decoded_vote) = decoded else {
            panic!("decoded the wrong network-message variant");
        };
        assert_eq!(*decoded_vote, vote);
        decoded_vote
            .validate_ingress()
            .expect("round-tripped vote retains its signature and proof of possession");
    }
    #[test]
    fn maximum_committee_lane_drain_vote_fits_the_ingress_wire_cap() {
        use iroha_crypto::{Algorithm, HashOf};
        use iroha_data_model::{
            consensus::VALIDATOR_SET_HASH_VERSION_V1,
            merge::{LaneDrainCertificateBodyV1, LaneDrainIntentV1},
        };
        let keypairs = (0..crate::lane_consensus::MAX_LANE_BLOCK_VALIDATORS)
            .map(|index| {
                let seed = u8::try_from(index + 1).expect("fixture index fits in u8");
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("derive maximum-committee BLS fixture keypair")
            })
            .collect::<Vec<_>>();
        let signer = PeerId::new(keypairs[0].public_key().clone());
        let origin = signer.clone();
        let mut validator_set = keypairs
            .iter()
            .map(|keypair| PeerId::new(keypair.public_key().clone()))
            .collect::<Vec<_>>();
        validator_set.sort();
        let validator_count =
            u32::try_from(validator_set.len()).expect("maximum committee count fits u32");
        let min_quorum = u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
            validator_set.len(),
        ))
        .expect("maximum committee quorum fits u32");
        let body = LaneDrainCertificateBodyV1 {
            version: 1,
            intent: LaneDrainIntentV1 {
                version: 1,
                network_id: test_network_id(b"maximum-lane-drain-network-genesis"),
                lane_id: LaneId::new(3),
                dataspace_id: DataSpaceId::new(7),
                lane_incarnation: Hash::new(b"maximum-lane-drain-network-incarnation"),
                close_global_height: 12,
                initial_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                    LaneId::new(3),
                    DataSpaceId::new(7),
                    Hash::new(b"maximum-lane-drain-network-incarnation"),
                    4,
                    Some(Hash::new(b"maximum-lane-drain-network-initial")),
                ),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set,
                validator_count,
                min_quorum,
            },
            final_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                LaneId::new(3),
                DataSpaceId::new(7),
                Hash::new(b"maximum-lane-drain-network-incarnation"),
                5,
                Some(Hash::new(b"maximum-lane-drain-network-final")),
            ),
        };
        let vote = crate::lane_consensus::LaneDrainVoteV1::new_signed(
            body,
            signer,
            keypairs[0].private_key(),
        )
        .expect("sign maximum-committee drain vote");
        let message = NetworkMessage::LaneDrainVote(Box::new(vote));
        let encoded = norito::to_bytes(&message).expect("encode maximum-committee lane-drain vote");
        let p2p_wire_len = iroha_p2p::network::data_frame_wire_len(&origin, None, &message);
        assert!(
            p2p_wire_len <= MAX_LANE_DRAIN_VOTE_WIRE_BYTES,
            "largest valid lane-drain vote P2P frame encoded to {p2p_wire_len} bytes, above the {}-byte ingress cap",
            MAX_LANE_DRAIN_VOTE_WIRE_BYTES
        );
        let view = ncore::from_bytes_view(&encoded).expect("inspect encoded network message");
        let limits = <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
            view.as_bytes(),
            p2p_wire_len,
            view.flags(),
        )
        .expect("select lane-drain decode policy")
        .expect("lane-drain variant must install decode limits");
        let decoded = ncore::decode_from_bytes_with_limits::<NetworkMessage>(&encoded, limits)
            .expect("maximum valid lane-drain vote must pass the inbound resource limits");
        assert!(matches!(decoded, NetworkMessage::LaneDrainVote(_)));
    }
    #[test]
    fn lane_drain_vote_with_excess_committee_hits_predecode_sequence_limit() {
        use iroha_crypto::{Algorithm, HashOf};
        use iroha_data_model::{
            consensus::VALIDATOR_SET_HASH_VERSION_V1,
            merge::{LaneDrainCertificateBodyV1, LaneDrainIntentV1},
        };
        let keypair = KeyPair::try_from_seed(vec![211; 32], Algorithm::BlsNormal)
            .expect("derive adversarial lane-drain BLS fixture keypair");
        let signer = PeerId::new(keypair.public_key().clone());
        let origin = signer.clone();
        let validator_set =
            vec![signer.clone(); crate::lane_consensus::MAX_LANE_BLOCK_VALIDATORS + 1];
        let validator_count =
            u32::try_from(validator_set.len()).expect("adversarial committee count fits u32");
        let vote = crate::lane_consensus::LaneDrainVoteV1 {
            body: LaneDrainCertificateBodyV1 {
                version: 1,
                intent: LaneDrainIntentV1 {
                    version: 1,
                    network_id: test_network_id(b"excess-lane-drain-network-genesis"),
                    lane_id: LaneId::new(3),
                    dataspace_id: DataSpaceId::new(7),
                    lane_incarnation: Hash::new(b"excess-lane-drain-network-incarnation"),
                    close_global_height: 12,
                    initial_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                        LaneId::new(3),
                        DataSpaceId::new(7),
                        Hash::new(b"excess-lane-drain-network-incarnation"),
                        4,
                        Some(Hash::new(b"excess-lane-drain-network-initial")),
                    ),
                    validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash: HashOf::new(&validator_set),
                    validator_set,
                    validator_count,
                    min_quorum: 1,
                },
                final_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                    LaneId::new(3),
                    DataSpaceId::new(7),
                    Hash::new(b"excess-lane-drain-network-incarnation"),
                    5,
                    Some(Hash::new(b"excess-lane-drain-network-final")),
                ),
            },
            signer,
            proof_of_possession: vec![0; crate::lane_consensus::LANE_BLS_PROOF_BYTES],
            bls_signature: vec![0; crate::lane_consensus::LANE_BLS_PROOF_BYTES],
        };
        let message = NetworkMessage::LaneDrainVote(Box::new(vote));
        let encoded = norito::to_bytes(&message).expect("encode adversarial lane-drain vote");
        let p2p_wire_len = iroha_p2p::network::data_frame_wire_len(&origin, None, &message);
        assert!(
            p2p_wire_len <= MAX_LANE_DRAIN_VOTE_WIRE_BYTES,
            "fixture must exercise the nested limit instead of the frame cap"
        );
        assert!(
            matches!(
                norito::decode_from_bytes::<NetworkMessage>(&encoded),
                Ok(NetworkMessage::LaneDrainVote(_))
            ),
            "the adversarial archive must be syntactically decodable without limits"
        );
        let view = ncore::from_bytes_view(&encoded).expect("inspect adversarial network message");
        let limits = <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
            view.as_bytes(),
            p2p_wire_len,
            view.flags(),
        )
        .expect("select lane-drain decode policy")
        .expect("lane-drain variant must install decode limits");
        let error = ncore::decode_from_bytes_with_limits::<NetworkMessage>(&encoded, limits)
            .expect_err("committee above the protocol cap must fail before allocation");
        assert!(
            matches!(
                &error,
                ncore::Error::SequenceLengthExceeded {
                    length,
                    limit
                } if *length == u64::from(validator_count)
                    && *limit == crate::lane_consensus::MAX_LANE_BLOCK_VALIDATORS as u64
            ),
            "unexpected bounded-decode rejection: {error:?}"
        );
    }

    #[test]
    fn native_amx_and_lane_relay_fall_back_to_canonical_global_decode_limits() {
        for (tag, label) in [(2_u32, "LaneRelay"), (6_u32, "NativeAmx")] {
            let payload = tag.to_le_bytes();
            assert!(
                <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
                    &payload,
                    payload.len(),
                    ncore::default_encode_flags(),
                )
                .unwrap_or_else(|error| panic!("derive {label} decode policy: {error}"))
                .is_none(),
                "{label} intentionally uses Norito's canonical payload-derived global limits"
            );
        }
        let canonical = norito::canonical_decode_limits(4 * 1024);
        assert_eq!(
            canonical.max_nesting_depth(),
            norito::core::MAX_OWNED_VALUE_DECODE_DEPTH
        );
        assert!(canonical.max_total_allocated_bytes() > 4 * 1024);
    }
    #[test]
    fn oversized_lane_drain_vote_frame_is_rejected_by_raw_policy() {
        use iroha_crypto::{Algorithm, HashOf};
        use iroha_data_model::{
            consensus::VALIDATOR_SET_HASH_VERSION_V1,
            merge::{LaneDrainCertificateBodyV1, LaneDrainIntentV1},
        };
        let keypair = KeyPair::try_from_seed(vec![212; 32], Algorithm::BlsNormal)
            .expect("derive oversized lane-drain BLS fixture keypair");
        let signer = PeerId::new(keypair.public_key().clone());
        let origin = signer.clone();
        let validator_set = vec![signer.clone()];
        let vote = crate::lane_consensus::LaneDrainVoteV1 {
            body: LaneDrainCertificateBodyV1 {
                version: 1,
                intent: LaneDrainIntentV1 {
                    version: 1,
                    network_id: test_network_id(b"oversized-lane-drain-network-genesis"),
                    lane_id: LaneId::new(3),
                    dataspace_id: DataSpaceId::new(7),
                    lane_incarnation: Hash::new(b"oversized-lane-drain-network-incarnation"),
                    close_global_height: 12,
                    initial_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                        LaneId::new(3),
                        DataSpaceId::new(7),
                        Hash::new(b"oversized-lane-drain-network-incarnation"),
                        4,
                        Some(Hash::new(b"oversized-lane-drain-network-initial")),
                    ),
                    validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash: HashOf::new(&validator_set),
                    validator_set,
                    validator_count: 1,
                    min_quorum: 1,
                },
                final_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                    LaneId::new(3),
                    DataSpaceId::new(7),
                    Hash::new(b"oversized-lane-drain-network-incarnation"),
                    5,
                    Some(Hash::new(b"oversized-lane-drain-network-final")),
                ),
            },
            signer,
            proof_of_possession: vec![0; MAX_LANE_DRAIN_VOTE_WIRE_BYTES],
            bls_signature: vec![0; crate::lane_consensus::LANE_BLS_PROOF_BYTES],
        };
        let message = NetworkMessage::LaneDrainVote(Box::new(vote));
        let encoded = norito::to_bytes(&message).expect("encode oversized lane-drain vote");
        let p2p_wire_len = iroha_p2p::network::data_frame_wire_len(&origin, None, &message);
        assert!(p2p_wire_len > MAX_LANE_DRAIN_VOTE_WIRE_BYTES);
        let view = ncore::from_bytes_view(&encoded).expect("inspect oversized network message");
        let error = <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
            view.as_bytes(),
            p2p_wire_len,
            view.flags(),
        )
        .expect_err("oversized lane-drain frame must fail before typed decode");
        assert!(matches!(
            error,
            ncore::Error::ArchiveLengthExceeded { length, limit }
                if length == p2p_wire_len as u64
                    && limit == MAX_LANE_DRAIN_VOTE_WIRE_BYTES as u64
        ));
    }
    #[test]
    fn certified_merge_sidecar_messages_roundtrip_on_bounded_consensus_topics() {
        use crate::merge_sidecar::{
            CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarChunkV1,
            CertifiedMergeSidecarCloseAckV1, CertifiedMergeSidecarCloseV1,
            CertifiedMergeSidecarGenerationHintV1, CertifiedMergeSidecarMessage,
            CertifiedMergeSidecarRequestV1, CertifiedMergeSidecarSemanticSequenceV1,
            CertifiedMergeSidecarServiceGenerationV1, CertifiedMergeSidecarStreamEpochV1,
        };
        use iroha_data_model::merge::MergeLedgerEntry;
        #[derive(Encode)]
        enum LegacySidecarCarrier {
            Payload(Box<CertifiedMergeSidecarMessage>),
        }
        #[derive(Encode)]
        enum SharedSidecarCarrier {
            Payload(Arc<CertifiedMergeSidecarMessage>),
        }
        let assert_shared_carrier_wire_compatible = |message: &CertifiedMergeSidecarMessage| {
            let legacy = LegacySidecarCarrier::Payload(Box::new(message.clone()));
            let shared = SharedSidecarCarrier::Payload(Arc::new(message.clone()));
            assert_eq!(
                legacy.encode(),
                shared.encode(),
                "Box-to-Arc carrier conversion must not alter canonical Norito bytes"
            );
        };
        let requester = PeerId::new(checked_topic_keypair().public_key().clone());
        let responder = PeerId::new(checked_topic_keypair().public_key().clone());
        let entry_hash = iroha_crypto::HashOf::<MergeLedgerEntry>::from_untyped_unchecked(
            Hash::new(b"merge-sidecar-entry"),
        );
        let stream_epoch = CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(1).expect("sidecar fixture stream epoch is non-zero"),
        );
        let service_generation = CertifiedMergeSidecarServiceGenerationV1::INITIAL;
        let mut request = CertifiedMergeSidecarRequestV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation,
            stream_epoch,
            semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1(
                NonZeroU64::new(1).expect("sidecar semantic sequence is non-zero"),
            ),
            closed_through: 0,
            request_id: Hash::prehashed([0; Hash::LENGTH]),
            entry_hash,
            encoded_len: 3,
            epoch_id: 4,
            reference_digest: Hash::new(b"merge-sidecar-reference"),
            requester: requester.clone(),
            responder: responder.clone(),
        };
        request.request_id = request.canonical_request_id();
        let request_message = NetworkMessage::CertifiedMergeSidecar(Arc::new(
            CertifiedMergeSidecarMessage::Request(request.clone()),
        ));
        let NetworkMessage::CertifiedMergeSidecar(request_payload) = &request_message else {
            unreachable!("request fixture uses the sidecar variant")
        };
        assert_shared_carrier_wire_compatible(request_payload.as_ref());
        assert_eq!(request_message.topic(), NetworkTopic::Consensus);
        assert_eq!(raw_network_tag(&request_message), 4);
        assert_eq!(raw_network_topic(&request_message), NetworkTopic::Consensus);
        let request_hash = HashOf::new(&request_message);
        let encoded = norito::to_bytes(&request_message).expect("encode sidecar request");
        let decoded =
            norito::decode_from_bytes::<NetworkMessage>(&encoded).expect("decode sidecar request");
        assert_eq!(HashOf::new(&decoded), request_hash);
        let NetworkMessage::CertifiedMergeSidecar(message) = decoded else {
            panic!("decoded sidecar request uses the sidecar variant");
        };
        let CertifiedMergeSidecarMessage::Request(decoded_request) = message.as_ref() else {
            panic!("decoded sidecar request preserves the request variant");
        };
        assert_eq!(decoded_request.service_generation, service_generation);
        assert_eq!(decoded_request.stream_epoch, stream_epoch);
        assert_eq!(decoded_request, &request);
        let mut close = CertifiedMergeSidecarCloseV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation,
            stream_epoch,
            closed_through: request.semantic_sequence.get(),
            close_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: requester.clone(),
            responder: responder.clone(),
        };
        close.close_id = close.canonical_close_id();
        let close_message = NetworkMessage::CertifiedMergeSidecar(Arc::new(
            CertifiedMergeSidecarMessage::Close(close.clone()),
        ));
        assert_eq!(close_message.topic(), NetworkTopic::Consensus);
        assert_eq!(raw_network_topic(&close_message), NetworkTopic::Consensus);
        let encoded = norito::to_bytes(&close_message).expect("encode sidecar close");
        let decoded =
            norito::decode_from_bytes::<NetworkMessage>(&encoded).expect("decode sidecar close");
        let NetworkMessage::CertifiedMergeSidecar(message) = decoded else {
            panic!("decoded sidecar close uses the sidecar variant");
        };
        let CertifiedMergeSidecarMessage::Close(decoded_close) = message.as_ref() else {
            panic!("decoded sidecar close preserves the close variant");
        };
        assert_eq!(decoded_close.service_generation, service_generation);
        assert_eq!(decoded_close.stream_epoch, stream_epoch);
        assert_eq!(decoded_close, &close);
        let close_ack = CertifiedMergeSidecarCloseAckV1 {
            version: close.version,
            service_generation: close.service_generation,
            stream_epoch: close.stream_epoch,
            closed_through: close.closed_through,
            close_id: close.close_id,
            requester: requester.clone(),
            responder: responder.clone(),
        };
        let close_ack_message = NetworkMessage::CertifiedMergeSidecar(Arc::new(
            CertifiedMergeSidecarMessage::CloseAck(close_ack.clone()),
        ));
        assert_eq!(close_ack_message.topic(), NetworkTopic::Consensus);
        assert_eq!(
            raw_network_topic(&close_ack_message),
            NetworkTopic::Consensus
        );
        let encoded = norito::to_bytes(&close_ack_message).expect("encode sidecar close ACK");
        let decoded = norito::decode_from_bytes::<NetworkMessage>(&encoded)
            .expect("decode sidecar close ACK");
        let NetworkMessage::CertifiedMergeSidecar(message) = decoded else {
            panic!("decoded sidecar close ACK uses the sidecar variant");
        };
        let CertifiedMergeSidecarMessage::CloseAck(decoded_close_ack) = message.as_ref() else {
            panic!("decoded sidecar close ACK preserves the close ACK variant");
        };
        assert_eq!(decoded_close_ack.service_generation, service_generation);
        assert_eq!(decoded_close_ack.stream_epoch, stream_epoch);
        assert_eq!(decoded_close_ack, &close_ack);
        let current_generation = CertifiedMergeSidecarServiceGenerationV1(
            NonZeroU64::new(2).expect("sidecar fixture successor generation is non-zero"),
        );
        let mut generation_hint = CertifiedMergeSidecarGenerationHintV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            observed_generation: service_generation,
            current_generation,
            observed_message_hash: HashOf::new(&request).into(),
            hint_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: requester.clone(),
            responder: responder.clone(),
        };
        generation_hint.hint_id = generation_hint.canonical_hint_id();
        let generation_hint_message = NetworkMessage::CertifiedMergeSidecar(Arc::new(
            CertifiedMergeSidecarMessage::GenerationHint(generation_hint.clone()),
        ));
        let NetworkMessage::CertifiedMergeSidecar(generation_hint_payload) =
            &generation_hint_message
        else {
            unreachable!("generation Hint fixture uses the sidecar variant")
        };
        assert_shared_carrier_wire_compatible(generation_hint_payload.as_ref());
        assert_eq!(generation_hint_message.topic(), NetworkTopic::Consensus);
        assert_eq!(
            raw_network_topic(&generation_hint_message),
            NetworkTopic::Consensus
        );
        let generation_hint_hash = HashOf::new(&generation_hint_message);
        let encoded =
            norito::to_bytes(&generation_hint_message).expect("encode sidecar generation Hint");
        let decoded = norito::decode_from_bytes::<NetworkMessage>(&encoded)
            .expect("decode sidecar generation Hint");
        assert_eq!(HashOf::new(&decoded), generation_hint_hash);
        let NetworkMessage::CertifiedMergeSidecar(message) = decoded else {
            panic!("decoded sidecar generation Hint uses the sidecar variant");
        };
        let CertifiedMergeSidecarMessage::GenerationHint(decoded_generation_hint) =
            message.as_ref()
        else {
            panic!("decoded sidecar generation Hint preserves the Hint variant");
        };
        assert_eq!(
            decoded_generation_hint.observed_generation,
            service_generation
        );
        assert_eq!(
            decoded_generation_hint.current_generation,
            current_generation
        );
        assert_eq!(decoded_generation_hint, &generation_hint);
        let chunk = CertifiedMergeSidecarChunkV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: request.service_generation,
            stream_epoch: request.stream_epoch,
            semantic_sequence: request.semantic_sequence,
            request_id: request.request_id,
            entry_hash,
            encoded_len: 3,
            epoch_id: 4,
            reference_digest: request.reference_digest,
            requester,
            responder,
            chunk_index: 0,
            chunk_count: 1,
            bytes: vec![1, 2, 3],
        };
        let chunk_message = NetworkMessage::CertifiedMergeSidecar(Arc::new(
            CertifiedMergeSidecarMessage::Chunk(chunk.clone()),
        ));
        let NetworkMessage::CertifiedMergeSidecar(chunk_payload) = &chunk_message else {
            unreachable!("chunk fixture uses the sidecar variant")
        };
        assert_shared_carrier_wire_compatible(chunk_payload.as_ref());
        assert_eq!(chunk_message.topic(), NetworkTopic::ConsensusChunk);
        assert_eq!(
            raw_network_topic(&chunk_message),
            NetworkTopic::ConsensusChunk
        );
        let encoded = norito::to_bytes(&chunk_message).expect("encode sidecar chunk");
        let chunk_hash = HashOf::new(&chunk_message);
        let decoded =
            norito::decode_from_bytes::<NetworkMessage>(&encoded).expect("decode sidecar chunk");
        assert_eq!(HashOf::new(&decoded), chunk_hash);
        let NetworkMessage::CertifiedMergeSidecar(message) = decoded else {
            panic!("decoded sidecar chunk uses the sidecar variant");
        };
        let CertifiedMergeSidecarMessage::Chunk(decoded_chunk) = message.as_ref() else {
            panic!("decoded sidecar chunk preserves the chunk variant");
        };
        assert_eq!(decoded_chunk.service_generation, service_generation);
        assert_eq!(decoded_chunk.stream_epoch, stream_epoch);
        assert_eq!(decoded_chunk, &chunk);
    }
    #[test]
    fn torii_proxy_control_message_classification_covers_current_variants() {
        let torii_request = NetworkMessage::ToriiProxyRequest(Arc::new(ToriiProxyRequestV1 {
            schema_version: TORII_PROXY_REQUEST_VERSION_V1,
            request_id: Hash::prehashed([0x14; 32]),
            deadline_unix_ms: 1_900_000_000_000,
            hop_count: 1,
            max_hops: 3,
            visited_peer_ids: Vec::new(),
            request: ToriiProxyRequestKindV1::Read(ToriiReadProxyRequestV1 {
                endpoint: ToriiReadEndpointV1::AccountsList,
                expected_route: ToriiRouteHintV1 {
                    lane_id: LaneId::SINGLE,
                    dataspace_id: DataSpaceId::UNIVERSAL,
                },
                path_args: Vec::new(),
                query_string: None,
                body: Vec::new(),
                response_format: ToriiProxyResponseFormatV1::Json,
            }),
        }));
        let torii_response = NetworkMessage::ToriiProxyResponse(Box::new(ToriiProxyResponseV1 {
            schema_version: TORII_PROXY_RESPONSE_VERSION_V1,
            request_id: Hash::prehashed([0x15; 32]),
            response: ToriiProxyHttpResponseV1 {
                status_code: 200,
                headers: Vec::new(),
                body: Vec::new(),
            },
        }));
        let queue_plan_publication = NetworkMessage::QueuePlanAdmissionPublication(Arc::new(
            QueuePlanAdmissionPublicationV1 {
                schema_version: QUEUE_PLAN_ADMISSION_PUBLICATION_VERSION_V1,
                certificate: vec![0x16],
            },
        ));
        assert!(torii_request.is_torii_proxy_control_message());
        assert!(torii_response.is_torii_proxy_control_message());
        assert!(queue_plan_publication.is_torii_proxy_control_message());
        assert!(!NetworkMessage::Health.is_torii_proxy_control_message());
        for (message, expected_tag) in [
            (&torii_request, 13),
            (&torii_response, 14),
            (&queue_plan_publication, 16),
        ] {
            assert_eq!(raw_network_tag(message), expected_tag);
            assert_eq!(message.topic(), NetworkTopic::Control);
            assert_eq!(raw_network_topic(message), NetworkTopic::Control);
            assert_eq!(message.subscriber_route(), SubscriberRoute::ToriiProxy);
            assert_eq!(
                iroha_p2p::network::reliable_progress_class(
                    message.topic(),
                    message.subscriber_route(),
                ),
                None,
                "Torii proxy request/response carriers must use recoverable best-effort admission, not the reliable-progress corridor"
            );
        }
        let target = PeerId::from(checked_topic_keypair().public_key().clone());
        let capped = crate::IrohaNetwork::closed_for_tests()
            .with_topic_plaintext_frame_cap_for_tests(NetworkTopic::Control, 1);
        for message in [torii_request.clone(), torii_response.clone()] {
            match capped.post_best_effort_recoverable(iroha_p2p::Post {
                data: message,
                peer_id: target.clone(),
                priority: iroha_p2p::Priority::High,
            }) {
                Err(iroha_p2p::network::NetworkPostAdmissionError::Rejected {
                    message,
                    reason: iroha_p2p::network::NetworkActorAdmissionRejection::FrameTooLarge,
                }) => {
                    assert_eq!(message.data.topic(), NetworkTopic::Control);
                    assert_eq!(message.data.subscriber_route(), SubscriberRoute::ToriiProxy);
                }
                other => panic!(
                    "oversized actual Torii proxy carrier must fail exact recoverable admission: {other:?}"
                ),
            }
        }
        let network = crate::IrohaNetwork::closed_for_tests();
        for message in [torii_request, torii_response] {
            match network.post_best_effort_recoverable(iroha_p2p::Post {
                data: message,
                peer_id: target.clone(),
                priority: iroha_p2p::Priority::High,
            }) {
                Err(iroha_p2p::network::NetworkPostAdmissionError::Closed { message }) => {
                    assert_eq!(message.data.topic(), NetworkTopic::Control);
                    assert_eq!(message.data.subscriber_route(), SubscriberRoute::ToriiProxy);
                }
                other => panic!(
                    "actual Torii proxy carrier must reach best-effort actor admission: {other:?}"
                ),
            }
        }
    }
    include!("tests/queue_plan_admission_handoff.rs");
    include!("tests/sumeragi_v2_decode_limits.rs");
    #[test]
    fn torii_proxy_carriers_preserve_request_wire_and_have_explicit_decode_caps() {
        #[derive(Encode)]
        #[norito(schema_name = "iroha_core::NetworkMessage")]
        enum BoxToriiProxyCarrier {
            #[codec(index = 13)]
            Request(Box<ToriiProxyRequestV1>),
        }
        let request = ToriiProxyRequestV1 {
            schema_version: TORII_PROXY_REQUEST_VERSION_V1,
            request_id: Hash::prehashed([0x24; 32]),
            deadline_unix_ms: 1_900_000_000_000,
            hop_count: 1,
            max_hops: 3,
            visited_peer_ids: Vec::new(),
            request: ToriiProxyRequestKindV1::Read(ToriiReadProxyRequestV1 {
                endpoint: ToriiReadEndpointV1::AccountsList,
                expected_route: ToriiRouteHintV1 {
                    lane_id: LaneId::SINGLE,
                    dataspace_id: DataSpaceId::UNIVERSAL,
                },
                path_args: Vec::new(),
                query_string: None,
                body: Vec::new(),
                response_format: ToriiProxyResponseFormatV1::Json,
            }),
        };
        let boxed = ncore::to_bytes(&BoxToriiProxyCarrier::Request(Box::new(request.clone())))
            .expect("encode Box proxy carrier");
        let shared = ncore::to_bytes(&NetworkMessage::ToriiProxyRequest(Arc::new(request)))
            .expect("encode Arc proxy carrier");
        assert_eq!(
            shared, boxed,
            "Box-to-Arc ownership must not change wire bytes"
        );
        let origin_key = KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::Ed25519)
            .expect("generate proxy relay origin key");
        let origin = PeerId::new(origin_key.public_key().clone());
        let live = ncore::decode_from_bytes::<NetworkMessage>(&shared)
            .expect("decode live Arc proxy carrier");
        let p2p_wire_len = iroha_p2p::network::data_frame_wire_len(&origin, None, &live);
        let view = ncore::from_bytes_view(&shared).expect("inspect proxy carrier frame");
        assert!(
            <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
                view.as_bytes(),
                p2p_wire_len,
                view.flags(),
            )
            .expect("derive proxy decode limits")
            .is_some()
        );
        assert!(matches!(
            <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
                view.as_bytes(),
                TORII_PROXY_REQUEST_MAX_FRAME_BYTES_V1 + 1,
                view.flags(),
            ),
            Err(ncore::Error::ArchiveLengthExceeded { .. })
        ));
        let worst_request_wire =
            iroha_p2p::network::broadcast_data_frame_wire_len_from_payload_len::<NetworkMessage>(
                TORII_PROXY_REQUEST_MAX_ENCODED_BYTES_V1
                    + TORII_PROXY_NETWORK_MESSAGE_OVERHEAD_BYTES_V1,
            );
        assert!(worst_request_wire <= TORII_PROXY_REQUEST_MAX_FRAME_BYTES_V1);
        let response = NetworkMessage::ToriiProxyResponse(Box::new(ToriiProxyResponseV1 {
            schema_version: TORII_PROXY_RESPONSE_VERSION_V1,
            request_id: Hash::prehashed([0x25; 32]),
            response: ToriiProxyHttpResponseV1 {
                status_code: 200,
                headers: Vec::new(),
                body: vec![0x5a; 32],
            },
        }));
        let response_bytes = ncore::to_bytes(&response).expect("encode proxy response carrier");
        let response_wire_len = iroha_p2p::network::data_frame_wire_len(&origin, None, &response);
        let response_view =
            ncore::from_bytes_view(&response_bytes).expect("inspect proxy response frame");
        assert!(
            <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
                response_view.as_bytes(),
                response_wire_len,
                response_view.flags(),
            )
            .expect("derive proxy-response decode limits")
            .is_some()
        );
        assert!(matches!(
            <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
                response_view.as_bytes(),
                TORII_PROXY_RESPONSE_MAX_FRAME_BYTES_V1 + 1,
                response_view.flags(),
            ),
            Err(ncore::Error::ArchiveLengthExceeded { .. })
        ));
        let worst_response_wire =
            iroha_p2p::network::broadcast_data_frame_wire_len_from_payload_len::<NetworkMessage>(
                TORII_PROXY_RESPONSE_MAX_ENCODED_BYTES_V1
                    + TORII_PROXY_NETWORK_MESSAGE_OVERHEAD_BYTES_V1,
            );
        assert!(worst_response_wire <= TORII_PROXY_RESPONSE_MAX_FRAME_BYTES_V1);
    }
    #[test]
    fn torii_proxy_submit_decode_budget_covers_ten_mib_transaction_carrier() {
        const TRANSACTION_BODY_BYTES: usize = 10 * 1024 * 1024;
        let (account, keypair) = gen_account_in("wonderland");
        let mut builder = TransactionBuilder::new(
            test_network_id(b"ten-mib-torii-proxy-submit"),
            account,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::from_millis(1));
        let transaction = builder
            .with_instructions([Log::new(Level::INFO, "P".repeat(TRANSACTION_BODY_BYTES))])
            .sign(keypair.private_key());
        let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        let message = NetworkMessage::ToriiProxyRequest(Arc::new(ToriiProxyRequestV1 {
            schema_version: TORII_PROXY_REQUEST_VERSION_V1,
            request_id: Hash::new(b"ten-mib-torii-proxy-submit-request"),
            deadline_unix_ms: 1_900_000_000_000,
            hop_count: 1,
            max_hops: 3,
            visited_peer_ids: Vec::new(),
            request: ToriiProxyRequestKindV1::SubmitTransaction {
                transaction: TransactionEntrypoint::External(transaction),
                expected_plan: ToriiRoutingPlanHintV1::from(RoutingPlan::single(route)),
                admission: ToriiProxyTransactionAdmissionV1::QueuePlanSynced,
                admission_binding: None,
            },
        }));
        let encoded = ncore::to_bytes(&message).expect("encode 10 MiB proxy submission");
        let view = ncore::from_bytes_view(&encoded).expect("inspect 10 MiB proxy submission");
        let limits = <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
            view.as_bytes(),
            encoded.len(),
            view.flags(),
        )
        .expect("select proxy submission decode policy")
        .expect("proxy submission installs explicit decode limits");
        assert_eq!(
            limits.max_total_allocated_bytes(),
            TORII_PROXY_REQUEST_MAX_DECODE_ALLOCATED_BYTES_V1
        );
        let decoded = ncore::decode_from_bytes_with_limits::<NetworkMessage>(&encoded, limits)
            .expect("decode exact 10 MiB proxy submission within its explicit allocation cap");
        let NetworkMessage::ToriiProxyRequest(decoded) = decoded else {
            panic!("decoded proxy submission changed its network-message variant");
        };
        let ToriiProxyRequestKindV1::SubmitTransaction { transaction, .. } = &decoded.request
        else {
            panic!("decoded proxy submission changed its request kind");
        };
        let TransactionEntrypoint::External(transaction) = transaction else {
            panic!("decoded proxy submission changed its transaction entrypoint");
        };
        let iroha_data_model::transaction::Executable::Instructions(instructions) =
            transaction.instructions()
        else {
            panic!("decoded proxy submission changed its executable kind");
        };
        let log = instructions
            .first()
            .and_then(|instruction| instruction.as_any().downcast_ref::<Log>())
            .expect("decoded proxy submission preserves its Log carrier");
        assert_eq!(instructions.len(), 1);
        assert_eq!(log.level, Level::INFO);
        assert_eq!(log.msg.len(), TRANSACTION_BODY_BYTES);
        assert!(log.msg.as_bytes().iter().all(|byte| *byte == b'P'));
    }
    #[test]
    fn authoritative_v2_safety_uses_dedicated_topic() {
        use iroha_data_model::block::consensus_v2 as wire;
        let context_id = wire::HeightContextId(
            iroha_crypto::HashOf::<wire::HeightContext>::from_untyped_unchecked(Hash::new(
                b"v2-safety-topic-context",
            )),
        );
        let round = wire::ConsensusRound {
            context_id,
            height: 7,
            view: 2,
        };
        let vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                    b"v2-safety-topic-block",
                )),
                payload_hash: Hash::new(b"v2-safety-topic-payload"),
            },
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"v2-safety-topic-parent-state"),
                Hash::new(b"v2-safety-topic-post-state"),
                Hash::new(b"v2-safety-topic-ordinary-writes"),
                1,
                Hash::new(b"v2-safety-topic-executed-block-wire"),
            ),
            signer: 0,
            signature: vec![1],
        };
        let message =
            NetworkMessage::SumeragiBlock(Arc::new(BlockMessageWire::new(BlockMessage::V2(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote)),
            ))));
        assert_eq!(message.topic(), NetworkTopic::ConsensusSafety);
        let encoded = ncore::to_bytes(&message).expect("encode owned Sumeragi field");
        let view = ncore::from_bytes_view(&encoded).expect("inspect owned Sumeragi field");
        let (tag, remaining) = super::inbound_enum_parts(view.as_bytes())
            .expect("extract network-message discriminant");
        assert_eq!(tag, 0);
        assert!(
            super::inbound_owned_enum_field(remaining, view.flags())
                .expect("unwrap enum and Box length prefixes")
                .starts_with(&ncore::MAGIC),
            "the nested raw classifier must receive the full BlockMessage frame"
        );
        assert_eq!(raw_network_topic(&message), NetworkTopic::ConsensusSafety);
    }
    fn signed_kura_replica_advert_message() -> NetworkMessage {
        let key = KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::BlsNormal)
            .expect("generate BLS-normal Kura replica keeper key");
        let mut advert = KuraReplicaAdvertV1 {
            version: KURA_REPLICA_ADVERT_VERSION_V1,
            network_id: test_network_id(b"network-kura-replica-advert-test"),
            height: 9,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"replica-block")),
            executed_block_wire_len: 2048,
            executed_block_wire_hash: Hash::new(b"replica-executed-wire"),
            finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(b"replica-finality")),
            keeper_index: 0,
            keeper: PeerId::new(key.public_key().clone()),
            signature: Vec::new(),
        };
        advert.signature = Signature::new(key.private_key(), &advert.signature_preimage())
            .payload()
            .to_vec();
        NetworkMessage::SumeragiBlock(Arc::new(BlockMessageWire::new(
            BlockMessage::KuraReplicaAdvert(advert),
        )))
    }
    #[test]
    fn kura_replica_advert_uses_bounded_consensus_auxiliary_topic() {
        let message = signed_kura_replica_advert_message();
        assert_eq!(message.topic(), NetworkTopic::Consensus);
        assert_eq!(raw_network_topic(&message), NetworkTopic::Consensus);
        assert!(message.is_outbound_allowed());
        let encoded = ncore::to_bytes(&message).expect("encode Kura replica advert network frame");
        let view = ncore::from_bytes_view(&encoded).expect("inspect Kura replica advert frame");
        let limits = <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
            view.as_bytes(),
            encoded.len(),
            view.flags(),
        )
        .expect("derive Kura replica advert decode limits");
        assert!(
            limits.is_some(),
            "the auxiliary advert must decode under an explicit bound"
        );
        assert!(matches!(
            <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
                view.as_bytes(),
                MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES + 1,
                view.flags(),
            ),
            Err(ncore::Error::ArchiveLengthExceeded { .. })
        ));
    }
    #[test]
    fn sumeragi_block_classifies_only_v2_as_global_consensus() {
        use iroha_data_model::block::consensus_v2::{
            ConsensusMessageV2, ConsensusMessageV2Payload, PayloadChunk, PayloadManifest,
        };
        let canonical_chunk =
            ConsensusMessageV2::new(ConsensusMessageV2Payload::PayloadChunk(PayloadChunk {
                manifest_hash: iroha_crypto::HashOf::<PayloadManifest>::from_untyped_unchecked(
                    Hash::new(b"v2-topic-manifest"),
                ),
                index: 0,
                bytes: vec![1, 2, 3],
                sender: 0,
                signature: vec![4],
            }));
        let v2_chunk = NetworkMessage::SumeragiBlock(Arc::new(BlockMessageWire::new(
            BlockMessage::V2(canonical_chunk.clone()),
        )));
        assert_eq!(v2_chunk.topic(), NetworkTopic::ConsensusChunk);
        assert_eq!(raw_network_topic(&v2_chunk), NetworkTopic::ConsensusChunk);
        assert!(v2_chunk.is_outbound_allowed());
        assert!(
            ncore::to_bytes(&v2_chunk).is_ok(),
            "canonical v2 traffic must remain live-encodable"
        );
        let mut wrong_version_chunk = canonical_chunk;
        wrong_version_chunk.protocol_version = 1;
        let wrong_version_chunk = NetworkMessage::SumeragiBlock(Arc::new(BlockMessageWire::new(
            BlockMessage::V2(wrong_version_chunk),
        )));
        assert_eq!(wrong_version_chunk.topic(), NetworkTopic::Other);
        assert!(!wrong_version_chunk.is_outbound_allowed());
        assert!(
            ncore::to_bytes(&wrong_version_chunk).is_err(),
            "a non-canonical protocol version must fail the wire boundary"
        );
    }
    #[test]
    fn network_message_roundtrip_cached_transaction_gossip() {
        let (account, keypair) = gen_account_in("wonderland");
        let network_id = test_network_id(b"cached-transaction-gossip");
        let mut builder = TransactionBuilder::new(
            network_id,
            account,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::from_millis(0));
        let signed = builder
            .with_instructions([Log::new(Level::INFO, "ping".to_owned())])
            .sign(keypair.private_key());
        let payload = canonical_signed_transaction_payload(&signed);
        let route = GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
        };
        let gossip = TransactionGossip {
            txs: vec![GossipTransaction::with_encoded(
                signed.clone(),
                Arc::clone(&payload),
            )],
            routes: vec![route],
            plans: vec![RoutingPlan::single(RoutingDecision::new(
                route.lane_id,
                route.dataspace_id,
            ))],
            plane: GossipPlane::Public,
        };
        let msg = NetworkMessage::TransactionGossiper(Arc::new(gossip));
        assert_eq!(raw_network_tag(&msg), 6);
        assert_eq!(raw_network_topic(&msg), NetworkTopic::TxGossip);
        let bytes = msg.encode();
        let (decoded, used) = <NetworkMessage as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
            .expect("decode gossip network");
        assert_eq!(used, bytes.len());
        match decoded {
            NetworkMessage::TransactionGossiper(gossip) => {
                assert_eq!(gossip.txs.len(), 1);
                assert_eq!(gossip.txs[0].as_signed().hash(), signed.hash());
                let wire = gossip.txs[0].encode();
                assert_eq!(wire.as_slice(), payload.as_slice());
                assert!(wire.starts_with(&ncore::MAGIC));
                assert_eq!(gossip.routes.len(), 1);
                assert_eq!(gossip.routes[0].lane_id, LaneId::SINGLE);
                assert_eq!(gossip.routes[0].dataspace_id, DataSpaceId::UNIVERSAL);
            }
            other => panic!("expected transaction gossip, got {other:?}"),
        }
    }
    #[test]
    fn network_message_roundtrip_cached_transaction_gossip_is_context_free() {
        let (account, keypair) = gen_account_in("wonderland");
        let network_id = test_network_id(b"context-free-cached-transaction-gossip");
        let mut builder = TransactionBuilder::new(
            network_id,
            account,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::from_millis(0));
        let signed = builder
            .with_instructions([Log::new(Level::INFO, "pong".to_owned())])
            .sign(keypair.private_key());
        let canonical_payload = canonical_signed_transaction_payload(&signed);
        let payload = {
            let _guard = ncore::DecodeFlagsGuard::enter(ncore::header_flags::COMPACT_LEN);
            Arc::new(
                ncore::to_bytes(
                    &iroha_data_model::transaction::TransactionEntrypoint::External(signed.clone()),
                )
                .expect("encode signed transaction entrypoint"),
            )
        };
        std::thread::spawn(move || {
            let route = GossipRoute {
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::UNIVERSAL,
            };
            let gossip = TransactionGossip {
                txs: vec![GossipTransaction::with_encoded(
                    signed.clone(),
                    Arc::clone(&payload),
                )],
                routes: vec![route],
                plans: vec![RoutingPlan::single(RoutingDecision::new(
                    route.lane_id,
                    route.dataspace_id,
                ))],
                plane: GossipPlane::Public,
            };
            let msg = NetworkMessage::TransactionGossiper(Arc::new(gossip));
            let bytes = msg.encode();
            let (decoded, used) =
                <NetworkMessage as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
                    .expect("decode gossip network");
            assert_eq!(used, bytes.len());
            match decoded {
                NetworkMessage::TransactionGossiper(gossip) => {
                    assert_eq!(gossip.txs.len(), 1);
                    assert_eq!(gossip.txs[0].as_signed().hash(), signed.hash());
                    let wire = gossip.txs[0].encode();
                    assert_eq!(wire.as_slice(), canonical_payload.as_slice());
                    assert!(wire.starts_with(&ncore::MAGIC));
                    assert_eq!(gossip.routes.len(), 1);
                    assert_eq!(gossip.routes[0].lane_id, LaneId::SINGLE);
                    assert_eq!(gossip.routes[0].dataspace_id, DataSpaceId::UNIVERSAL);
                }
                other => panic!("expected transaction gossip, got {other:?}"),
            }
        })
        .join()
        .expect("context-free network gossip thread");
    }
    #[test]
    fn cmp_role_id_with_owner() {
        let role_id_a: RoleId = "a".parse().expect("failed to parse RoleId");
        let role_id_b: RoleId = "b".parse().expect("failed to parse RoleId");
        let (account_id_a, _account_keypair_a) = gen_account_in("domain");
        let (account_id_b, _account_keypair_b) = gen_account_in("domain");
        let mut role_ids_with_owner = Vec::new();
        for account_id in [&account_id_a, &account_id_b] {
            for role_id in [&role_id_a, &role_id_b] {
                role_ids_with_owner.push(RoleIdWithOwner {
                    id: role_id.clone(),
                    account: account_id.clone(),
                })
            }
        }
        for role_id_with_owner_1 in &role_ids_with_owner {
            for role_id_with_owner_2 in &role_ids_with_owner {
                match (
                    role_id_with_owner_1
                        .account
                        .cmp(&role_id_with_owner_2.account),
                    role_id_with_owner_1.id.cmp(&role_id_with_owner_2.id),
                ) {
                    // `AccountId` take precedence in comparison
                    // if `AccountId`s are equal than comparison based on `RoleId`s
                    (Ordering::Equal, ordering) | (ordering, _) => assert_eq!(
                        role_id_with_owner_1.cmp(role_id_with_owner_2),
                        ordering,
                        "{role_id_with_owner_1:?} and {role_id_with_owner_2:?} are expected to be {ordering:?}"
                    ),
                }
            }
        }
    }
}
