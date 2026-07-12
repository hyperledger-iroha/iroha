//! Versioned SCCP network, lane, and source-identity wire types.
//!
//! These types deliberately model only the first-release network inventory.
//! There is no catch-all network, emitter, or arbitrary network identifier:
//! unsupported profiles must fail decoding instead of being interpreted by
//! node-local policy.

use core::cmp::Ordering;

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::SccpNativeTrustAnchorV1;

/// Largest integer encoded as a JSON number by the closed SCCP V1 capability surface.
///
/// Capping advertised byte budgets at `2^53 - 1` keeps their exact value portable across every
/// supported SDK, including runtimes whose JSON number type is IEEE-754 binary64.
pub const SCCP_V1_JSON_SAFE_INTEGER_MAX: u64 = (1_u64 << 53) - 1;

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
/// also limiting blocks whose transactions contain multiple SCCP instructions. A transaction
/// that would exceed this bound is rejected atomically, so failed execution never consumes an
/// index.
pub const SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1: u32 = 512;

/// A supported SCCP network profile for the V1 wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "network", content = "profile")]
pub enum SccpNetworkV1 {
    /// The sole production SORA endpoint admitted by SCCP V1.
    // Tag 0 is permanently reserved for the removed pre-release SORA profile.
    // Exact V1 contract and governance hashes commit Taira as tag 1.
    #[codec(index = 1)]
    #[norito(rename = "sora_taira")]
    SoraTaira,
    /// Ethereum mainnet.
    #[codec(index = 2)]
    #[norito(rename = "ethereum_mainnet")]
    EthereumMainnet,
    /// Ethereum Sepolia testnet.
    #[codec(index = 3)]
    #[norito(rename = "ethereum_sepolia")]
    EthereumSepolia,
    /// BNB Smart Chain mainnet.
    #[codec(index = 4)]
    #[norito(rename = "bsc_mainnet")]
    BscMainnet,
    /// BNB Smart Chain testnet.
    #[codec(index = 5)]
    #[norito(rename = "bsc_testnet")]
    BscTestnet,
    /// TRON mainnet.
    // Tags 6 through 9 are intentionally reserved. They belonged to retired
    // pre-release profile identities and must never be reassigned: the exact
    // transfer contracts already commit TRON as 10 through 12.
    #[codec(index = 10)]
    #[norito(rename = "tron_mainnet")]
    TronMainnet,
    /// TRON Nile testnet.
    #[codec(index = 11)]
    #[norito(rename = "tron_nile")]
    TronNile,
    /// TRON Shasta testnet.
    #[codec(index = 12)]
    #[norito(rename = "tron_shasta")]
    TronShasta,
}

impl SccpNetworkV1 {
    /// Return the SCCP protocol domain carried by messages for this profile.
    #[must_use]
    pub const fn domain_id(self) -> u32 {
        match self {
            Self::SoraTaira => 0,
            Self::EthereumMainnet | Self::EthereumSepolia => 1,
            Self::BscMainnet | Self::BscTestnet => 2,
            Self::TronMainnet | Self::TronNile | Self::TronShasta => 5,
        }
    }

    /// Return the canonical, stable textual key for this exact profile.
    #[must_use]
    pub const fn profile_key(self) -> &'static str {
        match self {
            Self::SoraTaira => "sora-taira",
            Self::EthereumMainnet => "ethereum-mainnet",
            Self::EthereumSepolia => "ethereum-sepolia",
            Self::BscMainnet => "bsc-mainnet",
            Self::BscTestnet => "bsc-testnet",
            Self::TronMainnet => "tron-mainnet",
            Self::TronNile => "tron-nile",
            Self::TronShasta => "tron-shasta",
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
            "ethereum-sepolia" => Some(Self::EthereumSepolia),
            "bsc-mainnet" => Some(Self::BscMainnet),
            "bsc-testnet" => Some(Self::BscTestnet),
            "tron-mainnet" => Some(Self::TronMainnet),
            "tron-nile" => Some(Self::TronNile),
            "tron-shasta" => Some(Self::TronShasta),
            _ => None,
        }
    }

    /// Return whether this is a production network profile.
    #[must_use]
    pub const fn is_production_profile(self) -> bool {
        matches!(
            self,
            Self::SoraTaira | Self::EthereumMainnet | Self::BscMainnet | Self::TronMainnet
        )
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
            Self::EthereumMainnet
                | Self::EthereumSepolia
                | Self::BscMainnet
                | Self::BscTestnet
                | Self::TronMainnet
                | Self::TronNile
                | Self::TronShasta
        )
    }
}

/// A directed SCCP lane between two exact V1 network profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
/// Exact network profiles prevent messages on two networks in the same SCCP
/// domain from aliasing each other. The destination binding is intentionally
/// excluded so rotating a governed rollout cannot replay an already-recorded
/// lane-bound message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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

    /// Build an ordered locator from one accepted terminal proof record.
    #[must_use]
    pub fn from_terminal(
        key: SccpOutboundMessageKeyV1,
        record: &SccpOutboundProofRecordV1,
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
    /// This sentinel is only a search bound and must never be persisted: its
    /// zero message identifier intentionally sorts before every well-formed
    /// entry at the same height. Since index ordering reverses height, a
    /// forward range beginning here contains exactly entries recorded at or
    /// before `height`, newest first.
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
/// The record is removed as soon as its destination proof is accepted. Its fixed descriptor
/// moves to [`SccpOutboundProofRecordV1`], while Kura's immutable finalized-height archive keeps
/// the canonical payload available for historical proof serving.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// Return the fixed replay descriptor shared with the terminal proof record.
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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

/// Durable accepted destination-proof evidence for a SORA-origin SCCP message.
///
/// The authoritative [`SccpOutboundMessageKeyV1`] is reused as the replay key,
/// so a deterministic `BTreeMap` lookup identifies one exact outbound lane and
/// message in `O(log n)` time. Only fixed-size commitments are retained here;
/// proof history may be pruned without weakening proof-submission replay protection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpOutboundProofRecordV1 {
    /// Hash of the exact canonical SCCP payload authenticated for the key.
    pub payload_hash: [u8; 32],
    /// Governed destination binding authenticated by the accepted artifact.
    pub destination_binding_hash: [u8; 32],
    /// Immutable governed route configuration authenticated by the artifact.
    pub route_configuration_hash: [u8; 32],
    /// Finalized Taira block hash authenticated by the destination proof.
    pub finality_block_hash: [u8; 32],
    /// Domain-separated proof-registry commitment to the accepted proof.
    pub destination_proof_commitment: [u8; 32],
    /// Finalized Taira height containing the authoritative outbound message.
    pub finality_height: u64,
    /// Zero-based position authenticated by the finalized block's SCCP commitment root.
    pub commitment_index: u32,
    /// Local Taira height at which proof admission was committed.
    pub accepted_at_height: u64,
}

impl SccpOutboundProofRecordV1 {
    /// Return the fixed descriptor retained after pending payload removal.
    #[must_use]
    pub const fn descriptor(self) -> SccpOutboundMessageDescriptorV1 {
        SccpOutboundMessageDescriptorV1 {
            destination_binding_hash: self.destination_binding_hash,
            route_configuration_hash: self.route_configuration_hash,
            payload_hash: self.payload_hash,
            recorded_at_height: self.finality_height,
            commitment_index: self.commitment_index,
        }
    }

    /// Return whether every commitment and both heights are nonzero and every
    /// hash role is distinct from the lane-bound message identifier.
    #[must_use]
    pub fn is_well_formed_for_key(&self, key: &SccpOutboundMessageKeyV1) -> bool {
        let hashes = [
            key.message_id,
            self.payload_hash,
            self.destination_binding_hash,
            self.route_configuration_hash,
            self.finality_block_hash,
            self.destination_proof_commitment,
        ];
        key.is_well_formed()
            && self.finality_height != 0
            && self.commitment_index < SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1
            && self.accepted_at_height != 0
            && self.accepted_at_height >= self.finality_height
            && self.descriptor().is_well_formed_for_key(key)
            && hashes.iter().all(nonzero)
            && hashes
                .iter()
                .enumerate()
                .all(|(index, hash)| !hashes[index + 1..].contains(hash))
    }
}

/// Durable replay key for a native external-to-SORA SCCP message.
///
/// The exact source and target profiles are part of the key. Consequently, a
/// message identifier observed on a test network, another external chain, or a
/// different SORA deployment cannot alias an admitted production message. A
/// `BTreeMap` keyed by this type provides deterministic `O(log n)` replay
/// checks in world state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpInboundMessageKeyV1 {
    /// Exact external-to-SORA lane on which the message was authenticated.
    pub lane: SccpLaneIdV1,
    /// Nonzero identifier derived from the canonical SCCP message payload.
    pub message_id: [u8; 32],
}

impl SccpInboundMessageKeyV1 {
    /// Construct a replay key after validating its inbound topology and identifier.
    #[must_use]
    pub fn new(lane: SccpLaneIdV1, message_id: [u8; 32]) -> Option<Self> {
        let key = Self { lane, message_id };
        key.is_well_formed().then_some(key)
    }

    /// Return whether this is an external-to-SORA lane with a nonzero message identifier.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.lane.is_well_formed()
            && self.lane.source.is_external()
            && self.lane.target.is_sora()
            && nonzero(&self.message_id)
    }
}

/// Durable high-water key for admissions under one governed native trust anchor.
///
/// The value stored under this key is the greatest authenticated
/// backend-specific consensus-progress coordinate admitted for the exact lane
/// and anchor. Governance uses it to prevent a successor checkpoint from
/// retroactively excluding already accepted evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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

/// Durable admission evidence bound to an inbound SCCP replay key.
///
/// This record stores only fixed-size commitments. The accepted native proof
/// remains reproducibly identifiable without retaining attacker-controlled
/// proof bytes in the replay index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpInboundMessageRecordV1 {
    /// Hash of the exact canonical SCCP payload admitted for the key.
    pub payload_hash: [u8; 32],
    /// Immutable governed route configuration, including typed settlement.
    pub route_configuration_hash: [u8; 32],
    /// Domain-separated hash of the exact typed source identity used at admission.
    pub source_identity_hash: [u8; 32],
    /// Governed native verifier and checkpoint commitment used at admission.
    pub trust_anchor: SccpNativeTrustAnchorV1,
    /// Authenticated backend-specific consensus-progress coordinate used for
    /// trust-anchor interval and retired-route cutoff admission.
    ///
    /// Ethereum lanes persist the finalized beacon slot. BSC and TRON lanes
    /// persist the finalized block height.
    pub anchor_interval_height: u64,
    /// Native source-chain finality height authenticated by the verifier.
    pub source_finality_height: u64,
    /// Native source-chain finalized block or checkpoint hash.
    pub source_finality_hash: [u8; 32],
    /// Domain-separated proof-registry commitment to the exact accepted native bridge proof.
    pub source_proof_commitment: [u8; 32],
    /// Local SORA block height at which admission was committed.
    pub admitted_at_height: u64,
}

impl SccpInboundMessageRecordV1 {
    /// Return whether every commitment and authenticated coordinate is valid.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        nonzero(&self.payload_hash)
            && nonzero(&self.route_configuration_hash)
            && nonzero(&self.source_identity_hash)
            && self.route_configuration_hash != self.payload_hash
            && self.route_configuration_hash != self.source_identity_hash
            && self.trust_anchor.is_well_formed()
            && self.anchor_interval_height >= self.trust_anchor.checkpoint_height
            && self.source_finality_height != 0
            && nonzero(&self.source_finality_hash)
            && nonzero(&self.source_proof_commitment)
            && self.admitted_at_height != 0
    }

    /// Return whether the recorded native verifier belongs to the key's exact source family.
    #[must_use]
    pub fn is_well_formed_for_lane(&self, lane: SccpLaneIdV1) -> bool {
        self.is_well_formed()
            && lane.is_well_formed()
            && lane.source.is_external()
            && lane.target.is_sora()
            && self
                .trust_anchor
                .backend
                .supports_source_network(lane.source)
    }
}

/// Exact direct, non-proxy EVM transfer-route contract identity.
///
/// The first-release verifier admits only a concrete route implementation
/// whose immutable configuration makes source emission inseparable from a
/// successful transfer. The finalized execution proof opens the runtime code;
/// the canonical transfer event carries the same route-configuration hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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

/// Exact source-bridge emitter identity for a supported external chain family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
}

impl SccpSourceEmitterV1 {
    /// Return whether this emitter variant belongs to the supplied network family.
    #[must_use]
    pub const fn matches_network(&self, network: SccpNetworkV1) -> bool {
        matches!(
            (self, network),
            (
                Self::Evm(_),
                SccpNetworkV1::EthereumMainnet
                    | SccpNetworkV1::EthereumSepolia
                    | SccpNetworkV1::BscMainnet
                    | SccpNetworkV1::BscTestnet
            ) | (
                Self::Tron(_),
                SccpNetworkV1::TronMainnet | SccpNetworkV1::TronNile | SccpNetworkV1::TronShasta
            )
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
        network.is_production_profile()
            && network.is_external()
            && network.supports_native_inbound_source()
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
        }
    }
}

/// A typed external-source identity bound to one inbound SORA lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    use crate::bridge::BridgeNativeProofBackendV1;
    use norito::codec::DecodeAll as _;

    const NETWORKS: [SccpNetworkV1; 8] = [
        SccpNetworkV1::SoraTaira,
        SccpNetworkV1::EthereumMainnet,
        SccpNetworkV1::EthereumSepolia,
        SccpNetworkV1::BscMainnet,
        SccpNetworkV1::BscTestnet,
        SccpNetworkV1::TronMainnet,
        SccpNetworkV1::TronNile,
        SccpNetworkV1::TronShasta,
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

    fn inbound_record(backend: BridgeNativeProofBackendV1) -> SccpInboundMessageRecordV1 {
        SccpInboundMessageRecordV1 {
            payload_hash: [10; 32],
            route_configuration_hash: [11; 32],
            source_identity_hash: [12; 32],
            trust_anchor: trust_anchor(backend),
            anchor_interval_height: 7,
            source_finality_height: 8,
            source_finality_hash: [13; 32],
            source_proof_commitment: [14; 32],
            admitted_at_height: 9,
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

    fn outbound_proof_record() -> SccpOutboundProofRecordV1 {
        SccpOutboundProofRecordV1 {
            payload_hash: [17; 32],
            destination_binding_hash: [15; 32],
            route_configuration_hash: [16; 32],
            finality_block_hash: [19; 32],
            destination_proof_commitment: [20; 32],
            finality_height: 10,
            commitment_index: 0,
            accepted_at_height: 11,
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
        assert_eq!(NETWORKS.len(), 8);
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
            "ETHEREUM-MAINNET",
            "solana-mainnet-beta",
            "solana-testnet",
            "ton-mainnet",
            "ton-testnet",
        ] {
            assert_eq!(SccpNetworkV1::from_profile_key(unsupported), None);
        }

        assert_eq!(SccpNetworkV1::SoraTaira.domain_id(), 0);
        assert_eq!(SccpNetworkV1::EthereumMainnet.domain_id(), 1);
        assert_eq!(SccpNetworkV1::EthereumSepolia.domain_id(), 1);
        assert_eq!(SccpNetworkV1::BscMainnet.domain_id(), 2);
        assert_eq!(SccpNetworkV1::BscTestnet.domain_id(), 2);
        assert_eq!(SccpNetworkV1::TronMainnet.domain_id(), 5);
        assert_eq!(SccpNetworkV1::TronNile.domain_id(), 5);
        assert_eq!(SccpNetworkV1::TronShasta.domain_id(), 5);
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

        for emitter in [evm_emitter(), tron_emitter()] {
            let encoded = emitter.encode();
            assert_eq!(
                SccpSourceEmitterV1::decode_all(&mut encoded.as_slice()).expect("emitter decodes"),
                emitter
            );
        }
    }

    #[test]
    fn unknown_binary_enum_tags_are_rejected() {
        for unsupported_tag in [0_u32, 6, 7, 8, 9, 13, u32::MAX] {
            let encoded = unsupported_tag.encode();
            assert!(
                SccpNetworkV1::decode_all(&mut encoded.as_slice()).is_err(),
                "network tag {unsupported_tag} unexpectedly decoded"
            );
        }

        for unsupported_tag in [2_u32, 3, u32::MAX] {
            let encoded = unsupported_tag.encode();
            assert!(
                SccpSourceEmitterV1::decode_all(&mut encoded.as_slice()).is_err(),
                "emitter tag {unsupported_tag} unexpectedly decoded"
            );
        }
    }

    #[test]
    fn binary_tags_reserve_zero_and_six_through_nine_and_match_contract_profiles() {
        let expected = [
            (SccpNetworkV1::SoraTaira, 1_u32),
            (SccpNetworkV1::EthereumMainnet, 2),
            (SccpNetworkV1::EthereumSepolia, 3),
            (SccpNetworkV1::BscMainnet, 4),
            (SccpNetworkV1::BscTestnet, 5),
            (SccpNetworkV1::TronMainnet, 10),
            (SccpNetworkV1::TronNile, 11),
            (SccpNetworkV1::TronShasta, 12),
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
    fn removed_networks_and_emitters_are_not_json_decodable() {
        for profile in [
            "sora_nexus",
            "solana_mainnet_beta",
            "solana_testnet",
            "ton_mainnet",
            "ton_testnet",
            "unknown_network",
        ] {
            let json = format!(r#"{{"network":"{profile}","profile":null}}"#);
            assert!(
                norito::json::from_json::<SccpNetworkV1>(&json).is_err(),
                "removed profile {profile} unexpectedly decoded"
            );
        }

        for emitter in ["solana", "ton", "unknown_emitter"] {
            let json = format!(r#"{{"emitter":"{emitter}","identity":{{}}}}"#);
            assert!(
                norito::json::from_json::<SccpSourceEmitterV1>(&json).is_err(),
                "removed emitter {emitter} unexpectedly decoded"
            );
        }
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_roundtrips_advertise_only_first_release_profiles() {
        for network in NETWORKS {
            let json = norito::json::to_json(&network).expect("network serializes");
            assert!(!json.to_ascii_lowercase().contains("solana"));
            assert!(!json.to_ascii_lowercase().contains("ton_"));
            assert_eq!(
                norito::json::from_json::<SccpNetworkV1>(&json).expect("network decodes"),
                network
            );
        }

        for emitter in [evm_emitter(), tron_emitter()] {
            let json = norito::json::to_json(&emitter).expect("emitter serializes");
            assert!(!json.to_ascii_lowercase().contains("solana"));
            assert!(!json.to_ascii_lowercase().contains(r#""ton""#));
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
        assert!(!inbound_lane(SccpNetworkV1::EthereumSepolia).is_production_environment());
        assert!(outbound_lane(SccpNetworkV1::TronNile).is_staging_environment());
    }

    #[test]
    fn native_source_support_is_closed_to_evm_bsc_and_tron() {
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
                    SccpNetworkV1::EthereumMainnet
                        | SccpNetworkV1::EthereumSepolia
                        | SccpNetworkV1::BscMainnet
                        | SccpNetworkV1::BscTestnet
                )
            );
            assert_eq!(
                tron_emitter().matches_network(network),
                matches!(
                    network,
                    SccpNetworkV1::TronMainnet
                        | SccpNetworkV1::TronNile
                        | SccpNetworkV1::TronShasta
                )
            );
        }
    }

    #[test]
    fn emitter_components_are_nonzero_and_role_separated() {
        assert!(evm_emitter().is_well_formed());
        assert!(tron_emitter().is_well_formed());

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
        ] {
            let identity = SccpSourceIdentityV1 {
                lane: inbound_lane(source),
                emitter,
            };
            assert!(identity.is_well_formed());
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
                emitter: SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                    address: [0; 20],
                    runtime_code_hash: [2; 32],
                    route_config_hash: [3; 32],
                }),
            },
        ] {
            assert!(!identity.is_well_formed(), "{identity:?}");
            assert!(!identity.has_production_source());
            assert!(!identity.is_production_lane());
        }
    }

    #[test]
    fn replay_keys_reject_zero_ids_and_wrong_direction() {
        let inbound = inbound_lane(SccpNetworkV1::EthereumMainnet);
        let outbound = outbound_lane(SccpNetworkV1::EthereumMainnet);

        assert!(SccpInboundMessageKeyV1::new(inbound, [1; 32]).is_some());
        assert!(SccpInboundMessageKeyV1::new(inbound, [0; 32]).is_none());
        assert!(SccpInboundMessageKeyV1::new(outbound, [1; 32]).is_none());
        assert!(SccpInboundAnchorHighWaterKeyV1::new(inbound, [2; 32]).is_some());
        assert!(SccpInboundAnchorHighWaterKeyV1::new(inbound, [0; 32]).is_none());
        assert!(SccpInboundAnchorHighWaterKeyV1::new(outbound, [2; 32]).is_none());

        assert!(SccpOutboundMessageKeyV1::new(outbound, [1; 32]).is_some());
        assert!(SccpOutboundMessageKeyV1::new(outbound, [0; 32]).is_none());
        assert!(SccpOutboundMessageKeyV1::new(inbound, [1; 32]).is_none());
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
        assert_eq!(
            SccpOutboundMessageIndexKeyV1::from_terminal(key, &outbound_proof_record()),
            Some(index),
            "moving a message to terminal replay state must preserve its ordered locator"
        );

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
    fn outbound_proof_record_is_fixed_size_and_rejects_aliases_and_invalid_heights() {
        let key =
            SccpOutboundMessageKeyV1::new(outbound_lane(SccpNetworkV1::EthereumMainnet), [18; 32])
                .expect("valid outbound proof key");
        let record = outbound_proof_record();
        assert!(record.is_well_formed_for_key(&key));

        let encoded = record.encode();
        assert_eq!(
            SccpOutboundProofRecordV1::decode_all(&mut encoded.as_slice())
                .expect("outbound proof record must roundtrip"),
            record
        );

        for hostile in [
            SccpOutboundProofRecordV1 {
                payload_hash: [0; 32],
                ..record
            },
            SccpOutboundProofRecordV1 {
                destination_binding_hash: record.payload_hash,
                ..record
            },
            SccpOutboundProofRecordV1 {
                route_configuration_hash: record.destination_binding_hash,
                ..record
            },
            SccpOutboundProofRecordV1 {
                finality_block_hash: record.route_configuration_hash,
                ..record
            },
            SccpOutboundProofRecordV1 {
                destination_proof_commitment: record.finality_block_hash,
                ..record
            },
            SccpOutboundProofRecordV1 {
                finality_height: 0,
                ..record
            },
            SccpOutboundProofRecordV1 {
                commitment_index: SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1,
                ..record
            },
            SccpOutboundProofRecordV1 {
                accepted_at_height: record.finality_height - 1,
                ..record
            },
        ] {
            assert!(!hostile.is_well_formed_for_key(&key), "{hostile:?}");
        }

        for collision in [
            record.payload_hash,
            record.destination_binding_hash,
            record.route_configuration_hash,
            record.finality_block_hash,
            record.destination_proof_commitment,
        ] {
            assert!(!record.is_well_formed_for_key(&SccpOutboundMessageKeyV1 {
                message_id: collision,
                ..key
            }));
        }
        let inbound_key = SccpOutboundMessageKeyV1 {
            lane: inbound_lane(SccpNetworkV1::EthereumMainnet),
            ..key
        };
        assert!(!record.is_well_formed_for_key(&inbound_key));
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
    fn inbound_record_requires_exact_backend_and_distinct_commitments() {
        let cases = [
            (
                SccpNetworkV1::EthereumMainnet,
                BridgeNativeProofBackendV1::EthereumBeacon,
            ),
            (
                SccpNetworkV1::EthereumSepolia,
                BridgeNativeProofBackendV1::EthereumBeacon,
            ),
            (
                SccpNetworkV1::BscMainnet,
                BridgeNativeProofBackendV1::BscParlia,
            ),
            (
                SccpNetworkV1::BscTestnet,
                BridgeNativeProofBackendV1::BscParlia,
            ),
            (
                SccpNetworkV1::TronMainnet,
                BridgeNativeProofBackendV1::TronDpos,
            ),
            (
                SccpNetworkV1::TronNile,
                BridgeNativeProofBackendV1::TronDpos,
            ),
            (
                SccpNetworkV1::TronShasta,
                BridgeNativeProofBackendV1::TronDpos,
            ),
        ];

        for (source, backend) in cases {
            let record = inbound_record(backend);
            let lane = inbound_lane(source);
            assert!(record.is_well_formed_for_lane(lane));

            for wrong in [
                BridgeNativeProofBackendV1::EthereumBeacon,
                BridgeNativeProofBackendV1::BscParlia,
                BridgeNativeProofBackendV1::TronDpos,
            ] {
                assert_eq!(
                    SccpInboundMessageRecordV1 {
                        trust_anchor: trust_anchor(wrong),
                        ..record
                    }
                    .is_well_formed_for_lane(lane),
                    wrong == backend
                );
            }
        }

        let valid = inbound_record(BridgeNativeProofBackendV1::EthereumBeacon);
        for hostile in [
            SccpInboundMessageRecordV1 {
                payload_hash: [0; 32],
                ..valid
            },
            SccpInboundMessageRecordV1 {
                route_configuration_hash: valid.payload_hash,
                ..valid
            },
            SccpInboundMessageRecordV1 {
                source_identity_hash: valid.route_configuration_hash,
                ..valid
            },
            SccpInboundMessageRecordV1 {
                anchor_interval_height: 0,
                ..valid
            },
            SccpInboundMessageRecordV1 {
                anchor_interval_height: valid.trust_anchor.checkpoint_height - 1,
                ..valid
            },
            SccpInboundMessageRecordV1 {
                source_finality_height: 0,
                ..valid
            },
            SccpInboundMessageRecordV1 {
                source_finality_hash: [0; 32],
                ..valid
            },
            SccpInboundMessageRecordV1 {
                source_proof_commitment: [0; 32],
                ..valid
            },
            SccpInboundMessageRecordV1 {
                admitted_at_height: 0,
                ..valid
            },
        ] {
            assert!(
                !hostile.is_well_formed_for_lane(inbound_lane(SccpNetworkV1::EthereumMainnet)),
                "{hostile:?}"
            );
        }
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
