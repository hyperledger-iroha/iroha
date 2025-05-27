//! Metrics and status reporting

use std::{num::NonZeroUsize, sync::Arc, time::Duration};

#[cfg(debug_assertions)]
use iroha_crypto::HashOf;
use iroha_data_model::block::BlockHeader;
use iroha_futures::supervisor::{Child, OnShutdown};
use iroha_p2p::OnlinePeers;
use iroha_primitives::time::TimeSource;
use iroha_telemetry::metrics::Metrics;
use mv::storage::StorageReadOnly;
use tokio::sync::{mpsc, oneshot, watch, RwLock};

use crate::{
    kura::Kura,
    queue::Queue,
    state::{State, StateReadOnly, WorldReadOnly},
};

/// Slice of metrics used to be used from within [`State`].
///
/// Needed to brake the circular dependency from [`Telemetry`] to [`State`].
#[derive(Default, Clone)]
pub struct StateTelemetry {
    metrics: Arc<Metrics>,
}

impl StateTelemetry {
    /// Create from [`Metrics`]
    pub fn new(metrics: Arc<Metrics>) -> Self {
        Self { metrics }
    }

    /// Commit an observation of amounts used in transactions
    pub fn observe_tx_amount(&self, value: f64) {
        self.metrics.tx_amounts.observe(value);
    }
}

const CHANNEL_CAPACITY: usize = 1024;

enum Message {
    Sync { reply: oneshot::Sender<()> },
}

/// Handle to the telemetry state
#[derive(Clone)]
pub struct Telemetry {
    actor: mpsc::Sender<Message>,
    last_reported_block: Arc<RwLock<Option<BlockCommitReport>>>,
    metrics: Arc<Metrics>,
    time_source: TimeSource,
}

impl Telemetry {
    /// Increase dropped messages metric
    pub fn inc_dropped_messages(&self) {
        self.metrics.dropped_messages.inc();
    }

    /// Set view changes metrics
    pub fn set_view_changes(&self, value: u64) {
        self.metrics.view_changes.set(value);
    }

    /// Report the event of block commit, measuring the block time.
    pub fn report_block_commit_blocking(&self, block_header: BlockHeader) {
        let report = BlockCommitReport::new(block_header, &self.time_source);
        #[allow(clippy::cast_precision_loss)]
        self.metrics
            .commit_time_ms
            .observe(report.commit_time.as_millis() as f64);

        // This function is called from within the main loop.
        // We absolutely don't want to block it.
        // However, there is only one reader (the actor loop),
        // and it acquires the lock for a very brief period of time;
        // thus it shouldn't be a problem.
        let mut lock = self.last_reported_block.blocking_write();
        *lock = Some(report);
    }

    /// Some metrics are updated lazily, on demand.
    /// This async function completes once data is up to date.
    pub async fn metrics(&self) -> &Metrics {
        let (tx, rx) = oneshot::channel();
        self.actor
            .send(Message::Sync { reply: tx })
            .await
            .expect("metrics actor must be alive");
        rx.await.expect("metrics actor must be alive");
        &self.metrics
    }
}

struct Actor {
    handle: mpsc::Receiver<Message>,
    last_reported_block: Arc<RwLock<Option<BlockCommitReport>>>,
    last_sync_block: usize,
    online_peers: watch::Receiver<OnlinePeers>,
    metrics: Arc<Metrics>,
    state: Arc<State>,
    kura: Arc<Kura>,
    queue: Arc<Queue>,
    time_source: TimeSource,
}

impl Actor {
    async fn run(mut self) {
        while let Some(message) = self.handle.recv().await {
            match message {
                Message::Sync { reply } => {
                    self.sync().await;
                    let _ = reply.send(());
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn sync(&mut self) {
        self.metrics
            .connected_peers
            .set(self.online_peers.borrow().len() as u64);
        self.metrics.queue_size.set(self.queue.tx_len() as u64);

        let last_reported_block = {
            let lock = self.last_reported_block.read().await;
            let Some(value) = *lock else {
                // wait until genesis
                return;
            };
            value
        };

        let state_view = self.state.view();

        let start_index = self.last_sync_block;
        {
            let mut inc_txs_accepted = 0;
            let mut inc_txs_rejected = 0;
            let mut inc_blocks = 0;
            let mut inc_blocks_non_empty = 0;

            let mut block_index = start_index;
            while block_index < last_reported_block.height {
                let Some(block) = NonZeroUsize::new(
                    block_index
                        .checked_add(1)
                        .expect("INTERNAL BUG: Blockchain height exceeds usize::MAX"),
                )
                .and_then(|index| self.kura.get_block(index)) else {
                    break;
                };
                block_index += 1;

                let block_txs_rejected = block.errors().count() as u64;
                let block_txs_all = block.transactions().count() as u64;
                let block_txs_approved = block_txs_all - block_txs_rejected;

                inc_blocks += 1;
                inc_txs_accepted += block_txs_approved;
                inc_txs_rejected += block_txs_rejected;
                if block_txs_all != 0 {
                    inc_blocks_non_empty += 1;
                }

                if block_index == last_reported_block.height {
                    // for some reason, using `debug_assert!(..)` doesn't work here
                    // in release build Rust complains about the absent `.hash` field (feature gated via
                    // `debug_assertions`), which doesn't make sense - the whole statement should be feature-gated too
                    #[cfg(debug_assertions)]
                    assert_eq!(
                        block.hash(),
                        last_reported_block.hash,
                        "BUG: Reported block hash is different (reported {}, actual {})",
                        last_reported_block.hash,
                        block.hash()
                    );
                    #[allow(clippy::cast_precision_loss)]
                    self.metrics.last_commit_time_ms.set(
                        u64::try_from(last_reported_block.commit_time.as_millis())
                            .expect("time should fit into u64"),
                    );
                }
            }
            self.last_sync_block = block_index;

            self.metrics
                .txs
                .with_label_values(&["accepted"])
                .inc_by(inc_txs_accepted);
            self.metrics
                .txs
                .with_label_values(&["rejected"])
                .inc_by(inc_txs_rejected);
            self.metrics
                .txs
                .with_label_values(&["total"])
                .inc_by(inc_txs_accepted + inc_txs_rejected);
            self.metrics.block_height.inc_by(inc_blocks);
            self.metrics
                .block_height_non_empty
                .inc_by(inc_blocks_non_empty);
        }

        #[allow(clippy::cast_possible_truncation)]
        if let Some(timestamp) = state_view.genesis_timestamp() {
            let curr_time = self.time_source.get_unix_time();

            // this will overflow in 584,942,417 years
            self.metrics.uptime_since_genesis_ms.set(
                (curr_time - timestamp)
                    .as_millis()
                    .try_into()
                    .expect("Timestamp should fit into u64"),
            )
        }

        // Below metrics could be out of sync with the "latest block" metric,
        // since state_view might be potentially ahead of the last reported block.
        // This is fine because this time window _should_ be very narrow.

        self.metrics
            .domains
            .set(state_view.world().domains().len() as u64);
        for domain in state_view.world().domains_iter() {
            match self
                .metrics
                .accounts
                .get_metric_with_label_values(&[domain.id.name.as_ref()])
            {
                Err(err) => {
                    #[cfg(debug_assertions)]
                    panic!("BUG: Failed to compose domains: {err}");
                    #[cfg(not(debug_assertions))]
                    iroha_logger::error!(?err, "Failed to compose domains")
                }
                Ok(metrics) => {
                    metrics.set(
                        state_view
                            .world()
                            .accounts_in_domain_iter(&domain.id)
                            .count() as u64,
                    );
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct BlockCommitReport {
    /// Only in debug, to ensure consistency
    #[cfg(debug_assertions)]
    hash: HashOf<BlockHeader>,
    height: usize,
    commit_time: Duration,
}

impl BlockCommitReport {
    fn new(block_header: BlockHeader, time_source: &TimeSource) -> Self {
        let commit_time = if block_header.is_genesis() {
            Duration::ZERO
        } else {
            let now = time_source.get_unix_time();
            let created_at = block_header.creation_time();
            debug_assert!(now >= created_at);
            now - created_at
        };
        Self {
            #[cfg(debug_assertions)]
            hash: block_header.hash(),
            height: usize::try_from(block_header.height().get())
                .expect("block height should fit into usize"),
            commit_time,
        }
    }
}

/// Start the telemetry service
pub fn start(
    metrics: Arc<Metrics>,
    state: Arc<State>,
    kura: Arc<Kura>,
    queue: Arc<Queue>,
    online_peers: watch::Receiver<OnlinePeers>,
    time_source: TimeSource,
) -> (Telemetry, Child) {
    let (actor, handle) = mpsc::channel(CHANNEL_CAPACITY);
    let last_reported_block = Arc::new(RwLock::new(None));
    (
        Telemetry {
            actor,
            last_reported_block: last_reported_block.clone(),
            metrics: metrics.clone(),
            time_source: time_source.clone(),
        },
        Child::new(
            tokio::spawn(
                Actor {
                    handle,
                    metrics,
                    state,
                    kura,
                    queue,
                    last_sync_block: 0,
                    last_reported_block,
                    online_peers,
                    time_source,
                }
                .run(),
            ),
            OnShutdown::Abort,
        ),
    )
}

#[cfg(test)]
#[allow(clippy::disallowed_types)]
mod tests {
    use std::{collections::HashSet, time::Duration};

    use iroha_crypto::{KeyPair, PrivateKey};
    use iroha_data_model::{
        account::AccountId,
        isi::Log,
        peer::{Peer, PeerId},
        prelude::TransactionBuilder,
        ChainId, Level,
    };
    use iroha_primitives::{
        addr::{socket_addr, SocketAddr},
        time::{MockTimeHandle, TimeSource},
    };
    use iroha_test_samples::gen_account_in;
    use nonzero_ext::nonzero;
    use tokio::task::spawn_blocking;

    use super::*;
    use crate::{
        block::{BlockBuilder, CommittedBlock, NewBlock},
        prelude::World,
        query::store::LiveQueryStore,
        sumeragi::network_topology::Topology,
        tx::AcceptedTransaction,
    };

    struct SystemUnderTest {
        telemetry: Telemetry,
        _child: Child,
        online_peers_tx: watch::Sender<OnlinePeers>,
        mock_time_handle: MockTimeHandle,
        time_source: TimeSource,
        kura: Arc<Kura>,
        state: Arc<State>,
        account_id: AccountId,
        account_keypair: KeyPair,
        leader_private_key: PrivateKey,
        topology: Topology,
    }

    impl SystemUnderTest {
        fn new() -> Self {
            let metrics = Arc::new(Metrics::default());

            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = Arc::new(State::with_telemetry(
                World::default(),
                kura.clone(),
                query_handle,
                StateTelemetry::new(metrics.clone()),
            ));
            let (peers_tx, peers_rx) = watch::channel(<_>::default());
            let (mock_time_handle, time_source) = TimeSource::new_mock(Duration::default());
            let queue = Arc::new(Queue::test(
                iroha_config::parameters::actual::Queue {
                    capacity: nonzero!(10usize),
                    capacity_per_user: nonzero!(10usize),
                    transaction_time_to_live: Duration::from_secs(100),
                },
                &time_source,
            ));

            let (telemetry, child) = start(
                metrics,
                state.clone(),
                kura.clone(),
                queue,
                peers_rx,
                time_source.clone(),
            );

            let (leader_public_key, leader_private_key) = KeyPair::random().into_parts();
            let peer_id = PeerId::new(leader_public_key);
            let topology = Topology::new(vec![peer_id]);
            let (account_id, account_keypair) = gen_account_in("wonderland");

            Self {
                telemetry,
                _child: child,
                mock_time_handle,
                time_source,
                kura,
                state,
                online_peers_tx: peers_tx,
                account_id,
                account_keypair,
                topology,
                leader_private_key,
            }
        }

        fn create_block(&self) -> NewBlock {
            let (max_clock_drift, tx_limits) = {
                let params = self.state.view().world.parameters;
                (params.sumeragi().max_clock_drift(), params.transaction)
            };

            let tx = TransactionBuilder::new_with_time_source(
                chain_id(),
                self.account_id.clone(),
                &self.time_source,
            )
            .with_instructions([Log::new(Level::DEBUG, "meow".to_string())])
            .sign(self.account_keypair.private_key());
            let tx =
                AcceptedTransaction::accept(tx, &chain_id(), max_clock_drift, tx_limits).unwrap();
            let block = BlockBuilder::new_with_time_source(vec![tx], self.time_source.clone())
                .chain(0, self.state.view().latest_block().as_deref())
                .sign(&self.leader_private_key)
                .unpack(|_| {});

            block
        }

        fn commit_block(&self, block: NewBlock) -> CommittedBlock {
            let mut state_block = self.state.block(block.header());
            let block = block
                .validate_and_record_transactions(&mut state_block)
                .unpack(|_| {})
                .commit(&self.topology)
                .unpack(|_| {})
                .unwrap();
            let _events =
                state_block.apply_without_execution(&block, self.topology.as_ref().to_owned());
            state_block.commit();

            self.kura.store_block(block.clone());

            block
        }

        async fn report_commit_block(&self, block_header: BlockHeader) {
            let handle = self.telemetry.clone();
            spawn_blocking(move || handle.report_block_commit_blocking(block_header))
                .await
                .unwrap();
        }
    }

    fn random_peer(addr: SocketAddr) -> Peer {
        Peer::new(addr, KeyPair::random().public_key().clone())
    }

    fn chain_id() -> ChainId {
        ChainId::from("05010203423210230402")
    }

    #[tokio::test]
    async fn initial_metrics() {
        let sut = SystemUnderTest::new();

        let metrics = sut.telemetry.metrics().await;

        assert_eq!(metrics.block_height.get(), 0);
        assert_eq!(metrics.last_commit_time_ms.get(), 0);
        assert_eq!(metrics.domains.get(), 0);
        assert_eq!(metrics.connected_peers.get(), 0);
    }

    #[tokio::test]
    async fn set_online_peers() {
        let sut = SystemUnderTest::new();
        let peers: HashSet<_> = vec![
            random_peer(socket_addr!(100.100.100.100:80)),
            random_peer(socket_addr!(200.100.30.1:9001)),
        ]
        .into_iter()
        .collect();

        sut.online_peers_tx.send(peers.clone()).unwrap();
        let metrics = sut.telemetry.metrics().await;

        assert_eq!(metrics.connected_peers.get(), 2);
    }

    #[tokio::test]
    async fn commit_blocks() {
        // this indicates time padding applied in the block builder
        const CORRECTION: u64 = 1;

        let sut = SystemUnderTest::new();

        // commit first (genesis) block
        let block = sut.create_block();
        sut.mock_time_handle.advance(Duration::from_millis(100));
        let block = sut.commit_block(block);
        sut.report_commit_block(block.as_ref().header()).await;

        let metrics = sut.telemetry.metrics().await;
        assert_eq!(metrics.block_height.get(), 1);
        assert_eq!(metrics.block_height_non_empty.get(), 1);
        assert_eq!(metrics.last_commit_time_ms.get(), 0); // zero for genesis

        // For some reason, the tx is rejected (doesn't really matter)
        assert_eq!(metrics.txs.with_label_values(&["accepted"]).get(), 0);
        assert_eq!(metrics.txs.with_label_values(&["rejected"]).get(), 1);
        assert_eq!(metrics.txs.with_label_values(&["total"]).get(), 1);

        // second block
        let block = sut.create_block();
        sut.mock_time_handle.advance(Duration::from_millis(150));
        let block = sut.commit_block(block);
        sut.report_commit_block(block.as_ref().header()).await;

        let metrics = sut.telemetry.metrics().await;
        assert_eq!(metrics.block_height.get(), 2);
        assert_eq!(metrics.block_height_non_empty.get(), 2);
        assert_eq!(metrics.last_commit_time_ms.get(), 150 - CORRECTION);
        assert_eq!(metrics.txs.with_label_values(&["accepted"]).get(), 0);
        assert_eq!(metrics.txs.with_label_values(&["rejected"]).get(), 2);
        assert_eq!(metrics.txs.with_label_values(&["total"]).get(), 2);

        // third block - committed, but not reported yet
        let block = sut.create_block();
        sut.mock_time_handle.advance(Duration::from_millis(170));
        let block = sut.commit_block(block);

        let metrics = sut.telemetry.metrics().await;
        // old data
        assert_eq!(metrics.block_height.get(), 2);
        assert_eq!(metrics.block_height_non_empty.get(), 2);
        assert_eq!(metrics.last_commit_time_ms.get(), 150 - CORRECTION);

        sut.report_commit_block(block.as_ref().header()).await;

        let metrics = sut.telemetry.metrics().await;
        assert_eq!(metrics.block_height.get(), 3);
        assert_eq!(metrics.block_height_non_empty.get(), 3);
        assert_eq!(metrics.last_commit_time_ms.get(), 170 - CORRECTION);
    }

    #[test]
    fn genesis_commit_time_is_zero() {
        let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1500));
        let header = BlockBuilder::new_with_time_source(vec![], time_source.clone())
            .chain(1, None)
            .sign(KeyPair::random().private_key())
            .unpack(|_| {})
            .header();

        time_handle.advance(Duration::from_secs(12));
        let report = BlockCommitReport::new(header, &time_source);

        assert_eq!(report.commit_time, Duration::ZERO)
    }
}
