//! Versioned SCCP network, lane, and source-identity wire types.
//!
//! These types deliberately model only the first-release network inventory. There is no catch-all
//! network, emitter, or arbitrary network identifier: unsupported profiles must fail decoding
//! instead of being interpreted by node-local policy.
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use core::cmp::Ordering;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
/// Maximum canonical SCCP application-payload bytes retained in one outbound record.
///
/// This consensus-visible bound is shared by transaction admission, durable state, and
/// protocol-native proof admission. Keeping the canonical payload in the authoritative outbox
/// record lets APIs project recent messages without reopening historical block bodies. V1 has
/// four variable fields individually capped at 256 bytes; 4 KiB deliberately leaves fixed-layout
/// and framing headroom while rejecting payload amplification far above the closed V1 surface.
pub const SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1: usize = 4 * 1024;
/// Maximum successful outbound SCCP messages committed by one block.
///
/// Commitment indices are consensus-visible and must be cheap to validate and reconstruct from
/// durable state. The bound matches the first-release default consensus-v2 transaction cap while
/// also limiting blocks whose transactions contain multiple SCCP instructions. A transaction that
/// would exceed this bound is rejected atomically, so failed execution never consumes an index.
pub const SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1: u32 = 512;
/// TON mainnet global identifier committed by the V1 network profile.
pub const SCCP_TON_MAINNET_GLOBAL_ID_V1: i32 = -239;
/// TON masterchain workchain used by both governed zero-state identities.
pub const SCCP_TON_MASTERCHAIN_WORKCHAIN_V1: i32 = -1;
/// TON all-shards masterchain shard id used by both governed zero states.
pub const SCCP_TON_MASTERCHAIN_SHARD_V1: u64 = 0x8000_0000_0000_0000;
/// TON zero-state sequence number.
pub const SCCP_TON_ZERO_STATE_SEQNO_V1: u32 = 0;
/// TON mainnet zero-state root hash.
pub const SCCP_TON_MAINNET_ZERO_STATE_ROOT_HASH_V1: [u8; 32] = [
    0x17, 0xa3, 0xa9, 0x29, 0x92, 0xaa, 0xbe, 0xa7, 0x85, 0xa7, 0xa0, 0x90, 0x98, 0x5a, 0x26, 0x5c,
    0xd3, 0x1f, 0x32, 0x3d, 0x84, 0x9d, 0xa5, 0x12, 0x39, 0x73, 0x7e, 0x32, 0x1f, 0xb0, 0x55, 0x69,
];
/// TON mainnet zero-state file hash.
pub const SCCP_TON_MAINNET_ZERO_STATE_FILE_HASH_V1: [u8; 32] = [
    0x5e, 0x99, 0x4f, 0xcf, 0x4d, 0x42, 0x5c, 0x0a, 0x6c, 0xe6, 0xa7, 0x92, 0x59, 0x4b, 0x71, 0x73,
    0x20, 0x5f, 0x74, 0x0a, 0x39, 0xcd, 0x56, 0xf5, 0x37, 0xde, 0xfd, 0x28, 0xb4, 0x8a, 0x0f, 0x6e,
];
/// TON basechain workchain used by the first-release SCCP contracts.
pub const SCCP_TON_BASECHAIN_WORKCHAIN_V1: i32 = 0;
/// A supported SCCP network profile for the V1 wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "network", content = "profile")]
pub enum SccpNetworkV1 {
    /// The sole production SORA endpoint admitted by SCCP V1.
    #[codec(index = 64)]
    #[norito(rename = "sora_taira")]
    SoraTaira,
    /// Ethereum mainnet.
    #[codec(index = 65)]
    #[norito(rename = "ethereum_mainnet")]
    EthereumMainnet,
    /// BNB Smart Chain mainnet.
    #[codec(index = 66)]
    #[norito(rename = "bsc_mainnet")]
    BscMainnet,
    /// TRON mainnet.
    #[codec(index = 67)]
    #[norito(rename = "tron_mainnet")]
    TronMainnet,
    /// TON mainnet, bound to global id [`SCCP_TON_MAINNET_GLOBAL_ID_V1`].
    #[codec(index = 68)]
    #[norito(rename = "ton_mainnet")]
    TonMainnet,
}
impl SccpNetworkV1 {
    /// Return the SCCP protocol domain carried by messages for this profile.
    #[must_use]
    pub const fn domain_id(self) -> u32 {
        match self {
            Self::SoraTaira => 0,
            Self::EthereumMainnet => 1,
            Self::BscMainnet => 2,
            Self::TonMainnet => 4,
            Self::TronMainnet => 5,
        }
    }
    /// Return the canonical, stable textual key for this exact profile.
    #[must_use]
    pub const fn profile_key(self) -> &'static str {
        match self {
            Self::SoraTaira => "sora-taira",
            Self::EthereumMainnet => "ethereum-mainnet",
            Self::BscMainnet => "bsc-mainnet",
            Self::TronMainnet => "tron-mainnet",
            Self::TonMainnet => "ton-mainnet",
        }
    }
    /// Parse an exact canonical profile key.
    ///
    /// Parsing is deliberately case-sensitive and accepts neither domain-wide
    /// aliases nor abbreviated chain names. This makes textual storage keys a
    /// one-to-one representation of the closed V1 network inventory.
    #[must_use]
    pub fn from_profile_key(profile: &str) -> Option<Self> {
        match profile {
            "sora-taira" => Some(Self::SoraTaira),
            "ethereum-mainnet" => Some(Self::EthereumMainnet),
            "bsc-mainnet" => Some(Self::BscMainnet),
            "tron-mainnet" => Some(Self::TronMainnet),
            "ton-mainnet" => Some(Self::TonMainnet),
            _ => None,
        }
    }
    /// Return whether this is a production network profile.
    #[must_use]
    pub const fn is_production_profile(self) -> bool {
        true
    }
    /// Return whether this is a staging or test network profile.
    #[must_use]
    pub const fn is_staging_profile(self) -> bool {
        !self.is_production_profile()
    }
    /// Return whether this profile belongs to the SORA domain.
    #[must_use]
    pub const fn is_sora(self) -> bool {
        matches!(self, Self::SoraTaira)
    }
    /// Return whether this profile is a supported external SCCP endpoint.
    #[must_use]
    pub const fn is_external(self) -> bool {
        !self.is_sora()
    }
    /// Return whether V1 can safely admit this external network as a message source.
    ///
    /// Only families with exact value-moving source and destination implementations
    /// are representable in the first-release registry.
    #[must_use]
    pub const fn supports_native_inbound_source(self) -> bool {
        matches!(
            self,
            Self::EthereumMainnet | Self::BscMainnet | Self::TronMainnet | Self::TonMainnet
        )
    }
}
/// A directed SCCP lane between two exact V1 network profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpLaneIdV1 {
    /// Network on which the SCCP message originates.
    pub source: SccpNetworkV1,
    /// Network on which the SCCP message is consumed.
    pub target: SccpNetworkV1,
}
impl SccpLaneIdV1 {
    /// Return whether the lane joins exactly one SORA endpoint and one external endpoint.
    ///
    /// This rejects self lanes, SORA-to-SORA lanes, external-to-external lanes,
    /// and profiles that share one SCCP domain.
    #[must_use]
    pub const fn is_well_formed(self) -> bool {
        self.source.is_sora() != self.target.is_sora()
            && self.source.domain_id() != self.target.domain_id()
    }
    /// Return whether both endpoints use production profiles.
    #[must_use]
    pub const fn is_production_environment(self) -> bool {
        self.source.is_production_profile() && self.target.is_production_profile()
    }
    /// Return whether at least one endpoint uses a staging or test profile.
    #[must_use]
    pub const fn is_staging_environment(self) -> bool {
        !self.is_production_environment()
    }
    /// Return the external endpoint when the lane topology is valid.
    #[must_use]
    pub const fn external_network(self) -> Option<SccpNetworkV1> {
        if !self.is_well_formed() {
            None
        } else if self.source.is_external() {
            Some(self.source)
        } else {
            Some(self.target)
        }
    }
    /// Return the SORA endpoint when the lane topology is valid.
    #[must_use]
    pub const fn sora_network(self) -> Option<SccpNetworkV1> {
        if !self.is_well_formed() {
            None
        } else if self.source.is_sora() {
            Some(self.source)
        } else {
            Some(self.target)
        }
    }
}
/// Exact governed context supplied when recording a SORA-origin SCCP message.
///
/// The binding hash is deliberately not part of the replay key. A destination
/// rollout may rotate after a message is finalized, but the same lane-bound
/// economic message must never become recordable again under the new binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpOutboundMessageContextV1 {
    /// Exact SORA-to-external lane on which the message is emitted.
    pub lane: SccpLaneIdV1,
    /// Governed destination binding active when the message is recorded.
    pub destination_binding_hash: [u8; 32],
    /// Immutable governed route configuration active at record admission.
    ///
    /// This commitment is present in the instruction bytes so block-root
    /// reconstruction never depends on consulting later world state.
    pub route_configuration_hash: [u8; 32],
}
impl SccpOutboundMessageContextV1 {
    /// Construct an outbound context after validating its topology and binding.
    #[must_use]
    pub fn new(
        lane: SccpLaneIdV1,
        destination_binding_hash: [u8; 32],
        route_configuration_hash: [u8; 32],
    ) -> Option<Self> {
        let context = Self {
            lane,
            destination_binding_hash,
            route_configuration_hash,
        };
        context.is_well_formed().then_some(context)
    }
    /// Return whether this is a SORA-to-external lane with distinct nonzero
    /// destination-binding and route-configuration commitments.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.lane.is_well_formed()
            && self.lane.source.is_sora()
            && self.lane.target.is_external()
            && nonzero(&self.destination_binding_hash)
            && nonzero(&self.route_configuration_hash)
            && self.destination_binding_hash != self.route_configuration_hash
    }
}
/// Durable replay key for a SORA-origin SCCP message.
///
/// Exact network profiles prevent messages on two networks in the same SCCP domain from aliasing
/// each other. The destination binding is intentionally excluded so rotating a governed rollout
/// cannot replay an already-recorded lane-bound message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpOutboundMessageKeyV1 {
    /// Exact SORA-to-external lane on which the message was emitted.
    pub lane: SccpLaneIdV1,
    /// Nonzero identifier derived from the exact lane and canonical payload.
    pub message_id: [u8; 32],
}
impl SccpOutboundMessageKeyV1 {
    /// Construct a replay key after validating its outbound topology and identifier.
    #[must_use]
    pub fn new(lane: SccpLaneIdV1, message_id: [u8; 32]) -> Option<Self> {
        let key = Self { lane, message_id };
        key.is_well_formed().then_some(key)
    }
    /// Return whether this is a SORA-to-external lane with a nonzero message identifier.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.lane.is_well_formed()
            && self.lane.source.is_sora()
            && self.lane.target.is_external()
            && nonzero(&self.message_id)
    }
}
/// Ordered locator for one durable outbound SCCP message.
///
/// Reverse-height ordering supports bounded newest-first pagination and direct
/// seeking to an inclusive historical height. Lane and message id make every
/// index entry self-checking against the authoritative replay map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpOutboundMessageIndexKeyV1 {
    /// Local SORA block height containing the recorded message.
    pub recorded_at_height: u64,
    /// Zero-based position in the block header's SCCP commitment Merkle tree.
    pub commitment_index: u32,
    /// Exact SORA-to-external lane of the replay key.
    pub lane: SccpLaneIdV1,
    /// Globally unique lane-bound SCCP message identifier.
    pub message_id: [u8; 32],
}
impl SccpOutboundMessageIndexKeyV1 {
    /// Build an ordered locator from one authoritative replay key and record.
    #[must_use]
    pub fn new(
        key: SccpOutboundMessageKeyV1,
        record: &SccpOutboundPendingMessageRecordV1,
    ) -> Option<Self> {
        if !record.is_well_formed_for_key(&key) {
            return None;
        }
        Self::from_descriptor(key, record.descriptor())
    }
    /// Build an ordered locator from a validated fixed descriptor.
    #[must_use]
    pub fn from_descriptor(
        key: SccpOutboundMessageKeyV1,
        descriptor: SccpOutboundMessageDescriptorV1,
    ) -> Option<Self> {
        if !descriptor.is_well_formed_for_key(&key) {
            return None;
        }
        Some(Self {
            recorded_at_height: descriptor.recorded_at_height,
            commitment_index: descriptor.commitment_index,
            lane: key.lane,
            message_id: key.message_id,
        })
    }
    /// Return the inclusive lower bound for a newest-first range at `height`.
    ///
    /// This sentinel is only a search bound and must never be persisted: its zero message
    /// identifier intentionally sorts before every well-formed entry at the same height. Since
    /// index ordering reverses height, a forward range beginning here contains exactly entries
    /// recorded at or before `height`, newest first.
    #[must_use]
    pub const fn range_start_at_or_before(height: u64) -> Self {
        Self {
            recorded_at_height: height,
            commitment_index: 0,
            lane: SccpLaneIdV1 {
                source: SccpNetworkV1::SoraTaira,
                target: SccpNetworkV1::EthereumMainnet,
            },
            message_id: [0; 32],
        }
    }
    /// Return the lower bound immediately after one commitment position at `height`.
    ///
    /// This is a search-only pagination sentinel. In particular, advancing after index 511
    /// produces index 512 so a forward range begins at the next older height; that sentinel is
    /// intentionally not persistable.
    #[must_use]
    pub const fn range_start_after(height: u64, commitment_index: u32) -> Option<Self> {
        if height == 0 || commitment_index >= SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 {
            return None;
        }
        Some(Self {
            recorded_at_height: height,
            commitment_index: commitment_index + 1,
            lane: SccpLaneIdV1 {
                source: SccpNetworkV1::SoraTaira,
                target: SccpNetworkV1::EthereumMainnet,
            },
            message_id: [0; 32],
        })
    }
    /// Return the authoritative composite replay key named by this locator.
    #[must_use]
    pub const fn message_key(self) -> SccpOutboundMessageKeyV1 {
        SccpOutboundMessageKeyV1 {
            lane: self.lane,
            message_id: self.message_id,
        }
    }
    /// Return whether the locator has a valid height and outbound replay key.
    #[must_use]
    pub fn is_well_formed(self) -> bool {
        self.recorded_at_height != 0
            && self.commitment_index < SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1
            && self.message_key().is_well_formed()
    }
}
impl Ord for SccpOutboundMessageIndexKeyV1 {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .recorded_at_height
            .cmp(&self.recorded_at_height)
            .then_with(|| self.commitment_index.cmp(&other.commitment_index))
            .then_with(|| self.lane.cmp(&other.lane))
            .then_with(|| self.message_id.cmp(&other.message_id))
    }
}
impl PartialOrd for SccpOutboundMessageIndexKeyV1 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
/// Fixed replay and discovery descriptor retained for every outbound SCCP message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpOutboundMessageDescriptorV1 {
    /// Governed destination binding authenticated at record admission.
    pub destination_binding_hash: [u8; 32],
    /// Immutable governed route configuration, including typed settlement.
    pub route_configuration_hash: [u8; 32],
    /// Hash of the exact canonical SCCP payload admitted for the key.
    pub payload_hash: [u8; 32],
    /// Local SORA block height at which the message was recorded.
    pub recorded_at_height: u64,
    /// Zero-based position authenticated by that block's SCCP commitment root.
    pub commitment_index: u32,
}
impl SccpOutboundMessageDescriptorV1 {
    /// Return whether the descriptor and replay key use four distinct nonzero hash roles.
    #[must_use]
    pub fn is_well_formed_for_key(&self, key: &SccpOutboundMessageKeyV1) -> bool {
        nonzero(&self.destination_binding_hash)
            && nonzero(&self.route_configuration_hash)
            && nonzero(&self.payload_hash)
            && self.destination_binding_hash != self.payload_hash
            && self.destination_binding_hash != self.route_configuration_hash
            && self.route_configuration_hash != self.payload_hash
            && self.recorded_at_height != 0
            && self.commitment_index < SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1
            && key.is_well_formed()
            && key.message_id != self.destination_binding_hash
            && key.message_id != self.route_configuration_hash
            && key.message_id != self.payload_hash
    }
}
/// Payload-bearing pending admission evidence for a SORA-origin outbound SCCP message.
///
/// The record is removed as soon as its destination proof is accepted. Its fixed descriptor and
/// replay transition are retained by the route-scoped accumulator and Kura's immutable
/// finalized-height archive, which also keeps the canonical payload available for historical
/// proof serving.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpOutboundPendingMessageRecordV1 {
    /// Governed destination binding authenticated at record admission.
    pub destination_binding_hash: [u8; 32],
    /// Immutable governed route configuration, including typed settlement.
    pub route_configuration_hash: [u8; 32],
    /// Hash of the exact canonical SCCP payload admitted for the key.
    pub payload_hash: [u8; 32],
    /// Exact canonical SCCP payload admitted for this authoritative outbox entry.
    ///
    /// Core admission and snapshot hydration revalidate these bytes against the lane, context,
    /// message identifier, and payload hash. The data-model layer enforces the shared storage
    /// bound without duplicating SCCP semantic decoding.
    pub payload_bytes: Vec<u8>,
    /// Local SORA block height at which the message was recorded.
    pub recorded_at_height: u64,
    /// Zero-based position authenticated by that block's SCCP commitment root.
    pub commitment_index: u32,
}
impl SccpOutboundPendingMessageRecordV1 {
    /// Return the fixed replay descriptor retained by the route accumulator and archive.
    #[must_use]
    pub const fn descriptor(&self) -> SccpOutboundMessageDescriptorV1 {
        SccpOutboundMessageDescriptorV1 {
            destination_binding_hash: self.destination_binding_hash,
            route_configuration_hash: self.route_configuration_hash,
            payload_hash: self.payload_hash,
            recorded_at_height: self.recorded_at_height,
            commitment_index: self.commitment_index,
        }
    }
    /// Return whether commitment roles are distinct and payload evidence is nonempty and bounded.
    ///
    /// This predicate is deliberately structural. Nodes must additionally perform canonical SCCP
    /// decode/re-encode and lane-bound identity validation before admitting untrusted records.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        nonzero(&self.destination_binding_hash)
            && nonzero(&self.route_configuration_hash)
            && nonzero(&self.payload_hash)
            && self.destination_binding_hash != self.payload_hash
            && self.destination_binding_hash != self.route_configuration_hash
            && self.route_configuration_hash != self.payload_hash
            && self.recorded_at_height != 0
            && self.commitment_index < SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1
            && !self.payload_bytes.is_empty()
            && self.payload_bytes.len() <= SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1
    }
    /// Return whether this bounded record and replay key use four distinct nonzero hash roles.
    #[must_use]
    pub fn is_well_formed_for_key(&self, key: &SccpOutboundMessageKeyV1) -> bool {
        self.is_well_formed() && self.descriptor().is_well_formed_for_key(key)
    }
}
/// Consensus-state usage of payload-bearing pending SCCP outbox entries.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpOutboundPendingUsageV1 {
    /// Number of payload-bearing pending messages.
    pub message_count: u64,
    /// Sum of their exact canonical payload byte lengths.
    pub payload_bytes: u64,
}
impl SccpOutboundPendingUsageV1 {
    /// Add one nonempty payload using checked arithmetic.
    #[must_use]
    pub fn checked_add_payload(self, payload_len: usize) -> Option<Self> {
        let payload_len = u64::try_from(payload_len).ok()?;
        if payload_len == 0 {
            return None;
        }
        Some(Self {
            message_count: self.message_count.checked_add(1)?,
            payload_bytes: self.payload_bytes.checked_add(payload_len)?,
        })
    }
    /// Remove one exact nonempty payload using checked arithmetic.
    #[must_use]
    pub fn checked_remove_payload(self, payload_len: usize) -> Option<Self> {
        let payload_len = u64::try_from(payload_len).ok()?;
        if payload_len == 0 {
            return None;
        }
        Some(Self {
            message_count: self.message_count.checked_sub(1)?,
            payload_bytes: self.payload_bytes.checked_sub(payload_len)?,
        })
    }
    /// Return whether zero/nonzero counters can describe a set of nonempty payloads.
    #[must_use]
    pub const fn is_structurally_valid(self) -> bool {
        (self.message_count == 0 && self.payload_bytes == 0)
            || (self.message_count != 0 && self.payload_bytes >= self.message_count)
    }
}
/// Durable high-water key for admissions under one governed native trust anchor.
///
/// The value stored under this key is the greatest authenticated backend-specific
/// consensus-progress coordinate admitted for the exact lane and anchor. Governance uses it to
/// prevent a successor checkpoint from retroactively excluding already accepted evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpInboundAnchorHighWaterKeyV1 {
    /// Exact external-to-SORA lane on which evidence was admitted.
    pub lane: SccpLaneIdV1,
    /// Nonzero hash of the retained native trust anchor used at admission.
    pub anchor_hash: [u8; 32],
}
impl SccpInboundAnchorHighWaterKeyV1 {
    /// Construct a high-water key after validating its topology and anchor hash.
    #[must_use]
    pub fn new(lane: SccpLaneIdV1, anchor_hash: [u8; 32]) -> Option<Self> {
        let key = Self { lane, anchor_hash };
        key.is_well_formed().then_some(key)
    }
    /// Return whether this is an external-to-SORA lane with a nonzero anchor hash.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.lane.is_well_formed()
            && self.lane.source.is_external()
            && self.lane.target.is_sora()
            && nonzero(&self.anchor_hash)
    }
}
/// Exact direct, non-proxy EVM transfer-route contract identity.
///
/// The first-release verifier admits only a concrete route implementation
/// whose immutable configuration makes source emission inseparable from a
/// successful transfer. The finalized execution proof opens the runtime code;
/// the canonical transfer event carries the same route-configuration hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpEvmSourceEmitterV1 {
    /// Canonical 20-byte contract address.
    pub address: [u8; 20],
    /// Keccak-256 hash of the deployed runtime bytecode.
    pub runtime_code_hash: [u8; 32],
    /// Keccak-256 commitment to the immutable route/token/network configuration.
    pub route_config_hash: [u8; 32],
}
/// Exact governed direct TRON transfer-route contract identity.
///
/// TRON headers do not commit smart-contract bytecode or immutable storage.
/// These fields are therefore an explicit governed deployment trust boundary;
/// native proofs still authenticate the successful concrete transfer call, its
/// sender, canonical arguments, and the block transaction root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpTronSourceEmitterV1 {
    /// Canonical 20-byte TRON account payload, without the network prefix.
    pub address: [u8; 20],
    /// Keccak-256 hash of the governed deployed TVM runtime bytecode.
    pub runtime_code_hash: [u8; 32],
    /// Keccak-256 commitment to the immutable route/token/network configuration.
    pub route_config_hash: [u8; 32],
}
/// Canonical TON raw account identifier.
///
/// Friendly/base64 address flags and checksums are presentation-only. SCCP
/// consensus state retains the signed workchain id and 256-bit account id
/// directly, so no network or bounceability flag can alter route identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpTonAddressV1 {
    /// Signed TON workchain identifier.
    pub workchain: i32,
    /// Raw 256-bit account identifier within the workchain.
    pub account: [u8; 32],
}
impl SccpTonAddressV1 {
    /// Return whether the raw account identifier is nonzero.
    #[must_use]
    pub fn is_well_formed(self) -> bool {
        nonzero(&self.account)
    }

    /// Return whether this address is a nonzero first-release basechain contract.
    #[must_use]
    pub fn is_sccp_basechain_contract(self) -> bool {
        self.workchain == SCCP_TON_BASECHAIN_WORKCHAIN_V1 && self.is_well_formed()
    }
}
/// Exact governed TON source bridge identity.
///
/// TON account-state proofs authenticate the account code hash and persistent
/// route commitment. V1 uses the same immutable bidirectional bridge contract
/// for native outbound events and proof-authenticated destination execution;
/// registry validation therefore requires this identity to match the TON
/// destination route address and code hash exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpTonSourceEmitterV1 {
    /// Canonical raw basechain source-bridge address.
    pub address: SccpTonAddressV1,
    /// TON representation hash of the immutable source-bridge code cell.
    pub code_hash: [u8; 32],
    /// Commitment to the immutable route/token/network configuration.
    pub route_config_hash: [u8; 32],
}
/// Exact source-bridge emitter identity for a supported external chain family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "emitter", content = "identity")]
pub enum SccpSourceEmitterV1 {
    /// EVM contract identity used by Ethereum and BSC profiles.
    #[codec(index = 0)]
    #[norito(rename = "evm")]
    Evm(SccpEvmSourceEmitterV1),
    /// TRON bridge-contract identity.
    #[codec(index = 1)]
    #[norito(rename = "tron")]
    Tron(SccpTronSourceEmitterV1),
    /// TON basechain source-bridge identity.
    #[codec(index = 3)]
    #[norito(rename = "ton")]
    Ton(SccpTonSourceEmitterV1),
}
impl SccpSourceEmitterV1 {
    /// Return whether this emitter variant belongs to the supplied network family.
    #[must_use]
    pub const fn matches_network(&self, network: SccpNetworkV1) -> bool {
        matches!(
            (self, network),
            (
                Self::Evm(_),
                SccpNetworkV1::EthereumMainnet | SccpNetworkV1::BscMainnet
            ) | (Self::Tron(_), SccpNetworkV1::TronMainnet)
                | (Self::Ton(_), SccpNetworkV1::TonMainnet)
        )
    }
    /// Return whether chain-specific identity roles match the exact network profile.
    ///
    /// Family invariants are encoded directly by their closed identity structs.
    #[must_use]
    pub fn matches_profile(&self, network: SccpNetworkV1) -> bool {
        self.matches_network(network)
    }
    /// Return whether this is admissible production source material for `network`.
    ///
    /// This method classifies only the external source. It does not classify a
    /// full lane, whose SORA target may still be Taira.
    #[must_use]
    pub fn is_production_source_for(&self, network: SccpNetworkV1) -> bool {
        network.is_production_profile() && self.is_governance_activatable_source_for(network)
    }
    /// Return whether reviewed material is complete enough for governance to
    /// activate native inbound settlement for `network`.
    ///
    /// Every external profile in the closed final-V1 inventory is a production profile.
    #[must_use]
    pub fn is_governance_activatable_source_for(&self, network: SccpNetworkV1) -> bool {
        network.is_external()
            && network.supports_native_inbound_source()
            && network.is_production_profile()
            && self.is_well_formed()
            && self.matches_profile(network)
    }
    /// Return whether every identity component is nonzero and role-separated.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        match self {
            Self::Evm(emitter) => {
                nonzero(&emitter.address)
                    && nonzero(&emitter.runtime_code_hash)
                    && nonzero(&emitter.route_config_hash)
                    && emitter.runtime_code_hash != emitter.route_config_hash
            }
            Self::Tron(emitter) => {
                nonzero(&emitter.address)
                    && nonzero(&emitter.runtime_code_hash)
                    && nonzero(&emitter.route_config_hash)
                    && emitter.runtime_code_hash != emitter.route_config_hash
            }
            Self::Ton(emitter) => {
                emitter.address.is_sccp_basechain_contract()
                    && nonzero(&emitter.code_hash)
                    && nonzero(&emitter.route_config_hash)
                    && emitter.code_hash != emitter.route_config_hash
            }
        }
    }
}
/// A typed external-source identity bound to one inbound SORA lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpSourceIdentityV1 {
    /// Inbound lane whose source identity is being authenticated.
    pub lane: SccpLaneIdV1,
    /// Chain-family-specific source emitter identity.
    pub emitter: SccpSourceEmitterV1,
}
impl SccpSourceIdentityV1 {
    /// Return whether this is a valid external-to-SORA identity with a matching emitter family.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.lane.is_well_formed()
            && self.lane.source.is_external()
            && self.lane.target.is_sora()
            && self.emitter.matches_profile(self.lane.source)
            && self.emitter.is_well_formed()
    }
    /// Return whether the external endpoint and emitter are admissible production source material.
    ///
    /// This deliberately says nothing about the SORA target; use
    /// [`Self::is_production_lane`] when the complete lane must be production.
    #[must_use]
    pub fn has_production_source(&self) -> bool {
        self.is_well_formed() && self.emitter.is_production_source_for(self.lane.source)
    }
    /// Return whether exact reviewed source material satisfies the closed governance-activation
    /// policy.
    #[must_use]
    pub fn has_governance_activatable_source(&self) -> bool {
        self.is_well_formed()
            && self
                .emitter
                .is_governance_activatable_source_for(self.lane.source)
    }
    /// Return whether the complete inbound lane uses production endpoints and source material.
    #[must_use]
    pub fn is_production_lane(&self) -> bool {
        self.has_production_source() && self.lane.is_production_environment()
    }
    /// Return whether the lane uses at least one staging or test endpoint.
    #[must_use]
    pub const fn uses_staging_environment(&self) -> bool {
        self.lane.is_staging_environment()
    }
}
fn nonzero<const N: usize>(bytes: &[u8; N]) -> bool {
    bytes.iter().any(|byte| *byte != 0)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::{BridgeNativeProofBackendV1, SccpNativeTrustAnchorV1};
    use norito::codec::DecodeAll as _;
    const NETWORKS: [SccpNetworkV1; 5] = [
        SccpNetworkV1::SoraTaira,
        SccpNetworkV1::EthereumMainnet,
        SccpNetworkV1::BscMainnet,
        SccpNetworkV1::TronMainnet,
        SccpNetworkV1::TonMainnet,
    ];
    fn evm_emitter() -> SccpSourceEmitterV1 {
        SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
            address: [1; 20],
            runtime_code_hash: [2; 32],
            route_config_hash: [3; 32],
        })
    }
    fn tron_emitter() -> SccpSourceEmitterV1 {
        SccpSourceEmitterV1::Tron(SccpTronSourceEmitterV1 {
            address: [4; 20],
            runtime_code_hash: [5; 32],
            route_config_hash: [6; 32],
        })
    }
    fn ton_address(byte: u8) -> SccpTonAddressV1 {
        SccpTonAddressV1 {
            workchain: SCCP_TON_BASECHAIN_WORKCHAIN_V1,
            account: [byte; 32],
        }
    }
    fn ton_emitter_value() -> SccpTonSourceEmitterV1 {
        SccpTonSourceEmitterV1 {
            address: ton_address(13),
            code_hash: [14; 32],
            route_config_hash: [15; 32],
        }
    }
    fn ton_emitter() -> SccpSourceEmitterV1 {
        SccpSourceEmitterV1::Ton(ton_emitter_value())
    }
    fn inbound_lane(source: SccpNetworkV1) -> SccpLaneIdV1 {
        SccpLaneIdV1 {
            source,
            target: SccpNetworkV1::SoraTaira,
        }
    }
    fn outbound_lane(target: SccpNetworkV1) -> SccpLaneIdV1 {
        SccpLaneIdV1 {
            source: SccpNetworkV1::SoraTaira,
            target,
        }
    }
    fn trust_anchor(backend: BridgeNativeProofBackendV1) -> SccpNativeTrustAnchorV1 {
        SccpNativeTrustAnchorV1 {
            backend,
            anchor_hash: [9; 32],
            checkpoint_height: 7,
        }
    }
    fn outbound_record() -> SccpOutboundPendingMessageRecordV1 {
        SccpOutboundPendingMessageRecordV1 {
            destination_binding_hash: [15; 32],
            route_configuration_hash: [16; 32],
            payload_hash: [17; 32],
            payload_bytes: vec![0x91],
            recorded_at_height: 10,
            commitment_index: 0,
        }
    }
    #[cfg(feature = "json")]
    fn insert_unknown_json_field(value: &mut norito::json::Value, path: &[&str]) {
        let mut current = value;
        for field in path {
            let norito::json::Value::Object(object) = current else {
                panic!("JSON path component `{field}` is not an object")
            };
            current = object
                .get_mut(*field)
                .unwrap_or_else(|| panic!("JSON path component `{field}` is absent"));
        }
        let norito::json::Value::Object(object) = current else {
            panic!("JSON target at {path:?} is not an object")
        };
        object.insert(
            "adversarial_extension".to_owned(),
            norito::json::Value::Null,
        );
    }
    #[test]
    fn network_inventory_and_profile_keys_are_exact() {
        assert_eq!(NETWORKS.len(), 5);
        for network in NETWORKS {
            assert_eq!(
                SccpNetworkV1::from_profile_key(network.profile_key()),
                Some(network)
            );
        }
        for unsupported in [
            "",
            "sora-nexus",
            "sora_nexus",
            "ethereum",
            "ethereum-sepolia",
            "ETHEREUM-MAINNET",
            "bsc-testnet",
            "tron-nile",
            "tron-shasta",
            "solana-mainnet-beta",
            "solana-testnet",
            "solana_testnet",
            "ton",
            "ton-testnet",
            "TON-MAINNET",
            "ton_mainnet",
        ] {
            assert_eq!(SccpNetworkV1::from_profile_key(unsupported), None);
        }
        assert_eq!(SccpNetworkV1::SoraTaira.domain_id(), 0);
        assert_eq!(SccpNetworkV1::EthereumMainnet.domain_id(), 1);
        assert_eq!(SccpNetworkV1::BscMainnet.domain_id(), 2);
        assert_eq!(SccpNetworkV1::TronMainnet.domain_id(), 5);
        assert_eq!(SccpNetworkV1::TonMainnet.domain_id(), 4);
        assert!(
            NETWORKS
                .into_iter()
                .all(SccpNetworkV1::is_production_profile)
        );
        assert!(
            NETWORKS
                .into_iter()
                .all(|network| !network.is_staging_profile())
        );
    }
    #[test]
    fn network_and_emitter_binary_roundtrips_cover_the_closed_inventory() {
        for network in NETWORKS {
            let encoded = network.encode();
            assert_eq!(
                SccpNetworkV1::decode_all(&mut encoded.as_slice()).expect("network decodes"),
                network
            );
        }
        for emitter in [evm_emitter(), tron_emitter(), ton_emitter()] {
            let encoded = emitter.encode();
            assert_eq!(
                SccpSourceEmitterV1::decode_all(&mut encoded.as_slice()).expect("emitter decodes"),
                emitter
            );
        }
    }
    #[test]
    fn unknown_binary_enum_tags_are_rejected() {
        for unsupported_tag in [
            0_u32,
            1,
            2,
            3,
            4,
            5,
            10,
            11,
            12,
            13,
            14,
            15,
            63,
            69,
            u32::MAX,
        ] {
            let encoded = unsupported_tag.encode();
            assert!(
                SccpNetworkV1::decode_all(&mut encoded.as_slice()).is_err(),
                "network tag {unsupported_tag} unexpectedly decoded"
            );
        }
        for unsupported_tag in [2_u32, 4, 5, u32::MAX] {
            let encoded = unsupported_tag.encode();
            assert!(
                SccpSourceEmitterV1::decode_all(&mut encoded.as_slice()).is_err(),
                "emitter tag {unsupported_tag} unexpectedly decoded"
            );
        }
    }
    #[test]
    fn binary_tags_use_the_fresh_final_v1_block() {
        let expected = [
            (SccpNetworkV1::SoraTaira, 0x40_u32),
            (SccpNetworkV1::EthereumMainnet, 0x41),
            (SccpNetworkV1::BscMainnet, 0x42),
            (SccpNetworkV1::TronMainnet, 0x43),
            (SccpNetworkV1::TonMainnet, 0x44),
        ];
        for (network, tag) in expected {
            assert_eq!(
                network.encode().get(..4),
                Some(tag.to_le_bytes().as_slice()),
                "wrong Norito tag for {network:?}"
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn unsupported_networks_and_emitters_are_not_json_decodable() {
        for profile in [
            "sora_nexus",
            "ethereum_sepolia",
            "bsc_testnet",
            "tron_nile",
            "tron_shasta",
            "solana_testnet",
            "ton_testnet",
            "unknown_network",
        ] {
            let json = format!(r#"{{"network":"{profile}","profile":null}}"#);
            assert!(
                norito::json::from_json::<SccpNetworkV1>(&json).is_err(),
                "unsupported profile {profile} unexpectedly decoded"
            );
        }
        for emitter in ["unknown_emitter"] {
            let json = format!(r#"{{"emitter":"{emitter}","identity":{{}}}}"#);
            assert!(
                norito::json::from_json::<SccpSourceEmitterV1>(&json).is_err(),
                "unsupported emitter {emitter} unexpectedly decoded"
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn json_roundtrips_advertise_only_first_release_profiles() {
        for network in NETWORKS {
            let json = norito::json::to_json(&network).expect("network serializes");
            assert_eq!(
                norito::json::from_json::<SccpNetworkV1>(&json).expect("network decodes"),
                network
            );
        }
        for emitter in [evm_emitter(), tron_emitter(), ton_emitter()] {
            let json = norito::json::to_json(&emitter).expect("emitter serializes");
            assert_eq!(
                norito::json::from_json::<SccpSourceEmitterV1>(&json).expect("emitter decodes"),
                emitter
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn source_identity_json_rejects_unknown_fields_recursively() {
        let identity = SccpSourceIdentityV1 {
            lane: inbound_lane(SccpNetworkV1::EthereumMainnet),
            emitter: evm_emitter(),
        };
        let json = norito::json::to_json(&identity).expect("source identity serializes");
        assert_eq!(
            norito::json::from_json::<SccpSourceIdentityV1>(&json)
                .expect("valid source identity decodes"),
            identity
        );
        for path in [
            &[][..],
            &["lane"][..],
            &["lane", "source"][..],
            &["emitter"][..],
            &["emitter", "identity"][..],
        ] {
            let mut hostile = norito::json::to_value(&identity).expect("serialize source identity");
            insert_unknown_json_field(&mut hostile, path);
            let hostile_json =
                norito::json::to_json(&hostile).expect("serialize hostile source identity");
            let error = norito::json::from_json::<SccpSourceIdentityV1>(&hostile_json)
                .expect_err("unknown source-identity field must fail");
            assert!(
                error.to_string().contains("adversarial_extension"),
                "unexpected error for path {path:?}: {error}"
            );
        }
    }
    #[test]
    fn lane_topology_matrix_is_exact_and_directional() {
        for source in NETWORKS {
            for target in NETWORKS {
                let lane = SccpLaneIdV1 { source, target };
                let expected = source.is_sora() != target.is_sora()
                    && source.domain_id() != target.domain_id();
                assert_eq!(
                    lane.is_well_formed(),
                    expected,
                    "unexpected topology result for {source:?} -> {target:?}"
                );
                assert_eq!(lane.external_network().is_some(), expected);
                assert_eq!(lane.sora_network().is_some(), expected);
            }
        }
        assert!(inbound_lane(SccpNetworkV1::EthereumMainnet).is_production_environment());
        assert!(outbound_lane(SccpNetworkV1::TronMainnet).is_production_environment());
    }
    #[test]
    fn native_source_support_is_closed_to_exact_external_inventory() {
        for network in NETWORKS {
            assert_eq!(
                network.supports_native_inbound_source(),
                network.is_external()
            );
        }
    }
    #[test]
    fn emitter_network_matrix_rejects_every_cross_family_pair() {
        for network in NETWORKS {
            assert_eq!(
                evm_emitter().matches_network(network),
                matches!(
                    network,
                    SccpNetworkV1::EthereumMainnet | SccpNetworkV1::BscMainnet
                )
            );
            assert_eq!(
                tron_emitter().matches_network(network),
                matches!(network, SccpNetworkV1::TronMainnet)
            );
            assert_eq!(
                ton_emitter().matches_network(network),
                matches!(network, SccpNetworkV1::TonMainnet)
            );
        }
    }
    #[test]
    fn emitter_components_are_nonzero_and_role_separated() {
        assert!(evm_emitter().is_well_formed());
        assert!(tron_emitter().is_well_formed());
        assert!(ton_emitter().is_well_formed());
        for invalid in [
            SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                address: [0; 20],
                runtime_code_hash: [2; 32],
                route_config_hash: [3; 32],
            }),
            SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                address: [1; 20],
                runtime_code_hash: [0; 32],
                route_config_hash: [3; 32],
            }),
            SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                address: [1; 20],
                runtime_code_hash: [2; 32],
                route_config_hash: [0; 32],
            }),
            SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                address: [1; 20],
                runtime_code_hash: [2; 32],
                route_config_hash: [2; 32],
            }),
            SccpSourceEmitterV1::Tron(SccpTronSourceEmitterV1 {
                address: [0; 20],
                runtime_code_hash: [5; 32],
                route_config_hash: [6; 32],
            }),
            SccpSourceEmitterV1::Tron(SccpTronSourceEmitterV1 {
                address: [4; 20],
                runtime_code_hash: [0; 32],
                route_config_hash: [6; 32],
            }),
            SccpSourceEmitterV1::Tron(SccpTronSourceEmitterV1 {
                address: [4; 20],
                runtime_code_hash: [5; 32],
                route_config_hash: [0; 32],
            }),
            SccpSourceEmitterV1::Tron(SccpTronSourceEmitterV1 {
                address: [4; 20],
                runtime_code_hash: [5; 32],
                route_config_hash: [5; 32],
            }),
            SccpSourceEmitterV1::Ton(SccpTonSourceEmitterV1 {
                address: ton_address(0),
                ..ton_emitter_value()
            }),
            SccpSourceEmitterV1::Ton(SccpTonSourceEmitterV1 {
                address: SccpTonAddressV1 {
                    workchain: SCCP_TON_MASTERCHAIN_WORKCHAIN_V1,
                    account: [13; 32],
                },
                ..ton_emitter_value()
            }),
            SccpSourceEmitterV1::Ton(SccpTonSourceEmitterV1 {
                code_hash: [0; 32],
                ..ton_emitter_value()
            }),
            SccpSourceEmitterV1::Ton(SccpTonSourceEmitterV1 {
                route_config_hash: [0; 32],
                ..ton_emitter_value()
            }),
            SccpSourceEmitterV1::Ton(SccpTonSourceEmitterV1 {
                route_config_hash: [14; 32],
                ..ton_emitter_value()
            }),
        ] {
            assert!(!invalid.is_well_formed(), "{invalid:?}");
        }
    }
    #[test]
    fn source_identity_rejects_wrong_direction_family_and_zero_material() {
        for (source, emitter) in [
            (SccpNetworkV1::EthereumMainnet, evm_emitter()),
            (SccpNetworkV1::BscMainnet, evm_emitter()),
            (SccpNetworkV1::TronMainnet, tron_emitter()),
            (SccpNetworkV1::TonMainnet, ton_emitter()),
        ] {
            let identity = SccpSourceIdentityV1 {
                lane: inbound_lane(source),
                emitter,
            };
            assert!(identity.is_well_formed());
            assert!(identity.has_governance_activatable_source());
            assert!(identity.has_production_source());
            assert!(identity.is_production_lane());
        }
        for identity in [
            SccpSourceIdentityV1 {
                lane: outbound_lane(SccpNetworkV1::EthereumMainnet),
                emitter: evm_emitter(),
            },
            SccpSourceIdentityV1 {
                lane: inbound_lane(SccpNetworkV1::EthereumMainnet),
                emitter: tron_emitter(),
            },
            SccpSourceIdentityV1 {
                lane: inbound_lane(SccpNetworkV1::TronMainnet),
                emitter: evm_emitter(),
            },
            SccpSourceIdentityV1 {
                lane: inbound_lane(SccpNetworkV1::EthereumMainnet),
                emitter: ton_emitter(),
            },
            SccpSourceIdentityV1 {
                lane: inbound_lane(SccpNetworkV1::TonMainnet),
                emitter: evm_emitter(),
            },
            SccpSourceIdentityV1 {
                lane: inbound_lane(SccpNetworkV1::EthereumMainnet),
                emitter: SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                    address: [0; 20],
                    runtime_code_hash: [2; 32],
                    route_config_hash: [3; 32],
                }),
            },
        ] {
            assert!(!identity.is_well_formed(), "{identity:?}");
            assert!(!identity.has_governance_activatable_source());
            assert!(!identity.has_production_source());
            assert!(!identity.is_production_lane());
        }
    }
    #[test]
    fn retained_replay_keys_reject_zero_ids_and_wrong_direction() {
        let inbound = inbound_lane(SccpNetworkV1::EthereumMainnet);
        let outbound = outbound_lane(SccpNetworkV1::EthereumMainnet);
        assert!(SccpInboundAnchorHighWaterKeyV1::new(inbound, [2; 32]).is_some());
        assert!(SccpInboundAnchorHighWaterKeyV1::new(inbound, [0; 32]).is_none());
        assert!(SccpInboundAnchorHighWaterKeyV1::new(outbound, [2; 32]).is_none());
        assert!(SccpOutboundMessageKeyV1::new(outbound, [1; 32]).is_some());
        assert!(SccpOutboundMessageKeyV1::new(outbound, [0; 32]).is_none());
        assert!(SccpOutboundMessageKeyV1::new(inbound, [1; 32]).is_none());
    }
    #[test]
    fn outbound_pending_record_schema_is_exact() {
        let schema = <SccpOutboundPendingMessageRecordV1 as iroha_schema::IntoSchema>::schema();
        let iroha_schema::Metadata::Struct(metadata) = schema
            .get::<SccpOutboundPendingMessageRecordV1>()
            .expect("pending outbound record has schema metadata")
        else {
            panic!("pending outbound record must have named-field schema metadata")
        };
        let field_names = metadata
            .declarations
            .iter()
            .map(|declaration| declaration.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            field_names,
            [
                "destination_binding_hash",
                "route_configuration_hash",
                "payload_hash",
                "payload_bytes",
                "recorded_at_height",
                "commitment_index",
            ]
        );
    }
    #[cfg(feature = "json")]
    #[test]
    fn retired_terminal_record_fields_are_rejected() {
        for retired_field in [
            "finality_block_hash",
            "destination_proof_commitment",
            "finality_height",
            "accepted_at_height",
        ] {
            let mut hostile =
                norito::json::to_value(&outbound_record()).expect("serialize outbound record");
            let norito::json::Value::Object(object) = &mut hostile else {
                panic!("outbound record JSON must be an object")
            };
            object.insert(retired_field.to_owned(), norito::json::Value::Null);
            let hostile_json =
                norito::json::to_json(&hostile).expect("serialize retired outbound shape");
            let error =
                norito::json::from_json::<SccpOutboundPendingMessageRecordV1>(&hostile_json)
                    .expect_err("retired terminal replay-map field must fail closed");
            assert!(
                error.to_string().contains(retired_field),
                "unexpected rejection for {retired_field}: {error}"
            );
        }
    }
    #[test]
    fn outbound_context_and_record_enforce_distinct_commitment_roles() {
        let lane = outbound_lane(SccpNetworkV1::EthereumMainnet);
        assert!(SccpOutboundMessageContextV1::new(lane, [1; 32], [2; 32]).is_some());
        assert!(SccpOutboundMessageContextV1::new(lane, [0; 32], [2; 32]).is_none());
        assert!(SccpOutboundMessageContextV1::new(lane, [1; 32], [0; 32]).is_none());
        assert!(SccpOutboundMessageContextV1::new(lane, [1; 32], [1; 32]).is_none());
        assert!(
            SccpOutboundMessageContextV1::new(
                inbound_lane(SccpNetworkV1::EthereumMainnet),
                [1; 32],
                [2; 32],
            )
            .is_none()
        );
        let key = SccpOutboundMessageKeyV1::new(lane, [18; 32]).expect("valid key");
        let record = outbound_record();
        assert!(record.is_well_formed_for_key(&key));
        for hostile in [
            SccpOutboundPendingMessageRecordV1 {
                destination_binding_hash: [0; 32],
                ..record.clone()
            },
            SccpOutboundPendingMessageRecordV1 {
                route_configuration_hash: record.destination_binding_hash,
                ..record.clone()
            },
            SccpOutboundPendingMessageRecordV1 {
                payload_hash: record.route_configuration_hash,
                ..record.clone()
            },
            SccpOutboundPendingMessageRecordV1 {
                recorded_at_height: 0,
                ..record.clone()
            },
            SccpOutboundPendingMessageRecordV1 {
                commitment_index: SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1,
                ..record.clone()
            },
            SccpOutboundPendingMessageRecordV1 {
                payload_bytes: Vec::new(),
                ..record.clone()
            },
            SccpOutboundPendingMessageRecordV1 {
                payload_bytes: vec![0x91; SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1 + 1],
                ..record.clone()
            },
        ] {
            assert!(!hostile.is_well_formed_for_key(&key), "{hostile:?}");
        }
        let colliding_key = SccpOutboundMessageKeyV1 {
            message_id: record.payload_hash,
            ..key
        };
        assert!(!record.is_well_formed_for_key(&colliding_key));
    }
    #[test]
    fn ordered_outbound_index_is_derived_and_self_checking() {
        let key =
            SccpOutboundMessageKeyV1::new(outbound_lane(SccpNetworkV1::TronMainnet), [18; 32])
                .expect("valid key");
        let record = outbound_record();
        let index = SccpOutboundMessageIndexKeyV1::new(key, &record).expect("valid ordered index");
        assert_eq!(index.recorded_at_height, record.recorded_at_height);
        assert_eq!(index.commitment_index, record.commitment_index);
        assert_eq!(index.message_key(), key);
        assert!(index.is_well_formed());
        assert!(
            SccpOutboundMessageIndexKeyV1::new(
                key,
                &SccpOutboundPendingMessageRecordV1 {
                    recorded_at_height: 0,
                    ..record.clone()
                },
            )
            .is_none()
        );
    }
    #[test]
    fn pending_usage_arithmetic_is_exact_and_fail_closed() {
        let empty = SccpOutboundPendingUsageV1::default();
        assert!(empty.is_structurally_valid());
        assert_eq!(empty.checked_add_payload(0), None);
        let one = empty.checked_add_payload(17).expect("add one payload");
        assert_eq!(one.message_count, 1);
        assert_eq!(one.payload_bytes, 17);
        assert!(one.is_structurally_valid());
        assert_eq!(one.checked_remove_payload(17), Some(empty));
        assert_eq!(one.checked_remove_payload(0), None);
        assert_eq!(one.checked_remove_payload(18), None);
        assert_eq!(
            SccpOutboundPendingUsageV1 {
                message_count: u64::MAX,
                payload_bytes: 1,
            }
            .checked_add_payload(1),
            None
        );
        assert_eq!(
            SccpOutboundPendingUsageV1 {
                message_count: 1,
                payload_bytes: u64::MAX,
            }
            .checked_add_payload(1),
            None
        );
        assert!(
            !SccpOutboundPendingUsageV1 {
                message_count: 0,
                payload_bytes: 1,
            }
            .is_structurally_valid()
        );
        assert!(
            !SccpOutboundPendingUsageV1 {
                message_count: 2,
                payload_bytes: 1,
            }
            .is_structurally_valid()
        );
    }
    #[test]
    fn ordered_outbound_index_seeks_newest_at_or_before_height() {
        use std::collections::BTreeSet;
        let index_at = |height: u64, commitment_index: u32, target: SccpNetworkV1, id: u8| {
            let key = SccpOutboundMessageKeyV1::new(outbound_lane(target), [id; 32])
                .expect("valid outbound key");
            SccpOutboundMessageIndexKeyV1::new(
                key,
                &SccpOutboundPendingMessageRecordV1 {
                    recorded_at_height: height,
                    commitment_index,
                    ..outbound_record()
                },
            )
            .expect("valid ordered index")
        };
        let entries = BTreeSet::from([
            index_at(1, 0, SccpNetworkV1::EthereumMainnet, 1),
            index_at(40, 1, SccpNetworkV1::EthereumMainnet, 2),
            index_at(40, 0, SccpNetworkV1::TronMainnet, 3),
            index_at(41, 0, SccpNetworkV1::BscMainnet, 4),
            index_at(u64::MAX, 0, SccpNetworkV1::EthereumMainnet, 5),
        ]);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.recorded_at_height)
                .collect::<Vec<_>>(),
            [u64::MAX, 41, 40, 40, 1]
        );
        let selected = entries
            .range(SccpOutboundMessageIndexKeyV1::range_start_at_or_before(40)..)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            selected
                .iter()
                .map(|entry| entry.recorded_at_height)
                .collect::<Vec<_>>(),
            [40, 40, 1]
        );
        assert_eq!(selected[0].commitment_index, 0);
        assert_eq!(selected[0].lane.target, SccpNetworkV1::TronMainnet);
        assert_eq!(selected[1].commitment_index, 1);
        assert_eq!(selected[1].lane.target, SccpNetworkV1::EthereumMainnet);
        assert!(
            !SccpOutboundMessageIndexKeyV1::range_start_at_or_before(40).is_well_formed(),
            "range sentinel must never be a persistable index entry"
        );
        let after_first =
            SccpOutboundMessageIndexKeyV1::range_start_after(40, 0).expect("valid compound cursor");
        assert_eq!(after_first.commitment_index, 1);
        assert_eq!(
            entries
                .range(after_first..)
                .next()
                .map(|entry| (entry.recorded_at_height, entry.commitment_index)),
            Some((40, 1))
        );
        let after_last = SccpOutboundMessageIndexKeyV1::range_start_after(
            40,
            SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 - 1,
        )
        .expect("last valid position has an older-height sentinel");
        assert_eq!(
            after_last.commitment_index,
            SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1
        );
        assert!(!after_last.is_well_formed());
        assert!(SccpOutboundMessageIndexKeyV1::range_start_after(0, 0).is_none());
        assert!(
            SccpOutboundMessageIndexKeyV1::range_start_after(
                40,
                SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1,
            )
            .is_none()
        );
    }
    #[test]
    fn trust_anchor_rejects_zero_hash_or_height() {
        assert!(trust_anchor(BridgeNativeProofBackendV1::EthereumBeacon).is_well_formed());
        assert!(
            !SccpNativeTrustAnchorV1 {
                anchor_hash: [0; 32],
                ..trust_anchor(BridgeNativeProofBackendV1::EthereumBeacon)
            }
            .is_well_formed()
        );
        assert!(
            !SccpNativeTrustAnchorV1 {
                checkpoint_height: 0,
                ..trust_anchor(BridgeNativeProofBackendV1::EthereumBeacon)
            }
            .is_well_formed()
        );
    }
    #[test]
    fn trust_anchor_interval_has_one_height_successor_overlap_and_current_is_open_ended() {
        let mut anchor = trust_anchor(BridgeNativeProofBackendV1::EthereumBeacon);
        anchor.checkpoint_height = 100;
        assert!(!anchor.admits_anchor_interval_height(99, Some(200)));
        assert!(anchor.admits_anchor_interval_height(100, Some(200)));
        assert!(anchor.admits_anchor_interval_height(199, Some(200)));
        assert!(anchor.admits_anchor_interval_height(200, Some(200)));
        assert!(!anchor.admits_anchor_interval_height(201, Some(200)));
        assert!(anchor.admits_anchor_interval_height(u64::MAX, None));
        let next = SccpNativeTrustAnchorV1 {
            anchor_hash: [0xA7; 32],
            checkpoint_height: 200,
            ..anchor
        };
        assert!(next.admits_anchor_interval_height(200, None));
        assert!(next.admits_anchor_interval_height(201, None));
    }
}
