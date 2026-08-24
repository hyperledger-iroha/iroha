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
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("initialize Native transition-preflight Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("Native transition-preflight lane entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
    let manifest_path = |height| {
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, height)
    };
    let receipt_path = |height| {
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, height)
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
    wrong_predecessor_receipt.participant_proposal.proposal_hash = wrong_predecessor_receipt
        .participant_proposal
        .computed_proposal_hash();
    wrong_predecessor_manifest.leaf.application_block_height = 2;
    wrong_predecessor_manifest.leaf.predecessor_descriptor_hash = wrong_predecessor_receipt
        .participant_proposal
        .descriptor
        .previous_lane_block_descriptor_hash;
    wrong_predecessor_manifest.leaf.descriptor_hash = wrong_predecessor_receipt
        .participant_proposal
        .descriptor
        .descriptor_hash;
    wrong_predecessor_manifest.leaf.proposal_hash =
        wrong_predecessor_receipt.participant_proposal.proposal_hash;
    wrong_predecessor_receipt.manifest_artifact_hash = HashOf::new(&wrong_predecessor_manifest);
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
    gap_receipt.participant_proposal.descriptor.descriptor_hash = gap_receipt
        .participant_proposal
        .descriptor
        .computed_descriptor_hash();
    gap_receipt.participant_proposal.proposal_hash =
        gap_receipt.participant_proposal.computed_proposal_hash();
    gap_manifest.leaf.participant_height = 3;
    gap_manifest.leaf.predecessor_height = 1;
    gap_manifest.leaf.descriptor_hash = gap_receipt.participant_proposal.descriptor.descriptor_hash;
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
    regressed_receipt.participant_proposal.proposal_hash = regressed_receipt
        .participant_proposal
        .computed_proposal_hash();
    regressed_manifest.leaf.participant_height = 0;
    regressed_manifest.leaf.descriptor_hash = regressed_receipt
        .participant_proposal
        .descriptor
        .descriptor_hash;
    regressed_manifest.leaf.proposal_hash = regressed_receipt.participant_proposal.proposal_hash;
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
    conflict_manifest.leaf.proposal_hash = conflict_receipt.participant_proposal.proposal_hash;
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
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
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
        .predecessor_descriptor_hash = Some(Hash::new(b"drifted retained Native predecessor"));
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
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("initialize Native punctured-startup Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("Native punctured-startup lane entry");
    install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
    let manifest =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 2);
    let receipt = Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 2);
    fs::remove_file(&manifest).expect("delete middle Native manifest");
    fs::remove_file(&receipt).expect("delete middle Native receipt");
    sync_dir(
        manifest
            .parent()
            .expect("Native punctured-startup evidence directory"),
    )
    .expect("sync punctured Native evidence directory");
    drop(kura);
    let error = match Kura::open_test_kura_with_configured_lane_config(
        &config,
        &RuntimeLaneConfig::default(),
    ) {
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
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
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
    let error = match Kura::open_test_kura_with_configured_lane_config(
        &config,
        &RuntimeLaneConfig::default(),
    ) {
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
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("initialize Native punctured-prune Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("Native punctured-prune lane entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
    let manifest =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 2);
    let receipt = Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 2);
    let latest = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipts[2]);
    let intent = NativeAmxEvidencePruneIntentV2 {
        version: NativeAmxEvidencePruneIntentV2::VERSION,
        lane_id: entry.lane_id,
        dataspace_id: entry.dataspace_id,
        lane_incarnation: latest.lane_incarnation,
        protected_latest: NativeAmxEvidencePruneProtectedLatestV2 {
            identity: latest,
            receipt_artifact_hash: HashOf::new(&receipts[2]),
        },
        entries: vec![
            NativeAmxEvidencePruneEntryV2 {
                kind: NativeAmxEvidencePruneIntentV2::MANIFEST_KIND,
                participant_height: 2,
                artifact_hash: Hash::new(fs::read(&manifest).expect("read middle Native manifest")),
            },
            NativeAmxEvidencePruneEntryV2 {
                kind: NativeAmxEvidencePruneIntentV2::RECEIPT_KIND,
                participant_height: 2,
                artifact_hash: Hash::new(fs::read(&receipt).expect("read middle Native receipt")),
            },
        ],
    };
    let _prune_guard = kura.prune_lock.lock();
    let _canonical_chain_guard = kura.canonical_chain_lock.lock();
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
fn native_amx_prune_special_paths(kura: &Kura, entry: &LaneConfigEntry) -> (PathBuf, PathBuf) {
    let directory = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root));
    (
        directory.join(NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE),
        directory.join(NATIVE_AMX_EVIDENCE_PRUNE_INTENT_TEMP_FILE),
    )
}
fn native_amx_prune_evidence_snapshot(
    kura: &Kura,
    entry: &LaneConfigEntry,
    heights: &[u64],
) -> BTreeMap<PathBuf, Vec<u8>> {
    heights
        .iter()
        .flat_map(|height| {
            [
                Kura::native_amx_application_manifest_path_for_entry(
                    entry,
                    &kura.store_root,
                    *height,
                ),
                Kura::native_amx_participant_receipt_path_for_entry(
                    entry,
                    &kura.store_root,
                    *height,
                ),
            ]
        })
        .map(|path| {
            let bytes = fs::read(&path).expect("snapshot Native prune evidence");
            (path, bytes)
        })
        .collect()
}
fn assert_native_amx_prune_evidence_snapshot(expected: &BTreeMap<PathBuf, Vec<u8>>, context: &str) {
    for (path, bytes) in expected {
        let actual = fs::read(path).unwrap_or_else(|error| {
            panic!(
                "read {context} Native prune evidence {}: {error}",
                path.display()
            )
        });
        assert_eq!(
            actual.as_slice(),
            bytes.as_slice(),
            "{context} changed Native prune evidence {}",
            path.display()
        );
    }
}
#[test]
fn native_amx_prune_special_files_reject_bounded_payload_damage_without_unlinking() {
    for special in ["stable", "temporary"] {
        for damage in ["empty", "truncated", "malformed", "oversized"] {
            let temp_dir = TempDir::new().expect("Native prune special-file directory");
            let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
            let lane_config = RuntimeLaneConfig::default();
            let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
                .expect("initialize Native prune special-file Kura");
            let entry = kura
                .lane_storage_entry(LaneId::SINGLE)
                .expect("Native prune special-file lane entry");
            let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
            let intent = native_amx_prune_intent_for_test(&kura, &entry, &receipts[1], &[1]);
            let encoded = norito::encode_canonical(&intent).expect("encode Native prune V2 intent");
            let (stable_path, temp_path) = native_amx_prune_special_paths(&kura, &entry);
            let path = if special == "stable" {
                stable_path
            } else {
                temp_path
            };
            let evidence = native_amx_prune_evidence_snapshot(&kura, &entry, &[1, 2]);
            match damage {
                "empty" => write_synced_native_amx_test_file(&path, &[]),
                "truncated" => write_synced_native_amx_test_file(
                    &path,
                    encoded
                        .get(..encoded.len().saturating_sub(1))
                        .expect("truncate non-empty Native prune intent"),
                ),
                "malformed" => write_synced_native_amx_test_file(&path, &[0xA5]),
                "oversized" => {
                    fs::File::create(&path)
                        .expect("create oversized Native prune special file")
                        .set_len(
                            u64::try_from(kura.native_amx_evidence_prune_intent_max_bytes())
                                .expect("Native prune maximum fits u64")
                                .checked_add(1)
                                .expect("Native prune oversize length"),
                        )
                        .expect("size oversized Native prune special file");
                    std::fs::File::open(&path)
                        .expect("open oversized Native prune special file")
                        .sync_all()
                        .expect("sync oversized Native prune special file");
                    sync_dir(path.parent().expect("Native prune special-file directory"))
                        .expect("sync oversized Native prune special-file directory");
                }
                _ => unreachable!("fixed Native prune payload damage matrix"),
            }
            let forensic_bytes =
                fs::read(&path).expect("snapshot damaged Native prune special bytes");
            let forensic_hash = Hash::new(&forensic_bytes);
            let forensic_metadata =
                fs::symlink_metadata(&path).expect("inspect damaged Native prune special file");
            let forensic_modified = forensic_metadata.modified().ok();
            #[cfg(unix)]
            let forensic_object = {
                use std::os::unix::fs::MetadataExt as _;
                (
                    forensic_metadata.dev(),
                    forensic_metadata.ino(),
                    forensic_metadata.nlink(),
                )
            };
            drop(kura);
            let error =
                match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
                    Ok(_) => panic!("{special} {damage} Native prune file must fail startup"),
                    Err(error) => error,
                };
            assert!(
                path.exists(),
                "{special} {damage} Native prune file must remain for forensics: {error}"
            );
            let retained_bytes =
                fs::read(&path).expect("read retained damaged Native prune special bytes");
            assert_eq!(retained_bytes, forensic_bytes);
            assert_eq!(Hash::new(&retained_bytes), forensic_hash);
            let retained_metadata =
                fs::symlink_metadata(&path).expect("reinspect damaged Native prune special file");
            assert_eq!(
                retained_metadata.len(),
                forensic_metadata.len(),
                "{special} {damage} Native prune file changed during failed startup"
            );
            assert_eq!(retained_metadata.modified().ok(), forensic_modified);
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt as _;
                assert_eq!(
                    (
                        retained_metadata.dev(),
                        retained_metadata.ino(),
                        retained_metadata.nlink(),
                    ),
                    forensic_object,
                    "{special} {damage} Native prune forensic object changed"
                );
            }
            assert_native_amx_prune_evidence_snapshot(&evidence, &format!("{special} {damage}"));
        }
    }
}
#[test]
#[allow(clippy::too_many_lines)]
fn native_amx_prune_intent_v2_rejects_every_route_and_entry_geometry_mutation() {
    let temp_dir = TempDir::new().expect("Native prune semantic-matrix directory");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.lane_history_retention = NonZeroUsize::new(2).expect("two-pair Native prune retention");
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("initialize Native prune semantic-matrix Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("Native prune semantic-matrix lane entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
    let base = native_amx_prune_intent_for_test(&kura, &entry, &receipts[2], &[1]);
    let evidence = native_amx_prune_evidence_snapshot(&kura, &entry, &[1, 2, 3]);
    let mut mutations = Vec::new();
    let mut mutation = base.clone();
    mutation.version = NativeAmxEvidencePruneIntentV2::VERSION.saturating_sub(1);
    mutations.push(("version", mutation));
    let mut mutation = base.clone();
    mutation.lane_id = LaneId::new(entry.lane_id.as_u32().saturating_add(1));
    mutations.push(("lane", mutation));
    let mut mutation = base.clone();
    mutation.dataspace_id = DataSpaceId::new(entry.dataspace_id.as_u64().saturating_add(1));
    mutations.push(("dataspace", mutation));
    let mut mutation = base.clone();
    mutation.lane_incarnation = Hash::new(b"stale Native prune incarnation");
    mutations.push(("incarnation", mutation));
    let mut mutation = base.clone();
    mutation.protected_latest.identity.lane_id =
        LaneId::new(entry.lane_id.as_u32().saturating_add(1));
    mutations.push(("protected route", mutation));
    let mut mutation = base.clone();
    mutation.protected_latest.identity.lane_incarnation =
        Hash::new(b"stale protected Native prune incarnation");
    mutations.push(("protected incarnation", mutation));
    let mut mutation = base.clone();
    mutation.protected_latest.receipt_artifact_hash =
        HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]));
    mutations.push(("protected receipt hash", mutation));
    let mut mutation = base.clone();
    mutation.entries.clear();
    mutations.push(("empty entries", mutation));
    let mut mutation = base.clone();
    mutation.entries[0].kind = u8::MAX;
    mutations.push(("unknown kind", mutation));
    let mut mutation = base.clone();
    mutation.entries[0].participant_height = 0;
    mutations.push(("zero height", mutation));
    let mut mutation = base.clone();
    mutation.entries[0].artifact_hash = Hash::prehashed([0; Hash::LENGTH]);
    mutations.push(("zero artifact hash", mutation));
    let mut mutation = base.clone();
    mutation.entries.reverse();
    mutations.push(("entry order", mutation));
    let mut mutation = base.clone();
    let duplicate = mutation.entries[0];
    mutation.entries.insert(1, duplicate);
    mutations.push(("duplicate entry", mutation));
    let mut mutation = base.clone();
    mutation.entries.pop();
    mutations.push(("incomplete pair", mutation));
    let mut mutation = base.clone();
    mutation.entries[0].artifact_hash = Hash::new(b"wrong Native prune artifact hash");
    mutations.push(("artifact hash", mutation));
    mutations.push((
        "protected target",
        native_amx_prune_intent_for_test(&kura, &entry, &receipts[2], &[3]),
    ));
    mutations.push((
        "non-prefix target",
        native_amx_prune_intent_for_test(&kura, &entry, &receipts[2], &[2]),
    ));
    let mut mutation = base.clone();
    let max_entries =
        Kura::native_amx_evidence_prune_intent_max_entries(config.lane_history_retention)
            .expect("derive Native prune semantic entry bound");
    let repeated = mutation.entries[0];
    mutation.entries.resize(
        max_entries
            .checked_add(1)
            .expect("Native prune over-limit length"),
        repeated,
    );
    mutations.push(("entry bound", mutation));
    let _prune_guard = kura.prune_lock.lock();
    let _canonical_guard = kura.canonical_chain_lock.lock();
    let _geometry_guard = kura.lane_geometry_lock.lock();
    let _sidecar_guard = kura.sidecar_lock.lock();
    let namespace = kura
        .native_amx_evidence_namespace_for_entry(&entry)
        .expect("bind Native prune semantic-matrix namespace");
    for (label, mutation) in mutations {
        if kura
            .validate_native_amx_evidence_prune_intent_locked(&entry, &namespace, &mutation)
            .is_ok()
        {
            panic!("{label} mutation unexpectedly passed");
        }
        assert_native_amx_prune_evidence_snapshot(&evidence, label);
    }
}
#[test]
fn native_amx_prune_stable_and_temporary_conflict_preserves_both_and_all_evidence() {
    let temp_dir = TempDir::new().expect("Native prune stable/temp conflict directory");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.lane_history_retention =
        NonZeroUsize::new(2).expect("two-pair Native prune conflict retention");
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("initialize Native prune stable/temp conflict Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("Native prune stable/temp conflict lane entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
    let stable = native_amx_prune_intent_for_test(&kura, &entry, &receipts[2], &[1]);
    let temporary = native_amx_prune_intent_for_test(&kura, &entry, &receipts[2], &[1, 2]);
    let stable_bytes =
        norito::encode_canonical(&stable).expect("encode stable Native prune intent");
    let temporary_bytes =
        norito::encode_canonical(&temporary).expect("encode temporary Native prune intent");
    assert_ne!(stable_bytes, temporary_bytes);
    let (stable_path, temp_path) = native_amx_prune_special_paths(&kura, &entry);
    write_synced_native_amx_test_file(&stable_path, &stable_bytes);
    write_synced_native_amx_test_file(&temp_path, &temporary_bytes);
    let evidence = native_amx_prune_evidence_snapshot(&kura, &entry, &[1, 2, 3]);
    drop(kura);
    let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
        Ok(_) => panic!("conflicting stable/temp Native prune intents must fail startup"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("conflict"),
        "unexpected Native prune stable/temp conflict error: {error}"
    );
    assert_eq!(
        fs::read(&stable_path).expect("read retained stable intent"),
        stable_bytes
    );
    assert_eq!(
        fs::read(&temp_path).expect("read retained temporary intent"),
        temporary_bytes
    );
    assert_native_amx_prune_evidence_snapshot(&evidence, "stable/temp conflict");
}
#[test]
fn native_amx_prune_identical_stable_and_temporary_converge_idempotently() {
    let temp_dir = TempDir::new().expect("Native prune identical stable/temp directory");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.lane_history_retention =
        NonZeroUsize::new(1).expect("one-pair Native prune identical retention");
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("initialize Native prune identical stable/temp Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("Native prune identical stable/temp lane entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
    let intent = native_amx_prune_intent_for_test(&kura, &entry, &receipts[1], &[1]);
    let bytes = norito::encode_canonical(&intent).expect("encode identical Native prune intent");
    let (stable_path, temp_path) = native_amx_prune_special_paths(&kura, &entry);
    write_synced_native_amx_test_file(&stable_path, &bytes);
    write_synced_native_amx_test_file(&temp_path, &bytes);
    let protected = native_amx_prune_evidence_snapshot(&kura, &entry, &[2]);
    let removed = native_amx_prune_evidence_snapshot(&kura, &entry, &[1])
        .into_keys()
        .collect::<Vec<_>>();
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("recover identical stable/temp Native prune intents");
    assert!(!stable_path.exists() && !temp_path.exists());
    assert!(removed.iter().all(|path| !path.exists()));
    assert_native_amx_prune_evidence_snapshot(&protected, "identical stable/temp recovery");
    let exact_usage = reopened
        .kura_total_disk_usage_bytes()
        .expect("scan exact usage after identical Native prune recovery");
    assert_eq!(
        reopened
            .disk_usage_bytes()
            .expect("read cached usage after identical Native prune recovery"),
        exact_usage
    );
    drop(reopened);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("idempotently reopen identical Native prune recovery");
    assert_eq!(
        reopened
            .disk_usage_bytes()
            .expect("read cached usage after idempotent Native prune recovery"),
        exact_usage
    );
    assert_eq!(
        reopened
            .kura_total_disk_usage_bytes()
            .expect("rescan usage after idempotent Native prune recovery"),
        exact_usage
    );
    assert_native_amx_prune_evidence_snapshot(&protected, "idempotent prune recovery");
}
#[cfg(unix)]
#[test]
fn native_amx_prune_special_files_reject_symlinks_and_hardlinks_on_both_paths() {
    for special in ["stable", "temporary"] {
        for unsafe_kind in ["symlink", "hardlink"] {
            let temp_dir = TempDir::new().expect("Native prune unsafe-file directory");
            let backing_dir = TempDir::new().expect("Native prune unsafe backing directory");
            let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
            let lane_config = RuntimeLaneConfig::default();
            let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
                .expect("initialize Native prune unsafe-file Kura");
            let entry = kura
                .lane_storage_entry(LaneId::SINGLE)
                .expect("Native prune unsafe-file lane entry");
            let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
            let intent = native_amx_prune_intent_for_test(&kura, &entry, &receipts[1], &[1]);
            let bytes =
                norito::encode_canonical(&intent).expect("encode Native prune unsafe-file intent");
            let (stable_path, temp_path) = native_amx_prune_special_paths(&kura, &entry);
            let path = if special == "stable" {
                stable_path
            } else {
                temp_path
            };
            let backing = backing_dir
                .path()
                .join(format!("{special}-{unsafe_kind}.norito"));
            write_synced_native_amx_test_file(&backing, &bytes);
            match unsafe_kind {
                "symlink" => std::os::unix::fs::symlink(&backing, &path)
                    .expect("install Native prune special-file symlink"),
                "hardlink" => fs::hard_link(&backing, &path)
                    .expect("install Native prune special-file hardlink"),
                _ => unreachable!("fixed Native prune unsafe-file matrix"),
            }
            sync_dir(path.parent().expect("Native prune unsafe-file directory"))
                .expect("sync Native prune unsafe-file directory");
            let evidence = native_amx_prune_evidence_snapshot(&kura, &entry, &[1, 2]);
            drop(kura);
            let error =
                match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
                    Ok(_) => panic!("{special} Native prune {unsafe_kind} must fail startup"),
                    Err(error) => error,
                };
            assert!(
                fs::symlink_metadata(&path).is_ok(),
                "{special} Native prune {unsafe_kind} must remain for forensics: {error}"
            );
            assert_eq!(fs::read(&backing).expect("read unsafe backing file"), bytes);
            assert_eq!(
                fs::read(&path).expect("read retained unsafe Native prune special file"),
                bytes
            );
            let retained_metadata = fs::symlink_metadata(&path)
                .expect("inspect retained unsafe Native prune special file");
            match unsafe_kind {
                "symlink" => assert!(retained_metadata.file_type().is_symlink()),
                "hardlink" => {
                    use std::os::unix::fs::MetadataExt as _;
                    assert_eq!(retained_metadata.nlink(), 2);
                }
                _ => unreachable!("fixed Native prune unsafe-file matrix"),
            }
            assert_native_amx_prune_evidence_snapshot(
                &evidence,
                &format!("{special} {unsafe_kind}"),
            );
        }
    }
}
#[test]
fn native_amx_prune_rejects_legacy_and_unexpected_special_names_without_downgrade() {
    for hostile_name in [
        "native_amx_evidence_prune_intent_v1.norito",
        "native_amx_evidence_prune_intent_v1.norito.tmp",
        "native_amx_evidence_prune_intent_v2.norito.bak",
    ] {
        let temp_dir = TempDir::new().expect("Native prune legacy-name directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("initialize Native prune legacy-name Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native prune legacy-name lane entry");
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
        let intent = native_amx_prune_intent_for_test(&kura, &entry, &receipts[1], &[1]);
        let bytes =
            norito::encode_canonical(&intent).expect("encode Native prune legacy-name bytes");
        let directory = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root));
        let hostile_path = directory.join(hostile_name);
        write_synced_native_amx_test_file(&hostile_path, &bytes);
        let evidence = native_amx_prune_evidence_snapshot(&kura, &entry, &[1, 2]);
        drop(kura);
        let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
            Ok(_) => panic!("legacy or unexpected Native prune name must fail startup"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("unexpected") || error.to_string().contains("legacy"),
            "unexpected Native prune legacy-name error for {hostile_name}: {error}"
        );
        assert_eq!(
            fs::read(&hostile_path).expect("read retained legacy Native prune artifact"),
            bytes
        );
        assert_native_amx_prune_evidence_snapshot(&evidence, hostile_name);
    }
}
#[test]
fn native_amx_prune_rejects_legacy_name_before_consuming_valid_v2_intent() {
    let temp_dir = TempDir::new().expect("Native prune mixed legacy/V2 directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("initialize Native prune mixed legacy/V2 Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("Native prune mixed legacy/V2 lane entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
    let intent = native_amx_prune_intent_for_test(&kura, &entry, &receipts[1], &[1]);
    let bytes = norito::encode_canonical(&intent).expect("encode mixed Native V2 intent");
    let directory = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root));
    let legacy_path = directory.join("native_amx_evidence_prune_intent_v1.norito");
    let (stable_path, _) = native_amx_prune_special_paths(&kura, &entry);
    write_synced_native_amx_test_file(&stable_path, &bytes);
    write_synced_native_amx_test_file(&legacy_path, &bytes);
    let evidence = native_amx_prune_evidence_snapshot(&kura, &entry, &[1, 2]);
    drop(kura);
    let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
        Ok(_) => panic!("mixed legacy/V2 Native prune files must fail startup"),
        Err(error) => error,
    };
    assert!(legacy_path.exists() && stable_path.exists(), "{error}");
    assert_eq!(
        fs::read(&legacy_path).expect("read retained legacy intent"),
        bytes
    );
    assert_eq!(
        fs::read(&stable_path).expect("read retained V2 intent"),
        bytes
    );
    assert_native_amx_prune_evidence_snapshot(&evidence, "mixed legacy/V2");
}
#[test]
fn native_amx_prune_two_pair_partial_unlinks_recover_every_prefix_idempotently() {
    for deleted_prefix in 0..=4 {
        let temp_dir = TempDir::new().expect("Native prune partial-prefix directory");
        let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        config.lane_history_retention =
            NonZeroUsize::new(2).expect("two-pair Native prune partial-prefix retention");
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("initialize Native prune partial-prefix Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native prune partial-prefix lane entry");
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
        let intent = native_amx_prune_intent_for_test(&kura, &entry, &receipts[2], &[1, 2]);
        let bytes =
            norito::encode_canonical(&intent).expect("encode Native prune partial-prefix intent");
        let (stable_path, temp_path) = native_amx_prune_special_paths(&kura, &entry);
        write_synced_native_amx_test_file(&stable_path, &bytes);
        let removal_paths = [
            Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1),
            Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1),
            Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 2),
            Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 2),
        ];
        for path in removal_paths.iter().take(deleted_prefix) {
            fs::remove_file(path).expect("stage Native prune partial unlink");
        }
        sync_dir(
            stable_path
                .parent()
                .expect("Native prune partial-prefix directory"),
        )
        .expect("sync Native prune partial-prefix crash shape");
        let protected = native_amx_prune_evidence_snapshot(&kura, &entry, &[3]);
        drop(kura);
        let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .unwrap_or_else(|error| {
                panic!("recover Native prune prefix {deleted_prefix}: {error}")
            });
        assert!(removal_paths.iter().all(|path| !path.exists()));
        assert!(!stable_path.exists() && !temp_path.exists());
        assert_native_amx_prune_evidence_snapshot(
            &protected,
            &format!("partial prefix {deleted_prefix}"),
        );
        let exact_usage = reopened
            .kura_total_disk_usage_bytes()
            .expect("scan Native prune partial-prefix usage");
        assert_eq!(
            reopened
                .disk_usage_bytes()
                .expect("read Native prune partial-prefix cached usage"),
            exact_usage
        );
        drop(reopened);
        let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("idempotently reopen Native prune partial-prefix recovery");
        assert_eq!(
            reopened
                .kura_total_disk_usage_bytes()
                .expect("rescan idempotent Native prune partial-prefix usage"),
            exact_usage
        );
        assert_eq!(
            reopened
                .disk_usage_bytes()
                .expect("read idempotent Native prune partial-prefix cached usage"),
            exact_usage
        );
    }
}
#[test]
fn native_amx_prune_exact_object_removal_rejects_same_length_in_place_rewrites() {
    for rewritten in ["first-target", "stable-intent"] {
        let temp_dir = TempDir::new().expect("Native prune in-place rewrite directory");
        let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        config.lane_history_retention =
            NonZeroUsize::new(1).expect("one-pair Native prune rewrite retention");
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("initialize Native prune in-place rewrite Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native prune in-place rewrite lane entry");
        let _receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
        let protected = native_amx_prune_evidence_snapshot(&kura, &entry, &[2]);
        let rewrite = std::sync::Arc::new(std::sync::Mutex::new(None));
        let rewrite_for_hook = std::sync::Arc::clone(&rewrite);
        set_native_amx_prune_pre_unlink_hook_for_tests(
            if rewritten == "first-target" { 0 } else { 2 },
            move |path| {
                let original = fs::read(path).expect("read Native prune object before rewrite");
                let mut replacement = original.clone();
                let last = replacement
                    .last_mut()
                    .expect("Native prune object is non-empty");
                *last ^= 0x5A;
                write_synced_native_amx_test_file(path, &replacement);
                *rewrite_for_hook
                    .lock()
                    .expect("lock Native prune rewrite result") =
                    Some((path.to_path_buf(), original, replacement));
            },
        );
        let error = {
            let _prune_guard = kura.prune_lock.lock();
            let _canonical_guard = kura.canonical_chain_lock.lock();
            let _geometry_guard = kura.lane_geometry_lock.lock();
            let _sidecar_guard = kura.sidecar_lock.lock();
            let namespace = kura
                .native_amx_evidence_namespace_for_entry(&entry)
                .expect("bind Native prune in-place rewrite namespace");
            kura.prune_native_amx_evidence_pairs_locked(&entry, &namespace)
                .expect_err("same-length Native prune rewrite must fail before unlink")
        };
        assert!(
            error.to_string().contains("changed") || error.to_string().contains("exact-object"),
            "unexpected {rewritten} Native prune rewrite error: {error}"
        );
        let (path, original, replacement) = rewrite
            .lock()
            .expect("lock completed Native prune rewrite")
            .take()
            .expect("Native prune rewrite hook ran");
        assert_ne!(original, replacement);
        assert_eq!(
            fs::read(&path).expect("read retained rewritten Native prune object"),
            replacement,
            "rewritten {rewritten} object must remain for forensics"
        );
        assert_native_amx_prune_evidence_snapshot(&protected, rewritten);
    }
}
#[test]
fn native_amx_prune_protected_checkpoint_or_commit_semantic_drift_fails_before_unlink() {
    for damage in ["checkpoint-state", "commit-checkpoint"] {
        let temp_dir = TempDir::new().expect("Native prune metadata-drift directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("initialize Native prune metadata-drift Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native prune metadata-drift lane entry");
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
        let intent = native_amx_prune_intent_for_test(&kura, &entry, &receipts[1], &[1]);
        let intent_bytes =
            norito::encode_canonical(&intent).expect("encode Native prune metadata-drift intent");
        let (stable_path, _) = native_amx_prune_special_paths(&kura, &entry);
        write_synced_native_amx_test_file(&stable_path, &intent_bytes);
        let evidence = native_amx_prune_evidence_snapshot(&kura, &entry, &[1, 2]);
        let checkpoint = kura
            .wsv_checkpoint(1)
            .expect("read Native prune metadata-drift checkpoint")
            .expect("Native prune metadata-drift checkpoint exists");
        let finality = kura
            .v2_finality_artifact(1)
            .expect("read Native prune metadata-drift finality")
            .expect("Native prune metadata-drift finality exists");
        match damage {
            "checkpoint-state" => {
                kura.remove_wsv_checkpoint_without_binding_for_tests(1)
                    .expect("remove exact Native prune checkpoint");
                kura.store_wsv_checkpoint(
                    1,
                    checkpoint.block_hash,
                    Hash::new(b"drifted Native prune checkpoint state"),
                )
                .expect("store semantically drifted Native prune checkpoint");
            }
            "commit-checkpoint" => {
                let drifted = CommitManifest::new(
                    1,
                    checkpoint.block_hash,
                    None,
                    None,
                    Hash::new(b"drifted Native prune commit checkpoint"),
                    None,
                )
                .with_authenticated_v2_commit_authority(&finality);
                kura.overwrite_commit_manifest_without_binding_for_tests(&drifted)
                    .expect("overwrite semantically drifted Native prune commit manifest");
            }
            _ => unreachable!("fixed Native prune metadata drift matrix"),
        }
        drop(kura);
        let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
            Ok(_) => panic!("{damage} must fail Native prune startup authentication"),
            Err(error) => error,
        };
        assert!(
            stable_path.exists(),
            "{damage} removed forensic intent: {error}"
        );
        assert_eq!(
            fs::read(&stable_path).expect("read retained metadata-drift intent"),
            intent_bytes
        );
        assert_native_amx_prune_evidence_snapshot(&evidence, damage);
    }
}
#[test]
fn native_amx_configured_shared_two_family_budget_accepts_exact_boundaries_only() {
    let temp_dir = TempDir::new().expect("configured Native evidence budget directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let mut limits = SumeragiV2RuntimeLimits::default();
    limits.pending_control_sidecar_bytes = NonZeroUsize::new(V2_PENDING_CONTROL_SIDECAR_BYTES_MIN)
        .expect("configured Native evidence byte minimum");
    let (kura, _) = open_configured_kura_with_pending_limits(&config, &limits)
        .expect("open Kura with configured Native evidence budget");
    let shared_bytes = u64::try_from(V2_PENDING_CONTROL_SIDECAR_BYTES_MIN)
        .expect("configured Native evidence budget fits u64");
    assert_eq!(
        kura.native_amx_participant_evidence_file_bytes(),
        shared_bytes
    );
    assert_eq!(
        kura.native_amx_participant_evidence_startup_bytes()
            .expect("configured Native startup budget"),
        shared_bytes
            .checked_mul(2)
            .expect("configured Native transient headroom fits u64")
    );
    let manifest_bytes = shared_bytes / 2;
    let receipt_bytes = shared_bytes - manifest_bytes;
    assert!(kura.native_amx_participant_evidence_pair_fits_stable_bytes(
        usize::try_from(manifest_bytes).expect("manifest boundary fits usize"),
        usize::try_from(receipt_bytes).expect("receipt boundary fits usize"),
    ));
    assert!(
        !kura.native_amx_participant_evidence_pair_fits_stable_bytes(
            usize::try_from(manifest_bytes).expect("manifest boundary fits usize"),
            usize::try_from(receipt_bytes + 1).expect("receipt overflow fits usize"),
        )
    );
    assert_eq!(
        kura.native_amx_evidence_prune_intent_max_bytes(),
        Kura::native_amx_evidence_prune_intent_max_bytes_for_retention(
            config.lane_history_retention,
            V2_PENDING_CONTROL_SIDECAR_BYTES_MIN,
        )
        .expect("configured Native prune-intent budget")
    );
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("configured Native evidence lane entry");
    let manifest_path =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
    let receipt_path =
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
    fs::File::create(&manifest_path)
        .expect("create boundary Native manifest")
        .set_len(manifest_bytes)
        .expect("size boundary Native manifest");
    fs::File::create(&receipt_path)
        .expect("create boundary Native receipt")
        .set_len(receipt_bytes)
        .expect("size boundary Native receipt");
    let _geometry_guard = kura.lane_geometry_lock.lock();
    let _sidecar_guard = kura.sidecar_lock.lock();
    let namespace = kura
        .native_amx_evidence_namespace_for_entry(&entry)
        .expect("bind configured Native evidence namespace");
    kura.inventory_native_amx_evidence_files_locked(&namespace, false)
        .expect("exact shared stable manifest/receipt boundary is admissible");
    fs::File::options()
        .write(true)
        .open(&receipt_path)
        .expect("open boundary Native receipt")
        .set_len(receipt_bytes + 1)
        .expect("grow combined stable evidence one byte past configured boundary");
    let error = kura
        .inventory_native_amx_evidence_files_locked(&namespace, false)
        .expect_err("shared stable Native evidence aggregate overflow must fail");
    assert!(
        error.to_string().contains("shared aggregate byte bound"),
        "unexpected configured Native evidence boundary error: {error}"
    );
    fs::File::options()
        .write(true)
        .open(&receipt_path)
        .expect("restore boundary Native receipt")
        .set_len(receipt_bytes)
        .expect("restore exact stable Native boundary");
    let transient_manifest =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 2)
            .with_extension("norito.tmp");
    let transient_receipt =
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 2)
            .with_extension("norito.tmp");
    fs::File::create(&transient_manifest)
        .expect("create transient Native manifest")
        .set_len(manifest_bytes)
        .expect("size transient Native manifest");
    fs::File::create(&transient_receipt)
        .expect("create transient Native receipt")
        .set_len(receipt_bytes)
        .expect("size transient Native receipt");
    kura.inventory_native_amx_evidence_files_locked(&namespace, true)
        .expect("one exact shared pair of transient headroom is admissible");
    fs::File::options()
        .write(true)
        .open(&transient_receipt)
        .expect("open transient Native receipt")
        .set_len(receipt_bytes + 1)
        .expect("grow transient pair one byte past its shared headroom");
    let error = kura
        .inventory_native_amx_evidence_files_locked(&namespace, true)
        .expect_err("shared transient Native evidence aggregate overflow must fail");
    assert!(
        error.to_string().contains("shared aggregate byte bound"),
        "unexpected configured Native transient boundary error: {error}"
    );
}
#[test]
fn native_amx_prevote_byte_budget_is_exact_per_route_and_finality_width_stable() {
    let block = crate::sumeragi::exec::result_bearing_native_manifest_block_for_tests();
    let manifest =
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(&block)
            .expect("build multi-route Native byte-budget manifest");
    assert!(
        manifest.entries().len() > 1,
        "byte-budget fixture must cover independent routes"
    );
    let placeholder_artifacts = native_amx_participant_application_artifacts(
        &manifest,
        native_amx_participant_application_finality_placeholder_hash(),
    )
    .expect("build placeholder Native artifact pairs");
    let actual_finality_hash: HashOf<V2FinalityArtifact> =
        HashOf::from_untyped_unchecked(Hash::new(b"actual Native finality artifact hash fixture"));
    let actual_artifacts =
        native_amx_participant_application_artifacts(&manifest, actual_finality_hash)
            .expect("build actual-hash Native artifact pairs");
    let mut pair_lengths = Vec::with_capacity(placeholder_artifacts.len());
    for (placeholder, actual) in placeholder_artifacts.iter().zip(&actual_artifacts) {
        let placeholder_bytes =
            native_amx_participant_application_pair_framed_bytes(&placeholder.0, &placeholder.1)
                .expect("frame placeholder Native artifact pair");
        let actual_bytes =
            native_amx_participant_application_pair_framed_bytes(&actual.0, &actual.1)
                .expect("frame actual-hash Native artifact pair");
        assert_eq!(
            (placeholder_bytes.0.len(), placeholder_bytes.1.len()),
            (actual_bytes.0.len(), actual_bytes.1.len()),
            "typed finality hash substitution must preserve exact artifact lengths"
        );
        pair_lengths.push(
            placeholder_bytes
                .0
                .len()
                .checked_add(placeholder_bytes.1.len())
                .expect("fixture pair length fits usize"),
        );
    }
    let largest_pair = pair_lengths
        .iter()
        .copied()
        .max()
        .expect("multi-route fixture has artifact pairs");
    let all_pair_bytes = pair_lengths
        .iter()
        .try_fold(0_usize, |total, pair| total.checked_add(*pair))
        .expect("fixture carrier artifact lengths fit usize");
    assert!(
        all_pair_bytes > largest_pair,
        "the configured bound must apply independently per route, not to the carrier sum"
    );
    let temp_dir = TempDir::new().expect("Native pre-vote byte-budget directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("initialize Native pre-vote byte-budget Kura");
    Arc::get_mut(&mut kura)
        .expect("byte-budget fixture has one Kura owner")
        .pending_control_sidecar_limits
        .aggregate_bytes = largest_pair;
    kura.validate_native_amx_participant_application_evidence_byte_budget(&manifest, None)
        .expect("largest exact route pair must fit its exact configured bound");
    kura.validate_native_amx_participant_application_evidence_byte_budget(
        &manifest,
        Some(actual_finality_hash),
    )
    .expect("actual finality identity must preserve the pre-vote byte decision");
    Arc::get_mut(&mut kura)
        .expect("byte-budget fixture still has one Kura owner")
        .pending_control_sidecar_limits
        .aggregate_bytes = largest_pair
        .checked_sub(1)
        .expect("Native artifact pair is non-empty");
    let error = kura
        .validate_native_amx_participant_application_evidence_byte_budget(&manifest, None)
        .expect_err("one byte below the largest route pair must fail closed");
    assert!(
        matches!(
            &error,
            NativeAmxParticipantApplicationEvidenceByteBudgetError::Budget(_)
        ) && error
            .to_string()
            .contains("configured shared stable aggregate"),
        "unexpected exact route-pair budget error: {error}"
    );
}
#[test]
fn native_amx_prevote_pair_geometry_rejects_empty_hard_cap_and_overflow() {
    let kura = Kura::blank_kura_for_testing();
    let empty = kura
        .validate_native_amx_participant_application_pair_byte_lengths(
            0,
            1,
            STRICT_INIT_MAX_BLOCK_BYTES,
        )
        .expect_err("empty Native manifest framing must fail closed");
    assert!(empty.to_string().contains("manifest framing is empty"));
    let standalone = kura
        .validate_native_amx_participant_application_pair_byte_lengths(2, 1, 1)
        .expect_err("an individually oversized Native manifest must fail closed");
    assert!(
        standalone
            .to_string()
            .contains("manifest is 2 bytes, exceeding the standalone payload budget")
    );
    let overflow = checked_native_amx_participant_application_pair_bytes(u64::MAX, 1)
        .expect_err("Native pair length overflow must fail closed");
    assert!(overflow.to_string().contains("byte length overflowed"));
}
#[test]
fn native_amx_manifest_temp_requires_qc_authenticated_finality_before_promotion() {
    let temp_dir = TempDir::new().expect("forged Native manifest temporary directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("initialize forged Native manifest temporary Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("forged Native manifest temporary lane entry");
    let _receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    let manifest_path =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
    let manifest_temp = manifest_path.with_extension("norito.tmp");
    let manifest_bytes = fs::read(&manifest_path).expect("read authenticated Native manifest");
    let mut forged = norito::decode_canonical::<NativeAmxParticipantApplicationManifestArtifactV1>(
        &manifest_bytes,
    )
    .expect("decode authenticated Native manifest");
    forged.finality_artifact_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"forged Native manifest temporary finality"));
    let forged_bytes = forged
        .encode_framed()
        .expect("encode structurally valid forged Native manifest");
    fs::remove_file(&manifest_path).expect("remove stable Native manifest");
    fs::write(&manifest_temp, &forged_bytes).expect("stage forged Native manifest temporary");
    std::fs::File::open(&manifest_temp)
        .expect("open forged Native manifest temporary")
        .sync_all()
        .expect("sync forged Native manifest temporary");
    sync_dir(
        manifest_temp
            .parent()
            .expect("forged Native manifest evidence directory"),
    )
    .expect("sync forged Native manifest evidence directory");
    let _prune_guard = kura.prune_lock.lock();
    let _canonical_chain_guard = kura.canonical_chain_lock.lock();
    let _geometry_guard = kura.lane_geometry_lock.lock();
    let _sidecar_guard = kura.sidecar_lock.lock();
    let namespace = kura
        .native_amx_evidence_namespace_for_entry(&entry)
        .expect("bind forged Native manifest evidence namespace");
    let error = kura
        .recover_native_amx_evidence_publication_temp_locked(
            &entry,
            &namespace,
            NativeAmxEvidenceRecoveryPhase::ManifestPublication,
        )
        .expect_err("a structurally valid manifest without matching finality must fail");
    assert!(
        error
            .to_string()
            .contains("authenticated by available finality"),
        "unexpected forged Native manifest temporary error: {error}"
    );
    assert_eq!(
        fs::read(&manifest_temp).expect("reread forged Native manifest temporary"),
        forged_bytes,
        "failed authentication must retain the exact temporary for forensics"
    );
    assert!(
        !manifest_path.exists(),
        "unauthenticated Native manifest temporary must not be promoted"
    );
}
#[test]
fn native_amx_receipt_temp_requires_manifest_finality_before_promotion() {
    let temp_dir = TempDir::new().expect("unbacked Native receipt temporary directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("initialize unbacked Native receipt temporary Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("unbacked Native receipt temporary lane entry");
    let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    let receipt_path =
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
    let receipt_temp = receipt_path.with_extension("norito.tmp");
    let receipt_bytes = fs::read(&receipt_path).expect("read authenticated Native receipt");
    fs::remove_file(&receipt_path).expect("remove stable Native receipt");
    fs::write(&receipt_temp, &receipt_bytes).expect("stage unbacked Native receipt temporary");
    std::fs::File::open(&receipt_temp)
        .expect("open unbacked Native receipt temporary")
        .sync_all()
        .expect("sync unbacked Native receipt temporary");
    kura.remove_v2_finality_without_binding_for_tests(receipt.application_block_height)
        .expect("remove finality backing the Native receipt temporary");
    sync_dir(
        receipt_temp
            .parent()
            .expect("unbacked Native receipt evidence directory"),
    )
    .expect("sync unbacked Native receipt evidence directory");
    let _prune_guard = kura.prune_lock.lock();
    let _canonical_chain_guard = kura.canonical_chain_lock.lock();
    let _geometry_guard = kura.lane_geometry_lock.lock();
    let _sidecar_guard = kura.sidecar_lock.lock();
    let namespace = kura
        .native_amx_evidence_namespace_for_entry(&entry)
        .expect("bind unbacked Native receipt evidence namespace");
    let error = kura
        .recover_native_amx_evidence_publication_temp_locked(
            &entry,
            &namespace,
            NativeAmxEvidenceRecoveryPhase::ReceiptPublication,
        )
        .expect_err("a receipt temporary without authenticated finality must fail");
    assert!(
        error
            .to_string()
            .contains("authenticated by available finality"),
        "unexpected unbacked Native receipt temporary error: {error}"
    );
    assert_eq!(
        fs::read(&receipt_temp).expect("reread unbacked Native receipt temporary"),
        receipt_bytes,
        "failed authentication must retain the exact receipt temporary for forensics"
    );
    assert!(
        !receipt_path.exists(),
        "unbacked Native receipt temporary must not be promoted"
    );
}
#[test]
fn native_amx_redundant_temp_is_not_deleted_before_finality_authentication() {
    let temp_dir = TempDir::new().expect("redundant Native temporary directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("initialize redundant Native temporary Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("redundant Native temporary lane entry");
    let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    let manifest_path =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
    let manifest_temp = manifest_path.with_extension("norito.tmp");
    let manifest_bytes = fs::read(&manifest_path).expect("read stable Native manifest");
    fs::write(&manifest_temp, &manifest_bytes).expect("stage redundant Native manifest temp");
    std::fs::File::open(&manifest_temp)
        .expect("open redundant Native manifest temp")
        .sync_all()
        .expect("sync redundant Native manifest temp");
    kura.remove_v2_finality_without_binding_for_tests(receipt.application_block_height)
        .expect("remove finality before redundant-temp recovery");
    sync_dir(
        manifest_temp
            .parent()
            .expect("redundant Native manifest evidence directory"),
    )
    .expect("sync redundant Native manifest evidence directory");
    let _prune_guard = kura.prune_lock.lock();
    let _canonical_chain_guard = kura.canonical_chain_lock.lock();
    let _geometry_guard = kura.lane_geometry_lock.lock();
    let _sidecar_guard = kura.sidecar_lock.lock();
    let namespace = kura
        .native_amx_evidence_namespace_for_entry(&entry)
        .expect("bind redundant Native evidence namespace");
    let error = kura
        .recover_native_amx_evidence_publication_temp_locked(
            &entry,
            &namespace,
            NativeAmxEvidenceRecoveryPhase::ManifestPublication,
        )
        .expect_err("redundant temporary cleanup must authenticate finality first");
    assert!(
        error
            .to_string()
            .contains("authenticated by available finality"),
        "unexpected redundant Native temporary error: {error}"
    );
    assert_eq!(
        fs::read(&manifest_path).expect("reread stable Native manifest"),
        manifest_bytes
    );
    assert_eq!(
        fs::read(&manifest_temp).expect("reread redundant Native manifest temporary"),
        manifest_bytes,
        "unauthenticated redundant temporary must remain untouched"
    );
}
#[test]
fn native_amx_startup_retention_waits_for_complete_post_wsv_evidence() {
    use NativeAmxParticipantReceiptStartupEvidence::{
        DurablyApplied, PendingManifestRepair, PendingTipMetadata,
    };
    assert!(native_amx_startup_retention_cleanup_authorized(
        Some(DurablyApplied),
        false,
    ));
    assert!(native_amx_startup_retention_cleanup_authorized(None, false,));
    assert!(!native_amx_startup_retention_cleanup_authorized(
        Some(PendingTipMetadata),
        false,
    ));
    assert!(!native_amx_startup_retention_cleanup_authorized(
        Some(PendingManifestRepair),
        false,
    ));
    assert!(!native_amx_startup_retention_cleanup_authorized(
        Some(DurablyApplied),
        true,
    ));
}
#[test]
fn native_amx_prepublication_retains_previous_pair_until_post_wsv_cleanup() {
    let temp_dir = TempDir::new().expect("prepublication retention Kura directory");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.lane_history_retention =
        NonZeroUsize::new(1).expect("one-record Native evidence retention");
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("initialize prepublication retention Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("prepublication primary lane entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
    let newest_receipt = receipts
        .last()
        .expect("newest prepublication receipt")
        .clone();
    let old_manifest_path =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
    let old_receipt_path =
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
    let newest_manifest_path =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 2);
    let newest_receipt_path =
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 2);
    let evidence_directory = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root));
    let namespace = kura
        .native_amx_evidence_namespace_for_entry(&entry)
        .expect("bind prepublication Native evidence namespace");
    let newest_manifest = kura
        .read_native_amx_participant_application_manifest_from_paths_locked(
            &entry,
            2,
            &newest_manifest_path,
            &namespace,
        )
        .expect("read newest prepublication manifest");
    drop(namespace);
    std::fs::remove_file(&newest_manifest_path)
        .expect("remove newest manifest before replaying prepublication");
    std::fs::remove_file(&newest_receipt_path)
        .expect("remove newest receipt before replaying prepublication");
    sync_dir(&evidence_directory).expect("sync prepublication evidence removal");
    let checkpoint = kura
        .wsv_checkpoint(1)
        .expect("read fixture checkpoint")
        .expect("fixture checkpoint exists");
    let finality = kura
        .v2_finality_artifact(1)
        .expect("read fixture finality")
        .expect("fixture finality exists");
    kura.remove_commit_manifest_without_binding_for_tests(1)
        .expect("remove post-apply commit manifest");
    kura.remove_wsv_checkpoint_without_binding_for_tests(1)
        .expect("remove post-apply WSV checkpoint");
    {
        let _publication_guard = kura.prune_lock.lock();
        kura.write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard(
                &newest_manifest,
                false,
            )
            .expect("prepublish newest Native manifest without cleanup");
        kura.write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard(
                &newest_receipt,
                &newest_manifest,
                false,
            )
            .expect("prepublish newest Native receipt without cleanup");
        kura.write_native_amx_participant_receipt_latest_index_under_publication_guard(
            &newest_receipt,
            &newest_manifest,
            false,
        )
        .expect("prepublish newest Native latest index without cleanup");
        kura.authenticate_native_amx_participant_application_prepublication_under_publication_guard(
                &newest_manifest,
                &newest_receipt,
                false,
            )
            .expect("authenticate pre-WSV evidence without post-apply metadata");
    }
    assert!(
        old_manifest_path.exists() && old_receipt_path.exists(),
        "retention=1 must preserve the previous complete pair before WSV commit"
    );
    kura.store_wsv_checkpoint(
        1,
        newest_receipt.application_block_hash,
        checkpoint.state_hash,
    )
    .expect("restore post-apply WSV checkpoint");
    kura.store_commit_manifest(
        CommitManifest::new(
            1,
            newest_receipt.application_block_hash,
            None,
            None,
            checkpoint.state_hash,
            None,
        )
        .with_authenticated_v2_commit_authority(&finality),
    )
    .expect("restore authenticated post-apply commit manifest");
    {
        let _publication_guard = kura.prune_lock.lock();
        kura.authenticate_native_amx_participant_application_prepublication_under_publication_guard(
                &newest_manifest,
                &newest_receipt,
                true,
            )
            .expect("reauthenticate Native evidence against post-WSV metadata");
        kura.cleanup_native_amx_participant_application_evidence_under_publication_guard(
            &newest_receipt,
        )
        .expect("perform post-WSV Native evidence cleanup");
    }
    assert!(
        !old_manifest_path.exists() && !old_receipt_path.exists(),
        "post-WSV cleanup may enforce retention after the exact join is authenticated"
    );
    assert!(
        newest_manifest_path.exists() && newest_receipt_path.exists(),
        "cleanup must retain the exact newest prepublished evidence"
    );
}
