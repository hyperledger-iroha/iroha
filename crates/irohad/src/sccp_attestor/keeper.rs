//! In-node inbound light-client keeper (`specs/sccp.md` §4.13.4).
//!
//! Every validator with an active or pending bridge key keeps Taira's inbound light clients
//! fresh: when a light client's head is older than `advance_after` (default `ws_bound_ms / 4`),
//! the keeper builds an advance from the configured (or compiled public) endpoints and submits
//! it fee-exempt from its bridge key's account. Each advance is stepped to the light client's
//! `max_updates_per_advance` and the keeper's `max_advance_bytes` (the builders keep the longest
//! prefix of updates that fits), so a light client far behind catches up over several polls
//! instead of being dropped; an advance that still does not fit is dropped with a warning
//! instead of being submitted. Several keepers are harmless: the
//! advance carries the expected state hash, so only the first one moves the head and the rest
//! fail admission instead of paying.
//!
//! Ethereum, BSC and TRON advances use HTTP endpoints, TON advances ADNL liteservers.
//!
//! # Task structure
//!
//! The keeper is its own supervised task, started next to the attestor and sharing only its
//! key directory and submission path, so public RPC never delays signing. Each network is a
//! [`Lane`] with its own schedule; the lanes run concurrently and a poll of one lane is:
//!
//! 1. **Read** ([`KeeperHost::local`], on a blocking worker): load the key directory, take one
//!    committed state view, copy out the live bridge key's address, the light client and
//!    whether it aged out of its weak-subjectivity window at the latest committed block time,
//!    and drop the view.
//! 2. **Decide** ([`decide`]): idle (no live key, SCCP absent or no light client), needs
//!    Parliament recovery (frozen or aged: no RPC is spent on it until it is re-initialized),
//!    fresh, or stale.
//! 3. **Build** (stale only, on a blocking worker that holds no state view): build the advance
//!    under the lane's [`PollBudget`] of `poll_budget`. A build still running after 5/4 ×
//!    `poll_budget` counts as a failed poll; the lane starts no other poll until it returns. A
//!    build that fails on the data it was served (malformed, inconsistent or incomplete) moves
//!    the lane's clients to their next endpoints ([`AdvanceSource::rotate_endpoints`]).
//! 4. **Submit** ([`KeeperHost::submit`]): quote, sign and enqueue the advance fee-exempt from
//!    the bridge key's account (§4.19), unchanged from the attestor's own submissions.
//!
//! A lane polls again `poll_interval` later plus up to a quarter of jitter; each consecutive
//! failed poll doubles that wait, up to 64 × `poll_interval` ([`Cadence`]).

use std::{sync::Arc, thread, time::Duration};

use eyre::WrapErr as _;
use iroha_config::parameters::actual::SccpLightClientKeeper;
use iroha_core::{
    queue::Queue,
    smartcontracts::isi::sccp::{light_clients::WorldLightClientView, store},
    state::{State, StateReadOnly},
};
use iroha_data_model::{
    bridge::SccpNetworkV1,
    isi::{InstructionBox, sccp::AdvanceSccpLightClientV1},
    sccp::{
        keys::SccpBridgeKeyStateV1,
        light_client::{SccpLcAdvanceBytesV1, SccpLightClientV1},
    },
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_model_base::peer::PeerId;
use iroha_sccp::v1::key_file::SccpBridgeKeyFileV1;
use iroha_sccp_rpc::{
    BeaconClient, EvmClient, HttpEndpointKind, HttpTransport, PollBudget, TronClient,
    builders::{
        AdvanceBudgetV1, BuildError, SourceChainBuilder, bsc::BscBuilder,
        ethereum::EthereumBuilder, ton::TonBuilder, tron::TronBuilder,
    },
    endpoints::endpoint_seed,
    ton::LiteClient,
};
use tokio::task::{JoinHandle, JoinSet};

use super::{key_store::KeyStore, plan as attestor_plan, wall_clock_ms};
use crate::panic_recovery::{join_recoverable, recover_joined, spawn_blocking_recoverable};

/// How often the keeper looks for a new TON key block: TON light clients have no
/// `ws_bound_ms` (their freshness comes from the epoch's own validity).
const TON_ADVANCE_AFTER_MS: u64 = 3_600_000;
/// Consecutive failed polls after which a lane's wait stops doubling: at most
/// `2^6 = 64 × poll_interval`.
const MAX_BACKOFF_DOUBLINGS: u32 = 6;
/// The networks the keeper advances, one lane each, in this order.
const NETWORKS: [SccpNetworkV1; 4] = [
    SccpNetworkV1::EthereumMainnet,
    SccpNetworkV1::BscMainnet,
    SccpNetworkV1::TronMainnet,
    SccpNetworkV1::TonMainnet,
];
/// Shutdown wait of the keeper task: it holds no work that must finish.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(1);

/// Start the keeper as its own supervised task, or return `None` when it is disabled.
///
/// The task never exits before shutdown, also when no network has usable endpoints, because
/// the supervisor stops the node when a child exits early and the node never stops over SCCP.
pub(super) fn start<H: KeeperHost>(
    host: Arc<H>,
    config: SccpLightClientKeeper,
    seed: u64,
    shutdown_signal: ShutdownSignal,
) -> Option<Child> {
    if !config.enabled {
        iroha_logger::info!("SCCP light-client keeper disabled by configuration");
        return None;
    }
    let config = Arc::new(config);
    let task = tokio::task::spawn(async move {
        let lanes = build(Arc::clone(&config), seed).await;
        run(lanes, host, config, shutdown_signal).await;
    });
    Some(Child::new(task, OnShutdown::Wait(SHUTDOWN_WAIT)))
}

/// Run `lanes` concurrently until `shutdown`, then stop them.
///
/// Returns only on shutdown. A lane that stops early (a bug) is logged and the other lanes go
/// on.
pub(super) async fn run<H: KeeperHost>(
    lanes: Vec<Lane>,
    host: Arc<H>,
    config: Arc<SccpLightClientKeeper>,
    shutdown: ShutdownSignal,
) {
    let mut running = JoinSet::new();
    for lane in lanes {
        running.spawn(run_lane(
            lane,
            Arc::clone(&host),
            Arc::clone(&config),
            shutdown.clone(),
        ));
    }
    loop {
        tokio::select! {
            () = shutdown.receive() => break,
            Some(joined) = running.join_next() => {
                if joined.is_err() {
                    iroha_logger::error!("SCCP keeper: a network lane stopped unexpectedly");
                }
            }
        }
    }
    running.shutdown().await;
}

/// Poll one lane on its own schedule until `shutdown`.
async fn run_lane<H: KeeperHost>(
    mut lane: Lane,
    host: Arc<H>,
    config: Arc<SccpLightClientKeeper>,
    shutdown: ShutdownSignal,
) {
    let mut wait = lane.cadence.first();
    loop {
        tokio::select! {
            () = tokio::time::sleep(wait) => {}
            () = shutdown.receive() => return,
        }
        let succeeded = tokio::select! {
            succeeded = lane.poll(&host, &config) => succeeded,
            () = shutdown.receive() => return,
        };
        wait = if succeeded {
            lane.cadence.after_success()
        } else {
            lane.cadence.after_failure()
        };
    }
}

/// One network's advance builder: an `iroha_sccp_rpc` builder in the node, a fake in tests.
///
/// Builds block on network I/O, so the keeper calls them only on blocking workers and never
/// while it holds a state view.
pub(super) trait AdvanceSource: Send + Sync + 'static {
    /// Build an advance of a light client whose newest stored set is `latest_set_id`, stepped
    /// to `budget` (the longest prefix of at most `budget.max_items` items whose frame fits
    /// `budget.max_bytes`).
    ///
    /// # Errors
    ///
    /// Any endpoint failure, an inconsistent answer, or nothing to advance yet.
    fn advance(
        &self,
        latest_set_id: u64,
        budget: AdvanceBudgetV1,
    ) -> Result<SccpLcAdvanceBytesV1, BuildError>;

    /// Move every client of the source to its next endpoint, so the next build starts
    /// elsewhere. Does no I/O.
    fn rotate_endpoints(&self);
}

impl AdvanceSource for EthereumBuilder {
    fn advance(
        &self,
        latest_set_id: u64,
        budget: AdvanceBudgetV1,
    ) -> Result<SccpLcAdvanceBytesV1, BuildError> {
        SourceChainBuilder::advance(self, latest_set_id, budget)
    }

    fn rotate_endpoints(&self) {
        EthereumBuilder::rotate_endpoints(self);
    }
}

impl AdvanceSource for BscBuilder {
    fn advance(
        &self,
        latest_set_id: u64,
        budget: AdvanceBudgetV1,
    ) -> Result<SccpLcAdvanceBytesV1, BuildError> {
        SourceChainBuilder::advance(self, latest_set_id, budget)
    }

    fn rotate_endpoints(&self) {
        BscBuilder::rotate_endpoints(self);
    }
}

impl AdvanceSource for TronBuilder {
    fn advance(
        &self,
        latest_set_id: u64,
        budget: AdvanceBudgetV1,
    ) -> Result<SccpLcAdvanceBytesV1, BuildError> {
        SourceChainBuilder::advance(self, latest_set_id, budget)
    }

    fn rotate_endpoints(&self) {
        TronBuilder::rotate_endpoints(self);
    }
}

impl AdvanceSource for TonBuilder {
    fn advance(
        &self,
        latest_set_id: u64,
        budget: AdvanceBudgetV1,
    ) -> Result<SccpLcAdvanceBytesV1, BuildError> {
        SourceChainBuilder::advance(self, latest_set_id, budget)
    }

    fn rotate_endpoints(&self) {
        TonBuilder::rotate_endpoints(self);
    }
}

/// Whether a failed build blames the data its endpoints served (malformed, inconsistent or
/// missing what the advance needs), so the lane moves to other endpoints for the next poll.
/// RPC failures are not: the transports already failed over, or moved past an endpoint whose
/// answer discredited it.
fn blames_served_data(error: &BuildError) -> bool {
    !matches!(error, BuildError::Rpc(_))
}

/// What one poll of a lane needs from local state, copied out of one short-lived committed
/// view.
#[derive(Default)]
pub(super) struct Local {
    /// The key file of this peer's active or pending bridge key, when the key directory holds
    /// it.
    pub(super) key: Option<SccpBridgeKeyFileV1>,
    /// The installed light client of the lane's network.
    pub(super) light_client: Option<SccpLightClientV1>,
    /// Whether that light client's newest signing set is beyond its weak-subjectivity bound at
    /// the latest committed block time, the time admission checks advances against.
    pub(super) aged: bool,
}

/// The node services a lane uses. Both calls block and run on blocking workers only.
pub(super) trait KeeperHost: Send + Sync + 'static {
    /// Read what a poll of `network` needs. An implementation holds a state view only inside
    /// this call, never across the network I/O that follows.
    ///
    /// # Errors
    ///
    /// When the key directory cannot be listed.
    fn local(&self, network: SccpNetworkV1) -> eyre::Result<Local>;

    /// Quote, sign with `key` as its account and enqueue `advance`, fee-exempt on success
    /// (§4.19).
    ///
    /// # Errors
    ///
    /// When building, quoting, signing, admission or enqueueing fails.
    fn submit(
        &self,
        key: &SccpBridgeKeyFileV1,
        advance: AdvanceSccpLightClientV1,
    ) -> eyre::Result<()>;
}

/// The node's [`KeeperHost`]: committed state, the local queue and the attestor's key
/// directory, which the keeper only reads.
pub(super) struct NodeHost {
    /// Committed state.
    pub(super) state: Arc<State>,
    /// Local transaction queue.
    pub(super) queue: Arc<Queue>,
    /// The attestor's bridge-key directory.
    pub(super) store: KeyStore,
    /// This node's peer.
    pub(super) me: PeerId,
}

impl KeeperHost for NodeHost {
    fn local(&self, network: SccpNetworkV1) -> eyre::Result<Local> {
        // The directory is read before the view is taken. Refused files are the attestor's to
        // report.
        let (keys, _refused) = self.store.load().wrap_err("list SCCP bridge keys")?;
        let (address, light_client, aged) = {
            let view = self.state.view();
            let world = view.world();
            if view.height() == 0 || store::parameters::get(world).is_none() {
                return Ok(Local::default());
            }
            let key_state = store::bridge_keys::get(world, &self.me)
                .cloned()
                .unwrap_or_default();
            let address = live_key_address(&keys, &self.me, &key_state, |address| {
                store::bridge_key_owners::get(world, address).cloned()
            });
            let light_client = store::light_clients::get(world, &network).copied();
            let aged = light_client.is_some()
                && iroha_sccp::light_client::is_aged(
                    &WorldLightClientView(world),
                    network,
                    view.query_ledger_time_ms(),
                ) == Ok(true);
            (address, light_client, aged)
        };
        // The view is dropped: everything below is owned.
        let key = address.and_then(|address| {
            keys.into_iter()
                .find(|key| key.address().ok() == Some(address))
        });
        Ok(Local {
            key,
            light_client,
            aged,
        })
    }

    fn submit(
        &self,
        key: &SccpBridgeKeyFileV1,
        advance: AdvanceSccpLightClientV1,
    ) -> eyre::Result<()> {
        super::submit_transaction(&self.state, &self.queue, key, InstructionBox::from(advance))
    }
}

/// The address of the key the keeper submits from: the lowest local key address that this
/// peer owns in `owner_of` and that is its active or pending key.
fn live_key_address(
    keys: &[SccpBridgeKeyFileV1],
    me: &PeerId,
    key_state: &SccpBridgeKeyStateV1,
    owner_of: impl Fn(&[u8; 20]) -> Option<PeerId>,
) -> Option<[u8; 20]> {
    let live = attestor_plan::live_addresses(key_state);
    attestor_plan::classify(keys, me, owner_of)
        .owned
        .into_iter()
        .find(|address| live.contains(address))
}

/// What a lane does with one reading of local state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    /// No live bridge key, SCCP absent, or no installed light client: nothing to do.
    Idle,
    /// The light client cannot be advanced until a Parliament `InitializeLightClient`
    /// re-seeds it, so no RPC is spent on it.
    Recovery(Recovery),
    /// The head is newer than the staleness bound.
    Fresh,
    /// The head is stale: build and submit an advance.
    Advance,
}

/// Why a light client needs Parliament recovery (§4.13.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Recovery {
    /// The light client is frozen.
    Frozen,
    /// Its newest signing set is beyond the weak-subjectivity bound.
    Aged,
}

/// Decide one poll from `local` at wall-clock time `now_ms`.
fn decide(config: &SccpLightClientKeeper, local: &Local, now_ms: u64) -> Step {
    let Some(light_client) = &local.light_client else {
        return Step::Idle;
    };
    if local.key.is_none() {
        return Step::Idle;
    }
    if light_client.is_frozen() {
        return Step::Recovery(Recovery::Frozen);
    }
    if local.aged {
        return Step::Recovery(Recovery::Aged);
    }
    if is_stale(config, light_client, now_ms) {
        Step::Advance
    } else {
        Step::Fresh
    }
}

/// One lane's schedule.
///
/// After a poll that needed no RPC or submitted an advance, the next poll comes
/// `poll_interval` later plus up to a quarter of that as jitter. Each consecutive failed poll
/// doubles the base wait, up to `2^MAX_BACKOFF_DOUBLINGS × poll_interval`. Jitter is a fixed
/// function of the seed and the number of draws, so tests are deterministic and lanes and
/// validators with different seeds drift apart.
#[derive(Debug, Clone)]
pub(super) struct Cadence {
    interval: Duration,
    seed: u64,
    failures: u32,
    draws: u64,
}

impl Cadence {
    /// A schedule of `interval` with jitter seeded by `seed`.
    pub(super) fn new(interval: Duration, seed: u64) -> Self {
        Self {
            interval,
            seed,
            failures: 0,
            draws: 0,
        }
    }

    /// Wait before the first poll: up to a quarter of the interval, so lanes and validators do
    /// not start in lockstep.
    pub(super) fn first(&mut self) -> Duration {
        self.jitter(self.interval / 4)
    }

    /// Wait after a poll that needed no RPC or submitted an advance; resets the backoff.
    pub(super) fn after_success(&mut self) -> Duration {
        self.failures = 0;
        self.next()
    }

    /// Wait after a failed or overrun poll; doubles the base wait up to the cap.
    pub(super) fn after_failure(&mut self) -> Duration {
        self.failures = self.failures.saturating_add(1);
        self.next()
    }

    /// Consecutive failed polls.
    #[cfg(test)]
    pub(super) fn failures(&self) -> u32 {
        self.failures
    }

    /// The base wait of the current failure count.
    fn base(&self) -> Duration {
        self.interval
            .saturating_mul(1_u32 << self.failures.min(MAX_BACKOFF_DOUBLINGS))
    }

    fn next(&mut self) -> Duration {
        let base = self.base();
        base.saturating_add(self.jitter(base / 4))
    }

    /// A draw in `[0, span]`.
    fn jitter(&mut self, span: Duration) -> Duration {
        self.draws = self.draws.wrapping_add(1);
        let span = u64::try_from(span.as_nanos()).unwrap_or(u64::MAX);
        if span == 0 {
            return Duration::ZERO;
        }
        let draw = splitmix64(self.seed ^ self.draws.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        Duration::from_nanos(draw % span.saturating_add(1))
    }
}

/// `SplitMix64` finalizer: a fixed, platform-independent mixing function.
fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The jitter seed of `network`'s lane on a node with `seed`.
fn lane_seed(seed: u64, network: SccpNetworkV1) -> u64 {
    let mut bytes = seed.to_le_bytes().to_vec();
    bytes.extend_from_slice(network.profile_key().as_bytes());
    endpoint_seed(&bytes)
}

/// A build result as it comes back from its blocking worker.
type BuildJob = JoinHandle<thread::Result<Result<SccpLcAdvanceBytesV1, BuildError>>>;

/// One network: its advance source, poll budget and schedule.
pub(super) struct Lane {
    network: SccpNetworkV1,
    source: Arc<dyn AdvanceSource>,
    /// Shared with the source's transports, which stop at its deadline.
    budget: PollBudget,
    cadence: Cadence,
    /// A build that overran its poll and still runs on its blocking worker.
    overrun: Option<BuildJob>,
    /// State hash of the light client last reported as needing recovery.
    reported_recovery: Option<[u8; 32]>,
}

impl Lane {
    /// A lane of `network` building with `source`, whose transports carry `budget`.
    pub(super) fn new(
        network: SccpNetworkV1,
        source: Arc<dyn AdvanceSource>,
        budget: PollBudget,
        cadence: Cadence,
    ) -> Self {
        Self {
            network,
            source,
            budget,
            cadence,
            overrun: None,
            reported_recovery: None,
        }
    }

    /// The lane's network.
    #[cfg(test)]
    pub(super) fn network(&self) -> SccpNetworkV1 {
        self.network
    }

    /// Poll once; returns whether the poll succeeded (needed no RPC or submitted an advance).
    async fn poll<H: KeeperHost>(
        &mut self,
        host: &Arc<H>,
        config: &Arc<SccpLightClientKeeper>,
    ) -> bool {
        let network = self.network;
        let profile = network.profile_key();
        if let Some(job) = &self.overrun {
            if !job.is_finished() {
                iroha_logger::warn!(
                    network = profile,
                    "SCCP keeper: the previous poll still runs past its budget; skipping this poll"
                );
                return false;
            }
            self.overrun = None;
        }
        let now_ms = wall_clock_ms();
        let read = {
            let host = Arc::clone(host);
            join_recoverable(spawn_blocking_recoverable(move || host.local(network))).await
        };
        let local = match read {
            Ok(Ok(local)) => local,
            Ok(Err(error)) => {
                iroha_logger::warn!(
                    ?error,
                    network = profile,
                    "SCCP keeper: reading local state failed"
                );
                return false;
            }
            Err(_panic) => {
                iroha_logger::error!(
                    network = profile,
                    "SCCP keeper: reading local state panicked"
                );
                return false;
            }
        };
        match decide(config, &local, now_ms) {
            Step::Idle | Step::Fresh => {
                self.reported_recovery = None;
                return true;
            }
            Step::Recovery(reason) => {
                let state_hash = local
                    .light_client
                    .map(|light_client| light_client.state_hash);
                if self.reported_recovery != state_hash {
                    self.reported_recovery = state_hash;
                    iroha_logger::warn!(
                        network = profile,
                        ?reason,
                        health = "sccp_keeper_needs_recovery",
                        "SCCP keeper: the light client is frozen or aged out of its weak-subjectivity \
                         window and is not advanced until a Parliament InitializeLightClient re-seeds it"
                    );
                }
                return true;
            }
            Step::Advance => {}
        }
        let (Some(key), Some(light_client)) = (local.key, local.light_client) else {
            return true;
        };
        let Some(advance) = self
            .build(
                &light_client,
                advance_budget(config, &light_client),
                config.poll_budget,
            )
            .await
        else {
            return false;
        };
        if !fits_advance_bound(config, network, &advance) {
            return false;
        }
        let instruction = AdvanceSccpLightClientV1 {
            network,
            expected_state_hash: Some(light_client.state_hash),
            advance,
        };
        let submitted = {
            let host = Arc::clone(host);
            join_recoverable(spawn_blocking_recoverable(move || {
                host.submit(&key, instruction)
            }))
            .await
        };
        match submitted {
            Ok(Ok(())) => true,
            Ok(Err(error)) => {
                iroha_logger::warn!(
                    ?error,
                    network = profile,
                    "SCCP keeper: submitting an advance failed"
                );
                false
            }
            Err(_panic) => {
                iroha_logger::error!(
                    network = profile,
                    "SCCP keeper: submitting an advance panicked"
                );
                false
            }
        }
    }

    /// Build an advance of `light_client`, stepped to `advance_budget`, on a blocking worker
    /// under the lane's poll budget.
    ///
    /// The worker owns only the source and plain values, never a state view. A build that has
    /// not returned after 5/4 × `poll_budget` is kept as the lane's overrun and counts as
    /// failed. A build that fails on the data it was served ([`blames_served_data`]) rotates
    /// the source's endpoints.
    async fn build(
        &mut self,
        light_client: &SccpLightClientV1,
        advance_budget: AdvanceBudgetV1,
        poll_budget: Duration,
    ) -> Option<SccpLcAdvanceBytesV1> {
        let profile = self.network.profile_key();
        let source = Arc::clone(&self.source);
        let budget = self.budget.clone();
        let latest = light_client.head.latest_set_id;
        let mut job: BuildJob = spawn_blocking_recoverable(move || {
            let _poll = budget.start(poll_budget);
            source.advance(latest, advance_budget)
        });
        let overdue = poll_budget.saturating_add(poll_budget / 4);
        let joined = match tokio::time::timeout(overdue, &mut job).await {
            Ok(joined) => joined,
            Err(_elapsed) => {
                iroha_logger::warn!(
                    network = profile,
                    ?poll_budget,
                    "SCCP keeper: building an advance overran its poll budget"
                );
                self.overrun = Some(job);
                return None;
            }
        };
        match recover_joined(joined) {
            Ok(Ok(advance)) => Some(advance),
            Ok(Err(error)) => {
                iroha_logger::warn!(
                    %error,
                    network = profile,
                    "SCCP keeper: building an advance failed"
                );
                if blames_served_data(&error) {
                    self.source.rotate_endpoints();
                }
                None
            }
            Err(_panic) => {
                iroha_logger::error!(
                    network = profile,
                    "SCCP keeper: building an advance panicked"
                );
                None
            }
        }
    }
}

/// Build the lanes on a blocking worker of the current Tokio runtime.
///
/// The HTTP transports own blocking `reqwest` clients. Building one starts the client's
/// internal runtime thread and blocks until it runs, which must never happen on an async
/// worker thread: debug builds panic there and release builds block the worker. A panic while
/// building leaves the keeper without lanes, because the node never aborts over SCCP.
async fn build(config: Arc<SccpLightClientKeeper>, seed: u64) -> Vec<Lane> {
    match join_recoverable(spawn_blocking_recoverable(move || {
        build_lanes(&config, seed)
    }))
    .await
    {
        Ok(lanes) => lanes,
        Err(_panic) => {
            iroha_logger::error!(
                "SCCP keeper: building the endpoint clients panicked; the keeper stays idle"
            );
            Vec::new()
        }
    }
}

/// One lane per network whose endpoints are usable. `seed` (derived from the peer id) seeds
/// the starting endpoints, the failover jitter and the lanes' schedules.
///
/// Blocks while the HTTP clients start, so async callers use [`build`].
fn build_lanes(config: &SccpLightClientKeeper, seed: u64) -> Vec<Lane> {
    let mut lanes = Vec::new();
    for network in NETWORKS {
        let budget = PollBudget::new();
        match build_source(config, network, seed, &budget) {
            Ok(source) => lanes.push(Lane::new(
                network,
                source,
                budget,
                Cadence::new(config.poll_interval, lane_seed(seed, network)),
            )),
            Err(error) => iroha_logger::warn!(
                %error,
                network = network.profile_key(),
                "SCCP keeper: the endpoints are unusable; this network is not kept"
            ),
        }
    }
    lanes
}

/// The advance builder of `network` over the keeper's endpoints, every transport bounded by
/// `budget`.
fn build_source(
    config: &SccpLightClientKeeper,
    network: SccpNetworkV1,
    seed: u64,
    budget: &PollBudget,
) -> Result<Arc<dyn AdvanceSource>, String> {
    let http = |kind| {
        HttpTransport::from_keeper_config(config, kind, seed)
            .map(|transport| transport.with_budget(budget.clone()))
            .map_err(|error| error.to_string())
    };
    Ok(match network {
        SccpNetworkV1::EthereumMainnet => Arc::new(EthereumBuilder::new(
            BeaconClient::new(http(HttpEndpointKind::EthereumBeacon)?),
            EvmClient::new(http(HttpEndpointKind::EthereumExecution)?),
        )),
        SccpNetworkV1::BscMainnet => Arc::new(BscBuilder::new(EvmClient::new(http(
            HttpEndpointKind::Bsc,
        )?))),
        SccpNetworkV1::TronMainnet => Arc::new(TronBuilder::new(TronClient::new(http(
            HttpEndpointKind::Tron,
        )?))),
        SccpNetworkV1::TonMainnet => Arc::new(TonBuilder::new(
            LiteClient::from_keeper_config(config, seed)
                .map_err(|error| error.to_string())?
                .with_budget(budget.clone()),
        )),
        SccpNetworkV1::SoraTaira => {
            return Err("Taira has no inbound light client".to_owned());
        }
    })
}

/// The budget the builders step an advance of `light_client` to: its
/// `max_updates_per_advance`, and the lower of its `max_advance_bytes` and the keeper's.
fn advance_budget(
    config: &SccpLightClientKeeper,
    light_client: &SccpLightClientV1,
) -> AdvanceBudgetV1 {
    AdvanceBudgetV1::for_params(&light_client.params, config.max_advance_bytes.get())
}

/// Whether a built `advance` fits the configured `max_advance_bytes`. The builders step their
/// advances to this budget; an advance that still exceeds it is dropped with a warning instead
/// of being submitted.
fn fits_advance_bound(
    config: &SccpLightClientKeeper,
    network: SccpNetworkV1,
    advance: &SccpLcAdvanceBytesV1,
) -> bool {
    let max_advance_bytes = config.max_advance_bytes.get();
    let fits = advance.len() <= max_advance_bytes;
    if !fits {
        iroha_logger::warn!(
            network = network.profile_key(),
            advance_bytes = advance.len(),
            max_advance_bytes,
            "SCCP keeper: dropping an advance larger than max_advance_bytes"
        );
    }
    fits
}

/// Whether `light_client` is unfrozen and its head is older than the keeper's staleness bound.
fn is_stale(config: &SccpLightClientKeeper, light_client: &SccpLightClientV1, now_ms: u64) -> bool {
    let after_ms = if light_client.params.ws_bound_ms == 0 {
        TON_ADVANCE_AFTER_MS
    } else {
        let after = config.advance_after_for(light_client.params.ws_bound_ms);
        u64::try_from(after.as_millis()).unwrap_or(u64::MAX)
    };
    !light_client.is_frozen()
        && now_ms.saturating_sub(light_client.head.last_progress_taira_ms) >= after_ms
}

/// Test doubles of a keeper's node and sources.
#[cfg(test)]
pub(super) mod fakes {
    use std::{
        collections::BTreeMap,
        sync::atomic::{AtomicUsize, Ordering},
        time::Instant,
    };

    use iroha_data_model::sccp::light_client::{
        SccpLcHeadV1, SccpLcPointV1, SccpLightClientParamsV1,
    };
    use parking_lot::{Condvar, Mutex};

    use super::*;

    /// A light client of `network` whose head last progressed at Taira time
    /// `last_progress_taira_ms`.
    pub(in crate::sccp_attestor) fn light_client(
        network: SccpNetworkV1,
        last_progress_taira_ms: u64,
    ) -> SccpLightClientV1 {
        SccpLightClientV1 {
            params: SccpLightClientParamsV1::defaults_for(network).expect("external network"),
            head: SccpLcHeadV1 {
                latest_set_id: 1,
                latest_finalized: SccpLcPointV1 {
                    source_height: 1,
                    block_hash: [1; 32],
                    source_time_ms: 1,
                },
                last_progress_taira_ms,
            },
            frozen: None,
            state_hash: [0; 32],
        }
    }

    /// `source` as a lane's advance source.
    pub(in crate::sccp_attestor) fn shared<S: AdvanceSource>(
        source: &Arc<S>,
    ) -> Arc<dyn AdvanceSource> {
        let source: Arc<S> = Arc::clone(source);
        source
    }

    /// A key file standing in for the node's live bridge key.
    pub(in crate::sccp_attestor) fn key() -> SccpBridgeKeyFileV1 {
        SccpBridgeKeyFileV1::new([7; 32], 0).expect("valid secret")
    }

    /// A keeper configuration with millisecond cadences.
    pub(in crate::sccp_attestor) fn fast_config(
        poll_interval_ms: u64,
        poll_budget_ms: u64,
    ) -> SccpLightClientKeeper {
        SccpLightClientKeeper {
            poll_interval: Duration::from_millis(poll_interval_ms),
            poll_budget: Duration::from_millis(poll_budget_ms),
            ..SccpLightClientKeeper::default()
        }
    }

    /// A node whose installed light clients are all stale (head at Taira time 0) and that holds
    /// a live bridge key; it records reads and submissions per network.
    #[derive(Default)]
    pub(in crate::sccp_attestor) struct FakeHost {
        aged: Mutex<BTreeMap<&'static str, bool>>,
        reads: Mutex<BTreeMap<&'static str, usize>>,
        submissions: Mutex<Vec<AdvanceSccpLightClientV1>>,
    }

    impl FakeHost {
        /// Marks the light client of `network` as aged out of its window.
        pub(in crate::sccp_attestor) fn set_aged(&self, network: SccpNetworkV1, aged: bool) {
            self.aged.lock().insert(network.profile_key(), aged);
        }

        /// Local-state reads of `network` so far.
        pub(in crate::sccp_attestor) fn reads(&self, network: SccpNetworkV1) -> usize {
            self.reads
                .lock()
                .get(network.profile_key())
                .copied()
                .unwrap_or(0)
        }

        /// Advances submitted for `network` so far.
        pub(in crate::sccp_attestor) fn submissions(&self, network: SccpNetworkV1) -> usize {
            self.submissions
                .lock()
                .iter()
                .filter(|advance| advance.network == network)
                .count()
        }
    }

    impl KeeperHost for FakeHost {
        fn local(&self, network: SccpNetworkV1) -> eyre::Result<Local> {
            *self.reads.lock().entry(network.profile_key()).or_default() += 1;
            Ok(Local {
                key: Some(key()),
                light_client: Some(light_client(network, 0)),
                aged: self
                    .aged
                    .lock()
                    .get(network.profile_key())
                    .copied()
                    .unwrap_or(false),
            })
        }

        fn submit(
            &self,
            _key: &SccpBridgeKeyFileV1,
            advance: AdvanceSccpLightClientV1,
        ) -> eyre::Result<()> {
            self.submissions.lock().push(advance);
            Ok(())
        }
    }

    /// A source whose every build blocks until it is opened, ignoring the poll budget like a
    /// hung endpoint would.
    #[derive(Default)]
    pub(in crate::sccp_attestor) struct GatedSource {
        open: Mutex<bool>,
        opened: Condvar,
        entered: AtomicUsize,
    }

    impl GatedSource {
        /// Lets every blocked and later build return.
        pub(in crate::sccp_attestor) fn open(&self) {
            *self.open.lock() = true;
            self.opened.notify_all();
        }

        /// Builds started so far.
        pub(in crate::sccp_attestor) fn entered(&self) -> usize {
            self.entered.load(Ordering::SeqCst)
        }
    }

    impl AdvanceSource for GatedSource {
        fn advance(
            &self,
            _latest_set_id: u64,
            _budget: AdvanceBudgetV1,
        ) -> Result<SccpLcAdvanceBytesV1, BuildError> {
            self.entered.fetch_add(1, Ordering::SeqCst);
            let mut open = self.open.lock();
            while !*open {
                self.opened.wait(&mut open);
            }
            Ok(SccpLcAdvanceBytesV1::new(vec![1]).expect("advance bytes"))
        }

        fn rotate_endpoints(&self) {}
    }

    /// A source that answers at once, successfully or not, and records when it was called, the
    /// budget of each call and how often its endpoints were rotated.
    pub(in crate::sccp_attestor) struct ScriptedSource {
        error: Option<fn() -> BuildError>,
        calls: Mutex<Vec<Instant>>,
        budgets: Mutex<Vec<AdvanceBudgetV1>>,
        rotations: AtomicUsize,
    }

    impl ScriptedSource {
        /// A source whose builds all succeed (`fail == false`) or all fail as unavailable.
        pub(in crate::sccp_attestor) fn new(fail: bool) -> Self {
            let unavailable: fn() -> BuildError =
                || BuildError::Unavailable("scripted failure".to_owned());
            Self::scripted(fail.then_some(unavailable))
        }

        /// A source whose builds all fail with `error()`.
        pub(in crate::sccp_attestor) fn failing_with(error: fn() -> BuildError) -> Self {
            Self::scripted(Some(error))
        }

        fn scripted(error: Option<fn() -> BuildError>) -> Self {
            Self {
                error,
                calls: Mutex::new(Vec::new()),
                budgets: Mutex::new(Vec::new()),
                rotations: AtomicUsize::new(0),
            }
        }

        /// When each build was called.
        pub(in crate::sccp_attestor) fn calls(&self) -> Vec<Instant> {
            self.calls.lock().clone()
        }

        /// The advance budget of each build.
        pub(in crate::sccp_attestor) fn budgets(&self) -> Vec<AdvanceBudgetV1> {
            self.budgets.lock().clone()
        }

        /// How often the endpoints were rotated.
        pub(in crate::sccp_attestor) fn rotations(&self) -> usize {
            self.rotations.load(Ordering::SeqCst)
        }
    }

    impl AdvanceSource for ScriptedSource {
        fn advance(
            &self,
            _latest_set_id: u64,
            budget: AdvanceBudgetV1,
        ) -> Result<SccpLcAdvanceBytesV1, BuildError> {
            self.calls.lock().push(Instant::now());
            self.budgets.lock().push(budget);
            self.error.map_or_else(
                || Ok(SccpLcAdvanceBytesV1::new(vec![1]).expect("advance bytes")),
                |error| Err(error()),
            )
        }

        fn rotate_endpoints(&self) {
            self.rotations.fetch_add(1, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroUsize, time::Instant};

    use iroha_crypto::KeyPair;
    use iroha_data_model::sccp::{
        keys::SccpBridgeKeyV1,
        light_client::{SccpLcFreezeReasonV1, SccpLcParliamentFreezeV1},
    };

    use super::{fakes::*, *};

    #[test]
    fn staleness_follows_the_configured_or_derived_bound() {
        let network = SccpNetworkV1::EthereumMainnet;
        let mut config = SccpLightClientKeeper {
            advance_after: Some(Duration::from_secs(10)),
            ..SccpLightClientKeeper::default()
        };
        assert!(!is_stale(&config, &light_client(network, 5_000), 14_000));
        assert!(is_stale(&config, &light_client(network, 5_000), 15_000));
        config.advance_after = None;
        let derived = light_client(network, 0).params.ws_bound_ms / 4;
        assert!(!is_stale(&config, &light_client(network, 0), derived - 1));
        assert!(is_stale(&config, &light_client(network, 0), derived));
    }

    #[test]
    fn ton_light_clients_are_polled_hourly() {
        let config = SccpLightClientKeeper::default();
        let ton = light_client(SccpNetworkV1::TonMainnet, 0);
        assert!(!is_stale(&config, &ton, TON_ADVANCE_AFTER_MS - 1));
        assert!(is_stale(&config, &ton, TON_ADVANCE_AFTER_MS));
    }

    #[test]
    fn advances_above_max_advance_bytes_are_dropped() {
        let config = SccpLightClientKeeper {
            max_advance_bytes: NonZeroUsize::new(4).expect("nonzero bound"),
            ..SccpLightClientKeeper::default()
        };
        let advance = |len: usize| SccpLcAdvanceBytesV1::new(vec![0; len]).expect("advance bytes");
        let network = SccpNetworkV1::EthereumMainnet;
        assert!(fits_advance_bound(&config, network, &advance(1)));
        assert!(fits_advance_bound(&config, network, &advance(4)));
        assert!(!fits_advance_bound(&config, network, &advance(5)));
        let default = SccpLightClientKeeper::default();
        let bound = default.max_advance_bytes.get();
        assert!(fits_advance_bound(&default, network, &advance(bound)));
        assert!(!fits_advance_bound(&default, network, &advance(bound + 1)));
    }

    #[test]
    fn advances_are_stepped_to_the_keeper_byte_bound() {
        let network = SccpNetworkV1::EthereumMainnet;
        let mut config = SccpLightClientKeeper::default();
        let budget = advance_budget(&config, &light_client(network, 0));
        assert_eq!(budget.max_items, 16);
        assert_eq!(budget.max_bytes, 262_144);
        config.max_advance_bytes = NonZeroUsize::new(usize::MAX).expect("nonzero bound");
        let mut wide = light_client(network, 0);
        wide.params.max_advance_bytes = 300_000;
        wide.params.max_updates_per_advance = 4;
        let budget = advance_budget(&config, &wide);
        assert_eq!(budget.max_items, 4);
        assert_eq!(budget.max_bytes, 300_000);
    }

    #[test]
    fn decisions_skip_idle_fresh_and_unrecoverable_light_clients() {
        let config = SccpLightClientKeeper::default();
        let network = SccpNetworkV1::BscMainnet;
        let bound = config.advance_after_for(light_client(network, 0).params.ws_bound_ms);
        let stale_at = u64::try_from(bound.as_millis()).expect("bound fits");
        let local = |with_key: bool, light_client: Option<SccpLightClientV1>, aged: bool| Local {
            key: with_key.then(key),
            light_client,
            aged,
        };
        let stale = Some(light_client(network, 0));
        assert_eq!(
            decide(&config, &local(true, None, false), stale_at),
            Step::Idle
        );
        assert_eq!(
            decide(&config, &local(false, stale, false), stale_at),
            Step::Idle
        );
        assert_eq!(
            decide(&config, &local(true, stale, false), stale_at - 1),
            Step::Fresh
        );
        assert_eq!(
            decide(&config, &local(true, stale, false), stale_at),
            Step::Advance
        );
        assert_eq!(
            decide(&config, &local(true, stale, true), stale_at),
            Step::Recovery(Recovery::Aged)
        );
        let mut frozen = light_client(network, 0);
        frozen.frozen = Some(SccpLcFreezeReasonV1::Parliament(SccpLcParliamentFreezeV1 {
            proposal_id: [3; 32],
        }));
        assert_eq!(
            decide(&config, &local(true, Some(frozen), false), stale_at),
            Step::Recovery(Recovery::Frozen)
        );
    }

    #[test]
    fn the_live_key_is_an_owned_active_or_pending_key() {
        let me = PeerId::new(KeyPair::random().public_key().clone());
        let other = PeerId::new(KeyPair::random().public_key().clone());
        let keys: Vec<_> = (1..=3_u8)
            .map(|seed| SccpBridgeKeyFileV1::new([seed; 32], 0).expect("valid secret"))
            .collect();
        let address = |index: usize| keys[index].address().expect("address");
        let entry = |index: usize| SccpBridgeKeyV1 {
            public_key: keys[index].public_key().expect("public key"),
            address: address(index),
            activation_epoch: 1,
            registered_at_height: 1,
            faulted: false,
        };
        let state = SccpBridgeKeyStateV1 {
            active: Some(entry(0)),
            pending: Some(entry(1)),
            ..SccpBridgeKeyStateV1::default()
        };
        let owners = |owned: Vec<usize>| {
            let owned: Vec<[u8; 20]> = owned.into_iter().map(address).collect();
            let (me, other) = (me.clone(), other.clone());
            move |candidate: &[u8; 20]| {
                if owned.contains(candidate) {
                    Some(me.clone())
                } else {
                    Some(other.clone())
                }
            }
        };
        let both = [address(0), address(1)];
        let lowest = *both.iter().min().expect("two addresses");
        assert_eq!(
            live_key_address(&keys, &me, &state, owners(vec![0, 1, 2])),
            Some(lowest)
        );
        assert_eq!(
            live_key_address(&keys, &me, &state, owners(vec![1])),
            Some(address(1))
        );
        assert_eq!(live_key_address(&keys, &me, &state, owners(vec![2])), None);
        assert_eq!(
            live_key_address(
                &keys,
                &me,
                &SccpBridgeKeyStateV1::default(),
                owners(vec![0, 1])
            ),
            None
        );
    }

    #[test]
    fn cadence_backs_off_per_failure_with_bounded_jitter() {
        let interval = Duration::from_secs(60);
        let mut cadence = Cadence::new(interval, 42);
        let first = cadence.first();
        assert!(first <= interval / 4, "{first:?}");
        let within = |wait: Duration, base: Duration| wait >= base && wait <= base + base / 4;
        let success = cadence.after_success();
        assert!(within(success, interval), "{success:?}");
        for doublings in 1..=MAX_BACKOFF_DOUBLINGS {
            let wait = cadence.after_failure();
            assert_eq!(cadence.failures(), doublings);
            assert!(
                within(wait, interval * (1 << doublings)),
                "{doublings}: {wait:?}"
            );
        }
        let cap = interval * (1 << MAX_BACKOFF_DOUBLINGS);
        for _ in 0..5 {
            let wait = cadence.after_failure();
            assert!(within(wait, cap), "capped: {wait:?}");
        }
        let reset = cadence.after_success();
        assert_eq!(cadence.failures(), 0);
        assert!(within(reset, interval), "{reset:?}");
    }

    #[test]
    fn cadence_jitter_is_deterministic_per_seed() {
        let interval = Duration::from_secs(60);
        let waits = |seed: u64| {
            let mut cadence = Cadence::new(interval, seed);
            let mut waits = vec![cadence.first()];
            waits.extend((0..8).map(|_| cadence.after_success()));
            waits
        };
        assert_eq!(waits(7), waits(7));
        assert_ne!(waits(7), waits(8));
        let spread: std::collections::BTreeSet<_> = waits(7).into_iter().skip(1).collect();
        assert!(spread.len() > 1, "jitter varies between polls");
        let mut zero = Cadence::new(Duration::ZERO, 1);
        assert_eq!(zero.first(), Duration::ZERO);
        assert_eq!(zero.after_failure(), Duration::ZERO);
    }

    #[test]
    fn lanes_of_one_node_get_distinct_seeds() {
        let seeds: std::collections::BTreeSet<_> = NETWORKS
            .iter()
            .map(|network| lane_seed(5, *network))
            .collect();
        assert_eq!(seeds.len(), NETWORKS.len());
        assert_eq!(
            lane_seed(5, SccpNetworkV1::TonMainnet),
            lane_seed(5, SccpNetworkV1::TonMainnet)
        );
        assert_ne!(
            lane_seed(5, SccpNetworkV1::TonMainnet),
            lane_seed(6, SccpNetworkV1::TonMainnet)
        );
    }

    #[test]
    fn a_disabled_keeper_starts_no_task() {
        let config = SccpLightClientKeeper {
            enabled: false,
            ..SccpLightClientKeeper::default()
        };
        let host = Arc::new(FakeHost::default());
        assert!(start(host, config, 0, ShutdownSignal::new()).is_none());
    }

    /// Every chain of the default configuration has a lane, in [`NETWORKS`] order.
    fn assert_default_lanes(lanes: &[Lane]) {
        let networks: Vec<_> = lanes.iter().map(Lane::network).collect();
        assert_eq!(networks, NETWORKS);
    }

    #[tokio::test]
    async fn the_default_keeper_builds_inside_a_current_thread_runtime() {
        let lanes = build(Arc::new(SccpLightClientKeeper::default()), 1).await;
        assert_default_lanes(&lanes);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_default_keeper_builds_inside_a_multi_thread_runtime() {
        let lanes = build(Arc::new(SccpLightClientKeeper::default()), 1).await;
        assert_default_lanes(&lanes);
        // The lanes, like the keeper task that owns them, are dropped on an async worker.
        drop(lanes);
    }

    #[tokio::test]
    async fn an_enabled_keeper_runs_until_shutdown() {
        let shutdown = ShutdownSignal::new();
        let host = Arc::new(FakeHost::default());
        let config = fast_config(3_600_000, 1_000);
        let task = tokio::spawn(run(Vec::new(), host, Arc::new(config), shutdown.clone()));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !task.is_finished(),
            "a keeper without lanes waits for shutdown"
        );
        shutdown.send();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the keeper stops on shutdown")
            .expect("the keeper does not panic");
    }

    /// Waits (asynchronously, so a current-thread runtime keeps running) until `ready`.
    async fn eventually(what: &str, ready: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !ready() {
            assert!(Instant::now() < deadline, "timed out waiting until {what}");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn networks_are_scheduled_independently_with_backoff() {
        let shutdown = ShutdownSignal::new();
        let host = Arc::new(FakeHost::default());
        host.set_aged(SccpNetworkV1::TonMainnet, true);
        let config = Arc::new(fast_config(25, 1_000));
        let working = Arc::new(ScriptedSource::new(false));
        let failing = Arc::new(ScriptedSource::new(true));
        let hung = Arc::new(GatedSource::default());
        let aged = Arc::new(ScriptedSource::new(false));
        let lane = |network, source: Arc<dyn AdvanceSource>| {
            Lane::new(
                network,
                source,
                PollBudget::new(),
                Cadence::new(config.poll_interval, lane_seed(9, network)),
            )
        };
        let lanes = vec![
            lane(SccpNetworkV1::BscMainnet, shared(&working)),
            lane(SccpNetworkV1::TronMainnet, shared(&failing)),
            lane(SccpNetworkV1::EthereumMainnet, shared(&hung)),
            lane(SccpNetworkV1::TonMainnet, shared(&aged)),
        ];
        let keeper = tokio::spawn(run(
            lanes,
            Arc::clone(&host),
            Arc::clone(&config),
            shutdown.clone(),
        ));
        eventually("the working network was advanced ten times", || {
            host.submissions(SccpNetworkV1::BscMainnet) >= 10
        })
        .await;
        // The hung Ethereum build never delays the other networks.
        assert_eq!(
            hung.entered(),
            1,
            "one hung build, no second poll beside it"
        );
        assert_eq!(host.submissions(SccpNetworkV1::EthereumMainnet), 0);
        // The aged TON light client is read but no RPC is spent on it.
        assert!(host.reads(SccpNetworkV1::TonMainnet) >= 2);
        assert!(aged.calls().is_empty());
        assert_eq!(host.submissions(SccpNetworkV1::TonMainnet), 0);
        // The failing TRON network backs off: its waits grow while BSC keeps its cadence.
        let tron = failing.calls();
        let bsc = working.calls();
        assert!(
            tron.len() < bsc.len(),
            "TRON {} vs BSC {}",
            tron.len(),
            bsc.len()
        );
        assert_eq!(host.submissions(SccpNetworkV1::TronMainnet), 0);
        eventually("TRON failed three times", || failing.calls().len() >= 3).await;
        assert!(
            failing.rotations() >= 2,
            "each failed TRON build moved to other endpoints"
        );
        assert_eq!(working.rotations(), 0);
        let tron = failing.calls();
        let first_gap = tron[1].duration_since(tron[0]);
        let second_gap = tron[2].duration_since(tron[1]);
        assert!(
            first_gap >= config.poll_interval * 2,
            "the first retry waits twice the interval: {first_gap:?}"
        );
        assert!(
            second_gap >= config.poll_interval * 4,
            "the second retry waits four times the interval: {second_gap:?}"
        );
        // Re-initialization makes the aged light client advanceable again.
        host.set_aged(SccpNetworkV1::TonMainnet, false);
        eventually("TON was advanced after recovery", || {
            host.submissions(SccpNetworkV1::TonMainnet) >= 1
        })
        .await;
        hung.open();
        shutdown.send();
        tokio::time::timeout(Duration::from_secs(5), keeper)
            .await
            .expect("the keeper stops on shutdown")
            .expect("the keeper does not panic");
    }

    #[test]
    fn only_failures_on_served_data_blame_the_endpoints() {
        let rpc = BuildError::Rpc(iroha_sccp_rpc::RpcError::Timeout {
            endpoint: "https://rpc.example.org".to_owned(),
        });
        assert!(!blames_served_data(&rpc));
        for error in [
            BuildError::Json("an update is not an object".to_owned()),
            BuildError::Inconsistent("a header does not hash to its hash".to_owned()),
            BuildError::Unavailable("block 7 is not served".to_owned()),
        ] {
            assert!(blames_served_data(&error), "{error}");
        }
    }

    #[tokio::test]
    async fn builds_failing_on_served_data_rotate_the_lane_endpoints() {
        fn inconsistent() -> BuildError {
            BuildError::Inconsistent("a header does not hash to its hash".to_owned())
        }
        fn malformed() -> BuildError {
            BuildError::Json("an update is not an object".to_owned())
        }
        fn exhausted() -> BuildError {
            BuildError::Rpc(iroha_sccp_rpc::RpcError::Exhausted {
                failures: Vec::new(),
            })
        }
        let host = Arc::new(FakeHost::default());
        let config = Arc::new(fast_config(25, 1_000));
        let lane = |source: Arc<dyn AdvanceSource>| {
            Lane::new(
                SccpNetworkV1::BscMainnet,
                source,
                PollBudget::new(),
                Cadence::new(config.poll_interval, 1),
            )
        };
        let cases: [(fn() -> BuildError, usize); 3] =
            [(inconsistent, 1), (malformed, 1), (exhausted, 0)];
        for (error, rotations) in cases {
            let source = Arc::new(ScriptedSource::failing_with(error));
            let mut failing = lane(shared(&source));
            assert!(
                !failing.poll(&host, &config).await,
                "a failed build fails the poll"
            );
            assert_eq!(source.calls().len(), 1);
            assert_eq!(source.rotations(), rotations, "{}", error());
        }
        assert_eq!(host.submissions(SccpNetworkV1::BscMainnet), 0);
        let working = Arc::new(ScriptedSource::new(false));
        let mut advancing = lane(shared(&working));
        assert!(advancing.poll(&host, &config).await);
        assert_eq!(working.rotations(), 0);
        assert_eq!(host.submissions(SccpNetworkV1::BscMainnet), 1);
        assert_eq!(
            working.budgets(),
            vec![advance_budget(
                &config,
                &light_client(SccpNetworkV1::BscMainnet, 0)
            )],
            "the lane steps its advance to the light client's and the keeper's bounds"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_overrun_poll_fails_and_blocks_only_its_own_network() {
        let shutdown = ShutdownSignal::new();
        let host = Arc::new(FakeHost::default());
        let config = Arc::new(fast_config(20, 60));
        let hung = Arc::new(GatedSource::default());
        let lane = Lane::new(
            SccpNetworkV1::EthereumMainnet,
            shared(&hung),
            PollBudget::new(),
            Cadence::new(config.poll_interval, 3),
        );
        let keeper = tokio::spawn(run(
            vec![lane],
            Arc::clone(&host),
            Arc::clone(&config),
            shutdown.clone(),
        ));
        eventually("the build hung", || hung.entered() == 1).await;
        // Several overrun budgets later the hung build is still the only one.
        tokio::time::sleep(config.poll_budget * 5).await;
        assert_eq!(hung.entered(), 1);
        assert_eq!(host.submissions(SccpNetworkV1::EthereumMainnet), 0);
        // Once the endpoint answers, a later poll builds and submits again.
        hung.open();
        eventually("a later poll submitted", || {
            host.submissions(SccpNetworkV1::EthereumMainnet) >= 1
        })
        .await;
        assert!(hung.entered() >= 2);
        shutdown.send();
        tokio::time::timeout(Duration::from_secs(5), keeper)
            .await
            .expect("the keeper stops on shutdown")
            .expect("the keeper does not panic");
    }
}
