//! The node's lane instances (`specs/sumeragi_lanes.md` §4.1): one driver per lane incarnation,
//! as a member when the node's key is in the pinned committee and as an observer otherwise. An
//! instance starts once the global chain has applied its `active_from` height and stops after
//! the global chain retires it.
//!
//! Every instance shares the node's transport, ingress (frames carry their instance id), safety
//! record store, key and driver limits with the global instance; its blocks go to the lane
//! store the global executor merges from.
//!
//! **Record provenance.** A lane instance exists only from the global block that creates its
//! incarnation, and the node signs for it only through this record store. When the store's
//! installation log shows that the global instance started with the key, every lane signature
//! of that key is in this store, so a lane instance without a record starts from a fresh one;
//! otherwise the global instance's provenance rules (§7.4 of `specs/sumeragi.md`) apply
//! unchanged and the node follows the lane without signing.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use iroha_config::parameters::actual::SumeragiLocalOverrides;
use iroha_crypto::KeyPair;
use iroha_data_model::{NetworkId, sumeragi_lanes::SumeragiLaneRecord};
use iroha_model_base::topology::LaneId;
use iroha_sumeragi::{crypto::NoAttestation, types::Hash32};
use parking_lot::Mutex;

use super::{
    executor::LaneExecutor,
    global::{AppliedWatch, GlobalAnchors, QueueLaneTransactions, StatelessChecks},
    lane_genesis_result, lane_height_config, lane_instance,
    registry::LaneStores,
};
use crate::{
    queue::Queue,
    state::{State, WorldReadOnly},
    sumeragi::{
        bodies::{BodyLimits, FileBodyStore},
        crypto::{BlsCrypto, KeyPairSigner, core_key},
        driver::{
            Driver, DriverConfig, DriverStart, RunningDriver, SharedCrypto, assemble_init,
            persist::install_records,
            traits::{
                BlockStore as _, Frame, LogEntry, Net, Observer, RecordStore as _, SystemClock,
            },
        },
        net::SumeragiIngress,
        node::{LogObserver, local_params, startup_nonce},
        records::{FileRecordStore, fresh_store_id},
    },
};

/// How often the runner re-checks the lane set without a new global height.
const IDLE_CHECK: Duration = Duration::from_millis(500);

/// The node's transport as every lane driver holds it.
struct SharedNet(Arc<dyn Net>);

impl Net for SharedNet {
    fn send(&self, to: &iroha_sumeragi::types::PublicKey, frame: &Frame) {
        self.0.send(to, frame);
    }
}

/// What lane instances share with the node's global instance.
pub struct LaneRunnerInputs {
    /// The node's state.
    pub state: Arc<State>,
    /// The transaction queue.
    pub queue: Arc<Queue>,
    /// The global chain's applied tip.
    pub watch: Arc<AppliedWatch>,
    /// The node's lane block stores.
    pub stores: Arc<LaneStores>,
    /// The node's cryptography (lane committee keys are admitted into it).
    pub crypto: Arc<BlsCrypto>,
    /// The transport.
    pub net: Arc<dyn Net>,
    /// The ingress routing inbound frames by instance, when the node is on a network.
    pub ingress: Option<Arc<SumeragiIngress>>,
    /// The node's safety record store.
    pub records: Arc<FileRecordStore>,
    /// The global instance id (its installation proves the store's custody of the key).
    pub global_instance: Hash32,
    /// Root of the body stores.
    pub bodies_dir: PathBuf,
    /// The node's consensus key pair.
    pub key_pair: KeyPair,
    /// The node's local parameter overrides.
    pub local: SumeragiLocalOverrides,
    /// Driver limits.
    pub driver: DriverConfig,
    /// The network.
    pub network: NetworkId,
    /// The chain id.
    pub chain_id: String,
}

struct RunningLane {
    instance: Hash32,
    driver: RunningDriver,
}

struct Inner {
    inputs: LaneRunnerInputs,
    net: Arc<SharedNet>,
    running: Mutex<BTreeMap<(LaneId, [u8; 32]), RunningLane>>,
    stop: AtomicBool,
}

/// The node's lane instances and the thread that keeps them in step with the global chain.
pub struct LaneRunner {
    inner: Arc<Inner>,
    thread: Option<JoinHandle<()>>,
}

impl core::fmt::Debug for LaneRunner {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LaneRunner")
            .field("running", &self.inner.running.lock().len())
            .finish_non_exhaustive()
    }
}

/// One running lane instance, for status reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaneInstance {
    /// The lane.
    pub lane: LaneId,
    /// Its incarnation.
    pub incarnation: [u8; 32],
    /// Its instance id.
    pub instance: Hash32,
}

impl LaneRunner {
    /// Start the lanes the applied state has activated and the thread that follows the global
    /// chain.
    ///
    /// # Errors
    /// The thread cannot be spawned.
    pub fn spawn(inputs: LaneRunnerInputs) -> std::io::Result<Self> {
        let inner = Arc::new(Inner {
            net: Arc::new(SharedNet(Arc::clone(&inputs.net))),
            inputs,
            running: Mutex::new(BTreeMap::new()),
            stop: AtomicBool::new(false),
        });
        inner.reconcile();
        let thread = {
            let inner = Arc::clone(&inner);
            crate::sumeragi::threads::sumeragi_thread_builder("sumeragi-lanes").spawn(
                move || {
                    while !inner.stop.load(Ordering::Acquire) {
                        let height = inner.inputs.watch.height();
                        let _ = inner
                            .inputs
                            .watch
                            .wait_for(height.saturating_add(1), IDLE_CHECK);
                        inner.reconcile();
                    }
                },
            )?
        };
        Ok(Self {
            inner,
            thread: Some(thread),
        })
    }

    /// The running lane instances.
    #[must_use]
    pub fn instances(&self) -> Vec<LaneInstance> {
        self.inner
            .running
            .lock()
            .iter()
            .map(|((lane, incarnation), running)| LaneInstance {
                lane: *lane,
                incarnation: *incarnation,
                instance: running.instance,
            })
            .collect()
    }

    /// Stop every lane instance and the runner thread.
    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.inner.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let running = std::mem::take(&mut *self.inner.running.lock());
        for (_, lane) in running {
            self.inner.stop_lane(lane);
        }
    }
}

impl Drop for LaneRunner {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Inner {
    /// Stop retired incarnations and start the activated ones.
    fn reconcile(&self) {
        let (applied, lanes) = {
            let view = self.inputs.state.view();
            (
                u64::try_from(view.height()).unwrap_or(0),
                view.world().sumeragi_lanes().clone(),
            )
        };
        let mut running = self.running.lock();
        let retired = running
            .keys()
            .filter(|(lane, incarnation)| {
                lanes
                    .lane(*lane)
                    .is_none_or(|record| record.incarnation != *incarnation)
            })
            .copied()
            .collect::<Vec<_>>();
        for key in retired {
            if let Some(lane) = running.remove(&key) {
                self.stop_lane(lane);
                self.inputs.stores.release(key.0, &key.1);
                iroha_logger::info!(lane = %key.0, "sumeragi: lane instance retired");
            }
        }
        for record in &lanes.lanes {
            let key = (record.lane, record.incarnation);
            if record.active_from > applied || running.contains_key(&key) {
                continue;
            }
            match self.start_lane(record) {
                Ok(lane) => {
                    iroha_logger::info!(
                        lane = %record.lane,
                        instance = %hex::encode(lane.instance.0),
                        "sumeragi: lane instance started"
                    );
                    running.insert(key, lane);
                }
                Err(error) => {
                    iroha_logger::warn!(lane = %record.lane, %error, "sumeragi: lane instance did not start");
                }
            }
        }
    }

    fn stop_lane(&self, lane: RunningLane) {
        if let Some(ingress) = &self.inputs.ingress {
            ingress.unregister(&lane.instance);
        }
        lane.driver.shutdown();
    }

    fn start_lane(&self, record: &SumeragiLaneRecord) -> Result<RunningLane, String> {
        let inputs = &self.inputs;
        let config = lane_height_config(record).map_err(|error| error.to_string())?;
        for member in &record.committee {
            inputs
                .crypto
                .admit(member.peer.public_key(), &member.pop)
                .map_err(|error| format!("committee key {}: {error:?}", member.peer))?;
        }
        let shared: SharedCrypto = inputs.crypto.clone();
        let instance = lane_instance(&*shared, &inputs.network, &inputs.chain_id, record);
        let store = inputs
            .stores
            .store(record.lane, &record.incarnation)
            .map_err(|error| error.to_string())?;
        let key = core_key(inputs.key_pair.public_key()).map_err(|error| error.to_string())?;
        let custody = inputs
            .records
            .log()
            .map_err(|error| error.to_string())?
            .iter()
            .any(|entry| {
                matches!(entry, LogEntry::Instance { instance, key: logged, .. }
                    if *instance == inputs.global_instance && *logged == key)
            });
        let found = install_records(
            &*inputs.records,
            &*shared,
            &instance,
            &[(key, false)],
            0,
            custody,
            &mut fresh_store_id,
        )
        .map_err(|error| error.to_string())?;
        let genesis = (
            super::lane_genesis_hash(&inputs.network, record),
            lane_genesis_result(record),
        );
        let tip = store.height();
        let configs = (tip..=tip.saturating_add(2))
            .map(|height| (height, config.clone()))
            .collect();
        let init = assemble_init(
            &*store,
            instance,
            0,
            genesis,
            record.params.demotion_window.get(),
            found,
            configs,
            startup_nonce(),
        )
        .map_err(|error| error.to_string())?;
        let bodies = Arc::new(
            FileBodyStore::open(
                &inputs.bodies_dir,
                &instance,
                Arc::clone(&shared),
                BodyLimits::default(),
            )
            .map_err(|error| error.to_string())?,
        );
        let member = record
            .committee
            .iter()
            .any(|member| member.peer.public_key() == inputs.key_pair.public_key());
        let transactions = member.then(|| {
            Arc::new(QueueLaneTransactions::new(
                record.lane,
                Arc::clone(&inputs.queue),
                Arc::clone(&inputs.state),
            ))
        });
        let executor = LaneExecutor::recover(
            record.clone(),
            config.clone(),
            Arc::new(GlobalAnchors::new(
                Arc::clone(&inputs.state),
                Arc::clone(&inputs.watch),
            )),
            StatelessChecks::new(inputs.network),
            transactions,
            genesis.0,
            &*store,
        )?;
        let signer = KeyPairSigner::new(&inputs.key_pair).map_err(|error| error.to_string())?;
        let observer: Arc<dyn Observer> = Arc::new(LogObserver);
        let driver = Driver::new(
            Arc::clone(&self.net),
            Arc::clone(&inputs.records),
            bodies,
            store,
            Arc::new(SystemClock::new()),
            executor,
            observer,
        )
        .spawn(
            inputs.driver,
            DriverStart {
                local: local_params(config.committee.n(), &inputs.local),
                init,
                signers: vec![Box::new(signer)],
                crypto: shared,
                attestor: Box::new(NoAttestation),
                verifier: Box::new(NoAttestation),
            },
        )
        .map_err(|error| error.to_string())?;
        if let Some(ingress) = &inputs.ingress {
            ingress.register(instance, Arc::new(driver.handle()));
        }
        Ok(RunningLane { instance, driver })
    }
}
