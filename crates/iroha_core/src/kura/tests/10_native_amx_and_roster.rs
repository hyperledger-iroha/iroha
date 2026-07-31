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
    let execution_commitment = ExecutionCommitment::new_with_native_amx_application_manifest(
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
        fs::write(&latest_path, &latest_bytes)
            .expect("persist Native prune-journal latest pointer");
        std::fs::File::open(&latest_path)
            .expect("open Native prune-journal latest pointer")
            .sync_all()
            .expect("sync Native prune-journal latest pointer");

        let manifest_path =
            Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
        let receipt_path =
            Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
        let intent = NativeAmxEvidencePruneIntentV1 {
            version: NativeAmxEvidencePruneIntentV1::VERSION,
            lane_id: entry.lane_id,
            dataspace_id: entry.dataspace_id,
            lane_incarnation: latest.lane_incarnation,
            entries: vec![
                NativeAmxEvidencePruneEntryV1 {
                    kind: NativeAmxEvidencePruneIntentV1::MANIFEST_KIND,
                    participant_height: 1,
                    artifact_hash: Hash::new(
                        fs::read(&manifest_path)
                            .expect("read oldest Native manifest before crash staging"),
                    ),
                },
                NativeAmxEvidencePruneEntryV1 {
                    kind: NativeAmxEvidencePruneIntentV1::RECEIPT_KIND,
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
    let protected_manifest_bytes =
        fs::read(&newest_manifest).expect("read protected latest Native manifest");
    let protected_receipt_bytes =
        fs::read(&newest_receipt).expect("read protected latest Native receipt");
    let hostile = NativeAmxEvidencePruneIntentV1 {
        version: NativeAmxEvidencePruneIntentV1::VERSION,
        lane_id: entry.lane_id,
        dataspace_id: entry.dataspace_id,
        lane_incarnation: latest.lane_incarnation,
        entries: vec![
            NativeAmxEvidencePruneEntryV1 {
                kind: NativeAmxEvidencePruneIntentV1::MANIFEST_KIND,
                participant_height: 2,
                artifact_hash: Hash::new(&protected_manifest_bytes),
            },
            NativeAmxEvidencePruneEntryV1 {
                kind: NativeAmxEvidencePruneIntentV1::RECEIPT_KIND,
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
        Ok(_) => panic!("startup must reject a prune intent targeting the derived latest"),
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
    assert!(
        hostile_path.exists(),
        "fail-closed startup must retain a latest-targeting prune intent for forensics"
    );
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
            || error.to_string().contains("another active route"),
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
            eviction_required_replicas:
                iroha_config::parameters::defaults::kura::EVICTION_REQUIRED_REPLICAS,
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
            eviction_required_replicas:
                iroha_config::parameters::defaults::kura::EVICTION_REQUIRED_REPLICAS,
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
            eviction_required_replicas:
                iroha_config::parameters::defaults::kura::EVICTION_REQUIRED_REPLICAS,
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
            eviction_required_replicas:
                iroha_config::parameters::defaults::kura::EVICTION_REQUIRED_REPLICAS,
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
fn native_amx_prepublication_token_rejects_every_state_frontier_drift_and_order_change() {
    let block = crate::sumeragi::exec::result_bearing_native_manifest_block_for_tests();
    let manifest =
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(&block)
            .expect("build canonical Native application manifest");
    let execution_commitment = ExecutionCommitment::new_with_native_amx_application_manifest(
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
        chain_id: ChainId::from("native-frontier-token-test"),
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
            encoding: PayloadEncoding::Plain,
            chunk_size_bytes: 1024,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 4096,
            max_chunk_count: 4,
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
    let frontiers = manifest
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
