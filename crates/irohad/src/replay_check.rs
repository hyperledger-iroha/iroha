//! `iroha3d --check-replay`: replay a copy of a stopped node's store with this build before it is
//! installed, and report the first block whose certified execution result this build does not
//! reproduce (`specs/live_runtime_upgrades.md` §9.2).
//!
//! The store is opened as `--check-storage` opens it (emergency-Fast Kura: the store-root lock,
//! the durable commit marker and read-only journals) and is never written. Signed genesis is
//! authenticated against the configured trust anchor first. The whole store is then copied, file
//! by file, into a temporary directory, and the lock on the original is released. On the copy the
//! State is built as a Strict startup builds it: restored from the newest authenticated snapshot
//! (the snapshot directory is only read), or empty to re-execute signed genesis, and never below
//! the configured upgrade checkpoint. Every later block is re-executed and its execution result
//! compared with the stored certificate ([`iroha_core::sumeragi::node::replay_check`]). The
//! tiered-state backend stays detached and no runtime-only secret is opened. Progress goes to
//! standard error; standard output ends with the one-line JSON report.

use super::*;
use iroha_allocation::AllocationBudget;
use iroha_core::{
    state::upgrade_checkpoint::UpgradeCheckpointV1,
    sumeragi::node::replay_check::{
        self, ReplayCheckInputs, ReplayCheckOutcome, ReplayCheckReport, ReplayCheckStart,
    },
};
use norito::derive::{JsonDeserialize, JsonSerialize};
use std::num::NonZeroUsize;

/// `start.source` of a replay that re-executed signed genesis first.
pub const START_GENESIS: &str = "genesis";
/// `start.source` of a replay that started from an authenticated snapshot.
pub const START_SNAPSHOT: &str = "snapshot";
/// `start.source` when nothing could be replayed: an empty store, or no signed genesis.
pub const START_NONE: &str = "none";
/// `outcome` when every replayed block reproduced its certified result.
pub const OUTCOME_MATCHED: &str = "matched";
/// `outcome` when a block's replayed result differs from its certificate.
pub const OUTCOME_DIVERGED: &str = "diverged";
/// `outcome` when the store no longer holds a block body the replay needs.
pub const OUTCOME_MISSING_HISTORY: &str = "missing_history";

/// Blocks between progress lines on standard error.
const PROGRESS_INTERVAL: u64 = 128;
/// Copied bytes between progress lines on standard error.
const COPY_PROGRESS_BYTES: u64 = 1 << 30;

/// Where the replay started.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ReplayStartV1 {
    /// Height of the State the replay started from (0 before signed genesis).
    pub height: u64,
    /// [`START_GENESIS`], [`START_SNAPSHOT`] or [`START_NONE`].
    pub source: String,
}

/// The first block whose replayed execution result differs from its certificate.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ReplayDivergenceV1 {
    /// Height of the block.
    pub height: u64,
    /// The first differing commitment component, such as `world_state_root`.
    pub component: String,
    /// The value the stored certificate commits to (hashes in lowercase hex).
    pub expected: String,
    /// The value this build produced.
    pub actual: String,
    /// Every differing component, in report order.
    pub differing_components: Vec<String>,
}

/// Result printed by `iroha3d --check-replay`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ReplayCheckReportV1 {
    /// Durable committed height of the store.
    pub tip_height: u64,
    /// Where the replay started.
    pub start: ReplayStartV1,
    /// Height of the State after the last block this build reproduced; the start height when it
    /// reproduced none.
    pub replayed_to: u64,
    /// [`OUTCOME_MATCHED`], [`OUTCOME_DIVERGED`] or [`OUTCOME_MISSING_HISTORY`].
    pub outcome: String,
    /// The first divergence, when `outcome` is [`OUTCOME_DIVERGED`].
    pub divergence: Option<ReplayDivergenceV1>,
    /// The first height without a stored body, when `outcome` is [`OUTCOME_MISSING_HISTORY`].
    pub missing_height: Option<u64>,
}

impl ReplayCheckReportV1 {
    /// Whether every replayed block reproduced its certified result.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.outcome == OUTCOME_MATCHED
    }

    /// The report of a check that replayed nothing, with `missing_height` when the store lacks
    /// the history the replay needs to start.
    fn unstarted(tip_height: u64, missing_height: Option<u64>) -> Self {
        Self {
            tip_height,
            start: ReplayStartV1 {
                height: 0,
                source: START_NONE.to_owned(),
            },
            replayed_to: 0,
            outcome: if missing_height.is_some() {
                OUTCOME_MISSING_HISTORY
            } else {
                OUTCOME_MATCHED
            }
            .to_owned(),
            divergence: None,
            missing_height,
        }
    }

    /// Project the core verdict onto the report format.
    fn from_core(tip_height: u64, report: &ReplayCheckReport) -> Self {
        let (outcome, divergence, missing_height) = match &report.outcome {
            ReplayCheckOutcome::Matched => (OUTCOME_MATCHED, None, None),
            ReplayCheckOutcome::Diverged(divergence) => (
                OUTCOME_DIVERGED,
                Some(ReplayDivergenceV1 {
                    height: divergence.height,
                    component: divergence.first.component.name().to_owned(),
                    expected: divergence.first.expected.clone(),
                    actual: divergence.first.actual.clone(),
                    differing_components: divergence
                        .components
                        .iter()
                        .map(|component| component.name().to_owned())
                        .collect(),
                }),
                None,
            ),
            ReplayCheckOutcome::MissingHistory { height } => {
                (OUTCOME_MISSING_HISTORY, None, Some(*height))
            }
        };
        Self {
            tip_height,
            start: ReplayStartV1 {
                height: report.start.height(),
                source: match report.start {
                    ReplayCheckStart::Genesis => START_GENESIS,
                    ReplayCheckStart::Snapshot(_) => START_SNAPSHOT,
                }
                .to_owned(),
            },
            replayed_to: report.replayed_to,
            outcome: outcome.to_owned(),
            divergence,
            missing_height,
        }
    }

    /// One sentence explaining a failed check.
    fn failure_summary(&self) -> String {
        match (&self.divergence, self.missing_height) {
            (Some(divergence), _) => format!(
                "block {} does not reproduce its certified result: {} is {} with this build, the certificate commits to {}",
                divergence.height, divergence.component, divergence.actual, divergence.expected
            ),
            (None, Some(height)) => format!(
                "the store holds no body at height {height}; blocks up to height {} reproduce their certified results",
                self.replayed_to
            ),
            (None, None) => format!("replay check outcome `{}`", self.outcome),
        }
    }
}

fn failure(message: impl Into<String>) -> Report<MainError> {
    Report::new(MainError::CheckReplay).attach(message.into())
}

/// Replay a copy of the stopped node's store with this build (see the module documentation).
///
/// # Errors
///
/// [`MainError::CheckReplay`] when the store cannot be opened or read, signed genesis fails its
/// trust anchor, the replay State cannot be built, or a block fails to replay for a reason other
/// than a different result. A different result or missing history is reported, not an error.
pub fn check_replay(config: &Config) -> ReportResult<ReplayCheckReportV1, MainError> {
    let store_root = config.kura.store_dir.resolve_relative_path();
    match std::fs::symlink_metadata(&store_root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ReplayCheckReportV1::unstarted(0, None));
        }
        Err(error) => {
            return Err(failure(format!(
                "cannot inspect Kura store {}: {error}",
                store_root.display()
            )));
        }
        Ok(_) => {}
    }
    let source = open_store_read_only(config)?;
    let tip = source
        .exact_durable_blocks_count()
        .map_err(|error| failure(format!("cannot read the durable Kura height: {error}")))?;
    let tip_height = u64::try_from(tip).map_err(|error| failure(error.to_string()))?;
    if tip == 0 {
        return Ok(ReplayCheckReportV1::unstarted(0, None));
    }
    // One pool for the copy, the snapshot restore and the replay State, as at startup.
    let budget = AllocationBudget::new(config.pipeline.ivm_execution_max_bytes);
    let Some(stored_genesis) = source
        .get_block(NonZeroUsize::MIN, &budget)
        .map_err(|error| failure(format!("cannot read signed genesis: {error}")))?
    else {
        return Ok(ReplayCheckReportV1::unstarted(tip_height, Some(1)));
    };
    let genesis: &SignedBlock = stored_genesis.as_ref();
    ResolvedGenesisTrustAnchor::resolve(
        &config.genesis.public_key,
        config.genesis.expected_hash,
        None,
    )
    .and_then(|anchor| anchor.verify(genesis))
    .change_context(MainError::CheckReplay)?;
    require_authenticated_genesis_crypto_implementations(genesis, &config.genesis.public_key)
        .change_context(MainError::CheckReplay)?;
    let (consensus_mode, context) = signed_genesis_context_metadata(genesis).map_err(failure)?;
    startup_root_topology::validate(&config.nexus, consensus_mode, context.root_scope)
        .map_err(failure)?;
    let network_id = NetworkId::from_genesis_hash(config.genesis.expected_hash);
    let lane_manifests = freeze_lane_manifests_for_startup_replay(&config.nexus)
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))
        .attach("lane manifest registry is not ready")?;
    eprintln!(
        "check-replay: copying the store {} ({tip_height} blocks) into the system temporary directory",
        store_root.display()
    );
    let started = Instant::now();
    let mut reported = 0;
    let copy = replay_check::copy_store(
        &source,
        &config.kura,
        &config.nexus.lane_config,
        &config.nexus.configured_lane_catalog,
        &mut |bytes| {
            if bytes >= reported + COPY_PROGRESS_BYTES {
                reported = bytes;
                eprintln!(
                    "check-replay: copied {} MiB ({} s)",
                    bytes >> 20,
                    started.elapsed().as_secs()
                );
            }
        },
    )
    .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    // Everything below reads only the copy: release the original store's lock.
    drop(source);
    eprintln!(
        "check-replay: copied the store into {} ({} s)",
        copy.store_root().display(),
        started.elapsed().as_secs()
    );
    let copied = copy
        .exact_durable_blocks_count()
        .map_err(|error| failure(format!("cannot read the copy's durable height: {error}")))?;
    let checkpoint = config
        .snapshot
        .upgrade_checkpoint
        .checkpoint()
        .map_err(failure)?
        .map(|checkpoint| UpgradeCheckpointV1::from_config(&checkpoint));
    if let Some(checkpoint) = &checkpoint {
        copy.set_upgrade_checkpoint(checkpoint.clone())
            .map_err(failure)?;
    }
    let (mut state, from_snapshot) = match restore_snapshot(
        config,
        &copy,
        &budget,
        &lane_manifests,
        network_id,
        copied,
    ) {
        Ok(state) => (state, true),
        Err(error) => {
            if let Some(checkpoint) = &checkpoint {
                return Err(Report::new(error).change_context(MainError::CheckReplay).attach(format!(
                        "blocks up to the upgrade checkpoint at height {} are the previous release's history and are never re-executed; replay needs the checkpoint snapshot or a later snapshot of this release",
                        checkpoint.height
                    )));
            }
            if !snapshot_failure_allows_empty_state_fallback(&error, false) {
                return Err(Report::new(error)
                    .change_context(MainError::CheckReplay)
                    .attach("the newest snapshot cannot be restored"));
            }
            if !matches!(error, TryReadSnapshotError::NotFound) {
                eprintln!(
                    "check-replay: no usable snapshot ({error}); replaying from signed genesis"
                );
            }
            (
                empty_state(config, &copy, &budget, genesis, network_id)?,
                false,
            )
        }
    };
    configure_state(&mut state, config, from_snapshot, &lane_manifests)?;
    let state: Arc<State> = Arc::from(state);
    match state.committed_height() {
        0 => eprintln!("check-replay: re-executing signed genesis"),
        height => eprintln!("check-replay: starting from the snapshot at height {height}"),
    }
    let started = Instant::now();
    let report = replay_check::check_replay(
        ReplayCheckInputs {
            state,
            source_lanes: copy.store_root().join("lanes"),
            genesis_account: AccountId::new(config.genesis.public_key.clone()),
            consensus_mode,
        },
        &mut |height, stored| {
            if height.is_multiple_of(PROGRESS_INTERVAL) || height == stored {
                eprintln!(
                    "check-replay: replayed {height}/{stored} blocks ({} s)",
                    started.elapsed().as_secs()
                );
            }
        },
    )
    .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    Ok(ReplayCheckReportV1::from_core(tip_height, &report))
}

/// Run `--check-replay`: print the report as Norito JSON and fail unless every block matched.
///
/// # Errors
///
/// [`MainError::CheckReplay`] when the check could not run, or after printing a report whose
/// replay diverged or lacks history.
pub fn run_check_replay(config: &Config) -> ReportResult<(), MainError> {
    let report = check_replay(config)?;
    let json = norito::json::to_json(&report).map_err(|error| failure(error.to_string()))?;
    println!("{json}");
    if report.is_ok() {
        Ok(())
    } else {
        Err(failure(report.failure_summary()))
    }
}

/// Open the existing store in emergency-Fast mode: lock, marker check, read-only journals.
fn open_store_read_only(config: &Config) -> ReportResult<Arc<Kura>, MainError> {
    let mut kura_config = config.kura.clone();
    kura_config.init_mode = InitMode::Fast;
    Kura::new_with_configured_lane_catalog(
        &kura_config,
        &config.nexus.lane_config,
        &config.nexus.configured_lane_catalog,
    )
    .map(|(kura, _)| kura)
    .map_err(|error| {
        failure(format!(
            "cannot open Kura store {} read-only: {error}",
            config.kura.store_dir.resolve_relative_path().display()
        ))
    })
}

/// The empty State a Strict startup builds when no snapshot is restored.
fn empty_state(
    config: &Config,
    kura: &Arc<Kura>,
    budget: &AllocationBudget,
    genesis: &SignedBlock,
    network_id: NetworkId,
) -> ReportResult<Box<State>, MainError> {
    preflight_empty_state_snapshot_fallback(
        kura,
        &network_id,
        &config.nexus.configured_lane_catalog,
    )
    .change_context(MainError::CheckReplay)?;
    let key = config.genesis.public_key.clone();
    let mut world = World::try_with_execution_budget(
        [genesis_domain(key.clone())],
        [genesis_account(key)],
        [],
        budget,
    )
    .map_err(|error: iroha_core::state::WorldConstructionError| {
        Report::new(error).change_context(MainError::CheckReplay)
    })?;
    iroha_core::sns::seed_genesis_alias_bootstrap(
        &mut world,
        genesis,
        &config.nexus.dataspace_catalog,
    )
    .map_err(|error| failure(format!("genesis alias bootstrap: {error}")))?;
    State::try_new_with_chain_and_network_id(
        budget.clone(),
        world,
        Arc::clone(kura),
        LiveQueryStore::from_config(config.live_query_store, ShutdownSignal::new())
            .into_inert_handle(),
        config.common.chain.clone(),
        network_id,
        #[cfg(feature = "telemetry")]
        StateTelemetry::default(),
    )
    .map(Box::new)
    .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))
}

/// Restore the newest authenticated snapshot whose certified successor the copy holds, exactly as
/// a Strict startup restores it. The snapshot directory is only read.
fn restore_snapshot(
    config: &Config,
    kura: &Arc<Kura>,
    budget: &AllocationBudget,
    lane_manifests: &LaneManifestRegistryHandle,
    network_id: NetworkId,
    block_count: usize,
) -> Result<Box<State>, TryReadSnapshotError> {
    if !snapshot_mode_allows_restore(config.snapshot.mode) {
        return Err(TryReadSnapshotError::NotFound);
    }
    let live_query_store =
        LiveQueryStore::from_config(config.live_query_store, ShutdownSignal::new())
            .into_inert_handle();
    let verification_key = config
        .snapshot
        .verification_public_key
        .as_ref()
        .unwrap_or_else(|| config.common.key_pair.public_key());
    let read_buffer_budget = AllocationBudget::new(config.snapshot.max_read_buffer_bytes.get());
    try_read_snapshot_with_limits(
        budget,
        config.snapshot.store_dir.resolve_relative_path(),
        kura,
        lane_manifests,
        &config.nexus,
        || live_query_store.clone(),
        iroha_core::kura::BlockCount(block_count),
        config.snapshot.merkle_chunk_size_bytes,
        config.snapshot.max_payload_bytes,
        config.snapshot.resources,
        verification_key,
        &config.common.chain,
        &network_id,
        &config.zk,
        #[cfg(feature = "telemetry")]
        StateTelemetry::default(),
        &read_buffer_budget,
    )
}

/// Install the node configuration a Strict startup installs before replay, on the copy. The
/// tiered-state backend stays detached: it is a derived local projection whose directory belongs
/// to the node.
fn configure_state(
    state: &mut State,
    config: &Config,
    from_snapshot: bool,
    lane_manifests: &LaneManifestRegistryHandle,
) -> ReportResult<(), MainError> {
    if state.network_id != NetworkId::from_genesis_hash(config.genesis.expected_hash) {
        return Err(failure(format!(
            "restored state network id {} differs from the genesis.expected_hash-derived id",
            state.network_id
        )));
    }
    if state.chain_id != config.common.chain {
        return Err(failure(
            "restored native chain identity differs from the configured chain",
        ));
    }
    if !from_snapshot {
        // A restored snapshot installed it before reconciliation.
        install_zk_config_before_kura_replay(state, config)
            .change_context(MainError::CheckReplay)?;
    }
    apply_state_runtime_config_before_snapshot_auth(state, config)
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    let policies =
        install_lane_policies_for_startup_replay(state, config.nexus.clone(), lane_manifests)
            .change_context(MainError::CheckReplay)?;
    apply_state_geometry_config_before_kura_replay(state, &policies)
        .change_context(MainError::CheckReplay)?;
    install_node_pipeline_config(state, config)
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    state.set_oracle(config.oracle.clone());
    state.install_node_fraud_monitoring(config.fraud_monitoring.clone());
    state.set_gov(config.gov.clone());
    Ok(())
}

#[cfg(test)]
mod tests;
