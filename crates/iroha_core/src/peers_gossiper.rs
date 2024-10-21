//! Peers gossiper is actor which is responsible for gossiping addresses of peers.
//!
//! E.g. peer A changes address, connects to peer B,
//! and then peer B will broadcast address of peer A to other peers.

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use iroha_config::parameters::actual::TrustedPeers;
use iroha_data_model::peer::{Peer, PeerId};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_p2p::{Broadcast, UpdatePeers};
use iroha_primitives::{addr::SocketAddr, unique_vec::UniqueVec};
use iroha_version::{Decode, Encode};
use tokio::sync::mpsc;

use crate::{IrohaNetwork, NetworkMessage};

/// [`Gossiper`] actor handle.
#[derive(Clone)]
pub struct PeersGossiperHandle {
    message_sender: mpsc::Sender<PeersGossip>,
}

impl PeersGossiperHandle {
    /// Send [`PeersGossip`] to actor
    pub async fn gossip(&self, gossip: PeersGossip) {
        self.message_sender
            .send(gossip)
            .await
            .expect("Gossiper must handle messages until there is at least one handle to it")
    }
}

/// Actor which gossips peers addresses.
pub struct PeersGossiper {
    /// Peers provided at startup
    initial_peers: HashMap<PeerId, SocketAddr>,
    /// Peers received via gossiping from other peers
    gossip_peers: HashMap<PeerId, SocketAddr>,
    network: IrohaNetwork,
}

/// Terminology:
/// * Topology - public keys of current network derived from blockchain (Register/Unregister Peer Isi)
/// * Peers addresses - currently known addresses for peers in topology. Might be unknown for some peer.
///
/// There are three sources of peers addresses:
/// 1. Provided at iroha startup (`TRUSTED_PEERS` env var)
/// 2. Currently connected online peers.
///    Some peer might change address and connect to our peer,
///    such connection will be accepted if peer public key is in topology.
/// 3. Received via gossiping from other peers.
impl PeersGossiper {
    /// Start actor.
    pub fn start(
        trusted_peers: TrustedPeers,
        network: IrohaNetwork,
        shutdown_signal: ShutdownSignal,
    ) -> (PeersGossiperHandle, Child) {
        let initial_peers = trusted_peers
            .others
            .into_iter()
            .map(|peer| (peer.id, peer.address))
            .collect();
        let gossiper = Self {
            initial_peers,
            gossip_peers: HashMap::new(),
            network,
        };
        gossiper.network_update_peers_addresses();

        let (message_sender, message_receiver) = mpsc::channel(1);
        (
            PeersGossiperHandle { message_sender },
            Child::new(
                tokio::task::spawn(gossiper.run(message_receiver, shutdown_signal)),
                OnShutdown::Abort,
            ),
        )
    }

    async fn run(
        mut self,
        mut message_receiver: mpsc::Receiver<PeersGossip>,
        shutdown_signal: ShutdownSignal,
    ) {
        let mut gossip_period = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = gossip_period.tick() => {
                    self.gossip_peers()
                }
                _ = self.network.wait_online_peers_update(|_| ()) => {
                    self.gossip_peers();
                }
                Some(peers_gossip) = message_receiver.recv() => {
                    self.handle_peers_gossip(peers_gossip);
                }
                () = shutdown_signal.receive() => {
                    iroha_logger::debug!("Shutting down peers gossiper");
                    break;
                },
            }
            tokio::task::yield_now().await;
        }
    }

    fn gossip_peers(&self) {
        let online_peers = self.network.online_peers(Clone::clone);
        let online_peers = UniqueVec::from_iter(online_peers);
        let data = NetworkMessage::PeersGossiper(Box::new(PeersGossip(online_peers)));
        self.network.broadcast(Broadcast { data });
    }

    fn handle_peers_gossip(&mut self, PeersGossip(peers): PeersGossip) {
        for peer in peers {
            self.gossip_peers.insert(peer.id, peer.address);
        }
        self.network_update_peers_addresses();
    }

    fn network_update_peers_addresses(&self) {
        let online_peers = self.network.online_peers(Clone::clone);
        let online_peers_ids = online_peers
            .into_iter()
            .map(|peer| peer.id)
            .collect::<HashSet<_>>();

        let mut peers = Vec::new();
        for (id, address) in &self.initial_peers {
            if !online_peers_ids.contains(id) {
                peers.push((id.clone(), address.clone()));
            }
        }
        for (id, address) in &self.gossip_peers {
            if !online_peers_ids.contains(id) {
                peers.push((id.clone(), address.clone()));
            }
        }

        let update = UpdatePeers(peers);
        self.network.update_peers_addresses(update);
    }
}

/// Message for gossiping peers addresses.
#[derive(Decode, Encode, Debug, Clone)]
pub struct PeersGossip(UniqueVec<Peer>);
