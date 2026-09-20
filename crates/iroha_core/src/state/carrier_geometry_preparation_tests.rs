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
    stage_structural_manual_plan(block, plan);
}

fn stage_structural_manual_plan(
    block: &mut StateBlock<'_>,
    plan: iroha_data_model::nexus::LaneLifecyclePlan,
) {
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
        assert!(prepared.has_pending_lifecycle());
        assert!(prepared.requires_storage_transition());
        assert!(!prepared.requires_queue_custody());
        assert!(prepared.matches_publication_target(&state, header()));
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
    assert!(prepared.matches_publication_target(&state, header()));
    assert!(!prepared.has_pending_lifecycle());
    assert!(!prepared.requires_storage_transition());
    assert!(!prepared.requires_queue_custody());
    let mut foreign_header = header();
    foreign_header.set_view_change_index(1);
    assert!(!prepared.is_identity_transition(foreign_header));
    assert!(!prepared.matches_publication_target(&state, foreign_header));
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

fn storage_fixture() -> (tempfile::TempDir, Box<State>, PreparedCarrierGeometry) {
    let temporary = tempfile::tempdir().unwrap();
    let state = Box::new(State::new_with_pre_genesis_nexus_for_testing(
        World::default(),
        iroha_config::parameters::actual::Nexus::default(),
        LiveQueryStore::start_test(),
    ));
    state
        .tiered_backend
        .lock()
        .reconfigure_without_storage_effects(
            true,
            0,
            0,
            0,
            Some(temporary.path().join("cold")),
            None,
            0,
            0,
        );
    let geometry = {
        let mut block = state.merge_preexecution_block(header());
        stage_structural_manual_addition(&mut block);
        block.prepare_carrier_geometry().unwrap()
    };
    (temporary, state, geometry)
}

#[test]
fn carrier_geometry_completion_requires_original_state_and_exact_header_before_effects() {
    let (temporary, state, mut geometry) = storage_fixture();
    let foreign = State::new_for_testing(
        World::default(),
        Arc::clone(&state.kura),
        LiveQueryStore::start_test(),
    );
    assert!(Arc::ptr_eq(&state.kura, &foreign.kura));
    assert!(!geometry.matches_publication_target(&foreign, header()));
    let mut foreign_header = header();
    foreign_header.set_view_change_index(1);
    let lease = state.kura.try_publication_lease().unwrap();
    geometry
        .prepare_under(&state.tiered_backend.lock(), &lease)
        .unwrap();
    let before = std::fs::read(state.kura.lane_geometry_journal_path()).unwrap();
    for (target, attempted_header) in [(&foreign, header()), (state.as_ref(), foreign_header)] {
        let result = geometry.complete_under(
            target,
            attempted_header,
            &mut state.tiered_backend.lock(),
            &lease,
        );
        assert!(matches!(result, Err(LaneLifecycleError::Storage(detail))
            if detail.contains("original State or carrier header")));
        assert_eq!(
            geometry.raw.as_ref().unwrap().phase(),
            crate::kura::RawGeometryPhase::Captured
        );
        assert!(!geometry.tiered.as_ref().unwrap().is_applied());
        assert!(!temporary.path().join("cold").exists());
        assert_eq!(
            std::fs::read(state.kura.lane_geometry_journal_path()).unwrap(),
            before
        );
    }
    assert!(
        geometry
            .complete_under(&state, header(), &mut state.tiered_backend.lock(), &lease)
            .unwrap()
            .updated_da_mapping()
            .is_some()
    );
    assert_eq!(state.committed_height(), 0);
    assert_eq!(foreign.committed_height(), 0);
}

#[test]
fn carrier_geometry_retirement_and_replacement_require_original_queue_custody() {
    let secondary = iroha_data_model::nexus::LaneConfig {
        id: LaneId::new(1),
        alias: "retiring-geometry".to_owned(),
        ..iroha_data_model::nexus::LaneConfig::default()
    };
    for replace in [false, true] {
        let mut nexus = iroha_config::parameters::actual::Nexus::default();
        nexus.lane_catalog = LaneCatalog::new(
            NonZeroU32::new(2).unwrap(),
            vec![
                iroha_data_model::nexus::LaneConfig::default(),
                secondary.clone(),
            ],
        )
        .unwrap();
        let state = Box::new(State::new_with_pre_genesis_nexus_for_testing(
            World::default(),
            nexus,
            LiveQueryStore::start_test(),
        ));
        let runtime = state.canonical_runtime.view().get().clone();
        let mut geometry = {
            let mut block = state.merge_preexecution_block(header());
            stage_structural_manual_plan(
                &mut block,
                iroha_data_model::nexus::LaneLifecyclePlan {
                    additions: if replace {
                        vec![secondary.clone()]
                    } else {
                        Vec::new()
                    },
                    retire: vec![secondary.id],
                },
            );
            block.prepare_carrier_geometry().unwrap()
        };
        assert!(geometry.has_pending_lifecycle());
        assert!(geometry.requires_storage_transition());
        assert!(geometry.requires_queue_custody());
        assert_eq!(
            geometry
                ._pending
                .as_ref()
                .unwrap()
                .catalog_update
                .replaced_lane_ids
                .contains(&secondary.id),
            replace
        );
        let lease = state.kura.try_publication_lease().unwrap();
        let before = std::fs::read(state.kura.lane_geometry_journal_path()).unwrap();
        let result =
            geometry.complete_under(&state, header(), &mut state.tiered_backend.lock(), &lease);
        assert!(matches!(result, Err(LaneLifecycleError::Storage(detail))
            if detail.contains("no retained original Queue cut")));
        assert!(geometry.raw.is_none());
        assert!(geometry.tiered.is_none());
        assert_eq!(
            std::fs::read(state.kura.lane_geometry_journal_path()).unwrap(),
            before
        );
        assert_eq!(state.canonical_runtime.view().get(), &runtime);
        assert_eq!(state.committed_height(), 0);
    }
}

#[test]
fn carrier_geometry_foreign_lease_refuses_before_descriptor_capture_or_effects() {
    let (temporary, state, mut geometry) = storage_fixture();
    let foreign = Kura::blank_kura_for_testing();
    let lease = foreign.try_publication_lease().unwrap();
    let before = std::fs::read(state.kura.lane_geometry_journal_path()).unwrap();
    assert!(
        geometry
            .resume_under(&mut state.tiered_backend.lock(), &lease)
            .is_err()
    );
    assert!(geometry.raw.is_none());
    assert!(geometry.tiered.is_none());
    assert!(!temporary.path().join("cold").exists());
    assert_eq!(
        std::fs::read(state.kura.lane_geometry_journal_path()).unwrap(),
        before
    );
}

#[test]
fn carrier_geometry_preparation_is_pure_and_root_change_refuses_before_raw_effects() {
    let (temporary, state, mut geometry) = storage_fixture();
    let lease = state.kura.try_publication_lease().unwrap();
    let before = std::fs::read(state.kura.lane_geometry_journal_path()).unwrap();
    geometry
        .prepare_under(&state.tiered_backend.lock(), &lease)
        .unwrap();
    assert_eq!(
        geometry.raw.as_ref().unwrap().phase(),
        crate::kura::RawGeometryPhase::Captured
    );
    assert!(!geometry.tiered.as_ref().unwrap().is_applied());
    let cold = temporary.path().join("cold");
    assert!(!cold.exists());
    let mut foreign = tiered::TieredStateBackend::default();
    let foreign_root = temporary.path().join("foreign");
    foreign.reconfigure_without_storage_effects(
        true,
        0,
        0,
        0,
        Some(foreign_root.clone()),
        None,
        0,
        0,
    );
    assert!(geometry.resume_under(&mut foreign, &lease).is_err());
    assert_eq!(
        geometry.raw.as_ref().unwrap().phase(),
        crate::kura::RawGeometryPhase::Captured
    );
    assert_eq!(
        std::fs::read(state.kura.lane_geometry_journal_path()).unwrap(),
        before
    );
    assert!(!cold.exists());
    assert!(!foreign_root.exists());
    // Repeating preparation must reuse its still-exclusive original claim.
    geometry
        .prepare_under(&state.tiered_backend.lock(), &lease)
        .unwrap();
}

#[test]
fn carrier_geometry_retries_sync_failure_under_held_lease_without_state_publication() {
    let (temporary, state, mut geometry) = storage_fixture();
    let runtime = state.canonical_runtime.view().get().clone();
    let world = norito::json::to_json(&state.world).unwrap();
    let cursors = format!("{:?}", state.da_shard_cursors.read());
    let manifests = Arc::clone(&state.lane_manifests.read());
    let generation = state.state_view_generation();
    let lease = state.kura.try_publication_lease().unwrap();
    geometry
        .prepare_under(&state.tiered_backend.lock(), &lease)
        .unwrap();
    crate::kura::fail_bound_progress_intent_directory_sync_for_tests(0, 0);
    assert!(
        geometry
            .resume_under(&mut state.tiered_backend.lock(), &lease)
            .is_err()
    );
    assert!(geometry.raw.as_ref().unwrap().has_pending_journal_write());
    assert!(!geometry.tiered.as_ref().unwrap().is_applied());
    assert!(!temporary.path().join("cold").exists());
    drop(lease);
    let lease = state.kura.try_publication_lease().unwrap();
    // No nested acquisition: this lease remains held throughout the exact retry.
    geometry
        .resume_under(&mut state.tiered_backend.lock(), &lease)
        .unwrap();
    assert_eq!(
        geometry.raw.as_ref().unwrap().phase(),
        crate::kura::RawGeometryPhase::FilesApplied
    );
    assert!(!geometry.raw.as_ref().unwrap().has_pending_journal_write());
    assert!(geometry.tiered.as_ref().unwrap().is_applied());
    let entry = geometry
        ._pending
        .as_ref()
        .unwrap()
        .catalog_update
        .updated_lane_config
        .entry(LaneId::new(1))
        .unwrap();
    assert!(
        temporary
            .path()
            .join("cold/lanes")
            .join(&entry.kura_segment)
            .is_dir()
    );
    geometry
        .resume_under(&mut state.tiered_backend.lock(), &lease)
        .unwrap();
    assert_eq!(state.canonical_runtime.view().get(), &runtime);
    assert_eq!(norito::json::to_json(&state.world).unwrap(), world);
    assert_eq!(format!("{:?}", state.da_shard_cursors.read()), cursors);
    assert!(Arc::ptr_eq(&state.lane_manifests.read(), &manifests));
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(state.committed_height(), 0);
    // Qualification does not mint source/catalog authority. Undo only the exact
    // fixture-owned effects so its unfinished raw claim is not abandoned.
    geometry
        .tiered
        .as_mut()
        .unwrap()
        .rollback(&mut state.tiered_backend.lock())
        .unwrap();
    geometry
        .raw
        .as_mut()
        .unwrap()
        .rollback_under(&lease)
        .unwrap();
}

#[test]
fn carrier_geometry_completion_requires_original_prepared_descriptors() {
    let (temporary, state, mut geometry) = storage_fixture();
    let lease = state.kura.try_publication_lease().unwrap();
    let before = std::fs::read(state.kura.lane_geometry_journal_path()).unwrap();
    assert!(
        geometry
            .resume_under(&mut state.tiered_backend.lock(), &lease)
            .is_err()
    );
    assert!(
        geometry
            .complete_under(&state, header(), &mut state.tiered_backend.lock(), &lease)
            .is_err()
    );
    assert!(geometry.raw.is_none());
    assert!(geometry.tiered.is_none());
    assert!(!temporary.path().join("cold").exists());
    assert_eq!(
        std::fs::read(state.kura.lane_geometry_journal_path()).unwrap(),
        before
    );
}

#[test]
#[cfg(unix)]
fn carrier_geometry_catalog_sync_retry_preserves_original_mapping_and_state() {
    use std::os::unix::fs::MetadataExt;

    let (_temporary, state, mut geometry) = storage_fixture();
    let runtime = state.canonical_runtime.view().get().clone();
    let world = norito::json::to_json(&state.world).unwrap();
    let cursors = format!("{:?}", state.da_shard_cursors.read());
    let manifests = Arc::clone(&state.lane_manifests.read());
    let generation = state.state_view_generation();
    let mapping: *const LaneConfig = &geometry
        ._pending
        .as_ref()
        .unwrap()
        .catalog_update
        .updated_lane_config;
    let lease = state.kura.try_publication_lease().unwrap();
    geometry
        .prepare_under(&state.tiered_backend.lock(), &lease)
        .unwrap();
    geometry
        .resume_under(&mut state.tiered_backend.lock(), &lease)
        .unwrap();
    crate::kura::fail_bound_progress_intent_directory_sync_for_tests(0, 0);
    assert!(
        geometry
            .complete_under(&state, header(), &mut state.tiered_backend.lock(), &lease)
            .is_err()
    );
    assert_eq!(
        geometry.raw.as_ref().unwrap().phase(),
        crate::kura::RawGeometryPhase::PublishingCatalog
    );
    assert!(geometry.raw.as_ref().unwrap().has_pending_journal_write());
    assert!(geometry.tiered.as_ref().unwrap().is_applied());
    let journal_path = state.kura.lane_geometry_journal_path();
    let pending_bytes = std::fs::read(&journal_path).unwrap();
    let pending_inode = std::fs::metadata(&journal_path).unwrap().ino();
    drop(lease);

    let foreign = Kura::blank_kura_for_testing();
    let foreign_lease = foreign.try_publication_lease().unwrap();
    assert!(
        geometry
            .complete_under(
                &state,
                header(),
                &mut state.tiered_backend.lock(),
                &foreign_lease
            )
            .is_err()
    );
    assert!(geometry.raw.as_ref().unwrap().has_pending_journal_write());
    assert_eq!(std::fs::read(&journal_path).unwrap(), pending_bytes);
    drop(foreign_lease);

    let lease = state.kura.try_publication_lease().unwrap();
    {
        let completed = geometry
            .complete_under(&state, header(), &mut state.tiered_backend.lock(), &lease)
            .unwrap();
        assert!(std::ptr::eq(
            completed.updated_da_mapping().unwrap(),
            mapping
        ));
    }
    assert_eq!(
        geometry.raw.as_ref().unwrap().phase(),
        crate::kura::RawGeometryPhase::CatalogPublished
    );
    assert!(!geometry.raw.as_ref().unwrap().has_pending_journal_write());
    assert_eq!(
        std::fs::metadata(&journal_path).unwrap().ino(),
        pending_inode
    );
    assert_eq!(std::fs::read(&journal_path).unwrap(), pending_bytes);
    let completed_metadata = std::fs::metadata(&journal_path).unwrap();
    // Completion retries read the retained original descriptor and installed map;
    // they cannot replace the journal or reconstruct the mapping from live State.
    {
        let completed = geometry
            .complete_under(&state, header(), &mut state.tiered_backend.lock(), &lease)
            .unwrap();
        assert!(std::ptr::eq(
            completed.updated_da_mapping().unwrap(),
            mapping
        ));
    }
    let retry_metadata = std::fs::metadata(&journal_path).unwrap();
    assert_eq!(retry_metadata.ino(), completed_metadata.ino());
    assert_eq!(
        retry_metadata.modified().unwrap(),
        completed_metadata.modified().unwrap()
    );
    assert_eq!(std::fs::read(&journal_path).unwrap(), pending_bytes);
    assert_eq!(state.canonical_runtime.view().get(), &runtime);
    assert_eq!(norito::json::to_json(&state.world).unwrap(), world);
    assert_eq!(format!("{:?}", state.da_shard_cursors.read()), cursors);
    assert!(Arc::ptr_eq(&state.lane_manifests.read(), &manifests));
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(state.committed_height(), 0);
}

#[test]
#[cfg(unix)]
fn carrier_geometry_completed_catalog_refuses_identical_replacement_journal() {
    use std::os::unix::fs::MetadataExt;

    let (_temporary, state, mut geometry) = storage_fixture();
    let lease = state.kura.try_publication_lease().unwrap();
    geometry
        .prepare_under(&state.tiered_backend.lock(), &lease)
        .unwrap();
    geometry
        .complete_under(&state, header(), &mut state.tiered_backend.lock(), &lease)
        .unwrap();
    let journal_path = state.kura.lane_geometry_journal_path();
    let original_bytes = std::fs::read(&journal_path).unwrap();
    let original_inode = std::fs::metadata(&journal_path).unwrap().ino();
    let original_path = journal_path.with_extension("original-test-owner");
    std::fs::rename(&journal_path, &original_path).unwrap();
    std::fs::write(&journal_path, &original_bytes).unwrap();
    assert_ne!(
        std::fs::metadata(&journal_path).unwrap().ino(),
        original_inode
    );
    assert!(
        geometry
            .complete_under(&state, header(), &mut state.tiered_backend.lock(), &lease)
            .is_err()
    );
    assert_eq!(
        geometry.raw.as_ref().unwrap().phase(),
        crate::kura::RawGeometryPhase::CatalogPublished
    );
    assert_eq!(std::fs::read(&journal_path).unwrap(), original_bytes);
    assert_eq!(state.committed_height(), 0);
    // Even restoring its name must not refresh the original file's metadata
    // baseline: an external rename changes ctime and requires storage recovery.
    std::fs::remove_file(&journal_path).unwrap();
    std::fs::rename(&original_path, &journal_path).unwrap();
    assert_eq!(
        std::fs::metadata(&journal_path).unwrap().ino(),
        original_inode
    );
    assert!(
        geometry
            .complete_under(&state, header(), &mut state.tiered_backend.lock(), &lease)
            .is_err()
    );
}

#[test]
fn carrier_geometry_no_change_completion_has_no_mapping_or_storage_owner() {
    let state = state();
    let mut geometry = state
        .merge_preexecution_block(header())
        .prepare_carrier_geometry()
        .unwrap();
    let lease = state.kura.try_publication_lease().unwrap();
    assert!(
        geometry
            .complete_under(&state, header(), &mut state.tiered_backend.lock(), &lease)
            .unwrap()
            .updated_da_mapping()
            .is_none()
    );
    assert!(geometry.raw.is_none());
    assert!(geometry.tiered.is_none());
}
