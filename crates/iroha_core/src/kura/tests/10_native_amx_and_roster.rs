#[test]
fn native_amx_manifest_artifact_rejects_leaf_or_proof_tampering() {
    let leaf = NativeAmxApplicationManifestLeafV1 {
        version: iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
        lane_id: LaneId::new(3),
        dataspace_id: DataSpaceId::new(4),
        lane_incarnation: Hash::new(b"native manifest test incarnation"),
        participant_height: 8,
        participant_view: 2,
        predecessor_height: 7,
        predecessor_descriptor_hash: Some(Hash::new(b"native manifest test predecessor")),
        descriptor_hash: Hash::new(b"native manifest test descriptor"),
        proposal_hash: Hash::new(b"native manifest test proposal"),
        settlement_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"native manifest test settlement",
        )),
        members: vec![
            iroha_data_model::block::consensus_v2::NativeAmxApplicationManifestMemberV1 {
                entrypoint_index: 5,
                source_id: [0x51; Hash::LENGTH],
                entrypoint_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"native manifest test entrypoint",
                )),
                result_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"native manifest test result",
                )),
            },
        ],
        application_block_height: 19,
        application_block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"native manifest test application",
        )),
        executed_block_wire_hash: Hash::new(b"native manifest test executed wire"),
    };
    let mut second_leaf = leaf.clone();
    second_leaf.lane_id = LaneId::new(5);
    second_leaf.dataspace_id = DataSpaceId::new(6);
    second_leaf.lane_incarnation = Hash::new(b"native manifest second incarnation");
    second_leaf.descriptor_hash = Hash::new(b"native manifest second descriptor");
    second_leaf.proposal_hash = Hash::new(b"native manifest second proposal");
    second_leaf.members[0].source_id = [0x62; Hash::LENGTH];
    second_leaf.members[0].entrypoint_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"native manifest second entrypoint"));
    second_leaf.members[0].result_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"native manifest second result"));
    let tree = [HashOf::new(&leaf), HashOf::new(&second_leaf)]
        .into_iter()
        .collect::<MerkleTree<_>>();
    let mut artifact = NativeAmxParticipantApplicationManifestArtifactV1 {
        version: NativeAmxParticipantApplicationManifestArtifactV1::VERSION,
        leaf,
        leaf_index: 0,
        proof: tree.get_proof(0).expect("two-leaf proof"),
        manifest_root: tree.root().map(Hash::from).expect("two-leaf root"),
        manifest_leaf_count: 2,
        finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"native manifest test finality",
        )),
    };
    Kura::validate_native_amx_participant_application_manifest_artifact(&artifact)
        .expect("canonical Native AMX manifest artifact");

    let canonical_leaf = artifact.leaf.clone();
    artifact.leaf.members[0].result_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"tampered Native AMX manifest result"));
    assert!(
        Kura::validate_native_amx_participant_application_manifest_artifact(&artifact).is_err(),
        "a changed leaf must no longer verify against the QC-authenticated root"
    );
    artifact.leaf = canonical_leaf;

    let mut tampered_path = artifact.proof.clone().into_audit_path();
    assert!(
        !tampered_path.is_empty(),
        "two leaves must produce one proof sibling"
    );
    tampered_path[0] = Some(HashOf::from_untyped_unchecked(Hash::new(
        b"forged Native AMX manifest sibling",
    )));
    artifact.proof = MerkleProof::from_audit_path(artifact.leaf_index, tampered_path);
    assert!(
        Kura::validate_native_amx_participant_application_manifest_artifact(&artifact).is_err(),
        "a changed audit-path hash must no longer verify against the QC-authenticated root"
    );
}

include!("10c_native_amx_latest_index_support_and_bounds.rs");
#[test]
fn native_amx_latest_index_startup_rejects_fully_unbacked_pointer() {
    let temp_dir = TempDir::new().expect("temporary Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("primary lane storage entry");
    let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
        .expect("publish exact V2 latest pointer");
    let descriptor = &receipt.participant_proposal.descriptor;
    let manifest_path = Kura::native_amx_application_manifest_path_for_entry(
        &entry,
        &kura.store_root,
        descriptor.lane_block_height,
    );
    let receipt_path = Kura::native_amx_participant_receipt_path_for_entry(
        &entry,
        &kura.store_root,
        descriptor.lane_block_height,
    );
    let latest_path =
        Kura::native_amx_participant_receipt_latest_index_path_for_entry(&entry, &kura.store_root);
    fs::remove_file(&manifest_path).expect("remove latest Native manifest");
    fs::remove_file(&receipt_path).expect("remove latest Native receipt");
    sync_dir(Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root)).as_path())
        .expect("sync fully unbacked Native evidence directory");
    drop(kura);

    let error = match Kura::new(&config, &lane_config) {
        Ok(_) => panic!("startup must reject a latest pointer with no evidence backing"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("not backed by its exact receipt or QC-authenticated manifest"),
        "unexpected fully unbacked latest-pointer error: {error}"
    );
    assert!(
        latest_path.exists(),
        "fail-closed startup must retain the unbacked pointer for forensics"
    );
}

#[test]
fn native_amx_latest_index_startup_rejects_manifest_binding_drift_without_receipt() {
    for drift_kind in ["executed wire", "finality", "manifest"] {
        let temp_dir = TempDir::new().expect("temporary Kura directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("primary lane storage entry");
        let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
        kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect("publish exact V2 latest pointer");
        let latest_path = Kura::native_amx_participant_receipt_latest_index_path_for_entry(
            &entry,
            &kura.store_root,
        );
        let mut latest = kura
            .decode_native_amx_participant_receipt_latest_index(&entry, &latest_path)
            .expect("decode V2 latest pointer")
            .expect("V2 latest pointer exists");
        match drift_kind {
            "executed wire" => {
                latest.executed_block_wire_hash = Hash::new(b"forged latest-index executed wire")
            }
            "finality" => {
                latest.finality_artifact_hash = HashOf::from_untyped_unchecked(Hash::new(
                    b"forged latest-index finality artifact",
                ))
            }
            "manifest" => {
                latest.manifest_artifact_hash = HashOf::from_untyped_unchecked(Hash::new(
                    b"forged latest-index manifest artifact",
                ))
            }
            _ => unreachable!(),
        }
        fs::write(
            &latest_path,
            norito::encode_canonical(&latest).expect("encode drifted V2 latest pointer"),
        )
        .expect("persist drifted V2 latest pointer");
        let descriptor = &receipt.participant_proposal.descriptor;
        let receipt_path = Kura::native_amx_participant_receipt_path_for_entry(
            &entry,
            &kura.store_root,
            descriptor.lane_block_height,
        );
        fs::remove_file(&receipt_path).expect("remove exact Native receipt");
        sync_dir(Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root)).as_path())
            .expect("sync manifest-backed Native evidence directory");
        drop(kura);

        let error = match Kura::new(&config, &lane_config) {
            Ok(_) => {
                panic!("startup must reject {drift_kind} drift without an exact receipt")
            }
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("not backed by its exact receipt or QC-authenticated manifest"),
            "unexpected {drift_kind} latest-pointer error: {error}"
        );
        assert!(
            latest_path.exists(),
            "fail-closed startup must retain the drifted pointer for forensics"
        );
    }
}

#[test]
fn native_amx_latest_index_startup_rejects_present_invalid_manifest_proof() {
    let temp_dir = TempDir::new().expect("temporary Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("primary lane storage entry");
    let _receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    let manifest_data_path =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
    let namespace = kura
        .native_amx_evidence_namespace_for_entry(&entry)
        .expect("bind proof-invalid Native evidence namespace");
    let mut manifest = kura
        .read_native_amx_participant_application_manifest_from_paths_locked(
            &entry,
            1,
            &manifest_data_path,
            &namespace,
        )
        .expect("read valid Native manifest fixture");
    manifest.manifest_root = Hash::new(b"forged startup Native manifest root");
    fs::write(
        &manifest_data_path,
        manifest
            .encode_framed()
            .expect("encode proof-invalid Native manifest"),
    )
    .expect("overwrite Native manifest with proof-invalid bytes");
    drop(kura);

    let error = match Kura::new(&config, &lane_config) {
        Ok(_) => panic!("startup must reject a present proof-invalid Native manifest"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("malformed")
            || error.to_string().contains("manifest")
            || error.to_string().contains("proof"),
        "unexpected proof-invalid startup error: {error}"
    );
}

#[test]
fn native_amx_latest_index_binds_route_incarnation_and_exact_receipt() {
    let (session, _) =
        sample_committed_lane_block_session_for_kura(LaneId::SINGLE, DataSpaceId::UNIVERSAL, 1);
    let proposal = session.proposal;
    let settlement = LaneBlockCommitment {
        block_height: 1,
        lane_id: LaneId::SINGLE,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        dataspace_id: DataSpaceId::UNIVERSAL,
        tx_count: 0,
        total_local_amount: "0".parse().expect("zero quantity"),
        total_xor_due: "0".parse().expect("zero quantity"),
        total_xor_after_haircut: "0".parse().expect("zero quantity"),
        total_xor_variance: "0".parse().expect("zero quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let settlement_hash = iroha_data_model::nexus::compute_settlement_hash(&settlement)
        .expect("hash fixture settlement");
    let mut receipt = NativeAmxParticipantApplicationReceiptArtifact {
        version: NativeAmxParticipantApplicationReceiptArtifact::VERSION,
        participant_proposal: proposal.clone(),
        participant_settlement: settlement,
        participant_settlement_hash: settlement_hash,
        application_block_height: proposal.descriptor.proposal_height,
        application_block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"native-latest-application-block",
        )),
        executed_block_wire_hash: Hash::new(b"native-latest-executed-wire"),
        finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"native-latest-finality",
        )),
        manifest_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"native-latest-manifest",
        )),
        source_ids: Vec::new(),
        entrypoint_indices: Vec::new(),
        entrypoint_hashes: Vec::new(),
        result_hashes: Vec::new(),
        results: Vec::new(),
    };
    let latest = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipt);
    assert!(
        latest.matches_receipt(&receipt),
        "latest index must match the exact authenticated receipt identity"
    );
    assert_eq!(latest.version, 2);
    assert_eq!(latest.lane_id, proposal.descriptor.lane_id);
    assert_eq!(latest.dataspace_id, proposal.descriptor.dataspace_id);
    assert_eq!(
        latest.lane_incarnation,
        proposal.descriptor.lane_incarnation
    );
    assert_eq!(
        latest.executed_block_wire_hash,
        receipt.executed_block_wire_hash
    );
    assert_eq!(
        latest.finality_artifact_hash,
        receipt.finality_artifact_hash
    );
    assert_eq!(
        latest.manifest_artifact_hash,
        receipt.manifest_artifact_hash
    );

    receipt.application_block_height = receipt.application_block_height.saturating_add(1);
    assert!(
        !latest.matches_receipt(&receipt),
        "application identity drift must change the authenticated pointer identity"
    );
}

#[test]
fn native_amx_latest_index_rebuilds_idempotently_after_receipt_append_crash() {
    let temp_dir = TempDir::new().expect("temporary Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("primary lane storage entry");
    let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    let latest_path =
        Kura::native_amx_participant_receipt_latest_index_path_for_entry(&entry, &kura.store_root);
    assert!(
        !latest_path.exists(),
        "fixture represents a crash before latest-index publication"
    );
    assert_eq!(
        kura.read_structural_native_amx_participant_application_receipt(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            1,
        ),
        Some(receipt.clone()),
        "diagnostics must read an exact structural sidecar without relying on the derived latest index"
    );
    kura.refresh_disk_usage_bytes()
        .expect("initialize exact Native evidence disk accounting");

    assert_eq!(
        kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect("rebuild missing latest index"),
        1
    );
    let latest = kura
        .decode_native_amx_participant_receipt_latest_index(&entry, &latest_path)
        .expect("decode rebuilt latest index")
        .expect("rebuilt latest index exists");
    assert!(latest.matches_receipt(&receipt));
    assert_eq!(
        kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect("repeat latest-index rebuild"),
        0,
        "reconstruction must be idempotent"
    );
    assert_eq!(
        kura.disk_usage.load(Ordering::Relaxed),
        kura.kura_disk_usage_bytes()
            .expect("scan enforced bytes after Native rebuild"),
        "Native rebuild must publish the receipt, manifest, and latest-pointer delta"
    );
    assert_eq!(
        kura.disk_usage_total.load(Ordering::Relaxed),
        kura.kura_total_disk_usage_bytes()
            .expect("scan total bytes after Native rebuild"),
        "Native rebuild must keep total accounting exact without a later rescan"
    );
}

#[test]
fn native_amx_latest_index_startup_reconciles_exact_temporary_matrix() {
    for crash_shape in ["lone", "older-stable", "identical"] {
        let temp_dir = TempDir::new().expect("Native latest-temp recovery directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) =
            Kura::new(&config, &lane_config).expect("initialize Native latest-temp Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native latest-temp primary lane entry");
        let receipt_heights: &[u64] = if crash_shape == "lone" { &[1] } else { &[1, 2] };
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, receipt_heights);
        let newest = receipts.last().expect("newest Native latest-temp receipt");
        let newest_index = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(newest);
        let newest_bytes =
            norito::encode_canonical(&newest_index).expect("encode newest Native latest index");
        let (latest_path, latest_temp_path) = native_amx_latest_index_test_paths(&kura, &entry);
        let mut stable_metadata_before = None;
        match crash_shape {
            "lone" => {}
            "older-stable" => {
                let older = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipts[0]);
                write_synced_native_amx_test_file(
                    &latest_path,
                    &norito::encode_canonical(&older).expect("encode older Native latest index"),
                );
            }
            "identical" => {
                write_synced_native_amx_test_file(&latest_path, &newest_bytes);
                stable_metadata_before = Kura::regular_sidecar_metadata_for(
                    &kura.store_root,
                    &latest_path,
                    latest_path
                        .parent()
                        .expect("Native latest index has a directory"),
                )
                .expect("inspect identical stable Native latest index");
            }
            _ => unreachable!("fixed Native latest-temp recovery matrix"),
        }
        write_synced_native_amx_test_file(&latest_temp_path, &newest_bytes);
        kura.refresh_disk_usage_bytes()
            .expect("initialize Native latest-temp disk accounting");

        let rebuilt = kura
            .rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .unwrap_or_else(|error| panic!("{crash_shape} latest-temp recovery failed: {error}"));
        assert_eq!(
            rebuilt,
            if crash_shape == "identical" { 0 } else { 1 },
            "only a byte-preserving promotion counts as an index rebuild"
        );
        assert_eq!(
            fs::read(&latest_path).expect("read recovered Native latest index"),
            newest_bytes,
            "{crash_shape} recovery must retain the exact staged bytes"
        );
        assert!(
            !latest_temp_path.exists(),
            "{crash_shape} recovery must consume the exact temporary"
        );
        if let Some(before) = stable_metadata_before {
            let after = Kura::regular_sidecar_metadata_for(
                &kura.store_root,
                &latest_path,
                latest_path
                    .parent()
                    .expect("Native latest index has a directory"),
            )
            .expect("reinspect identical stable Native latest index")
            .expect("identical stable Native latest index remains");
            assert!(
                Kura::sidecar_file_metadata_unchanged(&before.file, &after.file),
                "identical-temp cleanup must not replace or rewrite the stable pointer"
            );
        }
        assert_eq!(
            kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
                .expect("repeat exact Native latest-temp reconciliation"),
            0,
            "{crash_shape} recovery must be idempotent"
        );
        assert_eq!(
            kura.disk_usage.load(Ordering::Relaxed),
            kura.kura_disk_usage_bytes()
                .expect("scan enforced usage after Native latest-temp recovery"),
            "{crash_shape} recovery must keep enforced accounting exact"
        );
        assert_eq!(
            kura.disk_usage_total.load(Ordering::Relaxed),
            kura.kura_total_disk_usage_bytes()
                .expect("scan total usage after Native latest-temp recovery"),
            "{crash_shape} recovery must keep total accounting exact"
        );

        drop(kura);
        let (reopened, _) = Kura::new(&config, &lane_config)
            .unwrap_or_else(|error| panic!("{crash_shape} idempotent reopen failed: {error}"));
        assert_eq!(
            fs::read(&latest_path).expect("read Native latest index after idempotent reopen"),
            newest_bytes,
            "reopen must preserve the exact recovered latest-index bytes"
        );
        drop(reopened);
    }
}

#[test]
fn native_amx_latest_index_temporary_failures_retain_exact_forensics() {
    for damage in [
        "malformed",
        "truncated",
        "oversized",
        "wrong-route",
        "stale-incarnation",
        "unbacked-temp",
        "backed-older-temp",
        "unbacked-stable",
    ] {
        let temp_dir = TempDir::new().expect("damaged Native latest-temp directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) =
            Kura::new(&config, &lane_config).expect("initialize damaged Native latest-temp Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("damaged Native latest-temp lane entry");
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
        let newest = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(
            receipts.last().expect("newest damaged-temp receipt"),
        );
        let older = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipts[0]);
        let (latest_path, latest_temp_path) = native_amx_latest_index_test_paths(&kura, &entry);
        let mut temporary = newest;
        let mut stable = None;
        let temp_bytes = match damage {
            "malformed" => vec![0xA5],
            "truncated" => {
                let encoded = norito::encode_canonical(&newest)
                    .expect("encode Native latest index before truncation");
                encoded[..encoded.len().saturating_sub(1)].to_vec()
            }
            "oversized" => {
                vec![0x5A; NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_MAX_BYTES + 1]
            }
            "wrong-route" => {
                temporary.dataspace_id = DataSpaceId::new(91);
                norito::encode_canonical(&temporary).expect("encode wrong-route Native latest temp")
            }
            "stale-incarnation" => {
                temporary.lane_incarnation = Hash::new(b"stale Native latest-temp incarnation");
                norito::encode_canonical(&temporary)
                    .expect("encode stale-incarnation Native latest temp")
            }
            "unbacked-temp" => {
                temporary.participant_proposal_hash =
                    Hash::new(b"unbacked Native latest-temp proposal");
                norito::encode_canonical(&temporary).expect("encode unbacked Native latest temp")
            }
            "backed-older-temp" => {
                stable = Some(newest);
                norito::encode_canonical(&older).expect("encode backed older Native latest temp")
            }
            "unbacked-stable" => {
                let mut unbacked = newest;
                unbacked.participant_proposal_hash =
                    Hash::new(b"unbacked stable Native latest proposal");
                stable = Some(unbacked);
                norito::encode_canonical(&newest).expect("encode exact newest Native latest temp")
            }
            _ => unreachable!("fixed damaged Native latest-temp matrix"),
        };
        if let Some(stable) = stable {
            write_synced_native_amx_test_file(
                &latest_path,
                &norito::encode_canonical(&stable).expect("encode damaged-test stable pointer"),
            );
        }
        write_synced_native_amx_test_file(&latest_temp_path, &temp_bytes);
        let evidence_directory = latest_path
            .parent()
            .expect("damaged Native latest index has a directory");
        let before = snapshot_regular_files_recursively(evidence_directory);

        let error = kura
            .rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect_err("damaged Native latest temp must fail closed");
        assert!(
            error.to_string().contains("latest")
                || error.to_string().contains("oversized")
                || error.to_string().contains("decode")
                || error.to_string().contains("route"),
            "unexpected {damage} Native latest-temp error: {error}"
        );
        assert_eq!(
            fs::read(&latest_temp_path).expect("reread rejected Native latest temp"),
            temp_bytes,
            "{damage} must retain exact temporary bytes"
        );
        assert_eq!(
            snapshot_regular_files_recursively(evidence_directory),
            before,
            "{damage} must not mutate stable evidence or pointer bytes"
        );

        drop(kura);
        let reopen_error = match Kura::new(&config, &lane_config) {
            Ok(_) => panic!("{damage} Native latest temp must also fail real startup"),
            Err(error) => error,
        };
        assert!(
            reopen_error.to_string().contains("latest")
                || reopen_error.to_string().contains("oversized")
                || reopen_error.to_string().contains("decode")
                || reopen_error.to_string().contains("route"),
            "unexpected {damage} Native latest-temp reopen error: {reopen_error}"
        );
        assert_eq!(
            snapshot_regular_files_recursively(evidence_directory),
            before,
            "{damage} startup rejection must retain the full forensic inventory"
        );
    }
}

#[cfg(unix)]
#[test]
fn native_amx_latest_index_temporary_rejects_links_without_touching_targets() {
    use std::os::unix::fs::symlink;

    for link_kind in ["symlink", "hardlink"] {
        let temp_dir = TempDir::new().expect("linked Native latest-temp directory");
        let external_dir = TempDir::new().expect("external Native latest-temp directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) =
            Kura::new(&config, &lane_config).expect("initialize linked Native latest-temp Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("linked Native latest-temp lane entry");
        let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
        let latest = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipt);
        let exact_bytes =
            norito::encode_canonical(&latest).expect("encode linked Native latest temp");
        let (latest_path, latest_temp_path) = native_amx_latest_index_test_paths(&kura, &entry);
        let external = external_dir.path().join("attacker-owned-latest-index");
        write_synced_native_amx_test_file(&external, &exact_bytes);
        match link_kind {
            "symlink" => symlink(&external, &latest_temp_path)
                .expect("stage symlinked Native latest-index temp"),
            "hardlink" => fs::hard_link(&external, &latest_temp_path)
                .expect("stage hardlinked Native latest-index temp"),
            _ => unreachable!("fixed Native latest-temp link matrix"),
        }
        let linked_metadata = fs::symlink_metadata(&latest_temp_path)
            .expect("inspect linked Native latest-index temp");

        let error = kura
            .rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect_err("linked Native latest-index temp must fail closed");
        assert!(
            error.to_string().contains("symlink")
                || error.to_string().contains("link")
                || error.to_string().contains("regular"),
            "unexpected {link_kind} Native latest-temp error: {error}"
        );
        assert_eq!(
            fs::read(&external).expect("reread external Native latest-index target"),
            exact_bytes,
            "{link_kind} rejection must not alter external bytes"
        );
        let after = fs::symlink_metadata(&latest_temp_path)
            .expect("linked Native latest-index temp remains for forensics");
        assert_eq!(
            linked_metadata.file_type().is_symlink(),
            after.file_type().is_symlink(),
            "{link_kind} rejection must retain the original link kind"
        );
        assert!(
            !latest_path.exists(),
            "{link_kind} latest temp must never be promoted"
        );
    }
}

#[test]
fn native_amx_latest_index_temporary_rejects_recovery_journal_overlap_before_mutation() {
    for overlap in [
        "stable-prune-intent",
        "temporary-prune-intent",
        "manifest-temporary",
        "receipt-temporary",
    ] {
        let temp_dir = TempDir::new().expect("overlapped Native latest-temp directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::new(&config, &lane_config)
            .expect("initialize overlapped Native latest-temp Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("overlapped Native latest-temp lane entry");
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
        let newest = receipts.last().expect("newest overlap-test Native receipt");
        let newest_index = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(newest);
        let (latest_path, latest_temp_path) = native_amx_latest_index_test_paths(&kura, &entry);
        write_synced_native_amx_test_file(
            &latest_temp_path,
            &norito::encode_canonical(&newest_index)
                .expect("encode overlap-test Native latest temp"),
        );
        let evidence_directory = latest_path
            .parent()
            .expect("overlap-test Native evidence directory");
        let overlap_path = match overlap {
            "stable-prune-intent" | "temporary-prune-intent" => {
                let intent = native_amx_prune_intent_for_test(&kura, &entry, newest, &[1]);
                let path = evidence_directory.join(if overlap == "stable-prune-intent" {
                    NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE
                } else {
                    NATIVE_AMX_EVIDENCE_PRUNE_INTENT_TEMP_FILE
                });
                write_synced_native_amx_test_file(
                    &path,
                    &norito::encode_canonical(&intent)
                        .expect("encode overlap-test Native prune intent"),
                );
                path
            }
            "manifest-temporary" => {
                let stable = Kura::native_amx_application_manifest_path_for_entry(
                    &entry,
                    &kura.store_root,
                    2,
                );
                let path = stable.with_extension("norito.tmp");
                write_synced_native_amx_test_file(
                    &path,
                    &fs::read(stable).expect("read overlap-test Native manifest"),
                );
                path
            }
            "receipt-temporary" => {
                let stable = Kura::native_amx_participant_receipt_path_for_entry(
                    &entry,
                    &kura.store_root,
                    2,
                );
                let path = stable.with_extension("norito.tmp");
                write_synced_native_amx_test_file(
                    &path,
                    &fs::read(stable).expect("read overlap-test Native receipt"),
                );
                path
            }
            _ => unreachable!("fixed Native recovery-journal overlap matrix"),
        };
        let before = snapshot_regular_files_recursively(evidence_directory);

        let error = kura
            .rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect_err("overlapped Native latest temp must fail before startup recovery");
        assert!(
            error.to_string().contains("overlap") || error.to_string().contains("unresolved"),
            "unexpected {overlap} startup overlap error: {error}"
        );
        assert_eq!(
            snapshot_regular_files_recursively(evidence_directory),
            before,
            "{overlap} startup rejection must not consume either journal"
        );

        if overlap.contains("prune-intent") {
            let _prune_guard = kura.prune_lock.lock();
            let _canonical_guard = kura.canonical_chain_lock.lock();
            let _geometry_guard = kura.lane_geometry_lock.lock();
            let _sidecar_guard = kura.sidecar_lock.lock();
            let namespace = kura
                .native_amx_evidence_namespace_for_entry(&entry)
                .expect("open overlap-test Native namespace");
            kura.complete_native_amx_evidence_prune_intent_locked(&entry, &namespace)
                .expect_err("direct prune completion must reject an unresolved latest temp");
        } else {
            let manifest_path =
                Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 2);
            let receipt_path =
                Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 2);
            let manifest =
                norito::decode_canonical::<NativeAmxParticipantApplicationManifestArtifactV1>(
                    &fs::read(manifest_path).expect("read runtime-overlap Native manifest"),
                )
                .expect("decode runtime-overlap Native manifest");
            let receipt =
                norito::decode_canonical::<NativeAmxParticipantApplicationReceiptArtifact>(
                    &fs::read(receipt_path).expect("read runtime-overlap Native receipt"),
                )
                .expect("decode runtime-overlap Native receipt");
            let _prune_guard = kura.prune_lock.lock();
            if overlap == "manifest-temporary" {
                kura.write_native_amx_participant_application_manifest_artifact_under_publication_guard(
                    &manifest,
                )
                .expect_err("manifest writer must reject latest/evidence temp overlap");
            } else {
                kura.write_native_amx_participant_application_receipt_artifact_under_publication_guard(
                    &receipt,
                    &manifest,
                )
                .expect_err("receipt writer must reject latest/evidence temp overlap");
            }
        }
        assert!(
            overlap_path.exists() && latest_temp_path.exists(),
            "{overlap} rejection must retain both recovery artifacts"
        );
        assert_eq!(
            snapshot_regular_files_recursively(evidence_directory),
            before,
            "{overlap} direct recovery rejection must retain exact bytes"
        );
    }
}

#[test]
fn native_amx_latest_index_temporary_recovery_crash_boundaries_converge() {
    for boundary in [
        "temp-resync",
        "post-promotion-directory-sync",
        "identical-unlink-directory-sync",
    ] {
        let temp_dir = TempDir::new().expect("Native latest-temp crash-boundary directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::new(&config, &lane_config)
            .expect("initialize Native latest-temp crash-boundary Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native latest-temp crash-boundary lane entry");
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
        let older = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipts[0]);
        let newest = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(
            receipts
                .last()
                .expect("newest crash-boundary Native receipt"),
        );
        let older_bytes =
            norito::encode_canonical(&older).expect("encode crash-boundary older pointer");
        let newest_bytes =
            norito::encode_canonical(&newest).expect("encode crash-boundary newest pointer");
        let (latest_path, latest_temp_path) = native_amx_latest_index_test_paths(&kura, &entry);
        write_synced_native_amx_test_file(
            &latest_path,
            if boundary == "identical-unlink-directory-sync" {
                &newest_bytes
            } else {
                &older_bytes
            },
        );
        write_synced_native_amx_test_file(&latest_temp_path, &newest_bytes);
        let evidence_directory = latest_path
            .parent()
            .expect("crash-boundary Native evidence directory");
        let before = snapshot_regular_files_recursively(evidence_directory);
        kura.refresh_disk_usage_bytes()
            .expect("initialize Native latest-temp crash-boundary accounting");

        match boundary {
            "temp-resync" => fail_next_native_amx_latest_index_recovery_temp_sync_for_tests(),
            "post-promotion-directory-sync" | "identical-unlink-directory-sync" => {
                fail_next_indexed_sidecar_dir_sync_for_tests()
            }
            _ => unreachable!("fixed Native latest-temp crash-boundary matrix"),
        }
        let error = kura
            .rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect_err("injected Native latest-temp boundary must fail-stop");
        assert!(
            error.to_string().contains("sync") || error.to_string().contains("durability"),
            "unexpected {boundary} Native latest-temp error: {error}"
        );

        let kura = if boundary == "temp-resync" {
            assert_eq!(
                snapshot_regular_files_recursively(evidence_directory),
                before,
                "pre-promotion sync failure must preserve stable and temporary bytes"
            );
            assert_eq!(
                kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
                    .expect("retry pre-promotion Native latest-temp recovery"),
                1
            );
            kura
        } else {
            assert_eq!(
                fs::read(&latest_path).expect("read post-promotion stable Native latest index"),
                newest_bytes,
                "post-promotion failure must expose only the exact authenticated new bytes"
            );
            assert!(
                !latest_temp_path.exists(),
                "post-promotion failure has already consumed the renamed temporary"
            );
            drop(kura);
            let (reopened, _) = Kura::new(&config, &lane_config)
                .expect("restart after committed Native latest-index recovery boundary");
            assert_eq!(
                reopened
                    .rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
                    .expect("retry post-promotion Native latest-index recovery"),
                0,
                "retry must recognize the already-promoted exact pointer"
            );
            reopened
        };
        assert_eq!(
            fs::read(&latest_path).expect("read converged Native latest index"),
            newest_bytes,
            "{boundary} retry must converge to exact newest bytes"
        );
        assert!(!latest_temp_path.exists());
        assert_eq!(
            kura.disk_usage.load(Ordering::Relaxed),
            kura.kura_disk_usage_bytes()
                .expect("scan enforced usage after Native latest-temp crash recovery"),
            "{boundary} retry must reconcile enforced disk accounting"
        );
        assert_eq!(
            kura.disk_usage_total.load(Ordering::Relaxed),
            kura.kura_total_disk_usage_bytes()
                .expect("scan total usage after Native latest-temp crash recovery"),
            "{boundary} retry must reconcile total disk accounting"
        );
    }
}

#[test]
fn native_amx_latest_index_temporary_rejects_same_byte_swap_before_promotion() {
    let temp_dir = TempDir::new().expect("Native latest-temp swap directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) =
        Kura::new(&config, &lane_config).expect("initialize Native latest-temp swap Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("Native latest-temp swap lane entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
    let older = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipts[0]);
    let newest = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(
        receipts
            .last()
            .expect("newest Native latest-temp swap receipt"),
    );
    let older_bytes =
        norito::encode_canonical(&older).expect("encode older Native latest-temp swap pointer");
    let newest_bytes =
        norito::encode_canonical(&newest).expect("encode newest Native latest-temp swap pointer");
    let (latest_path, latest_temp_path) = native_amx_latest_index_test_paths(&kura, &entry);
    write_synced_native_amx_test_file(&latest_path, &older_bytes);
    write_synced_native_amx_test_file(&latest_temp_path, &newest_bytes);
    let displaced = temp_dir.path().join("displaced-native-latest-temp.norito");
    let displaced_for_hook = displaced.clone();
    set_native_amx_latest_index_pre_mutation_hook_for_tests(move |path| {
        let exact = fs::read(path).expect("read exact Native latest temp before swap");
        fs::rename(path, &displaced_for_hook).expect("displace exact Native latest temp");
        write_synced_native_amx_test_file(path, &exact);
    });

    let error = kura
        .rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
        .expect_err("same-byte Native latest-temp swap must fail before promotion");
    assert!(
        error.to_string().contains("changed") || error.to_string().contains("exact-object"),
        "unexpected Native latest-temp swap error: {error}"
    );
    assert_eq!(
        fs::read(&latest_path).expect("read stable pointer after rejected temp swap"),
        older_bytes,
        "rejected temp swap must not replace the stable pointer"
    );
    assert_eq!(
        fs::read(&latest_temp_path).expect("same-byte replacement temp survives rejection"),
        newest_bytes,
        "same-byte replacement temp must remain for forensics"
    );
    assert_eq!(
        fs::read(&displaced).expect("original authenticated temp remains displaced"),
        newest_bytes,
        "original authenticated temp bytes must remain available"
    );
    assert_eq!(
        kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect("retry Native latest-temp recovery after path swap"),
        1,
        "retry may authenticate and promote the now-stable replacement object"
    );
    assert_eq!(
        fs::read(&latest_path).expect("read converged Native latest pointer after swap"),
        newest_bytes
    );
    assert!(!latest_temp_path.exists());
}

#[test]
fn native_amx_prune_exact_object_removal_rejects_same_byte_path_swaps() {
    for swapped in ["first-target", "stable-intent"] {
        let temp_dir = TempDir::new().expect("Native prune path-swap directory");
        let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        config.roster_sidecar_retention =
            NonZeroUsize::new(1).expect("one-pair Native prune retention");
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) =
            Kura::new(&config, &lane_config).expect("initialize Native prune path-swap Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native prune path-swap lane entry");
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
        let newest = receipts
            .last()
            .expect("newest Native prune path-swap receipt");
        let (latest_path, _) = native_amx_latest_index_test_paths(&kura, &entry);
        write_synced_native_amx_test_file(
            &latest_path,
            &norito::encode_canonical(&NativeAmxParticipantReceiptLatestIndexV2::from_receipt(
                newest,
            ))
            .expect("encode Native prune path-swap latest pointer"),
        );
        let displaced = temp_dir.path().join(format!("displaced-{swapped}.norito"));
        let displaced_for_hook = displaced.clone();
        set_native_amx_prune_pre_unlink_hook_for_tests(
            if swapped == "first-target" { 0 } else { 2 },
            move |path| {
                let exact = fs::read(path).expect("read exact Native prune object before swap");
                fs::rename(path, &displaced_for_hook)
                    .expect("displace exact Native prune object before unlink");
                write_synced_native_amx_test_file(path, &exact);
            },
        );

        let error = {
            let _prune_guard = kura.prune_lock.lock();
            let _canonical_guard = kura.canonical_chain_lock.lock();
            let _geometry_guard = kura.lane_geometry_lock.lock();
            let _sidecar_guard = kura.sidecar_lock.lock();
            let namespace = kura
                .native_amx_evidence_namespace_for_entry(&entry)
                .expect("open Native prune path-swap namespace");
            kura.prune_native_amx_evidence_pairs_locked(&entry, &namespace)
                .expect_err("same-byte Native prune path swap must fail before unlink")
        };
        assert!(
            error.to_string().contains("changed")
                || error.to_string().contains("exact-object")
                || error.to_string().contains("identity"),
            "unexpected {swapped} Native prune path-swap error: {error}"
        );
        assert!(
            displaced.exists(),
            "the originally authenticated {swapped} object must remain available for forensics"
        );
        let displaced_bytes = fs::read(&displaced).expect("read displaced Native prune object");
        let replacement_path = if swapped == "first-target" {
            Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1)
        } else {
            latest_path
                .parent()
                .expect("Native prune intent has an evidence directory")
                .join(NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE)
        };
        assert_eq!(
            fs::read(&replacement_path).expect("replacement Native prune path survives rejection"),
            displaced_bytes,
            "same-byte replacement at {swapped} must not be unlinked"
        );
    }
}

#[test]
fn native_amx_drain_evidence_requires_exact_manifest_receipt_finality_and_latest_index() {
    let fixture = || {
        let temp_dir = TempDir::new().expect("temporary Kura directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("primary lane storage entry");
        let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
        kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect("publish exact Native AMX latest index");
        (temp_dir, kura, entry, receipt)
    };

    let (_temp_dir, kura, entry, receipt) = fixture();
    let evidence = kura
        .native_amx_participant_application_drain_evidence(&receipt)
        .expect("complete exact Native AMX drain evidence");
    let descriptor = &receipt.participant_proposal.descriptor;
    assert_eq!(evidence.participant_view, descriptor.lane_block_view);
    assert_eq!(evidence.predecessor_height, 0);
    assert_eq!(evidence.application_block_height, 1);
    assert_eq!(evidence.application_manifest_leaf_count, 1);
    assert_eq!(evidence.application_manifest_leaf_index, 0);

    let mut tampered_receipt = receipt.clone();
    tampered_receipt.executed_block_wire_hash =
        Hash::new(b"tampered Native AMX drain executed wire");
    assert_eq!(
        kura.native_amx_participant_application_drain_evidence(&tampered_receipt),
        None,
        "a receipt identity not backed by exact durable bytes must fail closed"
    );

    let latest_path =
        Kura::native_amx_participant_receipt_latest_index_path_for_entry(&entry, &kura.store_root);
    fs::write(&latest_path, [0xA5]).expect("corrupt Native AMX latest index");
    assert_eq!(
        kura.native_amx_participant_application_drain_evidence(&receipt),
        None,
        "a malformed latest-index artifact must fail drain evidence revalidation"
    );

    let (_temp_dir, kura, entry, receipt) = fixture();
    let manifest_data_path =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
    fs::remove_file(&manifest_data_path).expect("remove Native AMX manifest data");
    assert_eq!(
        kura.native_amx_participant_application_drain_evidence(&receipt),
        None,
        "a missing manifest leaf/proof artifact must fail drain evidence revalidation"
    );
}

#[test]
fn native_amx_retirement_scan_rejects_old_incarnation_evidence_after_aba_recreation() {
    let temp_dir = TempDir::new().expect("temporary Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("primary lane storage entry");
    let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
        .expect("publish exact Native AMX latest index");
    let old_incarnation = receipt.participant_proposal.descriptor.lane_incarnation;
    let recreated_incarnation = Hash::new(b"Native AMX retirement ABA incarnation B");
    assert_ne!(old_incarnation, recreated_incarnation);
    kura.install_lane_incarnation_marker_for_test(&entry, recreated_incarnation, 0)
        .expect("recreate the same lane route with incarnation B");

    let error = kura
        .first_release_lane_retirement_admissible_for_test(
            entry.lane_id,
            entry.dataspace_id,
            recreated_incarnation,
        )
        .expect_err("incarnation-A Native evidence in incarnation-B storage must fail closed");
    assert!(
        error
            .to_string()
            .contains("stale or duplicate Native AMX participant manifest identity"),
        "unexpected ABA retirement error: {error}"
    );
}

#[test]
fn native_amx_prune_intent_v2_rejects_b1_after_b2_recreation() {
    let temp_dir = TempDir::new().expect("Native prune-intent B1/B2 directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("initialize Native prune-intent B1/B2 Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("Native prune-intent B1/B2 lane entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
    let intent = native_amx_prune_intent_for_test(&kura, &entry, &receipts[1], &[1]);
    let removable_manifest =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
    let removable_receipt =
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
    let removable_manifest_bytes =
        fs::read(&removable_manifest).expect("snapshot B1 removable manifest");
    let removable_receipt_bytes =
        fs::read(&removable_receipt).expect("snapshot B1 removable receipt");
    let incarnation_b2 = Hash::new(b"Native prune intent recreated incarnation B2");
    assert_ne!(incarnation_b2, intent.lane_incarnation);
    kura.install_lane_incarnation_marker_for_test(&entry, incarnation_b2, 100)
        .expect("activate Native prune-intent incarnation B2");

    let _prune_guard = kura.prune_lock.lock();
    let _canonical_chain_guard = kura.canonical_chain_lock.lock();
    let _geometry_guard = kura.lane_geometry_lock.lock();
    let _sidecar_guard = kura.sidecar_lock.lock();
    let namespace = kura
        .native_amx_evidence_namespace_for_entry(&entry)
        .expect("bind recreated Native B2 namespace");
    let error = kura
        .validate_native_amx_evidence_prune_intent_locked(&entry, &namespace, &intent)
        .expect_err("a delayed B1 prune intent must not execute in B2");
    assert!(
        error.to_string().contains("stale route, incarnation"),
        "unexpected B1/B2 Native prune-intent error: {error}"
    );
    assert_eq!(
        fs::read(&removable_manifest).expect("reread B1 removable manifest"),
        removable_manifest_bytes
    );
    assert_eq!(
        fs::read(&removable_receipt).expect("reread B1 removable receipt"),
        removable_receipt_bytes
    );
}

#[test]
fn native_amx_latest_index_rebuild_accepts_only_narrow_pending_tip_metadata() {
    for pending_shape in ["metadata absent", "unbound checkpoint"] {
        let temp_dir = TempDir::new().expect("temporary Kura directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("primary lane storage entry");
        let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
        let descriptor = &receipt.participant_proposal.descriptor;
        kura.remove_commit_manifest_without_binding_for_tests(1)
            .expect("remove commit manifest");
        match pending_shape {
            "metadata absent" => kura
                .remove_wsv_checkpoint_without_binding_for_tests(1)
                .expect("remove WSV checkpoint"),
            "unbound checkpoint" => kura
                .overwrite_wsv_checkpoint_without_validation_for_tests(
                    1,
                    Hash::new(b"Native AMX latest-index WSV checkpoint"),
                    None,
                )
                .expect("clear checkpoint manifest binding"),
            _ => unreachable!(),
        }

        assert_eq!(
            kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
                .expect("rebuild pending-tip latest index"),
            1,
            "{pending_shape} is a recoverable exact-tip crash boundary"
        );
        let latest_path = Kura::native_amx_participant_receipt_latest_index_path_for_entry(
            &entry,
            &kura.store_root,
        );
        let latest = kura
            .decode_native_amx_participant_receipt_latest_index(&entry, &latest_path)
            .expect("decode pending-tip latest pointer")
            .expect("pending-tip latest pointer was rebuilt");
        assert!(
            latest.matches_receipt(&receipt),
            "startup must rebuild the exact pending-tip pointer"
        );
        assert_eq!(
            kura.read_native_amx_participant_application_receipt(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.lane_block_height,
            ),
            None,
            "normal runtime reads must remain strict while {pending_shape} is pending"
        );
        assert_eq!(
            kura.latest_native_amx_participant_application_receipt_matching(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                |_| true,
            ),
            None,
            "the rebuilt pointer must not weaken normal latest-evidence reads"
        );
    }
}

#[test]
fn native_amx_latest_index_rebuild_rejects_partial_or_below_tip_metadata() {
    for invalid_shape in [
        "manifest without checkpoint",
        "missing published commit manifest",
        "below-tip metadata gap",
    ] {
        let temp_dir = TempDir::new().expect("temporary Kura directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("primary lane storage entry");
        let _receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
        match invalid_shape {
            "manifest without checkpoint" => kura
                .remove_wsv_checkpoint_without_binding_for_tests(1)
                .expect("remove checkpoint"),
            "missing published commit manifest" => kura
                .remove_commit_manifest_without_binding_for_tests(1)
                .expect("remove commit manifest"),
            "below-tip metadata gap" => {
                kura.remove_commit_manifest_without_binding_for_tests(1)
                    .expect("remove commit manifest");
                kura.remove_wsv_checkpoint_without_binding_for_tests(1)
                    .expect("remove checkpoint");
                let parent = kura
                    .get_block(nonzero!(1_usize))
                    .expect("fixture application block");
                let mut generator = DummyBlocks {
                    blocks: vec![parent],
                };
                let successor = generator.next();
                kura.store_block(Arc::clone(&successor))
                    .expect("append exact child above Native evidence");
                assert_eq!(
                    kura.get_durable_block_hash(nonzero!(2_usize)),
                    Some(successor.hash())
                );
            }
            _ => unreachable!(),
        }
        let error = kura
            .rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect_err("partial or below-tip Native evidence must fail closed");
        assert!(
            error.to_string().contains("manifest")
                || error.to_string().contains("checkpoint")
                || error.to_string().contains("below the exact durable tip"),
            "unexpected {invalid_shape} error: {error}"
        );
    }
}

#[test]
fn native_amx_latest_index_startup_discards_unpublished_rewrite_data_temp() {
    let temp_dir = TempDir::new().expect("temporary Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("primary lane storage entry");
    let _receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    let data_path =
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
    let temporary = data_path.with_extension("norito.tmp");
    let exact_bytes = fs::read(&data_path).expect("read standalone Native receipt");
    fs::remove_file(&data_path).expect("stage crash before receipt promotion");
    fs::write(&temporary, &exact_bytes).expect("stage exact receipt publication temporary");
    drop(kura);

    let (_reopened, _) = Kura::new(&config, &lane_config)
        .expect("an exact lone publication temporary is mechanically recoverable");
    assert!(
        !temporary.exists(),
        "startup must consume the exact publication temporary"
    );
    assert_eq!(
        fs::read(&data_path).expect("read recovered standalone Native receipt"),
        exact_bytes,
        "startup must promote the exact receipt bytes without rewriting them"
    );

    let malformed_temp_dir = TempDir::new().expect("malformed temporary Kura directory");
    let malformed_config = kura_config_for_dir(&malformed_temp_dir, BLOCKS_IN_MEMORY);
    let (malformed_kura, _) =
        Kura::new(&malformed_config, &lane_config).expect("initialize malformed temporary Kura");
    let malformed_entry = malformed_kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("malformed temporary primary lane entry");
    let malformed_path = Kura::native_amx_participant_receipt_path_for_entry(
        &malformed_entry,
        &malformed_kura.store_root,
        1,
    )
    .with_extension("norito.tmp");
    fs::write(&malformed_path, b"incomplete receipt append")
        .expect("stage malformed Native publication temporary");
    drop(malformed_kura);
    let error = match Kura::new(&malformed_config, &lane_config) {
        Ok(_) => panic!("startup must reject a malformed Native publication temporary"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("decode")
            || error.to_string().contains("non-canonical")
            || error.to_string().contains("another active route")
            || error
                .to_string()
                .contains("deferred until its exact stable manifest exists"),
        "unexpected malformed Native temporary error: {error}"
    );
    assert!(
        malformed_path.exists(),
        "fail-closed startup must retain malformed temporary evidence for forensics"
    );

    for artifact_kind in ["manifest", "receipt"] {
        let oversized_temp_dir =
            TempDir::new().expect("oversized publication temporary Kura directory");
        let oversized_config = kura_config_for_dir(&oversized_temp_dir, BLOCKS_IN_MEMORY);
        let (oversized_kura, _) = Kura::new(&oversized_config, &lane_config)
            .expect("initialize oversized publication temporary Kura");
        let oversized_entry = oversized_kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("oversized publication temporary primary lane entry");
        let stable_path = match artifact_kind {
            "manifest" => Kura::native_amx_application_manifest_path_for_entry(
                &oversized_entry,
                &oversized_kura.store_root,
                1,
            ),
            "receipt" => Kura::native_amx_participant_receipt_path_for_entry(
                &oversized_entry,
                &oversized_kura.store_root,
                1,
            ),
            _ => unreachable!("fixed Native AMX artifact-kind matrix"),
        };
        let temporary = stable_path.with_extension("norito.tmp");
        assert!(
            !stable_path.exists(),
            "oversized {artifact_kind} temporary fixture must begin without stable evidence"
        );
        let oversized_len = STRICT_INIT_MAX_BLOCK_BYTES.saturating_add(1);
        fs::File::create(&temporary)
            .expect("create sparse oversized Native publication temporary")
            .set_len(oversized_len)
            .expect("size sparse oversized Native publication temporary");
        let evidence_directory = temporary
            .parent()
            .expect("oversized Native temporary has an evidence directory");
        let metadata_before = Kura::regular_sidecar_metadata_for(
            &oversized_kura.store_root,
            &temporary,
            evidence_directory,
        )
        .expect("inspect sparse oversized Native publication temporary")
        .expect("sparse oversized Native publication temporary exists");
        let store_root = oversized_kura.store_root.clone();
        drop(oversized_kura);

        let error = match Kura::new(&oversized_config, &lane_config) {
            Ok(_) => {
                panic!("startup must reject oversized canonical {artifact_kind} temporary")
            }
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("oversized") || error.to_string().contains("byte bound"),
            "unexpected oversized {artifact_kind} temporary error: {error}"
        );
        let metadata_after =
            Kura::regular_sidecar_metadata_for(&store_root, &temporary, evidence_directory)
                .expect("reinspect sparse oversized Native publication temporary")
                .expect("fail-closed startup retains oversized Native temporary");
        assert!(
            Kura::stable_sidecar_metadata_unchanged(&metadata_before, &metadata_after),
            "startup must reject an oversized {artifact_kind} temporary before mutating it"
        );
        assert_eq!(
            metadata_after.file.len(),
            oversized_len,
            "oversized {artifact_kind} temporary must retain its exact sparse length"
        );
        assert!(
            !stable_path.exists(),
            "oversized {artifact_kind} temporary must never be promoted to stable evidence"
        );
    }
}

#[test]
fn native_amx_latest_index_rebuild_rejects_conflicting_pointer() {
    let temp_dir = TempDir::new().expect("temporary Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("primary lane storage entry");
    let mut conflicting = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    conflicting.application_block_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"conflicting application block"));
    let conflicting_latest = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&conflicting);
    let latest_path =
        Kura::native_amx_participant_receipt_latest_index_path_for_entry(&entry, &kura.store_root);
    fs::write(
        &latest_path,
        norito::to_bytes(&conflicting_latest).expect("encode conflicting latest index"),
    )
    .expect("stage conflicting latest index");

    let error = kura
        .rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
        .expect_err("conflicting latest pointer must fail closed");
    assert!(
        error.to_string().contains("conflicting")
            || error.to_string().contains("unbacked")
            || error.to_string().contains("non-canonical"),
        "unexpected conflict error: {error}"
    );
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn native_amx_latest_index_startup_rebuild_rejects_symlink() {
    use std::os::unix::fs::symlink;

    let temp_dir = TempDir::new().expect("temporary Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("primary lane storage entry");
    let latest_path =
        Kura::native_amx_participant_receipt_latest_index_path_for_entry(&entry, &kura.store_root);
    drop(kura);

    let target = temp_dir.path().join("attacker-controlled-latest-index");
    fs::write(&target, b"attacker-controlled").expect("write symlink target");
    symlink(&target, &latest_path).expect("plant latest-index symlink");
    let error = match Kura::new(&config, &lane_config) {
        Ok(_) => panic!("startup must reject a symlinked Native AMX latest index"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("symlinked")
            || error.to_string().contains("non-regular")
            || error.to_string().contains("multi-link")
            || error.to_string().contains("single-link regular file"),
        "unexpected startup error: {error}"
    );

    let snapshot_regular_inventory = |directory: &Path| {
        let mut inventory = BTreeMap::new();
        for entry in fs::read_dir(directory)
            .expect("read Native AMX evidence inventory")
            .collect::<std::io::Result<Vec<_>>>()
            .expect("collect Native AMX evidence inventory")
        {
            let file_type = entry
                .file_type()
                .expect("inspect Native AMX evidence inventory entry");
            assert!(
                file_type.is_file() && !file_type.is_symlink(),
                "Native AMX publication fixture inventory must contain only regular files"
            );
            let file_name = entry
                .file_name()
                .into_string()
                .expect("Native AMX publication fixture uses UTF-8 names");
            let previous = inventory.insert(
                file_name,
                fs::read(entry.path()).expect("read Native AMX evidence inventory entry"),
            );
            assert!(
                previous.is_none(),
                "Native AMX publication fixture inventory names are unique"
            );
        }
        inventory
    };

    for artifact_kind in ["manifest", "receipt"] {
        let temp_dir = TempDir::new().expect("symlinked Native publication Kura directory");
        let external_dir = TempDir::new().expect("external symlinked Native publication directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) =
            Kura::new(&config, &lane_config).expect("initialize Native publication Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native publication primary lane storage entry");
        let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
        let manifest_path =
            Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
        let receipt_path =
            Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
        let namespace = kura
            .native_amx_evidence_namespace_for_entry(&entry)
            .expect("bind Native publication evidence namespace");
        let manifest = kura
            .read_native_amx_participant_application_manifest_from_paths_locked(
                &entry,
                1,
                &manifest_path,
                &namespace,
            )
            .expect("decode Native publication manifest fixture");
        drop(namespace);

        let evidence_directory = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root));
        let displaced_directory = temp_dir
            .path()
            .join(format!("displaced-{artifact_kind}-lane-artifacts"));
        let original_inventory = snapshot_regular_inventory(&evidence_directory);
        let omitted_name = match artifact_kind {
            "manifest" => manifest_path
                .file_name()
                .expect("manifest path has a file name"),
            "receipt" => receipt_path
                .file_name()
                .expect("receipt path has a file name"),
            _ => unreachable!("fixed Native AMX artifact-kind matrix"),
        };
        for (file_name, bytes) in &original_inventory {
            if Path::new(file_name).as_os_str() != omitted_name {
                fs::write(external_dir.path().join(file_name), bytes)
                    .expect("copy prerequisite Native evidence into external inventory");
            }
        }
        fs::write(
            external_dir.path().join("attacker-sentinel"),
            b"external Native AMX publication sentinel",
        )
        .expect("write external Native AMX publication sentinel");
        let external_inventory_before = snapshot_regular_inventory(external_dir.path());

        fs::rename(&evidence_directory, &displaced_directory)
            .expect("displace original Native AMX evidence directory");
        symlink(external_dir.path(), &evidence_directory)
            .expect("replace Native AMX evidence directory with a symlink");

        let _publication_guard = kura.prune_lock.lock();
        let error = match artifact_kind {
            "manifest" => kura
                .write_native_amx_participant_application_manifest_artifact_under_publication_guard(
                    &manifest,
                )
                .expect_err("manifest publication must reject a symlinked evidence directory"),
            "receipt" => kura
                .write_native_amx_participant_application_receipt_artifact_under_publication_guard(
                    &receipt, &manifest,
                )
                .expect_err("receipt publication must reject a symlinked evidence directory"),
            _ => unreachable!("fixed Native AMX artifact-kind matrix"),
        };
        assert!(
            error.to_string().contains("directory")
                || error.to_string().contains("symlink")
                || error.to_string().contains("canonical"),
            "unexpected symlinked {artifact_kind} publication error: {error}"
        );
        assert_eq!(
            snapshot_regular_inventory(external_dir.path()),
            external_inventory_before,
            "{artifact_kind} publication must not mutate the external sentinel or inventory"
        );
        assert_eq!(
            snapshot_regular_inventory(&displaced_directory),
            original_inventory,
            "{artifact_kind} publication must preserve displaced evidence byte-exact"
        );
    }
}

#[test]
fn roster_sidecar_roundtrip() {
    use iroha_config::base::WithOrigin;

    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
        },
        &RuntimeLaneConfig::default(),
    )
    .unwrap();

    let kp = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let peer = PeerId::new(kp.public_key().clone());
    let roster = vec![peer];
    let block_hash = store_dummy_blocks(&kura, 1)[0];
    let signers_bitmap = vec![0b0000_0001];
    let bls_aggregate_signature = vec![0xAB; 96];
    let cert = Qc {
        phase: Phase::Commit,
        subject_block_hash: block_hash,
        parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
        post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
        height: 1,
        view: 0,
        epoch: 0,
        chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: PERMISSIONED_TAG.to_string(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&roster),
        validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        validator_set: roster.clone(),
        aggregate: QcAggregate {
            signers_bitmap: signers_bitmap.clone(),
            bls_aggregate_signature: bls_aggregate_signature.clone(),
        },
    };
    let sidecar = RosterSidecar::new(1, block_hash, Some(cert.clone()), None, None);

    kura.write_roster_metadata(&sidecar);
    let got = kura.read_roster_metadata(1).expect("sidecar exists");

    assert_eq!(got.height, 1);
    assert_eq!(got.block_hash, block_hash);
    assert_eq!(got.format_label(), "roster.snapshot");
    assert_eq!(
        got.commit_qc.as_ref().map(|c| c.validator_set_hash),
        Some(HashOf::new(&roster))
    );
    assert!(got.stake_snapshot.is_none());
    assert_eq!(got.roster_snapshot(), Some(roster));
}

#[test]
fn roster_sidecar_retention_pins_genesis_across_compaction_and_restart() {
    let temp_dir = TempDir::new().expect("tempdir");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.roster_sidecar_retention =
        NonZeroUsize::new(2).expect("non-zero roster sidecar retention");
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("kura init");
    let block_hashes = store_dummy_blocks(&kura, 4);

    for (index, block_hash) in block_hashes.iter().copied().enumerate() {
        let height = u64::try_from(index.saturating_add(1)).expect("test height fits u64");
        assert!(
            kura.write_roster_metadata(&RosterSidecar::new(height, block_hash, None, None, None,)),
            "write roster sidecar at height {height}"
        );
    }

    let assert_retained_window = |kura: &Kura| {
        assert!(
            kura.read_roster_metadata(1).is_some(),
            "genesis sidecar must remain pinned outside the recent retention window"
        );
        assert!(
            kura.read_roster_metadata(2).is_none(),
            "old non-genesis sidecar must be pruned"
        );
        assert!(kura.read_roster_metadata(3).is_some());
        assert!(kura.read_roster_metadata(4).is_some());
    };
    assert_retained_window(&kura);

    drop(kura);
    let (reopened, BlockCount(block_count)) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("reopen compacted Kura");
    assert_eq!(block_count, 4);
    assert_retained_window(&reopened);
}

#[test]
fn roster_sidecar_rejects_height_mismatch() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
        },
        &RuntimeLaneConfig::default(),
    )
    .unwrap();

    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");

    let data_path = pipeline_dir.join(ROSTER_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(ROSTER_SIDECARS_INDEX_FILE);
    let block_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xBC; Hash::LENGTH]));
    let sidecar = RosterSidecar::new(2, block_hash, None, None, None);
    let payload = sidecar.encode_framed().expect("encode sidecar");
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            1,
            &payload,
            "roster sidecar",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::HeightOne,
        ),
        "append mismatched roster sidecar"
    );
    assert!(
        kura.read_roster_metadata(1).is_none(),
        "height mismatch should be rejected"
    );
}

#[test]
fn roster_sidecar_rejects_block_hash_mismatch() {
    use iroha_config::base::WithOrigin;

    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
        },
        &RuntimeLaneConfig::default(),
    )
    .unwrap();

    let mut blocks = DummyBlocks::new();
    let block = blocks.next();
    let expected_hash = block.hash();
    kura.store_block(block).expect("store block");

    let mismatch_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xDD; 32]));
    assert_ne!(expected_hash, mismatch_hash, "mismatch hash must differ");

    let sidecar = RosterSidecar::new(1, mismatch_hash, None, None, None);
    kura.write_roster_metadata(&sidecar);

    assert!(
        kura.read_roster_metadata(1).is_none(),
        "block hash mismatch should be rejected"
    );
}

#[test]
fn roster_sidecar_without_canonical_kura_hash_is_rejected_and_pruned_above_tip() {
    let temp_dir = TempDir::new().expect("tempdir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("kura init");
    let mut blocks = DummyBlocks::new();
    let block1 = blocks.next();
    let block2 = blocks.next();
    let block2_hash = block2.hash();
    kura.store_block(block1).expect("store canonical block 1");

    let stale = RosterSidecar::new(2, block2_hash, None, None, None);
    assert!(
        kura.write_roster_metadata(&stale),
        "fixture sidecar should be durably written before rollback"
    );
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    let data_path = pipeline_dir.join(ROSTER_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(ROSTER_SIDECARS_INDEX_FILE);
    assert_eq!(
        fs::metadata(&index_path).expect("roster index").len(),
        2 * PIPELINE_INDEX_ENTRY_SIZE_U64
    );
    assert!(
        fs::metadata(&data_path).expect("roster data").len() > 0,
        "height-2 fixture payload should exist before rollback"
    );
    assert!(
        kura.read_roster_metadata(2).is_none(),
        "a sidecar without any canonical Kura hash must never be exposed"
    );

    // The requested height equals the current block tip. Rollback still has to remove an
    // orphaned height+1 sidecar instead of returning early.
    kura.prune_to_height(1)
        .expect("equal-tip rollback should prune stale roster artifacts");
    assert_eq!(
        fs::metadata(&index_path)
            .expect("truncated roster index")
            .len(),
        PIPELINE_INDEX_ENTRY_SIZE_U64,
        "the index must not retain an address for height 2"
    );
    assert_eq!(
        fs::metadata(&data_path)
            .expect("compacted roster data")
            .len(),
        0,
        "the only payload was above the canonical tip and must be removed"
    );

    kura.store_block(block2).expect("store canonical block 2");
    assert!(
        kura.read_roster_metadata(2).is_none(),
        "later canonical growth must not resurrect the removed stale sidecar"
    );
}

#[test]
fn roster_sidecar_rejects_commit_qc_mismatch() {
    use iroha_config::base::WithOrigin;

    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
        },
        &RuntimeLaneConfig::default(),
    )
    .unwrap();

    let mut blocks = DummyBlocks::new();
    let block = blocks.next();
    let block_hash = block.hash();
    kura.store_block(block).expect("store block");

    let kp = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let peer = PeerId::new(kp.public_key().clone());
    let roster = vec![peer];
    let mismatch_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xEE; Hash::LENGTH]));
    assert_ne!(block_hash, mismatch_hash, "mismatch hash must differ");

    let cert = Qc {
        phase: Phase::Commit,
        subject_block_hash: mismatch_hash,
        parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
        post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
        height: 1,
        view: 0,
        epoch: 0,
        chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: PERMISSIONED_TAG.to_string(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&roster),
        validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        validator_set: roster,
        aggregate: QcAggregate {
            signers_bitmap: vec![0b0000_0001],
            bls_aggregate_signature: vec![0xAA; 96],
        },
    };
    let sidecar = RosterSidecar::new(1, block_hash, Some(cert), None, None);
    kura.write_roster_metadata(&sidecar);

    assert!(
        kura.read_roster_metadata(1).is_none(),
        "mismatched commit certificate should be rejected"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn native_amx_publication_temp_recovery_is_phase_aware_and_manifest_bound() {
    let temp_dir = TempDir::new().expect("phase-aware Native evidence Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("initialize phase-aware Native evidence Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("phase-aware Native evidence lane entry");
    let _receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    let manifest_path =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
    let receipt_path =
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
    let manifest_temp = manifest_path.with_extension("norito.tmp");
    let receipt_temp = receipt_path.with_extension("norito.tmp");
    let manifest_bytes = fs::read(&manifest_path).expect("read stable Native manifest");
    let receipt_bytes = fs::read(&receipt_path).expect("read stable Native receipt");
    fs::remove_file(&manifest_path).expect("remove stable Native manifest");
    fs::remove_file(&receipt_path).expect("remove stable Native receipt");
    fs::write(&manifest_temp, &manifest_bytes).expect("stage Native manifest temporary");
    fs::write(&receipt_temp, &receipt_bytes).expect("stage Native receipt temporary");
    std::fs::File::open(&manifest_temp)
        .expect("open Native manifest temporary")
        .sync_all()
        .expect("sync Native manifest temporary");
    std::fs::File::open(&receipt_temp)
        .expect("open Native receipt temporary")
        .sync_all()
        .expect("sync Native receipt temporary");

    let _prune_guard = kura.prune_lock.lock();
    let _canonical_chain_guard = kura.canonical_chain_lock.lock();
    let _geometry_guard = kura.lane_geometry_lock.lock();
    let _sidecar_guard = kura.sidecar_lock.lock();
    let namespace = kura
        .native_amx_evidence_namespace_for_entry(&entry)
        .expect("bind phase-aware Native evidence namespace");
    kura.recover_native_amx_evidence_publication_temp_locked(
        &entry,
        &namespace,
        NativeAmxEvidenceRecoveryPhase::ManifestPublication,
    )
    .expect("manifest phase recovers only the manifest temporary");
    assert_eq!(
        fs::read(&manifest_path).expect("read recovered Native manifest"),
        manifest_bytes
    );
    assert!(!manifest_temp.exists());
    assert!(
        receipt_temp.exists() && !receipt_path.exists(),
        "manifest phase must leave the receipt temporary unpublished"
    );

    kura.recover_native_amx_evidence_publication_temp_locked(
        &entry,
        &namespace,
        NativeAmxEvidenceRecoveryPhase::ReceiptPublication,
    )
    .expect("receipt phase recovers the manifest-bound receipt temporary");
    assert_eq!(
        fs::read(&receipt_path).expect("read recovered Native receipt"),
        receipt_bytes
    );
    assert!(!receipt_temp.exists());
    drop(namespace);
    drop(_sidecar_guard);
    drop(_geometry_guard);
    drop(_canonical_chain_guard);
    drop(_prune_guard);

    let missing_manifest_dir =
        TempDir::new().expect("missing-manifest Native evidence Kura directory");
    let missing_manifest_config = kura_config_for_dir(&missing_manifest_dir, BLOCKS_IN_MEMORY);
    let (missing_manifest_kura, _) =
        Kura::new(&missing_manifest_config, &RuntimeLaneConfig::default())
            .expect("initialize missing-manifest Native evidence Kura");
    let missing_manifest_entry = missing_manifest_kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("missing-manifest Native evidence lane entry");
    let _receipt = install_native_amx_latest_index_evidence_fixture(
        &missing_manifest_kura,
        &missing_manifest_entry,
    );
    let missing_manifest_path = Kura::native_amx_application_manifest_path_for_entry(
        &missing_manifest_entry,
        &missing_manifest_kura.store_root,
        1,
    );
    let missing_receipt_path = Kura::native_amx_participant_receipt_path_for_entry(
        &missing_manifest_entry,
        &missing_manifest_kura.store_root,
        1,
    );
    let missing_receipt_temp = missing_receipt_path.with_extension("norito.tmp");
    let missing_receipt_bytes =
        fs::read(&missing_receipt_path).expect("read receipt for missing-manifest crash");
    fs::remove_file(&missing_manifest_path).expect("remove exact stable manifest");
    fs::remove_file(&missing_receipt_path).expect("remove exact stable receipt");
    fs::write(&missing_receipt_temp, &missing_receipt_bytes)
        .expect("stage residual receipt temporary");
    std::fs::File::open(&missing_receipt_temp)
        .expect("open residual receipt temporary")
        .sync_all()
        .expect("sync residual receipt temporary");
    sync_dir(
        missing_receipt_temp
            .parent()
            .expect("missing-manifest Native evidence directory"),
    )
    .expect("sync missing-manifest Native evidence directory");
    let _prune_guard = missing_manifest_kura.prune_lock.lock();
    let _canonical_chain_guard = missing_manifest_kura.canonical_chain_lock.lock();
    let _geometry_guard = missing_manifest_kura.lane_geometry_lock.lock();
    let _sidecar_guard = missing_manifest_kura.sidecar_lock.lock();
    let namespace = missing_manifest_kura
        .native_amx_evidence_namespace_for_entry(&missing_manifest_entry)
        .expect("bind missing-manifest Native evidence namespace");
    let before = fs::read(&missing_receipt_temp)
        .expect("snapshot residual receipt temporary before startup recovery");
    let error = missing_manifest_kura
        .recover_native_amx_evidence_publication_temp_locked(
            &missing_manifest_entry,
            &namespace,
            NativeAmxEvidenceRecoveryPhase::Startup,
        )
        .expect_err("startup must not promote a receipt temporary without its manifest");
    assert!(
        error.to_string().contains("stable manifest"),
        "unexpected missing-manifest receipt recovery error: {error}"
    );
    assert_eq!(
        fs::read(&missing_receipt_temp).expect("reread deferred receipt temporary"),
        before
    );
    assert!(!missing_receipt_path.exists());
}

#[test]
#[allow(clippy::too_many_lines)]
fn native_amx_all_manifest_barrier_does_not_promote_another_routes_receipt_temp() {
    let block = crate::sumeragi::exec::result_bearing_native_manifest_block_for_tests();
    let manifest =
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(&block)
            .expect("build multi-route Native application manifest");
    assert!(
        manifest.entries().len() > 1,
        "multi-route Native barrier test requires at least two participants"
    );
    let finality_artifact_hash = HashOf::from_untyped_unchecked(Hash::new(
        b"multi-route Native barrier finality placeholder",
    ));
    let mut artifacts = Vec::new();
    for (index, manifest_entry) in manifest.entries().iter().enumerate() {
        let leaf_index = u32::try_from(index).expect("multi-route leaf index fits u32");
        let manifest_artifact = NativeAmxParticipantApplicationManifestArtifactV1 {
            version: NativeAmxParticipantApplicationManifestArtifactV1::VERSION,
            leaf: manifest_entry.leaf.clone(),
            leaf_index,
            proof: manifest
                .proof(leaf_index)
                .expect("multi-route Native manifest proof"),
            manifest_root: manifest.root(),
            manifest_leaf_count: manifest.count(),
            finality_artifact_hash,
        };
        let receipt = NativeAmxParticipantApplicationReceiptArtifact::new(
            manifest_entry,
            HashOf::new(&manifest_artifact),
            finality_artifact_hash,
        );
        artifacts.push((manifest_artifact, receipt));
    }
    let lane_count = artifacts
        .iter()
        .map(|(artifact, _)| artifact.leaf.lane_id.as_u32())
        .max()
        .expect("multi-route Native artifacts are non-empty")
        .checked_add(1)
        .and_then(NonZeroU32::new)
        .expect("multi-route Native lane bound is non-zero");
    let lanes = std::iter::once(ModelLaneConfig::default())
        .chain(
            artifacts
                .iter()
                .enumerate()
                .map(|(index, (artifact, _))| ModelLaneConfig {
                    id: artifact.leaf.lane_id,
                    dataspace_id: artifact.leaf.dataspace_id,
                    alias: format!("native-barrier-participant-{index}"),
                    ..ModelLaneConfig::default()
                }),
        )
        .collect::<Vec<_>>();
    let catalog =
        LaneCatalog::new(lane_count, lanes).expect("build multi-route Native barrier catalog");
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let temp_dir = TempDir::new().expect("multi-route Native barrier Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog)
        .expect("initialize multi-route Native barrier Kura");
    kura.restore_lane_segments(&lane_config)
        .expect("restore trusted multi-route Native barrier lane geometry");
    for (artifact, _) in &artifacts {
        let entry = kura
            .lane_storage_entry(artifact.leaf.lane_id)
            .expect("multi-route Native barrier lane entry");
        kura.install_lane_incarnation_marker_for_test(&entry, artifact.leaf.lane_incarnation, 0)
            .expect("install multi-route Native barrier incarnation");
    }

    let (residual_manifest, residual_receipt) = artifacts
        .last()
        .expect("multi-route Native barrier residual route");
    let residual_entry = kura
        .lane_storage_entry(residual_manifest.leaf.lane_id)
        .expect("residual Native barrier lane entry");
    let residual_manifest_path = Kura::native_amx_application_manifest_path_for_entry(
        &residual_entry,
        &kura.store_root,
        residual_manifest.leaf.participant_height,
    );
    let residual_receipt_path = Kura::native_amx_participant_receipt_path_for_entry(
        &residual_entry,
        &kura.store_root,
        residual_manifest.leaf.participant_height,
    );
    let residual_receipt_temp = residual_receipt_path.with_extension("norito.tmp");
    fs::write(
        &residual_manifest_path,
        residual_manifest
            .encode_framed()
            .expect("encode residual Native manifest"),
    )
    .expect("persist residual route manifest");
    let residual_receipt_bytes = residual_receipt
        .encode_framed()
        .expect("encode residual Native receipt");
    fs::write(&residual_receipt_temp, &residual_receipt_bytes)
        .expect("stage residual route receipt temporary");
    std::fs::File::open(&residual_receipt_temp)
        .expect("open residual route receipt temporary")
        .sync_all()
        .expect("sync residual route receipt temporary");
    sync_dir(
        residual_manifest_path
            .parent()
            .expect("residual Native evidence directory"),
    )
    .expect("sync residual Native evidence directory");

    let plan = NativeAmxParticipantApplicationEvidencePlan {
        application_block_height: block.header().height().get(),
        application_block_hash: block.hash(),
        executed_block_wire_hash: manifest.executed_block_wire_hash(),
        finality_artifact_hash,
        manifest_root: manifest.root(),
        manifest_leaf_count: manifest.count(),
        artifacts,
    };
    let _prune_guard = kura.prune_lock.lock();
    let error = kura
        .read_back_native_amx_plan_manifests_under_publication_guard(&plan)
        .expect_err("missing first-route manifest must stop the all-manifest barrier");
    assert!(
        error.to_string().contains("read-back is incomplete"),
        "unexpected multi-route Native manifest barrier error: {error}"
    );
    assert_eq!(
        fs::read(&residual_receipt_temp)
            .expect("reread residual receipt temporary after failed barrier"),
        residual_receipt_bytes
    );
    assert!(
        !residual_receipt_path.exists(),
        "failed all-manifest read-back must not promote another route's receipt temporary"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn native_amx_startup_repair_preflights_all_targets_then_skips_advanced_sibling() {
    let fixture = native_amx_two_route_repair_fixture();
    let route_a_dir =
        Kura::lane_artifact_dir(&fixture.entries[0].blocks_dir(&fixture.kura.store_root));
    let route_b_dir =
        Kura::lane_artifact_dir(&fixture.entries[1].blocks_dir(&fixture.kura.store_root));
    let route_b_latest = Kura::native_amx_participant_receipt_latest_index_path_for_entry(
        &fixture.entries[1],
        &fixture.kura.store_root,
    );
    let mut advanced =
        NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&fixture.plan.artifacts[1].1);
    advanced.lane_block_height = advanced
        .lane_block_height
        .checked_add(1)
        .expect("advanced sibling participant height");
    advanced.application_block_height = advanced
        .application_block_height
        .checked_add(1)
        .expect("advanced sibling carrier height");
    fs::write(
        &route_b_latest,
        norito::encode_canonical(&advanced).expect("encode advanced sibling latest index"),
    )
    .expect("install advanced sibling latest index");
    sync_dir(&route_b_dir).expect("sync advanced sibling namespace");
    let route_a_before = snapshot_regular_files_recursively(&route_a_dir);
    let route_b_before = snapshot_regular_files_recursively(&route_b_dir);

    let all_targets = fixture
        .kura
        .native_amx_participant_application_repair_target_indices(&fixture.plan, &fixture.markers)
        .expect("select both Native repair targets");
    let _publication_guard = fixture.kura.prune_lock.lock();
    let error = fixture
        .kura
        .persist_native_amx_participant_application_repair_targets_under_publication_guard(
            fixture.block.as_ref(),
            &fixture.plan,
            &all_targets,
        )
        .expect_err("an explicitly targeted advanced route must fail whole-target preflight");
    assert!(
        error.to_string().contains("stale") || error.to_string().contains("non-contiguous"),
        "unexpected advanced-target preflight error: {error}"
    );
    assert_eq!(
        snapshot_regular_files_recursively(&route_a_dir),
        route_a_before,
        "whole-target preflight must finish before the first target manifest write"
    );
    assert_eq!(
        snapshot_regular_files_recursively(&route_b_dir),
        route_b_before,
        "failed whole-target preflight must leave the advanced route untouched"
    );

    let route_a_targets = fixture
        .kura
        .native_amx_participant_application_repair_target_indices(
            &fixture.plan,
            &fixture.markers[..1],
        )
        .expect("select only State-owned route A");
    assert_eq!(
        fixture
            .kura
            .persist_native_amx_participant_application_repair_targets_under_publication_guard(
                fixture.block.as_ref(),
                &fixture.plan,
                &route_a_targets,
            )
            .expect("repair route A without consulting advanced route B"),
        1
    );
    assert_eq!(
        snapshot_regular_files_recursively(&route_b_dir),
        route_b_before,
        "A-only repair must not read-repair or mutate advanced route B"
    );
    drop(_publication_guard);
    let route_a = &fixture.markers[0];
    assert!(
        fixture
            .kura
            .read_native_amx_participant_application_receipt(
                route_a.lane_id,
                route_a.dataspace_id,
                route_a.lane_incarnation,
                route_a.lane_block_height,
            )
            .is_some(),
        "A-only repair must publish its exact durable receipt"
    );
}

#[test]
fn native_amx_startup_repair_does_not_require_retired_sibling_storage() {
    let fixture = native_amx_two_route_repair_fixture();
    let route_b_blocks = fixture.entries[1].blocks_dir(&fixture.kura.store_root);
    let retired_route_b = fixture._temp_dir.path().join("retired-native-route-b");
    fs::rename(&route_b_blocks, &retired_route_b).expect("retire route-B storage");
    let retired_before = snapshot_regular_files_recursively(&retired_route_b);
    let route_a_targets = fixture
        .kura
        .native_amx_participant_application_repair_target_indices(
            &fixture.plan,
            &fixture.markers[..1],
        )
        .expect("select route A from carrier with retired route B");
    let _publication_guard = fixture.kura.prune_lock.lock();
    assert_eq!(
        fixture
            .kura
            .persist_native_amx_participant_application_repair_targets_under_publication_guard(
                fixture.block.as_ref(),
                &fixture.plan,
                &route_a_targets,
            )
            .expect("repair route A after route B retirement"),
        1
    );
    assert_eq!(
        snapshot_regular_files_recursively(&retired_route_b),
        retired_before,
        "historical route-B storage must remain archived and untouched"
    );
    assert!(
        !route_b_blocks.exists(),
        "A-only repair must not recreate retired route-B storage"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn native_amx_startup_repair_ignores_recreated_b2_namespace_and_is_idempotent() {
    let fixture = native_amx_two_route_repair_fixture();
    let route_b = &fixture.entries[1];
    let route_b_blocks = route_b.blocks_dir(&fixture.kura.store_root);
    let archived_b1 = fixture._temp_dir.path().join("archived-native-route-b1");
    fs::rename(&route_b_blocks, &archived_b1).expect("archive Native route B1");
    fixture
        .kura
        .reconcile_lane_segments_for_testing(&[], &[], &[(route_b, route_b)])
        .expect("provision recreated Native route B2 storage");
    let incarnation_b2 = Hash::new(b"targeted Native repair recreated route B2");
    assert_ne!(incarnation_b2, fixture.markers[1].lane_incarnation);
    fixture
        .kura
        .install_lane_incarnation_marker_for_test(route_b, incarnation_b2, 100)
        .expect("activate recreated Native route B2");
    let route_b2_before = snapshot_regular_files_recursively(&route_b_blocks);
    let archived_b1_before = snapshot_regular_files_recursively(&archived_b1);

    let route_a_targets = fixture
        .kura
        .native_amx_participant_application_repair_target_indices(
            &fixture.plan,
            &fixture.markers[..1],
        )
        .expect("select only route A from historical A+B1 carrier");
    let _publication_guard = fixture.kura.prune_lock.lock();
    for attempt in 1..=2 {
        assert_eq!(
            fixture
                .kura
                .persist_native_amx_participant_application_repair_targets_under_publication_guard(
                    fixture.block.as_ref(),
                    &fixture.plan,
                    &route_a_targets,
                )
                .unwrap_or_else(|error| panic!("route-A repair attempt {attempt}: {error}")),
            1,
            "every exact repair retry reports its one State-owned target"
        );
        assert_eq!(
            snapshot_regular_files_recursively(&route_b_blocks),
            route_b2_before,
            "route-A retry {attempt} must not mutate the recreated B2 namespace"
        );
        assert_eq!(
            snapshot_regular_files_recursively(&archived_b1),
            archived_b1_before,
            "route-A retry {attempt} must not mutate archived B1 evidence"
        );
    }

    for invalid_markers in [
        Vec::new(),
        vec![fixture.markers[0].clone(), fixture.markers[0].clone()],
    ] {
        assert!(
            fixture
                .kura
                .native_amx_participant_application_repair_target_indices(
                    &fixture.plan,
                    &invalid_markers,
                )
                .is_err(),
            "empty and duplicate State marker sets must fail closed"
        );
    }
    let mut stale_b1_marker = fixture.markers[1].clone();
    stale_b1_marker.lane_incarnation = incarnation_b2;
    assert!(
        fixture
            .kura
            .native_amx_participant_application_repair_target_indices(
                &fixture.plan,
                &[stale_b1_marker],
            )
            .is_err(),
        "a B2 marker must not select the historical B1 carrier leaf"
    );
    let mut conflicting_carrier = fixture.markers[0].clone();
    conflicting_carrier.application_block_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"conflicting repair carrier"));
    assert!(
        fixture
            .kura
            .native_amx_participant_application_repair_target_indices(
                &fixture.plan,
                &[conflicting_carrier],
            )
            .is_err(),
        "a marker for another carrier must fail closed"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn native_amx_prepublication_token_rejects_every_state_frontier_drift_and_order_change() {
    let block = crate::sumeragi::exec::result_bearing_native_manifest_block_for_tests();
    let manifest =
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(&block)
            .expect("build canonical Native application manifest");
    let execution_commitment =
        ExecutionCommitment::new_with_native_amx_application_manifest_without_merge_carrier(
            Hash::new(b"Native frontier token parent state"),
            Hash::new(b"Native frontier token post state"),
            Hash::new(b"Native frontier token ordinary writes"),
            None,
            0,
            iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            manifest.root(),
            manifest.count(),
            manifest.executed_block_wire_len(),
            manifest.executed_block_wire_hash(),
        )
        .expect("build Native frontier token execution commitment");
    let roster = v2_finality_fixture_keys()
        .iter()
        .map(|keypair| ValidatorPower {
            validator: PeerId::new(keypair.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let context = HeightContext {
        network_id: crate::sumeragi::synthetic_network_id("native-frontier-token-test"),
        protocol_version: PROTOCOL_VERSION,
        height: block.header().height().get(),
        epoch: 0,
        epoch_end_height: block.header().height().get(),
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("Native frontier token quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"Native frontier token AMX context"),
        execution_policy_hash: Hash::new(b"Native frontier token execution policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0xA5; 32],
    };
    let subject = BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: block
            .canonical_proposal_wire_hash()
            .expect("hash Native frontier proposal wire"),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height: block.header().height().get(),
        view: block.header().view_change_index(),
    };
    let commit_qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    let finality = V2FinalityArtifact::new(context, subject, commit_qc, Vec::new());
    let finality_artifact_hash = HashOf::new(&finality);
    let mut artifacts = Vec::new();
    let mut identities = Vec::new();
    for (index, entry) in manifest.entries().iter().enumerate() {
        let leaf_index = u32::try_from(index).expect("Native frontier leaf index fits u32");
        let manifest_artifact = NativeAmxParticipantApplicationManifestArtifactV1 {
            version: NativeAmxParticipantApplicationManifestArtifactV1::VERSION,
            leaf: entry.leaf.clone(),
            leaf_index,
            proof: manifest.proof(leaf_index).expect("Native frontier proof"),
            manifest_root: manifest.root(),
            manifest_leaf_count: manifest.count(),
            finality_artifact_hash,
        };
        let receipt = NativeAmxParticipantApplicationReceiptArtifact::new(
            entry,
            HashOf::new(&manifest_artifact),
            finality_artifact_hash,
        );
        identities.push(
            NativeAmxParticipantApplicationPrepublicationIdentity::from_artifacts(
                &manifest_artifact,
                &receipt,
            )
            .expect("project Native frontier prepublication identity"),
        );
        artifacts.push((manifest_artifact, receipt));
    }
    let plan = NativeAmxParticipantApplicationEvidencePlan {
        application_block_height: block.header().height().get(),
        application_block_hash: block.hash(),
        executed_block_wire_hash: manifest.executed_block_wire_hash(),
        finality_artifact_hash,
        manifest_root: manifest.root(),
        manifest_leaf_count: manifest.count(),
        artifacts,
    };
    let token = NativeAmxParticipantApplicationPrepublicationToken::from_plan(&plan, identities)
        .expect("build Native frontier prepublication token");
    let expected_frontiers = manifest
        .entries()
        .iter()
        .map(|entry| {
            let leaf = &entry.leaf;
            crate::state::AppliedNativeAmxParticipantFrontierMarker {
                version: 2,
                lane_id: leaf.lane_id,
                dataspace_id: leaf.dataspace_id,
                lane_incarnation: leaf.lane_incarnation,
                lane_block_height: leaf.participant_height,
                participant_view: leaf.participant_view,
                previous_lane_block_height: leaf.predecessor_height,
                previous_lane_block_descriptor_hash: leaf.predecessor_descriptor_hash,
                lane_block_descriptor_hash: leaf.descriptor_hash,
                participant_proposal_hash: leaf.proposal_hash,
                participant_settlement_hash: leaf.settlement_hash,
                application_block_height: leaf.application_block_height,
                application_block_hash: leaf.application_block_hash,
                source_count: u64::try_from(leaf.members.len())
                    .expect("Native frontier source count fits u64"),
            }
        })
        .collect::<Vec<_>>();
    let frontiers = crate::state::State::native_amx_participant_frontier_markers(&block)
        .expect("derive Native State frontiers from the mixed-role carrier");
    assert_eq!(
        frontiers, expected_frontiers,
        "Kura prepublication identities and State must project the same exact participant evidence"
    );
    assert_eq!(
        frontiers.len(),
        2,
        "the participant-form coordinator leg must not create Kura/State evidence"
    );
    let marker_selector = Kura::blank_kura_for_testing();
    assert_eq!(
        marker_selector
            .native_amx_participant_application_repair_target_indices(&plan, &frontiers[..1],)
            .expect("select the first exact grouped-carrier State frontier"),
        vec![0],
        "one pending A marker must select only A from an authenticated grouped A+B carrier"
    );
    assert_eq!(
        marker_selector
            .native_amx_participant_application_repair_target_indices(&plan, &frontiers)
            .expect("select every exact grouped-carrier State frontier"),
        vec![0, 1],
        "the complete grouped A+B State projection must select both manifest leaves"
    );
    let receipt = block
        .execution_context()
        .and_then(|bundle| bundle.external.first())
        .and_then(|context| context.native_amx_receipt.as_ref())
        .expect("mixed-role Native AMX receipt");
    assert!(frontiers.iter().all(|frontier| {
        frontier.lane_id != receipt.lane_id
            || frontier.dataspace_id != receipt.dataspace_id
            || frontier.lane_incarnation != receipt.lane_incarnation
    }));
    assert!(token.authenticates_state_frontiers(&block, &manifest, &finality, &frontiers));

    let mutations: [fn(&mut crate::state::AppliedNativeAmxParticipantFrontierMarker); 14] = [
        |marker| marker.version ^= 1,
        |marker| marker.lane_id = LaneId::new(marker.lane_id.as_u32().wrapping_add(1)),
        |marker| {
            marker.dataspace_id = DataSpaceId::new(marker.dataspace_id.as_u64().wrapping_add(1));
        },
        |marker| marker.lane_incarnation = Hash::new(b"frontier incarnation drift"),
        |marker| marker.lane_block_height = marker.lane_block_height.saturating_add(1),
        |marker| marker.participant_view = marker.participant_view.saturating_add(1),
        |marker| {
            marker.previous_lane_block_height = marker.previous_lane_block_height.saturating_add(1);
        },
        |marker| {
            marker.previous_lane_block_descriptor_hash =
                Some(Hash::new(b"frontier predecessor drift"));
        },
        |marker| marker.lane_block_descriptor_hash = Hash::new(b"frontier descriptor drift"),
        |marker| marker.participant_proposal_hash = Hash::new(b"frontier proposal drift"),
        |marker| {
            marker.participant_settlement_hash =
                HashOf::from_untyped_unchecked(Hash::new(b"frontier settlement drift"));
        },
        |marker| {
            marker.application_block_height = marker.application_block_height.saturating_add(1);
        },
        |marker| {
            marker.application_block_hash =
                HashOf::from_untyped_unchecked(Hash::new(b"frontier block drift"));
        },
        |marker| marker.source_count = marker.source_count.saturating_add(1),
    ];
    for mutate in mutations {
        let mut drifted = frontiers.clone();
        mutate(
            drifted
                .first_mut()
                .expect("Native frontier fixture is non-empty"),
        );
        assert!(
            !token.authenticates_state_frontiers(&block, &manifest, &finality, &drifted),
            "every Native State frontier field must be exact"
        );
    }
    assert!(
        frontiers.len() > 1,
        "Native frontier order test requires multiple participant routes"
    );
    let mut reordered = frontiers.clone();
    reordered.swap(0, 1);
    assert!(!token.authenticates_state_frontiers(&block, &manifest, &finality, &reordered));
}
