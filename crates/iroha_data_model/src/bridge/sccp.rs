//! Versioned SCCP network, lane, and source-identity wire types.
//!
//! These types deliberately model only the first-release network inventory.
//! There is no catch-all network, emitter, or arbitrary network identifier:
//! unsupported profiles must fail decoding instead of being interpreted by
//! node-local policy.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::SccpNativeTrustAnchorV1;

/// A supported SCCP network profile for the V1 wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(tag = "network", content = "profile")]
pub enum SccpNetworkV1 {
    /// The production SORA Nexus network.
    #[codec(index = 0)]
    #[norito(rename = "sora_nexus")]
    SoraNexus,
    /// The SORA Taira staging network.
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
            Self::SoraNexus | Self::SoraTaira => 0,
            Self::EthereumMainnet | Self::EthereumSepolia => 1,
            Self::BscMainnet | Self::BscTestnet => 2,
            Self::TronMainnet | Self::TronNile | Self::TronShasta => 5,
        }
    }

    /// Return the canonical, stable textual key for this exact profile.
    #[must_use]
    pub const fn profile_key(self) -> &'static str {
        match self {
            Self::SoraNexus => "sora-nexus",
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
            "sora-nexus" => Some(Self::SoraNexus),
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
            Self::SoraNexus | Self::EthereumMainnet | Self::BscMainnet | Self::TronMainnet
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
        matches!(self, Self::SoraNexus | Self::SoraTaira)
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
/// Ordering by height first supports bounded newest/oldest pagination without
/// scanning sparse block history. Lane and message id make every index entry
/// self-checking against the authoritative replay map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
pub struct SccpOutboundMessageIndexKeyV1 {
    /// Local SORA block height containing the recorded message.
    pub recorded_at_height: u64,
    /// Exact SORA-to-external lane of the replay key.
    pub lane: SccpLaneIdV1,
    /// Globally unique lane-bound SCCP message identifier.
    pub message_id: [u8; 32],
}

impl SccpOutboundMessageIndexKeyV1 {
    /// Build an ordered locator from one authoritative replay key and record.
    #[must_use]
    pub fn new(key: SccpOutboundMessageKeyV1, record: SccpOutboundMessageRecordV1) -> Option<Self> {
        if !record.is_well_formed_for_key(&key) {
            return None;
        }
        Some(Self {
            recorded_at_height: record.recorded_at_height,
            lane: key.lane,
            message_id: key.message_id,
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
        self.recorded_at_height != 0 && self.message_key().is_well_formed()
    }
}

/// Durable admission evidence for a SORA-origin outbound SCCP message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
pub struct SccpOutboundMessageRecordV1 {
    /// Governed destination binding authenticated at record admission.
    pub destination_binding_hash: [u8; 32],
    /// Immutable governed route configuration, including typed settlement.
    pub route_configuration_hash: [u8; 32],
    /// Hash of the exact canonical SCCP payload admitted for the key.
    pub payload_hash: [u8; 32],
    /// Local SORA block height at which the message was recorded.
    pub recorded_at_height: u64,
}

impl SccpOutboundMessageRecordV1 {
    /// Return whether both distinct commitments and the admission height are nonzero.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        nonzero(&self.destination_binding_hash)
            && nonzero(&self.route_configuration_hash)
            && nonzero(&self.payload_hash)
            && self.destination_binding_hash != self.payload_hash
            && self.destination_binding_hash != self.route_configuration_hash
            && self.route_configuration_hash != self.payload_hash
            && self.recorded_at_height != 0
    }

    /// Return whether this record and replay key use three distinct nonzero hash roles.
    #[must_use]
    pub fn is_well_formed_for_key(&self, key: &SccpOutboundMessageKeyV1) -> bool {
        self.is_well_formed()
            && key.is_well_formed()
            && key.message_id != self.destination_binding_hash
            && key.message_id != self.route_configuration_hash
            && key.message_id != self.payload_hash
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
pub struct SccpInboundMessageRecordV1 {
    /// Hash of the exact canonical SCCP payload admitted for the key.
    pub payload_hash: [u8; 32],
    /// Immutable governed route configuration, including typed settlement.
    pub route_configuration_hash: [u8; 32],
    /// Domain-separated hash of the exact typed source identity used at admission.
    pub source_identity_hash: [u8; 32],
    /// Governed native verifier and checkpoint commitment used at admission.
    pub trust_anchor: SccpNativeTrustAnchorV1,
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
    /// Return whether every commitment and both chain heights are nonzero.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        nonzero(&self.payload_hash)
            && nonzero(&self.route_configuration_hash)
            && nonzero(&self.source_identity_hash)
            && self.route_configuration_hash != self.payload_hash
            && self.route_configuration_hash != self.source_identity_hash
            && self.trust_anchor.is_well_formed()
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
    /// This deliberately says nothing about the SORA target. In particular, a
    /// mainnet source targeting Taira has production source material but is not
    /// a production lane.
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

    const NETWORKS: [SccpNetworkV1; 9] = [
        SccpNetworkV1::SoraNexus,
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
            target: SccpNetworkV1::SoraNexus,
        }
    }

    fn outbound_lane(target: SccpNetworkV1) -> SccpLaneIdV1 {
        SccpLaneIdV1 {
            source: SccpNetworkV1::SoraNexus,
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
            source_finality_height: 8,
            source_finality_hash: [13; 32],
            source_proof_commitment: [14; 32],
            admitted_at_height: 9,
        }
    }

    fn outbound_record() -> SccpOutboundMessageRecordV1 {
        SccpOutboundMessageRecordV1 {
            destination_binding_hash: [15; 32],
            route_configuration_hash: [16; 32],
            payload_hash: [17; 32],
            recorded_at_height: 10,
        }
    }

    #[test]
    fn network_inventory_and_profile_keys_are_exact() {
        assert_eq!(NETWORKS.len(), 9);
        for network in NETWORKS {
            assert_eq!(
                SccpNetworkV1::from_profile_key(network.profile_key()),
                Some(network)
            );
        }

        for unsupported in [
            "",
            "ethereum",
            "ETHEREUM-MAINNET",
            "solana-mainnet-beta",
            "solana-testnet",
            "ton-mainnet",
            "ton-testnet",
        ] {
            assert_eq!(SccpNetworkV1::from_profile_key(unsupported), None);
        }

        assert_eq!(SccpNetworkV1::SoraNexus.domain_id(), 0);
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
        for unsupported_tag in [6_u32, 7, 8, 9, 13, u32::MAX] {
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
    fn binary_tags_reserve_six_through_nine_and_match_contract_profiles() {
        let expected = [
            (SccpNetworkV1::SoraNexus, 0_u32),
            (SccpNetworkV1::SoraTaira, 1),
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
            SccpOutboundMessageRecordV1 {
                destination_binding_hash: [0; 32],
                ..record
            },
            SccpOutboundMessageRecordV1 {
                route_configuration_hash: record.destination_binding_hash,
                ..record
            },
            SccpOutboundMessageRecordV1 {
                payload_hash: record.route_configuration_hash,
                ..record
            },
            SccpOutboundMessageRecordV1 {
                recorded_at_height: 0,
                ..record
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
        let index = SccpOutboundMessageIndexKeyV1::new(key, record).expect("valid ordered index");
        assert_eq!(index.recorded_at_height, record.recorded_at_height);
        assert_eq!(index.message_key(), key);
        assert!(index.is_well_formed());

        assert!(
            SccpOutboundMessageIndexKeyV1::new(
                key,
                SccpOutboundMessageRecordV1 {
                    recorded_at_height: 0,
                    ..record
                },
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
}
