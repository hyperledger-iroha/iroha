//! In-node SCCP attestor with zero-touch bridge-key management (`specs/sccp.md` §4.2, §4.9).
//!
//! A validator installs, starts and leaves exactly as without SCCP. On every tick the attestor
//! reads committed (durably final) state and:
//!
//! 1. generates the node's secp256k1 bridge key on first start (and whenever the store holds
//!    neither the peer's active or pending key nor an unregistered key) into the owner-only key
//!    directory;
//! 2. registers the newest unregistered key with `SetSccpBridgeKeyV1`, fee-exempt from the
//!    key's own implicitly registered account, once per epoch until it is pending or active;
//! 3. signs every subject whose generation holds one of its keys and whose bit is unset,
//!    rotation subjects first, computing each statement from local state only;
//! 4. submits one `SubmitSccpAttestationsV1` per key per block and resubmits entries still
//!    unrecorded after `resubmit_after_blocks`;
//! 5. deletes a key once every generation holding it expired plus one day.
//!
//! On a graceful shutdown it signs and submits every pending subject and waits for them to be
//! recorded, at most `shutdown_grace`. The node never aborts because of SCCP: an unusable key
//! directory leaves the attestor inert (`sccp_attestor_unconfigured`).
//!
//! The inbound light-client keeper ([`keeper`], §4.13.4) is started next to the attestor as its
//! own supervised task. It shares only the key directory (which it reads) and the submission
//! path, so its public-RPC calls never run inside, or delay, an attestor tick.
//!
//! TODO(ws34): export the health states as telemetry gauges; they are logged today.

#[path = "sccp_attestor/keeper.rs"]
mod keeper;
#[path = "sccp_attestor/key_store.rs"]
mod key_store;
#[path = "sccp_attestor/plan.rs"]
mod plan;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use eyre::WrapErr as _;
use iroha_core::{
    executor::quote_nexus_fee_admission_draft,
    queue::Queue,
    smartcontracts::isi::sccp::{bridge_keys, store, subjects},
    state::{State, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
use iroha_data_model::{
    isi::{InstructionBox, sccp::SubmitSccpAttestationsV1},
    sccp::attestation::SccpAttestationSignatureV1,
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_model_base::peer::PeerId;
use iroha_sccp::v1::key_file::SccpBridgeKeyFileV1;
use iroha_sccp_rpc::endpoints::endpoint_seed;
use parking_lot::Mutex;

use self::{key_store::KeyStore, plan::Duty};

/// A key is deleted once every generation holding it expired this long ago (§4.9 step 6).
const KEY_RETENTION_AFTER_EXPIRY_MS: u64 = 86_400_000;
/// Tick interval of the attestor loop.
const TICK: Duration = Duration::from_millis(500);

/// Start the attestor and, next to it, the light-client keeper as separate supervised tasks.
///
/// Returns no task when the attestor is disabled or its key directory is unusable, and only the
/// attestor's when the keeper is disabled.
pub(crate) fn start(
    state: Arc<State>,
    queue: Arc<Queue>,
    peer_key_pair: KeyPair,
    config: iroha_config::parameters::actual::SccpAttestor,
    keeper_config: iroha_config::parameters::actual::SccpLightClientKeeper,
    shutdown_signal: ShutdownSignal,
) -> Vec<Child> {
    if !config.enabled {
        iroha_logger::info!("SCCP attestor disabled by configuration");
        return Vec::new();
    }
    let dir = config.key_dir_path();
    let store = match KeyStore::open(&dir) {
        Ok(store) => store,
        Err(error) => {
            iroha_logger::error!(
                %error,
                health = "sccp_attestor_unconfigured",
                "SCCP attestor is inert: the bridge-key directory is unusable"
            );
            return Vec::new();
        }
    };
    let me = PeerId::new(peer_key_pair.public_key().clone());
    // Seeds the keeper's starting endpoints and jitter per node, so validators sharing the
    // compiled endpoint lists spread over them.
    let seed = endpoint_seed(me.to_string().as_bytes());
    let keeper = keeper::start(
        Arc::new(keeper::NodeHost {
            state: Arc::clone(&state),
            queue: Arc::clone(&queue),
            store: store.clone(),
            me,
        }),
        keeper_config,
        seed,
        shutdown_signal.clone(),
    );
    let attestor = Arc::new(Attestor {
        state,
        queue,
        peer_key_pair,
        config,
        store,
        memory: Mutex::new(Memory::default()),
    });
    let task = tokio::task::spawn(async move {
        attestor.run(shutdown_signal).await;
    });
    let mut children = vec![Child::new(task, OnShutdown::Wait(Duration::from_secs(1)))];
    children.extend(keeper);
    children
}

/// Mutable bookkeeping between ticks.
#[derive(Debug, Default)]
struct Memory {
    /// Committed height through which subjects were scanned for new duties.
    scanned_through: Option<u64>,
    /// Open duties: `(height, signer_index)` of subjects a local key has not yet signed.
    open: BTreeMap<(u64, u8), Duty>,
    /// Committed height at which each entry was last submitted.
    submitted: BTreeMap<(u64, u8), u64>,
    /// Committed height of the last attestation submission per key.
    last_batch: BTreeMap<[u8; 20], u64>,
    /// `(epoch, address)` of the last registration submitted.
    registration: Option<(u64, [u8; 20])>,
    /// Refused key-directory paths already reported.
    reported: BTreeSet<String>,
}

struct Attestor {
    state: Arc<State>,
    queue: Arc<Queue>,
    peer_key_pair: KeyPair,
    config: iroha_config::parameters::actual::SccpAttestor,
    store: KeyStore,
    memory: Mutex<Memory>,
}

fn wall_clock_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

impl Attestor {
    async fn run(self: Arc<Self>, shutdown_signal: ShutdownSignal) {
        let mut interval = tokio::time::interval(TICK);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = interval.tick() => self.tick_blocking().await,
                () = shutdown_signal.receive() => break,
            }
        }
        self.drain().await;
    }

    async fn tick_blocking(self: &Arc<Self>) {
        let attestor = Arc::clone(self);
        match crate::panic_recovery::join_recoverable(
            crate::panic_recovery::spawn_blocking_recoverable(move || attestor.tick()),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => iroha_logger::warn!(?error, "SCCP attestor tick failed"),
            Err(_panic) => iroha_logger::error!("SCCP attestor tick panicked"),
        }
    }

    /// Sign and submit pending subjects until they are recorded or `shutdown_grace` elapses
    /// (§4.9 step 7).
    async fn drain(self: &Arc<Self>) {
        let deadline = Instant::now() + self.config.shutdown_grace;
        loop {
            self.tick_blocking().await;
            let open = self.memory.lock().open.len();
            if open == 0 {
                return;
            }
            if Instant::now() >= deadline {
                iroha_logger::warn!(
                    open,
                    "SCCP attestor shut down with unrecorded subjects of its keys"
                );
                return;
            }
            tokio::time::sleep(TICK).await;
        }
    }

    fn me(&self) -> PeerId {
        PeerId::new(self.peer_key_pair.public_key().clone())
    }

    fn tick(&self) -> eyre::Result<()> {
        let view = self.state.view();
        let committed = u64::try_from(view.height()).unwrap_or(u64::MAX);
        let world = view.world();
        let Some(params) = store::parameters::get(world).clone() else {
            return Ok(());
        };
        if committed == 0 {
            return Ok(());
        }
        let now_ms = wall_clock_ms();
        let me = self.me();
        let mut keys = self.load_keys()?;
        let key_state = store::bridge_keys::get(world, &me)
            .cloned()
            .unwrap_or_default();
        if key_state.is_barred() {
            iroha_logger::warn!(
                health = "sccp_attestor_barred",
                "SCCP bridge key of this peer is barred by a recorded fault"
            );
        }
        let owner_of = |address: &[u8; 20]| store::bridge_key_owners::get(world, address).cloned();
        let mut classified = plan::classify(&keys, &me, owner_of);
        for address in &classified.foreign {
            iroha_logger::error!(
                address = %iroha_sccp::v1::hashes::to_hex(address),
                health = "sccp_attestor_foreign_key",
                "SCCP bridge key in the key directory belongs to another peer; ignored"
            );
        }
        if plan::needs_new_key(&classified, &key_state) {
            let key = SccpBridgeKeyFileV1::generate(now_ms)
                .map_err(|error| eyre::eyre!("generate SCCP bridge key: {error}"))?;
            let path = self.store.write(&key).wrap_err("write SCCP bridge key")?;
            iroha_logger::info!(path = %path.display(), "generated SCCP bridge key");
            keys.push(key);
            classified = plan::classify(&keys, &me, owner_of);
        }
        if self.config.auto_register && world.peers().iter().any(|peer| peer == &me) {
            self.register(&view, committed, &keys, &classified, &key_state)?;
        }
        let owned: BTreeMap<[u8; 20], &SccpBridgeKeyFileV1> = keys
            .iter()
            .filter_map(|key| key.address().ok().map(|address| (address, key)))
            .filter(|(address, _)| classified.owned.contains(address))
            .collect();
        self.scan(&view, committed, &owned);
        self.attest(
            &view,
            committed,
            now_ms,
            &owned,
            params.max_attestation_entries_per_instruction,
        )?;
        self.cleanup(world, now_ms, &owned, &key_state);
        Ok(())
    }

    fn load_keys(&self) -> eyre::Result<Vec<SccpBridgeKeyFileV1>> {
        let (keys, refused) = self.store.load().wrap_err("list SCCP bridge keys")?;
        let mut memory = self.memory.lock();
        for error in refused {
            if memory.reported.insert(error.to_string()) {
                iroha_logger::error!(%error, "refused file in the SCCP bridge-key directory");
            }
        }
        Ok(keys)
    }

    /// Register the newest unregistered key once per epoch (§4.9 step 3).
    fn register(
        &self,
        view: &iroha_core::state::StateView<'_>,
        committed: u64,
        keys: &[SccpBridgeKeyFileV1],
        classified: &plan::ClassifiedKeys,
        key_state: &iroha_data_model::sccp::keys::SccpBridgeKeyStateV1,
    ) -> eyre::Result<()> {
        let Some(candidate) = plan::registration_candidate(classified, key_state) else {
            return Ok(());
        };
        let Some(epoch) = bridge_keys::current_epoch(view.world(), committed.saturating_add(1))
        else {
            return Ok(());
        };
        if self.memory.lock().registration == Some((epoch, candidate)) {
            return Ok(());
        }
        let key = keys
            .iter()
            .find(|key| key.address().ok() == Some(candidate))
            .ok_or_else(|| eyre::eyre!("registration candidate disappeared"))?;
        let instruction = plan::registration(
            *self.state.network_id_ref(),
            &self.peer_key_pair,
            key,
            epoch.saturating_add(1),
            key_state.next_binding_nonce,
        )?;
        self.submit(key, InstructionBox::from(instruction))
            .wrap_err("submit SetSccpBridgeKeyV1")?;
        self.memory.lock().registration = Some((epoch, candidate));
        iroha_logger::info!(
            address = %iroha_sccp::v1::hashes::to_hex(&candidate),
            activation_epoch = epoch.saturating_add(1),
            "submitted SCCP bridge-key registration"
        );
        Ok(())
    }

    /// Find new duties at heights committed since the last scan and drop recorded ones.
    fn scan(
        &self,
        view: &iroha_core::state::StateView<'_>,
        committed: u64,
        owned: &BTreeMap<[u8; 20], &SccpBridgeKeyFileV1>,
    ) {
        let world = view.world();
        let mut memory = self.memory.lock();
        let from = memory.scanned_through.map_or_else(
            || store::prune_cursor::get(world).signatures_height.max(1),
            |through| through.saturating_add(1),
        );
        if from <= committed {
            for (height, subject) in store::attestation_subjects::range(world, from..=committed) {
                let Some(roster) = store::rosters::get(world, &subject.generation) else {
                    continue;
                };
                for (index, member) in roster.members.iter().enumerate() {
                    let Ok(signer_index) = u8::try_from(index) else {
                        continue;
                    };
                    if owned.contains_key(&member.address) {
                        memory.open.insert(
                            (*height, signer_index),
                            Duty {
                                height: *height,
                                signer_index,
                                address: member.address,
                                rotation: subject.is_rotation(),
                                timestamp_ms: subject.timestamp_ms,
                            },
                        );
                    }
                }
            }
        }
        memory.scanned_through = Some(committed);
        memory.open.retain(|(height, signer_index), duty| {
            owned.contains_key(&duty.address)
                && !store::attestation_status::get(world, height)
                    .is_some_and(|status| status.has_signer(*signer_index))
        });
        let open: BTreeSet<_> = memory.open.keys().copied().collect();
        memory.submitted.retain(|key, _| open.contains(key));
    }

    /// Sign open duties and submit one batch per key (§4.9 steps 4–5).
    fn attest(
        &self,
        view: &iroha_core::state::StateView<'_>,
        committed: u64,
        now_ms: u64,
        owned: &BTreeMap<[u8; 20], &SccpBridgeKeyFileV1>,
        on_chain_max: u32,
    ) -> eyre::Result<()> {
        let resubmit_after = self.config.resubmit_after_blocks.get();
        let max_drift_ms =
            u64::try_from(self.config.max_clock_drift.as_millis()).unwrap_or(u64::MAX);
        let duties: Vec<Duty> = {
            let memory = self.memory.lock();
            plan::order(memory.open.values().copied().collect())
                .into_iter()
                .filter(|duty| {
                    memory
                        .submitted
                        .get(&(duty.height, duty.signer_index))
                        .is_none_or(|at| at.saturating_add(resubmit_after) <= committed)
                })
                .filter(|duty| memory.last_batch.get(&duty.address) != Some(&committed))
                .collect()
        };
        let mut signed = Vec::new();
        for duty in duties {
            if plan::future_dated(&duty, now_ms, max_drift_ms) {
                iroha_logger::warn!(
                    height = duty.height,
                    health = "sccp_attestor_future_dated",
                    "refusing to sign a future-dated SCCP rotation subject"
                );
                continue;
            }
            let Some(key) = owned.get(&duty.address) else {
                continue;
            };
            // The statement is recomputed from local durably final state; nothing else is
            // ever signed.
            let Some(digest) = subjects::statement_digest_of(view, duty.height) else {
                continue;
            };
            let signature = key
                .sign_digest(&digest)
                .map_err(|error| eyre::eyre!("sign SCCP statement {}: {error}", duty.height))?;
            signed.push((
                duty.address,
                SccpAttestationSignatureV1 {
                    height: duty.height,
                    signer_index: duty.signer_index,
                    signature,
                },
            ));
        }
        let max_entries = usize::try_from(
            self.config
                .max_entries_per_transaction
                .get()
                .min(on_chain_max),
        )
        .unwrap_or(usize::MAX);
        for (address, entries) in plan::batches(signed, max_entries) {
            let Some(key) = owned.get(&address) else {
                continue;
            };
            let keys: Vec<(u64, u8)> = entries
                .iter()
                .map(|entry| (entry.height, entry.signer_index))
                .collect();
            match self.submit(
                key,
                InstructionBox::from(SubmitSccpAttestationsV1 { entries }),
            ) {
                Ok(()) => {
                    let mut memory = self.memory.lock();
                    memory.last_batch.insert(address, committed);
                    for key in keys {
                        memory.submitted.insert(key, committed);
                    }
                }
                Err(error) => iroha_logger::warn!(?error, "SCCP attestation submission failed"),
            }
        }
        Ok(())
    }

    /// Delete owned keys that are neither live nor held by an unexpired generation (§4.9
    /// step 6).
    fn cleanup(
        &self,
        world: &impl WorldReadOnly,
        now_ms: u64,
        owned: &BTreeMap<[u8; 20], &SccpBridgeKeyFileV1>,
        key_state: &iroha_data_model::sccp::keys::SccpBridgeKeyStateV1,
    ) {
        let live = plan::live_addresses(key_state);
        for address in owned.keys().filter(|address| !live.contains(*address)) {
            let still_needed = store::rosters::iter(world).any(|(_, roster)| {
                roster
                    .members
                    .iter()
                    .any(|member| member.address == *address)
                    && roster
                        .valid_until_ms
                        .saturating_add(KEY_RETENTION_AFTER_EXPIRY_MS)
                        >= now_ms
            });
            if still_needed {
                continue;
            }
            match self.store.delete(address) {
                Ok(()) => iroha_logger::info!(
                    address = %iroha_sccp::v1::hashes::to_hex(address),
                    "deleted expired SCCP bridge key"
                ),
                Err(error) => {
                    iroha_logger::warn!(%error, "failed to delete expired SCCP bridge key")
                }
            }
        }
    }

    /// Quote, sign with `key` as `account_of(key)` and enqueue one transaction.
    fn submit(&self, key: &SccpBridgeKeyFileV1, instruction: InstructionBox) -> eyre::Result<()> {
        submit_transaction(&self.state, &self.queue, key, instruction)
    }
}

/// Quote, sign with `key` as `account_of(key)` and enqueue one transaction carrying
/// `instruction`: the submission path of the attestor and the keeper.
fn submit_transaction(
    state: &State,
    queue: &Queue,
    key: &SccpBridgeKeyFileV1,
    instruction: InstructionBox,
) -> eyre::Result<()> {
    let public_key = key
        .public_key()
        .map_err(|error| eyre::eyre!("bridge key public key: {error}"))?;
    let authority = bridge_keys::account_of(&public_key)
        .map_err(|error| eyre::eyre!("bridge key account: {error}"))?;
    let mut payload = TransactionBuilder::new(
        *state.network_id_ref(),
        authority,
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .into_payload()
    .wrap_err("build SCCP transaction")?;
    let route = queue
        .route_payload_plan_with_state(&payload, state)
        .wrap_err("route SCCP transaction")?;
    let iroha_core::queue::RoutingPlan::Single(route) = route else {
        eyre::bail!("SCCP submission requires one resolved route");
    };
    let latest_header = state.latest_block_header_fast();
    let observation_time_ms = latest_header
        .as_ref()
        .map_or(0, |header| header.creation_time_ms);
    let next_block_height = latest_header
        .as_ref()
        .map_or(1, |header| header.height().get().saturating_add(1));
    let quote = {
        let world = state.world_view();
        quote_nexus_fee_admission_draft(
            &world,
            &state.nexus_snapshot(),
            &state.pipeline_snapshot(),
            &payload,
            observation_time_ms,
            next_block_height,
            Some(route.route.dataspace_id),
        )
    }
    .map_err(|error| eyre::eyre!("quote SCCP transaction: {error:?}"))?;
    payload.fee_payment = quote.recommended_intent;
    let private_key = PrivateKey::from_bytes(Algorithm::Secp256k1, key.secret())
        .map_err(|error| eyre::eyre!("bridge key private key: {error}"))?;
    let transaction = TransactionBuilder::from_payload(payload)
        .wrap_err("rebuild quoted SCCP transaction")?
        .try_sign(&private_key)
        .map_err(|error| eyre::eyre!("sign SCCP transaction: {error:?}"))?;
    let (max_clock_drift, transaction_params) = {
        let world = state.world_view();
        let params = world.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let crypto = state.crypto();
    let accepted = AcceptedTransaction::accept(
        transaction,
        state.network_id_ref(),
        max_clock_drift,
        transaction_params,
        crypto.as_ref(),
    )
    .wrap_err("accept SCCP transaction")?;
    queue
        .push_with_lane_with_state(accepted, state)
        .map(|_| ())
        .map_err(|failure| eyre::eyre!("enqueue SCCP transaction: {}", failure.err))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use iroha_config::parameters::actual::{
        Queue as QueueConfig, SccpAttestor, SccpLightClientKeeper,
    };
    use iroha_config_base::WithOrigin;
    use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::World};
    use iroha_data_model::bridge::SccpNetworkV1;
    use iroha_sccp_rpc::PollBudget;

    use super::{
        keeper::{Cadence, Lane, fakes},
        *,
    };

    /// An empty node: committed state without SCCP and a local queue.
    fn node() -> (Arc<State>, Arc<Queue>) {
        let state = Arc::new(State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let (events, _) = tokio::sync::broadcast::channel(1);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events));
        (state, queue)
    }

    fn attestor_config(dir: &Path) -> SccpAttestor {
        let mut config =
            SccpAttestor::defaults_for_kura_store_dir(&WithOrigin::inline(dir.to_path_buf()));
        config.key_dir = WithOrigin::inline(dir.join("bridge-keys"));
        config
    }

    fn attestor(dir: &Path) -> Arc<Attestor> {
        let (state, queue) = node();
        let config = attestor_config(dir);
        Arc::new(Attestor {
            state,
            queue,
            peer_key_pair: KeyPair::random(),
            store: KeyStore::open(&config.key_dir_path()).expect("key directory"),
            config,
            memory: Mutex::new(Memory::default()),
        })
    }

    /// Waits (asynchronously, so a current-thread runtime keeps running) until `ready`.
    async fn eventually(what: &str, ready: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !ready() {
            assert!(Instant::now() < deadline, "timed out waiting until {what}");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// The keeper's network I/O hangs on a blocking worker while the attestor keeps ticking on
    /// the only async worker of a current-thread runtime.
    #[tokio::test(flavor = "current_thread")]
    async fn a_blocked_keeper_does_not_delay_attestor_ticks() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let attestor = attestor(dir.path());
        let shutdown = ShutdownSignal::new();
        let host = Arc::new(fakes::FakeHost::default());
        let config = Arc::new(fakes::fast_config(10, 50));
        let hung = Arc::new(fakes::GatedSource::default());
        let lane = Lane::new(
            SccpNetworkV1::EthereumMainnet,
            fakes::shared(&hung),
            PollBudget::new(),
            Cadence::new(config.poll_interval, 1),
        );
        let keeper = tokio::spawn(keeper::run(
            vec![lane],
            Arc::clone(&host),
            config,
            shutdown.clone(),
        ));
        eventually("the keeper's build hung", || hung.entered() == 1).await;
        let attestor_run = tokio::spawn(Arc::clone(&attestor).run(shutdown.clone()));
        for _ in 0..6 {
            let started = Instant::now();
            tokio::time::timeout(Duration::from_secs(5), attestor.tick_blocking())
                .await
                .expect("an attestor tick finishes while the keeper is blocked");
            assert!(started.elapsed() < Duration::from_secs(5));
        }
        // The run loop ticks on its own schedule meanwhile.
        tokio::time::sleep(TICK * 3).await;
        assert!(!attestor_run.is_finished());
        assert_eq!(
            hung.entered(),
            1,
            "the keeper is still blocked in its first build"
        );
        assert_eq!(host.submissions(SccpNetworkV1::EthereumMainnet), 0);
        hung.open();
        eventually("the keeper submitted after the endpoint answered", || {
            host.submissions(SccpNetworkV1::EthereumMainnet) >= 1
        })
        .await;
        shutdown.send();
        tokio::time::timeout(Duration::from_secs(10), keeper)
            .await
            .expect("the keeper stops on shutdown")
            .expect("the keeper does not panic");
        tokio::time::timeout(Duration::from_secs(10), attestor_run)
            .await
            .expect("the attestor stops on shutdown")
            .expect("the attestor does not panic");
    }

    #[tokio::test]
    async fn the_attestor_and_the_keeper_start_as_separate_tasks() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let shutdown = ShutdownSignal::new();
        let started = |attestor: SccpAttestor, keeper: SccpLightClientKeeper| {
            let (state, queue) = node();
            start(
                state,
                queue,
                KeyPair::random(),
                attestor,
                keeper,
                shutdown.clone(),
            )
            .len()
        };
        let enabled = attestor_config(dir.path());
        assert_eq!(
            started(enabled.clone(), SccpLightClientKeeper::default()),
            2
        );
        let idle_keeper = SccpLightClientKeeper {
            enabled: false,
            ..SccpLightClientKeeper::default()
        };
        assert_eq!(started(enabled.clone(), idle_keeper), 1);
        let mut disabled = enabled.clone();
        disabled.enabled = false;
        assert_eq!(started(disabled, SccpLightClientKeeper::default()), 0);
        let mut unusable = enabled;
        let not_a_directory = dir.path().join("file");
        std::fs::write(&not_a_directory, b"x").expect("write file");
        unusable.key_dir = WithOrigin::inline(not_a_directory);
        assert_eq!(started(unusable, SccpLightClientKeeper::default()), 0);
        shutdown.send();
    }

    #[tokio::test]
    async fn submissions_on_a_node_without_sccp_fail_without_panicking() {
        let (state, queue) = node();
        let key = fakes::key();
        let advance = iroha_data_model::isi::sccp::AdvanceSccpLightClientV1 {
            network: SccpNetworkV1::BscMainnet,
            expected_state_hash: None,
            advance: iroha_data_model::sccp::light_client::SccpLcAdvanceBytesV1::new(vec![1])
                .expect("advance bytes"),
        };
        assert!(
            submit_transaction(&state, &queue, &key, InstructionBox::from(advance)).is_err(),
            "an advance on a node without SCCP is refused"
        );
    }
}
