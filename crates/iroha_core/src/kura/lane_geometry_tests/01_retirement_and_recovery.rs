#[test]
fn native_amx_retirement_targets_exact_participant_incarnation_and_fails_closed() {
    let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let coordinator_incarnation = Hash::new(b"retirement-classifier-coordinator");
    let participant_lane_id = LaneId::new(7);
    let participant_dataspace_id = DataSpaceId::new(9);
    let participant_incarnation = Hash::new(b"retirement-classifier-participant-a");
    let recreated_incarnation = Hash::new(b"retirement-classifier-participant-b");
    let (_, _, payload) = autonomous_retirement_payload(
        coordinator_incarnation,
        participant_lane_id,
        participant_dataspace_id,
        participant_incarnation,
        &producer,
    );
    let receipt = payload.native_amx_receipts[0]
        .as_ref()
        .expect("native AMX retirement receipt");
    let exact = BTreeSet::from([LaneRetirementIdentity {
        lane_id: participant_lane_id,
        dataspace_id: participant_dataspace_id,
        lane_incarnation: participant_incarnation,
    }]);
    let recreated = BTreeSet::from([LaneRetirementIdentity {
        lane_id: participant_lane_id,
        dataspace_id: participant_dataspace_id,
        lane_incarnation: recreated_incarnation,
    }]);
    assert_eq!(
        native_amx_receipt_targets_retirement(receipt, &exact),
        Ok(true)
    );
    assert_eq!(
        native_amx_receipt_targets_retirement(receipt, &recreated),
        Ok(false),
        "incarnation-A evidence must not ABA-block incarnation B"
    );
    assert!(lane_payload_targets_retirement(&payload, &exact));
    assert!(!lane_payload_targets_retirement(&payload, &recreated));
    let mut malformed = payload.clone();
    malformed.native_amx_receipts[0]
        .as_mut()
        .expect("native AMX retirement receipt")
        .legs[0]
        .prepare_qc
        .body
        .participant_lane_incarnation = Hash::new(b"malformed retirement participant");
    assert!(
        lane_payload_targets_retirement(&malformed, &recreated),
        "internally inconsistent participant evidence must fail closed"
    );
    let mut misaligned = payload;
    misaligned.native_amx_receipts.clear();
    assert!(
        lane_payload_targets_retirement(&misaligned, &recreated),
        "routing/receipt vector misalignment must fail closed"
    );
}
#[test]
fn mixed_role_native_amx_retirement_ignores_coordinator_and_targets_remote_routes() {
    let block = crate::sumeragi::exec::result_bearing_native_manifest_block_for_tests();
    let receipt = block
        .execution_context()
        .and_then(|bundle| bundle.external.first())
        .and_then(|context| context.native_amx_receipt.as_ref())
        .expect("mixed-role Native AMX receipt");
    let coordinator = LaneRetirementIdentity {
        lane_id: receipt.lane_id,
        dataspace_id: receipt.dataspace_id,
        lane_incarnation: receipt.lane_incarnation,
    };
    assert_eq!(
        native_amx_receipt_targets_retirement(receipt, &BTreeSet::from([coordinator])),
        Ok(false),
        "participant-form coordinator evidence must not block retirement"
    );
    let separate_participants = receipt
        .legs
        .iter()
        .filter_map(|leg| {
            match crate::native_amx::native_amx_participant_application_role(receipt, leg)
                .expect("mixed-role retirement fixture leg identity")
            {
                crate::native_amx::NativeAmxParticipantApplicationRole::Coordinator => None,
                crate::native_amx::NativeAmxParticipantApplicationRole::SeparateParticipant => {
                    let descriptor = &leg.participant_proposal.descriptor;
                    Some(LaneRetirementIdentity {
                        lane_id: descriptor.lane_id,
                        dataspace_id: descriptor.dataspace_id,
                        lane_incarnation: descriptor.lane_incarnation,
                    })
                }
            }
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        separate_participants.len(),
        2,
        "fixture must exercise two independent participant retirements"
    );
    for participant in separate_participants.iter().copied() {
        assert_eq!(
            native_amx_receipt_targets_retirement(receipt, &BTreeSet::from([participant]),),
            Ok(true),
            "each exact remote participant must block retirement"
        );
    }
    assert_eq!(
        native_amx_receipt_targets_retirement(receipt, &separate_participants),
        Ok(true)
    );
}
#[test]
fn unjournaled_nonzero_activation_without_marker_fails_closed_before_intent() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, extended_activations) = retirement_test_geometry();
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let kura = open_kura(&root, &extended);
    let error = kura
        .apply_lane_geometry_transition(
            &extended,
            &initial,
            &extended_incarnations,
            &initial_incarnations,
            &extended_activations,
            &initial_activations,
            &BTreeSet::new(),
        )
        .expect_err("unjournaled dynamic storage must not be adopted without its marker");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "active dynamic lane storage has no incarnation marker",
    );
    assert!(
        kura.read_lane_geometry_journal()
            .expect("default geometry journal")
            .records
            .is_empty(),
        "missing-marker rejection must precede retirement intent publication"
    );
    let participant_blocks = extended
        .entry(LaneId::new(1))
        .expect("dynamic participant lane")
        .blocks_dir(&root);
    assert!(participant_blocks.is_dir());
    assert!(
        !participant_blocks.join(MARKER_FILE_NAME).exists(),
        "rejection must not synthesize authority for the unjournaled dynamic lane"
    );
}
#[test]
fn scale_in_conservatively_rejects_pending_native_amx_participant_route() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, extended_activations) = retirement_test_geometry();
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let (kura, journal_before, _) = open_published_retirement_kura(
        &root,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) = autonomous_retirement_payload(
        extended_incarnations[&LaneId::SINGLE],
        LaneId::new(1),
        DataSpaceId::new(8),
        extended_incarnations[&LaneId::new(1)],
        &producer,
    );
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist coordinator-owned participant work");
    let error = kura
        .apply_lane_geometry_transition(
            &extended,
            &initial,
            &extended_incarnations,
            &initial_incarnations,
            &extended_activations,
            &initial_activations,
            &BTreeSet::new(),
        )
        .expect_err("pending participant route must conservatively pin retirement");
    assert_geometry_io_error(
        &error,
        ErrorKind::WouldBlock,
        "pending autonomous payload targets a retiring lane incarnation",
    );
    assert!(
        extended
            .entry(LaneId::new(1))
            .expect("participant lane")
            .blocks_dir(&root)
            .exists(),
        "retirement admission fails before moving lane files"
    );
    assert_eq!(
        fs::read(kura.lane_geometry_journal_path()).expect("unchanged geometry journal"),
        journal_before,
        "rejected retirement must not alter the published geometry journal"
    );
    let certified_retirement = BTreeSet::from([(
        LaneId::new(1),
        DataSpaceId::new(8),
        extended_incarnations[&LaneId::new(1)],
    )]);
    let error = kura
        .apply_lane_geometry_transition_with_certified_retirements(
            &extended,
            &initial,
            &extended_incarnations,
            &initial_incarnations,
            &extended_activations,
            &initial_activations,
            &BTreeSet::new(),
            &certified_retirement,
        )
        .expect_err("a drain certificate must not discard participant work coordinated elsewhere");
    assert!(
        format!("{error:?}")
            .contains("pending autonomous payload targets a retiring lane incarnation"),
        "unexpected certified-retirement participant error: {error:?}"
    );
}
#[test]
fn certified_drain_frontier_admission_rejects_stale_route_and_missing_native_evidence() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("drain-frontier-admission");
    let (_, extended) = retirement_test_configs();
    let (extended_incarnations, _) = retirement_test_geometry();
    let kura = open_kura(&root, &extended);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(8);
    let lane_incarnation = extended_incarnations[&lane_id];
    let mut frontier = LaneDrainFrontierV1::ordinary(
        lane_id,
        dataspace_id,
        lane_incarnation,
        1,
        Some(Hash::new(b"retirement drain frontier")),
    );
    let mut stale = frontier;
    stale.lane_incarnation = Hash::new(b"stale retirement incarnation");
    let error = kura
        .validate_certified_lane_drain_frontier(lane_id, dataspace_id, lane_incarnation, &stale)
        .expect_err("stale drain frontier route must fail Kura admission");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidInput,
        "certified lane drain frontier is malformed or targets another incarnation",
    );
    frontier.native_application =
        Some(iroha_data_model::merge::LaneDrainNativeFrontierEvidenceV1 {
            version: 1,
            participant_view: 0,
            predecessor_height: 0,
            predecessor_descriptor_hash: None,
            participant_proposal_hash: Hash::new(b"retirement Native proposal"),
            participant_settlement_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"retirement Native settlement",
            )),
            source_count: 1,
            application_block_height: 7,
            application_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"retirement Native application block",
            )),
            executed_block_wire_hash: Hash::new(b"retirement Native executed wire"),
            finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"retirement Native finality",
            )),
            application_manifest_root: Hash::new(b"retirement Native manifest root"),
            application_manifest_leaf_count: 1,
            application_manifest_leaf_index: 0,
            manifest_artifact_hash: Hash::new(b"retirement Native manifest"),
            receipt_artifact_hash: Hash::new(b"retirement Native receipt"),
            latest_index_artifact_hash: Hash::new(b"retirement Native latest index"),
        });
    let error = kura
        .validate_certified_lane_drain_frontier(lane_id, dataspace_id, lane_incarnation, &frontier)
        .expect_err("missing Native drain sidecars must fail Kura admission");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "certified Native-derived drain frontier lacks its exact durable receipt",
    );
}
#[test]
fn scale_in_allows_unrelated_participant_work() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("unrelated-lane");
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, extended_activations) = retirement_test_geometry();
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let (kura, _, _) = open_published_retirement_kura(
        &root,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) = autonomous_retirement_payload(
        extended_incarnations[&LaneId::SINGLE],
        LaneId::new(9),
        DataSpaceId::new(19),
        Hash::new(b"unrelated participant incarnation"),
        &producer,
    );
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist non-target participant work");
    kura.apply_lane_geometry_transition(
        &extended,
        &initial,
        &extended_incarnations,
        &initial_incarnations,
        &extended_activations,
        &initial_activations,
        &BTreeSet::new(),
    )
    .unwrap_or_else(|error| panic!("unrelated lane should not pin old retirement: {error}"));
}
#[test]
fn certified_scale_in_archives_pending_work_owned_by_exact_retiring_incarnation() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, extended_activations) = retirement_test_geometry();
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let kura = open_kura(&root, &extended);
    install_retirement_test_lane_markers(
        &kura,
        &extended,
        &extended_incarnations,
        &extended_activations,
    );
    let retiring_lane = LaneId::new(1);
    let retiring_dataspace = DataSpaceId::new(8);
    let retiring_incarnation = extended_incarnations[&retiring_lane];
    let artifact =
        certified_geometry_lane_block(retiring_lane, retiring_dataspace, retiring_incarnation, 1);
    kura.write_certified_lane_block_artifact(&artifact)
        .expect("persist late local certified work");
    let error = kura
        .apply_lane_geometry_transition(
            &extended,
            &initial,
            &extended_incarnations,
            &initial_incarnations,
            &extended_activations,
            &initial_activations,
            &BTreeSet::new(),
        )
        .expect_err("ordinary retirement must remain blocked by pending certified work");
    assert!(
        format!("{error:?}")
            .contains("pending certified work belongs to a retiring lane incarnation"),
        "unexpected ordinary-retirement error: {error:?}"
    );
    kura.apply_lane_geometry_transition_with_certified_retirements(
        &extended,
        &initial,
        &extended_incarnations,
        &initial_incarnations,
        &extended_activations,
        &initial_activations,
        &BTreeSet::new(),
        &BTreeSet::from([(retiring_lane, retiring_dataspace, retiring_incarnation)]),
    )
    .expect("globally certified exact-incarnation retirement archives local stragglers");
    assert!(
        !extended
            .entry(retiring_lane)
            .expect("retiring lane entry")
            .blocks_dir(&root)
            .exists(),
        "certified retirement must move the old lane directory into the journal archive"
    );
}
#[test]
fn capacity_blocked_retirement_archives_recovered_lane_history_intact() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("capacity-blocked-recovered-history");
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, extended_activations) = retirement_test_geometry();
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let (mut kura, _, _) = open_published_retirement_kura(
        &root,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let retiring_lane = LaneId::new(1);
    let retiring_entry = extended.entry(retiring_lane).expect("retiring lane entry");
    let retiring_incarnation = extended_incarnations[&retiring_lane];
    let work = install_merge_applied_retirement_work(&kura, retiring_incarnation);
    let execution = work
        .entry
        .execution_batch
        .as_ref()
        .and_then(|batch| batch.lanes.first())
        .expect("merge-applied retirement lane execution");
    let artifact = Kura::merge_lane_block_artifact(execution);
    let (data_path, index_path) = Kura::lane_artifact_paths_for_entry(retiring_entry, &root);
    let payload = artifact
        .encode_framed()
        .expect("encode retiring lane artifact history");
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            execution.proposal.descriptor.lane_block_height,
            &payload,
            "lane block artifact",
            FsyncMode::Always,
            None,
            crate::kura::SidecarIndexOrigin::FirstWrite,
        ),
        "persist retiring lane artifact history",
    );
    let stable_data = fs::read(&data_path).expect("read stable retiring lane artifact data");
    let stable_index = fs::read(&index_path).expect("read stable retiring lane artifact index");
    let temp_data_path = data_path.with_extension("norito.tmp");
    let temp_index_path = index_path.with_extension("index.tmp");
    fs::write(&temp_data_path, &stable_data).expect("stage retiring lane crash-temp data");
    fs::write(&temp_index_path, &stable_index).expect("stage retiring lane crash-temp index");
    kura.refresh_disk_usage_bytes()
        .expect("account retiring lane crash temps");
    Arc::get_mut(&mut kura)
        .expect("exclusive Kura before capacity-blocked retirement")
        .max_disk_usage_bytes = 1;
    kura.apply_lane_geometry_transition(
        &extended,
        &initial,
        &extended_incarnations,
        &initial_incarnations,
        &extended_activations,
        &initial_activations,
        &BTreeSet::new(),
    )
    .expect("capacity-blocked compaction must not strand retirement");
    let journal = kura
        .read_lane_geometry_journal()
        .expect("read capacity-blocked retirement journal");
    let retirement = journal.records.last().expect("retirement transition");
    let operation = retirement
        .operations
        .iter()
        .find(|operation| operation.lane_id == retiring_lane)
        .expect("retiring lane operation");
    let archived_blocks = kura
        .resolve_relative_path(&operation.archived_blocks_path)
        .expect("resolve authenticated retiring lane archive");
    let archived_artifacts = archived_blocks.join(LANE_ARTIFACTS_DIR_NAME);
    let archived_data = archived_artifacts.join(LANE_ARTIFACTS_DATA_FILE);
    let archived_index = archived_artifacts.join(LANE_ARTIFACTS_INDEX_FILE);
    assert_eq!(
        fs::read(&archived_data).expect("read archived lane artifact data"),
        stable_data,
    );
    assert_eq!(
        fs::read(&archived_index).expect("read archived lane artifact index"),
        stable_index,
    );
    assert!(
        !archived_data.with_extension("norito.tmp").exists()
            && !archived_index.with_extension("index.tmp").exists(),
        "retirement must archive only the recovered stable pair",
    );
}
#[test]
fn certified_scale_in_rejects_wrong_retirement_identity() {
    for (label, dataspace, incarnation) in [
        (
            "wrong-dataspace",
            DataSpaceId::new(9),
            Hash::prehashed([0x62; Hash::LENGTH]),
        ),
        (
            "wrong-incarnation",
            DataSpaceId::new(8),
            Hash::prehashed([0x63; Hash::LENGTH]),
        ),
    ] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(label);
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let kura = open_kura(&root, &extended);
        install_retirement_test_lane_markers(
            &kura,
            &extended,
            &extended_incarnations,
            &extended_activations,
        );
        let error = kura
            .apply_lane_geometry_transition_with_certified_retirements(
                &extended,
                &initial,
                &extended_incarnations,
                &initial_incarnations,
                &extended_activations,
                &initial_activations,
                &BTreeSet::new(),
                &BTreeSet::from([(LaneId::new(1), dataspace, incarnation)]),
            )
            .expect_err("mismatched certified identity must not bypass retirement admission");
        assert!(
            format!("{error:?}").contains(
                "certified retirement identity does not exactly match the retiring geometry"
            ),
            "unexpected {label} identity error: {error:?}"
        );
        assert!(
            kura.read_lane_geometry_journal()
                .expect("geometry journal")
                .records
                .is_empty(),
            "{label} must fail before publishing a retirement intent"
        );
    }
}
#[test]
fn scale_in_allows_recreated_incarnation_and_unrelated_participant_work() {
    for (label, participant_lane, participant_dataspace, participant_incarnation) in [
        (
            "recreated-incarnation",
            LaneId::new(1),
            DataSpaceId::new(8),
            Hash::prehashed([0x63; Hash::LENGTH]),
        ),
        (
            "unrelated-lane",
            LaneId::new(9),
            DataSpaceId::new(19),
            Hash::prehashed([0x69; Hash::LENGTH]),
        ),
    ] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(label);
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let kura = open_kura(&root, &extended);
        install_retirement_test_lane_markers(
            &kura,
            &extended,
            &extended_incarnations,
            &extended_activations,
        );
        let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (network_id, epoch, payload) = autonomous_retirement_payload(
            extended_incarnations[&LaneId::SINGLE],
            participant_lane,
            participant_dataspace,
            participant_incarnation,
            &producer,
        );
        kura.persist_lane_executable_payload(&payload, network_id, epoch)
            .expect("persist non-target participant work");
        kura.apply_lane_geometry_transition(
            &extended,
            &initial,
            &extended_incarnations,
            &initial_incarnations,
            &extended_activations,
            &initial_activations,
            &BTreeSet::new(),
        )
        .unwrap_or_else(|error| panic!("{label} should not pin old retirement: {error}"));
    }
}
#[test]
fn scale_in_rejects_unknown_and_malformed_artifact_files_before_intent() {
    for (label, file_name, bytes, expected_kind, expected_message) in [
        (
            "unknown",
            "operator-junk.bin",
            b"junk".as_slice(),
            ErrorKind::InvalidData,
            "lane retirement scan encountered an unknown artifact filename",
        ),
        (
            "stale-temp",
            "autonomous_blocks.norito.tmp",
            b"partial".as_slice(),
            ErrorKind::WouldBlock,
            "lane retirement scan found an in-flight autonomous sidecar",
        ),
        (
            "malformed-view",
            "autonomous_view_1.norito",
            b"not-a-view-state".as_slice(),
            ErrorKind::InvalidData,
            "lane retirement scan encountered a non-canonical view-state filename",
        ),
        (
            "orphan-view",
            "autonomous_view_00000000000000000001.norito",
            b"not-a-view-state".as_slice(),
            ErrorKind::InvalidData,
            "lane retirement scan found an orphan autonomous view state",
        ),
    ] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(label);
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let (kura, journal_before, _) = open_published_retirement_kura(
            &root,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let artifact_dir = Kura::lane_artifact_dir(
            &extended
                .entry(LaneId::SINGLE)
                .expect("coordinator lane")
                .blocks_dir(&root),
        );
        fs::create_dir_all(&artifact_dir).expect("artifact directory");
        fs::write(artifact_dir.join(file_name), bytes).expect("hostile artifact");
        let error = kura
            .apply_lane_geometry_transition(
                &extended,
                &initial,
                &extended_incarnations,
                &initial_incarnations,
                &extended_activations,
                &initial_activations,
                &BTreeSet::new(),
            )
            .expect_err("hostile retirement artifact must fail before intent publication");
        assert_geometry_io_error(&error, expected_kind, expected_message);
        assert_eq!(
            fs::read(kura.lane_geometry_journal_path()).expect("unchanged geometry journal"),
            journal_before,
            "{label} artifact must fail before an intent is published"
        );
    }
}
#[test]
fn first_release_retirement_policy_rejects_every_stale_autonomous_sidecar_class() {
    for (label, file_name, expected_kind, expected_message) in [
        (
            "data-only",
            OBSOLETE_AUTONOMOUS_LANE_BLOCKS_DATA_FILE,
            ErrorKind::InvalidData,
            "lane retirement scan encountered an unknown artifact filename",
        ),
        (
            "index-only",
            OBSOLETE_AUTONOMOUS_LANE_BLOCKS_INDEX_FILE,
            ErrorKind::InvalidData,
            "lane retirement scan encountered an unknown artifact filename",
        ),
        (
            "view-state",
            "autonomous_view_00000000000000000001.norito",
            ErrorKind::InvalidData,
            "lane retirement scan encountered an unknown artifact filename",
        ),
        (
            "legacy-native-manifest",
            "native_amx_application_manifests.norito",
            ErrorKind::InvalidData,
            "unexpected or legacy Native AMX evidence artifact",
        ),
        (
            "legacy-native-receipt-index",
            "native_amx_participant_receipts.index",
            ErrorKind::InvalidData,
            "unexpected or legacy Native AMX evidence artifact",
        ),
    ] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(label);
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let (kura, _, _) = open_published_retirement_kura(
            &root,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let retiring_lane = LaneId::new(1);
        let retiring = [LaneRetirementIdentity {
            lane_id: retiring_lane,
            dataspace_id: extended
                .entry(retiring_lane)
                .expect("retiring lane")
                .dataspace_id,
            lane_incarnation: extended_incarnations[&retiring_lane],
        }];
        let artifact_dir = Kura::lane_artifact_dir(
            &extended
                .entry(retiring_lane)
                .expect("retiring lane")
                .blocks_dir(&root),
        );
        fs::create_dir_all(&artifact_dir).expect("artifact directory");
        fs::write(artifact_dir.join(file_name), b"stale autonomous bytes")
            .expect("stale autonomous artifact");
        let _prune_guard = kura.prune_lock.lock();
        let _canonical_chain_guard = kura.canonical_chain_lock.lock();
        let pending_canonical_bytes = kura
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("measure pending canonical bytes before retirement scan");
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        let error = kura
            .ensure_first_release_lane_retirement_admissible_locked(
                pending_canonical_bytes,
                &retiring,
            )
            .expect_err("the production first-release policy must reject stale autonomous state");
        assert_geometry_io_error(&error, expected_kind, expected_message);
    }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn first_release_retirement_discards_unpublished_temp_for_every_fixed_pair() {
    for (label, data_file) in [
        ("ownership", LANE_ARTIFACTS_DATA_FILE),
        ("execution-input", LANE_BLOCK_EXECUTION_INPUTS_DATA_FILE),
        (
            "execution-preflight",
            LANE_BLOCK_EXECUTION_PREFLIGHTS_DATA_FILE,
        ),
        ("certified", CERTIFIED_LANE_BLOCKS_DATA_FILE),
        (
            "application-receipt",
            LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE,
        ),
    ] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(label);
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let (kura, _, _) = open_published_retirement_kura(
            &root,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let retiring_lane = LaneId::new(1);
        let retiring_entry = extended.entry(retiring_lane).expect("retiring lane entry");
        let artifact_dir = Kura::lane_artifact_dir(&retiring_entry.blocks_dir(&root));
        fs::create_dir_all(&artifact_dir).expect("artifact directory");
        let temp_path = artifact_dir.join(data_file).with_extension("norito.tmp");
        fs::write(&temp_path, b"unpublished progress rewrite")
            .expect("stage unpublished progress data temp");
        kura.first_release_lane_retirement_admissible_for_test(
            retiring_lane,
            retiring_entry.dataspace_id,
            extended_incarnations[&retiring_lane],
        )
        .unwrap_or_else(|error| {
            panic!("{label} unpublished temp should be recoverable: {error:?}")
        });
        assert!(
            !temp_path.exists(),
            "{label} unpublished temp must be removed before the immutable scan"
        );
    }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn first_release_retirement_rejects_directory_substitution_at_pair_refresh() {
    for (label, data_file, kind) in [
        (
            "ownership",
            LANE_ARTIFACTS_DATA_FILE,
            "lane retirement lane-block artifact",
        ),
        (
            "execution-input",
            LANE_BLOCK_EXECUTION_INPUTS_DATA_FILE,
            "lane retirement execution input",
        ),
        (
            "execution-preflight",
            LANE_BLOCK_EXECUTION_PREFLIGHTS_DATA_FILE,
            "lane retirement execution preflight",
        ),
        (
            "certified",
            CERTIFIED_LANE_BLOCKS_DATA_FILE,
            "lane retirement certified lane block",
        ),
        (
            "application-receipt",
            LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE,
            "lane retirement application receipt",
        ),
    ] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(label);
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let (kura, _, _) = open_published_retirement_kura(
            &root,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let retiring_lane = LaneId::new(1);
        let retiring_entry = extended.entry(retiring_lane).expect("retiring lane entry");
        let artifact_dir = Kura::lane_artifact_dir(&retiring_entry.blocks_dir(&root));
        fs::create_dir_all(&artifact_dir).expect("artifact directory");
        let temp_data_path = artifact_dir.join(data_file).with_extension("norito.tmp");
        fs::write(&temp_data_path, b"unpublished progress rewrite")
            .expect("stage recoverable progress temp");
        let sentinel_name = "refresh-identity-sentinel";
        let sentinel: &[u8] = b"bound directory object must remain authoritative";
        fs::write(artifact_dir.join(sentinel_name), sentinel)
            .expect("write bound-directory identity sentinel");
        let displaced = root.join("displaced-lane-artifacts");
        substitute_progress_directory_after_recovery_for_test(
            kind,
            &artifact_dir,
            displaced.clone(),
        );
        let error = kura
            .first_release_lane_retirement_admissible_for_test(
                retiring_lane,
                retiring_entry.dataspace_id,
                extended_incarnations[&retiring_lane],
            )
            .expect_err("directory substitution must fail at the pair-refresh boundary");
        assert_geometry_io_error(
            &error,
            ErrorKind::InvalidData,
            &format!("{kind} artifact directory changed during progress recovery"),
        );
        assert!(
            artifact_dir.is_dir(),
            "replacement directory remains visible"
        );
        assert_eq!(
            fs::read(displaced.join(sentinel_name)).expect("read displaced identity sentinel"),
            sentinel,
        );
        assert!(
            !displaced
                .join(data_file)
                .with_extension("norito.tmp")
                .exists(),
            "{label} cleanup must finish before the refresh substitution"
        );
    }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn first_release_retirement_classifies_recovery_sync_failure_as_retryable() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("retryable-pair-recovery");
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, extended_activations) = retirement_test_geometry();
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let (kura, _, _) = open_published_retirement_kura(
        &root,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let retiring_lane = LaneId::new(1);
    let retiring_entry = extended.entry(retiring_lane).expect("retiring lane entry");
    let artifact_dir = Kura::lane_artifact_dir(&retiring_entry.blocks_dir(&root));
    fs::create_dir_all(&artifact_dir).expect("artifact directory");
    let data_path = artifact_dir.join(LANE_ARTIFACTS_DATA_FILE);
    let index_path = artifact_dir.join(LANE_ARTIFACTS_INDEX_FILE);
    let temp_data_path = data_path.with_extension("norito.tmp");
    let temp_index_path = index_path.with_extension("index.tmp");
    let payload: &[u8] = b"retryable ownership rewrite";
    fs::write(&temp_data_path, payload).expect("stage retryable ownership payload");
    fs::write(
        &temp_index_path,
        SidecarIndexEntry {
            offset: 0,
            len: u64::try_from(payload.len()).expect("test payload length"),
        }
        .to_bytes(),
    )
    .expect("stage retryable ownership index");
    ProgressSidecarDurabilityFault::Data.inject();
    let error = kura
        .first_release_lane_retirement_admissible_for_test(
            retiring_lane,
            retiring_entry.dataspace_id,
            extended_incarnations[&retiring_lane],
        )
        .expect_err("an interrupted recovery sync must remain retryable");
    assert_geometry_io_error(
        &error,
        ErrorKind::WouldBlock,
        "lane retirement lane-block artifact recovery did not reach a durable fixed point",
    );
    assert!(temp_data_path.is_file());
    assert!(temp_index_path.is_file());
    kura.first_release_lane_retirement_admissible_for_test(
        retiring_lane,
        retiring_entry.dataspace_id,
        extended_incarnations[&retiring_lane],
    )
    .expect("retry must recover the same complete rewrite exactly once");
    assert!(data_path.is_file());
    assert!(index_path.is_file());
    assert!(!temp_data_path.exists());
    assert!(!temp_index_path.exists());
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn first_release_retirement_recovers_complete_certified_rewrite_before_snapshot() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("complete-certified-rewrite");
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, extended_activations) = retirement_test_geometry();
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let (kura, _, _) = open_published_retirement_kura(
        &root,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let retiring_lane = LaneId::new(1);
    let retiring_entry = extended.entry(retiring_lane).expect("retiring lane entry");
    let retiring_incarnation = extended_incarnations[&retiring_lane];
    let _work = install_merge_applied_retirement_work(&kura, retiring_incarnation);
    let (data_path, index_path) = Kura::certified_lane_block_paths_for_entry(retiring_entry, &root);
    let data_before = fs::read(&data_path).expect("certified data before crash window");
    let index_before = fs::read(&index_path).expect("certified index before crash window");
    let temp_data_path = data_path.with_extension("norito.tmp");
    let temp_index_path = index_path.with_extension("index.tmp");
    fs::rename(&data_path, &temp_data_path).expect("stage complete certified data temp");
    fs::rename(&index_path, &temp_index_path).expect("stage complete certified index temp");
    kura.first_release_lane_retirement_admissible_for_test(
        retiring_lane,
        retiring_entry.dataspace_id,
        retiring_incarnation,
    )
    .expect("complete certified rewrite must recover before retirement admission");
    assert_eq!(
        fs::read(&data_path).expect("recovered certified data"),
        data_before
    );
    assert_eq!(
        fs::read(&index_path).expect("recovered certified index"),
        index_before
    );
    assert!(!temp_data_path.exists());
    assert!(!temp_index_path.exists());
}
#[test]
fn first_release_retirement_repairs_frontier_only_certified_work_before_snapshot() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("frontier-only-certified-work");
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, extended_activations) = retirement_test_geometry();
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let (kura, _, _) = open_published_retirement_kura(
        &root,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let retiring_lane = LaneId::new(1);
    let retiring_entry = extended.entry(retiring_lane).expect("retiring lane entry");
    let retiring_incarnation = extended_incarnations[&retiring_lane];
    let work = install_merge_applied_retirement_work(&kura, retiring_incarnation);
    let (data_path, index_path) = Kura::certified_lane_block_paths_for_entry(retiring_entry, &root);
    let (frontier_path, _) =
        Kura::latest_certified_lane_block_frontier_paths_for_entry(retiring_entry, &root);
    assert!(
        frontier_path.is_file(),
        "durable frontier must precede the pair"
    );
    fs::remove_file(&data_path).expect("simulate lost certified data after frontier publish");
    fs::remove_file(&index_path).expect("simulate lost certified index after frontier publish");
    kura.first_release_lane_retirement_admissible_for_test(
        retiring_lane,
        retiring_entry.dataspace_id,
        retiring_incarnation,
    )
    .expect("retirement scan must repair the exact frontier certificate before its snapshot");
    assert!(data_path.is_file(), "frontier recovery must recreate data");
    assert!(
        index_path.is_file(),
        "frontier recovery must recreate index"
    );
    assert_eq!(
        kura.read_certified_lane_block_artifact(
            retiring_lane,
            work.certified.proposal.descriptor.lane_block_height,
        )
        .as_ref(),
        Some(&work.certified),
        "retirement recovery must restore the frontier certificate exactly"
    );
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn first_release_retirement_rejects_obsolete_autonomous_rewrite_without_promotion() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("complete-autonomous-rewrite");
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, extended_activations) = retirement_test_geometry();
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let (kura, _, _) = open_published_retirement_kura(
        &root,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let retiring_lane = LaneId::new(1);
    let retiring_entry = extended.entry(retiring_lane).expect("retiring lane entry");
    let artifact_dir = Kura::lane_artifact_dir(&retiring_entry.blocks_dir(&root));
    fs::create_dir_all(&artifact_dir).expect("artifact directory");
    let data_path = artifact_dir.join(OBSOLETE_AUTONOMOUS_LANE_BLOCKS_DATA_FILE);
    let index_path = artifact_dir.join(OBSOLETE_AUTONOMOUS_LANE_BLOCKS_INDEX_FILE);
    let temp_data_path = data_path.with_extension("norito.tmp");
    let temp_index_path = index_path.with_extension("index.tmp");
    let data: &[u8] = b"complete stale autonomous payload";
    fs::write(&temp_data_path, data).expect("stage complete autonomous data temp");
    fs::write(
        &temp_index_path,
        SidecarIndexEntry {
            offset: 0,
            len: u64::try_from(data.len()).expect("autonomous test payload length"),
        }
        .to_bytes(),
    )
    .expect("stage complete autonomous index temp");
    let error = kura
        .first_release_lane_retirement_admissible_for_test(
            retiring_lane,
            retiring_entry.dataspace_id,
            extended_incarnations[&retiring_lane],
        )
        .expect_err("obsolete autonomous rewrite staging must fail closed");
    assert_geometry_io_error(
        &error,
        ErrorKind::WouldBlock,
        "lane retirement scan found an in-flight autonomous sidecar",
    );
    assert!(!data_path.exists());
    assert!(!index_path.exists());
    assert!(temp_data_path.exists());
    assert!(temp_index_path.exists());
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn first_release_retirement_recovery_rejects_temp_symlink_without_external_writes() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("recovery-temp-symlink");
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, extended_activations) = retirement_test_geometry();
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let (kura, _, _) = open_published_retirement_kura(
        &root,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let retiring_lane = LaneId::new(1);
    let retiring_entry = extended.entry(retiring_lane).expect("retiring lane entry");
    let artifact_dir = Kura::lane_artifact_dir(&retiring_entry.blocks_dir(&root));
    fs::create_dir_all(&artifact_dir).expect("artifact directory");
    let temp_data_path = artifact_dir
        .join(CERTIFIED_LANE_BLOCKS_DATA_FILE)
        .with_extension("norito.tmp");
    let external = root.join("external-progress-sentinel");
    let sentinel: &[u8] = b"must remain outside retirement recovery";
    fs::write(&external, sentinel).expect("write external recovery sentinel");
    symlink(&external, &temp_data_path).expect("install recovery temp symlink");
    let error = kura
        .first_release_lane_retirement_admissible_for_test(
            retiring_lane,
            retiring_entry.dataspace_id,
            extended_incarnations[&retiring_lane],
        )
        .expect_err("a recovery temp symlink must fail closed");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "lane retirement certified lane block recovery did not reach a durable fixed point",
    );
    let external_after = fs::read(&external).expect("read external recovery sentinel");
    assert_eq!(external_after.as_slice(), sentinel);
    assert!(
        fs::symlink_metadata(&temp_data_path)
            .expect("recovery temp symlink retained")
            .file_type()
            .is_symlink()
    );
}
#[test]
fn first_release_retirement_requires_bound_progress_sidecar_durability() {
    for (label, fault, expected_message) in [
        (
            "data",
            ProgressSidecarDurabilityFault::Data,
            "lane retirement certified lane block durability attestation failed",
        ),
        (
            "index",
            ProgressSidecarDurabilityFault::Index,
            "lane retirement certified lane block durability attestation failed",
        ),
        (
            "immediate-directory",
            ProgressSidecarDurabilityFault::ImmediateDirectory,
            "absent progress sidecar durability attestation failed",
        ),
        (
            "ancestor",
            ProgressSidecarDurabilityFault::Ancestor(0),
            "absent progress sidecar durability attestation failed",
        ),
    ] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(label);
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let (kura, journal_before, _) = open_published_retirement_kura(
            &root,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let retiring_lane = LaneId::new(1);
        let retiring_entry = extended.entry(retiring_lane).expect("retiring lane entry");
        let retiring_blocks = retiring_entry.blocks_dir(&root);
        let retiring_incarnation = extended_incarnations[&retiring_lane];
        let work = install_merge_applied_retirement_work(&kura, retiring_incarnation);
        assert_eq!(work.certified.proposal.descriptor.lane_id, retiring_lane);
        assert_eq!(work.entry.epoch_id, 1);
        assert_eq!(work.carrier.header().height().get(), 2);
        fault.inject();
        let error = kura
            .first_release_lane_retirement_admissible_for_test(
                retiring_lane,
                retiring_entry.dataspace_id,
                retiring_incarnation,
            )
            .expect_err("a failed durability barrier must block production retirement");
        assert_geometry_io_error(&error, ErrorKind::WouldBlock, expected_message);
        assert!(
            retiring_blocks.is_dir(),
            "{label} barrier failure must retain the live retiring lane"
        );
        assert_eq!(
            fs::read(kura.lane_geometry_journal_path()).expect("unchanged retirement journal"),
            journal_before,
            "{label} barrier failure must not publish a retirement intent"
        );
        kura.first_release_lane_retirement_admissible_for_test(
            retiring_lane,
            retiring_entry.dataspace_id,
            retiring_incarnation,
        )
        .unwrap_or_else(|error| {
            panic!("{label} durability recovery must make retirement admissible: {error:?}")
        });
        kura.apply_lane_geometry_transition(
            &extended,
            &initial,
            &extended_incarnations,
            &initial_incarnations,
            &extended_activations,
            &initial_activations,
            &BTreeSet::new(),
        )
        .unwrap_or_else(|error| {
            panic!("{label} durability recovery must permit retirement: {error:?}")
        });
        assert!(
            !retiring_blocks.exists(),
            "{label} recovery must allow the authenticated lane move"
        );
    }
}
#[test]
fn fake_stale_and_forked_payload_hints_cannot_bypass_scale_in_admission() {
    for label in ["fork-hash", "stale-height", "stale-view"] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(label);
        let (initial, extended) = retirement_test_configs();
        let (extended_incarnations, extended_activations) = retirement_test_geometry();
        let initial_incarnations =
            BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
        let initial_activations =
            BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
        let (kura, journal_before, _) = open_published_retirement_kura(
            &root,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let mut certified = certified_geometry_lane_block(
            LaneId::SINGLE,
            DataSpaceId::new(7),
            extended_incarnations[&LaneId::SINGLE],
            1,
        );
        let canonical_height = certified.proposal.descriptor.proposal_height;
        let canonical_height_index = NonZeroUsize::new(
            usize::try_from(canonical_height).expect("canonical height fits usize"),
        )
        .expect("canonical height is non-zero");
        let stale_height = canonical_height
            .checked_sub(1)
            .filter(|height| *height > 0)
            .expect("fixture proposal height has a stale predecessor");
        let (canonical_hash, _) = durable_geometry_snapshot_identity(&kura, canonical_height);
        let canonical_view = kura
            .get_block(canonical_height_index)
            .expect("canonical block")
            .header()
            .view_change_index();
        let hint = match label {
            "fork-hash" => LaneBlockProposalPayloadHintV1 {
                proposal_height: canonical_height,
                proposal_view: canonical_view,
                proposal_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"geometry-retirement-fork",
                )),
            },
            "stale-height" => LaneBlockProposalPayloadHintV1 {
                proposal_height: stale_height,
                proposal_view: canonical_view,
                proposal_block_hash: canonical_hash,
            },
            "stale-view" => LaneBlockProposalPayloadHintV1 {
                proposal_height: canonical_height,
                proposal_view: canonical_view.saturating_add(1),
                proposal_block_hash: canonical_hash,
            },
            _ => unreachable!(),
        };
        certified.proposal.payload_block_hint = Some(hint);
        kura.write_certified_lane_block_artifact(&certified)
            .expect("persist adversarial hinted certificate");
        let error = kura
            .apply_lane_geometry_transition(
                &extended,
                &initial,
                &extended_incarnations,
                &initial_incarnations,
                &extended_activations,
                &initial_activations,
                &BTreeSet::new(),
            )
            .expect_err("an unproven hint cannot mark certified work as applied");
        let expected_message = match label {
            "fork-hash" => {
                "lane retirement payload hint does not identify the canonical durable block"
            }
            "stale-height" => {
                "lane retirement payload hint height differs from the certified descriptor"
            }
            "stale-view" => "lane retirement payload hint differs from its canonical block header",
            _ => unreachable!(),
        };
        assert_geometry_io_error(&error, ErrorKind::InvalidData, expected_message);
        assert_eq!(
            fs::read(kura.lane_geometry_journal_path()).expect("unchanged geometry journal"),
            journal_before,
            "{label} hint must fail before retirement intent"
        );
    }
}
#[test]
fn canonical_block_and_current_receipt_release_applied_participant_work() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, extended_activations) = retirement_test_geometry();
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let (kura, _, baseline_records) = open_published_retirement_kura(
        &root,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let network_id = crate::sumeragi::synthetic_network_id("geometry-retirement-committed");
    let transaction = TransactionBuilder::new(
        network_id,
        (*SAMPLE_GENESIS_ACCOUNT_ID).clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "geometry retirement committed participant work".to_owned(),
    )])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let source_hash = transaction.hash();
    let mut source_id = [0_u8; Hash::LENGTH];
    source_id.copy_from_slice(source_hash.as_ref());
    let parent: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, None)
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    kura.store_block(Arc::new(parent.clone()))
        .expect("store pre-activation parent block");
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
    let mut block: SignedBlock = BlockBuilder::new(vec![accepted])
        .chain(0, Some(&parent))
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    let entrypoint = block
        .external_entrypoints_cloned()
        .next()
        .expect("committed external entrypoint");
    let entrypoint_hash = entrypoint.hash();
    let (mut proposal, ownership) = geometry_lane_proposal_and_ownership(
        LaneId::SINGLE,
        DataSpaceId::new(7),
        extended_incarnations[&LaneId::SINGLE],
        block.header().height().get(),
        block.header().view_change_index(),
        1,
        0,
        Hash::from(entrypoint_hash),
        &producer,
    );
    let plan = crate::queue::RoutingPlan::native_amx(
        crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::new(7)),
        vec![crate::queue::RouteLeg::new(
            crate::queue::RoutingDecision::new(LaneId::new(1), DataSpaceId::new(8)),
            crate::queue::RouteLegRole::Participant,
        )],
    );
    let receipt = geometry_native_amx_receipt(
        network_id,
        source_id,
        entrypoint_hash,
        &plan,
        &proposal,
        extended_incarnations[&LaneId::new(1)],
        0,
        &producer,
    );
    let context = crate::queue::execution_context_for_routing_plan(entrypoint_hash, &plan)
        .with_native_amx_receipt(receipt);
    block.set_execution_context(Some(
        BlockExecutionContextBundle::new(vec![context])
            .with_lane_payload_ownerships(vec![ownership]),
    ));
    let entrypoint_hashes = block
        .external_entrypoints_cloned()
        .map(|entrypoint| entrypoint.hash())
        .collect::<Vec<_>>();
    block
        .set_transaction_results(
            Vec::new(),
            &entrypoint_hashes,
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        )
        .expect("attach committed result");
    let block = Arc::new(block);
    proposal.payload_block_hint = Some(LaneBlockProposalPayloadHintV1 {
        proposal_height: block.header().height().get(),
        proposal_view: block.header().view_change_index(),
        proposal_block_hash: block.hash(),
    });
    kura.store_block(Arc::clone(&block))
        .expect("store canonical global block");
    let certified = certified_geometry_lane_block_for_proposal(proposal.clone(), &producer);
    kura.write_certified_lane_block_artifact(&certified)
        .expect("persist globally backed lane certificate");
    kura.persist_lane_block_application_receipt(&proposal)
        .expect("persist canonical current application receipt");
    kura.apply_lane_geometry_transition(
        &extended,
        &initial,
        &extended_incarnations,
        &initial_incarnations,
        &extended_activations,
        &initial_activations,
        &BTreeSet::new(),
    )
    .expect("canonically applied participant work no longer pins scale-in");
    assert_eq!(
        kura.read_lane_geometry_journal()
            .expect("geometry journal")
            .records
            .len(),
        baseline_records + 1
    );
}
#[test]
fn consecutive_published_retirements_do_not_resurrect_intermediate_lanes() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let catalog = |active: u32| {
        let lanes = (0..active)
            .map(|raw| ModelLaneConfig {
                id: LaneId::new(raw),
                alias: if raw == 0 {
                    "default".to_owned()
                } else {
                    format!("elastic-{raw}")
                },
                ..ModelLaneConfig::default()
            })
            .collect::<Vec<_>>();
        let catalog = LaneCatalog::new(
            NonZeroU32::new(4).expect("fixed geometry bound is non-zero"),
            lanes,
        )
        .expect("consecutive retirement catalog");
        RuntimeLaneConfig::from_catalog(&catalog)
    };
    let configs = [catalog(4), catalog(3), catalog(2), catalog(1)];
    let all_incarnations = (0_u32..4)
        .map(|raw| {
            (
                LaneId::new(raw),
                Hash::new(format!("consecutive-retirement-incarnation-{raw}")),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let all_activations = (0_u32..4)
        .map(|raw| (LaneId::new(raw), u64::from(raw)))
        .collect::<BTreeMap<_, _>>();
    let geometry_for = |active: u32| {
        let incarnations = all_incarnations
            .iter()
            .filter(|(lane, _)| lane.as_u32() < active)
            .map(|(lane, incarnation)| (*lane, *incarnation))
            .collect::<BTreeMap<_, _>>();
        let activations = all_activations
            .iter()
            .filter(|(lane, _)| lane.as_u32() < active)
            .map(|(lane, activation)| (*lane, *activation))
            .collect::<BTreeMap<_, _>>();
        (incarnations, activations)
    };
    let geometries = [
        geometry_for(4),
        geometry_for(3),
        geometry_for(2),
        geometry_for(1),
    ];
    let kura = open_kura(&root, &configs[0]);
    install_retirement_test_lane_markers(&kura, &configs[0], &geometries[0].0, &geometries[0].1);
    for index in 0..3 {
        kura.apply_lane_geometry_transition(
            &configs[index],
            &configs[index + 1],
            &geometries[index].0,
            &geometries[index + 1].0,
            &geometries[index].1,
            &geometries[index + 1].1,
            &BTreeSet::new(),
        )
        .unwrap_or_else(|error| panic!("consecutive retirement {} failed: {error:?}", index + 1));
        kura.mark_lane_geometry_catalog_published(
            &configs[index + 1],
            &geometries[index + 1].0,
            &geometries[index + 1].1,
            None,
        )
        .expect("publish consecutive retirement catalog");
        for retired in (4 - u32::try_from(index).expect("index fits u32") - 1)..4 {
            assert!(
                !configs[0]
                    .entry(LaneId::new(retired))
                    .expect("retired lane entry")
                    .blocks_dir(&root)
                    .exists(),
                "published transition resurrected retired lane {retired}"
            );
        }
    }
    assert_eq!(
        kura.read_lane_geometry_journal()
            .expect("geometry journal")
            .records
            .len(),
        3,
        "every consecutive retirement remains recoverable until checkpoint GC"
    );
}
#[test]
fn zero_file_create_intent_rolls_back_to_a_sealed_image_and_replays() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    let operation = persist_create_intent(
        &kura,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let updated = operation.updated.as_ref().expect("created binding");
    let live_blocks = kura.binding_blocks_path(updated);
    let live_merge = kura.binding_merge_path(updated);
    let unpublished_blocks = kura
        .resolve_relative_path(&operation.unpublished_blocks_path)
        .expect("unpublished blocks");
    let unpublished_merge = kura
        .resolve_relative_path(&operation.unpublished_merge_path)
        .expect("unpublished merge");
    assert!(!live_blocks.exists());
    assert!(!live_merge.exists());
    assert!(!unpublished_blocks.exists());
    assert!(!unpublished_merge.exists());
    for _ in 0..2 {
        kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
            .expect("zero-file Intent rollback is idempotent");
        assert!(!live_blocks.exists());
        assert!(!live_merge.exists());
        kura.require_sealed_geometry_pair_at(
            updated,
            &unpublished_blocks,
            &unpublished_merge,
            &unpublished_blocks,
            &unpublished_merge,
        )
        .expect("rollback persists an authenticated empty image");
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[0].phase,
            LaneGeometryPhase::RolledBack
        );
    }
    // A same-authority retry must resume when replay durably retargeted the retained pair to
    // live but crashed before the first rename. The terminal phase is deliberately left at
    // `RolledBack` across that filesystem window.
    kura.seal_geometry_pair_move(
        updated,
        &unpublished_blocks,
        &unpublished_merge,
        &live_blocks,
        &live_merge,
    )
    .expect("inject same-authority replay crash before first rename");
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect("same-authority replay resumes its pre-rename seal");
    kura.require_complete_geometry_binding_at(updated, &live_blocks, &live_merge)
        .expect("created lane is live after same-authority replay");
    let live_lane_artifacts = live_blocks.join(LANE_ARTIFACTS_DIR_NAME);
    let lane_artifacts_metadata = fs::symlink_metadata(&live_lane_artifacts)
        .expect("created lane artifact directory metadata");
    assert!(
        lane_artifacts_metadata.is_dir() && !lane_artifacts_metadata.file_type().is_symlink(),
        "lane creation must publish a direct lane-artifact directory before activation"
    );
    assert!(
        fs::read_dir(&live_lane_artifacts)
            .expect("read created lane artifact directory")
            .next()
            .is_none(),
        "a newly activated lane must start with an empty artifact namespace"
    );
    let unexpected_artifact = live_lane_artifacts.join("unexpected.norito");
    fs::write(&unexpected_artifact, b"foreign").expect("inject foreign lane artifact");
    let error = preflight_empty_block_store_without_marker(&live_blocks, Some(updated), true)
        .expect_err("unexpected lane-artifact contents must fail empty-image preflight");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "journal-owned empty block store has a non-empty lane-artifact directory",
    );
    fs::remove_file(unexpected_artifact).expect("restore empty lane artifact directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let displaced = root.join("displaced-direct-lane-artifacts");
        let outside = root.join("outside-lane-artifacts");
        fs::create_dir(&outside).expect("create outside lane artifact directory");
        fs::rename(&live_lane_artifacts, &displaced)
            .expect("displace direct lane artifact directory");
        symlink(&outside, &live_lane_artifacts).expect("inject lane artifact symlink");
        let error = preflight_empty_block_store_without_marker(&live_blocks, Some(updated), true)
            .expect_err("symlinked lane-artifact namespace must fail empty-image preflight");
        assert_geometry_io_error(
            &error,
            ErrorKind::InvalidData,
            "journal-owned empty block store has an invalid lane-artifact directory",
        );
        fs::remove_file(&live_lane_artifacts).expect("remove lane artifact symlink");
        fs::rename(displaced, &live_lane_artifacts)
            .expect("restore direct lane artifact directory");
        fs::remove_dir(outside).expect("remove outside lane artifact directory");
    }
    assert_eq!(
        kura.read_lane_geometry_journal().expect("journal").records[0].phase,
        LaneGeometryPhase::CatalogPublished
    );
    kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
        .expect("return replayed create to its retained rollback image");
    kura.require_sealed_geometry_pair_at(
        updated,
        &unpublished_blocks,
        &unpublished_merge,
        &unpublished_blocks,
        &unpublished_merge,
    )
    .expect("same-authority lifecycle restores an immutable rollback image");
    // Replay persisted its live-target seal but died before either rename. Remaining on the
    // old catalog must recognize that exact opposite-path seal and normalize it back to the
    // retained rollback image.
    kura.seal_geometry_pair_move(
        updated,
        &unpublished_blocks,
        &unpublished_merge,
        &live_blocks,
        &live_merge,
    )
    .expect("inject RolledBack replay crash before first rename");
    kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
        .expect("old-catalog recovery reverses the pre-rename replay seal");
    kura.require_sealed_geometry_pair_at(
        updated,
        &unpublished_blocks,
        &unpublished_merge,
        &unpublished_blocks,
        &unpublished_merge,
    )
    .expect("rollback image is sealed back to itself");
    for _ in 0..2 {
        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect("sealed rollback image replays exactly");
        kura.require_complete_geometry_binding_at(updated, &live_blocks, &live_merge)
            .expect("created lane is live after replay");
        assert!(!unpublished_blocks.exists());
        assert!(!unpublished_merge.exists());
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[0].phase,
            LaneGeometryPhase::CatalogPublished
        );
    }
}
#[test]
fn create_intent_repairs_authenticated_blocks_before_merge_for_rollback_and_replay() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    let operation = persist_create_intent(
        &kura,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let updated = operation.updated.as_ref().expect("created binding");
    let staged = LaneGeometryBinding {
        blocks_path: operation.unpublished_blocks_path.clone(),
        merge_path: operation.unpublished_merge_path.clone(),
        ..updated.clone()
    };
    let staged_blocks = kura.binding_blocks_path(&staged);
    let staged_merge = kura.binding_merge_path(&staged);
    kura.provision_geometry_binding(&staged)
        .expect("provision journal-owned staging");
    fs::remove_file(&staged_merge).expect("inject crash before merge creation");
    assert!(staged_blocks.join(MARKER_FILE_NAME).is_file());
    assert!(!staged_merge.exists());
    kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
        .expect("rollback repairs authenticated partial provisioning");
    kura.require_sealed_geometry_pair_at(
        updated,
        &staged_blocks,
        &staged_merge,
        &staged_blocks,
        &staged_merge,
    )
    .expect("repaired rollback image is sealed");
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect("replay consumes the repaired image");
    kura.require_complete_geometry_binding_at(
        updated,
        &kura.binding_blocks_path(updated),
        &kura.binding_merge_path(updated),
    )
    .expect("created binding is complete after replay");
}
#[test]
fn create_intent_rejects_merge_only_staging_without_adopting_it() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    let operation = persist_create_intent(
        &kura,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let staged_blocks = kura
        .resolve_relative_path(&operation.unpublished_blocks_path)
        .expect("staged blocks");
    let staged_merge = kura
        .resolve_relative_path(&operation.unpublished_merge_path)
        .expect("staged merge");
    create_dir_all_with_context(staged_merge.parent().expect("merge parent"))
        .expect("create merge parent");
    fs::write(&staged_merge, b"").expect("inject merge-only staging");
    let error = kura
        .recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect_err("merge-only staging must fail closed");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "replacement provisioning has an orphan block or merge-log path",
    );
    assert!(!staged_blocks.exists());
    assert!(staged_merge.is_file());
    assert_eq!(
        kura.read_lane_geometry_journal().expect("journal").records[0].phase,
        LaneGeometryPhase::Intent
    );
}
#[test]
fn create_intent_rejects_complete_unsealed_foreign_pairs() {
    for location in ["staging", "live"] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(format!("kura-{location}"));
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        let operation = persist_create_intent(
            &kura,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let updated = operation.updated.as_ref().expect("created binding");
        let injected = if location == "staging" {
            LaneGeometryBinding {
                blocks_path: operation.unpublished_blocks_path.clone(),
                merge_path: operation.unpublished_merge_path.clone(),
                ..updated.clone()
            }
        } else {
            updated.clone()
        };
        kura.provision_geometry_binding(&injected)
            .expect("provision valid-looking unsealed pair");
        let injected_blocks = kura.binding_blocks_path(&injected);
        let injected_merge = kura.binding_merge_path(&injected);
        let sentinel = injected_blocks.join("foreign-intent-payload");
        fs::write(&sentinel, b"must-not-be-adopted").expect("inject foreign block payload");
        let error = kura
            .recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
            .expect_err("an unsealed nonempty pair must not gain authority from Intent");
        assert_geometry_io_error(
            &error,
            ErrorKind::InvalidData,
            "unbound configured primary block store contains an unexpected entry",
        );
        assert_eq!(
            fs::read(&sentinel).expect("foreign payload retained for diagnosis"),
            b"must-not-be-adopted"
        );
        assert!(injected_merge.is_file());
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[0].phase,
            LaneGeometryPhase::Intent
        );
    }
}
#[test]
fn terminal_geometry_replay_never_reauthorizes_empty_provisioning() {
    // A failed rollback of a published transition must retain `CatalogPublished`; otherwise a
    // restart could reinterpret it as a first-application Intent and manufacture empty state.
    {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("apply create transition");
        kura.mark_lane_geometry_catalog_published(
            &extended,
            &extended_incarnations,
            &extended_activations,
            None,
        )
        .expect("publish create transition");
        let operation = kura
            .read_lane_geometry_journal()
            .expect("published journal")
            .records[0]
            .operations[0]
            .clone();
        let updated = operation.updated.as_ref().expect("created binding");
        fs::remove_dir_all(kura.binding_blocks_path(updated))
            .expect("simulate loss of published blocks");
        fs::remove_file(kura.binding_merge_path(updated))
            .expect("simulate loss of published merge log");
        for _ in 0..2 {
            let error = kura
                .recover_lane_geometry_journal(
                    &initial,
                    &initial_incarnations,
                    &initial_activations,
                )
                .expect_err("missing published evidence must fail on every retry");
            assert_geometry_io_error(
                &error,
                ErrorKind::NotFound,
                "durable lane geometry evidence is missing; refusing to provision an empty replacement",
            );
            assert_eq!(
                kura.read_lane_geometry_journal().expect("journal").records[0].phase,
                LaneGeometryPhase::CatalogPublished
            );
            assert!(
                !kura
                    .resolve_relative_path(&operation.unpublished_blocks_path)
                    .expect("unpublished blocks")
                    .exists()
            );
            assert!(
                !kura
                    .resolve_relative_path(&operation.unpublished_merge_path)
                    .expect("unpublished merge")
                    .exists()
            );
        }
    }
    // The inverse direction must likewise retain `RolledBack` when its authenticated retained
    // image disappears; replay is not authority to create a replacement from nothing.
    {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("apply create transition");
        kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
            .expect("roll transition back to its retained image");
        let operation = kura
            .read_lane_geometry_journal()
            .expect("rolled-back journal")
            .records[0]
            .operations[0]
            .clone();
        let unpublished_blocks = kura
            .resolve_relative_path(&operation.unpublished_blocks_path)
            .expect("unpublished blocks");
        let unpublished_merge = kura
            .resolve_relative_path(&operation.unpublished_merge_path)
            .expect("unpublished merge");
        fs::remove_dir_all(&unpublished_blocks).expect("simulate loss of retained block image");
        fs::remove_file(&unpublished_merge).expect("simulate loss of retained merge image");
        for _ in 0..2 {
            let error = kura
                .recover_lane_geometry_journal(
                    &extended,
                    &extended_incarnations,
                    &extended_activations,
                )
                .expect_err("missing retained evidence must fail on every retry");
            assert_geometry_io_error(
                &error,
                ErrorKind::NotFound,
                "durable lane geometry evidence is missing; refusing to provision an empty replacement",
            );
            assert_eq!(
                kura.read_lane_geometry_journal().expect("journal").records[0].phase,
                LaneGeometryPhase::RolledBack
            );
            assert!(!unpublished_blocks.exists());
            assert!(!unpublished_merge.exists());
        }
    }
}
#[test]
fn recovery_rolls_back_partial_unpublished_create_and_replays_it_idempotently() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect("prepare journaled create");
    let lane1 = extended.entry(LaneId::new(1)).expect("lane one");
    assert!(lane1.blocks_dir(&root).exists());
    kura.apply_lane_geometry_transition(
        &initial,
        &initial,
        &initial_incarnations,
        &initial_incarnations,
        &initial_activations,
        &initial_activations,
        &BTreeSet::new(),
    )
    .expect("same-catalog startup rolls back unpublished create");
    assert!(!lane1.blocks_dir(&root).exists());
    // Model a process dying after restoring only the block directory from
    // the unpublished archive. Recovery of the old catalog must complete
    // the inverse operation without duplicating or dropping either path.
    let mut journal = kura.read_lane_geometry_journal().expect("read journal");
    journal.records[0].phase = LaneGeometryPhase::Intent;
    kura.write_lane_geometry_journal(&journal)
        .expect("persist in-progress roll-forward phase");
    let operation = journal.records[0].operations[0].clone();
    let updated = operation.updated.as_ref().expect("created binding");
    let unpublished_blocks = kura
        .resolve_relative_path(&operation.unpublished_blocks_path)
        .expect("unpublished blocks path");
    let unpublished_merge = kura
        .resolve_relative_path(&operation.unpublished_merge_path)
        .expect("unpublished merge path");
    let live_blocks = lane1.blocks_dir(&root);
    let live_merge = lane1.merge_log_path(&root);
    kura.seal_geometry_pair_move(
        updated,
        &unpublished_blocks,
        &unpublished_merge,
        &live_blocks,
        &live_merge,
    )
    .expect("seal partial roll-forward exactly as production does");
    kura.move_geometry_path(&unpublished_blocks, &live_blocks, true)
        .expect("inject partial roll-forward");
    assert!(lane1.blocks_dir(&root).exists());
    for _ in 0..2 {
        kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
            .expect("idempotent rollback recovery");
        assert!(!lane1.blocks_dir(&root).exists());
        assert!(!lane1.merge_log_path(&root).exists());
    }
    // The catalog is now authoritative (as after snapshot/block replay),
    // so the same retained intent must roll forward and recover the exact
    // unpublished segment instead of provisioning an empty replacement.
    for _ in 0..2 {
        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect("idempotent roll-forward recovery");
        assert!(lane1.blocks_dir(&root).exists());
        assert!(lane1.merge_log_path(&root).exists());
    }
    let recovered = kura.read_lane_geometry_journal().expect("read journal");
    assert_eq!(
        recovered.records[0].phase,
        LaneGeometryPhase::CatalogPublished
    );
}
#[test]
fn geometry_moves_never_clobber_targets_materialized_after_preflight() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    let source_blocks = root.join("move-collision/source-blocks");
    let target_blocks = root.join("move-collision/target-blocks");
    fs::create_dir_all(&source_blocks).expect("seed source block directory");
    fs::write(source_blocks.join("sentinel"), b"source-blocks")
        .expect("seed source block sentinel");
    *GEOMETRY_MOVE_TARGET_COLLISION
        .lock()
        .expect("geometry collision hook lock") = Some(target_blocks.clone());
    kura.move_geometry_path(&source_blocks, &target_blocks, true)
        .expect_err("a target created after preflight must stop the block-directory move");
    assert_eq!(
        fs::read(source_blocks.join("sentinel")).expect("source block sentinel retained"),
        b"source-blocks"
    );
    assert!(
        target_blocks.is_dir(),
        "the injected target must not be replaced by the source directory"
    );
    assert!(
        fs::read_dir(&target_blocks)
            .expect("read injected block target")
            .next()
            .is_none(),
        "the injected directory must remain untouched"
    );
    let source_merge = root.join("move-collision/source-merge.log");
    let target_merge = root.join("move-collision/target-merge.log");
    fs::write(&source_merge, b"source-merge").expect("seed source merge file");
    *GEOMETRY_MOVE_TARGET_COLLISION
        .lock()
        .expect("geometry collision hook lock") = Some(target_merge.clone());
    kura.move_geometry_path(&source_merge, &target_merge, false)
        .expect_err("a target created after preflight must stop the merge-file move");
    assert_eq!(
        fs::read(&source_merge).expect("source merge file retained"),
        b"source-merge"
    );
    assert_eq!(
        fs::read(&target_merge).expect("injected merge target retained"),
        b"injected-no-clobber-target"
    );
}
#[cfg(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox"
))]
#[test]
fn runtime_geometry_recovery_cannot_escape_a_substituted_parent() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let initial = RuntimeLaneConfig::from_catalog(&configured_primary_catalog("parent-alpha"));
    let updated = RuntimeLaneConfig::from_catalog(&configured_primary_catalog("parent-beta"));
    let (incarnations, activations) = initial_geometry();
    let kura = open_kura(&root, &initial);
    let _ = durable_geometry_snapshot_identity(&kura, 1);
    kura.apply_lane_geometry_transition(
        &initial,
        &updated,
        &incarnations,
        &incarnations,
        &activations,
        &activations,
        &BTreeSet::new(),
    )
    .expect("apply primary relabel before recovery");
    assert_eq!(
        kura.read_lane_geometry_journal()
            .expect("geometry journal")
            .records[0]
            .phase,
        LaneGeometryPhase::FilesApplied
    );
    let previous_blocks = initial.primary().blocks_dir(&root);
    let updated_blocks = updated.primary().blocks_dir(&root);
    let blocks_parent = previous_blocks.parent().expect("block-store parent");
    assert_eq!(updated_blocks.parent(), Some(blocks_parent));
    let displaced_parent = root.join("authenticated-blocks-parent");
    let outside_parent = temp.path().join("outside-blocks-parent");
    fs::create_dir(&outside_parent).expect("create outside replacement parent");
    *GEOMETRY_MOVE_PARENT_SUBSTITUTION
        .lock()
        .expect("geometry parent-substitution hook lock") = Some((
        blocks_parent.to_path_buf(),
        displaced_parent.clone(),
        outside_parent.clone(),
    ));
    kura.recover_lane_geometry_journal(&initial, &incarnations, &activations)
        .expect_err("a substituted runtime parent must fail recovery closed");
    assert!(
        GEOMETRY_MOVE_PARENT_SUBSTITUTION
            .lock()
            .expect("geometry parent-substitution hook lock")
            .is_none(),
        "the parent substitution must occur at the pre-rename barrier"
    );
    assert!(
        fs::symlink_metadata(blocks_parent)
            .expect("substituted parent metadata")
            .file_type()
            .is_symlink()
    );
    for path in [&previous_blocks, &updated_blocks] {
        assert!(
            !outside_parent
                .join(path.file_name().expect("block-store name"))
                .exists(),
            "descriptor-relative recovery must not publish through the replacement symlink"
        );
    }
    assert!(
        displaced_parent
            .join(
                previous_blocks
                    .file_name()
                    .expect("previous block-store name")
            )
            .is_dir(),
        "the authenticated parent handle must receive the recovered block store"
    );
    assert!(
        !displaced_parent
            .join(
                updated_blocks
                    .file_name()
                    .expect("updated block-store name")
            )
            .exists(),
        "the descriptor-relative rename must consume its authenticated source"
    );
    fs::remove_file(blocks_parent).expect("remove substituted parent symlink");
    fs::rename(&displaced_parent, blocks_parent).expect("restore authenticated block-store parent");
    assert_eq!(
        kura.read_lane_geometry_journal()
            .expect("unchanged geometry journal")
            .records[0]
            .phase,
        LaneGeometryPhase::FilesApplied,
        "failed recovery must not advance the durable journal phase"
    );
}
#[test]
fn bootstrap_geometry_moves_never_clobber_targets_materialized_after_preflight() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    fs::create_dir_all(&root).expect("create bootstrap Kura root");
    let source_blocks = root.join("bootstrap-collision/source-blocks");
    let target_blocks = root.join("bootstrap-collision/target-blocks");
    fs::create_dir_all(&source_blocks).expect("seed bootstrap source blocks");
    fs::write(source_blocks.join("sentinel"), b"source-blocks")
        .expect("seed bootstrap source sentinel");
    *GEOMETRY_MOVE_TARGET_COLLISION
        .lock()
        .expect("geometry collision hook lock") = Some(target_blocks.clone());
    bootstrap_move_geometry_path(&root, &source_blocks, &target_blocks, true)
        .expect_err("descriptor-relative bootstrap rename must reject the racing target");
    assert_eq!(
        fs::read(source_blocks.join("sentinel")).expect("source sentinel remains"),
        b"source-blocks"
    );
    assert!(
        fs::read_dir(&target_blocks)
            .expect("read injected target")
            .next()
            .is_none(),
        "the racing directory must remain empty and must not receive source contents"
    );
    let source_merge = root.join("bootstrap-collision/source-merge.log");
    let target_merge = root.join("bootstrap-collision/target-merge.log");
    fs::write(&source_merge, b"source-merge").expect("seed bootstrap source merge");
    *GEOMETRY_MOVE_TARGET_COLLISION
        .lock()
        .expect("geometry collision hook lock") = Some(target_merge.clone());
    bootstrap_move_geometry_path(&root, &source_merge, &target_merge, false)
        .expect_err("descriptor-relative bootstrap rename must reject the racing file");
    assert_eq!(
        fs::read(source_merge).expect("source merge remains"),
        b"source-merge"
    );
    assert_eq!(
        fs::read(target_merge).expect("racing merge target remains"),
        b"injected-no-clobber-target"
    );
}
#[cfg(unix)]
#[test]
fn bootstrap_geometry_move_rejects_a_symlinked_target_parent() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&root).expect("create bootstrap Kura root");
    fs::create_dir_all(&outside).expect("create outside target");
    let source = root.join("source.log");
    fs::write(&source, b"canonical-source").expect("seed canonical source");
    symlink(&outside, root.join("escaped-parent")).expect("plant target-parent symlink");
    let target = root.join("escaped-parent/target.log");
    bootstrap_move_geometry_path(&root, &source, &target, false)
        .expect_err("bootstrap move must reject an ancestor symlink");
    assert_eq!(
        fs::read(source).expect("canonical source remains"),
        b"canonical-source"
    );
    assert!(
        !outside.join("target.log").exists(),
        "bootstrap recovery must not publish outside the authenticated Kura root"
    );
}
#[test]
fn mutable_pair_move_supports_a_stationary_block_path_and_later_merge_appends() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    let blocks = root.join("pair-move/live-blocks");
    let old_merge = root.join("pair-move/old-merge.log");
    let new_merge = root.join("pair-move/new-merge.log");
    let binding = LaneGeometryBinding {
        lane_id: LaneId::new(7),
        incarnation: Hash::new(b"stationary-block-pair"),
        activation_height: 1,
        blocks_path: kura
            .relative_geometry_path(&blocks)
            .expect("relative block path"),
        merge_path: kura
            .relative_geometry_path(&old_merge)
            .expect("relative old merge path"),
    };
    kura.provision_geometry_binding(&binding)
        .expect("provision movable geometry pair");
    fs::write(&old_merge, b"before-move").expect("seed merge bytes");
    kura.move_geometry_binding_pair(
        &binding,
        &blocks,
        &old_merge,
        &blocks,
        &new_merge,
        GeometryPairTargetKind::MutableLive,
    )
    .expect("move only the merge half under a stationary block path");
    assert!(!old_merge.exists());
    assert_eq!(fs::read(&new_merge).expect("moved merge"), b"before-move");
    let marker = kura
        .read_lane_marker(&blocks.join(MARKER_FILE_NAME))
        .expect("read completed live marker");
    assert!(marker.move_target_blocks.is_none());
    assert!(marker.move_target_merge.is_none());
    fs::write(&new_merge, b"before-move-and-legitimate-append").expect("append live merge history");
    kura.move_geometry_binding_pair(
        &binding,
        &blocks,
        &old_merge,
        &blocks,
        &new_merge,
        GeometryPairTargetKind::MutableLive,
    )
    .expect("a completed live move remains idempotent after legitimate merge growth");
    assert_eq!(
        fs::read(&new_merge).expect("appended merge retained"),
        b"before-move-and-legitimate-append"
    );
}
