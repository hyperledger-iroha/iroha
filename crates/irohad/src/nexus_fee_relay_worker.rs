//! Asynchronous Nexus fee relay worker for activated DPN lane-relay burns.
//!
//! The worker never mutates world state directly. It watches finalized lane
//! relay envelopes published by core, builds FastPQ/AXT proofs, and submits the
//! corresponding protocol ISIs through the normal transaction queue.
//! The process-local Sumeragi status cache is only a bounded notification
//! surface: a relay becomes proof-eligible only when the exact envelope is also
//! present in State's finality-authenticated lane-relay store. The worker
//! constructs proofs; normal ISI execution performs their authoritative
//! verification before any state mutation. Its retry journal is treated as
//! untrusted startup input: fixed byte, item, key, and proof budgets are
//! enforced before decoded state is retained.
use eyre::{Result, WrapErr};
use iroha_config::parameters::actual::{
    Fastpq, FastpqExecutionMode, FastpqPoseidonMode, NexusRelayWorker as NexusRelayWorkerConfig,
};
use iroha_core::{
    queue::Queue,
    state::{LaneRelayStore, State, StateView, WorldReadOnly},
    sumeragi::{self, SumeragiHandle},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{
    account::{AccountId, ParsedAccountId},
    asset::id::AssetDefinitionId,
    isi::{
        InstructionBox,
        nexus::{RegisterVerifiedFeeSponsorVaultAllocation, RegisterVerifiedLaneRelay},
    },
    metadata::Metadata,
    name::Name,
    nexus::{
        AxtEffectBinding, AxtFastpqBinding, AxtProofEnvelope, DataSpaceId, FeeSponsorAssetBudget,
        FeeSponsorEligibility, FeeSponsorProgramId, FeeSponsorProgramLifecycle,
        FeeSponsorProgramRevisionKey, FeeSponsorVaultAllocationClaim, FeeSponsorVaultKey,
        LANE_RELAY_FASTPQ_EFFECT_TYPE, LaneFastpqProofMaterial, LaneRelayEnvelope,
        MAX_ACTIVE_EXECUTION_LANES, ProofBlob,
        VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX,
        VERIFIED_LANE_RELAY_STATE_KEY_PREFIX, VerifiedFeeSponsorVaultAllocation,
        VerifiedLaneRelayRecord, fee_sponsor_vault_allocation_claim_digest,
        fee_sponsor_vault_source_state_root, lane_relay_fastpq_claim_digest,
    },
    state_path::StatePath,
    transaction::{SignedTransaction, TransactionBuilder},
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_primitives::{
    json::Json,
    numeric::{MAX_DECIMAL_SCALE, Numeric, Quantity, RoundingMode},
};
use mv::storage::StorageReadOnly;
use norito::{
    DecodeLimits,
    codec::{Decode, Encode},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};
const WORKER_STATE_FILE: &str = "nexus_fee_relay_worker_state.norito";
const FEE_SPONSOR_VAULT_ALLOCATION_EFFECT_TYPE: &str = "fee_sponsor_vault_allocation";
const WORKER_STATE_MAX_ITEMS_PER_KIND: usize = MAX_ACTIVE_EXECUTION_LANES;
const WORKER_STATE_MAX_BYTES: usize = 64 * 1024 * 1024;
const WORKER_STATE_MAX_RELAY_BYTES: usize = 16 * 1024 * 1024;
const WORKER_STATE_MAX_PROOF_BYTES: usize = 16 * 1024 * 1024;
const WORKER_STATE_MAX_TOTAL_PROOF_BYTES: usize = 32 * 1024 * 1024;
const WORKER_STATE_MAX_KEY_BYTES: usize = 4 * 1024;
const WORKER_STATE_MAX_DECODE_ALLOCATED_BYTES: usize = 128 * 1024 * 1024;
const WORKER_STATE_MAX_DECODE_DEPTH: usize = 32;
#[derive(Clone, Debug, Default, Decode, Encode)]
struct DurableWorkerState {
    relays: BTreeMap<String, DurableRelayWork>,
    allocations: BTreeMap<String, DurableAllocationWork>,
}
#[derive(Clone, Debug, Decode, Encode)]
struct DurableRelayWork {
    envelope: LaneRelayEnvelope,
    status: DurableWorkStatus,
    attempts: u32,
    last_height: u64,
}
enum RelayAttemptDecision {
    Deferred,
    Rejected,
    Ready(Box<LaneRelayEnvelope>),
}
#[derive(Clone, Debug, Decode, Encode)]
struct DurableAllocationWork {
    program_id: FeeSponsorProgramId,
    program_revision: u64,
    asset_definition_id: AssetDefinitionId,
    verified_allocation: Quantity,
    source_dataspace_id: DataSpaceId,
    source_height: u64,
    source_state_root: Hash,
    expires_at_height: u64,
    lease_id: Hash,
    manifest_root: [u8; 32],
    proof_blob: Option<ProofBlob>,
    status: DurableWorkStatus,
    attempts: u32,
    last_height: u64,
}
struct AllocationCandidatePlanV1<'a> {
    program_id: &'a FeeSponsorProgramId,
    program_revision: u64,
    current_height: u64,
    expiry_height: u64,
    routes: &'a [(DataSpaceId, [u8; 32])],
}
#[derive(Encode)]
struct FeeSponsorVaultLeaseBinding {
    version: u8,
    program_id: FeeSponsorProgramId,
    program_revision: u64,
    asset_definition_id: AssetDefinitionId,
    source_dataspace_id: DataSpaceId,
    source_height: u64,
    source_state_root: Hash,
    expires_at_height: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
enum DurableWorkStatus {
    Pending,
    Proving,
    Submitted,
    Accepted,
    Rejected,
}
/// Queue-backed DPN/Nexus fee settlement relay worker.
pub struct NexusFeeRelayWorker {
    config: NexusRelayWorkerConfig,
    state_path: PathBuf,
    queue: Arc<Queue>,
    state: Arc<State>,
    sumeragi: SumeragiHandle,
    authority: AccountId,
    key_pair: KeyPair,
    fastpq: Fastpq,
    durable: parking_lot::Mutex<DurableWorkerState>,
    announced_relays: parking_lot::Mutex<BTreeSet<String>>,
}
/// Node-owned dependencies consumed when constructing the Nexus fee relay worker.
pub struct NexusFeeRelayWorkerContext {
    /// Private durable worker-state directory.
    pub storage_root: PathBuf,
    /// Transaction queue used for ordinary admission.
    pub queue: Arc<Queue>,
    /// Committed state used for finalized reads.
    pub state: Arc<State>,
    /// Consensus handle used to observe finalized relay work.
    pub sumeragi: SumeragiHandle,
    /// Node authority used for internal transactions.
    pub authority: AccountId,
    /// Node key used to sign internal transactions.
    pub key_pair: KeyPair,
    /// Configured deterministic proof backend.
    pub fastpq: Fastpq,
}
impl NexusFeeRelayWorker {
    /// Construct a worker. The optional configured relayer account must match the node key.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured authority is noncanonical or does
    /// not match the node authority.
    pub fn new(
        config: NexusRelayWorkerConfig,
        context: NexusFeeRelayWorkerContext,
    ) -> Result<Self> {
        let NexusFeeRelayWorkerContext {
            storage_root,
            queue,
            state,
            sumeragi,
            authority,
            key_pair,
            fastpq,
        } = context;
        if let Some(raw) = config.authority_account_id.as_deref() {
            let configured = parse_canonical_account_id(raw).wrap_err_with(|| {
                format!("parse nexus.relay_worker.authority_account_id `{raw}`")
            })?;
            if configured != authority {
                eyre::bail!(
                    "nexus.relay_worker.authority_account_id `{configured}` does not match node authority `{authority}`"
                );
            }
        }
        validate_worker_item_limit(config.max_pending_relays.get())?;
        let state_path = storage_root.join(WORKER_STATE_FILE);
        let mut durable = load_durable_state(&state_path, config.max_pending_relays.get())
            .unwrap_or_else(|error| {
                iroha_logger::warn!(
                    ?error,
                    path = %state_path.display(),
                    "failed to load Nexus fee relay worker state; starting with an empty retry set"
                );
                DurableWorkerState::default()
            });
        if prune_durable_worker_state(&mut durable, config.max_pending_relays.get()) {
            persist_durable_state(&state_path, &durable, config.max_pending_relays.get())
                .wrap_err("persist bounded Nexus fee relay worker state after startup pruning")?;
        }
        Ok(Self {
            config,
            state_path,
            queue,
            state,
            sumeragi,
            authority,
            key_pair,
            fastpq,
            durable: parking_lot::Mutex::new(durable),
            announced_relays: parking_lot::Mutex::new(BTreeSet::new()),
        })
    }
    /// Start the worker reconciliation loop.
    pub fn start(self, shutdown_signal: ShutdownSignal) -> Child {
        let worker = Arc::new(self);
        let task = tokio::task::spawn({
            let worker = Arc::clone(&worker);
            async move {
                let mut interval = tokio::time::interval(worker.config.retry_backoff);
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            let worker = Arc::clone(&worker);
                            match tokio::task::spawn_blocking(move || worker.reconcile_once()).await {
                                Ok(Ok(())) => {}
                                Ok(Err(error)) => {
                                    iroha_logger::warn!(
                                        ?error,
                                        "Nexus fee relay worker reconciliation failed"
                                    );
                                }
                                Err(error) => {
                                    iroha_logger::warn!(
                                        ?error,
                                        "Nexus fee relay worker task panicked"
                                    );
                                }
                            }
                        }
                        () = shutdown_signal.receive() => {
                            iroha_logger::debug!("Nexus fee relay worker is being shut down");
                            break;
                        }
                        else => break,
                    }
                }
            }
        });
        Child::new(task, OnShutdown::Wait(Duration::from_secs(2)))
    }
    fn reconcile_once(&self) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        self.mark_accepted_allocations()?;
        self.announce_verified_relays()?;
        self.enqueue_status_relays()?;
        self.submit_pending_relays()?;
        self.refresh_allocations_if_due()?;
        Ok(())
    }
    fn persist_bounded_state(&self, durable: &mut DurableWorkerState) -> Result<()> {
        let _ = prune_durable_worker_state(durable, self.config.max_pending_relays.get());
        persist_durable_state(
            &self.state_path,
            durable,
            self.config.max_pending_relays.get(),
        )
    }
    fn enqueue_status_relays(&self) -> Result<()> {
        let candidates = sumeragi::status::lane_relay_envelopes_snapshot();
        let envelopes = {
            let recorded_relays = self.state.lane_relays.read();
            candidates
                .into_iter()
                .filter_map(|candidate| {
                    authoritative_status_relay(&recorded_relays, &candidate).cloned()
                })
                .collect::<Vec<_>>()
        };
        if envelopes.is_empty() {
            return Ok(());
        }
        let mut durable = self.durable.lock();
        for envelope in envelopes {
            let key = relay_work_key(&envelope);
            validate_relay_work_bounds(&key, &envelope)?;
            if durable.relays.len() >= self.config.max_pending_relays.get()
                && !durable.relays.contains_key(&key)
                && !reclaim_oldest_rejected_relay(&mut durable)
            {
                iroha_logger::warn!(
                    max_pending_relays = self.config.max_pending_relays.get(),
                    "Nexus fee relay worker bounded durable relay set is full of active work; deferring finalized relay enqueue"
                );
                continue;
            }
            if self.verified_relay_exists(&envelope)? {
                continue;
            }
            if !durable.relays.contains_key(&key) {
                durable.relays.insert(
                    key.clone(),
                    DurableRelayWork {
                        envelope,
                        status: DurableWorkStatus::Pending,
                        attempts: 0,
                        last_height: self.committed_height(),
                    },
                );
                if let Err(error) =
                    validate_durable_state_bounds(&durable, self.config.max_pending_relays.get())
                {
                    durable.relays.remove(&key);
                    return Err(error);
                }
            }
        }
        self.persist_bounded_state(&mut durable)
    }
    fn submit_pending_relays(&self) -> Result<()> {
        let keys = {
            let durable = self.durable.lock();
            durable
                .relays
                .iter()
                .filter_map(|(key, work)| match work.status {
                    DurableWorkStatus::Pending
                    | DurableWorkStatus::Proving
                    | DurableWorkStatus::Submitted => Some(key.clone()),
                    DurableWorkStatus::Accepted | DurableWorkStatus::Rejected => None,
                })
                .take(self.config.max_pending_relays.get())
                .collect::<Vec<_>>()
        };
        for key in keys {
            self.submit_relay_work(&key)?;
        }
        Ok(())
    }
    fn submit_relay_work(&self, key: &str) -> Result<()> {
        let Some(mut envelope) = self.relay_work_for_attempt(key)? else {
            return Ok(());
        };
        if self.verified_relay_exists(&envelope)? {
            self.update_relay_status(key, DurableWorkStatus::Accepted, None)?;
            return Ok(());
        }
        if envelope.manifest_root.is_none() {
            envelope.manifest_root = self.manifest_root_for(envelope.dataspace_id);
        }
        let Some(manifest_root) = envelope.manifest_root else {
            iroha_logger::warn!(
                lane_id = envelope.lane_id.as_u32(),
                dataspace_id = envelope.dataspace_id.as_u64(),
                block_height = envelope.block_height,
                "Nexus fee relay worker cannot prove relay without a manifest root"
            );
            return Ok(());
        };
        if manifest_root.iter().all(|byte| *byte == 0) {
            self.reject_relay_attempt(key, envelope, "zero manifest root")?;
            return Ok(());
        }
        self.update_relay_status(key, DurableWorkStatus::Proving, Some(envelope.clone()))?;
        let current_height = self.committed_height();
        let expiry_slot =
            current_height.saturating_add(self.state.view().nexus.axt.replay_retention_slots.get());
        let execution_commitment = match self
            .state
            .authenticated_lane_relay_execution_commitment(&envelope)
        {
            Ok(commitment) => commitment,
            Err(error) => {
                self.reject_or_retry_relay(key, envelope, &eyre::eyre!(error))?;
                return Ok(());
            }
        };
        let (proven_envelope, proof_blob) = match prove_lane_relay_envelope(
            &envelope,
            execution_commitment.parent_state_root,
            execution_commitment.post_state_root,
            expiry_slot,
            current_height,
            &self.fastpq,
        ) {
            Ok(proven) => proven,
            Err(error) => {
                self.reject_or_retry_relay(key, envelope, &error)?;
                return Ok(());
            }
        };
        self.update_relay_status(
            key,
            DurableWorkStatus::Submitted,
            Some(proven_envelope.clone()),
        )?;
        self.submit_instruction(
            InstructionBox::from(RegisterVerifiedLaneRelay {
                envelope: proven_envelope,
                proof_blob,
                effect_proof_blob: None,
            }),
            "/internal/nexus/fee-relay/register-verified-lane-relay",
        )
    }
    fn relay_work_for_attempt(&self, key: &str) -> Result<Option<LaneRelayEnvelope>> {
        let current_height = self.committed_height();
        let mut durable = self.durable.lock();
        let Some(work) = durable.relays.get_mut(key) else {
            return Ok(None);
        };
        match prepare_relay_attempt(work, current_height, self.config.max_retry_attempts.get()) {
            RelayAttemptDecision::Deferred => Ok(None),
            RelayAttemptDecision::Rejected => {
                self.persist_bounded_state(&mut durable)?;
                Ok(None)
            }
            RelayAttemptDecision::Ready(envelope) => {
                self.persist_bounded_state(&mut durable)?;
                Ok(Some(*envelope))
            }
        }
    }
    fn update_relay_status(
        &self,
        key: &str,
        status: DurableWorkStatus,
        envelope: Option<LaneRelayEnvelope>,
    ) -> Result<()> {
        let mut durable = self.durable.lock();
        if status == DurableWorkStatus::Accepted {
            durable.relays.remove(key);
        } else if let Some(work) = durable.relays.get_mut(key) {
            if let Some(envelope) = envelope {
                work.envelope = envelope;
            }
            work.status = status;
            work.last_height = self.committed_height();
        }
        self.persist_bounded_state(&mut durable)
    }
    fn reject_relay_attempt(
        &self,
        key: &str,
        envelope: LaneRelayEnvelope,
        reason: &str,
    ) -> Result<()> {
        iroha_logger::warn!(
            lane_id = envelope.lane_id.as_u32(),
            dataspace_id = envelope.dataspace_id.as_u64(),
            block_height = envelope.block_height,
            reason,
            "Nexus fee relay worker rejected relay proof work"
        );
        self.update_relay_status(key, DurableWorkStatus::Rejected, Some(envelope))
    }
    fn reject_or_retry_relay(
        &self,
        key: &str,
        envelope: LaneRelayEnvelope,
        error: &eyre::Report,
    ) -> Result<()> {
        let mut durable = self.durable.lock();
        let Some(work) = durable.relays.get_mut(key) else {
            return Ok(());
        };
        work.envelope = envelope;
        work.status = if work.attempts >= self.config.max_retry_attempts.get() {
            iroha_logger::warn!(
                ?error,
                attempts = work.attempts,
                "Nexus fee relay worker exhausted relay proof attempts"
            );
            DurableWorkStatus::Rejected
        } else {
            iroha_logger::warn!(
                ?error,
                attempts = work.attempts,
                "Nexus fee relay worker will retry relay proof work"
            );
            DurableWorkStatus::Pending
        };
        work.last_height = self.committed_height();
        self.persist_bounded_state(&mut durable)
    }
    fn announce_verified_relays(&self) -> Result<()> {
        let records = self.verified_relay_records(self.config.max_pending_relays.get())?;
        let current_keys = records
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<BTreeSet<_>>();
        {
            let mut durable = self.durable.lock();
            for key in &current_keys {
                durable.relays.remove(key);
            }
            self.persist_bounded_state(&mut durable)?;
        }
        self.announced_relays
            .lock()
            .retain(|key| current_keys.contains(key));
        for (key, record) in records {
            let mut announced = self.announced_relays.lock();
            if announced.contains(&key) {
                continue;
            }
            if self
                .sumeragi
                .try_incoming_lane_relay(record.relay_envelope.clone())
            {
                announced.insert(key);
            }
        }
        Ok(())
    }
    fn verified_relay_records(
        &self,
        limit: usize,
    ) -> Result<Vec<(String, VerifiedLaneRelayRecord)>> {
        let view = self.state.view();
        let mut newest_records = BTreeMap::new();
        for (key, payload) in view.world().smart_contract_state().iter() {
            let key_text = key.to_string();
            if !key_text.starts_with(VERIFIED_LANE_RELAY_STATE_KEY_PREFIX) {
                continue;
            }
            let json: Json = norito::decode_from_bytes(payload)
                .wrap_err("decode verified lane relay record JSON payload")?;
            let record: VerifiedLaneRelayRecord = norito::json::from_slice(json.get().as_bytes())
                .wrap_err("decode verified lane relay record")?;
            let relay_key = record.relay_ref.relay_state_key();
            newest_records.insert(
                (record.relay_envelope.block_height, relay_key.clone()),
                (relay_key, record),
            );
            if newest_records.len() > limit {
                newest_records.pop_first();
            }
        }
        Ok(newest_records.into_values().collect())
    }
    fn verified_relay_exists(&self, envelope: &LaneRelayEnvelope) -> Result<bool> {
        let key = StatePath::from_str(&envelope.relay_ref().relay_state_key())
            .wrap_err("parse verified lane relay state key")?;
        Ok(self
            .state
            .view()
            .world()
            .smart_contract_state()
            .get(key.as_ref())
            .is_some())
    }
    fn refresh_allocations_if_due(&self) -> Result<()> {
        let current_height = self.committed_height();
        if current_height == 0 {
            return Ok(());
        }
        for candidate in self.allocation_candidates(current_height)? {
            if self
                .latest_verified_allocation_for(&candidate)?
                .is_some_and(|record| current_height <= record.expires_at_height)
            {
                // A lease is a source lock, not a replaceable balance snapshot.
                // Refresh only after expiry so two independently valid proofs
                // can never authorize the same vault capacity concurrently.
                continue;
            }
            let key = allocation_work_key(&candidate);
            let mut work = self.prepare_allocation_work(&key, candidate);
            if self.verified_allocation_for_work(&work)?.is_some() {
                work.status = DurableWorkStatus::Accepted;
                work.last_height = current_height;
                let _ = self.store_allocation_work(key, work)?;
                continue;
            }
            if work.attempts >= self.config.max_retry_attempts.get() {
                work.status = DurableWorkStatus::Rejected;
                let _ = self.store_allocation_work(key, work)?;
                continue;
            }
            work.status = DurableWorkStatus::Proving;
            work.attempts = work.attempts.saturating_add(1);
            work.last_height = current_height;
            if !self.store_allocation_work(key.clone(), work.clone())? {
                continue;
            }
            let proof_blob = match prove_fee_sponsor_vault_allocation(&work, &self.fastpq) {
                Ok(proof) => proof,
                Err(error) => {
                    work.status = if work.attempts >= self.config.max_retry_attempts.get() {
                        DurableWorkStatus::Rejected
                    } else {
                        DurableWorkStatus::Pending
                    };
                    let _ = self.store_allocation_work(key, work)?;
                    iroha_logger::warn!(
                        ?error,
                        "Nexus fee relay worker failed to prove sponsor-program vault allocation"
                    );
                    continue;
                }
            };
            work.status = DurableWorkStatus::Submitted;
            work.proof_blob = Some(proof_blob.clone());
            work.last_height = current_height;
            if !self.store_allocation_work(key, work.clone())? {
                continue;
            }
            self.submit_instruction(
                InstructionBox::from(RegisterVerifiedFeeSponsorVaultAllocation {
                    program_id: work.program_id,
                    program_revision: work.program_revision,
                    asset_definition_id: work.asset_definition_id,
                    verified_allocation: work.verified_allocation,
                    source_dataspace_id: work.source_dataspace_id,
                    source_height: work.source_height,
                    source_state_root: work.source_state_root,
                    expires_at_height: work.expires_at_height,
                    lease_id: work.lease_id,
                    manifest_root: work.manifest_root,
                    proof_blob,
                }),
                "/internal/nexus/fee-relay/register-verified-fee-sponsor-vault-allocation",
            )?;
        }
        Ok(())
    }
    fn allocation_candidates(&self, current_height: u64) -> Result<Vec<DurableAllocationWork>> {
        let view = self.state.view();
        let replay_retention_slots = view.nexus.axt.replay_retention_slots.get();
        let mut candidates = Vec::new();
        for (program_id, program) in view.world().fee_sponsor_programs().iter() {
            if program.lifecycle != FeeSponsorProgramLifecycle::Active {
                continue;
            }
            let Some(program_revision) = program.active_revision else {
                continue;
            };
            let Some(expiry_height) = fee_sponsor_allocation_expiry_height(
                current_height,
                replay_retention_slots,
                program
                    .scheduled_activation
                    .map(|activation| activation.activate_at_height),
            ) else {
                // A worker submission cannot execute before the scheduled
                // switch, so an old-revision proof would be stale on arrival.
                continue;
            };
            let revision_key =
                FeeSponsorProgramRevisionKey::new(program_id.clone(), program_revision);
            let Some(revision) = view
                .world()
                .fee_sponsor_program_revisions()
                .get(&revision_key)
            else {
                iroha_logger::warn!(
                    program_id = %program_id,
                    program_revision,
                    "active fee sponsor program revision is missing; allocation refresh skipped"
                );
                continue;
            };
            let routes = self.eligible_allocation_routes(&view, program_id, revision.eligibility);
            if routes.is_empty() {
                iroha_logger::warn!(
                    program_id = %program_id,
                    "active fee sponsor program has no eligible dataspace with a non-zero AXT manifest root"
                );
                continue;
            }
            let plan = AllocationCandidatePlanV1 {
                program_id,
                program_revision,
                current_height,
                expiry_height,
                routes: &routes,
            };
            for budget in &revision.asset_budgets {
                candidates.extend(Self::allocation_candidates_for_budget(
                    &view, &plan, budget,
                )?);
            }
        }
        Ok(candidates)
    }
    fn eligible_allocation_routes(
        &self,
        view: &StateView<'_>,
        program_id: &FeeSponsorProgramId,
        eligibility: FeeSponsorEligibility,
    ) -> Vec<(DataSpaceId, [u8; 32])> {
        let has_enrollment = view
            .world()
            .fee_sponsor_enrollments()
            .iter()
            .any(|(key, _)| &key.program_id == program_id);
        let mut routes = view
            .nexus
            .dataspace_catalog
            .entries()
            .iter()
            .filter_map(|entry| {
                let route_default =
                    view.nexus.dataspace_fee_sponsor_program_ids.get(&entry.id) == Some(program_id);
                fee_sponsor_route_allocation_eligible(has_enrollment, eligibility, route_default)
                    .then(|| {
                        self.manifest_root_for(entry.id)
                            .map(|root| (entry.id, root))
                    })
                    .flatten()
            })
            .collect::<Vec<_>>();
        routes.sort_by_key(|(dataspace_id, _)| *dataspace_id);
        routes
    }
    fn allocation_candidates_for_budget(
        view: &StateView<'_>,
        plan: &AllocationCandidatePlanV1<'_>,
        budget: &FeeSponsorAssetBudget,
    ) -> Result<Vec<DurableAllocationWork>> {
        let vault_key = FeeSponsorVaultKey {
            program_id: plan.program_id.clone(),
            asset_definition_id: budget.asset_definition_id.clone(),
        };
        let Some(vault) = view.world().fee_sponsor_vaults().get(&vault_key) else {
            return Ok(Vec::new());
        };
        if vault.balance.is_zero() {
            return Ok(Vec::new());
        }
        let output_scale = view
            .world()
            .asset_definitions()
            .get(&budget.asset_definition_id)
            .and_then(|definition| definition.spec().scale())
            .unwrap_or(MAX_DECIMAL_SCALE);
        let allocations =
            partition_fee_sponsor_vault(&vault.balance, plan.routes.len(), output_scale)?;
        let mut candidates = Vec::new();
        for ((source_dataspace_id, manifest_root), verified_allocation) in
            plan.routes.iter().copied().zip(allocations)
        {
            if verified_allocation.is_zero() {
                continue;
            }
            let source_state_root = fee_sponsor_vault_source_state_root(
                plan.program_id,
                plan.program_revision,
                &budget.asset_definition_id,
                &vault.balance,
                source_dataspace_id,
                plan.current_height,
            );
            let lease_id = fee_sponsor_vault_lease_id(
                plan.program_id,
                plan.program_revision,
                &budget.asset_definition_id,
                source_dataspace_id,
                plan.current_height,
                source_state_root,
                plan.expiry_height,
            )?;
            candidates.push(DurableAllocationWork {
                program_id: plan.program_id.clone(),
                program_revision: plan.program_revision,
                asset_definition_id: budget.asset_definition_id.clone(),
                verified_allocation,
                source_dataspace_id,
                source_height: plan.current_height,
                source_state_root,
                expires_at_height: plan.expiry_height,
                lease_id,
                manifest_root,
                proof_blob: None,
                status: DurableWorkStatus::Pending,
                attempts: 0,
                last_height: plan.current_height,
            });
        }
        Ok(candidates)
    }
    fn prepare_allocation_work(
        &self,
        key: &str,
        candidate: DurableAllocationWork,
    ) -> DurableAllocationWork {
        self.durable
            .lock()
            .allocations
            .get(key)
            .filter(|work| {
                matches!(
                    work.status,
                    DurableWorkStatus::Pending
                        | DurableWorkStatus::Proving
                        | DurableWorkStatus::Submitted
                ) && work.expires_at_height <= candidate.expires_at_height
            })
            .cloned()
            .unwrap_or(candidate)
    }
    fn store_allocation_work(&self, key: String, work: DurableAllocationWork) -> Result<bool> {
        validate_allocation_work_bounds(&key, &work)?;
        let mut durable = self.durable.lock();
        if matches!(
            work.status,
            DurableWorkStatus::Accepted | DurableWorkStatus::Rejected
        ) {
            durable.allocations.remove(&key);
            self.persist_bounded_state(&mut durable)?;
            return Ok(true);
        }
        if !durable.allocations.contains_key(&key)
            && durable.allocations.len() >= self.config.max_pending_relays.get()
        {
            iroha_logger::warn!(
                max_pending_items = self.config.max_pending_relays.get(),
                "Nexus fee relay worker bounded durable allocation set is full; deferring new allocation work"
            );
            return Ok(false);
        }
        let previous = durable.allocations.insert(key.clone(), work);
        if let Err(error) =
            validate_durable_state_bounds(&durable, self.config.max_pending_relays.get())
        {
            match previous {
                Some(previous) => {
                    durable.allocations.insert(key, previous);
                }
                None => {
                    durable.allocations.remove(&key);
                }
            }
            return Err(error);
        }
        self.persist_bounded_state(&mut durable)?;
        Ok(true)
    }
    fn mark_accepted_allocations(&self) -> Result<()> {
        let works = self.durable.lock().allocations.clone();
        let mut accepted = Vec::new();
        for (key, work) in works {
            if self.verified_allocation_for_work(&work)?.is_some() {
                accepted.push((key, work));
            }
        }
        if accepted.is_empty() {
            return Ok(());
        }
        let mut durable = self.durable.lock();
        for (key, _) in accepted {
            durable.allocations.remove(&key);
        }
        self.persist_bounded_state(&mut durable)
    }
    fn verified_allocation_for_work(
        &self,
        work: &DurableAllocationWork,
    ) -> Result<Option<VerifiedFeeSponsorVaultAllocation>> {
        let key = StatePath::from_str(&VerifiedFeeSponsorVaultAllocation::state_key_for(
            &work.program_id,
            &work.asset_definition_id,
            &work.lease_id,
        ))
        .wrap_err("parse verified fee sponsor vault allocation state key")?;
        let view = self.state.view();
        let Some(payload) = view.world().smart_contract_state().get(key.as_ref()) else {
            return Ok(None);
        };
        decode_verified_allocation_record(payload).map(Some)
    }
    fn latest_verified_allocation_for(
        &self,
        work: &DurableAllocationWork,
    ) -> Result<Option<VerifiedFeeSponsorVaultAllocation>> {
        let view = self.state.view();
        let mut latest = None;
        for (key, payload) in view.world().smart_contract_state().iter() {
            if !key
                .to_string()
                .starts_with(VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX)
            {
                continue;
            }
            let record = decode_verified_allocation_record(payload)?;
            if record.program_id == work.program_id
                && record.program_revision == work.program_revision
                && record.asset_definition_id == work.asset_definition_id
                && record.source_dataspace_id == work.source_dataspace_id
                && latest
                    .as_ref()
                    .is_none_or(|current: &VerifiedFeeSponsorVaultAllocation| {
                        record.verified_at_height > current.verified_at_height
                    })
            {
                latest = Some(record);
            }
        }
        Ok(latest)
    }
    fn manifest_root_for(&self, dsid: DataSpaceId) -> Option<[u8; 32]> {
        self.state
            .axt_policy_snapshot()
            .entries
            .iter()
            .find(|entry| entry.dsid == dsid)
            .map(|entry| entry.policy.manifest_root)
            .filter(|root| root.iter().any(|byte| *byte != 0))
    }
    fn committed_height(&self) -> u64 {
        u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX)
    }
    fn submit_instruction(
        &self,
        instruction: InstructionBox,
        endpoint: &'static str,
    ) -> Result<()> {
        let tx = sign_nexus_fee_relay_submission_transaction(
            *self.state.network_id_ref(),
            self.authority.clone(),
            instruction,
            worker_submission_metadata(endpoint),
            &self.key_pair,
            endpoint,
        )?;
        let view = self.state.view();
        let params = view.world().parameters();
        let accepted = AcceptedTransaction::accept(
            tx,
            self.state.network_id_ref(),
            params.sumeragi().max_clock_drift(),
            params.transaction(),
            self.state.crypto().as_ref(),
        )
        .wrap_err_with(|| format!("accept internal Nexus fee relay mutation at `{endpoint}`"))?;
        drop(view);
        self.queue
            .push_with_lane_with_state(accepted, self.state.as_ref())
            .map(|_| ())
            .map_err(|failure| {
                eyre::eyre!(
                    "enqueue internal Nexus fee relay mutation at `{endpoint}`: {}",
                    failure.err
                )
            })
    }
}
fn prepare_relay_attempt(
    work: &mut DurableRelayWork,
    current_height: u64,
    max_retry_attempts: u32,
) -> RelayAttemptDecision {
    if current_height < work.envelope.block_header.height().get() {
        return RelayAttemptDecision::Deferred;
    }
    if work.attempts >= max_retry_attempts {
        work.status = DurableWorkStatus::Rejected;
        return RelayAttemptDecision::Rejected;
    }
    work.attempts = work.attempts.saturating_add(1);
    work.last_height = current_height;
    RelayAttemptDecision::Ready(Box::new(work.envelope.clone()))
}
fn sign_nexus_fee_relay_submission_transaction(
    network_id: iroha_data_model::NetworkId,
    authority: AccountId,
    instruction: InstructionBox,
    metadata: Metadata,
    key_pair: &KeyPair,
    endpoint: &'static str,
) -> Result<SignedTransaction> {
    TransactionBuilder::new(
        network_id,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .with_metadata(metadata)
    .try_sign(key_pair.private_key())
    .wrap_err_with(|| format!("sign internal Nexus fee relay mutation at `{endpoint}`"))
}
fn prune_durable_worker_state(durable: &mut DurableWorkerState, max_items_per_kind: usize) -> bool {
    let original_relay_count = durable.relays.len();
    let original_allocation_count = durable.allocations.len();
    durable
        .relays
        .retain(|_, work| work.status != DurableWorkStatus::Accepted);
    durable.allocations.retain(|_, work| {
        !matches!(
            work.status,
            DurableWorkStatus::Accepted | DurableWorkStatus::Rejected
        )
    });
    let relay_overflow = durable.relays.len().saturating_sub(max_items_per_kind);
    if relay_overflow > 0 {
        let mut eviction_order = durable
            .relays
            .iter()
            .map(|(key, work)| {
                (
                    work.status != DurableWorkStatus::Rejected,
                    work.last_height,
                    key.clone(),
                )
            })
            .collect::<Vec<_>>();
        eviction_order.sort_unstable();
        for (_, _, key) in eviction_order.into_iter().take(relay_overflow) {
            durable.relays.remove(&key);
        }
    }
    let allocation_overflow = durable.allocations.len().saturating_sub(max_items_per_kind);
    if allocation_overflow > 0 {
        let mut eviction_order = durable
            .allocations
            .iter()
            .map(|(key, work)| (work.last_height, key.clone()))
            .collect::<Vec<_>>();
        eviction_order.sort_unstable();
        for (_, key) in eviction_order.into_iter().take(allocation_overflow) {
            durable.allocations.remove(&key);
        }
    }
    durable.relays.len() != original_relay_count
        || durable.allocations.len() != original_allocation_count
}
fn reclaim_oldest_rejected_relay(durable: &mut DurableWorkerState) -> bool {
    let rejected_key = durable
        .relays
        .iter()
        .filter(|(_, work)| work.status == DurableWorkStatus::Rejected)
        .min_by(|(left_key, left), (right_key, right)| {
            (left.last_height, left_key.as_str()).cmp(&(right.last_height, right_key.as_str()))
        })
        .map(|(key, _)| key.clone());
    rejected_key
        .and_then(|key| durable.relays.remove(&key))
        .is_some()
}
fn relay_work_key(envelope: &LaneRelayEnvelope) -> String {
    envelope.relay_ref().relay_state_key()
}
fn authoritative_status_relay<'a>(
    recorded_relays: &'a LaneRelayStore,
    candidate: &LaneRelayEnvelope,
) -> Option<&'a LaneRelayEnvelope> {
    let recorded = recorded_relays.get(
        candidate.lane_id,
        candidate.dataspace_id,
        candidate.lane_incarnation,
        candidate.block_height,
    )?;
    (recorded.finality_authority.is_some() && recorded == candidate).then_some(recorded)
}
fn allocation_work_key(work: &DurableAllocationWork) -> String {
    format!(
        "{}|{}|{}|{}",
        work.source_dataspace_id.as_u64(),
        work.program_id,
        work.program_revision,
        work.asset_definition_id,
    )
}
fn fee_sponsor_route_allocation_eligible(
    has_enrollment: bool,
    eligibility: FeeSponsorEligibility,
    route_default: bool,
) -> bool {
    has_enrollment
        || (eligibility == FeeSponsorEligibility::EnrolledOrRouteDefault && route_default)
}
fn fee_sponsor_allocation_expiry_height(
    current_height: u64,
    replay_retention_slots: u64,
    scheduled_activation_height: Option<u64>,
) -> Option<u64> {
    let normal_expiry = current_height.saturating_add(replay_retention_slots);
    let Some(activation_height) = scheduled_activation_height else {
        return Some(normal_expiry);
    };
    let earliest_execution_height = current_height.checked_add(1)?;
    if earliest_execution_height >= activation_height {
        return None;
    }
    Some(normal_expiry.min(activation_height.checked_sub(1)?))
}
fn partition_fee_sponsor_vault(
    vault_balance: &Quantity,
    route_count: usize,
    output_scale: u32,
) -> Result<Vec<Quantity>> {
    let route_count_u64 =
        u64::try_from(route_count).wrap_err("fee sponsor allocation route count exceeds u64")?;
    if route_count_u64 == 0 {
        return Ok(Vec::new());
    }
    let share = vault_balance
        .try_div_decimal_round(
            &Numeric::from(route_count_u64),
            output_scale,
            RoundingMode::TowardZero,
        )
        .wrap_err("partition fee sponsor vault across eligible dataspaces")?;
    let mut allocations = Vec::with_capacity(route_count);
    let mut allocated = Quantity::zero();
    for index in 0..route_count {
        let allocation = if index + 1 == route_count {
            vault_balance
                .checked_sub(&allocated)
                .wrap_err("compute final fee sponsor vault allocation remainder")?
        } else {
            share.clone()
        };
        allocated = allocated
            .checked_add(&allocation)
            .wrap_err("sum partitioned fee sponsor vault allocation")?;
        allocations.push(allocation);
    }
    debug_assert_eq!(&allocated, vault_balance);
    Ok(allocations)
}
fn fee_sponsor_vault_lease_id(
    program_id: &FeeSponsorProgramId,
    program_revision: u64,
    asset_definition_id: &AssetDefinitionId,
    source_dataspace_id: DataSpaceId,
    source_height: u64,
    source_state_root: Hash,
    expires_at_height: u64,
) -> Result<Hash> {
    let binding = FeeSponsorVaultLeaseBinding {
        version: 1,
        program_id: program_id.clone(),
        program_revision,
        asset_definition_id: asset_definition_id.clone(),
        source_dataspace_id,
        source_height,
        source_state_root,
        expires_at_height,
    };
    let encoded =
        norito::to_bytes(&binding).wrap_err("encode fee sponsor vault spend-lease binding")?;
    Ok(Hash::new(encoded))
}
fn decode_verified_allocation_record(payload: &[u8]) -> Result<VerifiedFeeSponsorVaultAllocation> {
    let json: Json = norito::decode_from_bytes(payload)
        .wrap_err("decode verified fee sponsor vault allocation JSON payload")?;
    norito::json::from_slice(json.get().as_bytes())
        .wrap_err("decode verified fee sponsor vault allocation")
}
fn validate_worker_item_limit(max_items_per_kind: usize) -> Result<()> {
    if max_items_per_kind == 0 || max_items_per_kind > WORKER_STATE_MAX_ITEMS_PER_KIND {
        eyre::bail!(
            "nexus.relay_worker.max_pending_relays must be within 1..={WORKER_STATE_MAX_ITEMS_PER_KIND}, got {max_items_per_kind}"
        );
    }
    Ok(())
}
fn validate_relay_work_bounds(key: &str, envelope: &LaneRelayEnvelope) -> Result<()> {
    validate_relay_resource_lengths(key.len(), envelope.encoded_len())
}
fn validate_relay_resource_lengths(key_bytes: usize, envelope_bytes: usize) -> Result<()> {
    if key_bytes > WORKER_STATE_MAX_KEY_BYTES {
        eyre::bail!(
            "Nexus fee relay key is {key_bytes} bytes (maximum {WORKER_STATE_MAX_KEY_BYTES})"
        );
    }
    if envelope_bytes > WORKER_STATE_MAX_RELAY_BYTES {
        eyre::bail!(
            "Nexus fee relay envelope is {envelope_bytes} bytes (maximum {WORKER_STATE_MAX_RELAY_BYTES})"
        );
    }
    Ok(())
}
fn validate_allocation_work_bounds(key: &str, work: &DurableAllocationWork) -> Result<()> {
    validate_allocation_resource_lengths(
        key.len(),
        work.proof_blob
            .as_ref()
            .map_or(0, |proof| proof.payload.len()),
    )
}
fn validate_allocation_resource_lengths(key_bytes: usize, proof_bytes: usize) -> Result<()> {
    if key_bytes > WORKER_STATE_MAX_KEY_BYTES {
        eyre::bail!(
            "Nexus fee relay allocation key is {} bytes (maximum {WORKER_STATE_MAX_KEY_BYTES})",
            key_bytes
        );
    }
    if proof_bytes > WORKER_STATE_MAX_PROOF_BYTES {
        eyre::bail!(
            "Nexus fee relay allocation proof is {} bytes (maximum {WORKER_STATE_MAX_PROOF_BYTES})",
            proof_bytes
        );
    }
    Ok(())
}
fn validate_durable_state_bounds(
    durable: &DurableWorkerState,
    max_items_per_kind: usize,
) -> Result<()> {
    validate_worker_item_limit(max_items_per_kind)?;
    if durable.relays.len() > max_items_per_kind {
        eyre::bail!(
            "Nexus fee relay journal contains {} relay items (configured maximum {max_items_per_kind})",
            durable.relays.len()
        );
    }
    if durable.allocations.len() > max_items_per_kind {
        eyre::bail!(
            "Nexus fee relay journal contains {} allocation items (configured maximum {max_items_per_kind})",
            durable.allocations.len()
        );
    }
    for (key, work) in &durable.relays {
        validate_relay_work_bounds(key, &work.envelope)?;
    }
    let mut total_proof_bytes = 0usize;
    for (key, work) in &durable.allocations {
        validate_allocation_work_bounds(key, work)?;
        if let Some(proof) = work.proof_blob.as_ref() {
            total_proof_bytes = total_proof_bytes
                .checked_add(proof.payload.len())
                .ok_or_else(|| eyre::eyre!("Nexus fee relay proof-byte total overflowed"))?;
        }
    }
    if total_proof_bytes > WORKER_STATE_MAX_TOTAL_PROOF_BYTES {
        eyre::bail!(
            "Nexus fee relay journal retains {total_proof_bytes} proof bytes (maximum {WORKER_STATE_MAX_TOTAL_PROOF_BYTES})"
        );
    }
    let encoded_bytes = durable.encoded_len();
    if encoded_bytes > WORKER_STATE_MAX_BYTES {
        eyre::bail!(
            "encoded Nexus fee relay worker state is {encoded_bytes} bytes (maximum {WORKER_STATE_MAX_BYTES})"
        );
    }
    Ok(())
}
fn worker_state_decode_limits() -> DecodeLimits {
    DecodeLimits::new(
        WORKER_STATE_MAX_PROOF_BYTES,
        WORKER_STATE_MAX_PROOF_BYTES,
        WORKER_STATE_MAX_BYTES,
        WORKER_STATE_MAX_DECODE_ALLOCATED_BYTES,
        WORKER_STATE_MAX_DECODE_DEPTH,
    )
}
fn load_durable_state(path: &Path, max_items_per_kind: usize) -> Result<DurableWorkerState> {
    validate_worker_item_limit(max_items_per_kind)?;
    let initial = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DurableWorkerState::default());
        }
        Err(error) => return Err(error).wrap_err_with(|| format!("inspect {}", path.display())),
    };
    if !initial.file_type().is_file()
        || initial.len() > u64::try_from(WORKER_STATE_MAX_BYTES).unwrap_or(u64::MAX)
    {
        eyre::bail!(
            "Nexus fee relay worker state at {} is not a regular file within the {WORKER_STATE_MAX_BYTES}-byte limit",
            path.display()
        );
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        let nofollow = i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits())
            .expect("NOFOLLOW flag bits fit the platform custom-flags type");
        options.custom_flags(nofollow);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options
        .open(path)
        .wrap_err_with(|| format!("open {}", path.display()))?;
    let opened = file
        .metadata()
        .wrap_err_with(|| format!("inspect opened {}", path.display()))?;
    if !opened.is_file() || !same_worker_state_snapshot(&initial, &opened) {
        eyre::bail!(
            "Nexus fee relay worker state at {} changed while opening",
            path.display()
        );
    }
    let capacity = usize::try_from(opened.len())
        .map_err(|_| eyre::eyre!("Nexus fee relay state length is not addressable"))?;
    let mut bytes = Vec::with_capacity(capacity);
    Read::by_ref(&mut file)
        .take(
            u64::try_from(WORKER_STATE_MAX_BYTES)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        )
        .read_to_end(&mut bytes)
        .wrap_err_with(|| format!("read {}", path.display()))?;
    if bytes.len() > WORKER_STATE_MAX_BYTES {
        eyre::bail!(
            "Nexus fee relay worker state at {} grew beyond its {WORKER_STATE_MAX_BYTES}-byte limit",
            path.display()
        );
    }
    let after_read = file
        .metadata()
        .wrap_err_with(|| format!("re-inspect opened {}", path.display()))?;
    let current =
        fs::symlink_metadata(path).wrap_err_with(|| format!("re-inspect {}", path.display()))?;
    if !current.file_type().is_file()
        || bytes.len() != capacity
        || !same_worker_state_snapshot(&opened, &after_read)
        || !same_worker_state_snapshot(&after_read, &current)
    {
        eyre::bail!(
            "Nexus fee relay worker state at {} changed while reading",
            path.display()
        );
    }
    let durable: DurableWorkerState =
        norito::decode_from_bytes_with_limits(&bytes, worker_state_decode_limits())
            .wrap_err_with(|| format!("decode {}", path.display()))?;
    // A configuration decrease is allowed to load an older, still
    // protocol-bounded journal; the constructor deterministically prunes it to
    // the configured limit before the worker starts or persists it again.
    validate_durable_state_bounds(&durable, WORKER_STATE_MAX_ITEMS_PER_KIND)?;
    Ok(durable)
}
fn persist_durable_state(
    path: &Path,
    durable: &DurableWorkerState,
    max_items_per_kind: usize,
) -> Result<()> {
    validate_durable_state_bounds(durable, max_items_per_kind)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
    }
    let bytes = norito::to_bytes(durable).wrap_err("encode Nexus fee relay worker state")?;
    if bytes.len() > WORKER_STATE_MAX_BYTES {
        eyre::bail!(
            "encoded Nexus fee relay worker state is {} bytes (maximum {WORKER_STATE_MAX_BYTES})",
            bytes.len()
        );
    }
    let tmp = path.with_extension("norito.tmp");
    fs::write(&tmp, bytes).wrap_err_with(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path).wrap_err_with(|| format!("replace {}", path.display()))
}
#[cfg(unix)]
fn same_worker_state_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
#[cfg(not(unix))]
fn same_worker_state_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}
fn parse_canonical_account_id(raw: &str) -> Result<AccountId> {
    AccountId::parse_encoded(raw)
        .map(ParsedAccountId::into_account_id)
        .map_err(|error| eyre::eyre!("{error}"))
}
fn worker_submission_metadata(endpoint: &'static str) -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("nexus_protocol_worker").expect("static metadata key"),
        Json::new("fee_relay_v1"),
    );
    metadata.insert(
        Name::from_str("nexus_protocol_endpoint").expect("static metadata key"),
        Json::new(endpoint),
    );
    metadata
}
fn prove_lane_relay_envelope(
    envelope: &LaneRelayEnvelope,
    parent_state_root: Hash,
    post_state_root: Hash,
    expiry_slot: u64,
    verified_at_height: u64,
    fastpq: &Fastpq,
) -> Result<(LaneRelayEnvelope, ProofBlob)> {
    let manifest_root = envelope
        .manifest_root
        .ok_or_else(|| eyre::eyre!("lane relay envelope missing manifest_root"))?;
    if manifest_root.iter().all(|byte| *byte == 0) {
        eyre::bail!("lane relay envelope has zero manifest_root");
    }
    envelope
        .validate_finality_authority_ref()
        .wrap_err("lane relay proof requires global finality authority")?;
    let lane_finality_statement_hash = envelope
        .lane_finality_statement_hash()
        .wrap_err("derive finalized lane relay statement")?;
    let relay_ref = envelope.relay_ref();
    let relay_ref_bytes = norito::to_bytes(&relay_ref).wrap_err("encode lane relay ref")?;
    let source_tx_commitment = worker_digest(
        b"nexus-fee-relay:lane-relay-source-tx:v1",
        &[&relay_ref_bytes],
    );
    let claim_digest =
        lane_relay_fastpq_claim_digest(envelope).wrap_err("compute lane relay claim digest")?;
    let witness_commitment = worker_digest(
        b"nexus-fee-relay:lane-relay-witness:v1",
        &[envelope.settlement_hash.as_ref()],
    );
    let policy_commitment =
        worker_digest(b"nexus-fee-relay:lane-relay-policy:v1", &[&manifest_root]);
    let dsid = envelope.dataspace_id;
    let binding = AxtFastpqBinding {
        parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
        source_dsid: dsid.as_u64(),
        source_dataspace: format!("dataspace-{}", dsid.as_u64()),
        source_receipt_id: format!("relay-{}", hex::encode(&relay_ref_bytes)),
        source_tx_commitment: hex::encode(source_tx_commitment.as_ref()),
        claim_type: "authorization".to_owned(),
        claim_digest: hex::encode(claim_digest.as_ref()),
        witness_commitment: hex::encode(witness_commitment.as_ref()),
        policy_commitment: hex::encode(policy_commitment.as_ref()),
        verified_effect_type: LANE_RELAY_FASTPQ_EFFECT_TYPE.to_owned(),
        corridor: "nexus-fee-relay".to_owned(),
        verifier_id: "fastpq".to_owned(),
        verifier_version: "v1".to_owned(),
        target_dsids: vec![DataSpaceId::UNIVERSAL.as_u64()],
        effect_binding: None,
    };
    let mut batch = transition_batch(
        dsid,
        expiry_slot,
        parent_state_root,
        post_state_root,
        worker_digest(
            b"nexus-fee-relay:lane-relay-perm-root:v1",
            &[&manifest_root],
        ),
        lane_finality_statement_hash,
    );
    batch.push(fastpq_prover::StateTransition::new(
        b"axt/nexus/lane-relay".to_vec(),
        relay_ref_bytes.clone(),
        claim_digest.as_ref().to_vec(),
        fastpq_prover::OperationKind::MetaSet,
    ));
    batch.sort();
    batch.metadata.insert(
        "entry_hash".to_owned(),
        source_tx_commitment.as_ref().to_vec(),
    );
    fastpq_prover::bind_axt_batch(&mut batch, &binding).wrap_err("bind lane relay AXT batch")?;
    let proof = prover_from_config(fastpq)?
        .prove(&batch)
        .wrap_err("prove lane relay AXT batch")?;
    let payload =
        fastpq_prover::encode_axt_fastpq_payload(&batch, proof).wrap_err("encode AXT payload")?;
    let proof_envelope = AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: envelope
            .da_commitment_hash
            .map(|commitment| Hash::from(commitment).into()),
        proof: payload,
        fastpq_binding: Some(binding),
        committed_amount: None,
        amount_commitment: None,
    };
    let proof_blob = ProofBlob {
        payload: norito::to_bytes(&proof_envelope).wrap_err("encode lane relay proof envelope")?,
        expiry_slot: Some(expiry_slot),
    };
    let proven_envelope =
        envelope
            .clone()
            .with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
                proof_digest: Hash::new(proof_blob.payload.as_slice()),
                verified_at_height,
            }));
    Ok((proven_envelope, proof_blob))
}
fn prove_fee_sponsor_vault_allocation(
    work: &DurableAllocationWork,
    fastpq: &Fastpq,
) -> Result<ProofBlob> {
    if work.manifest_root.iter().all(|byte| *byte == 0) {
        eyre::bail!("fee sponsor vault allocation proof manifest_root is zero");
    }
    if work.program_revision == 0
        || work.verified_allocation.is_zero()
        || work.source_height == 0
        || work.expires_at_height < work.source_height
    {
        eyre::bail!("fee sponsor vault allocation proof inputs are invalid");
    }
    let claim = FeeSponsorVaultAllocationClaim {
        program_id: work.program_id.clone(),
        program_revision: work.program_revision,
        asset_definition_id: work.asset_definition_id.clone(),
        verified_allocation: work.verified_allocation.clone(),
        source_dataspace_id: work.source_dataspace_id,
        source_height: work.source_height,
        source_state_root: work.source_state_root,
        expires_at_height: work.expires_at_height,
        lease_id: work.lease_id,
    };
    let claim_bytes = norito::to_bytes(&claim).wrap_err("encode sponsor vault allocation claim")?;
    let source_tx_commitment = worker_digest(
        b"nexus-fee-relay:sponsor-vault-source-tx:v1",
        &[claim_bytes.as_slice()],
    );
    let claim_digest = fee_sponsor_vault_allocation_claim_digest(&claim);
    let witness_commitment = worker_digest(
        b"nexus-fee-relay:sponsor-vault-witness:v1",
        &[work.source_state_root.as_ref(), work.lease_id.as_ref()],
    );
    let policy_commitment = worker_digest(
        b"nexus-fee-relay:sponsor-vault-policy:v1",
        &[&work.manifest_root],
    );
    let program_text = work.program_id.to_string();
    let binding = fee_sponsor_vault_allocation_binding(
        work,
        &program_text,
        &source_tx_commitment,
        &claim_digest,
        &witness_commitment,
        &policy_commitment,
    );
    let mut batch = transition_batch(
        work.source_dataspace_id,
        work.expires_at_height,
        work.source_state_root,
        work.source_state_root,
        worker_digest(
            b"nexus-fee-relay:sponsor-vault-perm-root:v1",
            &[program_text.as_bytes()],
        ),
        worker_digest(
            b"nexus-fee-relay:sponsor-vault-tx-set:v1",
            &[claim_digest.as_ref()],
        ),
    );
    batch.push(fastpq_prover::StateTransition::new(
        b"axt/nexus/fee-sponsor-vault-allocation".to_vec(),
        work.lease_id.as_ref().to_vec(),
        claim_digest.as_ref().to_vec(),
        fastpq_prover::OperationKind::MetaSet,
    ));
    batch.sort();
    batch.metadata.insert(
        "entry_hash".to_owned(),
        source_tx_commitment.as_ref().to_vec(),
    );
    fastpq_prover::bind_axt_batch(&mut batch, &binding)
        .wrap_err("bind fee sponsor vault allocation AXT batch")?;
    let proof = prover_from_config(fastpq)?
        .prove(&batch)
        .wrap_err("prove fee sponsor vault allocation AXT batch")?;
    let payload =
        fastpq_prover::encode_axt_fastpq_payload(&batch, proof).wrap_err("encode AXT payload")?;
    let proof_envelope = AxtProofEnvelope {
        dsid: work.source_dataspace_id,
        manifest_root: work.manifest_root,
        da_commitment: None,
        proof: payload,
        fastpq_binding: Some(binding),
        committed_amount: integer_mantissa(&work.verified_allocation),
        amount_commitment: None,
    };
    Ok(ProofBlob {
        payload: norito::to_bytes(&proof_envelope)
            .wrap_err("encode fee sponsor vault allocation proof envelope")?,
        expiry_slot: Some(work.expires_at_height),
    })
}
fn fee_sponsor_vault_allocation_binding(
    work: &DurableAllocationWork,
    program_text: &str,
    source_tx_commitment: &Hash,
    claim_digest: &Hash,
    witness_commitment: &Hash,
    policy_commitment: &Hash,
) -> AxtFastpqBinding {
    AxtFastpqBinding {
        parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
        source_dsid: work.source_dataspace_id.as_u64(),
        source_dataspace: format!("dataspace-{}", work.source_dataspace_id.as_u64()),
        source_receipt_id: format!("fee-sponsor-vault-{}", hex::encode(work.lease_id.as_ref())),
        source_tx_commitment: hex::encode(source_tx_commitment.as_ref()),
        claim_type: "authorization".to_owned(),
        claim_digest: hex::encode(claim_digest.as_ref()),
        witness_commitment: hex::encode(witness_commitment.as_ref()),
        policy_commitment: hex::encode(policy_commitment.as_ref()),
        verified_effect_type: FEE_SPONSOR_VAULT_ALLOCATION_EFFECT_TYPE.to_owned(),
        corridor: format!("fee-sponsor-program:{program_text}"),
        verifier_id: "fastpq".to_owned(),
        verifier_version: "v1".to_owned(),
        target_dsids: vec![DataSpaceId::UNIVERSAL.as_u64()],
        effect_binding: Some(AxtEffectBinding {
            destination_domain: None,
            destination_account_id: Some(work.program_id.sponsor.to_string()),
            vault_account_id: None,
            issuance_account_id: None,
            source_asset_definition_id: Some(work.asset_definition_id.to_string()),
            destination_asset_definition_id: None,
            source_amount_i64: None,
            destination_amount_i64: None,
        }),
    }
}
fn transition_batch(
    dsid: DataSpaceId,
    slot: u64,
    old_root: Hash,
    new_root: Hash,
    perm_root: Hash,
    tx_set_hash: Hash,
) -> fastpq_prover::TransitionBatch {
    let mut dsid_bytes = [0_u8; 16];
    dsid_bytes[..8].copy_from_slice(&dsid.as_u64().to_le_bytes());
    fastpq_prover::TransitionBatch::new(
        fastpq_prover::AXT_DEFAULT_PARAMETER,
        fastpq_prover::PublicInputs {
            dsid: dsid_bytes,
            slot,
            old_root: old_root.into(),
            new_root: new_root.into(),
            perm_root: perm_root.into(),
            tx_set_hash: tx_set_hash.into(),
        },
    )
}
fn prover_from_config(fastpq: &Fastpq) -> Result<fastpq_prover::Prover> {
    fastpq_prover::Prover::canonical_with_modes(
        fastpq_prover::AXT_DEFAULT_PARAMETER,
        map_execution_mode(fastpq.execution_mode),
        map_poseidon_mode(fastpq.poseidon_mode),
    )
    .wrap_err("initialise FastPQ prover")
}
fn map_execution_mode(mode: FastpqExecutionMode) -> fastpq_prover::ExecutionMode {
    match mode {
        FastpqExecutionMode::Cpu => fastpq_prover::ExecutionMode::Cpu,
        FastpqExecutionMode::Gpu => fastpq_prover::ExecutionMode::Gpu,
    }
}
fn map_poseidon_mode(mode: FastpqPoseidonMode) -> fastpq_prover::PoseidonExecutionMode {
    match mode {
        FastpqPoseidonMode::Cpu => fastpq_prover::PoseidonExecutionMode::Cpu,
        FastpqPoseidonMode::Gpu => fastpq_prover::PoseidonExecutionMode::Gpu,
    }
}
fn worker_digest(label: &[u8], parts: &[&[u8]]) -> Hash {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(label);
    bytes.push(0);
    for part in parts {
        bytes.extend_from_slice(&(part.len() as u64).to_le_bytes());
        bytes.extend_from_slice(part);
    }
    Hash::new(bytes)
}
fn integer_mantissa(value: &Quantity) -> Option<u128> {
    if value.scale() == 0 {
        value.as_numeric().try_mantissa_u128()
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, HashOf, MerkleProof};
    use iroha_data_model::{
        Level,
        block::{BlockHeader, consensus::LaneBlockCommitment},
        domain::DomainId,
        isi::Log,
        nexus::{LaneFinalityAuthorityV1, LaneId, LaneRelayEnvelope},
    };
    use iroha_primitives::numeric::Quantity;
    use std::num::NonZeroU64;
    fn test_fastpq() -> Fastpq {
        Fastpq {
            execution_mode: FastpqExecutionMode::Cpu,
            poseidon_mode: FastpqPoseidonMode::Cpu,
            proof_sidecar_queue_cap:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_QUEUE_CAP,
            proof_sidecar_max_bytes:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_BYTES,
            proof_sidecar_max_retries:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_RETRIES,
            device_class: None,
            chip_family: None,
            gpu_kind: None,
            metal_queue_fanout: None,
            metal_queue_column_threshold: None,
            metal_max_in_flight: None,
            metal_threadgroup_width: None,
            metal_trace: false,
            metal_debug_enum: false,
            metal_debug_fused: false,
        }
    }
    fn checked_nexus_fee_relay_key_fixture() -> KeyPair {
        KeyPair::try_random().expect("generate checked Nexus fee relay key fixture")
    }
    #[test]
    fn nexus_fee_relay_fixture_uses_checked_random_key_generation() {
        let key_pair = checked_nexus_fee_relay_key_fixture();
        let algorithm = key_pair
            .public_key()
            .try_algorithm()
            .expect("Nexus fee relay fixture key advertises a valid algorithm");
        assert_eq!(algorithm, Algorithm::default());
    }
    fn sample_envelope(manifest_root: [u8; 32]) -> LaneRelayEnvelope {
        let header = BlockHeader::new(
            NonZeroU64::new(7).expect("non-zero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let settlement_commitment = LaneBlockCommitment {
            block_height: header.height().get(),
            lane_id: LaneId::new(3),
            lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
            dataspace_id: DataSpaceId::new(10),
            tx_count: 1,
            total_local_amount: "0.000076".parse().expect("valid settlement quantity"),
            total_xor_due: "0.000001".parse().expect("valid settlement quantity"),
            total_xor_after_haircut: "0.000001".parse().expect("valid settlement quantity"),
            total_xor_variance: "0".parse().expect("valid settlement quantity"),
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        };
        LaneRelayEnvelope::new(header, None, settlement_commitment, 0)
            .expect("valid envelope")
            .with_manifest_root(Some(manifest_root))
            .with_lane_block_descriptor_hash(Some(Hash::new(
                b"relay-worker-test-lane-block-descriptor",
            )))
    }
    fn sample_allocation_work(
        last_height: u64,
        status: DurableWorkStatus,
    ) -> DurableAllocationWork {
        let sponsor = AccountId::new(checked_nexus_fee_relay_key_fixture().public_key().clone());
        DurableAllocationWork {
            program_id: FeeSponsorProgramId::new(
                sponsor,
                "relay".parse().expect("valid sponsor program name"),
            ),
            program_revision: 1,
            asset_definition_id: AssetDefinitionId::derive_from_components(
                DomainId::try_new("universal", "universal").expect("valid universal domain"),
                "xor".parse().expect("valid fee asset name"),
            ),
            verified_allocation: Quantity::from(1_u32),
            source_dataspace_id: DataSpaceId::new(10),
            source_height: last_height,
            source_state_root: Hash::new(last_height.to_le_bytes()),
            expires_at_height: last_height.saturating_add(10),
            lease_id: Hash::new([u8::try_from(last_height).unwrap_or(u8::MAX)]),
            manifest_root: [0x63; 32],
            proof_blob: None,
            status,
            attempts: 1,
            last_height,
        }
    }
    #[test]
    fn durable_worker_resource_limits_accept_boundaries_and_reject_first_overflow() {
        assert!(validate_worker_item_limit(1).is_ok());
        assert!(validate_worker_item_limit(WORKER_STATE_MAX_ITEMS_PER_KIND).is_ok());
        assert!(validate_worker_item_limit(0).is_err());
        assert!(validate_worker_item_limit(WORKER_STATE_MAX_ITEMS_PER_KIND + 1).is_err());
        assert!(
            validate_relay_resource_lengths(
                WORKER_STATE_MAX_KEY_BYTES,
                WORKER_STATE_MAX_RELAY_BYTES,
            )
            .is_ok()
        );
        assert!(
            validate_relay_resource_lengths(
                WORKER_STATE_MAX_KEY_BYTES + 1,
                WORKER_STATE_MAX_RELAY_BYTES,
            )
            .is_err()
        );
        assert!(
            validate_relay_resource_lengths(
                WORKER_STATE_MAX_KEY_BYTES,
                WORKER_STATE_MAX_RELAY_BYTES + 1,
            )
            .is_err()
        );
        assert!(
            validate_allocation_resource_lengths(
                WORKER_STATE_MAX_KEY_BYTES,
                WORKER_STATE_MAX_PROOF_BYTES,
            )
            .is_ok()
        );
        assert!(
            validate_allocation_resource_lengths(
                WORKER_STATE_MAX_KEY_BYTES + 1,
                WORKER_STATE_MAX_PROOF_BYTES,
            )
            .is_err()
        );
        assert!(
            validate_allocation_resource_lengths(
                WORKER_STATE_MAX_KEY_BYTES,
                WORKER_STATE_MAX_PROOF_BYTES + 1,
            )
            .is_err()
        );
    }
    #[test]
    fn durable_worker_journal_rejects_sparse_oversize_file_before_decode() {
        let directory = tempfile::tempdir().expect("create worker-state test directory");
        let path = directory.path().join(WORKER_STATE_FILE);
        let file = fs::File::create(&path).expect("create sparse worker-state file");
        file.set_len(
            u64::try_from(WORKER_STATE_MAX_BYTES)
                .expect("worker-state limit fits u64")
                .saturating_add(1),
        )
        .expect("extend sparse worker-state file");
        let error = load_durable_state(&path, 1).expect_err("oversize journal must fail closed");
        assert!(
            error.to_string().contains("not a regular file within"),
            "unexpected error: {error:?}"
        );
    }
    #[test]
    fn durable_worker_journal_loads_protocol_bound_before_configured_pruning() {
        let directory = tempfile::tempdir().expect("create worker-state test directory");
        let path = directory.path().join(WORKER_STATE_FILE);
        let envelope = sample_envelope([0x40; 32]);
        let mut durable = DurableWorkerState::default();
        for key in ["older", "newer"] {
            durable.relays.insert(
                key.to_owned(),
                DurableRelayWork {
                    envelope: envelope.clone(),
                    status: DurableWorkStatus::Pending,
                    attempts: 0,
                    last_height: u64::from(key == "newer"),
                },
            );
        }
        persist_durable_state(&path, &durable, 2).expect("persist bounded worker journal");
        let mut loaded = load_durable_state(&path, 1).expect("load protocol-bounded older journal");
        assert_eq!(loaded.relays.len(), 2);
        assert!(prune_durable_worker_state(&mut loaded, 1));
        assert_eq!(loaded.relays.len(), 1);
        assert!(loaded.relays.contains_key("newer"));
    }
    #[test]
    fn durable_worker_pruning_bounds_both_kinds_and_discards_terminal_payloads_first() {
        let envelope = sample_envelope([0x40; 32]);
        let mut durable = DurableWorkerState::default();
        for (key, status, last_height) in [
            ("accepted", DurableWorkStatus::Accepted, 4),
            ("rejected", DurableWorkStatus::Rejected, 1),
            ("pending-old", DurableWorkStatus::Pending, 2),
            ("pending-new", DurableWorkStatus::Submitted, 3),
        ] {
            durable.relays.insert(
                key.to_owned(),
                DurableRelayWork {
                    envelope: envelope.clone(),
                    status,
                    attempts: 1,
                    last_height,
                },
            );
        }
        for (key, status, last_height) in [
            ("accepted", DurableWorkStatus::Accepted, 5),
            ("rejected", DurableWorkStatus::Rejected, 4),
            ("pending-old", DurableWorkStatus::Pending, 1),
            ("pending-mid", DurableWorkStatus::Proving, 2),
            ("pending-new", DurableWorkStatus::Submitted, 3),
        ] {
            durable
                .allocations
                .insert(key.to_owned(), sample_allocation_work(last_height, status));
        }
        assert!(prune_durable_worker_state(&mut durable, 2));
        assert_eq!(
            durable.relays.keys().cloned().collect::<Vec<_>>(),
            vec!["pending-new".to_owned(), "pending-old".to_owned()]
        );
        assert_eq!(
            durable.allocations.keys().cloned().collect::<Vec<_>>(),
            vec!["pending-mid".to_owned(), "pending-new".to_owned()]
        );
        assert!(
            !prune_durable_worker_state(&mut durable, 2),
            "already-bounded state must not trigger another checkpoint rewrite"
        );
    }
    #[test]
    fn verified_worker_records_use_state_path_keys() {
        let envelope = sample_envelope([0x40; 32]);
        let relay_key_text = envelope.relay_ref().relay_state_key();
        let relay_key = StatePath::from_str(&relay_key_text).expect("valid relay state path");
        assert_eq!(relay_key.as_ref(), relay_key_text);
        let allocation = sample_allocation_work(7, DurableWorkStatus::Pending);
        let allocation_key_text = VerifiedFeeSponsorVaultAllocation::state_key_for(
            &allocation.program_id,
            &allocation.asset_definition_id,
            &allocation.lease_id,
        );
        let allocation_key =
            StatePath::from_str(&allocation_key_text).expect("valid allocation state path");
        assert_eq!(allocation_key.as_ref(), allocation_key_text);
        let records = BTreeMap::from([
            (relay_key.clone(), vec![0xA5]),
            (allocation_key.clone(), vec![0x5A]),
        ]);
        assert_eq!(
            records
                .get(&relay_key)
                .expect("stored relay record")
                .as_slice(),
            &[0xA5]
        );
        assert_eq!(
            records
                .get(&allocation_key)
                .expect("stored allocation record")
                .as_slice(),
            &[0x5A]
        );
    }
    #[test]
    fn rejected_relay_slot_is_reclaimed_before_active_work() {
        let envelope = sample_envelope([0x41; 32]);
        let mut durable = DurableWorkerState::default();
        for (key, status, last_height) in [
            ("pending", DurableWorkStatus::Pending, 1),
            ("rejected-new", DurableWorkStatus::Rejected, 3),
            ("rejected-old", DurableWorkStatus::Rejected, 2),
        ] {
            durable.relays.insert(
                key.to_owned(),
                DurableRelayWork {
                    envelope: envelope.clone(),
                    status,
                    attempts: 1,
                    last_height,
                },
            );
        }
        assert!(reclaim_oldest_rejected_relay(&mut durable));
        assert!(!durable.relays.contains_key("rejected-old"));
        assert!(durable.relays.contains_key("rejected-new"));
        assert!(durable.relays.contains_key("pending"));
    }
    fn attach_test_finality_authority(envelope: &mut LaneRelayEnvelope) {
        envelope.finality_authority = Some(LaneFinalityAuthorityV1 {
            version: 1,
            global_block_height: envelope.block_header.height().get(),
            finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"relay-worker-test-finality-artifact",
            )),
            statement_proof: MerkleProof::from_audit_path(0, Vec::new()),
        });
    }
    #[test]
    fn status_relay_requires_exact_finality_authenticated_state_entry() {
        let pending = sample_envelope([0x40; 32]);
        let mut recorded_relays = LaneRelayStore::default();
        assert!(
            authoritative_status_relay(&recorded_relays, &pending).is_none(),
            "a pending status snapshot without an authoritative State entry must not trigger proof generation"
        );
        let mut finalized = pending.clone();
        attach_test_finality_authority(&mut finalized);
        recorded_relays
            .insert(finalized.clone())
            .expect("insert relay with compact finality authority");
        assert!(
            authoritative_status_relay(&recorded_relays, &pending).is_none(),
            "a pending status payload cannot borrow authority from a finalized State entry"
        );
        assert_eq!(
            authoritative_status_relay(&recorded_relays, &finalized),
            Some(&finalized),
            "the exact finalized State entry is proof-eligible"
        );
        let mut altered = finalized;
        altered.rbc_bytes_total = altered.rbc_bytes_total.saturating_add(1);
        assert!(
            authoritative_status_relay(&recorded_relays, &altered).is_none(),
            "status payloads cannot substitute fields after State authentication"
        );
    }
    #[test]
    fn future_relay_deferral_preserves_durable_retry_budget_until_proposal_commits() {
        let envelope = sample_envelope([0x41; 32]);
        let key = relay_work_key(&envelope);
        let mut durable = DurableWorkerState::default();
        durable.relays.insert(
            key.clone(),
            DurableRelayWork {
                envelope: envelope.clone(),
                status: DurableWorkStatus::Pending,
                attempts: 1,
                last_height: 3,
            },
        );
        let before = norito::to_bytes(&durable).expect("encode durable relay work before deferral");
        for committed_height in 0..envelope.block_header.height().get() {
            let decision = prepare_relay_attempt(
                durable.relays.get_mut(&key).expect("durable relay work"),
                committed_height,
                3,
            );
            assert!(matches!(decision, RelayAttemptDecision::Deferred));
            assert_eq!(
                norito::to_bytes(&durable).expect("encode deferred durable relay work"),
                before,
                "a future proposal must not consume attempts or mutate persisted retry state"
            );
        }
        let proposal_height = envelope.block_header.height().get();
        let decision = prepare_relay_attempt(
            durable.relays.get_mut(&key).expect("durable relay work"),
            proposal_height,
            3,
        );
        let RelayAttemptDecision::Ready(ready) = decision else {
            panic!("relay should become retryable at its committed proposal height");
        };
        assert_eq!(*ready, envelope);
        let work = durable.relays.get(&key).expect("durable relay work");
        assert_eq!(work.attempts, 2);
        assert_eq!(work.last_height, proposal_height);
        assert_eq!(work.status, DurableWorkStatus::Pending);
    }
    #[test]
    fn exhausted_future_relay_is_not_rejected_before_proposal_commits() {
        let envelope = sample_envelope([0x42; 32]);
        let key = relay_work_key(&envelope);
        let mut durable = DurableWorkerState::default();
        durable.relays.insert(
            key.clone(),
            DurableRelayWork {
                envelope: envelope.clone(),
                status: DurableWorkStatus::Submitted,
                attempts: 3,
                last_height: 4,
            },
        );
        let before = norito::to_bytes(&durable).expect("encode exhausted future relay work");
        let proposal_height = envelope.block_header.height().get();
        let decision = prepare_relay_attempt(
            durable.relays.get_mut(&key).expect("durable relay work"),
            proposal_height - 1,
            3,
        );
        assert!(matches!(decision, RelayAttemptDecision::Deferred));
        assert_eq!(
            norito::to_bytes(&durable).expect("encode deferred exhausted relay work"),
            before,
            "retry exhaustion must not reject a relay before its proposal commits"
        );
        let decision = prepare_relay_attempt(
            durable.relays.get_mut(&key).expect("durable relay work"),
            proposal_height,
            3,
        );
        assert!(matches!(decision, RelayAttemptDecision::Rejected));
        let work = durable.relays.get(&key).expect("durable relay work");
        assert_eq!(work.status, DurableWorkStatus::Rejected);
        assert_eq!(work.attempts, 3);
        assert_eq!(work.last_height, 4);
    }
    #[test]
    fn fee_relay_submission_transaction_checked_signing_verifies() -> Result<()> {
        let key_pair = checked_nexus_fee_relay_key_fixture();
        let authority = AccountId::new(key_pair.public_key().clone());
        let endpoint = "/internal/nexus/fee-relay/test";
        let tx = sign_nexus_fee_relay_submission_transaction(
            iroha_data_model::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [0x15; Hash::LENGTH],
                )),
            ),
            authority.clone(),
            InstructionBox::from(Log::new(Level::INFO, "checked fee relay signing".into())),
            worker_submission_metadata(endpoint),
            &key_pair,
            endpoint,
        )?;
        tx.verify_signature()
            .wrap_err("verify checked Nexus fee relay submission signature")?;
        assert_eq!(tx.authority(), &authority);
        Ok(())
    }
    #[test]
    fn fee_sponsor_vault_partition_never_duplicates_capacity_across_dataspaces() -> Result<()> {
        let balance: Quantity = "10.000000001".parse()?;
        let allocations = partition_fee_sponsor_vault(&balance, 2, 9)?;
        assert_eq!(allocations.len(), 2);
        assert_eq!(
            allocations[0].checked_add(&allocations[1])?,
            balance,
            "two dataspace leases must partition, not duplicate, the source vault"
        );
        assert!(allocations.iter().all(|allocation| allocation < &balance));
        Ok(())
    }
    #[test]
    fn enrolled_program_gets_allocation_for_explicit_non_default_route() {
        assert!(fee_sponsor_route_allocation_eligible(
            true,
            FeeSponsorEligibility::EnrolledOnly,
            false,
        ));
        assert!(!fee_sponsor_route_allocation_eligible(
            false,
            FeeSponsorEligibility::EnrolledOnly,
            true,
        ));
        assert!(fee_sponsor_route_allocation_eligible(
            false,
            FeeSponsorEligibility::EnrolledOrRouteDefault,
            true,
        ));
    }
    #[test]
    fn fee_sponsor_allocation_expiry_respects_scheduled_revision_boundary() {
        assert_eq!(fee_sponsor_allocation_expiry_height(10, 20, None), Some(30));
        assert_eq!(
            fee_sponsor_allocation_expiry_height(10, 20, Some(25)),
            Some(24),
            "an old-revision lease must drain before the scheduled activation"
        );
        assert_eq!(
            fee_sponsor_allocation_expiry_height(10, 5, Some(25)),
            Some(15),
            "a later activation must not extend the ordinary lease"
        );
        assert_eq!(
            fee_sponsor_allocation_expiry_height(10, 20, Some(12)),
            Some(11),
            "the last executable pre-activation block remains eligible"
        );
        assert_eq!(
            fee_sponsor_allocation_expiry_height(10, 20, Some(11)),
            None,
            "work that cannot execute before activation must not be emitted"
        );
    }
    #[test]
    fn lane_relay_worker_proof_verifies_and_binds_claim() -> Result<()> {
        let mut envelope = sample_envelope([0x42; 32]);
        attach_test_finality_authority(&mut envelope);
        let (proven, proof_blob) = prove_lane_relay_envelope(
            &envelope,
            Hash::new(b"relay-worker-test-parent-state-root"),
            Hash::new(b"relay-worker-test-post-state-root"),
            20,
            7,
            &test_fastpq(),
        )?;
        proven.validate_fastpq_proof_metadata()?;
        let proof_envelope: AxtProofEnvelope = norito::decode_from_bytes(&proof_blob.payload)?;
        let verified = fastpq_prover::verify_axt_proof_envelope(&proof_envelope)?;
        assert_eq!(proof_envelope.dsid, envelope.dataspace_id);
        assert_eq!(proof_envelope.manifest_root, [0x42; 32]);
        let binding = proof_envelope.fastpq_binding.expect("fastpq binding");
        assert_eq!(
            binding.claim_digest,
            hex::encode(lane_relay_fastpq_claim_digest(&envelope)?.as_ref())
        );
        assert_eq!(binding.verified_effect_type, LANE_RELAY_FASTPQ_EFFECT_TYPE);
        assert_eq!(binding.target_dsids, vec![DataSpaceId::UNIVERSAL.as_u64()]);
        assert_ne!(verified.proof_digest, Hash::new(b"test-only-digest"));
        Ok(())
    }
    #[test]
    fn fee_sponsor_vault_allocation_worker_proof_verifies() -> Result<()> {
        let sponsor = AccountId::new(checked_nexus_fee_relay_key_fixture().public_key().clone());
        let program_id = FeeSponsorProgramId::new(
            sponsor,
            "relay".parse().expect("valid sponsor program name"),
        );
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("universal", "universal").expect("valid universal domain"),
            "xor".parse().expect("valid fee asset name"),
        );
        let verified_allocation = Quantity::from(50_u32);
        let source_dataspace_id = DataSpaceId::new(10);
        let source_height = 7;
        let expires_at_height = 20;
        let source_state_root = fee_sponsor_vault_source_state_root(
            &program_id,
            3,
            &asset_definition_id,
            &verified_allocation,
            source_dataspace_id,
            source_height,
        );
        let lease_id = fee_sponsor_vault_lease_id(
            &program_id,
            3,
            &asset_definition_id,
            source_dataspace_id,
            source_height,
            source_state_root,
            expires_at_height,
        )?;
        let work = DurableAllocationWork {
            program_id: program_id.clone(),
            program_revision: 3,
            asset_definition_id: asset_definition_id.clone(),
            verified_allocation: verified_allocation.clone(),
            source_dataspace_id,
            source_height,
            source_state_root,
            expires_at_height,
            lease_id,
            manifest_root: [0x63; 32],
            proof_blob: None,
            status: DurableWorkStatus::Pending,
            attempts: 0,
            last_height: source_height,
        };
        let proof_blob = prove_fee_sponsor_vault_allocation(&work, &test_fastpq())?;
        let proof_envelope: AxtProofEnvelope = norito::decode_from_bytes(&proof_blob.payload)?;
        fastpq_prover::verify_axt_proof_envelope(&proof_envelope)?;
        let binding = proof_envelope.fastpq_binding.expect("fastpq binding");
        assert_eq!(
            binding.verified_effect_type,
            FEE_SPONSOR_VAULT_ALLOCATION_EFFECT_TYPE
        );
        assert_eq!(binding.target_dsids, vec![DataSpaceId::UNIVERSAL.as_u64()]);
        let claim = FeeSponsorVaultAllocationClaim {
            program_id,
            program_revision: 3,
            asset_definition_id,
            verified_allocation,
            source_dataspace_id,
            source_height,
            source_state_root,
            expires_at_height,
            lease_id,
        };
        assert_eq!(
            binding.claim_digest,
            hex::encode(fee_sponsor_vault_allocation_claim_digest(&claim).as_ref())
        );
        assert_eq!(proof_envelope.committed_amount, Some(50));
        Ok(())
    }
}
