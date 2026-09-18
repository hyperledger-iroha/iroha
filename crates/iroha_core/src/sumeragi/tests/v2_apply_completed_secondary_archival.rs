// A completed secondary coordinator archives only after real economic Apply and
// Queue terminal publication. Equal-input/equal-incarnation ABA is tested as a
// separate rejected transition in Kura; this positive uses fresh lineage.

/// Snapshot exact directory structure and bytes, refusing symlinks in the fixture.
fn completed_secondary_tree(
    root: &std::path::Path,
) -> BTreeMap<std::path::PathBuf, Option<Vec<u8>>> {
    fn visit(
        root: &std::path::Path,
        path: &std::path::Path,
        result: &mut BTreeMap<std::path::PathBuf, Option<Vec<u8>>>,
    ) {
        for item in std::fs::read_dir(path).expect("enumerate completed-secondary storage") {
            let path = item.expect("read storage entry").path();
            let metadata = std::fs::symlink_metadata(&path).expect("inspect direct storage entry");
            assert!(!metadata.file_type().is_symlink());
            let relative = path
                .strip_prefix(root)
                .expect("path below snapshot root")
                .to_path_buf();
            if metadata.is_dir() {
                result.insert(relative, None);
                visit(root, &path, result);
            } else {
                assert!(metadata.is_file());
                result.insert(
                    relative,
                    Some(std::fs::read(path).expect("read exact storage bytes")),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

/// Open a root whose lifetime belongs to the test, using the configured primary floor.
fn completed_secondary_kura_config(
    root: &std::path::Path,
) -> iroha_config::parameters::actual::Kura {
    use iroha_config::parameters::defaults::kura as defaults;
    iroha_config::parameters::actual::Kura {
        init_mode: iroha_config::kura::InitMode::Strict,
        store_dir: iroha_config::base::WithOrigin::inline(root.to_path_buf()),
        max_disk_usage_bytes: defaults::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: defaults::BLOCKS_IN_MEMORY,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: defaults::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: iroha_config::kura::FsyncMode::Batched,
        fsync_interval: defaults::FSYNC_INTERVAL,
        lane_history_retention: defaults::LANE_HISTORY_RETENTION,
        fastpq_artifacts: defaults::FASTPQ_ARTIFACT_POLICY,
        replica_advert: defaults::REPLICA_ADVERT_POLICY,
    }
}

/// Check valid incremental caches before exercising the normal lazy refresh path.
#[track_caller]
fn assert_completed_secondary_accounting(kura: &Kura) {
    let before = kura
        .disk_usage_accounting_snapshot_for_tests()
        .expect("scan exact storage accounting without refreshing caches");
    // Denied operations may conservatively invalidate their mutation guard's
    // counters. Never refresh a valid cache before checking it: that would hide
    // an incorrect incremental update in archival or startup reconciliation.
    if before.enforced_initialized {
        assert_eq!(before.cached_enforced_bytes, before.exact_enforced_bytes);
    }
    if before.total_initialized {
        assert_eq!(before.cached_total_bytes, before.exact_total_bytes);
    }
    if !before.enforced_initialized || !before.total_initialized {
        assert_eq!(
            kura.refresh_disk_usage_bytes()
                .expect("normal lazy accounting refresh"),
            before.exact_enforced_bytes,
        );
    }
    let after = kura
        .disk_usage_accounting_snapshot_for_tests()
        .expect("independently check refreshed accounting");
    assert!(after.enforced_initialized && after.total_initialized);
    assert_eq!(after.exact_enforced_bytes, before.exact_enforced_bytes);
    assert_eq!(after.exact_total_bytes, before.exact_total_bytes);
    assert_eq!(after.cached_enforced_bytes, before.exact_enforced_bytes);
    assert_eq!(after.cached_total_bytes, before.exact_total_bytes);
}

v2_apply_test!(
    completed_secondary_autonomous_application_retains_original_instance_and_reopens_without_replay,
    {
        use crate::state::WorldReadOnly as _;
        use iroha_data_model::isi::SetKeyValue;

        let root = tempfile::tempdir().expect("persistent secondary archival root");
        let config = completed_secondary_kura_config(root.path());
        let configured_catalog = iroha_data_model::nexus::LaneCatalog::default();
        let configured_lanes =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&configured_catalog);
        let (kura, _) =
            Kura::new_with_configured_lane_catalog(&config, &configured_lanes, &configured_catalog)
                .expect("authenticate configured primary storage");
        let fixture = ApplyFixture::new_for_completed_secondary_archival(kura);
        fixture
            .execute(&mut fixture.reopen_body_store())
            .expect("apply actual genesis");
        let admission_context = verified_successor_context_at_fixture_tip(&fixture);
        assert_eq!(admission_context.context().height, 2);
        let lane_id = LaneId::new(1);
        let dataspace_id = DataSpaceId::new(7);
        let old_incarnation = fixture.state.lane_incarnations_snapshot()[&lane_id];
        let old_activation = fixture.state.view().lane_incarnation_activation_heights[&lane_id];
        let old_lane = fixture
            .state
            .lane_storage_identity(lane_id)
            .expect("exact original secondary identity");
        let old_blocks = old_lane.blocks_dir(root.path());
        let blocks = old_blocks.clone();
        let domains = (0..2)
            .map(|index| {
                DomainId::try_new(format!("nativeparticipant{index}"), "independent-dataspace")
                    .expect("secondary domain")
            })
            .collect::<Vec<_>>();
        let metadata_key: iroha_model_base::name::Name = "completed-secondary-archival"
            .parse()
            .expect("metadata key");
        let read_values = || {
            let view = fixture.state.view();
            domains
                .iter()
                .map(|domain| {
                    view.world
                        .domain(domain)
                        .expect("preseeded secondary domain")
                        .metadata
                        .get(&metadata_key)
                        .cloned()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(read_values(), vec![None, None]);
        let expected_values = (0..2_u32)
            .map(|value| Some(iroha_primitives::json::Json::new(value + 11)))
            .collect::<Vec<_>>();
        let (events, _receiver) = tokio::sync::broadcast::channel(32);
        let queue = fixture_queue(fixture.state.as_ref(), events.clone());
        let journals = tempfile::tempdir().expect("secondary producer journals");
        let plans_path = journals.path().join("plans.norito");
        let reservations_path = journals.path().join("reservations.norito");
        queue
            .install_plan_journal(&plans_path, 1024 * 1024, true)
            .expect("open strict plan journal");
        queue
            .install_lane_reservation_journal(&reservations_path, 1024 * 1024)
            .expect("open reservation journal");
        let prepared = prepare_canonical_autonomous_batch_with_instructions(
            &fixture,
            &queue,
            admission_context.context(),
            2,
            |index| {
                vec![InstructionBox::from(SetKeyValue::domain(
                    domains[index].clone(),
                    metadata_key.clone(),
                    iroha_primitives::json::Json::new(
                        u32::try_from(index).expect("bounded index") + 11,
                    ),
                ))]
            },
            false,
            |_| {},
        );
        assert_ne!(prepared.expected_fifo[0], prepared.expected_fifo[1]);
        for plan in &prepared.planned_routing {
            assert_eq!(
                plan.coordinator_route(),
                crate::queue::RoutingDecision {
                    lane_id,
                    dataspace_id
                }
            );
            assert_eq!(
                plan,
                &crate::queue::RoutingPlan::single(crate::queue::RoutingDecision {
                    lane_id,
                    dataspace_id
                })
            );
        }
        let controls = cold_fixture_queue_plan_certificates(
            &fixture,
            &prepared.admission_bindings,
            &prepared.entrypoints,
        );
        let mut admission = build_apply_fixture_at_context_with_queue_plan_admissions(
            &fixture,
            admission_context.context().clone(),
            controls,
        );
        fixture
            .service
            .execute(&admission.context, &mut admission.store, &admission.task)
            .expect("globally admit both exact inputs");
        assert_eq!(fixture.state.committed_height(), 2);
        assert_eq!(read_values(), vec![None, None]);
        let source_context = verified_successor_context_at_fixture_tip(&fixture);
        let (payload, entrypoints) = reserve_prepared_canonical_autonomous_batch(
            &fixture,
            &queue,
            source_context.context(),
            prepared,
            None,
        );
        let descriptor = payload.origin_proposal.descriptor.clone();
        assert_eq!(
            (descriptor.lane_id, descriptor.dataspace_id),
            (lane_id, dataspace_id)
        );
        assert_eq!(descriptor.lane_incarnation, old_incarnation);
        assert_eq!(descriptor.proposal_height, 3);
        assert_eq!(descriptor.lane_block_height, 1);
        assert!(payload.native_amx_receipts.iter().all(Option::is_none));
        let reservation_keys = payload.reservation_keys.clone();
        let envelope = crate::lane_consensus::autonomous_lane_payload_envelope(
            &payload,
            payload.network_id,
            payload.epoch,
        )
        .expect("canonical autonomous source envelope");
        let mut source = build_apply_fixture_at_context_with_autonomous_payloads(
            &fixture,
            source_context.context().clone(),
            vec![envelope],
        );
        fixture
            .service
            .execute(&source.context, &mut source.store, &source.task)
            .expect("apply globally finalized secondary source");
        assert_eq!(fixture.state.committed_height(), 3);
        assert_eq!(read_values(), vec![None, None]);
        assert!(
            entrypoints
                .iter()
                .all(|hash| !fixture.state.has_committed_entrypoint(*hash))
        );
        let source_hash = source.body.hash();
        let source_finality = fixture
            .kura
            .v2_finality_artifact(3)
            .expect("read genuine source finality")
            .expect("source finality retained");
        let apply_context = verified_successor_context_at_fixture_tip(&fixture);
        assert_eq!(apply_context.context().height, 4);

        // Restart the real Queue producer before reconstructing from public finality.
        drop(queue);
        let queue = fixture_queue(fixture.state.as_ref(), events.clone());
        let replay = queue
            .install_lane_reservation_journal(&reservations_path, 1024 * 1024)
            .expect("restore original reservation owner");
        assert_eq!(replay.restored, 2);
        queue
            .install_plan_journal(&plans_path, 1024 * 1024, true)
            .expect("restore strict plan journal");
        queue
            .replay_plan_journal(fixture.state.as_ref())
            .expect("restore admitted executable inputs");
        let planning = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &apply_context,
            None,
        )
        .expect("plan genuine historical recovery");
        let LaneReservationReconciliationPlanning::InstallHistoricalAutonomousRecoveries(installs) =
            planning
        else {
            panic!("source must require its exact historical installation");
        };
        assert_eq!(installs.len(), 1);
        let install = installs.into_iter().next().expect("one exact source group");
        assert_eq!(install.canonical_body.block_hash, source_hash);
        assert_eq!(install.payload.entrypoints, payload.entrypoints);
        assert_eq!(install.payload.reservation_keys, reservation_keys);
        assert_eq!(
            install_historical_autonomous_lane_recovery(
                fixture.state.as_ref(),
                fixture.kura.as_ref(),
                &install
            )
            .expect("persist canonical historical source"),
            HistoricalAutonomousLaneRecoveryInstallOutcome::Installed
        );
        let records = fixture
            .kura
            .historical_autonomous_lane_recovery_records_bounded(1)
            .expect("read retained recovery owner");
        assert_eq!(records.len(), 1);
        let record = records[0].clone();
        let record_bytes = norito::codec::Encode::encode(&record);
        let record_relative = completed_secondary_tree(&blocks)
            .into_iter()
            .find_map(|(path, bytes)| {
                (bytes.as_deref() == Some(record_bytes.as_slice())).then_some(path)
            })
            .expect("exact original recovery frame on disk");
        let planning = plan_lane_reservation_ownership(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            &apply_context,
            None,
        )
        .expect("replan persisted source");
        let LaneReservationReconciliationPlanning::Ready(plan) = planning else {
            panic!("installed source must reconcile");
        };
        let summary = apply_lane_reservation_reconciliation_plan(
            fixture.state.as_ref(),
            queue.as_ref(),
            fixture.kura.as_ref(),
            plan,
        )
        .expect("publish recovered reservation ownership");
        assert_eq!(summary.recovered, 2);
        assert_eq!(summary.retained_historical_recovery, 2);

        let lane_keys = descriptor
            .validator_set
            .iter()
            .map(|peer| {
                fixture
                    .validator_keys
                    .iter()
                    .find(|key| key.public_key() == peer.public_key())
                    .expect("exact ordered lane key")
                    .clone()
            })
            .collect::<Vec<_>>();
        let producer = payload.producer.clone();
        let local_key = lane_keys
            .iter()
            .find(|key| key.public_key() == producer.public_key())
            .expect("actual producer key")
            .clone();
        let nonzero = NonZeroUsize::new(8).expect("lane-work bound");
        let limits = crate::sumeragi::v2_lane_work::V2LaneWorkLimits::new(
        nonzero, nonzero, nonzero, nonzero, nonzero, nonzero, nonzero,
        iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONSENSUS,
        iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC,
        iroha_config::parameters::defaults::sumeragi::V2_AUTHENTICATED_MERGE_QC_CAPACITY,
        iroha_config::parameters::defaults::sumeragi::V2_MERGE_LEADER_BODY_FRAME_HEADROOM_BYTES,
        iroha_config::parameters::defaults::sumeragi::V2_AUTONOMOUS_CARRIER_HEADROOM_BYTES,
        iroha_config::parameters::defaults::sumeragi::V2_AUTONOMOUS_PRODUCER_RECHECK,
        Duration::from_millis(10), Duration::from_secs(1),
        iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_STUCK_ATTEMPTS,
        iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_RETRY_TIER_ATTEMPTS,
        iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_MAX_RETRY_TIER,
        iroha_config::parameters::defaults::sumeragi::V2_SIDECAR_SERVICE_BURST,
        crate::merge_sidecar::MergeSidecarLimits::defaults(),
        crate::merge_sidecar::MergeSigningGuardLimits::defaults(),
        crate::native_amx::NativeAmxSigningGuardLimits::new(
            iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_CAPACITY,
            iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_BYTES,
            iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_ANCHOR_BYTES,
        ).expect("native signing bounds"),
    );
        let mut lane_work = crate::sumeragi::v2_lane_work::V2LaneWorkAdapter::new(
            apply_context.context().clone(),
            producer.clone(),
            local_key.clone(),
            true,
            Arc::clone(&fixture.state),
            Arc::clone(&fixture.kura),
            limits,
            None,
        )
        .expect("hydrate real secondary lane recovery");
        let generation = fixture
            .kura
            .claim_autonomous_lifecycle_process_generation(payload.network_id, &producer)
            .expect("claim real process generation");
        let _lifecycle_group = install_live_lifecycle_cursor_for_apply_test(
            fixture.kura.as_ref(),
            &generation,
            &install.payload,
            install.historical_context_id,
            &producer,
            &local_key,
        );
        let (certificate, _, _) =
            terminal_cycle_certificate(&fixture, &install.payload, &lane_keys);
        assert_eq!(
            lane_work.accept_lane_message(
                crate::sumeragi::InboundBlockMessage::from_authenticated_peer(
                    crate::sumeragi::message::BlockMessage::LaneBlockCertificate(Box::new(
                        certificate.clone()
                    )),
                    PeerId::new(lane_keys[0].public_key().clone())
                ),
                0
            ),
            crate::sumeragi::v2_lane_work::V2LaneIngressOutcome::Inserted
        );
        assert!(matches!(
            lane_work
                .service_next_historical_recovery()
                .expect("publish actual certified secondary bundle"),
            crate::sumeragi::v2_lane_work::HistoricalRecoveryServiceOutcome::Complete(_)
        ));
        let application_header = lane_work
            .merge_carrier_context_header(0)
            .expect("exact global application header");
        let candidate = fixture
            .state
            .build_merge_execution_candidate(
                application_header.clone(),
                apply_context.context().mode,
            )
            .expect("execute real secondary candidate against WSV");
        let batch = candidate
            .execution_batch
            .as_ref()
            .expect("actual secondary execution batch");
        assert_eq!(batch.lanes.len(), 1);
        assert_eq!(batch.lanes[0].proposal, certificate.proposal);
        assert!(batch.lanes[0].results.iter().all(|result| result.is_ok()));
        let global_keys = fixture_validator_keys();
        let entry = terminal_cycle_merge_entry(&candidate, apply_context.context(), &global_keys);
        fixture
            .state
            .validate_certified_merge_entry_for_global_order(&entry, apply_context.context().mode)
            .expect("revalidate exact WSV execution");
        fixture
            .kura
            .persist_pending_certified_merge_entry(&entry)
            .expect("retain actual merge sidecar");
        let service = V2ApplyService::new(
            Arc::clone(&fixture.state),
            Arc::clone(&queue),
            Arc::clone(&fixture.kura),
            None,
            None,
            fixture.service.block_cadence,
            fixture.service.genesis_account.clone(),
            events.clone(),
            fixture.service.validator_set_pops.clone(),
        );
        let mut merge = terminal_cycle_merge_apply_fixture(
            &fixture,
            &service,
            apply_context.context(),
            &entry,
            &application_header,
            &global_keys,
        );
        service
            .execute(&merge.context, &mut merge.store, &merge.task)
            .expect("apply actual secondary economic carrier");
        assert_eq!(fixture.state.committed_height(), 4);
        assert_eq!(read_values(), expected_values);
        assert!(
            entrypoints
                .iter()
                .all(|hash| fixture.state.has_committed_entrypoint(*hash))
        );
        assert!(queue.live_lane_reservations().is_empty());
        assert!(queue.lane_reservation_commit_barriers().is_empty());
        assert!(queue.lane_reservation_release_barriers().is_empty());
        assert!(queue.lane_reservation_group_is_finalized_for_diagnostics(&reservation_keys));
        for key in &reservation_keys {
            assert!(!queue.has_durable_plan_claim_for_test(key.entrypoint_hash));
        }
        let receipt = fixture
            .kura
            .read_lane_block_application_receipt(lane_id, 1)
            .expect("genuine secondary economic application receipt");
        assert_eq!(receipt.proposal, certificate.proposal);
        assert_eq!(
            receipt.format,
            crate::kura::LaneBlockApplicationReceiptArtifactFormat::MergeExecution
        );
        assert_eq!(receipt.application_block_height, 4);
        assert_eq!(receipt.application_block_hash, merge.body.hash());
        assert!(
            fixture
                .state
                .certified_autonomous_lane_block_is_globally_applied(&certificate.proposal)
                .expect("authenticate economic completion")
        );
        assert!(
            fixture
                .kura
                .pending_autonomous_lifecycle_terminal_outcome_inventory()
                .expect("read pending terminal inventory")
                .is_empty()
        );
        let terminal_path = fixture
            .kura
            .autonomous_lifecycle_terminal_outcome_path_for_test(lane_id, 1, 3)
            .expect("real Complete owner path");
        let terminal_bytes =
            std::fs::read(&terminal_path).expect("actual Apply published durable Complete");
        assert!(!terminal_bytes.is_empty());
        service
            .execute(&merge.context, &mut merge.store, &merge.task)
            .expect("repeat same Apply idempotently");
        assert_eq!(read_values(), expected_values);
        assert_eq!(fixture.state.merge_ledger.snapshot(), vec![Arc::new(entry)]);
        assert_eq!(
            std::fs::read(&terminal_path).expect("same terminal proof"),
            terminal_bytes
        );

        // A cold Queue replay observes terminal ownership; it cannot resurrect inputs.
        drop(service);
        drop(queue);
        let cold_queue = fixture_queue(fixture.state.as_ref(), events);
        cold_queue
            .install_lane_reservation_journal(&reservations_path, 1024 * 1024)
            .expect("cold terminal reservations");
        cold_queue
            .install_plan_journal(&plans_path, 1024 * 1024, true)
            .expect("cold terminal plan owner");
        cold_queue
            .replay_plan_journal(fixture.state.as_ref())
            .expect("cold replay authenticates completed State");
        assert!(cold_queue.live_lane_reservations().is_empty());
        assert!(cold_queue.lane_reservation_commit_barriers().is_empty());
        assert!(cold_queue.lane_reservation_release_barriers().is_empty());
        assert!(cold_queue.lane_reservation_group_is_finalized_for_diagnostics(&reservation_keys));
        for key in &reservation_keys {
            assert!(!cold_queue.has_durable_plan_claim_for_test(key.entrypoint_hash));
        }
        drop(cold_queue);
        drop(lane_work);
        drop(generation);

        let next_lane = fixture
            .state
            .nexus_snapshot()
            .lane_catalog
            .lanes()
            .iter()
            .find(|lane| lane.id == lane_id)
            .expect("actual secondary catalog entry")
            .clone();
        let replacement = iroha_data_model::nexus::LaneLifecyclePlan {
            additions: vec![next_lane],
            retire: vec![lane_id],
        };
        // This test-only lifecycle entry point supplies geometry authorization, but
        // its Kura prearchive gate must independently authenticate economic custody.
        // Removing the real Complete file cannot be replaced by the receipt alone.
        std::fs::remove_file(&terminal_path).expect("temporarily remove exact Complete owner");
        let without_terminal = completed_secondary_tree(root.path());
        let old_state_hash = crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref())
            .expect("stable valid fixture snapshot");
        assert!(fixture.state.apply_lane_lifecycle(&replacement).is_err());
        assert_eq!(completed_secondary_tree(root.path()), without_terminal);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref())
                .expect("stable valid fixture snapshot"),
            old_state_hash
        );
        std::fs::write(&terminal_path, &terminal_bytes).expect("restore exact real Complete bytes");
        let old_segment = completed_secondary_tree(&blocks);
        let old_merge_bytes = std::fs::read(old_lane.merge_log_path(root.path()))
            .expect("retain every old merge-journal byte");
        assert_eq!(
            old_segment.get(&record_relative),
            Some(&Some(record_bytes.clone()))
        );
        fixture
            .state
            .apply_lane_lifecycle(&replacement)
            .expect("archive economically completed secondary incarnation");
        let new_incarnation = fixture.state.lane_incarnations_snapshot()[&lane_id];
        assert_ne!(
            new_incarnation, old_incarnation,
            "valid recreation advances lineage rather than reusing an ABA hash"
        );
        // Capture the same exact geometry tuple used by State snapshot restore;
        // a runtime catalog alone must not authorize a changed incarnation.
        let (
            runtime_lanes,
            runtime_incarnations,
            runtime_activations,
            lineage_root,
            restore_height,
        ) = {
            let view = fixture.state.view();
            (
                view.nexus.lane_config.clone(),
                view.lane_incarnations.clone(),
                view.lane_incarnation_activation_heights.clone(),
                crate::state::lane_incarnation_lineage_root(
                    &payload.network_id,
                    &view.lane_incarnation_lineage,
                ),
                u64::try_from(view.block_hashes.len()).expect("captured height fits u64"),
            )
        };
        let new_lane = fixture
            .state
            .lane_storage_identity(lane_id)
            .expect("exact replacement identity");
        let blocks = new_lane.blocks_dir(root.path());
        assert_ne!(
            blocks, old_blocks,
            "new incarnation has a distinct immutable address"
        );
        for name in ["blocks.index", "blocks.data", "blocks.hashes"] {
            assert!(blocks.join(name).is_file());
        }
        let artifacts = blocks.join("lane_artifacts");
        let metadata = std::fs::symlink_metadata(&artifacts).expect("new artifact namespace");
        assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
        assert_eq!(
            std::fs::read_dir(&artifacts)
                .expect("new artifact directory")
                .count(),
            0
        );
        assert_eq!(
            std::fs::metadata(new_lane.merge_log_path(root.path()))
                .expect("new empty merge log")
                .len(),
            0
        );
        assert!(
            fixture
                .kura
                .historical_autonomous_lane_recovery_records_bounded(1)
                .expect("fresh namespace inventory")
                .is_empty()
        );
        let archive = std::fs::canonicalize(&old_blocks).expect("retained original instance");
        let archived_merge = std::fs::canonicalize(old_lane.merge_log_path(root.path()))
            .expect("retained original merge object");
        assert_eq!(std::fs::read(&archived_merge).unwrap(), old_merge_bytes);
        fixture
            .kura
            .validate_retained_lane_pair_for_test(old_lane, &archive, &archived_merge)
            .expect("completed journal reference and exact original pair");
        assert!(
            fixture
                .kura
                .validate_retained_lane_pair_for_test(
                    crate::kura::LaneStorageIdentity {
                        incarnation: new_incarnation,
                        ..old_lane
                    },
                    &archive,
                    &archived_merge
                )
                .is_err(),
            "new incarnation cannot authenticate the old object"
        );
        assert!(
            fixture
                .kura
                .validate_retained_lane_pair_for_test(
                    crate::kura::LaneStorageIdentity {
                        activation_height: old_activation.checked_add(1).unwrap(),
                        ..old_lane
                    },
                    &archive,
                    &archived_merge
                )
                .is_err(),
            "activation height is part of the exact retained identity"
        );
        assert!(
            fixture
                .kura
                .validate_retained_lane_pair_for_test(
                    old_lane,
                    &archive,
                    &std::fs::canonicalize(new_lane.merge_log_path(root.path())).unwrap()
                )
                .is_err(),
            "new instance merge log cannot substitute for the original pair"
        );
        let archived_segment = completed_secondary_tree(&archive);
        assert_eq!(
            archived_segment, old_segment,
            "reference publication changes no byte of the retained original instance"
        );
        assert!(
            !root.path().join("retired/lane_geometry").exists(),
            "retirement does not relocate physical objects before authenticated GC"
        );
        let fresh_tree = completed_secondary_tree(&blocks);
        assert!(
            fixture
                .kura
                .persist_historical_autonomous_lane_recovery_record(&record)
                .is_err()
        );
        let session = crate::lane_consensus::CommittedLaneBlockSession {
            proposal: certificate.proposal.clone(),
            prepare_qc: certificate.prepare_qc.clone(),
            commit_qc: certificate.commit_qc.clone(),
        };
        let pops = lane_keys
            .iter()
            .take(3)
            .map(|key| {
                (
                    key.public_key().clone(),
                    iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("real lane PoP"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        Kura::validate_certified_lane_block_artifact(
            &crate::kura::CertifiedLaneBlockArtifact::new(session.clone(), pops.clone()),
        )
        .expect("stale-QC control retains exact valid cryptographic evidence");
        assert!(
            fixture
                .kura
                .persist_committed_lane_block_session(&session, &pops)
                .is_err()
        );
        assert!(matches!(
            fixture.kura.classify_autonomous_lane_reservation_groups(
                std::slice::from_ref(&record.reservation_group),
                payload.network_id,
                &[payload.epoch],
            ),
            Err(crate::kura::AutonomousLaneReservationEvidenceError::Kura(_))
        ));
        assert_eq!(completed_secondary_tree(&blocks), fresh_tree);
        assert_eq!(completed_secondary_tree(&archive), archived_segment);
        assert_eq!(std::fs::read(&archived_merge).unwrap(), old_merge_bytes);
        assert_eq!(read_values(), expected_values);
        assert!(
            entrypoints
                .iter()
                .all(|hash| fixture.state.has_committed_entrypoint(*hash))
        );
        assert_completed_secondary_accounting(fixture.kura.as_ref());

        // Body stores and Apply fixtures must not retain any old process owner.
        // The lifecycle group is a Copy identity, not a lease; its process claim
        // and lane-work owner were explicitly dropped above.
        drop(admission);
        drop(source);
        drop(merge);
        let old_kura = Arc::downgrade(&fixture.kura);
        drop(fixture);
        assert!(
            old_kura.upgrade().is_none(),
            "all old Kura owners are gone before cold reopen"
        );
        let (reopened, count) =
            Kura::new_with_configured_lane_catalog(&config, &configured_lanes, &configured_catalog)
                .expect("cold open actual canonical storage");
        assert_eq!(count.0, 4);
        reopened
            .bind_lane_storage_network(payload.network_id)
            .expect("bind the original authenticated chain network");
        reopened
            .restore_lane_segments_with_geometry_at_height_and_lineage_root(
                &runtime_lanes,
                &runtime_incarnations,
                &runtime_activations,
                restore_height,
                lineage_root,
            )
            .expect(
                "authenticate exact captured geometry and retained journal after strict startup",
            );
        reopened
            .validate_retained_lane_pair_for_test(old_lane, &archive, &archived_merge)
            .expect("cold reopen reauthenticates the retained journal and exact original pair");
        assert!(
            reopened
                .historical_autonomous_lane_recovery_records_bounded(1)
                .expect("cold fresh namespace inventory")
                .is_empty()
        );
        assert_eq!(
            reopened
                .get_block(NonZeroUsize::new(3).expect("source height"))
                .expect("retain actual source")
                .hash(),
            source_hash
        );
        assert_eq!(
            reopened
                .v2_finality_artifact(3)
                .expect("read retained actual source proof"),
            Some(source_finality)
        );
        assert!(
            reopened
                .persist_historical_autonomous_lane_recovery_record(&record)
                .is_err()
        );
        assert!(
            reopened
                .persist_committed_lane_block_session(&session, &pops)
                .is_err()
        );
        assert!(matches!(
            reopened.classify_autonomous_lane_reservation_groups(
                std::slice::from_ref(&record.reservation_group),
                payload.network_id,
                &[payload.epoch],
            ),
            Err(crate::kura::AutonomousLaneReservationEvidenceError::Kura(_))
        ));
        assert_eq!(completed_secondary_tree(&blocks), fresh_tree);
        assert_eq!(completed_secondary_tree(&archive), archived_segment);
        assert_eq!(std::fs::read(&archived_merge).unwrap(), old_merge_bytes);
        // A copied old seal in the new namespace must be rejected read-only, even
        // though its old public source and economic finality are still authentic.
        let injected = blocks.join(&record_relative);
        let injected_parent = injected.parent().expect("recovery directory");
        std::fs::create_dir(injected_parent).expect("new recovery namespace for adverse control");
        std::fs::write(&injected, &record_bytes).expect("inject old seal unchanged");
        let injected_tree = completed_secondary_tree(&blocks);
        assert!(
            reopened
                .historical_autonomous_lane_recovery_records_bounded(1)
                .is_err()
        );
        assert_eq!(completed_secondary_tree(&blocks), injected_tree);
        std::fs::remove_file(&injected).expect("remove exactly the injected old seal");
        std::fs::remove_dir(injected_parent).expect("remove empty injected directory");
        assert_eq!(completed_secondary_tree(&blocks), fresh_tree);
        assert_eq!(completed_secondary_tree(&archive), archived_segment);
        assert_eq!(std::fs::read(&archived_merge).unwrap(), old_merge_bytes);
        assert_completed_secondary_accounting(reopened.as_ref());
    }
);
