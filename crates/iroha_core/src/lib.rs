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

#[cfg(all(feature = "kaigi_privacy_mocks", not(any(debug_assertions, test))))]
compile_error!(
    "`kaigi_privacy_mocks` is a test-only feature; production builds must run real Kaigi roster verification"
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
pub mod beacon;
/// Block types and helpers.
pub mod block;
/// Block synchronization protocol and messages.
pub mod block_sync;
/// Bridge finality proof helpers.
pub mod bridge;
/// Durable archival commit-roster journal used by internal recovery audits.
///
/// Finality authority is the Kura-owned, cryptographically verified v2
/// artifact; journal records are intentionally not part of the public API.
pub(crate) mod commit_roster_journal;
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
/// Genesis helpers (bootstrap protocol).
pub mod genesis;
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
/// Minimal Merkle Mountain Range for bridge commitments.
pub mod mmr;
/// Native AMX participant attestation control plane.
pub mod native_amx;
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
/// Query API types and execution.
pub mod query;
/// Transaction queue and mempool logic.
pub mod queue;
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
    expected_chain_id: &iroha_data_model::ChainId,
) -> Result<(), block::InvalidGenesisError> {
    block::check_genesis_block(block, genesis_account, expected_chain_id)
}

#[cfg(test)]
/// Test-only helpers shared across core modules.
pub mod test_alias {
    /// Historical helper retained for callers; account alias resolvers are no longer installed.
    pub fn ensure() {
        // No-op by design.
    }
}

use core::time::Duration;
use std::sync::Arc;

use gossiper::TransactionGossip;
use iroha_data_model::{events::EventBox, prelude::*};
use iroha_primitives::unique_vec::UniqueVec;
use norito::{
    codec::{Decode, Encode},
    streaming::ControlFrame,
};

/// Re-export of Norito JSON derive macros for core crate internals.
pub mod json_macros {
    pub use norito::derive::{JsonDeserialize, JsonSerialize};
}
use iroha_data_model::{merge::MergeCommitteeSignature, nexus::LaneRelayEnvelope};
use iroha_torii_shared::connect as connect_proto;
use tokio::sync::broadcast;

use crate::{
    block_sync::message::Message as BlockSyncMessage,
    merge_sidecar::{CertifiedMergeSidecarMessage, MergeCandidateMessage},
    peers_gossiper::{PeerTrustGossip, PeersGossip},
    sumeragi::message::{BlockMessage, BlockMessageWire, ControlFlow},
};

/// The interval at which sumeragi checks if there are tx in the `queue`.
pub const TX_RETRIEVAL_INTERVAL: Duration = Duration::from_millis(100);
/// Maximum encoded P2P frame size accepted for one lane-drain vote.
///
/// The cap covers the largest valid embedded lane committee and is enforced by
/// `irohad` before the vote reaches the Sumeragi actor queue.
pub const MAX_LANE_DRAIN_VOTE_WIRE_BYTES: usize = lane_consensus::MAX_LANE_DRAIN_VOTE_BYTES;
const NETWORK_MESSAGE_LANE_DRAIN_VOTE_TAG: u32 = 4;
const MAX_LANE_DRAIN_VOTE_DECODE_ELEMENTS: usize = MAX_LANE_DRAIN_VOTE_WIRE_BYTES;
// A canonical 128-member BLS committee needs just over 256 KiB under Norito's
// conservative nested alignment-copy accounting. Keep deterministic headroom
// while the 16 KiB frame and exact 128-element sequence caps remain primary.
const MAX_LANE_DRAIN_VOTE_DECODE_ALLOCATED_BYTES: usize = 512 * 1024;
const MAX_LANE_DRAIN_VOTE_DECODE_DEPTH: usize = 64;

/// Specialized type of Iroha Network
pub type IrohaNetwork = iroha_p2p::NetworkHandle<NetworkMessage>;

/// Ids of peers.
pub type Peers = UniqueVec<PeerId>;

/// Type of `Sender<EventBox>` which should be used for channels of `Event` messages.
pub type EventsSender = broadcast::Sender<EventBox>;

/// Network message envelope exchanged between peers.
#[derive(Clone, Debug, Decode, Encode)]
pub enum NetworkMessage {
    /// Live Sumeragi v2 or lane-local consensus data message.
    ///
    /// The nested enum retains global v1 variants for archive decoding, but
    /// [`BlockMessageWire`] rejects those variants during serialization.
    SumeragiBlock(Box<BlockMessageWire>),
    /// Archived v1 consensus control-flow frame; live serialization and ingress reject it.
    SumeragiControlFlow(Box<ControlFlow>),
    /// Lane settlement relay envelope (NX-4).
    LaneRelay(Box<LaneRelayEnvelope>),
    /// Merge committee signature share for merge-ledger quorum certificates.
    MergeCommitteeSignature(Box<MergeCommitteeSignature>),
    /// Lane-committee signature share for an automatic drain certificate.
    LaneDrainVote(Box<crate::lane_consensus::LaneDrainVoteV1>),
    /// Authenticated request/chunk traffic for a block-referenced certified merge sidecar.
    CertifiedMergeSidecar(Box<CertifiedMergeSidecarMessage>),
    /// Round-leader announcement/request/chunk traffic for pre-QC merge candidates.
    MergeCandidate(Box<MergeCandidateMessage>),
    /// Native AMX participant attestation control-plane message.
    NativeAmx(Box<native_amx::NativeAmxMessage>),
    /// Archived v1 block-sync frame; live serialization rejects it and v2 uses certified bodies.
    BlockSync(Box<BlockSyncMessage>),
    /// Transaction gossiper message.
    TransactionGossiper(Arc<TransactionGossip>),
    /// Genesis bootstrap request (preflight or payload).
    GenesisRequest(Box<genesis::GenesisRequest>),
    /// Genesis bootstrap response.
    GenesisResponse(Box<genesis::GenesisResponse>),
    /// Peer address gossip message.
    PeersGossiper(Box<PeersGossip>),
    /// Peer trust gossip message.
    PeerTrustGossip(Box<PeerTrustGossip>),
    /// Health check message.
    Health,
    /// Network Time Service: time synchronization ping.
    TimePing(Box<crate::time::TimePing>),
    /// Network Time Service: time synchronization pong.
    TimePong(Box<crate::time::TimePong>),
    /// Iroha Connect (WalletConnect-style) authenticated P2P control message.
    Connect(Box<connect_proto::ConnectP2pMessage>),
    /// Soracloud local-read proxy request routed to the authoritative primary host.
    SoracloudLocalReadProxyRequest(Box<soracloud_runtime::SoracloudLocalReadProxyRequestV1>),
    /// Soracloud local-read proxy response returned to the ingress node.
    SoracloudLocalReadProxyResponse(Box<soracloud_runtime::SoracloudLocalReadProxyResponseV1>),
    /// Torii proxy request routed across bounded Torii ingress proxy hops.
    ToriiProxyRequest(Box<torii_proxy::ToriiProxyRequestV2>),
    /// Torii proxy response returned to the ingress node.
    ToriiProxyResponse(Box<torii_proxy::ToriiProxyResponseV1>),
    /// Norito Streaming control-plane frame.
    StreamingControl(Box<ControlFrame>),
    /// Gossip for `SoraNet` `PoW`/puzzle runtime configuration (Norito-encoded bytes).
    SoranetPowConfig(Vec<u8>),
}

impl NetworkMessage {
    /// Returns `true` when the message is handled by Torii's proxy-plane P2P
    /// subscribers instead of the generic `irohad` relay path.
    #[must_use]
    pub const fn is_torii_proxy_control_message(&self) -> bool {
        matches!(
            self,
            Self::SoracloudLocalReadProxyRequest(_)
                | Self::SoracloudLocalReadProxyResponse(_)
                | Self::ToriiProxyRequest(_)
                | Self::ToriiProxyResponse(_)
        )
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for NetworkMessage {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        use std::borrow::Cow;

        let min_size = core::mem::size_of::<norito::core::Archived<Self>>();
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
                            ConsensusMessageV2Payload::PayloadManifest(_)
                            | ConsensusMessageV2Payload::CertifiedBodyResponse(_) => {
                                T::ConsensusPayload
                            }
                            ConsensusMessageV2Payload::Proposal(_)
                            | ConsensusMessageV2Payload::Vote(_)
                            | ConsensusMessageV2Payload::QuorumCertificate(_)
                            | ConsensusMessageV2Payload::TimeoutVote(_)
                            | ConsensusMessageV2Payload::TimeoutCertificate(_)
                            | ConsensusMessageV2Payload::CommitCertificateResponse(_) => {
                                T::ConsensusSafety
                            }
                            ConsensusMessageV2Payload::CertifiedBodyRequest(_)
                            | ConsensusMessageV2Payload::CommitCertificateRequest(_) => {
                                T::Consensus
                            }
                        }
                    }
                }
                BlockMessage::LaneBlockProposal(_)
                | BlockMessage::LaneBlockNewViewVote(_)
                | BlockMessage::LaneBlockNewViewCertificate(_)
                | BlockMessage::LaneBlockVote(_)
                | BlockMessage::LaneBlockQc(_) => T::Consensus,
                BlockMessage::VrfCommit(_) | BlockMessage::VrfReveal(_) => T::ConsensusSafety,
                BlockMessage::LaneExecutablePayload(_)
                | BlockMessage::LaneExecutablePayloadHandoff(_) => T::ConsensusPayload,
                // Every remaining `BlockMessage` variant belongs to the retired
                // global v1 protocol.  Keep those variants decodable for archive
                // tooling, but never schedule them on correctness-critical live
                // consensus queues.
                _ => T::Other,
            },
            NetworkMessage::CertifiedMergeSidecar(message) => match message.as_ref() {
                CertifiedMergeSidecarMessage::Request(_) => T::Consensus,
                CertifiedMergeSidecarMessage::Chunk(_) => T::ConsensusChunk,
            },
            NetworkMessage::MergeCandidate(message) => match message.as_ref() {
                MergeCandidateMessage::Advert(_) | MergeCandidateMessage::Request(_) => {
                    T::Consensus
                }
                MergeCandidateMessage::Chunk(_) => T::ConsensusChunk,
            },
            NetworkMessage::LaneRelay(_)
            | NetworkMessage::MergeCommitteeSignature(_)
            | NetworkMessage::LaneDrainVote(_)
            | NetworkMessage::NativeAmx(_) => T::Consensus,
            NetworkMessage::SoracloudLocalReadProxyRequest(_)
            | NetworkMessage::SoracloudLocalReadProxyResponse(_)
            | NetworkMessage::ToriiProxyRequest(_)
            | NetworkMessage::ToriiProxyResponse(_)
            | NetworkMessage::StreamingControl(_)
            | NetworkMessage::GenesisRequest(_)
            | NetworkMessage::GenesisResponse(_) => T::Control,
            // The global v1 control-flow and block-sync envelopes are likewise
            // decode-only. Send admission, serialization, and daemon ingress
            // all reject them.
            NetworkMessage::SumeragiControlFlow(_) | NetworkMessage::BlockSync(_) => T::Other,
            NetworkMessage::TransactionGossiper(gossip) => match gossip.plane {
                gossiper::GossipPlane::Public => T::TxGossip,
                gossiper::GossipPlane::Restricted => T::TxGossipRestricted,
            },
            NetworkMessage::PeersGossiper(_) => T::PeerGossip,
            NetworkMessage::SoranetPowConfig(_) => T::Control,
            NetworkMessage::PeerTrustGossip(_) => T::TrustGossip,
            NetworkMessage::Health
            | NetworkMessage::TimePing(_)
            | NetworkMessage::TimePong(_)
            | NetworkMessage::Connect(_) => T::Health,
        }
    }

    fn inbound_decode_limits(
        payload: &[u8],
        framed_len: usize,
        _flags: u8,
    ) -> Result<Option<norito::DecodeLimits>, norito::core::Error> {
        let discriminant = payload
            .get(..core::mem::size_of::<u32>())
            .ok_or(norito::core::Error::LengthMismatch)?;
        let mut discriminant_bytes = [0_u8; core::mem::size_of::<u32>()];
        discriminant_bytes.copy_from_slice(discriminant);
        if u32::from_le_bytes(discriminant_bytes) != NETWORK_MESSAGE_LANE_DRAIN_VOTE_TAG {
            return Ok(None);
        }

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

    fn is_outbound_allowed(&self) -> bool {
        match self {
            Self::SumeragiBlock(message) => {
                message.as_ref().as_message().ensure_live_outbound().is_ok()
            }
            Self::SumeragiControlFlow(_) | Self::BlockSync(_) => false,
            _ => true,
        }
    }
}

/// Compact wire representation of the PoW/puzzle runtime settings.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct SoranetPowConfigBroadcast {
    /// Monotonic version for ordering PoW policy updates.
    pub version: u64,
    /// Whether `PoW` is required for inbound circuits.
    pub required: bool,
    /// Leading zero bits required.
    pub difficulty: u8,
    /// Maximum allowed ticket future skew (seconds).
    pub max_future_skew_secs: u64,
    /// Minimum ticket TTL (seconds).
    pub min_ticket_ttl_secs: u64,
    /// Target ticket TTL (seconds).
    pub ticket_ttl_secs: u64,
    /// Optional Argon2 puzzle parameters.
    pub puzzle: Option<SoranetPuzzleConfigBroadcast>,
}

/// Compact wire representation of the Argon2 puzzle gate.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct SoranetPuzzleConfigBroadcast {
    /// Memory cost expressed in kibibytes.
    pub memory_kib: u32,
    /// Time cost (iterations).
    pub time_cost: u32,
    /// Argon2 lanes.
    pub lanes: u32,
}

pub mod role {
    //! Module with extension for [`RoleId`] to be stored inside state.

    use core::{fmt, str::FromStr};

    use derive_more::Constructor;
    use iroha_primitives::impl_as_dyn_key;
    use mv::json::JsonKeyCodec;
    use norito::json;

    use super::*;

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
            let account = AccountId::parse_encoded(account_raw)
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .map_err(|_| {
                    iroha_data_model::ParseError::new(
                        "Invalid account component in RoleIdWithOwner",
                    )
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
    pub use iroha_crypto::{Algorithm, Hash, KeyPair, PrivateKey, PublicKey};

    #[doc(inline)]
    pub use crate::{
        oracle::{ObservationAdmission, OracleAggregator, aggregate, validate_connector_request},
        smartcontracts::ValidSingularQuery,
        state::{StateReadOnly, StateView, World, WorldReadOnly},
        tx::AcceptedTransaction,
    };
}

#[cfg(test)]
mod tests {
    use std::{
        cmp::Ordering,
        collections::{BTreeMap, BTreeSet},
        num::NonZeroU64,
        sync::Arc,
        time::Duration,
    };

    use iroha_crypto::{Hash, KeyPair, SignatureOf};
    use iroha_data_model::block::{BlockHeader, BlockSignature, builder::BlockBuilder};
    use iroha_data_model::nexus::{DataSpaceId, LaneId};
    use iroha_data_model::peer::PeerId;
    use iroha_data_model::role::RoleId;
    use iroha_data_model::transaction::TransactionBuilder;
    use iroha_data_model::{ChainId, Level, isi::Log};
    use iroha_p2p::{ClassifyTopic, network::message::Topic as NetworkTopic};
    use iroha_test_samples::gen_account_in;
    use norito::json;
    use norito::{codec::Encode, core as ncore};

    use crate::{
        MAX_LANE_DRAIN_VOTE_WIRE_BYTES, NetworkMessage, PeerTrustGossip, SoranetPowConfigBroadcast,
        SoranetPuzzleConfigBroadcast,
        gossiper::{GossipPlane, GossipRoute, GossipTransaction, TransactionGossip},
        queue::{RoutingDecision, RoutingPlan},
        role::RoleIdWithOwner,
        soracloud_runtime::{
            SORACLOUD_LOCAL_READ_PROXY_REQUEST_VERSION_V1,
            SORACLOUD_LOCAL_READ_PROXY_RESPONSE_VERSION_V1, SoracloudLocalReadProxyOutcomeV1,
            SoracloudLocalReadProxyRequestV1, SoracloudLocalReadProxyResponseV1,
            SoracloudLocalReadRequest,
        },
        sumeragi::{
            consensus::{
                ConsensusBlockHeader, Phase, Proposal, QcHeaderRef, RbcChunk, RbcDeliver, RbcInit,
                RbcReady,
            },
            message::{
                BlockCreated, BlockMessage, BlockMessageWire, BlockSyncUpdate, FetchPendingBlock,
            },
        },
        torii_proxy::{
            TORII_PROXY_REQUEST_VERSION_V2, TORII_PROXY_RESPONSE_VERSION_V1,
            ToriiProxyHttpResponseV1, ToriiProxyRequestKindV1, ToriiProxyRequestV2,
            ToriiProxyResponseFormatV1, ToriiProxyResponseV1, ToriiReadEndpointV1,
            ToriiReadProxyRequestV1, ToriiRouteHintV1,
        },
    };

    fn checked_topic_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked network topic keypair")
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
        let gossip = PeerTrustGossip { trust: Vec::new() };
        let msg = NetworkMessage::PeerTrustGossip(Box::new(gossip));

        assert!(matches!(
            msg.topic(),
            iroha_p2p::network::message::Topic::TrustGossip
        ));
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
    fn soranet_pow_broadcast_roundtrip_and_topic() {
        let broadcast = SoranetPowConfigBroadcast {
            version: 1,
            required: true,
            difficulty: 6,
            max_future_skew_secs: 900,
            min_ticket_ttl_secs: 120,
            ticket_ttl_secs: 300,
            puzzle: Some(SoranetPuzzleConfigBroadcast {
                memory_kib: 64 * 1024,
                time_cost: 3,
                lanes: 2,
            }),
        };
        let expected_puzzle = broadcast
            .puzzle
            .expect("baseline broadcast includes puzzle");

        let json = json::to_json(&broadcast).expect("serialize broadcast");
        let decoded: SoranetPowConfigBroadcast =
            json::from_slice(json.as_bytes()).expect("decode broadcast");

        assert_eq!(decoded.version, broadcast.version);
        assert_eq!(decoded.required, broadcast.required);
        assert_eq!(decoded.difficulty, broadcast.difficulty);
        assert_eq!(decoded.ticket_ttl_secs, broadcast.ticket_ttl_secs);
        assert_eq!(decoded.puzzle.expect("puzzle decoded"), expected_puzzle);

        let topic = NetworkMessage::SoranetPowConfig(json.into_bytes()).topic();
        assert_eq!(topic, NetworkTopic::Control);
    }

    #[test]
    fn network_message_decode_from_slice_roundtrip() {
        let message = NetworkMessage::Health;
        let bytes = norito::to_bytes(&message).expect("encode network message");
        let view = norito::core::from_bytes_view(&bytes).expect("archive view");
        let decoded: NetworkMessage = view.decode().expect("decode network message");

        assert!(matches!(decoded, NetworkMessage::Health));
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
                chain_id_digest: Hash::new(b"lane-drain-network-chain"),
                lane_id: LaneId::new(3),
                dataspace_id: DataSpaceId::new(7),
                lane_incarnation: Hash::new(b"lane-drain-network-incarnation"),
                close_global_height: 12,
                initial_merged_lane_height: 4,
                initial_merged_descriptor_hash: Some(Hash::new(b"lane-drain-network-initial")),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set,
                validator_count: 1,
                min_quorum: 1,
            },
            final_lane_block_height: 5,
            final_lane_block_descriptor_hash: Some(Hash::new(b"lane-drain-network-final")),
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
                chain_id_digest: Hash::new(b"maximum-lane-drain-network-chain"),
                lane_id: LaneId::new(3),
                dataspace_id: DataSpaceId::new(7),
                lane_incarnation: Hash::new(b"maximum-lane-drain-network-incarnation"),
                close_global_height: 12,
                initial_merged_lane_height: 4,
                initial_merged_descriptor_hash: Some(Hash::new(
                    b"maximum-lane-drain-network-initial",
                )),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set,
                validator_count,
                min_quorum,
            },
            final_lane_block_height: 5,
            final_lane_block_descriptor_hash: Some(Hash::new(b"maximum-lane-drain-network-final")),
        };
        let vote = crate::lane_consensus::LaneDrainVoteV1::new_signed(
            body,
            signer,
            keypairs[0].private_key(),
        )
        .expect("sign maximum-committee drain vote");
        let message = NetworkMessage::LaneDrainVote(Box::new(vote));
        let encoded = norito::to_bytes(&message).expect("encode maximum-committee lane-drain vote");
        let p2p_wire_len = iroha_p2p::network::data_frame_wire_len(
            &origin,
            None,
            1,
            iroha_p2p::Priority::High,
            &message,
        );

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
                    chain_id_digest: Hash::new(b"excess-lane-drain-network-chain"),
                    lane_id: LaneId::new(3),
                    dataspace_id: DataSpaceId::new(7),
                    lane_incarnation: Hash::new(b"excess-lane-drain-network-incarnation"),
                    close_global_height: 12,
                    initial_merged_lane_height: 4,
                    initial_merged_descriptor_hash: Some(Hash::new(
                        b"excess-lane-drain-network-initial",
                    )),
                    validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash: HashOf::new(&validator_set),
                    validator_set,
                    validator_count,
                    min_quorum: 1,
                },
                final_lane_block_height: 5,
                final_lane_block_descriptor_hash: Some(Hash::new(
                    b"excess-lane-drain-network-final",
                )),
            },
            signer,
            proof_of_possession: vec![0; crate::lane_consensus::LANE_BLS_PROOF_BYTES],
            bls_signature: vec![0; crate::lane_consensus::LANE_BLS_PROOF_BYTES],
        };
        let message = NetworkMessage::LaneDrainVote(Box::new(vote));
        let encoded = norito::to_bytes(&message).expect("encode adversarial lane-drain vote");
        let p2p_wire_len = iroha_p2p::network::data_frame_wire_len(
            &origin,
            None,
            1,
            iroha_p2p::Priority::High,
            &message,
        );
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
                    chain_id_digest: Hash::new(b"oversized-lane-drain-network-chain"),
                    lane_id: LaneId::new(3),
                    dataspace_id: DataSpaceId::new(7),
                    lane_incarnation: Hash::new(b"oversized-lane-drain-network-incarnation"),
                    close_global_height: 12,
                    initial_merged_lane_height: 4,
                    initial_merged_descriptor_hash: Some(Hash::new(
                        b"oversized-lane-drain-network-initial",
                    )),
                    validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash: HashOf::new(&validator_set),
                    validator_set,
                    validator_count: 1,
                    min_quorum: 1,
                },
                final_lane_block_height: 5,
                final_lane_block_descriptor_hash: Some(Hash::new(
                    b"oversized-lane-drain-network-final",
                )),
            },
            signer,
            proof_of_possession: vec![0; MAX_LANE_DRAIN_VOTE_WIRE_BYTES],
            bls_signature: vec![0; crate::lane_consensus::LANE_BLS_PROOF_BYTES],
        };
        let message = NetworkMessage::LaneDrainVote(Box::new(vote));
        let encoded = norito::to_bytes(&message).expect("encode oversized lane-drain vote");
        let p2p_wire_len = iroha_p2p::network::data_frame_wire_len(
            &origin,
            None,
            1,
            iroha_p2p::Priority::High,
            &message,
        );
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
        use iroha_data_model::merge::MergeLedgerEntry;

        use crate::merge_sidecar::{
            CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarChunkV1,
            CertifiedMergeSidecarMessage, CertifiedMergeSidecarRequestV1, MergeCandidateAdvertV1,
            MergeCandidateChunkV1, MergeCandidateMessage, MergeCandidateRequestV1,
        };

        let requester = PeerId::new(checked_topic_keypair().public_key().clone());
        let responder = PeerId::new(checked_topic_keypair().public_key().clone());
        let entry_hash = iroha_crypto::HashOf::<MergeLedgerEntry>::from_untyped_unchecked(
            Hash::new(b"merge-sidecar-entry"),
        );
        let request = CertifiedMergeSidecarRequestV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            request_id: Hash::new(b"merge-sidecar-request"),
            entry_hash,
            encoded_len: 3,
            epoch_id: 4,
            reference_digest: Hash::new(b"merge-sidecar-reference"),
            requester: requester.clone(),
            responder: responder.clone(),
        };
        let request_message = NetworkMessage::CertifiedMergeSidecar(Box::new(
            CertifiedMergeSidecarMessage::Request(request.clone()),
        ));
        assert_eq!(request_message.topic(), NetworkTopic::Consensus);
        let encoded = norito::to_bytes(&request_message).expect("encode sidecar request");
        let decoded =
            norito::decode_from_bytes::<NetworkMessage>(&encoded).expect("decode sidecar request");
        assert!(matches!(
            decoded,
            NetworkMessage::CertifiedMergeSidecar(message)
                if *message == CertifiedMergeSidecarMessage::Request(request.clone())
        ));

        let chunk = CertifiedMergeSidecarChunkV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
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
        let chunk_message = NetworkMessage::CertifiedMergeSidecar(Box::new(
            CertifiedMergeSidecarMessage::Chunk(chunk.clone()),
        ));
        assert_eq!(chunk_message.topic(), NetworkTopic::ConsensusChunk);
        let encoded = norito::to_bytes(&chunk_message).expect("encode sidecar chunk");
        let decoded =
            norito::decode_from_bytes::<NetworkMessage>(&encoded).expect("decode sidecar chunk");
        assert!(matches!(
            decoded,
            NetworkMessage::CertifiedMergeSidecar(message)
                if *message == CertifiedMergeSidecarMessage::Chunk(chunk)
        ));

        let advert = MergeCandidateAdvertV1::new(
            4,
            1,
            9,
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"candidate-parent")),
            iroha_crypto::HashOf::new(&vec![request.requester.clone(), request.responder.clone()]),
            Hash::new(b"candidate-digest"),
            Hash::new(b"candidate-body"),
            128,
            request.responder.clone(),
        );
        let advert_message =
            NetworkMessage::MergeCandidate(Box::new(MergeCandidateMessage::Advert(advert.clone())));
        assert_eq!(advert_message.topic(), NetworkTopic::Consensus);
        let encoded = norito::to_bytes(&advert_message).expect("encode candidate advert");
        let decoded =
            norito::decode_from_bytes::<NetworkMessage>(&encoded).expect("decode candidate advert");
        assert!(matches!(
            decoded,
            NetworkMessage::MergeCandidate(message)
                if *message == MergeCandidateMessage::Advert(advert.clone())
        ));

        let candidate_request = MergeCandidateRequestV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            request_id: Hash::new(b"merge-candidate-request"),
            advert: advert.clone(),
            requester: request.requester.clone(),
            responder: request.responder.clone(),
        };
        let request_message = NetworkMessage::MergeCandidate(Box::new(
            MergeCandidateMessage::Request(candidate_request.clone()),
        ));
        assert_eq!(request_message.topic(), NetworkTopic::Consensus);
        let encoded = norito::to_bytes(&request_message).expect("encode candidate request");
        let decoded = norito::decode_from_bytes::<NetworkMessage>(&encoded)
            .expect("decode candidate request");
        assert!(matches!(
            decoded,
            NetworkMessage::MergeCandidate(message)
                if *message == MergeCandidateMessage::Request(candidate_request.clone())
        ));

        let candidate_chunk = MergeCandidateChunkV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            request_id: candidate_request.request_id,
            advert,
            requester: candidate_request.requester,
            responder: candidate_request.responder,
            chunk_index: 0,
            chunk_count: 1,
            bytes: vec![7; 128],
        };
        let chunk_message = NetworkMessage::MergeCandidate(Box::new(MergeCandidateMessage::Chunk(
            candidate_chunk.clone(),
        )));
        assert_eq!(chunk_message.topic(), NetworkTopic::ConsensusChunk);
        let encoded = norito::to_bytes(&chunk_message).expect("encode candidate chunk");
        let decoded =
            norito::decode_from_bytes::<NetworkMessage>(&encoded).expect("decode candidate chunk");
        assert!(matches!(
            decoded,
            NetworkMessage::MergeCandidate(message)
                if *message == MergeCandidateMessage::Chunk(candidate_chunk)
        ));
    }

    #[test]
    fn torii_proxy_control_message_classification_covers_shared_proxy_variants() {
        let soracloud_request = NetworkMessage::SoracloudLocalReadProxyRequest(Box::new(
            SoracloudLocalReadProxyRequestV1 {
                schema_version: SORACLOUD_LOCAL_READ_PROXY_REQUEST_VERSION_V1,
                request_id: Hash::prehashed([0x11; 32]),
                request: SoracloudLocalReadRequest {
                    observed_height: 1,
                    observed_block_hash: None,
                    service_name: "svc".to_owned(),
                    service_version: "1.0.0".to_owned(),
                    handler_name: "read".to_owned(),
                    handler_class: crate::soracloud_runtime::SoracloudLocalReadKind::Query,
                    request_method: "GET".to_owned(),
                    request_path: "/v1/soracloud/test".to_owned(),
                    handler_path: "/test".to_owned(),
                    request_query: None,
                    request_headers: BTreeMap::new(),
                    request_body: Vec::new(),
                    request_commitment: Hash::prehashed([0x12; 32]),
                },
            },
        ));
        let soracloud_response = NetworkMessage::SoracloudLocalReadProxyResponse(Box::new(
            SoracloudLocalReadProxyResponseV1 {
                schema_version: SORACLOUD_LOCAL_READ_PROXY_RESPONSE_VERSION_V1,
                request_id: Hash::prehashed([0x13; 32]),
                outcome: SoracloudLocalReadProxyOutcomeV1::Err(
                    crate::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                        crate::soracloud_runtime::SoracloudRuntimeExecutionErrorKind::Unavailable,
                        "proxy unavailable",
                    ),
                ),
            },
        ));
        let torii_request = NetworkMessage::ToriiProxyRequest(Box::new(ToriiProxyRequestV2 {
            schema_version: TORII_PROXY_REQUEST_VERSION_V2,
            request_id: Hash::prehashed([0x14; 32]),
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

        assert!(soracloud_request.is_torii_proxy_control_message());
        assert!(soracloud_response.is_torii_proxy_control_message());
        assert!(torii_request.is_torii_proxy_control_message());
        assert!(torii_response.is_torii_proxy_control_message());
        assert!(!NetworkMessage::Health.is_torii_proxy_control_message());
    }

    #[test]
    fn authoritative_v2_safety_uses_dedicated_topic() {
        use iroha_data_model::block::consensus_v2 as wire;

        let context_id = wire::HeightContextId(
            iroha_crypto::HashOf::<wire::HeightContext>::from_untyped_unchecked(Hash::new(
                b"v2-safety-topic-context",
            )),
        );
        let vote = wire::Vote {
            round: wire::ConsensusRound {
                context_id,
                height: 7,
                view: 2,
            },
            phase: wire::GlobalPhase::Prepare,
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                    b"v2-safety-topic-block",
                )),
                payload_hash: Hash::new(b"v2-safety-topic-payload"),
            },
            execution_commitment: wire::ExecutionCommitment::without_topups(
                Hash::new(b"v2-safety-topic-parent-state"),
                Hash::new(b"v2-safety-topic-post-state"),
                Hash::new(b"v2-safety-topic-ordinary-writes"),
            ),
            signer: 0,
            signature: vec![1],
        };
        let message =
            NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(BlockMessage::V2(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote)),
            ))));
        assert_eq!(message.topic(), NetworkTopic::ConsensusSafety);

        let vrf = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::VrfCommit(crate::sumeragi::consensus::VrfCommit {
                epoch: 3,
                commitment: [0xA5; 32],
                signer: 0,
                bls_sig: vec![0x5A],
            }),
        )));
        assert_eq!(vrf.topic(), NetworkTopic::ConsensusSafety);
        assert!(vrf.is_outbound_allowed());
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
        let v2_chunk = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::V2(canonical_chunk.clone()),
        )));
        assert_eq!(v2_chunk.topic(), NetworkTopic::ConsensusChunk);
        assert!(v2_chunk.is_outbound_allowed());
        assert!(
            ncore::to_bytes(&v2_chunk).is_ok(),
            "canonical v2 traffic must remain live-encodable"
        );

        let mut wrong_version_chunk = canonical_chunk;
        wrong_version_chunk.protocol_version = 1;
        let wrong_version_chunk = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::V2(wrong_version_chunk),
        )));
        assert_eq!(wrong_version_chunk.topic(), NetworkTopic::Other);
        assert!(!wrong_version_chunk.is_outbound_allowed());
        assert!(
            ncore::to_bytes(&wrong_version_chunk).is_err(),
            "a non-canonical protocol version must fail the wire boundary"
        );

        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let block_hash = header.hash();
        let block = BlockBuilder::new(header.clone()).build(BTreeSet::new());
        let created = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::BlockCreated(BlockCreated {
                block: block.clone(),
                frontier: None,
            }),
        )));
        assert_eq!(created.topic(), NetworkTopic::Other);

        let fetch = FetchPendingBlock {
            requester: PeerId::from(checked_topic_keypair().public_key().clone()),
            block_hash,
            height: 1,
            view: 0,
            priority: None,
            requester_roster_proof_known: None,
            commit_qc_only: None,
        };
        let fetch_msg = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::FetchPendingBlock(fetch),
        )));
        assert_eq!(fetch_msg.topic(), NetworkTopic::Other);

        let roster_hash = Hash::prehashed([1; 32]);
        let chunk_root = Hash::prehashed([2; 32]);
        let payload_hash = Hash::prehashed([3; 32]);
        let leader_keypair = checked_topic_keypair();
        let leader_signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(leader_keypair.private_key(), block_hash)
                .expect("test block signing should succeed"),
        );
        let init = RbcInit {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            roster: Vec::new(),
            roster_hash,
            total_chunks: 0,
            encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
            chunk_size_bytes: 0,
            payload_size_bytes: 0,
            data_shards: 0,
            parity_shards: 0,
            chunk_digests: Vec::new(),
            payload_hash,
            chunk_root,
            block_header: header.clone(),
            leader_signature,
        };
        let init_msg = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::RbcInit(init),
        )));
        assert_eq!(init_msg.topic(), NetworkTopic::Other);

        let chunk = RbcChunk {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            idx: 0,
            bytes: vec![1, 2, 3],
        };
        let payload = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::RbcChunk(chunk),
        )));
        assert_eq!(payload.topic(), NetworkTopic::Other);

        let proposal = Proposal {
            header: ConsensusBlockHeader {
                parent_hash: block_hash,
                tx_root: Hash::new(b"tx"),
                state_root: Hash::new(b"state"),
                proposer: 0,
                height: 1,
                view: 0,
                epoch: 0,
                highest_qc: QcHeaderRef {
                    height: 0,
                    view: 0,
                    epoch: 0,
                    subject_block_hash: block_hash,
                    phase: Phase::Prepare,
                },
            },
            payload_hash,
        };
        let proposal_msg = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::Proposal(proposal),
        )));
        assert_eq!(proposal_msg.topic(), NetworkTopic::Other);

        let sync = BlockSyncUpdate::from(&block);
        let sync_msg = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::BlockSyncUpdate(sync),
        )));
        assert_eq!(sync_msg.topic(), NetworkTopic::Other);

        let ready = RbcReady {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            roster_hash,
            chunk_root,
            sender: 0,
            signature: vec![7],
        };
        let ready_msg = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::RbcReady(ready),
        )));
        assert_eq!(ready_msg.topic(), NetworkTopic::Other);

        let deliver = RbcDeliver {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            roster_hash,
            chunk_root,
            sender: 0,
            signature: vec![9],
            ready_signatures: Vec::new(),
        };
        let deliver_msg = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::RbcDeliver(deliver),
        )));
        assert_eq!(deliver_msg.topic(), NetworkTopic::Other);
    }

    #[test]
    fn network_message_refuses_decoded_archival_block_message() {
        let msg = BlockMessage::VrfCommit(iroha_data_model::block::consensus::VrfCommit {
            epoch: 9,
            commitment: [0x91; 32],
            signer: 1,
            bls_sig: vec![0x92],
        });
        let encoded = ncore::to_bytes(&msg).expect("encode archival block-message fixture");
        assert!(encoded.starts_with(&norito::core::MAGIC));
        let wire = <BlockMessageWire as ncore::DecodeFromSlice>::decode_from_slice(&encoded)
            .expect("decode archival block-message fixture")
            .0;
        let network = NetworkMessage::SumeragiBlock(Box::new(wire));

        match &network {
            NetworkMessage::SumeragiBlock(wire) => {
                assert!(matches!(wire.as_ref().as_ref(), BlockMessage::VrfCommit(_)));
            }
            other => panic!("expected sumeragi block message, got {other:?}"),
        }
        assert!(
            ncore::to_bytes(&network).is_err(),
            "retired global v1 message must fail at the network serialization boundary"
        );
    }

    #[test]
    fn network_message_roundtrip_cached_transaction_gossip() {
        let (account, keypair) = gen_account_in("wonderland");
        let chain_id: ChainId = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("valid chain id");
        let mut builder = TransactionBuilder::new(chain_id, account);
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
        let chain_id: ChainId = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("valid chain id");
        let mut builder = TransactionBuilder::new(chain_id, account);
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
