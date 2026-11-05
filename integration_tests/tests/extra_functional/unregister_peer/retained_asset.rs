//! Read the isolated observer's balance by authenticated replay of its original store.
use std::{collections::BTreeSet, num::NonZeroUsize, path::Path, sync::Arc};

use eyre::{Result, ensure, eyre};
use iroha_allocation::AllocationBudget;
use iroha_config::{
    base::{env::MockEnv, read::ConfigReader},
    kura::InitMode,
    parameters::user,
    snapshot::Mode,
};
use iroha_core::{
    governance::manifest::LaneManifestRegistry,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World, WorldReadOnly as _},
    sumeragi::node::replay_check::{
        ReplayCheckInputs, ReplayCheckOutcome, ReplayCheckReport, ReplayCheckStart, check_replay,
        copy_store,
    },
};
use iroha_data_model::prelude::*;
use iroha_genesis::GenesisBlock;
use iroha_model_base::{chain::ChainId, peer::PeerId};

/// Reconstruct this small, snapshot-disabled permissioned fixture using the
/// actual run configuration and independently retained signed genesis. The
/// source is locked and read-only; only the temporary Strict copy is executed.
#[allow(clippy::too_many_lines)]
pub(super) fn replay_asset(
    config_path: &Path,
    store: &Path,
    genesis: &GenesisBlock,
    chain: &ChainId,
    voters: &BTreeSet<PeerId>,
    asset: &AssetId,
    expected_height: u64,
) -> Result<Quantity> {
    ensure!(
        (1..=16).contains(&expected_height),
        "unexpected fixture replay height"
    );
    // This fixture uses ordinary permissioned startup, without --sora or
    // environment policy overrides. Read the exact saved source graph, rather
    // than reconstructing defaults or importing a later World snapshot.
    let reader = ConfigReader::new()
        .with_env(MockEnv::default())
        .read_toml_with_extends(config_path)
        .map_err(|error| eyre!("read observer run config: {error:?}"))?;
    let mut config = reader
        .read_and_complete::<user::Root>()
        .map_err(|error| eyre!("complete observer run config: {error:?}"))?
        .parse()
        .map_err(|error| eyre!("parse observer run config: {error:?}"))?;
    config.apply_storage_budget();
    ensure!(
        config.common.chain == *chain,
        "observer chain label changed"
    );
    ensure!(
        config.kura.store_dir.resolve_relative_path() == store,
        "observer store changed"
    );
    ensure!(
        matches!(config.snapshot.mode, Mode::Disabled),
        "fixture must replay from genesis"
    );
    ensure!(
        config
            .snapshot
            .upgrade_checkpoint
            .checkpoint()
            .map_err(|error| eyre!("invalid observer upgrade checkpoint: {error}"))?
            .is_none(),
        "unexpected upgrade checkpoint"
    );
    ensure!(
        !config.nexus.compliance.enabled,
        "unexpected fixture compliance policy"
    );
    ensure!(
        config.genesis.expected_hash == genesis.0.hash(),
        "observer genesis pin changed"
    );
    let authority = AccountId::new(config.genesis.public_key.clone());
    iroha_core::validate_genesis_block(&genesis.0, &authority)?;
    let (epoch, _) =
        iroha_data_model::sumeragi_finality::authenticated_genesis(&genesis.0)?.into_parts();
    ensure!(
        epoch.mode == iroha_data_model::parameter::system::ConsensusMode::Permissioned,
        "fixture must retain its signed permissioned profile"
    );
    ensure!(
        epoch
            .committee
            .iter()
            .map(|member| member.validator.clone())
            .collect::<BTreeSet<_>>()
            == *voters,
        "fixture signed committee changed"
    );

    let budget = AllocationBudget::new(config.pipeline.ivm_execution_max_bytes);
    let mut inspection = config.kura.clone();
    inspection.init_mode = InitMode::Fast;
    let (source, _) = Kura::new_with_configured_lane_catalog(
        &inspection,
        &config.nexus.lane_config,
        &config.nexus.configured_lane_catalog,
    )?;
    ensure!(
        u64::try_from(source.exact_durable_blocks_count()?)? == expected_height,
        "isolated observer retained a different durable prefix"
    );
    let stored = source
        .get_block(NonZeroUsize::MIN, &budget)?
        .ok_or_else(|| eyre!("observer store omitted signed genesis"))?;
    ensure!(
        stored.canonical_resultless_proposal()?.encode_wire()?
            == genesis.0.canonical_resultless_proposal()?.encode_wire()?,
        "retained genesis differs from the independent original"
    );
    let copy = copy_store(
        &source,
        &config.kura,
        &config.nexus.lane_config,
        &config.nexus.configured_lane_catalog,
        &mut |_| {},
    )?;
    drop(source);
    ensure!(
        u64::try_from(copy.exact_durable_blocks_count()?)? == expected_height,
        "copy changed the durable prefix"
    );
    State::preflight_configured_primary_geometry_replay(
        &copy,
        &epoch.network_id,
        &config.nexus.configured_lane_catalog,
    )?;
    let mut world = World::try_with_execution_budget(
        [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&authority)],
        [Account::new(authority.clone()).build(&authority)],
        [],
        &budget,
    )?;
    iroha_core::sns::seed_genesis_alias_bootstrap(
        &mut world,
        &stored,
        &config.nexus.dataspace_catalog,
    )?;
    let mut state = State::try_new_with_chain_and_network_id_with_default_telemetry(
        budget,
        world,
        Arc::clone(&copy),
        LiveQueryStore::from_config(config.live_query_store, Default::default())
            .into_inert_handle(),
        chain.clone(),
        epoch.network_id,
    )?;
    // The same public owners and ordering as daemon Strict startup. These
    // setters install only node resources; signed genesis supplies validity.
    ensure!(
        state.install_node_zk(config.zk.clone())?.is_empty(),
        "unexpected local ZK validity override"
    );
    ensure!(
        state
            .install_node_pipeline(config.pipeline.clone())?
            .is_empty(),
        "unexpected local pipeline validity override"
    );
    state.set_crypto(config.crypto.clone());
    state.set_oracle(config.oracle.clone());
    state.install_node_fraud_monitoring(config.fraud_monitoring.clone());
    state.set_gov(config.gov.clone());
    state.content = config.content.clone();
    state.set_settlement(config.settlement);
    let baseline = Arc::new(LaneManifestRegistry::from_config(
        &config.nexus.configured_lane_catalog,
        &config.nexus.governance,
        &config.nexus.registry,
    ));
    baseline.validate_active_coverage_for_catalog(&config.nexus.configured_lane_catalog)?;
    let nexus = state.nexus_with_committed_catalog(config.nexus.clone())?;
    let manifests = state.lane_manifests_with_committed_catalog(&baseline, &nexus)?;
    state.install_materialized_lane_manifests_for_catalog(
        &manifests,
        &nexus.lane_catalog,
        &nexus.governance,
    )?;
    state.install_lane_compliance_engine(None);
    state.prepare_configured_primary_geometry_anchor(&nexus.configured_lane_catalog)?;
    state.restore_kura_lane_segments_before_startup_replay()?;
    iroha_core::smartcontracts::isi::sccp::params::validate_committed_staking_policy(
        &state.world_view(),
    )?;
    state.set_nexus_from_config(nexus)?;
    let state = Arc::new(state);
    let report = check_replay(
        ReplayCheckInputs {
            state: Arc::clone(&state),
            source_lanes: copy.store_root().join("lanes"),
            genesis_account: authority,
            consensus_mode: epoch.mode,
        },
        &mut |height, tip| {
            assert!(
                height <= expected_height && tip == expected_height,
                "replay escaped its exact retained prefix"
            );
        },
    )?;
    require_complete_replay(
        &report,
        u64::try_from(state.committed_height())?,
        expected_height,
    )?;
    let view = state.try_view_once()?;
    Ok(view.world().asset(asset)?.value().as_ref().clone())
}

fn require_complete_replay(report: &ReplayCheckReport, applied: u64, expected: u64) -> Result<()> {
    ensure!(
        report.start == ReplayCheckStart::Genesis
            && report.outcome == ReplayCheckOutcome::Matched
            && report.replayed_to == expected
            && applied == expected,
        "observer original replay did not match its complete certified prefix: {report:?}"
    );
    Ok(())
}

#[test]
fn retained_replay_requires_genesis_complete_success_and_exact_state_height() -> Result<()> {
    let report = ReplayCheckReport {
        start: ReplayCheckStart::Genesis,
        replayed_to: 5,
        outcome: ReplayCheckOutcome::Matched,
    };
    require_complete_replay(&report, 5, 5)?;
    assert!(require_complete_replay(&report, 4, 5).is_err());
    assert!(require_complete_replay(&report, 6, 5).is_err());
    assert!(require_complete_replay(&report, 5, 4).is_err());
    for invalid in [
        ReplayCheckReport {
            start: ReplayCheckStart::Snapshot(4),
            ..report.clone()
        },
        ReplayCheckReport {
            replayed_to: 4,
            ..report.clone()
        },
        ReplayCheckReport {
            outcome: ReplayCheckOutcome::MissingHistory { height: 5 },
            ..report
        },
    ] {
        assert!(require_complete_replay(&invalid, 5, 5).is_err());
    }
    Ok(())
}
