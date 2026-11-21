//! Candidate-linked exact checkpoint restore and original suffix replay, with no genesis fallback.

use super::*;
use crate::{
    governance::manifest::LaneManifestRegistry, query::store::LiveQueryStore,
    sumeragi::node::replay_check,
};

fn restore(
    directory: &Path,
    input: &RootInput,
    configuration: TestChainConfig,
) -> ReplayedFloorCheckpoint {
    // This is the same deterministic fixture's local configuration, never a substitute World.
    // The snapshot owns geometry, rows, committed policies and the actual H/H+1 boundary.
    let nexus = configuration.nexus.unwrap_or_default();
    let kura_configuration = kura_config(&directory.join("kura"));
    let (source, _) = Kura::new_with_configured_lane_catalog(
        &kura_configuration,
        &nexus.lane_config,
        &nexus.configured_lane_catalog,
    )
    .unwrap();
    assert!(source.is_locked_read_only_source());
    let budget = AllocationBudget::new(configuration.pipeline.ivm_execution_max_bytes);
    let genesis = source
        .get_block(NonZeroUsize::MIN, &budget)
        .unwrap()
        .unwrap();
    assert_eq!(NetworkId::from_genesis_hash(genesis.hash()), input.network);
    let original_tip = source
        .get_block(
            NonZeroUsize::new(input.tip.try_into().unwrap()).unwrap(),
            &budget,
        )
        .unwrap()
        .unwrap();
    assert_eq!(original_tip.hash().as_ref(), &input.tip_hash);
    assert_eq!(
        source.exact_durable_blocks_count().unwrap() as u64,
        input.tip
    );
    let copy = replay_check::copy_store(
        &source,
        &kura_configuration,
        &nexus.lane_config,
        &nexus.configured_lane_catalog,
        &mut |_| {},
    )
    .unwrap();
    let world = World::try_with_execution_budget([], [], [], &budget).unwrap();
    let mut template = State::try_new_with_chain_and_network_id(
        budget.clone(),
        world,
        copy,
        LiveQueryStore::start_test(),
        configuration.chain_id,
        input.network,
        #[cfg(feature = "telemetry")]
        crate::telemetry::StateTelemetry::default(),
    )
    .unwrap();
    template
        .install_node_pipeline(configuration.pipeline)
        .unwrap();
    template.install_node_fraud_monitoring(configuration.fraud_monitoring);
    if let Some(crypto) = configuration.crypto {
        template.set_crypto(crypto);
    }
    if let Some(zk) = configuration.zk {
        template.set_zk(zk).unwrap();
    }
    if let Some(governance) = configuration.governance {
        template.set_gov(governance);
    }
    let manifests = configuration.lane_manifests.unwrap_or_else(|| {
        Arc::new(LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance))
    });
    *template.lane_manifests.write() = Arc::clone(&manifests);
    let snapshots = SnapshotCheckpoints::from_config(
        &snapshot_config(&directory.join("snapshots")),
        snapshot_key().public_key().clone(),
        AllocationBudget::new(defaults::snapshot::MAX_READ_BUFFER_BYTES.get()),
    );
    let held = snapshots
        .held(template.chain_id_ref(), input.network)
        .unwrap();
    assert!(
        held.iter()
            .any(|checkpoint| checkpoint.height == input.floor)
    );
    assert!(
        held.iter()
            .any(|checkpoint| checkpoint.height > input.floor),
        "the fixture must catch accidental newest-snapshot substitution"
    );
    assert!(
        snapshots
            .restore_exact_for_floor(
                1,
                Arc::clone(&source),
                &template,
                &nexus,
                &manifests,
                None,
                Default::default()
            )
            .is_err(),
        "genesis is never a checkpoint fallback"
    );
    assert!(
        snapshots
            .restore_exact_for_floor(
                input.tip + 1,
                Arc::clone(&source),
                &template,
                &nexus,
                &manifests,
                None,
                Default::default()
            )
            .is_err(),
        "a missing exact floor cannot choose newest"
    );
    let mut replayed = Vec::new();
    let completed = snapshots
        .restore_exact_for_floor(
            input.floor,
            source,
            &template,
            &nexus,
            &manifests,
            None,
            Default::default(),
        )
        .unwrap()
        .replay(&mut |height, _| replayed.push(height))
        .unwrap();
    assert_eq!(
        replayed,
        ((input.floor + 1)..=input.tip).collect::<Vec<_>>(),
        "every original suffix block must execute in the source-removed candidate"
    );
    let state = completed.checked_replayed_state().unwrap();
    assert_eq!(state.committed_height() as u64, input.tip);
    let view = state.view();
    let tip = view.native_execution_tip().unwrap();
    let floor =
        crate::runtime::checkpoint::history_boundary(view.world(), tip.creation_time_ms()).unwrap();
    assert_eq!(
        floor.height, input.floor,
        "the actual restored effective boundary selects the oldest floor"
    );
    let block = state
        .kura()
        .get_block(
            NonZeroUsize::new(input.floor.try_into().unwrap()).unwrap(),
            &budget,
        )
        .unwrap()
        .unwrap();
    assert_eq!(floor.block_hash, block.hash());
    drop(view);
    completed
}

pub(super) fn rehearse() {
    assert!(
        !source_root().join(HANDLER).exists(),
        "the executable must be linked from the omitted-source tree"
    );
    let directory = fixture_dir();
    let inputs: Inputs =
        norito::json::from_slice(&std::fs::read(directory.join("roots.json")).unwrap()).unwrap();
    let (owner, aliases) = reviewed_aliases();
    let catalog = crate::runtime::release_catalog().unwrap();
    for id in aliases.keys() {
        assert!(
            catalog.definitions.contains_key(id),
            "immutable descriptor survives physical code removal"
        );
        assert_eq!(catalog.native_module(id), None);
        assert!(
            !catalog.executes_definition(id),
            "omitted executable support must never be advertised"
        );
    }
    let parent = restore(&directory.join("parent"), &inputs.parent, parent_config());
    let home = crate::sns::dataspace_id_for_sns_alias(ALIAS).unwrap();
    let mut configuration = child_fixture::config_for_root(inputs.parent.network, home);
    configuration.genesis_parameters.push(checkpoint_policy());
    let child = restore(&directory.join("child"), &inputs.child, configuration);
    // Both opaque replay owners stay alive while original complete-State cuts and receipt
    // verifiers inspect the exact parent and its registered child's final anchored cursor.
    let parent_state = parent.checked_replayed_state().unwrap();
    let child_state = child.checked_replayed_state().unwrap();
    let parent_cut = parent_state.runtime_retirement_cut().unwrap();
    let child_cuts = [child_state.runtime_retirement_cut().unwrap()];
    let (manifest, request) = manifest(owner);
    let sources = source_ownership::check_sources(
        &directory.join("reviewed-originals"),
        &manifest,
        source_ownership::manifest_digest(&manifest).unwrap(),
        &request,
        read_limits(),
    )
    .unwrap();
    let parent_root = original_root(parent_cut.view()).unwrap();
    let evidence = verify_source_candidate(&sources, &parent_cut, &child_cuts, &mut |id, root| {
        let state = if root == parent_root {
            parent_state
        } else {
            assert_eq!(root, original_root(child_cuts[0].view()).unwrap());
            child_state
        };
        Ok(crate::runtime::retirement::receipt_proof(&state.view(), id).unwrap())
    })
    .unwrap();
    assert_eq!(evidence.roots(), 2);
    assert_eq!(evidence.aliases(), aliases.len());
    evidence.recheck_sources().unwrap();
    // Reader remains callable after handler deletion; retained stored records are still owned
    // by the unchanged native DTO and State codecs. No native instruction is reintroduced.
    assert_eq!(
        crate::smartcontracts::isi::account_signing_key::account_signing_key(
            parent_cut.view().world(),
            &AccountId::new(payer().public_key().clone())
        ),
        None
    );
    parent.checked_replayed_state().unwrap();
    child.checked_replayed_state().unwrap();
}
