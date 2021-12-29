#![allow(
    clippy::restriction,
    missing_debug_implementations,
    clippy::future_not_send,
    clippy::pedantic
)]

use std::{fmt::Debug, ops::Deref, time::Duration};

use eyre::Result;
use iroha_actor::prelude::*;
use iroha_core::{
    block_sync::{BlockSynchronizer, BlockSynchronizerTrait},
    genesis::{config::GenesisConfiguration, GenesisNetworkTrait},
    kura::KuraTrait,
    prelude::*,
    sumeragi::{
        network_topology::Topology, Gossip, IsLeader, RetrieveTransactions, Sumeragi,
        SumeragiTrait, SumeragiWithFault,
    },
    wsv::WorldTrait,
};
use iroha_data_model::prelude::*;
use test_network::*;
use tokio::time;
use utils::{genesis, sumeragi, world};

pub mod utils {
    use iroha_core::genesis::RawGenesisBlock;

    use super::*;

    pub mod genesis {
        use iroha_core::IrohaNetwork;

        use super::*;

        #[derive(Debug, Clone, Copy, Default)]
        pub struct NoGenesis;

        impl Deref for NoGenesis {
            type Target = Vec<VersionedAcceptedTransaction>;
            fn deref(&self) -> &Self::Target {
                unreachable!()
            }
        }

        #[async_trait::async_trait]
        impl GenesisNetworkTrait for NoGenesis {
            fn from_configuration(
                _submit_genesis: bool,
                _block_path: RawGenesisBlock,
                _genesis_config: &GenesisConfiguration,
                _max_instructions_number: u64,
            ) -> Result<Option<Self>> {
                Ok(None)
            }

            async fn wait_for_peers(
                &self,
                _: PeerId,
                _: Topology,
                _: Addr<IrohaNetwork>,
            ) -> Result<Topology> {
                unreachable!()
            }

            fn genesis_submission_delay_ms(&self) -> u64 {
                0
            }
        }
    }

    pub mod sumeragi {
        use std::{fmt::Debug, marker::PhantomData};

        use iroha_core::{
            genesis::GenesisNetworkTrait,
            kura::KuraTrait,
            sumeragi::{
                message::Message as SumeragiMessage, network_topology::Role, FaultInjection,
                SumeragiWithFault,
            },
            wsv::WorldTrait,
        };

        pub trait ConstRoleTrait: Debug + Send + 'static {
            /// Returns true if this peer has this `role`
            fn role(role: Role) -> bool;
        }

        #[derive(Debug, Clone, Copy, Default)]
        struct Not<R>(PhantomData<R>);

        impl<R: ConstRoleTrait> ConstRoleTrait for Not<R> {
            fn role(role: Role) -> bool {
                !R::role(role)
            }
        }

        macro_rules! impl_role {
            ($($name:ident),* $(,)? ) => {$(
                #[derive(Debug, Clone, Copy, Default)]
                pub struct $name;
                impl ConstRoleTrait for $name {
                    fn role(role: Role) -> bool {
                        Role::$name == role
                    }
                }
            )*};
        }

        impl_role!(Leader, ValidatingPeer, ObservingPeer, ProxyTail);

        #[derive(Debug, Clone, Copy, Default)]
        pub struct Any;

        impl ConstRoleTrait for Any {
            fn role(_: Role) -> bool {
                true
            }
        }

        #[derive(Debug, Clone, Copy, Default)]
        pub struct EmptyBlockCreated;

        impl FaultInjection for EmptyBlockCreated {
            fn faulty_message<G, K, W>(
                _: &SumeragiWithFault<G, K, W, Self>,
                msg: SumeragiMessage,
            ) -> Option<SumeragiMessage>
            where
                G: GenesisNetworkTrait,
                K: KuraTrait,
                W: WorldTrait,
            {
                let msg = if let SumeragiMessage::BlockCreated(mut block) = msg {
                    block.block.as_mut_v1().transactions = Vec::new();
                    SumeragiMessage::BlockCreated(block)
                } else {
                    msg
                };
                Some(msg)
            }
        }

        #[derive(Debug, Clone, Copy, Default)]
        pub struct Skip<M, R>(PhantomData<M>, PhantomData<R>);

        macro_rules! impl_skip {
            ( $($name:ident),* $(,)? ) => {$(
                #[derive(Debug, Clone, Copy, Default)]
                pub struct $name;

                impl<R: ConstRoleTrait + Send + Sync> FaultInjection for Skip<$name, R> {
                    fn faulty_message<G, K, W>(
                        sumeragi: &SumeragiWithFault<G, K, W, Self>,
                        msg: SumeragiMessage,
                    ) -> Option<SumeragiMessage>
                    where
                        G: GenesisNetworkTrait,
                        K: KuraTrait,
                        W: WorldTrait,
                    {
                        if let SumeragiMessage::$name(..) = msg {
                            if R::role(sumeragi.role()) {
                                iroha_logger::error!("Fault behaviour: Skipping {}", stringify!($name));
                                None
                            } else {
                                Some(msg)
                            }
                        } else {
                            Some(msg)
                        }
                    }
                }
            )*};
        }

        impl_skip!(
            BlockCreated,
            BlockSigned,
            BlockCommitted,
            TransactionReceived,
            TransactionForwarded,
            ViewChangeSuggested
        );

        #[derive(Debug, Clone, Copy, Default)]
        pub struct SkipTxForwardedAndGossipOnLeader;

        impl FaultInjection for SkipTxForwardedAndGossipOnLeader {
            fn faulty_message<G, K, W>(
                sumeragi: &SumeragiWithFault<G, K, W, Self>,
                msg: SumeragiMessage,
            ) -> Option<SumeragiMessage>
            where
                G: GenesisNetworkTrait,
                K: KuraTrait,
                W: WorldTrait,
            {
                match (sumeragi.role(), msg) {
                    (Role::Leader, SumeragiMessage::TransactionForwarded(_))
                    | (Role::Leader, SumeragiMessage::TransactionGossip(_)) => {
                        iroha_logger::error!(
                            "Fault behaviour: Skipping TransactionForwarded and TransactionGossip"
                        );
                        None
                    }
                    (_, msg) => Some(msg),
                }
            }
        }
    }

    pub mod world {
        use std::ops::{Deref, DerefMut};

        use iroha_core::{prelude::*, tx::Domain, wsv::WorldTrait};
        use iroha_data_model::prelude::*;
        use once_cell::sync::Lazy;

        #[derive(Debug, Clone, Default)]
        pub struct WithAlice(World);

        impl Deref for WithAlice {
            type Target = World;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl DerefMut for WithAlice {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        pub static ALICE_KEYS: Lazy<KeyPair> = Lazy::new(|| KeyPair::generate().unwrap());
        pub static ALICE_ID: Lazy<AccountId> = Lazy::new(|| AccountId::test("alice", "wonderland"));
        pub static ALICE: Lazy<Account> = Lazy::new(|| {
            let mut account = Account::new(ALICE_ID.clone());
            account.signatories.push(ALICE_KEYS.public_key.clone());
            account
        });
        pub static WONDERLAND: Lazy<Domain> = Lazy::new(|| {
            let mut domain = Domain::test("wonderland");
            domain.accounts.insert(ALICE_ID.clone(), ALICE.clone());
            domain
        });

        impl WorldTrait for WithAlice {
            /// Creates `World` with these `domains` and `trusted_peers_ids`
            fn with(
                domains: impl IntoIterator<Item = (DomainId, Domain)>,
                trusted_peers_ids: impl IntoIterator<Item = PeerId>,
            ) -> Self {
                Self(World::with(
                    vec![(WONDERLAND.id.clone(), WONDERLAND.clone())]
                        .into_iter()
                        .chain(domains),
                    trusted_peers_ids,
                ))
            }
        }

        pub fn sign_tx(isi: impl IntoIterator<Item = Instruction>) -> VersionedAcceptedTransaction {
            let instructions: Vec<_> = isi.into_iter().collect();
            let tx = Transaction::new(ALICE_ID.clone(), instructions.into(), 100_000)
                .sign(&ALICE_KEYS)
                .unwrap();
            VersionedAcceptedTransaction::from_transaction(tx, 4096).unwrap()
        }
    }
}

/// Checks if blocks applied on all peers
async fn blocks_applied<W, G, S, K, B>(network: &Network<W, G, K, S, B>, n_blocks: usize)
where
    W: WorldTrait,
    G: GenesisNetworkTrait,
    K: KuraTrait<World = W>,
    S: SumeragiTrait<GenesisNetwork = G, Kura = K, World = W>,
    B: BlockSynchronizerTrait<Sumeragi = S, World = W>,
{
    for peer in network.peers() {
        assert_eq!(peer.iroha.as_ref().unwrap().wsv.height(), n_blocks as u64)
    }
}

async fn put_tx_in_queue_to_peer<W, G, S, K, B>(network: &Network<W, G, K, S, B>, to_leader: bool)
where
    W: WorldTrait,
    G: GenesisNetworkTrait,
    K: KuraTrait<World = W>,
    S: SumeragiTrait<GenesisNetwork = G, Kura = K, World = W>,
    B: BlockSynchronizerTrait<Sumeragi = S, World = W>,
{
    let tx = world::sign_tx(vec![]);
    let leader = network
        .send_to_actor_on_peers(|iroha| &iroha.sumeragi, IsLeader)
        .await;
    let (_, peer) = leader
        .into_iter()
        .find(|(leader, _)| if to_leader { *leader } else { !*leader })
        .unwrap();
    let peer = network.peer_by_id(&peer).unwrap();
    peer.iroha
        .as_ref()
        .unwrap()
        .queue
        .push(tx, &*peer.iroha.as_ref().unwrap().wsv)
        .unwrap();
}

async fn put_tx_in_queue_to_all<W, G, S, K, B>(network: &Network<W, G, K, S, B>)
where
    W: WorldTrait,
    G: GenesisNetworkTrait,
    K: KuraTrait<World = W>,
    S: SumeragiTrait<GenesisNetwork = G, Kura = K, World = W>,
    B: BlockSynchronizerTrait<Sumeragi = S, World = W>,
{
    let tx = world::sign_tx(vec![]);
    for peer in network.peers() {
        peer.iroha
            .as_ref()
            .unwrap()
            .queue
            .push(tx.clone(), &*peer.iroha.as_ref().unwrap().wsv)
            .unwrap();
    }
}

async fn round<W, G, S, K, B>(network: &Network<W, G, K, S, B>)
where
    W: WorldTrait,
    G: GenesisNetworkTrait,
    K: KuraTrait<World = W>,
    S: SumeragiTrait<GenesisNetwork = G, Kura = K, World = W>,
    B: BlockSynchronizerTrait<Sumeragi = S, World = W>,
{
    for peer in network.peers() {
        peer.iroha
            .as_ref()
            .unwrap()
            .sumeragi
            .do_send(RetrieveTransactions)
            .await;
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "mock"]
async fn all_peers_commit_block() {
    iroha_logger::install_panic_hook().unwrap();
    let (network, _) = <Network<
        world::WithAlice,
        genesis::NoGenesis,
        iroha_core::kura::Kura<_>,
        Sumeragi<_, _, _>,
        BlockSynchronizer<_, _>,
    >>::start_test(10, 1)
    .await;

    // Send tx to leader
    put_tx_in_queue_to_peer(&network, true).await;
    round(&network).await;
    time::sleep(Duration::from_secs(2)).await;

    blocks_applied(&network, 1).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "mock"]
async fn change_view_on_commit_timeout() {
    iroha_logger::install_panic_hook().unwrap();
    let (network, _) = <Network<
        world::WithAlice,
        genesis::NoGenesis,
        iroha_core::kura::Kura<_>,
        SumeragiWithFault<_, _, _, sumeragi::Skip<sumeragi::BlockSigned, sumeragi::ProxyTail>>,
        BlockSynchronizer<_, _>,
    >>::start_test(10, 1)
    .await;

    // Send tx to leader
    put_tx_in_queue_to_peer(&network, true).await;
    round(&network).await;
    time::sleep(Duration::from_secs(4)).await;

    blocks_applied(&network, 0).await;

    let topologies = network
        .send_to_actor_on_peers(
            |iroha| &iroha.sumeragi,
            iroha_core::sumeragi::CurrentNetworkTopology,
        )
        .await;
    let invalid_block_hashes = network
        .send_to_actor_on_peers(
            |iroha| &iroha.sumeragi,
            iroha_core::sumeragi::InvalidatedBlockHashes,
        )
        .await;

    for (topology, _) in topologies {
        assert_eq!(topology.view_change_proofs().len(), 1);
    }
    for (hashes, _) in invalid_block_hashes {
        assert_eq!(hashes.len(), 1);
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "mock"]
async fn change_view_on_tx_receipt_timeout() {
    iroha_logger::install_panic_hook().unwrap();
    let (network, _) = <Network<
        world::WithAlice,
        genesis::NoGenesis,
        iroha_core::kura::Kura<_>,
        SumeragiWithFault<_, _, _, sumeragi::SkipTxForwardedAndGossipOnLeader>,
        BlockSynchronizer<_, _>,
    >>::start_test(10, 1)
    .await;

    // send to not leader
    put_tx_in_queue_to_peer(&network, false).await;

    // Let peers gossip tx.
    for peer in network.peers() {
        peer.iroha.as_ref().unwrap().sumeragi.do_send(Gossip).await;
    }

    // Wait while tx is gossiped
    time::sleep(Duration::from_millis(500)).await;

    // Let peers retrieve the gossiped tx and send to leader, so they can all understand the leader is unresponsive.
    for peer in network.peers() {
        peer.iroha
            .as_ref()
            .unwrap()
            .sumeragi
            .do_send(RetrieveTransactions)
            .await;
    }

    time::sleep(Duration::from_secs(3)).await;

    blocks_applied(&network, 0).await;

    let topologies = network
        .send_to_actor_on_peers(
            |iroha| &iroha.sumeragi,
            iroha_core::sumeragi::CurrentNetworkTopology,
        )
        .await;
    for (topology, _) in topologies {
        assert_eq!(topology.view_change_proofs().len(), 1);
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "mock"]
async fn change_view_on_block_creation_timeout() {
    iroha_logger::install_panic_hook().unwrap();
    let (network, _) = <Network<
        world::WithAlice,
        genesis::NoGenesis,
        iroha_core::kura::Kura<_>,
        SumeragiWithFault<_, _, _, sumeragi::Skip<sumeragi::BlockCreated, sumeragi::Any>>,
        BlockSynchronizer<_, _>,
    >>::start_test(10, 1)
    .await;

    // send to not leader
    put_tx_in_queue_to_all(&network).await;
    round(&network).await;
    time::sleep(Duration::from_secs(3)).await;

    blocks_applied(&network, 0).await;

    let topologies = network
        .send_to_actor_on_peers(
            |iroha| &iroha.sumeragi,
            iroha_core::sumeragi::CurrentNetworkTopology,
        )
        .await;

    for (topology, _) in topologies {
        assert_eq!(topology.view_change_proofs().len(), 1);
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "mock"]
async fn not_enough_votes() {
    iroha_logger::install_panic_hook().unwrap();
    let (network, _) = <Network<
        world::WithAlice,
        genesis::NoGenesis,
        iroha_core::kura::Kura<_>,
        SumeragiWithFault<_, _, _, sumeragi::EmptyBlockCreated>,
        BlockSynchronizer<_, _>,
    >>::start_test(10, 1)
    .await;

    put_tx_in_queue_to_peer(&network, true).await;
    round(&network).await;
    time::sleep(Duration::from_secs(4)).await;

    blocks_applied(&network, 0).await;

    let topologies = network
        .send_to_actor_on_peers(
            |iroha| &iroha.sumeragi,
            iroha_core::sumeragi::CurrentNetworkTopology,
        )
        .await;
    let invalid_block_hashes = network
        .send_to_actor_on_peers(
            |iroha| &iroha.sumeragi,
            iroha_core::sumeragi::InvalidatedBlockHashes,
        )
        .await;

    for (topology, _) in topologies {
        assert_eq!(topology.view_change_proofs().len(), 1);
    }
    for (hashes, _) in invalid_block_hashes {
        assert_eq!(hashes.len(), 1);
    }
}
