//! Asynchronous Nexus fee relay worker for activated DPN lane-relay burns.
//!
//! The worker never mutates world state directly. It watches finalized lane
//! relay envelopes published by core, builds FastPQ/AXT proofs, and submits the
//! corresponding protocol ISIs through the normal transaction queue.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use eyre::{Result, WrapErr};
use iroha_config::parameters::actual::{
    Fastpq, FastpqExecutionMode, FastpqPoseidonMode, NexusRelayWorker as NexusRelayWorkerConfig,
};
use iroha_core::{
    queue::Queue,
    state::{State, WorldReadOnly},
    sumeragi::{self, SumeragiHandle},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{
    ChainId,
    account::{AccountId, ParsedAccountId},
    asset::{
        AssetDefinitionAlias,
        id::{AssetDefinitionId, AssetId},
    },
    isi::{
        InstructionBox,
        nexus::{RegisterVerifiedLaneRelay, RegisterVerifiedNexusFeeBudget},
    },
    metadata::Metadata,
    name::Name,
    nexus::{
        AxtEffectBinding, AxtFastpqBinding, AxtProofEnvelope, DataSpaceId,
        LANE_RELAY_FASTPQ_EFFECT_TYPE, LaneFastpqProofMaterial, LaneRelayEnvelope, ProofBlob,
        VERIFIED_LANE_RELAY_STATE_KEY_PREFIX, VerifiedLaneRelayRecord,
        VerifiedNexusFeeBudgetRecord, lane_relay_fastpq_claim_digest,
        nexus_fee_budget_claim_digest,
    },
    transaction::TransactionBuilder,
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_primitives::{json::Json, numeric::Numeric};
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};

const WORKER_STATE_FILE: &str = "nexus_fee_relay_worker_state.norito";
const FEE_BUDGET_EFFECT_TYPE: &str = "nexus_fee_budget";

#[derive(Clone, Debug, Default, Decode, Encode)]
struct DurableWorkerState {
    relays: BTreeMap<String, DurableRelayWork>,
    budget: Option<DurableBudgetWork>,
}

#[derive(Clone, Debug, Decode, Encode)]
struct DurableRelayWork {
    envelope: LaneRelayEnvelope,
    status: DurableWorkStatus,
    attempts: u32,
    last_height: u64,
}

#[derive(Clone, Debug, Decode, Encode)]
struct DurableBudgetWork {
    sponsor_account_id: AccountId,
    fee_asset_id: String,
    verified_balance: Numeric,
    manifest_root: [u8; 32],
    proof_blob: Option<ProofBlob>,
    status: DurableWorkStatus,
    attempts: u32,
    last_height: u64,
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
pub(crate) struct NexusFeeRelayWorker {
    config: NexusRelayWorkerConfig,
    state_path: PathBuf,
    chain_id: Arc<ChainId>,
    queue: Arc<Queue>,
    state: Arc<State>,
    sumeragi: SumeragiHandle,
    authority: AccountId,
    key_pair: KeyPair,
    fastpq: Fastpq,
    durable: parking_lot::Mutex<DurableWorkerState>,
    announced_relays: parking_lot::Mutex<BTreeSet<String>>,
}

impl NexusFeeRelayWorker {
    /// Construct a worker. The optional configured relayer account must match the node key.
    pub(crate) fn new(
        config: NexusRelayWorkerConfig,
        storage_root: PathBuf,
        chain_id: Arc<ChainId>,
        queue: Arc<Queue>,
        state: Arc<State>,
        sumeragi: SumeragiHandle,
        authority: AccountId,
        key_pair: KeyPair,
        fastpq: Fastpq,
    ) -> Result<Self> {
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

        let state_path = storage_root.join(WORKER_STATE_FILE);
        let durable = load_durable_state(&state_path).unwrap_or_else(|error| {
            iroha_logger::warn!(
                ?error,
                path = %state_path.display(),
                "failed to load Nexus fee relay worker state; starting with an empty retry set"
            );
            DurableWorkerState::default()
        });

        Ok(Self {
            config,
            state_path,
            chain_id,
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
    pub(crate) fn start(self, shutdown_signal: ShutdownSignal) -> Child {
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

        self.mark_accepted_budget_if_present()?;
        self.announce_verified_relays()?;
        self.enqueue_status_relays()?;
        self.submit_pending_relays()?;
        self.refresh_budget_if_due()?;
        Ok(())
    }

    fn enqueue_status_relays(&self) -> Result<()> {
        let envelopes = sumeragi::status::lane_relay_envelopes_snapshot();
        if envelopes.is_empty() {
            return Ok(());
        }

        let mut durable = self.durable.lock();
        for envelope in envelopes {
            if durable_pending_relay_count(&durable) >= self.config.max_pending_relays.get()
                && !durable.relays.contains_key(&relay_work_key(&envelope))
            {
                iroha_logger::warn!(
                    max_pending_relays = self.config.max_pending_relays.get(),
                    "Nexus fee relay worker pending set is full; dropping finalized relay enqueue"
                );
                continue;
            }

            if self.verified_relay_exists(&envelope)? {
                continue;
            }
            let key = relay_work_key(&envelope);
            durable.relays.entry(key).or_insert(DurableRelayWork {
                envelope,
                status: DurableWorkStatus::Pending,
                attempts: 0,
                last_height: self.committed_height(),
            });
        }
        persist_durable_state(&self.state_path, &durable)
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
        let (proven_envelope, proof_blob) =
            match prove_lane_relay_envelope(&envelope, expiry_slot, current_height, &self.fastpq) {
                Ok(proven) => proven,
                Err(error) => {
                    self.reject_or_retry_relay(key, envelope, error)?;
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
            }),
            "/internal/nexus/fee-relay/register-verified-lane-relay",
        )
    }

    fn relay_work_for_attempt(&self, key: &str) -> Result<Option<LaneRelayEnvelope>> {
        let mut durable = self.durable.lock();
        let Some(work) = durable.relays.get_mut(key) else {
            return Ok(None);
        };
        if work.attempts >= self.config.max_retry_attempts.get() {
            work.status = DurableWorkStatus::Rejected;
            persist_durable_state(&self.state_path, &durable)?;
            return Ok(None);
        }
        work.attempts = work.attempts.saturating_add(1);
        work.last_height = self.committed_height();
        let envelope = work.envelope.clone();
        persist_durable_state(&self.state_path, &durable)?;
        Ok(Some(envelope))
    }

    fn update_relay_status(
        &self,
        key: &str,
        status: DurableWorkStatus,
        envelope: Option<LaneRelayEnvelope>,
    ) -> Result<()> {
        let mut durable = self.durable.lock();
        if let Some(work) = durable.relays.get_mut(key) {
            if let Some(envelope) = envelope {
                work.envelope = envelope;
            }
            work.status = status;
            work.last_height = self.committed_height();
        }
        persist_durable_state(&self.state_path, &durable)
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
        error: eyre::Report,
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
        persist_durable_state(&self.state_path, &durable)
    }

    fn announce_verified_relays(&self) -> Result<()> {
        let records = self.verified_relay_records()?;
        for (key, record) in records {
            {
                let mut durable = self.durable.lock();
                if let Some(work) = durable.relays.get_mut(&key) {
                    work.status = DurableWorkStatus::Accepted;
                    work.envelope = record.relay_envelope.clone();
                    work.last_height = self.committed_height();
                }
                persist_durable_state(&self.state_path, &durable)?;
            }

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

    fn verified_relay_records(&self) -> Result<BTreeMap<String, VerifiedLaneRelayRecord>> {
        let view = self.state.view();
        let mut records = BTreeMap::new();
        for (key, payload) in view.world().smart_contract_state().iter() {
            let key_text = key.to_string();
            if !key_text.starts_with(VERIFIED_LANE_RELAY_STATE_KEY_PREFIX) {
                continue;
            }
            let json: Json = norito::decode_from_bytes(payload)
                .wrap_err("decode verified lane relay record JSON payload")?;
            let record: VerifiedLaneRelayRecord = norito::json::from_slice(json.get().as_bytes())
                .wrap_err("decode verified lane relay record")?;
            records.insert(record.relay_ref.relay_state_key(), record);
        }
        Ok(records)
    }

    fn verified_relay_exists(&self, envelope: &LaneRelayEnvelope) -> Result<bool> {
        let key = Name::from_str(&envelope.relay_ref().relay_state_key())
            .wrap_err("parse verified lane relay state key")?;
        Ok(self
            .state
            .view()
            .world()
            .smart_contract_state()
            .get(&key)
            .is_some())
    }

    fn refresh_budget_if_due(&self) -> Result<()> {
        let Some(sponsor) = self.canonical_sponsor()? else {
            return Ok(());
        };
        let fee_asset_id = self.state.view().nexus.fees.fee_asset_id.trim().to_owned();
        if fee_asset_id.is_empty() {
            return Ok(());
        }
        let current_height = self.committed_height();
        if let Some(record) = self.verified_budget_record(&sponsor, &fee_asset_id)?
            && current_height.saturating_sub(record.verified_at_height)
                < self.config.budget_refresh_interval_blocks.get()
        {
            return Ok(());
        }

        let mut budget_work = self.prepare_budget_work(&sponsor, &fee_asset_id)?;
        if budget_work.attempts >= self.config.max_retry_attempts.get() {
            budget_work.status = DurableWorkStatus::Rejected;
            self.store_budget_work(budget_work)?;
            return Ok(());
        }
        budget_work.status = DurableWorkStatus::Proving;
        budget_work.attempts = budget_work.attempts.saturating_add(1);
        budget_work.last_height = current_height;
        self.store_budget_work(budget_work.clone())?;

        let expiry_slot =
            current_height.saturating_add(self.state.view().nexus.axt.replay_retention_slots.get());
        let proof_blob = match prove_fee_budget(
            &budget_work.sponsor_account_id,
            &budget_work.fee_asset_id,
            &budget_work.verified_balance,
            budget_work.manifest_root,
            expiry_slot,
            &self.fastpq,
        ) {
            Ok(proof) => proof,
            Err(error) => {
                budget_work.status = if budget_work.attempts >= self.config.max_retry_attempts.get()
                {
                    DurableWorkStatus::Rejected
                } else {
                    DurableWorkStatus::Pending
                };
                self.store_budget_work(budget_work)?;
                iroha_logger::warn!(
                    ?error,
                    "Nexus fee relay worker failed to prove sponsor fee budget"
                );
                return Ok(());
            }
        };
        budget_work.status = DurableWorkStatus::Submitted;
        budget_work.proof_blob = Some(proof_blob.clone());
        budget_work.last_height = current_height;
        self.store_budget_work(budget_work.clone())?;
        self.submit_instruction(
            InstructionBox::from(RegisterVerifiedNexusFeeBudget {
                sponsor_account_id: budget_work.sponsor_account_id,
                fee_asset_id: budget_work.fee_asset_id,
                verified_balance: budget_work.verified_balance,
                manifest_root: budget_work.manifest_root,
                proof_blob,
            }),
            "/internal/nexus/fee-relay/register-verified-fee-budget",
        )
    }

    fn prepare_budget_work(
        &self,
        sponsor: &AccountId,
        fee_asset_id: &str,
    ) -> Result<DurableBudgetWork> {
        let existing = self.durable.lock().budget.clone();
        if let Some(work) = existing
            && work.sponsor_account_id == *sponsor
            && work.fee_asset_id == fee_asset_id
            && matches!(
                work.status,
                DurableWorkStatus::Pending
                    | DurableWorkStatus::Proving
                    | DurableWorkStatus::Submitted
            )
        {
            return Ok(work);
        }

        let manifest_root = self
            .manifest_root_for(DataSpaceId::UNIVERSAL)
            .ok_or_else(|| {
                eyre::eyre!("no non-zero universal AXT manifest root for Nexus fee budget proof")
            })?;
        let verified_balance = self.sponsor_fee_balance(sponsor, fee_asset_id)?;
        Ok(DurableBudgetWork {
            sponsor_account_id: sponsor.clone(),
            fee_asset_id: fee_asset_id.to_owned(),
            verified_balance,
            manifest_root,
            proof_blob: None,
            status: DurableWorkStatus::Pending,
            attempts: 0,
            last_height: self.committed_height(),
        })
    }

    fn store_budget_work(&self, work: DurableBudgetWork) -> Result<()> {
        let mut durable = self.durable.lock();
        durable.budget = Some(work);
        persist_durable_state(&self.state_path, &durable)
    }

    fn mark_accepted_budget_if_present(&self) -> Result<()> {
        let Some(work) = self.durable.lock().budget.clone() else {
            return Ok(());
        };
        if self
            .verified_budget_record(&work.sponsor_account_id, &work.fee_asset_id)?
            .is_some()
        {
            let mut updated = work;
            updated.status = DurableWorkStatus::Accepted;
            updated.last_height = self.committed_height();
            self.store_budget_work(updated)?;
        }
        Ok(())
    }

    fn verified_budget_record(
        &self,
        sponsor: &AccountId,
        fee_asset_id: &str,
    ) -> Result<Option<VerifiedNexusFeeBudgetRecord>> {
        let key = Name::from_str(&VerifiedNexusFeeBudgetRecord::state_key_for(
            sponsor,
            fee_asset_id,
        ))
        .wrap_err("parse verified Nexus fee budget state key")?;
        let view = self.state.view();
        let Some(payload) = view.world().smart_contract_state().get(&key) else {
            return Ok(None);
        };
        let json: Json = norito::decode_from_bytes(payload)
            .wrap_err("decode verified Nexus fee budget record JSON payload")?;
        norito::json::from_slice(json.get().as_bytes())
            .map(Some)
            .wrap_err("decode verified Nexus fee budget record")
    }

    fn canonical_sponsor(&self) -> Result<Option<AccountId>> {
        let view = self.state.view();
        let Some(raw) = view
            .nexus
            .fees
            .canonical_sponsor_account_id
            .as_deref()
            .map(str::trim)
            .filter(|raw| !raw.is_empty())
        else {
            iroha_logger::warn!(
                "Nexus fee relay worker enabled without nexus.fees.canonical_sponsor_account_id"
            );
            return Ok(None);
        };
        parse_canonical_account_id(raw)
            .map(Some)
            .wrap_err_with(|| format!("parse canonical sponsor account id `{raw}`"))
    }

    fn sponsor_fee_balance(&self, sponsor: &AccountId, fee_asset_id: &str) -> Result<Numeric> {
        let view = self.state.view();
        let now_ms = self
            .state
            .latest_block_header_fast()
            .map(|header| header.creation_time_ms)
            .unwrap_or(0);
        let Some(asset_definition_id) =
            parse_asset_definition_selector(view.world(), fee_asset_id, now_ms)
        else {
            eyre::bail!("invalid or unresolved Nexus fee asset selector `{fee_asset_id}`");
        };
        let asset_id = AssetId::new(asset_definition_id, sponsor.clone());
        Ok(view
            .world()
            .asset(&asset_id)
            .map(|asset| asset.value().clone().into_inner())
            .unwrap_or_else(|_| Numeric::zero()))
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
        let tx = TransactionBuilder::new((*self.chain_id).clone(), self.authority.clone())
            .with_instructions([instruction])
            .with_metadata(worker_submission_metadata(endpoint))
            .sign(self.key_pair.private_key());
        let view = self.state.view();
        let params = view.world().parameters();
        let accepted = AcceptedTransaction::accept(
            tx,
            &self.chain_id,
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

fn durable_pending_relay_count(durable: &DurableWorkerState) -> usize {
    durable
        .relays
        .values()
        .filter(|work| {
            !matches!(
                work.status,
                DurableWorkStatus::Accepted | DurableWorkStatus::Rejected
            )
        })
        .count()
}

fn relay_work_key(envelope: &LaneRelayEnvelope) -> String {
    envelope.relay_ref().relay_state_key()
}

fn load_durable_state(path: &Path) -> Result<DurableWorkerState> {
    if !path.exists() {
        return Ok(DurableWorkerState::default());
    }
    let bytes = fs::read(path).wrap_err_with(|| format!("read {}", path.display()))?;
    norito::decode_from_bytes(&bytes).wrap_err_with(|| format!("decode {}", path.display()))
}

fn persist_durable_state(path: &Path, durable: &DurableWorkerState) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).wrap_err_with(|| format!("create {}", parent.display()))?;
    }
    let bytes = norito::to_bytes(durable).wrap_err("encode Nexus fee relay worker state")?;
    let tmp = path.with_extension("norito.tmp");
    fs::write(&tmp, bytes).wrap_err_with(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path).wrap_err_with(|| format!("replace {}", path.display()))
}

fn parse_canonical_account_id(raw: &str) -> Result<AccountId> {
    AccountId::parse_encoded(raw)
        .map(ParsedAccountId::into_account_id)
        .map_err(|error| eyre::eyre!("{error}"))
}

fn parse_asset_definition_selector(
    world: &impl WorldReadOnly,
    raw: &str,
    now_ms: u64,
) -> Option<AssetDefinitionId> {
    let trimmed = raw.trim();
    AssetDefinitionId::parse_address_literal(trimmed)
        .ok()
        .or_else(|| {
            AssetDefinitionAlias::from_str(trimmed)
                .ok()
                .and_then(|alias| world.asset_definition_id_by_alias_at(&alias, now_ms))
        })
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
        worker_digest(
            b"nexus-fee-relay:lane-relay-old-root:v1",
            &[relay_ref_bytes.as_slice()],
        ),
        Hash::new(manifest_root),
        worker_digest(
            b"nexus-fee-relay:lane-relay-perm-root:v1",
            &[&manifest_root],
        ),
        worker_digest(
            b"nexus-fee-relay:lane-relay-tx-set:v1",
            &[claim_digest.as_ref()],
        ),
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
        da_commitment: None,
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

fn prove_fee_budget(
    sponsor: &AccountId,
    fee_asset_id: &str,
    verified_balance: &Numeric,
    manifest_root: [u8; 32],
    expiry_slot: u64,
    fastpq: &Fastpq,
) -> Result<ProofBlob> {
    if manifest_root.iter().all(|byte| *byte == 0) {
        eyre::bail!("fee budget proof manifest_root is zero");
    }
    let fee_asset_id = fee_asset_id.trim();
    let sponsor_text = sponsor.to_string();
    let balance_text = verified_balance.to_string();
    let source_tx_commitment = worker_digest(
        b"nexus-fee-relay:budget-source-tx:v1",
        &[sponsor_text.as_bytes(), fee_asset_id.as_bytes()],
    );
    let claim_digest = nexus_fee_budget_claim_digest(sponsor, fee_asset_id, verified_balance);
    let witness_commitment = worker_digest(
        b"nexus-fee-relay:budget-witness:v1",
        &[sponsor_text.as_bytes(), balance_text.as_bytes()],
    );
    let policy_commitment = worker_digest(b"nexus-fee-relay:budget-policy:v1", &[&manifest_root]);
    let dsid = DataSpaceId::UNIVERSAL;
    let binding = AxtFastpqBinding {
        parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
        source_dsid: dsid.as_u64(),
        source_dataspace: "universal".to_owned(),
        source_receipt_id: format!("budget-{}", hex::encode(source_tx_commitment.as_ref())),
        source_tx_commitment: hex::encode(source_tx_commitment.as_ref()),
        claim_type: "authorization".to_owned(),
        claim_digest: hex::encode(claim_digest.as_ref()),
        witness_commitment: hex::encode(witness_commitment.as_ref()),
        policy_commitment: hex::encode(policy_commitment.as_ref()),
        verified_effect_type: FEE_BUDGET_EFFECT_TYPE.to_owned(),
        corridor: "nexus-fee-budget".to_owned(),
        verifier_id: "fastpq".to_owned(),
        verifier_version: "v1".to_owned(),
        target_dsids: vec![dsid.as_u64()],
        effect_binding: Some(AxtEffectBinding {
            destination_domain: None,
            destination_account_id: Some(sponsor_text.clone()),
            vault_account_id: None,
            issuance_account_id: None,
            source_asset_definition_id: Some(fee_asset_id.to_owned()),
            destination_asset_definition_id: None,
            source_amount_i64: None,
            destination_amount_i64: None,
        }),
    };
    let mut batch = transition_batch(
        dsid,
        expiry_slot,
        worker_digest(
            b"nexus-fee-relay:budget-old-root:v1",
            &[fee_asset_id.as_bytes()],
        ),
        Hash::new(manifest_root),
        worker_digest(
            b"nexus-fee-relay:budget-perm-root:v1",
            &[sponsor_text.as_bytes()],
        ),
        worker_digest(
            b"nexus-fee-relay:budget-tx-set:v1",
            &[balance_text.as_bytes()],
        ),
    );
    batch.push(fastpq_prover::StateTransition::new(
        b"axt/nexus/fee-budget".to_vec(),
        sponsor_text.as_bytes().to_vec(),
        balance_text.as_bytes().to_vec(),
        fastpq_prover::OperationKind::MetaSet,
    ));
    batch.sort();
    batch.metadata.insert(
        "entry_hash".to_owned(),
        source_tx_commitment.as_ref().to_vec(),
    );
    fastpq_prover::bind_axt_batch(&mut batch, &binding).wrap_err("bind fee budget AXT batch")?;
    let proof = prover_from_config(fastpq)?
        .prove(&batch)
        .wrap_err("prove fee budget AXT batch")?;
    let payload =
        fastpq_prover::encode_axt_fastpq_payload(&batch, proof).wrap_err("encode AXT payload")?;
    let proof_envelope = AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: None,
        proof: payload,
        fastpq_binding: Some(binding),
        committed_amount: integer_mantissa(verified_balance),
        amount_commitment: None,
    };
    Ok(ProofBlob {
        payload: norito::to_bytes(&proof_envelope).wrap_err("encode fee budget proof envelope")?,
        expiry_slot: Some(expiry_slot),
    })
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

fn integer_mantissa(value: &Numeric) -> Option<u128> {
    if value.scale() == 0 {
        value.try_mantissa_u128()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU64;

    use iroha_data_model::{
        block::{BlockHeader, consensus::LaneBlockCommitment},
        nexus::{LaneId, LaneRelayEnvelope},
    };

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
            dataspace_id: DataSpaceId::new(10),
            tx_count: 1,
            total_local_micro: 76,
            total_xor_due_micro: 1,
            total_xor_after_haircut_micro: 1,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
        };
        LaneRelayEnvelope::new(header, None, None, settlement_commitment, 0)
            .expect("valid envelope")
            .with_manifest_root(Some(manifest_root))
    }

    #[test]
    fn lane_relay_worker_proof_verifies_and_binds_claim() -> Result<()> {
        let envelope = sample_envelope([0x42; 32]);
        let (proven, proof_blob) = prove_lane_relay_envelope(&envelope, 20, 7, &test_fastpq())?;
        proven.verify_fastpq_proof_material()?;
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
        assert_ne!(verified.proof_digest, Hash::new(b"test-only-digest"));
        Ok(())
    }

    #[test]
    fn fee_budget_worker_proof_verifies() -> Result<()> {
        let sponsor = AccountId::new(KeyPair::random().public_key().clone());
        let verified_balance = Numeric::from(50_u32);
        let proof_blob = prove_fee_budget(
            &sponsor,
            "xor#universal",
            &verified_balance,
            [0x63; 32],
            20,
            &test_fastpq(),
        )?;
        let proof_envelope: AxtProofEnvelope = norito::decode_from_bytes(&proof_blob.payload)?;
        fastpq_prover::verify_axt_proof_envelope(&proof_envelope)?;
        let binding = proof_envelope.fastpq_binding.expect("fastpq binding");
        assert_eq!(binding.verified_effect_type, FEE_BUDGET_EFFECT_TYPE);
        assert_eq!(
            binding.claim_digest,
            hex::encode(
                nexus_fee_budget_claim_digest(&sponsor, "xor#universal", &verified_balance)
                    .as_ref()
            )
        );
        assert_eq!(proof_envelope.committed_amount, Some(50));
        Ok(())
    }
}
