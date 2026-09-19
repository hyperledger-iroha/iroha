//! Structural owner checks only; these scopes grant no finality or publication.

use super::*;
use crate::query::store::LiveQueryStore;

fn state() -> Box<State> {
    Box::new(State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ))
}

fn header() -> BlockHeader {
    BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0)
}

fn stage_structural_manual_addition(block: &mut StateBlock<'_>) {
    let plan = iroha_data_model::nexus::LaneLifecyclePlan {
        additions: vec![iroha_data_model::nexus::LaneConfig {
            id: LaneId::new(1),
            alias: "prepared-geometry".to_owned(),
            ..iroha_data_model::nexus::LaneConfig::default()
        }],
        retire: Vec::new(),
    };
    let update = prepare_lane_lifecycle_update(
        &block.nexus,
        &block.lane_incarnations,
        &block.lane_incarnation_lineage,
        &block.lane_incarnation_activation_heights,
        &block.network_id,
        block._curr_block.hash(),
        &plan,
        block._curr_block.height().get(),
        false,
    )
    .unwrap();
    let manifests = rebind_lane_manifests_for_lifecycle(
        &block.lane_manifests,
        &update.updated_catalog,
        &block.nexus.governance,
    )
    .unwrap();
    let expected_incarnation_root =
        lane_lifecycle_incarnation_root(&block.nexus.lane_catalog, &block.lane_incarnations)
            .unwrap();
    block.nexus.lane_catalog = update.updated_catalog.clone();
    block.nexus.lane_config = update.updated_lane_config.clone();
    block.lane_incarnations = update.updated_lane_incarnations.clone();
    block.lane_incarnation_lineage = update.updated_lane_incarnation_lineage.clone();
    block.lane_incarnation_activation_heights =
        update.updated_lane_incarnation_activation_heights.clone();
    block.lane_manifests = Arc::clone(&manifests);
    block.pending_autoscale_lifecycle = Some(PendingAutoscaleLaneLifecycle {
        catalog_update: update,
        updated_lane_manifests: manifests,
        plan,
        transition: PendingAutoscaleTransition::Manual,
        transition_height: block._curr_block.height().get(),
        expected_incarnation_root,
        runtime_catalog: None,
    });
    block.refresh_canonical_runtime();
}

#[test]
fn carrier_geometry_captures_original_predecessor_and_drop_does_not_publish() {
    let state = state();
    let before = state.canonical_runtime.view().get().clone();
    let world_before = norito::json::to_json(&state.world).unwrap();
    let prepared = {
        let mut block = state.merge_preexecution_block(header());
        stage_structural_manual_addition(&mut block);
        let prepared = block.prepare_carrier_geometry().unwrap();
        assert_eq!(prepared._predecessor, before);
        assert_eq!(&prepared._successor, block.canonical_runtime.get());
        assert_eq!(prepared._header, header());
        assert!(prepared._pending.is_some());
        assert!(!prepared.is_identity_transition(header()));
        prepared
    };
    drop(prepared);
    assert_eq!(state.canonical_runtime.view().get(), &before);
    assert_eq!(norito::json::to_json(&state.world).unwrap(), world_before);
    assert_eq!(state.kura.blocks_count(), 0);
}

#[test]
fn carrier_geometry_identity_requires_its_exact_captured_header() {
    let state = state();
    let block = state.merge_preexecution_block(header());
    let prepared = block.prepare_carrier_geometry().unwrap();
    assert!(prepared.is_identity_transition(header()));
    let mut foreign_header = header();
    foreign_header.set_view_change_index(1);
    assert!(!prepared.is_identity_transition(foreign_header));
}

#[test]
fn carrier_geometry_rejects_changed_header_and_forged_pending_predecessor() {
    let state = state();
    let mut block = state.merge_preexecution_block(header());
    stage_structural_manual_addition(&mut block);
    let original = block._curr_block;
    block._curr_block = BlockHeader::new(NonZeroU64::MIN, None, None, 2, 0);
    assert!(block.prepare_carrier_geometry().is_err());
    block._curr_block = original;
    let pending = block.pending_autoscale_lifecycle.as_mut().unwrap();
    pending
        .catalog_update
        .previous_lane_incarnation_lineage
        .get_mut(&LaneId::SINGLE)
        .unwrap()
        .generation += 1;
    assert!(block.prepare_carrier_geometry().is_err());
}

#[test]
fn carrier_geometry_rejects_ownerless_successor_and_ignores_physical_cache() {
    let state = state();
    let mut block = state.merge_preexecution_block(header());
    stage_structural_manual_addition(&mut block);
    state.nexus.write().lane_catalog = LaneCatalog::default();
    state.nexus.write().autoscale.enabled = !block.nexus.autoscale.enabled;
    block.prepare_carrier_geometry().unwrap();
    block.pending_autoscale_lifecycle = None;
    assert!(block.prepare_carrier_geometry().is_err());
}

#[test]
fn carrier_geometry_replacement_uses_actual_undo_including_retired_lineage() {
    let state = state();
    let predecessor = state.canonical_runtime.view().get().clone();
    // Retain a real MV metadata change, without constructing a block/QC fixture.
    let mut tip = state.canonical_runtime.block();
    tip.get_mut()
        .lane_incarnation_lineage
        .push(SnapshotLaneIncarnationLineage {
            lane_id: LaneId::new(7),
            generation: 1,
            incarnation: Hash::new(b"retained geometry tip"),
            activation_height: 0,
        });
    tip.commit();
    let current = state.canonical_runtime.view().get().clone();
    {
        let ordinary = state.merge_preexecution_block(header());
        let prepared = ordinary.prepare_carrier_geometry().unwrap();
        assert_eq!(prepared._predecessor, current);
    }
    {
        let replacement = state.block_and_revert(header());
        let prepared = replacement.prepare_carrier_geometry().unwrap();
        assert_eq!(prepared._predecessor, predecessor);
        assert_ne!(prepared._predecessor, current);
    }
    assert_eq!(state.canonical_runtime.view().get(), &current);
    assert_eq!(
        state.canonical_runtime.predecessor_view().get(),
        &Some(predecessor)
    );
}
