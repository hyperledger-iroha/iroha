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
/// Durable commit-roster journal for block-sync recovery.
pub mod commit_roster_journal;
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
/// Merge-ledger reduction helpers.
pub mod merge;
/// Minimal Merkle Mountain Range for bridge commitments.
pub mod mmr;
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
pub mod tx;
/// Zero-knowledge verification helpers (backend dispatch + envelope validation).
pub mod zk;
/// Native STARK/FRI verifier under `zk-stark` (`stark/fri/*`).
#[cfg(feature = "zk-stark")]
pub mod zk_stark;

pub use block::InvalidGenesisError;

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
    peers_gossiper::{PeerTrustGossip, PeersGossip},
    sumeragi::message::{BlockMessage, BlockMessageWire, ControlFlow},
};

/// The interval at which sumeragi checks if there are tx in the `queue`.
pub const TX_RETRIEVAL_INTERVAL: Duration = Duration::from_millis(100);

/// Specialized type of Iroha Network
pub type IrohaNetwork = iroha_p2p::NetworkHandle<NetworkMessage>;

/// Ids of peers.
pub type Peers = UniqueVec<PeerId>;

/// Type of `Sender<EventBox>` which should be used for channels of `Event` messages.
pub type EventsSender = broadcast::Sender<EventBox>;

/// Network message envelope exchanged between peers.
#[derive(Clone, Debug, Decode, Encode)]
pub enum NetworkMessage {
    /// Blockchain consensus data message.
    SumeragiBlock(Box<BlockMessageWire>),
    /// Consensus control-flow frames: `NEW_VIEW`, evidence, and view-change proofs.
    SumeragiControlFlow(Box<ControlFlow>),
    /// Lane settlement relay envelope (NX-4).
    LaneRelay(Box<LaneRelayEnvelope>),
    /// Merge committee signature share for merge-ledger quorum certificates.
    MergeCommitteeSignature(Box<MergeCommitteeSignature>),
    /// Block sync message.
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
    /// Iroha Connect (WalletConnect-style) frame relay.
    Connect(Box<connect_proto::ConnectFrame>),
    /// Soracloud local-read proxy request routed to the authoritative primary host.
    SoracloudLocalReadProxyRequest(Box<soracloud_runtime::SoracloudLocalReadProxyRequestV1>),
    /// Soracloud local-read proxy response returned to the ingress node.
    SoracloudLocalReadProxyResponse(Box<soracloud_runtime::SoracloudLocalReadProxyResponseV1>),
    /// Norito Streaming control-plane frame.
    StreamingControl(Box<ControlFrame>),
    /// Gossip for `SoraNet` `PoW`/puzzle runtime configuration (Norito-encoded bytes).
    SoranetPowConfig(Vec<u8>),
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
    fn topic(&self) -> iroha_p2p::network::message::Topic {
        use iroha_p2p::network::message::Topic as T;
        match self {
            NetworkMessage::SumeragiBlock(msg) => match msg.as_ref().as_ref() {
                BlockMessage::BlockCreated(_)
                | BlockMessage::FetchPendingBlock(_)
                | BlockMessage::RbcInit(_)
                | BlockMessage::RbcReady(_)
                | BlockMessage::RbcDeliver(_)
                | BlockMessage::ConsensusParams(_)
                | BlockMessage::ExecWitness(_)
                | BlockMessage::ProposalHint(_)
                | BlockMessage::Qc(_)
                | BlockMessage::QcVote(_)
                | BlockMessage::VrfCommit(_)
                | BlockMessage::VrfReveal(_) => T::Consensus,
                BlockMessage::BlockSyncUpdate(_) | BlockMessage::Proposal(_) => T::ConsensusPayload,
                BlockMessage::RbcChunk(_) | BlockMessage::RbcChunkCompact(_) => T::ConsensusChunk,
            },
            NetworkMessage::SumeragiControlFlow(_)
            | NetworkMessage::LaneRelay(_)
            | NetworkMessage::MergeCommitteeSignature(_)
            | NetworkMessage::SoracloudLocalReadProxyRequest(_)
            | NetworkMessage::SoracloudLocalReadProxyResponse(_)
            | NetworkMessage::StreamingControl(_)
            | NetworkMessage::GenesisRequest(_)
            | NetworkMessage::GenesisResponse(_) => T::Control,
            NetworkMessage::BlockSync(_) => T::BlockSync,
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
    use std::{cmp::Ordering, collections::BTreeSet, num::NonZeroU64, sync::Arc, time::Duration};

    use iroha_crypto::{Hash, KeyPair, SignatureOf};
    use iroha_data_model::block::{BlockHeader, BlockSignature, builder::BlockBuilder};
    use iroha_data_model::nexus::{DataSpaceId, LaneId};
    use iroha_data_model::peer::PeerId;
    use iroha_data_model::role::RoleId;
    use iroha_data_model::transaction::TransactionBuilder;
    use iroha_data_model::{ChainId, Level, isi::Log};
    use iroha_p2p::{ClassifyTopic, network::message::Topic as NetworkTopic};
    use iroha_test_samples::gen_account_in;
    use norito::codec::{Decode, Encode};
    use norito::json;

    use crate::{
        NetworkMessage, PeerTrustGossip, SoranetPowConfigBroadcast, SoranetPuzzleConfigBroadcast,
        gossiper::{GossipPlane, GossipRoute, GossipTransaction, TransactionGossip},
        role::RoleIdWithOwner,
        sumeragi::{
            consensus::{RbcChunk, RbcDeliver, RbcInit, RbcReady},
            message::{
                BlockCreated, BlockMessage, BlockMessageWire, BlockSyncUpdate,
                ConsensusParamsAdvert, FetchPendingBlock,
            },
        },
    };

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
    fn sumeragi_block_classifies_topics() {
        let params = ConsensusParamsAdvert {
            collectors_k: 1,
            redundant_send_r: 1,
            membership: None,
        };
        let msg = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::ConsensusParams(params),
        )));
        assert_eq!(msg.topic(), NetworkTopic::Consensus);

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
            }),
        )));
        assert_eq!(created.topic(), NetworkTopic::Consensus);

        let fetch = FetchPendingBlock {
            requester: PeerId::from(KeyPair::random().public_key().clone()),
            block_hash,
            height: 1,
            view: 0,
            priority: None,
            requester_roster_proof_known: None,
        };
        let fetch_msg = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::FetchPendingBlock(fetch),
        )));
        assert_eq!(fetch_msg.topic(), NetworkTopic::Consensus);

        let roster_hash = Hash::prehashed([1; 32]);
        let chunk_root = Hash::prehashed([2; 32]);
        let payload_hash = Hash::prehashed([3; 32]);
        let leader_keypair = KeyPair::random();
        let leader_signature = BlockSignature::new(
            0,
            SignatureOf::from_hash(leader_keypair.private_key(), block_hash),
        );
        let init = RbcInit {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            roster: Vec::new(),
            roster_hash,
            total_chunks: 0,
            chunk_digests: Vec::new(),
            payload_hash,
            chunk_root,
            block_header: header.clone(),
            leader_signature,
        };
        let init_msg = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::RbcInit(init),
        )));
        assert_eq!(init_msg.topic(), NetworkTopic::Consensus);

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
        assert_eq!(payload.topic(), NetworkTopic::ConsensusChunk);

        let sync = BlockSyncUpdate::from(&block);
        let sync_msg = NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(
            BlockMessage::BlockSyncUpdate(sync),
        )));
        assert_eq!(sync_msg.topic(), NetworkTopic::ConsensusPayload);

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
        assert_eq!(ready_msg.topic(), NetworkTopic::Consensus);

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
        assert_eq!(deliver_msg.topic(), NetworkTopic::Consensus);
    }

    #[test]
    fn network_message_roundtrip_cached_block_message() {
        let params = ConsensusParamsAdvert {
            collectors_k: 1,
            redundant_send_r: 1,
            membership: None,
        };
        let msg = BlockMessage::ConsensusParams(params);
        let encoded = BlockMessageWire::encode_message(&msg);
        let wire = BlockMessageWire::with_encoded(Arc::new(msg), Arc::new(encoded));
        let network = NetworkMessage::SumeragiBlock(Box::new(wire));

        let bytes = network.encode();
        let decoded: NetworkMessage =
            Decode::decode(&mut bytes.as_slice()).expect("decode network");

        match decoded {
            NetworkMessage::SumeragiBlock(wire) => match wire.as_ref().as_ref() {
                BlockMessage::ConsensusParams(advert) => {
                    assert_eq!(advert.collectors_k, 1);
                    assert_eq!(advert.redundant_send_r, 1);
                    assert!(advert.membership.is_none());
                }
                other => panic!("expected consensus params, got {other:?}"),
            },
            other => panic!("expected sumeragi block message, got {other:?}"),
        }
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
        let payload = Arc::new(signed.encode());
        let gossip = TransactionGossip {
            txs: vec![GossipTransaction::with_encoded(
                signed.clone(),
                Arc::clone(&payload),
            )],
            routes: vec![GossipRoute {
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::GLOBAL,
            }],
            plane: GossipPlane::Public,
        };
        let msg = NetworkMessage::TransactionGossiper(Arc::new(gossip));

        let bytes = msg.encode();
        let decoded: NetworkMessage =
            Decode::decode(&mut bytes.as_slice()).expect("decode gossip network");

        match decoded {
            NetworkMessage::TransactionGossiper(gossip) => {
                assert_eq!(gossip.txs.len(), 1);
                assert_eq!(gossip.txs[0].as_signed().hash(), signed.hash());
                assert_eq!(gossip.routes.len(), 1);
                assert_eq!(gossip.routes[0].lane_id, LaneId::SINGLE);
                assert_eq!(gossip.routes[0].dataspace_id, DataSpaceId::GLOBAL);
            }
            other => panic!("expected transaction gossip, got {other:?}"),
        }
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
