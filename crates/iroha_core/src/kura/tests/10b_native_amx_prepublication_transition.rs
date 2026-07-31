    #[test]
    #[allow(clippy::too_many_lines)]
    fn native_amx_prepublication_transition_preflight_is_read_only_and_exact() {
        fn snapshot(directory: &Path) -> BTreeMap<String, Vec<u8>> {
            fs::read_dir(directory)
                .expect("read Native preflight evidence directory")
                .map(|entry| {
                    let entry = entry.expect("read Native preflight evidence entry");
                    (
                        entry
                            .file_name()
                            .into_string()
                            .expect("Native preflight evidence name is UTF-8"),
                        fs::read(entry.path()).expect("read Native preflight evidence bytes"),
                    )
                })
                .collect()
        }

        let temp_dir = TempDir::new().expect("Native transition-preflight Kura directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default())
            .expect("initialize Native transition-preflight Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native transition-preflight lane entry");
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
        let manifest_path = |height| {
            Kura::native_amx_application_manifest_path_for_entry(
                &entry,
                &kura.store_root,
                height,
            )
        };
        let receipt_path = |height| {
            Kura::native_amx_participant_receipt_path_for_entry(
                &entry,
                &kura.store_root,
                height,
            )
        };
        let namespace = kura
            .native_amx_evidence_namespace_for_entry(&entry)
            .expect("bind Native transition-preflight namespace");
        let manifest_one = kura
            .read_native_amx_participant_application_manifest_from_paths_locked(
                &entry,
                1,
                &manifest_path(1),
                &namespace,
            )
            .expect("read Native predecessor manifest");
        let manifest_two = kura
            .read_native_amx_participant_application_manifest_from_paths_locked(
                &entry,
                2,
                &manifest_path(2),
                &namespace,
            )
            .expect("read Native incoming manifest");
        drop(namespace);
        {
            let _prune_guard = kura.prune_lock.lock();
            kura.write_native_amx_participant_receipt_latest_index_under_publication_guard(
                &receipts[0],
                &manifest_one,
                true,
            )
            .expect("install exact predecessor latest pointer");
        }
        fs::remove_file(manifest_path(2)).expect("remove incoming manifest before preflight");
        fs::remove_file(receipt_path(2)).expect("remove incoming receipt before preflight");
        let directory = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root));

        let _prune_guard = kura.prune_lock.lock();
        let exact_before = snapshot(&directory);
        let exact = kura
            .preflight_native_amx_participant_application_route_under_publication_guard(
                &manifest_one,
                &receipts[0],
            )
            .expect("exact latest identity is an idempotent retry");
        assert_eq!(
            exact.current,
            Some(NativeAmxParticipantReceiptLatestIndexV2::from_receipt(
                &receipts[0]
            ))
        );
        assert_eq!(snapshot(&directory), exact_before);

        let mut wrong_predecessor_manifest = manifest_two.clone();
        let mut wrong_predecessor_receipt = receipts[1].clone();
        wrong_predecessor_receipt.application_block_height = 2;
        wrong_predecessor_receipt
            .participant_proposal
            .descriptor
            .proposal_height = 2;
        wrong_predecessor_receipt
            .participant_proposal
            .descriptor
            .previous_lane_block_descriptor_hash =
            Some(Hash::new(b"wrong Native predecessor descriptor"));
        wrong_predecessor_receipt
            .participant_proposal
            .descriptor
            .descriptor_hash = wrong_predecessor_receipt
            .participant_proposal
            .descriptor
            .computed_descriptor_hash();
        wrong_predecessor_receipt.participant_proposal.proposal_hash =
            wrong_predecessor_receipt
                .participant_proposal
                .computed_proposal_hash();
        wrong_predecessor_manifest.leaf.application_block_height = 2;
        wrong_predecessor_manifest.leaf.predecessor_descriptor_hash =
            wrong_predecessor_receipt
                .participant_proposal
                .descriptor
                .previous_lane_block_descriptor_hash;
        wrong_predecessor_manifest.leaf.descriptor_hash = wrong_predecessor_receipt
            .participant_proposal
            .descriptor
            .descriptor_hash;
        wrong_predecessor_manifest.leaf.proposal_hash =
            wrong_predecessor_receipt.participant_proposal.proposal_hash;
        wrong_predecessor_receipt.manifest_artifact_hash =
            HashOf::new(&wrong_predecessor_manifest);
        let before = snapshot(&directory);
        let error = kura
            .preflight_native_amx_participant_application_route_under_publication_guard(
                &wrong_predecessor_manifest,
                &wrong_predecessor_receipt,
            )
            .expect_err("wrong durable predecessor must fail prepublication");
        assert!(
            error.to_string().contains("predecessor descriptor"),
            "unexpected wrong-predecessor error: {error}"
        );
        assert_eq!(snapshot(&directory), before);

        let mut gap_manifest = wrong_predecessor_manifest.clone();
        let mut gap_receipt = wrong_predecessor_receipt.clone();
        gap_receipt
            .participant_proposal
            .descriptor
            .lane_block_height = 3;
        gap_receipt
            .participant_proposal
            .descriptor
            .previous_lane_block_height = 1;
        gap_receipt
            .participant_proposal
            .descriptor
            .descriptor_hash = gap_receipt
            .participant_proposal
            .descriptor
            .computed_descriptor_hash();
        gap_receipt.participant_proposal.proposal_hash =
            gap_receipt.participant_proposal.computed_proposal_hash();
        gap_manifest.leaf.participant_height = 3;
        gap_manifest.leaf.predecessor_height = 1;
        gap_manifest.leaf.descriptor_hash =
            gap_receipt.participant_proposal.descriptor.descriptor_hash;
        gap_manifest.leaf.proposal_hash = gap_receipt.participant_proposal.proposal_hash;
        gap_receipt.manifest_artifact_hash = HashOf::new(&gap_manifest);
        let before = snapshot(&directory);
        let error = kura
            .preflight_native_amx_participant_application_route_under_publication_guard(
                &gap_manifest,
                &gap_receipt,
            )
            .expect_err("participant-height gap must fail prepublication");
        assert!(
            error.to_string().contains("non-contiguous"),
            "unexpected participant-gap error: {error}"
        );
        assert_eq!(snapshot(&directory), before);

        let mut regressed_manifest = manifest_one.clone();
        let mut regressed_receipt = receipts[0].clone();
        regressed_receipt
            .participant_proposal
            .descriptor
            .lane_block_height = 0;
        regressed_receipt
            .participant_proposal
            .descriptor
            .descriptor_hash = regressed_receipt
            .participant_proposal
            .descriptor
            .computed_descriptor_hash();
        regressed_receipt.participant_proposal.proposal_hash =
            regressed_receipt.participant_proposal.computed_proposal_hash();
        regressed_manifest.leaf.participant_height = 0;
        regressed_manifest.leaf.descriptor_hash = regressed_receipt
            .participant_proposal
            .descriptor
            .descriptor_hash;
        regressed_manifest.leaf.proposal_hash =
            regressed_receipt.participant_proposal.proposal_hash;
        regressed_receipt.manifest_artifact_hash = HashOf::new(&regressed_manifest);
        let before = snapshot(&directory);
        let error = kura
            .preflight_native_amx_participant_application_route_under_publication_guard(
                &regressed_manifest,
                &regressed_receipt,
            )
            .expect_err("regressed participant height must fail prepublication");
        assert!(
            error.to_string().contains("regress"),
            "unexpected regressed-height error: {error}"
        );
        assert_eq!(snapshot(&directory), before);

        let mut conflict_receipt = receipts[0].clone();
        conflict_receipt.participant_proposal.proposal_hash =
            Hash::new(b"same-height Native proposal conflict");
        let mut conflict_manifest = manifest_one.clone();
        conflict_manifest.leaf.proposal_hash =
            conflict_receipt.participant_proposal.proposal_hash;
        conflict_receipt.manifest_artifact_hash = HashOf::new(&conflict_manifest);
        let before = snapshot(&directory);
        let error = kura
            .preflight_native_amx_participant_application_route_under_publication_guard(
                &conflict_manifest,
                &conflict_receipt,
            )
            .expect_err("same-height identity conflict must fail prepublication");
        assert!(
            error.to_string().contains("same-height"),
            "unexpected same-height conflict error: {error}"
        );
        assert_eq!(snapshot(&directory), before);
    }

    #[test]
    fn native_amx_retained_history_requires_one_exact_contiguous_descriptor_chain() {
        let temp_dir = TempDir::new().expect("Native retained-chain Kura directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default())
            .expect("initialize Native retained-chain Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native retained-chain lane entry");
        install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        let namespace = kura
            .native_amx_evidence_namespace_for_entry(&entry)
            .expect("bind Native retained-chain namespace");
        let inventory = kura
            .inventory_native_amx_evidence_files_locked(&namespace, false)
            .expect("inventory complete Native retained chain");
        let manifests = inventory
            .manifests
            .iter()
            .map(|(height, file)| {
                (
                    *height,
                    kura.decode_native_amx_manifest_file_locked(&entry, &namespace, file)
                        .expect("decode Native retained-chain manifest"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let receipts = inventory
            .receipts
            .iter()
            .map(|(height, file)| {
                (
                    *height,
                    kura.decode_native_amx_receipt_file_locked(&entry, &namespace, file)
                        .expect("decode Native retained-chain receipt"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        Kura::validate_native_amx_retained_history_continuity(&manifests, &receipts, false)
            .expect("complete adjacent Native retained chain");

        let mut punctured_manifests = manifests.clone();
        let mut punctured_receipts = receipts.clone();
        punctured_manifests.remove(&2);
        punctured_receipts.remove(&2);
        let error = Kura::validate_native_amx_retained_history_continuity(
            &punctured_manifests,
            &punctured_receipts,
            false,
        )
        .expect_err("symmetric middle-pair deletion must puncture the retained suffix");
        assert!(
            error.contains("contiguous"),
            "unexpected Native retained puncture error: {error}"
        );

        let mut drifted_manifests = manifests;
        let mut highest_partial_receipts = receipts;
        highest_partial_receipts.remove(&3);
        drifted_manifests
            .get_mut(&3)
            .expect("highest Native manifest exists")
            .leaf
            .predecessor_descriptor_hash =
            Some(Hash::new(b"drifted retained Native predecessor"));
        let error = Kura::validate_native_amx_retained_history_continuity(
            &drifted_manifests,
            &highest_partial_receipts,
            true,
        )
        .expect_err("adjacent retained predecessor drift must fail closed");
        assert!(
            error.contains("predecessor identity"),
            "unexpected Native retained predecessor error: {error}"
        );
    }

    #[test]
    fn native_amx_startup_rejects_symmetric_middle_pair_deletion() {
        let temp_dir = TempDir::new().expect("Native punctured-startup Kura directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default())
            .expect("initialize Native punctured-startup Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native punctured-startup lane entry");
        install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
        let manifest = Kura::native_amx_application_manifest_path_for_entry(
            &entry,
            &kura.store_root,
            2,
        );
        let receipt =
            Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 2);
        fs::remove_file(&manifest).expect("delete middle Native manifest");
        fs::remove_file(&receipt).expect("delete middle Native receipt");
        sync_dir(
            manifest
                .parent()
                .expect("Native punctured-startup evidence directory"),
        )
        .expect("sync punctured Native evidence directory");
        drop(kura);

        let error = match Kura::new(&config, &RuntimeLaneConfig::default()) {
            Ok(_) => panic!("startup must reject a symmetric middle Native pair deletion"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("contiguous"),
            "unexpected punctured Native startup error: {error}"
        );
        assert!(
            !manifest.exists() && !receipt.exists(),
            "fail-closed startup must not fabricate the deleted middle pair"
        );
    }

    #[test]
    fn native_amx_startup_rejects_authenticated_retained_predecessor_drift() {
        let temp_dir = TempDir::new().expect("Native drifted-startup Kura directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default())
            .expect("initialize Native drifted-startup Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native drifted-startup lane entry");
        install_native_amx_evidence_fixture_heights_with_predecessor_drift(
            &kura,
            &entry,
            &[1, 2, 3],
            Some(3),
        );
        drop(kura);

        let error = match Kura::new(&config, &RuntimeLaneConfig::default()) {
            Ok(_) => panic!("startup must reject authenticated retained predecessor drift"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("predecessor identity"),
            "unexpected drifted Native startup error: {error}"
        );
    }

    #[test]
    fn native_amx_prune_intent_rejects_middle_pair_puncture() {
        let temp_dir = TempDir::new().expect("Native punctured-prune Kura directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default())
            .expect("initialize Native punctured-prune Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native punctured-prune lane entry");
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
        let manifest = Kura::native_amx_application_manifest_path_for_entry(
            &entry,
            &kura.store_root,
            2,
        );
        let receipt =
            Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 2);
        let intent = NativeAmxEvidencePruneIntentV1 {
            version: NativeAmxEvidencePruneIntentV1::VERSION,
            lane_id: entry.lane_id,
            dataspace_id: entry.dataspace_id,
            lane_incarnation: receipts[2]
                .participant_proposal
                .descriptor
                .lane_incarnation,
            entries: vec![
                NativeAmxEvidencePruneEntryV1 {
                    kind: NativeAmxEvidencePruneIntentV1::MANIFEST_KIND,
                    participant_height: 2,
                    artifact_hash: Hash::new(
                        fs::read(&manifest).expect("read middle Native manifest"),
                    ),
                },
                NativeAmxEvidencePruneEntryV1 {
                    kind: NativeAmxEvidencePruneIntentV1::RECEIPT_KIND,
                    participant_height: 2,
                    artifact_hash: Hash::new(
                        fs::read(&receipt).expect("read middle Native receipt"),
                    ),
                },
            ],
        };
        let _prune_guard = kura.prune_lock.lock();
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        let namespace = kura
            .native_amx_evidence_namespace_for_entry(&entry)
            .expect("bind Native punctured-prune namespace");
        let error = kura
            .validate_native_amx_evidence_prune_intent_locked(&entry, &namespace, &intent)
            .expect_err("a prune intent must not remove a retained middle pair");
        assert!(
            error.to_string().contains("oldest contiguous prefix"),
            "unexpected punctured Native prune-intent error: {error}"
        );
        assert!(
            manifest.exists() && receipt.exists(),
            "failed prune-intent validation must leave the middle pair untouched"
        );
    }
