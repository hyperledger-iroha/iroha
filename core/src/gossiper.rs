//! Gossiper is actor which is responsible for transaction gossiping

use std::{sync::Arc, time::Duration};

use iroha_config::sumeragi::Configuration;
use iroha_data_model::transaction::{
    AcceptedTransaction, InBlock, Transaction, VersionedAcceptedTransaction,
    VersionedSignedTransaction,
};
use iroha_p2p::Broadcast;
use parity_scale_codec::{Decode, Encode};
use tokio::sync::mpsc;

use crate::{queue::Queue, sumeragi::SumeragiHandle, IrohaNetwork, NetworkMessage};

/// [`Gossiper`] actor handle.
#[derive(Clone)]
pub struct TransactionGossiperHandle {
    message_sender: mpsc::Sender<TransactionGossip>,
}

impl TransactionGossiperHandle {
    /// Send [`TransactionGossip`] to actor
    pub async fn gossip(&self, gossip: TransactionGossip) {
        self.message_sender
            .send(gossip)
            .await
            .expect("Gossiper must handle messages until there is at least one handle to it")
    }
}

/// Actor to gossip transactions and receive transaction gossips
pub struct TransactionGossiper {
    /// The size of batch that is being gossiped. Smaller size leads
    /// to longer time to synchronise, useful if you have high packet loss.
    gossip_batch_size: u32,
    /// The time between gossiping. More frequent gossiping shortens
    /// the time to sync, but can overload the network.
    gossip_period: Duration,
    /// Address of queue
    queue: Arc<Queue>,
    /// [`iroha_p2p::Network`] actor handle
    network: IrohaNetwork,
    /// Sumearagi
    sumeragi: SumeragiHandle,
}

impl TransactionGossiper {
    /// Start [`Self`] actor.
    pub fn start(self) -> TransactionGossiperHandle {
        let (message_sender, message_receiver) = mpsc::channel(1);
        tokio::task::spawn(self.run(message_receiver));
        TransactionGossiperHandle { message_sender }
    }

    /// Construct [`Self`] from configuration
    pub fn from_configuration(
        // Currently we are using configuration parameters from sumeragi not to break configuration
        configuartion: &Configuration,
        network: IrohaNetwork,
        queue: Arc<Queue>,
        sumeragi: SumeragiHandle,
    ) -> Self {
        Self {
            queue,
            sumeragi,
            network,
            gossip_batch_size: configuartion.gossip_batch_size,
            gossip_period: Duration::from_millis(configuartion.gossip_period_ms),
        }
    }

    async fn run(self, mut message_receiver: mpsc::Receiver<TransactionGossip>) {
        let mut gossip_period = tokio::time::interval(self.gossip_period);
        #[allow(clippy::arithmetic_side_effects)]
        loop {
            tokio::select! {
                _ = gossip_period.tick() => self.gossip_transactions(),
                transaction_gossip = message_receiver.recv() => {
                    let Some(transaction_gossip) = transaction_gossip else {
                        iroha_logger::info!("All handler to Gossiper are dropped. Shutting down...");
                        break;
                    };
                    self.handle_transaction_gossip(transaction_gossip);
                }
            }
            tokio::task::yield_now().await;
        }
    }

    fn gossip_transactions(&self) {
        let txs = self.sumeragi.wsv(|wsv| {
            self.queue
                .n_random_transactions(self.gossip_batch_size, wsv)
        });

        if txs.is_empty() {
            iroha_logger::debug!("Nothing to gossip");
            return;
        }

        iroha_logger::trace!(tx_count = txs.len(), "Gossiping transactions");
        self.network.broadcast(Broadcast {
            data: NetworkMessage::TransactionGossiper(Box::new(TransactionGossip::new(txs))),
        });
    }

    fn handle_transaction_gossip(&self, TransactionGossip { txs }: TransactionGossip) {
        iroha_logger::trace!(size = txs.len(), "Received new transaction gossip");

        let transaction_limits = self
            .sumeragi
            .wsv(|wsv| wsv.config.borrow().transaction_limits);

        for tx in txs {
            match <AcceptedTransaction as InBlock>::accept(tx.into_v1(), &transaction_limits) {
                Ok(tx) => match self.sumeragi.wsv(|wsv| self.queue.push(tx.into(), wsv)) {
                    Ok(_) => {}
                    Err(crate::queue::Failure {
                        tx,
                        err: crate::queue::Error::InBlockchain,
                    }) => {
                        iroha_logger::debug!(tx_hash = %tx.hash(), "Transaction already in blockchain, ignoring...")
                    }
                    Err(crate::queue::Failure { tx, err }) => {
                        iroha_logger::error!(?err, tx_hash = %tx.hash(), "Failed to enqueue transaction.")
                    }
                },
                Err(err) => iroha_logger::error!(%err, "Transaction rejected"),
            }
        }
    }
}

/// Message for gossiping batches of transactions.
#[derive(Decode, Encode, Debug, Clone)]
pub struct TransactionGossip {
    /// Batch of transactions.
    pub txs: Vec<VersionedSignedTransaction>,
}

impl TransactionGossip {
    /// Constructor.
    pub fn new(txs: Vec<VersionedAcceptedTransaction>) -> Self {
        Self {
            // Converting into non-accepted transaction because it's not possible
            // to guarantee that the sending peer checked transaction limits
            txs: txs.into_iter().map(Into::into).collect(),
        }
    }
}
