fn install_native_amx_latest_index_evidence_fixture(
    kura: &Kura,
    entry: &LaneConfigEntry,
) -> NativeAmxParticipantApplicationReceiptArtifact {
    install_native_amx_evidence_fixture_heights(kura, entry, &[1])
        .into_iter()
        .next()
        .expect("one Native AMX evidence fixture")
}

fn install_native_amx_evidence_fixture_heights(
    kura: &Kura,
    entry: &LaneConfigEntry,
    participant_heights: &[u64],
) -> Vec<NativeAmxParticipantApplicationReceiptArtifact> {
    install_native_amx_evidence_fixture_heights_with_predecessor_drift(
        kura,
        entry,
        participant_heights,
        None,
    )
}

fn install_native_amx_evidence_fixture_heights_with_predecessor_drift(
    kura: &Kura,
    entry: &LaneConfigEntry,
    participant_heights: &[u64],
    predecessor_drift_height: Option<u64>,
) -> Vec<NativeAmxParticipantApplicationReceiptArtifact> {
    assert!(
        !participant_heights.is_empty()
            && participant_heights.iter().all(|height| *height > 0)
            && participant_heights
                .windows(2)
                .all(|pair| pair[0].checked_add(1) == Some(pair[1])),
        "Native AMX evidence fixture heights must be a non-zero contiguous suffix"
    );
    let block = store_dummy_block_arcs(kura, 1)
        .into_iter()
        .next()
        .expect("one durable application block");
    let application_block_height = block.header().height().get();
    let executed_block_wire = block
        .encode_wire()
        .expect("encode exact result-bearing application block wire");
    let executed_block_wire_len =
        u64::try_from(executed_block_wire.len()).expect("application block wire length fits u64");
    let executed_block_wire_hash = Hash::new(&executed_block_wire);
    let mut proposals: Vec<LaneBlockProposalV1> = Vec::with_capacity(participant_heights.len());
    let mut settlements = Vec::with_capacity(participant_heights.len());
    let mut source_ids = Vec::with_capacity(participant_heights.len());
    let mut results = Vec::with_capacity(participant_heights.len());
    let mut entrypoint_hashes = Vec::with_capacity(participant_heights.len());
    let mut leaves = Vec::with_capacity(participant_heights.len());

    for participant_height in participant_heights.iter().copied() {
        let (session, _) = sample_committed_lane_block_session_for_kura(
            entry.lane_id,
            entry.dataspace_id,
            participant_height,
        );
        let mut proposal = session.proposal;
        proposal.descriptor.proposal_height = application_block_height;
        if let Some(predecessor) = proposals.last() {
            proposal.descriptor.previous_lane_block_height =
                predecessor.descriptor.lane_block_height;
            proposal.descriptor.previous_lane_block_descriptor_hash =
                Some(predecessor.descriptor.descriptor_hash);
        }
        if predecessor_drift_height == Some(participant_height) {
            assert!(
                proposals.last().is_some(),
                "Native predecessor drift requires a retained predecessor"
            );
            proposal.descriptor.previous_lane_block_descriptor_hash = Some(Hash::new(
                b"authenticated retained Native predecessor drift",
            ));
        }
        proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
        proposal.proposal_hash = proposal.computed_proposal_hash();
        crate::lane_consensus::validate_lane_block_proposal(&proposal)
            .expect("canonical multi-height Native AMX fixture proposal");

        let mut source_id = [0x5A; Hash::LENGTH];
        source_id[..u64::BITS as usize / u8::BITS as usize]
            .copy_from_slice(&participant_height.to_le_bytes());
        let result = TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::new()));
        let entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            proposal.descriptor.accepted_transaction_hashes[0],
        );
        let settlement = LaneBlockCommitment {
            block_height: proposal.descriptor.lane_block_height,
            lane_id: proposal.descriptor.lane_id,
            lane_incarnation: proposal.descriptor.lane_incarnation,
            dataspace_id: proposal.descriptor.dataspace_id,
            tx_count: 1,
            total_local_amount: "0".parse().expect("zero quantity"),
            total_xor_due: "0".parse().expect("zero quantity"),
            total_xor_after_haircut: "0".parse().expect("zero quantity"),
            total_xor_variance: "0".parse().expect("zero quantity"),
            swap_metadata: None,
            receipts: vec![iroha_data_model::block::consensus::LaneSettlementReceipt {
                source_id,
                local_amount: "0".parse().expect("zero quantity"),
                xor_due: "0".parse().expect("zero quantity"),
                xor_after_haircut: "0".parse().expect("zero quantity"),
                xor_variance: "0".parse().expect("zero quantity"),
                timestamp_ms: application_block_height,
            }],
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        };
        let settlement_hash = iroha_data_model::nexus::compute_settlement_hash(&settlement)
            .expect("hash Native AMX fixture settlement");
        let leaf = NativeAmxApplicationManifestLeafV1 {
            version: iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            lane_id: proposal.descriptor.lane_id,
            dataspace_id: proposal.descriptor.dataspace_id,
            lane_incarnation: proposal.descriptor.lane_incarnation,
            participant_height: proposal.descriptor.lane_block_height,
            participant_view: proposal.descriptor.lane_block_view,
            predecessor_height: proposal.descriptor.previous_lane_block_height,
            predecessor_descriptor_hash: proposal.descriptor.previous_lane_block_descriptor_hash,
            descriptor_hash: proposal.descriptor.descriptor_hash,
            proposal_hash: proposal.proposal_hash,
            settlement_hash,
            members: vec![
                iroha_data_model::block::consensus_v2::NativeAmxApplicationManifestMemberV1 {
                    entrypoint_index: proposal.descriptor.accepted_candidate_indices[0],
                    source_id,
                    entrypoint_hash,
                    result_hash: result.hash(),
                },
            ],
            application_block_height,
            application_block_hash: block.hash(),
            executed_block_wire_hash,
        };
        leaf.validate()
            .expect("canonical Native AMX fixture manifest leaf");
        proposals.push(proposal);
        settlements.push(settlement);
        source_ids.push(source_id);
        results.push(result);
        entrypoint_hashes.push(entrypoint_hash);
        leaves.push(leaf);
    }
    let lane_incarnation = proposals
        .first()
        .expect("non-empty Native AMX fixture proposals")
        .descriptor
        .lane_incarnation;
    kura.install_lane_incarnation_marker_for_test(entry, lane_incarnation, 0)
        .expect("install active Native AMX participant incarnation");

    let tree = leaves.iter().map(HashOf::new).collect::<MerkleTree<_>>();
    let manifest_root = tree
        .root()
        .map(Hash::from)
        .expect("non-empty Native AMX fixture manifest root");
    let manifest_leaf_count = u32::try_from(leaves.len()).expect("fixture leaf count fits u32");
    let execution_commitment =
        ExecutionCommitment::new_with_native_amx_application_manifest_without_merge_carrier(
            Hash::new(b"Native AMX latest-index parent state"),
            Hash::new(b"Native AMX latest-index post state"),
            Hash::new(b"Native AMX latest-index ordinary writes"),
            None,
            0,
            iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            manifest_root,
            manifest_leaf_count,
            executed_block_wire_len,
            executed_block_wire_hash,
        )
        .expect("canonical Native AMX execution commitment");
    let finality =
        v2_finality_artifact_for_block_with_execution(block.as_ref(), execution_commitment);
    let _ = kura
        .store_v2_finality_artifact(&finality)
        .expect("persist exact Native AMX finality");

    let checkpoint_hash = Hash::new(b"Native AMX latest-index WSV checkpoint");
    kura.store_wsv_checkpoint(application_block_height, block.hash(), checkpoint_hash)
        .expect("persist Native AMX WSV checkpoint");
    let commit_manifest = CommitManifest::new(
        application_block_height,
        block.hash(),
        None,
        None,
        checkpoint_hash,
        None,
    )
    .with_authenticated_v2_commit_authority(&finality);
    kura.store_commit_manifest(commit_manifest)
        .expect("persist authenticated Native AMX commit manifest");

    let finality_artifact_hash = HashOf::new(&finality);
    let mut receipts = Vec::with_capacity(leaves.len());
    for (index, (((leaf, proposal), settlement), (source_id, (result, entrypoint_hash)))) in leaves
        .into_iter()
        .zip(proposals)
        .zip(settlements)
        .zip(
            source_ids
                .into_iter()
                .zip(results.into_iter().zip(entrypoint_hashes)),
        )
        .enumerate()
    {
        let leaf_index = u32::try_from(index).expect("fixture leaf index fits u32");
        let participant_height = leaf.participant_height;
        let settlement_hash = leaf.settlement_hash;
        let manifest_artifact = NativeAmxParticipantApplicationManifestArtifactV1 {
            version: NativeAmxParticipantApplicationManifestArtifactV1::VERSION,
            leaf,
            leaf_index,
            proof: tree
                .get_proof(leaf_index)
                .expect("Native AMX fixture manifest proof"),
            manifest_root,
            manifest_leaf_count,
            finality_artifact_hash,
        };
        Kura::validate_native_amx_participant_application_manifest_artifact(&manifest_artifact)
            .expect("valid Native AMX manifest sidecar");
        let receipt = NativeAmxParticipantApplicationReceiptArtifact {
            version: NativeAmxParticipantApplicationReceiptArtifact::VERSION,
            participant_proposal: proposal,
            participant_settlement: settlement,
            participant_settlement_hash: settlement_hash,
            application_block_height,
            application_block_hash: block.hash(),
            executed_block_wire_hash,
            finality_artifact_hash,
            manifest_artifact_hash: HashOf::new(&manifest_artifact),
            source_ids: vec![source_id],
            entrypoint_indices: vec![0],
            entrypoint_hashes: vec![entrypoint_hash],
            result_hashes: vec![result.hash()],
            results: vec![result],
        };
        Kura::validate_native_amx_participant_application_receipt_artifact(&receipt)
            .expect("valid Native AMX receipt sidecar");

        let manifest_path = Kura::native_amx_application_manifest_path_for_entry(
            entry,
            &kura.store_root,
            participant_height,
        );
        let receipt_path = Kura::native_amx_participant_receipt_path_for_entry(
            entry,
            &kura.store_root,
            participant_height,
        );
        fs::write(
            &manifest_path,
            manifest_artifact
                .encode_framed()
                .expect("encode Native AMX manifest"),
        )
        .expect("persist standalone Native AMX manifest");
        fs::write(
            &receipt_path,
            receipt.encode_framed().expect("encode Native AMX receipt"),
        )
        .expect("persist standalone Native AMX receipt");
        std::fs::File::open(&manifest_path)
            .expect("open standalone Native AMX manifest")
            .sync_all()
            .expect("sync standalone Native AMX manifest");
        std::fs::File::open(&receipt_path)
            .expect("open standalone Native AMX receipt")
            .sync_all()
            .expect("sync standalone Native AMX receipt");
        receipts.push(receipt);
    }
    sync_dir(Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root)).as_path())
        .expect("sync Native AMX fixture evidence directory");
    receipts
}

fn native_amx_prune_intent_for_test(
    kura: &Kura,
    entry: &LaneConfigEntry,
    protected_receipt: &NativeAmxParticipantApplicationReceiptArtifact,
    removal_heights: &[u64],
) -> NativeAmxEvidencePruneIntentV2 {
    let identity = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(protected_receipt);
    let mut entries = Vec::with_capacity(removal_heights.len().saturating_mul(2));
    for participant_height in removal_heights {
        let manifest_path = Kura::native_amx_application_manifest_path_for_entry(
            entry,
            &kura.store_root,
            *participant_height,
        );
        let receipt_path = Kura::native_amx_participant_receipt_path_for_entry(
            entry,
            &kura.store_root,
            *participant_height,
        );
        entries.push(NativeAmxEvidencePruneEntryV2 {
            kind: NativeAmxEvidencePruneIntentV2::MANIFEST_KIND,
            participant_height: *participant_height,
            artifact_hash: Hash::new(
                fs::read(&manifest_path).expect("read Native prune test manifest"),
            ),
        });
        entries.push(NativeAmxEvidencePruneEntryV2 {
            kind: NativeAmxEvidencePruneIntentV2::RECEIPT_KIND,
            participant_height: *participant_height,
            artifact_hash: Hash::new(
                fs::read(&receipt_path).expect("read Native prune test receipt"),
            ),
        });
    }
    NativeAmxEvidencePruneIntentV2 {
        version: NativeAmxEvidencePruneIntentV2::VERSION,
        lane_id: entry.lane_id,
        dataspace_id: entry.dataspace_id,
        lane_incarnation: identity.lane_incarnation,
        protected_latest: NativeAmxEvidencePruneProtectedLatestV2 {
            identity,
            receipt_artifact_hash: HashOf::new(protected_receipt),
        },
        entries,
    }
}

struct NativeAmxTwoRouteRepairFixture {
    _temp_dir: TempDir,
    kura: Arc<Kura>,
    block: Arc<SignedBlock>,
    plan: NativeAmxParticipantApplicationEvidencePlan,
    markers: Vec<crate::state::AppliedNativeAmxParticipantFrontierMarker>,
    entries: [LaneConfigEntry; 2],
}

#[allow(clippy::too_many_lines)]
fn native_amx_two_route_repair_fixture() -> NativeAmxTwoRouteRepairFixture {
    let route_configs = [
        (LaneId::new(2), DataSpaceId::new(8)),
        (LaneId::new(3), DataSpaceId::new(9)),
    ];
    let lanes =
        std::iter::once(ModelLaneConfig::default())
            .chain(route_configs.iter().copied().enumerate().map(
                |(index, (lane_id, dataspace_id))| ModelLaneConfig {
                    id: lane_id,
                    dataspace_id,
                    alias: format!("native-targeted-repair-{index}"),
                    ..ModelLaneConfig::default()
                },
            ))
            .collect::<Vec<_>>();
    let catalog = LaneCatalog::new(
        NonZeroU32::new(4).expect("two-route Native lane bound"),
        lanes,
    )
    .expect("build two-route Native repair catalog");
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let temp_dir = TempDir::new().expect("two-route Native repair Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog)
        .expect("initialize two-route Native repair Kura");
    kura.restore_lane_segments(&lane_config)
        .expect("restore trusted two-route Native repair lane geometry");
    let entries = route_configs.map(|(lane_id, _)| {
        kura.lane_storage_entry(lane_id)
            .expect("two-route Native repair lane entry")
    });
    let block = store_dummy_block_arcs(&kura, 1)
        .into_iter()
        .next()
        .expect("one durable two-route Native carrier");
    let application_block_height = block.header().height().get();
    let application_block_hash = block.hash();
    let executed_block_wire = block
        .encode_wire()
        .expect("encode two-route Native carrier wire");
    let executed_block_wire_len =
        u64::try_from(executed_block_wire.len()).expect("two-route carrier length fits u64");
    let executed_block_wire_hash = Hash::new(&executed_block_wire);

    let mut route_evidence = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let (session, _) =
            sample_committed_lane_block_session_for_kura(entry.lane_id, entry.dataspace_id, 1);
        let mut proposal = session.proposal;
        proposal.descriptor.proposal_height = application_block_height;
        proposal.descriptor.previous_lane_block_height = 0;
        proposal.descriptor.previous_lane_block_descriptor_hash = None;
        proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
        proposal.proposal_hash = proposal.computed_proposal_hash();
        crate::lane_consensus::validate_lane_block_proposal(&proposal)
            .expect("canonical two-route Native proposal");
        kura.install_lane_incarnation_marker_for_test(
            entry,
            proposal.descriptor.lane_incarnation,
            0,
        )
        .expect("install two-route Native incarnation");

        let mut source_id = [0_u8; Hash::LENGTH];
        source_id[0] = u8::try_from(index + 1).expect("two-route source index fits u8");
        let entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            proposal.descriptor.accepted_transaction_hashes[0],
        );
        let result =
            TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::default()));
        let settlement = LaneBlockCommitment {
            block_height: proposal.descriptor.lane_block_height,
            lane_id: proposal.descriptor.lane_id,
            lane_incarnation: proposal.descriptor.lane_incarnation,
            dataspace_id: proposal.descriptor.dataspace_id,
            tx_count: 1,
            total_local_amount: "0".parse().expect("zero quantity"),
            total_xor_due: "0".parse().expect("zero quantity"),
            total_xor_after_haircut: "0".parse().expect("zero quantity"),
            total_xor_variance: "0".parse().expect("zero quantity"),
            swap_metadata: None,
            receipts: vec![iroha_data_model::block::consensus::LaneSettlementReceipt {
                source_id,
                local_amount: "0".parse().expect("zero quantity"),
                xor_due: "0".parse().expect("zero quantity"),
                xor_after_haircut: "0".parse().expect("zero quantity"),
                xor_variance: "0".parse().expect("zero quantity"),
                timestamp_ms: application_block_height,
            }],
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        };
        let settlement_hash = iroha_data_model::nexus::compute_settlement_hash(&settlement)
            .expect("hash two-route Native settlement");
        let leaf = NativeAmxApplicationManifestLeafV1 {
            version: iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            lane_id: proposal.descriptor.lane_id,
            dataspace_id: proposal.descriptor.dataspace_id,
            lane_incarnation: proposal.descriptor.lane_incarnation,
            participant_height: proposal.descriptor.lane_block_height,
            participant_view: proposal.descriptor.lane_block_view,
            predecessor_height: proposal.descriptor.previous_lane_block_height,
            predecessor_descriptor_hash: proposal.descriptor.previous_lane_block_descriptor_hash,
            descriptor_hash: proposal.descriptor.descriptor_hash,
            proposal_hash: proposal.proposal_hash,
            settlement_hash,
            members: vec![
                iroha_data_model::block::consensus_v2::NativeAmxApplicationManifestMemberV1 {
                    entrypoint_index: proposal.descriptor.accepted_candidate_indices[0],
                    source_id,
                    entrypoint_hash,
                    result_hash: result.hash(),
                },
            ],
            application_block_height,
            application_block_hash,
            executed_block_wire_hash,
        };
        leaf.validate().expect("canonical two-route Native leaf");
        route_evidence.push((
            leaf,
            proposal,
            settlement,
            source_id,
            entrypoint_hash,
            result,
        ));
    }

    let tree = route_evidence
        .iter()
        .map(|(leaf, ..)| HashOf::new(leaf))
        .collect::<MerkleTree<_>>();
    let manifest_root = tree
        .root()
        .map(Hash::from)
        .expect("two-route Native manifest root");
    let manifest_leaf_count =
        u32::try_from(route_evidence.len()).expect("two-route leaf count fits u32");
    let execution_commitment =
        ExecutionCommitment::new_with_native_amx_application_manifest_without_merge_carrier(
            Hash::new(b"two-route targeted repair parent state"),
            Hash::new(b"two-route targeted repair post state"),
            Hash::new(b"two-route targeted repair ordinary writes"),
            None,
            0,
            iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            manifest_root,
            manifest_leaf_count,
            executed_block_wire_len,
            executed_block_wire_hash,
        )
        .expect("build two-route Native execution commitment");
    let finality =
        v2_finality_artifact_for_block_with_execution(block.as_ref(), execution_commitment);
    let _finality_commit_receipt = kura
        .store_v2_finality_artifact(&finality)
        .expect("persist two-route Native finality");
    let checkpoint_hash = Hash::new(b"two-route targeted repair WSV checkpoint");
    kura.store_wsv_checkpoint(
        application_block_height,
        application_block_hash,
        checkpoint_hash,
    )
    .expect("persist two-route Native checkpoint");
    kura.store_commit_manifest(
        CommitManifest::new(
            application_block_height,
            application_block_hash,
            None,
            None,
            checkpoint_hash,
            None,
        )
        .with_authenticated_v2_commit_authority(&finality),
    )
    .expect("persist two-route Native commit manifest");

    let finality_artifact_hash = HashOf::new(&finality);
    let mut artifacts = Vec::with_capacity(route_evidence.len());
    let mut markers = Vec::with_capacity(route_evidence.len());
    for (index, (leaf, proposal, settlement, source_id, entrypoint_hash, result)) in
        route_evidence.into_iter().enumerate()
    {
        let leaf_index = u32::try_from(index).expect("two-route leaf index fits u32");
        let manifest = NativeAmxParticipantApplicationManifestArtifactV1 {
            version: NativeAmxParticipantApplicationManifestArtifactV1::VERSION,
            leaf: leaf.clone(),
            leaf_index,
            proof: tree
                .get_proof(leaf_index)
                .expect("two-route Native Merkle proof"),
            manifest_root,
            manifest_leaf_count,
            finality_artifact_hash,
        };
        let receipt = NativeAmxParticipantApplicationReceiptArtifact {
            version: NativeAmxParticipantApplicationReceiptArtifact::VERSION,
            participant_proposal: proposal,
            participant_settlement: settlement,
            participant_settlement_hash: leaf.settlement_hash,
            application_block_height,
            application_block_hash,
            executed_block_wire_hash,
            finality_artifact_hash,
            manifest_artifact_hash: HashOf::new(&manifest),
            source_ids: vec![source_id],
            entrypoint_indices: vec![leaf.members[0].entrypoint_index],
            entrypoint_hashes: vec![entrypoint_hash],
            result_hashes: vec![result.hash()],
            results: vec![result],
        };
        Kura::validate_native_amx_participant_application_manifest_artifact(&manifest)
            .expect("validate two-route Native manifest artifact");
        Kura::validate_native_amx_participant_application_receipt_artifact(&receipt)
            .expect("validate two-route Native receipt artifact");
        markers.push(crate::state::AppliedNativeAmxParticipantFrontierMarker {
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
            application_block_height,
            application_block_hash,
            source_count: u64::try_from(leaf.members.len())
                .expect("two-route source count fits u64"),
        });
        artifacts.push((manifest, receipt));
    }
    let plan = NativeAmxParticipantApplicationEvidencePlan {
        application_block_height,
        application_block_hash,
        executed_block_wire_hash,
        finality_artifact_hash,
        manifest_root,
        manifest_leaf_count,
        artifacts,
    };
    NativeAmxTwoRouteRepairFixture {
        _temp_dir: temp_dir,
        kura,
        block,
        plan,
        markers,
        entries,
    }
}

fn snapshot_regular_files_recursively(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut entries = fs::read_dir(directory)
            .expect("read snapshot directory")
            .map(|entry| entry.expect("read snapshot entry"))
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).expect("inspect snapshot entry");
            assert!(!metadata.file_type().is_symlink());
            if metadata.is_dir() {
                collect(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root)
                        .expect("snapshot file is below root")
                        .to_path_buf(),
                    fs::read(&path).expect("read snapshot file"),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    collect(root, root, &mut files);
    files
}

fn native_amx_latest_index_test_paths(kura: &Kura, entry: &LaneConfigEntry) -> (PathBuf, PathBuf) {
    let stable =
        Kura::native_amx_participant_receipt_latest_index_path_for_entry(entry, &kura.store_root);
    let temporary = stable
        .parent()
        .expect("Native AMX latest-index test path has a parent")
        .join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE);
    (stable, temporary)
}

fn write_synced_native_amx_test_file(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("write Native AMX test artifact");
    std::fs::File::open(path)
        .expect("open Native AMX test artifact")
        .sync_all()
        .expect("sync Native AMX test artifact");
    sync_dir(
        path.parent()
            .expect("Native AMX test artifact has a parent"),
    )
    .expect("sync Native AMX test artifact directory");
}

#[test]
fn native_amx_latest_index_startup_rebuild_rejects_unbacked_corruption() {
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

    for corrupt in [
        vec![0xA5],
        vec![0x5A; NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_MAX_BYTES + 1],
    ] {
        fs::write(&latest_path, corrupt).expect("stage corrupt derived latest index");
        let error = match Kura::new(&config, &lane_config) {
            Ok(_) => panic!("startup must reject an unbacked corrupt latest index"),
            Err(error) => error,
        };
        assert!(
            latest_path.exists(),
            "fail-closed reconstruction must not erase forensic evidence"
        );
        assert!(
            error.to_string().contains("latest index") || error.to_string().contains("byte limit"),
            "unexpected startup error: {error}"
        );
    }

    for corrupt_kind in ["older manifest", "older receipt"] {
        let temp_dir = TempDir::new().expect("temporary Kura directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("primary lane storage entry");
        let corrupt_path = match corrupt_kind {
            "older manifest" => {
                Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 2)
            }
            "older receipt" => {
                Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 2)
            }
            _ => unreachable!(),
        };
        fs::write(&corrupt_path, [0xA5]).expect("stage malformed standalone Native evidence");
        let _latest = install_native_amx_latest_index_evidence_fixture(&kura, &entry);

        let error = kura
            .rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect_err("startup must decode every retained Native evidence payload");
        assert!(
            error.to_string().contains("decode")
                || error.to_string().contains("non-canonical")
                || error.to_string().contains("another active route"),
            "unexpected {corrupt_kind} history error: {error}"
        );
        assert!(
            corrupt_path.exists(),
            "fail-closed validation must retain the corrupt older evidence for forensics"
        );
    }
}

#[test]
fn native_amx_latest_index_startup_rejects_legacy_v1_filename() {
    let temp_dir = TempDir::new().expect("temporary Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("primary lane storage entry");
    let evidence_directory = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root));
    let legacy_name = ["native_amx_participant_receipts.latest_v", "1", ".norito"].concat();
    let legacy_path = evidence_directory.join(legacy_name);
    fs::write(&legacy_path, [0xA5]).expect("stage unsupported legacy latest pointer");
    drop(kura);

    let error = match Kura::new(&config, &lane_config) {
        Ok(_) => panic!("startup must reject a legacy Native latest-index filename"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("unexpected or legacy"),
        "unexpected legacy latest-index error: {error}"
    );
    assert!(
        legacy_path.exists(),
        "fail-closed startup must retain the legacy pointer for forensics"
    );
}

#[test]
fn native_amx_latest_index_startup_rejects_oversized_append_indexes_before_scanning() {
    for artifact_kind in ["manifest", "receipt"] {
        let temp_dir = TempDir::new().expect("temporary Kura directory");
        let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        config.roster_sidecar_retention =
            NonZeroUsize::new(2).expect("small Native history test bound");
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("primary lane storage entry");
        let store_root = kura.store_root.clone();
        let artifact_path = |height| match artifact_kind {
            "manifest" => {
                Kura::native_amx_application_manifest_path_for_entry(&entry, &store_root, height)
            }
            "receipt" => {
                Kura::native_amx_participant_receipt_path_for_entry(&entry, &store_root, height)
            }
            _ => unreachable!(),
        };
        let hostile_entries = config.roster_sidecar_retention.get().saturating_add(2);
        for height in 1..=hostile_entries {
            fs::write(
                artifact_path(u64::try_from(height).expect("hostile height fits u64")),
                [0xA5],
            )
            .expect("stage excess standalone Native record");
        }
        drop(kura);

        let error = match Kura::new(&config, &lane_config) {
            Ok(_) => panic!("startup must reject excess {artifact_kind} records"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("retained record bound"),
            "unexpected {artifact_kind} startup bound error: {error}"
        );
    }

    let temp_dir = TempDir::new().expect("bounded Native compaction Kura directory");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.roster_sidecar_retention =
        NonZeroUsize::new(2).expect("small Native compaction retention");
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) =
        Kura::new(&config, &lane_config).expect("initialize bounded Native compaction Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("bounded Native compaction primary lane entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
    let newest = receipts.last().expect("newest Native compaction receipt");
    let latest_path =
        Kura::native_amx_participant_receipt_latest_index_path_for_entry(&entry, &kura.store_root);
    let oldest_manifest =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
    let oldest_receipt =
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
    drop(kura);

    let (reopened, _) = Kura::new(&config, &lane_config)
        .expect("retention plus one fully valid pair must compact at startup");
    let reopened_entry = reopened
        .lane_storage_entry(LaneId::SINGLE)
        .expect("reopened bounded Native compaction lane entry");
    assert!(
        !oldest_manifest.exists() && !oldest_receipt.exists(),
        "startup compaction must remove the oldest complete pair"
    );
    for retained_height in [2_u64, 3] {
        assert!(
            Kura::native_amx_application_manifest_path_for_entry(
                &reopened_entry,
                &reopened.store_root,
                retained_height,
            )
            .exists()
                && Kura::native_amx_participant_receipt_path_for_entry(
                    &reopened_entry,
                    &reopened.store_root,
                    retained_height,
                )
                .exists(),
            "startup compaction must retain complete height {retained_height} evidence"
        );
    }
    let latest = reopened
        .decode_native_amx_participant_receipt_latest_index(&reopened_entry, &latest_path)
        .expect("decode rebuilt Native compaction latest pointer")
        .expect("rebuilt Native compaction latest pointer exists");
    assert!(
        latest.matches_receipt(newest) && latest.lane_block_height == 3,
        "startup compaction must preserve and point to the exact newest receipt"
    );
    assert_eq!(
        reopened
            .disk_usage_bytes()
            .expect("read cached disk accounting after Native compaction"),
        reopened
            .kura_total_disk_usage_bytes()
            .expect("scan exact disk accounting after Native compaction"),
        "Native compaction must reconcile cached and exact disk accounting"
    );
    let latest_bytes = fs::read(&latest_path).expect("read compacted Native latest-pointer bytes");
    let retained_bytes = [2_u64, 3]
        .into_iter()
        .flat_map(|height| {
            [
                Kura::native_amx_application_manifest_path_for_entry(
                    &reopened_entry,
                    &reopened.store_root,
                    height,
                ),
                Kura::native_amx_participant_receipt_path_for_entry(
                    &reopened_entry,
                    &reopened.store_root,
                    height,
                ),
            ]
        })
        .map(|path| fs::read(path).expect("read retained Native compaction evidence"))
        .collect::<Vec<_>>();
    let exact_usage = reopened
        .kura_total_disk_usage_bytes()
        .expect("scan exact Native compaction usage before idempotent reopen");
    drop(reopened);

    let (reopened_again, _) = Kura::new(&config, &lane_config)
        .expect("Native compaction startup repair must be idempotent");
    let reopened_again_entry = reopened_again
        .lane_storage_entry(LaneId::SINGLE)
        .expect("twice-reopened bounded Native compaction lane entry");
    assert_eq!(
        fs::read(&latest_path).expect("reread compacted Native latest pointer"),
        latest_bytes,
        "idempotent startup must preserve the exact latest-pointer bytes"
    );
    let retained_bytes_after = [2_u64, 3]
        .into_iter()
        .flat_map(|height| {
            [
                Kura::native_amx_application_manifest_path_for_entry(
                    &reopened_again_entry,
                    &reopened_again.store_root,
                    height,
                ),
                Kura::native_amx_participant_receipt_path_for_entry(
                    &reopened_again_entry,
                    &reopened_again.store_root,
                    height,
                ),
            ]
        })
        .map(|path| fs::read(path).expect("reread retained Native compaction evidence"))
        .collect::<Vec<_>>();
    assert_eq!(
        retained_bytes_after, retained_bytes,
        "idempotent startup must preserve every retained Native evidence byte"
    );
    assert_eq!(
        reopened_again
            .disk_usage_bytes()
            .expect("read cached usage after idempotent Native reopen"),
        exact_usage
    );
    assert_eq!(
        reopened_again
            .kura_total_disk_usage_bytes()
            .expect("scan exact usage after idempotent Native reopen"),
        exact_usage
    );
}

#[test]
fn native_amx_latest_index_startup_rejects_oversized_aggregate_data_before_scanning() {
    for artifact_kind in ["manifest", "receipt"] {
        let temp_dir = TempDir::new().expect("temporary Kura directory");
        let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        config.roster_sidecar_retention =
            NonZeroUsize::new(2).expect("small Native history test bound");
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("primary lane storage entry");
        let store_root = kura.store_root.clone();
        let artifact_path = |height| match artifact_kind {
            "manifest" => {
                Kura::native_amx_application_manifest_path_for_entry(&entry, &store_root, height)
            }
            "receipt" => {
                Kura::native_amx_participant_receipt_path_for_entry(&entry, &store_root, height)
            }
            _ => unreachable!(),
        };
        drop(kura);

        for (height, hostile_len) in [
            (1, STRICT_INIT_MAX_BLOCK_BYTES),
            (2, STRICT_INIT_MAX_BLOCK_BYTES),
            (3, 1),
        ] {
            fs::File::create(artifact_path(height))
                .expect("create sparse hostile Native evidence file")
                .set_len(hostile_len)
                .expect("size sparse hostile Native evidence file");
        }

        let error = match Kura::new(&config, &lane_config) {
            Ok(_) => panic!("startup must reject oversized {artifact_kind} aggregate data"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("aggregate byte bound"),
            "unexpected {artifact_kind} aggregate bound error: {error}"
        );
    }
}

#[test]
fn native_amx_latest_index_startup_truncates_unindexed_append_tail() {
    for artifact_kind in ["manifest", "receipt"] {
        let temp_dir = TempDir::new().expect("temporary Kura directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("primary lane storage entry");
        let _receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
        let data_path = match artifact_kind {
            "manifest" => {
                Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1)
            }
            "receipt" => {
                Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1)
            }
            _ => unreachable!(),
        };
        let stable_bytes = fs::read(&data_path).expect("read stable Native evidence");
        let temp_path = data_path.with_extension("norito.tmp");
        fs::write(&temp_path, &stable_bytes)
            .expect("stage same-height Native publication temporary");
        std::fs::File::open(&temp_path)
            .expect("open Native publication temporary")
            .sync_all()
            .expect("sync Native publication temporary");
        drop(kura);

        let (_reopened, _) = Kura::new(&config, &lane_config)
            .expect("startup must mechanically recover a duplicate publication temporary");
        assert_eq!(
            fs::read(&data_path).expect("read recovered Native evidence"),
            stable_bytes,
            "{artifact_kind} startup repair must preserve exact stable bytes"
        );
        assert!(
            !temp_path.exists(),
            "{artifact_kind} startup repair must remove the exact duplicate temporary"
        );
    }

    for crash_stage in [
        "temp-only",
        "stable-before-delete",
        "after-manifest-unlink",
        "after-both-unlinks",
        "stable-plus-identical-temp",
        "pointerless-stable-before-delete",
    ] {
        let temp_dir = TempDir::new().expect("Native prune-journal crash Kura directory");
        let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        config.roster_sidecar_retention =
            NonZeroUsize::new(2).expect("small Native prune-journal retention");
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) =
            Kura::new(&config, &lane_config).expect("initialize Native prune-journal Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("Native prune-journal primary lane entry");
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
        let newest = receipts
            .last()
            .expect("newest Native prune-journal receipt");
        let latest = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(newest);
        let latest_path = Kura::native_amx_participant_receipt_latest_index_path_for_entry(
            &entry,
            &kura.store_root,
        );
        let latest_bytes =
            norito::to_bytes(&latest).expect("encode Native prune-journal latest pointer");
        if crash_stage != "pointerless-stable-before-delete" {
            fs::write(&latest_path, &latest_bytes)
                .expect("persist Native prune-journal latest pointer");
            std::fs::File::open(&latest_path)
                .expect("open Native prune-journal latest pointer")
                .sync_all()
                .expect("sync Native prune-journal latest pointer");
        }

        let manifest_path =
            Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
        let receipt_path =
            Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
        let intent = NativeAmxEvidencePruneIntentV2 {
            version: NativeAmxEvidencePruneIntentV2::VERSION,
            lane_id: entry.lane_id,
            dataspace_id: entry.dataspace_id,
            lane_incarnation: latest.lane_incarnation,
            protected_latest: NativeAmxEvidencePruneProtectedLatestV2 {
                identity: latest,
                receipt_artifact_hash: HashOf::new(newest),
            },
            entries: vec![
                NativeAmxEvidencePruneEntryV2 {
                    kind: NativeAmxEvidencePruneIntentV2::MANIFEST_KIND,
                    participant_height: 1,
                    artifact_hash: Hash::new(
                        fs::read(&manifest_path)
                            .expect("read oldest Native manifest before crash staging"),
                    ),
                },
                NativeAmxEvidencePruneEntryV2 {
                    kind: NativeAmxEvidencePruneIntentV2::RECEIPT_KIND,
                    participant_height: 1,
                    artifact_hash: Hash::new(
                        fs::read(&receipt_path)
                            .expect("read oldest Native receipt before crash staging"),
                    ),
                },
            ],
        };
        let intent_bytes = norito::to_bytes(&intent).expect("encode Native prune-journal intent");
        let evidence_directory = manifest_path
            .parent()
            .expect("Native prune-journal evidence directory");
        let intent_path = evidence_directory.join(NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE);
        let intent_temp_path = evidence_directory.join(NATIVE_AMX_EVIDENCE_PRUNE_INTENT_TEMP_FILE);
        match crash_stage {
            "temp-only" => {
                fs::write(&intent_temp_path, &intent_bytes)
                    .expect("stage temporary-only Native prune intent");
                std::fs::File::open(&intent_temp_path)
                    .expect("open temporary-only Native prune intent")
                    .sync_all()
                    .expect("sync temporary-only Native prune intent");
            }
            "stable-before-delete" => {
                fs::write(&intent_path, &intent_bytes)
                    .expect("stage stable Native prune intent before deletion");
                std::fs::File::open(&intent_path)
                    .expect("open stable Native prune intent before deletion")
                    .sync_all()
                    .expect("sync stable Native prune intent before deletion");
            }
            "after-manifest-unlink" => {
                fs::write(&intent_path, &intent_bytes)
                    .expect("stage stable Native prune intent before manifest unlink");
                std::fs::File::open(&intent_path)
                    .expect("open stable Native prune intent before manifest unlink")
                    .sync_all()
                    .expect("sync stable Native prune intent before manifest unlink");
                fs::remove_file(&manifest_path).expect("stage crash after Native manifest unlink");
            }
            "after-both-unlinks" => {
                fs::write(&intent_path, &intent_bytes)
                    .expect("stage stable Native prune intent before pair unlink");
                std::fs::File::open(&intent_path)
                    .expect("open stable Native prune intent before pair unlink")
                    .sync_all()
                    .expect("sync stable Native prune intent before pair unlink");
                fs::remove_file(&manifest_path).expect("stage crash after Native manifest unlink");
                fs::remove_file(&receipt_path).expect("stage crash after Native receipt unlink");
            }
            "stable-plus-identical-temp" => {
                fs::write(&intent_path, &intent_bytes).expect("stage stable Native prune intent");
                fs::write(&intent_temp_path, &intent_bytes)
                    .expect("stage identical Native prune-intent temporary");
                std::fs::File::open(&intent_path)
                    .expect("open stable Native prune intent")
                    .sync_all()
                    .expect("sync stable Native prune intent");
                std::fs::File::open(&intent_temp_path)
                    .expect("open identical Native prune-intent temporary")
                    .sync_all()
                    .expect("sync identical Native prune-intent temporary");
            }
            "pointerless-stable-before-delete" => {
                fs::write(&intent_path, &intent_bytes)
                    .expect("stage pointerless stable Native prune intent");
                std::fs::File::open(&intent_path)
                    .expect("open pointerless stable Native prune intent")
                    .sync_all()
                    .expect("sync pointerless stable Native prune intent");
                assert!(
                    !latest_path.exists(),
                    "pointerless prune recovery must not depend on a derived pointer"
                );
            }
            _ => unreachable!("fixed Native prune-journal crash matrix"),
        }
        sync_dir(evidence_directory).expect("sync staged Native prune-journal crash boundary");
        drop(kura);

        let (reopened, _) = Kura::new(&config, &lane_config)
            .unwrap_or_else(|error| panic!("{crash_stage} recovery failed: {error}"));
        let reopened_entry = reopened
            .lane_storage_entry(LaneId::SINGLE)
            .expect("reopened Native prune-journal lane entry");
        assert!(
            !manifest_path.exists()
                && !receipt_path.exists()
                && !intent_path.exists()
                && !intent_temp_path.exists(),
            "{crash_stage} recovery must finish the exact pair deletion and clear its journal"
        );
        for retained_height in [2_u64, 3] {
            assert!(
                Kura::native_amx_application_manifest_path_for_entry(
                    &reopened_entry,
                    &reopened.store_root,
                    retained_height,
                )
                .exists()
                    && Kura::native_amx_participant_receipt_path_for_entry(
                        &reopened_entry,
                        &reopened.store_root,
                        retained_height,
                    )
                    .exists(),
                "{crash_stage} recovery must retain complete height {retained_height} evidence"
            );
        }
        assert_eq!(
            fs::read(&latest_path).expect("read recovered Native latest pointer"),
            latest_bytes,
            "{crash_stage} recovery must preserve the exact latest pointer"
        );
        let decoded_latest = reopened
            .decode_native_amx_participant_receipt_latest_index(&reopened_entry, &latest_path)
            .expect("decode recovered Native latest pointer")
            .expect("recovered Native latest pointer exists");
        assert!(
            decoded_latest.matches_receipt(newest) && decoded_latest.lane_block_height == 3,
            "{crash_stage} recovery must keep the exact newest receipt protected"
        );
        let exact_usage = reopened
            .kura_total_disk_usage_bytes()
            .expect("scan exact usage after Native prune-journal recovery");
        assert_eq!(
            reopened
                .disk_usage_bytes()
                .expect("read cached usage after Native prune-journal recovery"),
            exact_usage,
            "{crash_stage} recovery must reconcile exact disk accounting"
        );
        drop(reopened);

        let (reopened_again, _) = Kura::new(&config, &lane_config)
            .unwrap_or_else(|error| panic!("{crash_stage} idempotent reopen failed: {error}"));
        assert_eq!(
            reopened_again
                .disk_usage_bytes()
                .expect("read cached usage after idempotent prune-journal reopen"),
            exact_usage,
            "{crash_stage} second reopen must not change disk accounting"
        );
        assert_eq!(
            reopened_again
                .kura_total_disk_usage_bytes()
                .expect("scan exact usage after idempotent prune-journal reopen"),
            exact_usage
        );
        assert_eq!(
            fs::read(&latest_path).expect("reread recovered Native latest pointer"),
            latest_bytes,
            "{crash_stage} second reopen must be byte-idempotent"
        );
    }

    let temp_dir = TempDir::new().expect("latest-protected Native prune Kura directory");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.roster_sidecar_retention =
        NonZeroUsize::new(2).expect("latest-protected Native prune retention");
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) =
        Kura::new(&config, &lane_config).expect("initialize latest-protected Native prune Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("latest-protected Native prune lane entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
    let newest = receipts.last().expect("latest-protected Native receipt");
    let latest = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(newest);
    let latest_path =
        Kura::native_amx_participant_receipt_latest_index_path_for_entry(&entry, &kura.store_root);
    fs::write(
        &latest_path,
        norito::to_bytes(&latest).expect("encode latest-protected Native pointer"),
    )
    .expect("persist latest-protected Native pointer");
    let newest_manifest =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 2);
    let newest_receipt =
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 2);
    let oldest_manifest =
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
    let oldest_receipt =
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
    let oldest_manifest_bytes = fs::read(&oldest_manifest).expect("read oldest Native manifest");
    let oldest_receipt_bytes = fs::read(&oldest_receipt).expect("read oldest Native receipt");
    let protected_manifest_bytes =
        fs::read(&newest_manifest).expect("read protected latest Native manifest");
    let protected_receipt_bytes =
        fs::read(&newest_receipt).expect("read protected latest Native receipt");
    let hostile = NativeAmxEvidencePruneIntentV2 {
        version: NativeAmxEvidencePruneIntentV2::VERSION,
        lane_id: entry.lane_id,
        dataspace_id: entry.dataspace_id,
        lane_incarnation: latest.lane_incarnation,
        protected_latest: NativeAmxEvidencePruneProtectedLatestV2 {
            identity: latest,
            receipt_artifact_hash: HashOf::new(newest),
        },
        entries: vec![
            NativeAmxEvidencePruneEntryV2 {
                kind: NativeAmxEvidencePruneIntentV2::MANIFEST_KIND,
                participant_height: 1,
                artifact_hash: Hash::new(&oldest_manifest_bytes),
            },
            NativeAmxEvidencePruneEntryV2 {
                kind: NativeAmxEvidencePruneIntentV2::RECEIPT_KIND,
                participant_height: 1,
                artifact_hash: Hash::new(&oldest_receipt_bytes),
            },
            NativeAmxEvidencePruneEntryV2 {
                kind: NativeAmxEvidencePruneIntentV2::MANIFEST_KIND,
                participant_height: 2,
                artifact_hash: Hash::new(&protected_manifest_bytes),
            },
            NativeAmxEvidencePruneEntryV2 {
                kind: NativeAmxEvidencePruneIntentV2::RECEIPT_KIND,
                participant_height: 2,
                artifact_hash: Hash::new(&protected_receipt_bytes),
            },
        ],
    };
    let hostile_path = newest_manifest
        .parent()
        .expect("latest-protected Native evidence directory")
        .join(NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE);
    fs::write(
        &hostile_path,
        norito::to_bytes(&hostile).expect("encode latest-targeting Native prune intent"),
    )
    .expect("stage latest-targeting Native prune intent");
    drop(kura);
    let error = match Kura::new(&config, &lane_config) {
        Ok(_) => panic!("startup must reject a prune intent targeting every retained pair"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("latest identity"),
        "unexpected latest-targeting Native prune-intent error: {error}"
    );
    assert_eq!(
        fs::read(&newest_manifest).expect("reread protected latest Native manifest"),
        protected_manifest_bytes
    );
    assert_eq!(
        fs::read(&newest_receipt).expect("reread protected latest Native receipt"),
        protected_receipt_bytes
    );
    assert_eq!(
        fs::read(&oldest_manifest).expect("reread oldest Native manifest"),
        oldest_manifest_bytes
    );
    assert_eq!(
        fs::read(&oldest_receipt).expect("reread oldest Native receipt"),
        oldest_receipt_bytes
    );
    assert!(
        hostile_path.exists(),
        "fail-closed startup must retain a latest-targeting prune intent for forensics"
    );
}

#[test]
fn native_amx_prune_intent_v2_temporary_cannot_delete_all_pointerless_pairs() {
    let temp_dir = TempDir::new().expect("hostile temporary Native prune Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) =
        Kura::new(&config, &lane_config).expect("initialize hostile temporary Native prune Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("hostile temporary Native prune lane entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
    let intent = native_amx_prune_intent_for_test(&kura, &entry, &receipts[1], &[1, 2]);
    let evidence_directory = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root));
    let intent_path = evidence_directory.join(NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE);
    let intent_temp_path = evidence_directory.join(NATIVE_AMX_EVIDENCE_PRUNE_INTENT_TEMP_FILE);
    let latest_path =
        Kura::native_amx_participant_receipt_latest_index_path_for_entry(&entry, &kura.store_root);
    assert!(!latest_path.exists());
    fs::write(
        &intent_temp_path,
        norito::encode_canonical(&intent).expect("encode hostile temporary Native prune intent"),
    )
    .expect("stage hostile temporary Native prune intent");
    sync_dir(&evidence_directory).expect("sync hostile temporary Native prune intent");
    let evidence_paths = [
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1),
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1),
        Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 2),
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 2),
    ];
    let evidence_bytes = evidence_paths
        .iter()
        .map(|path| fs::read(path).expect("snapshot retained Native evidence"))
        .collect::<Vec<_>>();
    drop(kura);

    let error = match Kura::new(&config, &lane_config) {
        Ok(_) => panic!("temporary V2 intent must not delete every pointerless pair"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("protected-latest identity"),
        "unexpected hostile temporary Native prune error: {error}"
    );
    assert!(
        !intent_path.exists() && intent_temp_path.exists(),
        "failed temporary intent promotion must retain the hostile temporary for forensics"
    );
    for (path, expected) in evidence_paths.iter().zip(evidence_bytes) {
        assert_eq!(
            fs::read(path).expect("reread retained Native evidence"),
            expected
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn native_amx_prune_intent_v2_requires_exact_protected_pair_and_metadata_join() {
    for damage in [
        "missing-manifest",
        "tampered-manifest",
        "missing-receipt",
        "tampered-receipt",
        "missing-finality",
        "tampered-finality",
        "missing-checkpoint",
        "missing-commit-manifest",
        "receipt-hash-drift",
        "stable-pointer-conflict",
    ] {
        let temp_dir = TempDir::new().expect("protected Native prune damage directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::new(&config, &lane_config)
            .expect("initialize protected Native prune damage Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("protected Native prune damage lane entry");
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
        let mut intent = native_amx_prune_intent_for_test(&kura, &entry, &receipts[1], &[1]);
        if damage == "receipt-hash-drift" {
            intent.protected_latest.receipt_artifact_hash = HashOf::from_untyped_unchecked(
                Hash::new(b"drifted protected Native receipt artifact"),
            );
        }
        let evidence_directory = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root));
        let intent_path = evidence_directory.join(NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE);
        fs::write(
            &intent_path,
            norito::encode_canonical(&intent).expect("encode protected Native prune intent"),
        )
        .expect("stage protected Native prune intent");
        let removal_manifest =
            Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
        let removal_receipt =
            Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
        let removal_manifest_bytes =
            fs::read(&removal_manifest).expect("snapshot removable Native manifest");
        let removal_receipt_bytes =
            fs::read(&removal_receipt).expect("snapshot removable Native receipt");
        let protected_manifest =
            Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 2);
        let protected_receipt =
            Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 2);
        match damage {
            "missing-manifest" => {
                fs::remove_file(&protected_manifest).expect("remove protected Native manifest");
            }
            "tampered-manifest" => {
                fs::write(&protected_manifest, [0xA5]).expect("tamper protected Native manifest");
            }
            "missing-receipt" => {
                fs::remove_file(&protected_receipt).expect("remove protected Native receipt");
            }
            "tampered-receipt" => {
                fs::write(&protected_receipt, [0x5A]).expect("tamper protected Native receipt");
            }
            "missing-finality" => {
                kura.remove_v2_finality_without_binding_for_tests(1)
                    .expect("remove protected Native finality");
            }
            "tampered-finality" => {
                fs::write(kura.v2_finality_artifact_path(1), [0xC3])
                    .expect("tamper protected Native finality");
            }
            "missing-checkpoint" => {
                kura.remove_wsv_checkpoint_without_binding_for_tests(1)
                    .expect("remove protected Native WSV checkpoint");
            }
            "missing-commit-manifest" => {
                kura.remove_commit_manifest_without_binding_for_tests(1)
                    .expect("remove protected Native commit manifest");
            }
            "receipt-hash-drift" => {}
            "stable-pointer-conflict" => {
                let conflicting_latest =
                    NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipts[0]);
                let latest_path = Kura::native_amx_participant_receipt_latest_index_path_for_entry(
                    &entry,
                    &kura.store_root,
                );
                fs::write(
                    latest_path,
                    norito::encode_canonical(&conflicting_latest)
                        .expect("encode conflicting Native latest pointer"),
                )
                .expect("stage conflicting Native latest pointer");
            }
            _ => unreachable!("fixed protected Native prune damage matrix"),
        }
        sync_dir(&evidence_directory).expect("sync protected Native prune damage");
        drop(kura);

        let error = match Kura::new(&config, &lane_config) {
            Ok(_) => panic!("{damage} must block Native prune recovery"),
            Err(error) => error,
        };
        assert!(
            intent_path.exists(),
            "{damage} must retain the V2 prune intent for forensics: {error}"
        );
        assert_eq!(
            fs::read(&removal_manifest).expect("reread removable Native manifest"),
            removal_manifest_bytes,
            "{damage} must fail before the first unlink"
        );
        assert_eq!(
            fs::read(&removal_receipt).expect("reread removable Native receipt"),
            removal_receipt_bytes,
            "{damage} must fail before the second unlink"
        );
    }
}

#[test]
fn native_amx_latest_index_startup_leaves_missing_evidence_repair_pending() {
    for missing_kind in ["manifest", "receipt"] {
        let temp_dir = TempDir::new().expect("temporary Kura directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("primary lane storage entry");
        let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
        kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect("publish exact derived latest pointer");
        let latest_path = Kura::native_amx_participant_receipt_latest_index_path_for_entry(
            &entry,
            &kura.store_root,
        );
        let missing_data_path = match missing_kind {
            "manifest" => {
                Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1)
            }
            "receipt" => {
                Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1)
            }
            _ => unreachable!(),
        };
        if missing_kind == "manifest" {
            let descriptor = &receipt.participant_proposal.descriptor;
            let manifest_data_before =
                fs::read(&missing_data_path).expect("read canonical Native manifest data");
            let enforced_before = kura
                .refresh_disk_usage_bytes()
                .expect("refresh disk accounting before manifest crash");
            let total_before = kura
                .disk_usage_bytes()
                .expect("read total disk accounting before manifest crash");

            for result in [
                kura.remove_latest_native_amx_participant_manifest_for_testing(
                    descriptor.lane_id,
                    descriptor.dataspace_id,
                    descriptor.lane_incarnation,
                    0,
                    receipt.application_block_hash,
                ),
                kura.remove_latest_native_amx_participant_manifest_for_testing(
                    descriptor.lane_id,
                    DataSpaceId::new(descriptor.dataspace_id.as_u64().saturating_add(1)),
                    descriptor.lane_incarnation,
                    descriptor.lane_block_height,
                    receipt.application_block_hash,
                ),
                kura.remove_latest_native_amx_participant_manifest_for_testing(
                    descriptor.lane_id,
                    descriptor.dataspace_id,
                    Hash::new(b"inactive Native AMX manifest test incarnation"),
                    descriptor.lane_block_height,
                    receipt.application_block_hash,
                ),
                kura.remove_latest_native_amx_participant_manifest_for_testing(
                    descriptor.lane_id,
                    descriptor.dataspace_id,
                    descriptor.lane_incarnation,
                    descriptor.lane_block_height.saturating_add(1),
                    receipt.application_block_hash,
                ),
                kura.remove_latest_native_amx_participant_manifest_for_testing(
                    descriptor.lane_id,
                    descriptor.dataspace_id,
                    descriptor.lane_incarnation,
                    descriptor.lane_block_height,
                    HashOf::from_untyped_unchecked(Hash::new(
                        b"wrong Native AMX application block",
                    )),
                ),
            ] {
                assert!(
                    result.is_err(),
                    "manifest crash hook must reject every inexact identity"
                );
                assert_eq!(
                    fs::read(&missing_data_path)
                        .expect("reread Native manifest data after rejection"),
                    manifest_data_before
                );
                assert_eq!(
                    kura.kura_disk_usage_bytes()
                        .expect("scan enforced usage after rejection"),
                    enforced_before
                );
                assert_eq!(
                    kura.kura_total_disk_usage_bytes()
                        .expect("scan total usage after rejection"),
                    total_before
                );
                assert_eq!(
                    kura.disk_usage_bytes()
                        .expect("read cached total usage after rejection"),
                    total_before
                );
            }

            kura.remove_latest_native_amx_participant_manifest_for_testing(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.lane_block_height,
                receipt.application_block_hash,
            )
            .expect("create exact latest-manifest crash shape");
            assert!(
                !missing_data_path.exists(),
                "exact standalone manifest removal must remove only its canonical file"
            );
            let enforced_after = kura
                .kura_disk_usage_bytes()
                .expect("scan enforced usage after exact manifest removal");
            let total_after = kura
                .kura_total_disk_usage_bytes()
                .expect("scan total usage after exact manifest removal");
            assert!(enforced_after < enforced_before);
            assert!(total_after < total_before);
            assert_eq!(
                kura.disk_usage_bytes()
                    .expect("read cached total usage after exact manifest removal"),
                total_after
            );
        } else {
            fs::remove_file(&missing_data_path)
                .expect("remove interrupted standalone Native receipt");
        }
        drop(kura);

        let (reopened, _) = Kura::new(&config, &lane_config)
            .expect("missing structurally valid Native evidence must remain repair-pending");
        let reopened_entry = reopened
            .lane_storage_entry(LaneId::SINGLE)
            .expect("reopened primary lane storage entry");
        let latest = reopened
            .decode_native_amx_participant_receipt_latest_index(&reopened_entry, &latest_path)
            .expect("decode retained repair-pending latest pointer")
            .expect("repair-pending latest pointer remains structurally valid");
        assert_eq!(
            latest.lane_block_height,
            receipt.participant_proposal.descriptor.lane_block_height
        );
        assert_eq!(
            reopened.latest_native_amx_participant_application_receipt_matching(
                receipt.participant_proposal.descriptor.lane_id,
                receipt.participant_proposal.descriptor.dataspace_id,
                receipt.participant_proposal.descriptor.lane_incarnation,
                |_| true,
            ),
            None,
            "repair-pending {missing_kind} must never satisfy strict runtime evidence"
        );
    }

    // Exercise the two identities that a one-record fixture cannot cover:
    // an otherwise exact but non-newest record, and a newest record whose
    // retained prefix has a malformed index entry. Neither rejection may
    // rewrite forensic evidence.
    let temp_dir = TempDir::new().expect("temporary strict-removal Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::new(&config, &lane_config).expect("initialize strict-removal Kura");
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("strict-removal primary lane storage entry");
    let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    let descriptor = &receipt.participant_proposal.descriptor;
    let namespace = kura
        .native_amx_evidence_namespace_for_entry(&entry)
        .expect("bind strict-removal Native evidence namespace");
    let manifest_data_path = Kura::native_amx_application_manifest_path_for_entry(
        &entry,
        &kura.store_root,
        descriptor.lane_block_height,
    );
    let first = kura
        .read_native_amx_participant_application_manifest_from_paths_locked(
            &entry,
            descriptor.lane_block_height,
            &manifest_data_path,
            &namespace,
        )
        .expect("read first strict-removal manifest");
    let mut second = first.clone();
    second.leaf.participant_height = descriptor.lane_block_height.saturating_add(1);
    second.leaf.participant_view = descriptor.lane_block_view.saturating_add(1);
    second.leaf.predecessor_height = descriptor.lane_block_height;
    second.leaf.predecessor_descriptor_hash = Some(first.leaf.descriptor_hash);
    second.leaf.descriptor_hash = Hash::new(b"second strict-removal manifest descriptor");
    second.leaf.proposal_hash = Hash::new(b"second strict-removal manifest proposal");
    let tree = [HashOf::new(&second.leaf)]
        .into_iter()
        .collect::<MerkleTree<_>>();
    second.leaf_index = 0;
    second.proof = tree.get_proof(0).expect("one-leaf second manifest proof");
    second.manifest_root = tree.root().map(Hash::from).expect("second manifest root");
    second.manifest_leaf_count = 1;
    Kura::validate_native_amx_participant_application_manifest_artifact(&second)
        .expect("valid second strict-removal manifest");
    let second_manifest_path = Kura::native_amx_application_manifest_path_for_entry(
        &entry,
        &kura.store_root,
        second.leaf.participant_height,
    );
    fs::write(
        &second_manifest_path,
        second
            .encode_framed()
            .expect("encode second strict-removal manifest"),
    )
    .expect("persist second standalone strict-removal manifest");

    let two_record_data =
        fs::read(&manifest_data_path).expect("read two-record strict-removal data");
    let second_record_data =
        fs::read(&second_manifest_path).expect("read second strict-removal data");
    assert!(
        kura.remove_latest_native_amx_participant_manifest_for_testing(
            descriptor.lane_id,
            descriptor.dataspace_id,
            descriptor.lane_incarnation,
            descriptor.lane_block_height,
            receipt.application_block_hash,
        )
        .is_err(),
        "an exact older manifest must not be removed while a newer record exists"
    );
    assert_eq!(
        fs::read(&manifest_data_path).expect("reread data after non-newest rejection"),
        two_record_data
    );
    assert_eq!(
        fs::read(&second_manifest_path).expect("reread second data after rejection"),
        second_record_data
    );

    let mut malformed_record = second_record_data.clone();
    malformed_record.push(0xA5);
    fs::write(&second_manifest_path, &malformed_record)
        .expect("forge non-canonical newest standalone manifest");
    assert!(
        kura.remove_latest_native_amx_participant_manifest_for_testing(
            descriptor.lane_id,
            descriptor.dataspace_id,
            descriptor.lane_incarnation,
            second.leaf.participant_height,
            second.leaf.application_block_hash,
        )
        .is_err(),
        "newest removal must fail closed when any retained index entry is malformed"
    );
    assert_eq!(
        fs::read(&manifest_data_path).expect("reread data after strict rejection"),
        two_record_data
    );
    assert_eq!(
        fs::read(&second_manifest_path)
            .expect("reread malformed standalone manifest after strict rejection"),
        malformed_record
    );
    assert!(!manifest_data_path.with_extension("norito.tmp").exists());
    assert!(!second_manifest_path.with_extension("norito.tmp").exists());
}
