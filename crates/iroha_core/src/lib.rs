//! Iroha — A simple, enterprise-grade decentralized ledger.
#![allow(unexpected_cfgs)]
// Nested `if` blocks remain intentional for readability/instrumentation; Clippy's
// `collapsible_if` lint would force let-chains that obscure the control flow.
#![allow(clippy::collapsible_if)]
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
/// Zero-knowledge verification helpers (scaffolding)
pub mod zk;
/// Minimal STARK (FRI, Goldilocks, SHA-256) verifier under `zk-stark`.
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
    use std::{
        fmt::Write as _,
        sync::{Arc, LazyLock},
    };

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        account::{AccountId, set_account_alias_resolver},
        domain::DomainId,
    };

    static INSTALL: LazyLock<()> = LazyLock::new(|| {
        set_account_alias_resolver(Arc::new(|label, domain| {
            Some(deterministic_account(label, domain))
        }));
    });

    /// Install the deterministic alias resolver for tests.
    pub fn ensure() {
        LazyLock::force(&INSTALL);
    }

    fn deterministic_account(label: &str, domain: &DomainId) -> AccountId {
        let mut material = String::new();
        let _ = write!(&mut material, "{label}@{domain}");
        let seed_bytes: [u8; Hash::LENGTH] = Hash::new(material).into();
        let keypair = KeyPair::from_seed(seed_bytes.to_vec(), Algorithm::default());
        AccountId::new(domain.clone(), keypair.public_key().clone())
    }
}

use core::time::Duration;

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
    sumeragi::message::{BlockMessage, ControlFlow},
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
    SumeragiBlock(Box<BlockMessage>),
    /// Consensus control-flow frames: `NEW_VIEW`, evidence, and view-change proofs.
    SumeragiControlFlow(Box<ControlFlow>),
    /// Lane settlement relay envelope (NX-4).
    LaneRelay(Box<LaneRelayEnvelope>),
    /// Merge committee signature share for merge-ledger quorum certificates.
    MergeCommitteeSignature(Box<MergeCommitteeSignature>),
    /// Block sync message.
    BlockSync(Box<BlockSyncMessage>),
    /// Transaction gossiper message.
    TransactionGossiper(Box<TransactionGossip>),
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
    Connect(Box<connect_proto::ConnectFrameV1>),
    /// Norito Streaming control-plane frame.
    StreamingControl(Box<ControlFrame>),
    /// Gossip for `SoraNet` `PoW`/puzzle runtime configuration (Norito-encoded bytes).
    SoranetPowConfig(Vec<u8>),
}

// Derive Encode/Decode above for NetworkMessage

// Classify core network messages into P2P topics for scheduling.
impl iroha_p2p::network::message::ClassifyTopic for NetworkMessage {
    fn topic(&self) -> iroha_p2p::network::message::Topic {
        use iroha_p2p::network::message::Topic as T;
        match self {
            NetworkMessage::SumeragiBlock(_) => T::Consensus,
            NetworkMessage::SumeragiControlFlow(_)
            | NetworkMessage::LaneRelay(_)
            | NetworkMessage::MergeCommitteeSignature(_)
            | NetworkMessage::StreamingControl(_)
            | NetworkMessage::GenesisRequest(_)
            | NetworkMessage::GenesisResponse(_) => T::Control,
            NetworkMessage::BlockSync(_) => T::BlockSync,
            NetworkMessage::TransactionGossiper(gossip) => match gossip.plane {
                gossiper::GossipPlane::Public => T::TxGossip,
                gossiper::GossipPlane::Restricted => T::TxGossipRestricted,
            },
            NetworkMessage::PeersGossiper(_) | NetworkMessage::SoranetPowConfig(_) => T::PeerGossip,
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
            let account = account_raw.parse().map_err(|_| {
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
    use std::cmp::Ordering;

    use iroha_data_model::role::RoleId;
    use iroha_p2p::{ClassifyTopic, network::message::Topic as NetworkTopic};
    use iroha_test_samples::gen_account_in;
    use norito::json;

    use crate::{
        NetworkMessage, PeerTrustGossip, SoranetPowConfigBroadcast, SoranetPuzzleConfigBroadcast,
        role::RoleIdWithOwner,
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
        assert_eq!(decoded.account, account);
        assert_eq!(decoded.id, role);
    }

    #[test]
    fn soranet_pow_broadcast_roundtrip_and_topic() {
        let broadcast = SoranetPowConfigBroadcast {
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

        assert_eq!(decoded.required, broadcast.required);
        assert_eq!(decoded.difficulty, broadcast.difficulty);
        assert_eq!(decoded.ticket_ttl_secs, broadcast.ticket_ttl_secs);
        assert_eq!(decoded.puzzle.expect("puzzle decoded"), expected_puzzle);

        let topic = NetworkMessage::SoranetPowConfig(json.into_bytes()).topic();
        assert_eq!(topic, NetworkTopic::PeerGossip);
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
