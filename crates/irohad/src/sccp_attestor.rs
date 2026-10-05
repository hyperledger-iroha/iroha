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
    beacon::{
        GlobalThresholdBeaconSessionBindingV1,
        active_global_threshold_beacon_session_id_v1,
        authenticated_global_threshold_beacon_roster_hash_iter_v1,
    },
    executor::quote_nexus_fee_admission_draft,
    queue::Queue,
    smartcontracts::isi::sccp::{bridge_keys, store, subjects},
    state::{State, StateReadOnly as _, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
use iroha_data_model::{
    block::consensus::SumeragiRootScope,
    isi::{InstructionBox, sccp::SubmitSccpAttestationsV1},
    sumeragi::epoch::{BeaconEpochBindingV1, InstalledBeaconEpochBindingV1},
    sccp::attestation::SccpAttestationSignatureV1,
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_model_base::peer::PeerId;
use iroha_sccp::v1::key_file::SccpBridgeKeyFileV1;
use mv::storage::StorageReadOnly as _;
use parking_lot::Mutex;

use self::{key_store::KeyStore, plan::Duty};

/// A key is deleted once every generation holding it expired this long ago (§4.9 step 6).
const KEY_RETENTION_AFTER_EXPIRY_MS: u64 = 86_400_000;
/// Tick interval of the attestor loop.
const TICK: Duration = Duration::from_millis(500);

/// Start the attestor, or return `None` when it is disabled or its key directory is unusable.
pub(crate) fn start(
    state: Arc<State>,
    queue: Arc<Queue>,
    peer_key_pair: KeyPair,
    config: iroha_config::parameters::actual::SccpAttestor,
    keeper_config: iroha_config::parameters::actual::SccpLightClientKeeper,
    shutdown_signal: ShutdownSignal,
) -> Option<Child> {
    if !config.enabled {
        iroha_logger::info!("SCCP attestor disabled by configuration");
        return None;
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
            return None;
        }
    };
    let task = tokio::task::spawn(async move {
        // The keeper's blocking HTTP clients are built on a blocking worker, never on the
        // async worker that runs this task.
        let keeper = keeper::Keeper::build(keeper_config).await;
        let attestor = Arc::new(Attestor {
            state,
            queue,
            peer_key_pair,
            config,
            store,
            memory: Mutex::new(Memory::default()),
            keeper: Mutex::new(keeper),
        });
        attestor.run(shutdown_signal).await;
    });
    Some(Child::new(task, OnShutdown::Wait(Duration::from_secs(1))))
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
    keeper: Mutex<keeper::Keeper>,
}

fn wall_clock_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Automatic SCCP transactions wait for the original global native bootstrap.
/// Local bridge-key custody may be prepared while the signed DKG owns its phase heights.
/// Private roots keep their existing behavior and cannot acquire global beacon custody.
fn automatic_submissions_ready(view: &iroha_core::state::StateView<'_>) -> eyre::Result<bool> {
    let world = view.world();
    let scope = iroha_core::sumeragi::lanes::routing::committed_root_scope(world)
        .ok_or_else(|| eyre::eyre!("SCCP automatic submission requires immutable root scope"))?;
    if !matches!(scope, SumeragiRootScope::Global) {
        return Ok(true);
    }
    let committed = u64::try_from(view.height())?;
    let next = committed
        .checked_add(1)
        .ok_or_else(|| eyre::eyre!("SCCP next submission height overflows"))?;
    let schedule = world.consensus_schedule();
    let current = &schedule
        .ready(next)
        .map_err(|error| eyre::eyre!("SCCP submission schedule: {error}"))?
        .epoch;
    current.validate().map_err(|error| eyre::eyre!("SCCP submission epoch: {error}"))?;
    if current.network_id != *view.network_id() {
        eyre::bail!("SCCP submission epoch belongs to another network");
    }
    let Some(id) = active_global_threshold_beacon_session_id_v1(world)? else {
        return Ok(false);
    };
    let record = world
        .global_beacon_key_sessions()
        .get(&id)
        .ok_or_else(|| eyre::eyre!("SCCP active beacon session is absent"))?;
    // This is the original sealed World record, not a caller-provided public DTO.
    record.validate()?;
    let binding = InstalledBeaconEpochBindingV1 {
        session_id: id,
        transcript_hash: record.session.transcript_hash,
    };
    if record.session.network_id != current.network_id
        || record.session.adaptive_dkg.finalized_at_height > committed
        || record.session.adaptive_dkg.session.authority_generation != current.authority.generation
        || !record.is_active_at(next)
        || (current.authorization.beacon != BeaconEpochBindingV1::Bootstrap
            && current.authorization.beacon != BeaconEpochBindingV1::Installed(binding))
    {
        return Ok(false);
    }
    let roster_hash = authenticated_global_threshold_beacon_roster_hash_iter_v1(
        &record.session,
        current.committee.iter().map(|seat| &seat.validator),
    )?;
    record.session.check_binding(&GlobalThresholdBeaconSessionBindingV1 {
        network_id: current.network_id,
        session_id: id,
        roster_hash,
        transcript_hash: record.session.transcript_hash,
    })?;
    Ok(true)
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
        // A real pending bootstrap is not a successful submission: no registration or
        // attestation bookkeeping changes until this original committed cut is usable.
        // Shutdown drain calls this same tick and cannot bypass the boundary.
        if !automatic_submissions_ready(&view)? {
            return Ok(());
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
        self.keep_light_clients(world, now_ms, &owned, &key_state);
        Ok(())
    }

    /// Advance stale inbound light clients from a live bridge key's account (§4.13.4).
    fn keep_light_clients(
        &self,
        world: &impl WorldReadOnly,
        now_ms: u64,
        owned: &BTreeMap<[u8; 20], &SccpBridgeKeyFileV1>,
        key_state: &iroha_data_model::sccp::keys::SccpBridgeKeyStateV1,
    ) {
        let live = plan::live_addresses(key_state);
        let Some(key) = owned
            .iter()
            .find(|(address, _)| live.contains(*address))
            .map(|(_, key)| *key)
        else {
            return;
        };
        let advances = {
            let mut keeper = self.keeper.lock();
            if !keeper.due() {
                return;
            }
            let light_clients: Vec<_> = store::light_clients::iter(world)
                .map(|(_, light_client)| *light_client)
                .collect();
            keeper.advances(&light_clients, now_ms)
        };
        for advance in advances {
            if let Err(error) = self.submit(key, InstructionBox::from(advance)) {
                iroha_logger::warn!(?error, "SCCP keeper: submitting an advance failed");
            }
        }
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
        let public_key = key
            .public_key()
            .map_err(|error| eyre::eyre!("bridge key public key: {error}"))?;
        let authority = bridge_keys::account_of(&public_key)
            .map_err(|error| eyre::eyre!("bridge key account: {error}"))?;
        let mut payload = TransactionBuilder::new(
            *self.state.network_id_ref(),
            authority,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .into_payload()
        .wrap_err("build SCCP transaction")?;
        let route = self
            .queue
            .route_payload_plan_with_state(&payload, self.state.as_ref())
            .wrap_err("route SCCP transaction")?;
        let iroha_core::queue::RoutingPlan::Single(route) = route else {
            eyre::bail!("SCCP submission requires one resolved route");
        };
        let latest_header = self.state.latest_block_header_fast();
        let observation_time_ms = latest_header
            .as_ref()
            .map_or(0, |header| header.creation_time_ms);
        let next_block_height = latest_header
            .as_ref()
            .map_or(1, |header| header.height().get().saturating_add(1));
        let quote = {
            let world = self.state.world_view();
            quote_nexus_fee_admission_draft(
                &world,
                &self.state.nexus_snapshot(),
                &self.state.pipeline_snapshot(),
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
            let world = self.state.world_view();
            let params = world.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let crypto = self.state.crypto();
        let accepted = AcceptedTransaction::accept(
            transaction,
            self.state.network_id_ref(),
            max_clock_drift,
            transaction_params,
            crypto.as_ref(),
        )
        .wrap_err("accept SCCP transaction")?;
        self.queue
            .push_with_lane_with_state(accepted, self.state.as_ref())
            .map(|_| ())
            .map_err(|failure| eyre::eyre!("enqueue SCCP transaction: {}", failure.err))
    }
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU64, os::unix::fs::PermissionsExt as _};

    use iroha_config_base::{ParameterOrigin, WithOrigin};
    use iroha_core::{
        beacon::{
            FinalizedGlobalThresholdBeaconKeySessionRecordV1,
            complete_beacon_dkg_fixture_for_exact_session_v1,
            complete_beacon_dkg_fixture_for_seat_v1,
            global_threshold_beacon_roster_hash_v1,
        },
        state::{
            THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1, World,
            threshold_key_lifecycle_certificate_preimage_v1,
        },
        sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
    };
    use iroha_crypto::{Hash, HashOf, Signature};
    use iroha_data_model::{
        NetworkId,
        isi::{
            consensus_keys::{
                ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleActionV1,
                ThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleSignatureV1,
            },
            sccp::{InitializeSccpV1, SetSccpBridgeKeyV1},
        },
        parameter::{
            Parameter,
            system::{SumeragiConsensusMode, SumeragiNposParameters, SumeragiParameter},
        },
        sccp::params::SccpParametersV1,
        transaction::Executable,
    };

    use super::*;

    struct Fixture {
        chain: CertifiedTestChain,
        validator_keys: Vec<KeyPair>,
        genesis_key: KeyPair,
        key_dir: tempfile::TempDir,
    }

    fn original_config() -> (TestChainConfig, Vec<KeyPair>) {
        // These original keys are the public all-edge DKG fixture's canonical roster.
        // The signed genesis, not a World overlay, selects them and the NPoS policy.
        let mut keys = (1_u8..=4)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
        let mut config = TestChainConfig::new(World::new(), wall_clock_ms().saturating_sub(1_000));
        config.validator_keys = Some(keys.clone());
        config.consensus_mode = SumeragiConsensusMode::Npos;
        let policy = SumeragiNposParameters {
            epoch_length_blocks: NonZeroU64::new(64).unwrap(),
            max_validators: 4,
            evidence_horizon_blocks: 128,
            slashing_delay_blocks: 2,
            ..SumeragiNposParameters::default()
        };
        policy.validate().unwrap();
        config.genesis_parameters.extend([
            Parameter::Sumeragi(SumeragiParameter::EpochLengthBlocks(
                policy.epoch_length_blocks,
            )),
            Parameter::Custom(policy.into_custom_parameter()),
        ]);
        config.genesis_instructions.push(
            InitializeSccpV1 {
                parameters: SccpParametersV1::taira_default(),
                reset_nonce: [0x83; 32],
            }
            .into(),
        );
        (config, keys)
    }

    impl Fixture {
        fn new() -> Self {
            let (config, validator_keys) = original_config();
            let genesis_key = config.genesis_key.clone();
            let chain = CertifiedTestChain::start(config).expect("original signed SCCP genesis");
            assert_eq!(chain.height(), 1);
            assert!(store::parameters::get(chain.state().view().world()).is_some());
            Self {
                chain,
                validator_keys,
                genesis_key,
                key_dir: tempfile::tempdir().unwrap(),
            }
        }

        async fn actor(&self) -> Arc<Attestor> {
            let kura_dir = WithOrigin::new(
                self.key_dir.path().to_path_buf(),
                ParameterOrigin::custom("original SCCP actor test custody".into()),
            );
            let mut config =
                iroha_config::parameters::actual::SccpAttestor::defaults_for_kura_store_dir(&kura_dir);
            config.enabled = true;
            config.auto_register = true;
            // Disabled endpoint clients are original configuration, not a substitute submitter.
            let keeper = keeper::Keeper::build(
                iroha_config::parameters::actual::SccpLightClientKeeper {
                    enabled: false,
                    ..Default::default()
                },
            )
            .await;
            let (events, _receiver) = tokio::sync::broadcast::channel(64);
            Arc::new(Attestor {
                state: Arc::clone(self.chain.state()),
                queue: Arc::new(Queue::from_config(Default::default(), events)),
                peer_key_pair: self.validator_keys[0].clone(),
                store: KeyStore::open(&config.key_dir_path()).unwrap(),
                config,
                memory: Mutex::new(Memory::default()),
                keeper: Mutex::new(keeper),
            })
        }

        fn advance_to(&mut self, height: u64) {
            while self.chain.height() < height {
                self.chain.commit(Vec::new());
            }
        }

        fn original_beacon(&self) -> FinalizedGlobalThresholdBeaconKeySessionRecordV1 {
            let (session, _seat_custody) = complete_beacon_dkg_fixture_for_seat_v1(
                self.chain.network_id(),
                [0x84; 32],
                4,
                1,
            );
            FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(
                session,
                &self.chain.state().ivm_execution_budget(),
            )
            .unwrap()
        }

        fn certificate(
            &self,
            record: &FinalizedGlobalThresholdBeaconKeySessionRecordV1,
        ) -> ThresholdKeyLifecycleCertificateV1 {
            let view = self.chain.state().view();
            let roster = view
                .world()
                .consensus_schedule()
                .ready(self.chain.height() + 1)
                .unwrap()
                .epoch
                .committee
                .iter()
                .map(|seat| seat.validator.clone())
                .collect::<Vec<_>>();
            assert_eq!(
                roster,
                self.validator_keys
                    .iter()
                    .map(|key| PeerId::new(key.public_key().clone()))
                    .collect::<Vec<_>>()
            );
            let mut certificate = ThresholdKeyLifecycleCertificateV1 {
                version: THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
                action: ThresholdKeyLifecycleActionV1::FinalizeGlobalBeaconKey,
                expected_active_session_id: view.world().active_global_beacon_key_session(),
                effective_height: self.chain.height() + 1,
                network_id: self.chain.network_id(),
                roster_hash: global_threshold_beacon_roster_hash_v1(&roster),
                committee_size: 4,
                quorum: 3,
                session_id: record.session.session_id,
                transcript_hash: record.session.transcript_hash,
                public_state: norito::encode_canonical(record).unwrap(),
                signatures: Vec::new(),
            };
            self.sign_certificate(&mut certificate);
            certificate
        }

        fn sign_certificate(&self, certificate: &mut ThresholdKeyLifecycleCertificateV1) {
            let preimage = threshold_key_lifecycle_certificate_preimage_v1(certificate).unwrap();
            certificate.signatures = self.validator_keys
                .iter()
                .take(3)
                .enumerate()
                .map(|(index, key)| ThresholdKeyLifecycleSignatureV1 {
                    signer_index: u16::try_from(index).unwrap(),
                    signature: Signature::try_new(key.private_key(), &preimage).unwrap(),
                })
                .collect();
        }

        fn commit_certificate(&mut self, certificate: ThresholdKeyLifecycleCertificateV1) -> bool {
            let original_time = self.chain.committed(self.chain.height()).block_time_ms();
            let signed = self.chain.sign(
                &self.genesis_key,
                [ApplyThresholdKeyLifecycleCertificateV1 { certificate }.into()],
                original_time,
            );
            self.chain.commit(vec![signed])[0]
        }
    }

    fn retained_key_address(actor: &Attestor) -> [u8; 20] {
        let (keys, refusals) = actor.store.load().unwrap();
        assert!(refusals.is_empty());
        assert_eq!(keys.len(), 1);
        assert_eq!(std::fs::metadata(actor.store.dir()).unwrap().permissions().mode() & 0o777, 0o700);
        let leaves = std::fs::read_dir(actor.store.dir())
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].metadata().unwrap().permissions().mode() & 0o777, 0o600);
        keys[0].address().unwrap()
    }

    fn assert_deferred(actor: &Attestor) {
        assert_eq!(actor.queue.queued_len(), 0);
        let memory = actor.memory.lock();
        assert!(memory.registration.is_none());
        assert!(memory.scanned_through.is_none());
        assert!(memory.open.is_empty());
        assert!(memory.submitted.is_empty());
        assert!(memory.last_batch.is_empty());
    }

    #[test]
    fn uncommitted_signed_genesis_cannot_supply_an_inferred_root_scope() {
        let (config, _) = original_config();
        let prepared = CertifiedTestChain::prepare(config).unwrap();
        let view = prepared.state.view();
        assert_eq!(view.height(), 0);
        assert!(automatic_submissions_ready(&view).is_err());
    }

    #[tokio::test]
    async fn genesis_and_dkg_phase_heights_retain_keys_without_any_automatic_queue_work() {
        let mut fixture = Fixture::new();
        let actor = fixture.actor().await;
        let mut original_address = None;
        for height in 1..=4 {
            fixture.advance_to(height);
            assert!(!automatic_submissions_ready(&fixture.chain.state().view()).unwrap());
            actor.tick().unwrap();
            assert_deferred(&actor);
            let address = retained_key_address(&actor);
            assert_eq!(*original_address.get_or_insert(address), address);
            assert!(fixture.chain.state().view().world().active_global_beacon_key_session().is_none());
        }
    }

    #[tokio::test]
    async fn restart_and_shutdown_drain_cannot_queue_a_bootstrap_registration() {
        let fixture = Fixture::new();
        let actor = fixture.actor().await;
        actor.tick().unwrap();
        let original_address = retained_key_address(&actor);
        assert_deferred(&actor);
        drop(actor);
        let restarted = fixture.actor().await;
        restarted.tick().unwrap();
        assert_eq!(retained_key_address(&restarted), original_address);
        restarted.drain().await;
        assert_eq!(retained_key_address(&restarted), original_address);
        assert_deferred(&restarted);
    }

    #[tokio::test]
    async fn applied_original_beacon_lifecycle_releases_one_network_bound_registration() {
        let mut fixture = Fixture::new();
        let actor = fixture.actor().await;
        actor.tick().unwrap();
        let original_address = retained_key_address(&actor);
        fixture.advance_to(4);
        let record = fixture.original_beacon();
        assert_eq!(record.session.adaptive_dkg.finalized_at_height, 4);
        let certificate = fixture.certificate(&record);
        assert!(fixture.commit_certificate(certificate));
        assert_eq!(fixture.chain.height(), 5);
        {
            let view = fixture.chain.state().view();
            let retained = view.world().global_beacon_key_sessions().get(&record.session.session_id).unwrap();
            assert_eq!(retained.activated_at_height, Some(6));
            assert!(automatic_submissions_ready(&view).unwrap());
        }
        actor.tick().unwrap();
        actor.tick().unwrap();
        assert_eq!(actor.queue.queued_len(), 1);
        assert_eq!(retained_key_address(&actor), original_address);
        let signed = {
            let view = fixture.chain.state().view();
            let pending = actor.queue.all_transactions(&view).collect::<Vec<_>>();
            assert_eq!(pending.len(), 1);
            let signed = pending[0].external().unwrap().clone();
            let Executable::Instructions(instructions) = signed.instructions() else {
                panic!("original automatic registration is an instruction transaction");
            };
            assert_eq!(instructions.len(), 1);
            let registration = instructions[0].as_any().downcast_ref::<SetSccpBridgeKeyV1>().unwrap();
            assert_eq!(registration.peer, actor.me());
            registration.peer_signature.verify(
                fixture.validator_keys[0].public_key(),
                &registration.binding(fixture.chain.network_id()),
            ).unwrap();
            assert!(bridge_keys::check_binding(
                view.world(),
                &fixture.chain.network_id(),
                6,
                false,
                registration,
                signed.authority(),
            ).unwrap().is_some());
            let foreign_network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"foreign registration network")));
            assert!(bridge_keys::check_binding(
                view.world(), &foreign_network, 6, false, registration, signed.authority(),
            ).is_err());
            signed
        };
        assert_eq!(fixture.chain.commit(vec![signed]), [true]);
        drop(actor);
        let restarted = fixture.actor().await;
        restarted.tick().unwrap();
        assert_eq!(retained_key_address(&restarted), original_address);
        assert_eq!(restarted.queue.queued_len(), 0);
        assert!(restarted.memory.lock().registration.is_none());
    }

    #[tokio::test]
    async fn malformed_foreign_retired_and_future_lifecycle_inputs_do_not_release_automatic_work() {
        for refusal in ["malformed", "foreign", "retired", "future", "generation"] {
            let mut fixture = Fixture::new();
            let actor = fixture.actor().await;
            actor.tick().unwrap();
            let original_address = retained_key_address(&actor);
            fixture.advance_to(4);
            let mut record = fixture.original_beacon();
            match refusal {
                "malformed" => record.session.transcript_hash = [0; 32],
                "foreign" => {
                    let foreign_network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"foreign genuine DKG network")));
                    let (session, _custody) = complete_beacon_dkg_fixture_for_seat_v1(foreign_network, [0x85; 32], 4, 1);
                    record = FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(session, &fixture.chain.state().ivm_execution_budget()).unwrap();
                }
                "retired" => {
                    record.activated_at_height = Some(4);
                    record.retired_at_height = Some(5);
                }
                "future" | "generation" => {
                    let mut session = record.session.adaptive_dkg.session.clone();
                    session.session_id = if refusal == "future" { [0x86; 32] } else { [0x87; 32] };
                    session.attempt_id = session.session_id;
                    if refusal == "future" {
                        session.start_height = 100;
                        session.commitments_end_height = 101;
                        session.deliveries_end_height = 102;
                        session.acceptances_end_height = 103;
                    } else {
                        session.authority_generation = 1;
                    }
                    let (session, _custody) = complete_beacon_dkg_fixture_for_exact_session_v1(session, 1);
                    record = FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(session, &fixture.chain.state().ivm_execution_budget()).unwrap();
                }
                _ => unreachable!(),
            }
            let certificate = fixture.certificate(&record);
            assert!(!fixture.commit_certificate(certificate), "{refusal} must be natively rejected");
            let view = fixture.chain.state().view();
            assert!(view.world().active_global_beacon_key_session().is_none());
            assert!(view.world().global_beacon_key_sessions().get(&record.session.session_id).is_none());
            assert!(!automatic_submissions_ready(&view).unwrap());
            drop(view);
            actor.tick().unwrap();
            actor.drain().await;
            assert_eq!(retained_key_address(&actor), original_address);
            assert_deferred(&actor);
        }
    }
}
