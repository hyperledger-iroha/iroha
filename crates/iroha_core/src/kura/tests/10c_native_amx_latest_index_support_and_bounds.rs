// These startup negatives target Native evidence in an established canonical
// store. A completely empty journal-owned store has an earlier namespace gate.
fn install_native_amx_startup_carrier_without_participant_evidence(kura: &Kura) {
    assert_eq!(
        kura.exact_durable_blocks_count()
            .expect("empty fixture block count"),
        0
    );
    publish_initial_configured_lane_geometry_for_test(
        kura,
        &RuntimeLaneConfig::default(),
        &BTreeMap::from([(
            LaneId::SINGLE,
            Hash::new(b"non-Native startup carrier active lane incarnation"),
        )]),
    );
    let mut block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, None)
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    attach_ok_results_to_block(&mut block);
    let finality = v2_finality_artifact_for_block(&block);
    let block = Arc::new(block);
    kura.store_block(Arc::clone(&block))
        .expect("store exact non-Native startup carrier");
    let _ = kura
        .store_v2_finality_artifact(&finality)
        .expect("publish authenticated non-Native startup carrier finality");
    assert!(
        kura.read_native_amx_participant_application_history(LaneId::SINGLE)
            .expect("carrier alone has no Native authority")
            .entries()
            .next()
            .is_none()
    );
}

#[test]
fn native_amx_finality_gate_rejects_same_depth_manifest_count_substitution() {
    let temp_dir = TempDir::new().expect("same-depth Native manifest Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("initialize same-depth Native manifest Kura");
    establish_dummy_store_primary_anchor(&kura);
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("same-depth Native manifest primary lane entry");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3, 4]);
    assert_eq!(receipts.len(), 4);
    let manifests = (1..=4)
        .map(|participant_height| {
            let path = Kura::native_amx_application_manifest_path_for_entry(
                &entry,
                &kura.store_root,
                participant_height,
            );
            norito::decode_canonical::<NativeAmxParticipantApplicationManifestArtifactV1>(
                &fs::read(path).expect("read four-leaf Native manifest artifact"),
            )
            .expect("decode four-leaf Native manifest artifact")
        })
        .collect::<Vec<_>>();
    let qc_manifest = manifests
        .first()
        .expect("four-leaf Native manifest fixture")
        .clone();
    assert_eq!(qc_manifest.manifest_leaf_count, 4);
    Kura::validate_native_amx_participant_application_manifest_artifact(&qc_manifest)
        .expect("QC-backed four-leaf manifest artifact is self-consistent");
    let substituted_tree = manifests
        .iter()
        .take(3)
        .map(|artifact| HashOf::new(&artifact.leaf))
        .collect::<MerkleTree<_>>();
    let mut substituted = qc_manifest.clone();
    substituted.proof = substituted_tree
        .get_proof(substituted.leaf_index)
        .expect("three-leaf same-depth proof");
    substituted.manifest_root = substituted_tree
        .root()
        .map(Hash::from)
        .expect("three-leaf same-depth root");
    substituted.manifest_leaf_count = 3;
    assert_eq!(qc_manifest.proof.audit_path().len(), 2);
    assert_eq!(
        substituted.proof.audit_path().len(),
        qc_manifest.proof.audit_path().len(),
        "three- and four-leaf manifests deliberately have the same proof depth"
    );
    assert_ne!(substituted.manifest_root, qc_manifest.manifest_root);
    Kura::validate_native_amx_participant_application_manifest_artifact(&substituted)
        .expect("substituted three-leaf root/count/proof are internally self-consistent");
    let finality = kura
        .v2_finality_artifact(qc_manifest.leaf.application_block_height)
        .expect("read Native finality")
        .expect("Native finality exists");
    let qc_execution = finality.commit_qc.execution_commitment;
    assert_eq!(
        qc_execution.native_amx_application_manifest_root,
        qc_manifest.manifest_root
    );
    assert_eq!(
        qc_execution.native_amx_application_manifest_count,
        qc_manifest.manifest_leaf_count
    );
    let _prune_guard = kura.prune_lock.lock();
    let _canonical_chain_guard = kura.canonical_chain_lock.lock();
    assert!(
        kura.native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards(
            &qc_manifest,
        ),
        "the exact four-leaf manifest commitment must pass its QC finality gate"
    );
    assert!(
        !kura.native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards(
            &substituted,
        ),
        "an internally valid same-depth three-leaf commitment must not substitute for the four-leaf QC execution commitment"
    );
}
// Keep a genuine outstanding publication through the fault injection. The normal
// pre-WSV publisher creates all three artifact kinds; only later authenticated
// completion may retire the durable index installed by the canonical Native store.
fn native_amx_indexed_latest_index_evidence_fixture() -> (
    NativeAmxPublicationCapacityFixture,
    LaneStorageEntry,
    NativeAmxParticipantApplicationReceiptArtifact,
) {
    let fixture = native_amx_publication_capacity_fixture_with_route_count(2);
    let kura = &fixture.kura;
    kura.store_block(Arc::clone(&fixture.block))
        .expect("admit the actual two-route Native carrier");
    let carrier = Kura::native_amx_publication_carrier(&fixture.block)
        .expect("exact result-bearing Native carrier");
    let indexed = Kura::read_native_amx_publication_index_for_store(&kura.store_root)
        .expect("read store-owned pending index")
        .records;
    assert_eq!(indexed.len(), 1);
    assert!(indexed.contains_key(&carrier));
    let finality_receipt = kura
        .store_v2_finality_artifact(&fixture.finality)
        .expect("persist the actual Native finality");
    assert_v2_commit_receipt_matches_artifact(&finality_receipt, &fixture.finality);
    kura.prepublish_native_amx_participant_application_evidence(&fixture.block, None)
        .expect("prepublish exact manifests, receipts and latest pointers");
    let height = fixture.block.header().height().get();
    let checkpoint_hash = Hash::new(b"indexed latest-pointer repair WSV checkpoint");
    kura.store_wsv_checkpoint(height, fixture.block.hash(), checkpoint_hash)
        .expect("persist the exact pending carrier checkpoint");
    kura.store_commit_manifest(
        CommitManifest::new(
            height,
            fixture.block.hash(),
            None,
            None,
            checkpoint_hash,
            None,
        )
        .with_authenticated_v2_commit_authority(&fixture.finality),
    )
    .expect("persist the finality-bound pending carrier commit manifest");
    assert_eq!(
        Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .expect("prepublication and metadata must retain the original index")
            .records,
        indexed
    );
    {
        let owners = kura.native_amx_publication_capacity_reservations.lock();
        let owner = owners.get(&carrier).expect("pending real carrier owner");
        assert_eq!(owner.routes.len(), 2);
        assert!(owner.routes.values().all(|route| !route.cleanup_complete));
    }
    let (manifest, receipt) = native_amx_participant_application_artifacts(
        &fixture.manifest,
        HashOf::new(&fixture.finality),
    )
    .expect("derive exact artifacts from the admitted Native carrier")
    .into_iter()
    .next()
    .expect("first actual publication route");
    let entry = kura
        .lane_storage_entry(manifest.leaf.lane_id)
        .expect("journal-authenticated selected publication route");
    let latest_path =
        Kura::native_amx_participant_receipt_latest_index_path_for_entry(&entry, &kura.store_root);
    assert_eq!(
        kura.decode_native_amx_participant_receipt_latest_index(&entry, &latest_path)
            .expect("read production-prepublished latest pointer"),
        Some(NativeAmxParticipantReceiptLatestIndexV2::from_receipt(
            &receipt
        ))
    );
    (fixture, entry, receipt)
}

fn install_native_amx_latest_index_evidence_fixture(
    kura: &Kura,
    entry: &LaneStorageEntry,
) -> NativeAmxParticipantApplicationReceiptArtifact {
    install_native_amx_evidence_fixture_heights(kura, entry, &[1])
        .into_iter()
        .next()
        .expect("one Native AMX evidence fixture")
}
fn install_native_amx_evidence_fixture_heights(
    kura: &Kura,
    entry: &LaneStorageEntry,
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
    entry: &LaneStorageEntry,
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
    if entry.lane_id == LaneId::SINGLE {
        // Preserve the authoritative incarnation when State has already initialized Kura.
        establish_dummy_store_primary_anchor(kura);
    } else {
        kura.active_lane_incarnation_marker(
            &kura
                .lane_storage_entry(entry.lane_id)
                .expect("exact active identity"),
        )
        .expect("secondary evidence uses its already admitted exact instance");
    }
    let block = store_dummy_block_arcs(kura, 1)
        .into_iter()
        .next()
        .expect("one durable application block");
    install_native_amx_evidence_fixture_at_block(
        kura,
        entry,
        participant_heights,
        predecessor_drift_height,
        block,
        None,
        None,
    )
}
fn install_native_amx_evidence_fixture_at_block(
    kura: &Kura,
    entry: &LaneStorageEntry,
    participant_heights: &[u64],
    predecessor_drift_height: Option<u64>,
    block: Arc<SignedBlock>,
    previous_proposal: Option<&LaneBlockProposalV1>,
    mut previous_native_settlement_hash: Option<
        HashOf<iroha_data_model::block::consensus::NativeAmxParticipantSettlement>,
    >,
) -> Vec<NativeAmxParticipantApplicationReceiptArtifact> {
    let application_block_height = block.header().height().get();
    let (lane_incarnation, _) = {
        let _geometry_guard = kura.lane_geometry_lock.lock();
        kura.active_lane_incarnation_marker(
            &kura
                .lane_storage_entry(entry.lane_id)
                .expect("exact active identity"),
        )
        .expect("Native AMX fixture requires its durably bound lane geometry")
    };
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
        proposal.descriptor.lane_incarnation = lane_incarnation;
        proposal.descriptor.proposal_height = application_block_height;
        if let Some(predecessor) = proposals.last().or(previous_proposal) {
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
        let settlement =
            iroha_data_model::block::consensus::NativeAmxParticipantSettlement::try_new(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.lane_incarnation,
                proposal.descriptor.lane_block_height,
                application_block_height,
                previous_native_settlement_hash,
                vec![source_id],
            )
            .expect("valid Native participant control");
        let settlement_hash = settlement
            .computed_hash()
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
            previous_native_settlement_hash,
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
        previous_native_settlement_hash = Some(settlement_hash);
    }
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
    let parent_finality = application_block_height
        .checked_sub(1)
        .filter(|height| *height > 0)
        .map(|height| {
            kura.v2_finality_artifact(height)
                .expect("read exact previous carrier finality")
                .expect("previous carrier finality is installed")
        });
    let finality = v2_finality_artifact_for_block_with_keys(
        block.as_ref(),
        parent_finality.as_ref(),
        &v2_finality_fixture_keys(),
        execution_commitment,
    );
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
    entry: &LaneStorageEntry,
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
        removed_settlements: native_amx_prune_settlement_preimages_for_test(
            kura,
            entry,
            removal_heights,
        ),
    }
}
fn native_amx_prune_settlement_preimages_for_test(
    kura: &Kura,
    entry: &LaneStorageEntry,
    heights: &[u64],
) -> Vec<iroha_data_model::block::consensus::NativeAmxParticipantSettlement> {
    heights
        .iter()
        .map(|height| {
            let path = Kura::native_amx_participant_receipt_path_for_entry(
                entry,
                &kura.store_root,
                *height,
            );
            let receipt =
                norito::decode_canonical::<NativeAmxParticipantApplicationReceiptArtifact>(
                    &fs::read(path)
                        .expect("read exact removed Native receipt before staging prune"),
                )
                .expect("decode exact removed Native receipt preimage");
            Kura::validate_native_amx_participant_application_receipt_artifact(&receipt)
                .expect("removed Native settlement comes from structurally valid exact receipt");
            receipt.participant_settlement
        })
        .collect()
}
struct NativeAmxTwoRouteRepairFixture {
    _temp_dir: TempDir,
    kura: Arc<Kura>,
    lane_config: RuntimeLaneConfig,
    block: Arc<SignedBlock>,
    plan: NativeAmxParticipantApplicationEvidencePlan,
    markers: Vec<crate::state::AppliedNativeAmxParticipantFrontierMarker>,
    entries: [LaneStorageEntry; 2],
}
#[allow(clippy::too_many_lines)]
fn native_amx_two_route_repair_fixture() -> NativeAmxTwoRouteRepairFixture {
    let NativeAmxPublicationCapacityFixture {
        _temp_dir: temp_dir,
        kura,
        block,
        manifest,
        finality,
        lane_config,
    } = native_amx_publication_capacity_fixture_with_route_count(2);
    // The real canonical store installs the exact pending index and reservation
    // before its result-bearing Native block becomes durable.
    kura.store_block(Arc::clone(&block))
        .expect("store actual two-route Native carrier");
    let application_block_height = block.header().height().get();
    let application_block_hash = block.hash();
    let executed_block_wire_hash = manifest.executed_block_wire_hash();
    let manifest_root = manifest.root();
    let manifest_leaf_count = manifest.count();
    assert_eq!(manifest_leaf_count, 2);
    let _finality_commit_receipt = kura
        .store_v2_finality_artifact(&finality)
        .expect("persist exact two-route Native finality");
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
    let artifacts = native_amx_participant_application_artifacts(&manifest, finality_artifact_hash)
        .expect("derive all real two-route Native artifacts");
    assert_eq!(artifacts.len(), 2);
    let mut markers = Vec::with_capacity(2);
    let mut entries = Vec::with_capacity(2);
    for (manifest, receipt) in &artifacts {
        Kura::validate_native_amx_participant_application_manifest_artifact(manifest)
            .expect("validate two-route Native manifest artifact");
        Kura::validate_native_amx_participant_application_receipt_artifact(receipt)
            .expect("validate two-route Native receipt artifact");
        let leaf = &manifest.leaf;
        entries.push(
            kura.lane_storage_entry(leaf.lane_id)
                .expect("exact two-route Native entry"),
        );
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
        lane_config,
        _temp_dir: temp_dir,
        kura,
        block,
        plan,
        markers,
        entries: entries
            .try_into()
            .expect("exactly two distinct Native publication routes"),
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
fn native_amx_latest_index_test_paths(kura: &Kura, entry: &LaneStorageEntry) -> (PathBuf, PathBuf) {
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
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("initialize Kura");
    install_native_amx_startup_carrier_without_participant_evidence(&kura);
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
        let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
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
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
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
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("initialize Kura");
    install_native_amx_startup_carrier_without_participant_evidence(&kura);
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("primary lane storage entry");
    let evidence_directory = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root));
    let legacy_name = ["native_amx_participant_receipts.latest_v", "1", ".norito"].concat();
    let legacy_path = evidence_directory.join(legacy_name);
    fs::write(&legacy_path, [0xA5]).expect("stage unsupported legacy latest pointer");
    drop(kura);
    let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
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
        config.lane_history_retention =
            NonZeroUsize::new(2).expect("small Native history test bound");
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("initialize Kura");
        install_native_amx_startup_carrier_without_participant_evidence(&kura);
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
        let hostile_entries = config.lane_history_retention.get().saturating_add(2);
        for height in 1..=hostile_entries {
            fs::write(
                artifact_path(u64::try_from(height).expect("hostile height fits u64")),
                [0xA5],
            )
            .expect("stage excess standalone Native record");
        }
        drop(kura);
        let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
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
    config.lane_history_retention =
        NonZeroUsize::new(2).expect("small Native compaction retention");
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("initialize bounded Native compaction Kura");
    establish_dummy_store_primary_anchor(&kura);
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
    let store_root = kura.store_root.clone();
    let retained_payloads = [2_u64, 3]
        .into_iter()
        .flat_map(|height| {
            [
                Kura::native_amx_application_manifest_path_for_entry(&entry, &store_root, height),
                Kura::native_amx_participant_receipt_path_for_entry(&entry, &store_root, height),
            ]
        })
        .map(|path| {
            let bytes = fs::read(&path).expect("exact retained payload before startup compaction");
            (path, bytes)
        })
        .collect::<Vec<_>>();
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("retention plus one fully valid pair must compact at startup");
    assert!(reopened.lane_storage_entries.lock().is_empty());
    let reopened_entry = entry.clone();
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
    for (path, expected_bytes) in retained_payloads {
        assert_eq!(
            fs::read(path).expect("retained payload after startup compaction"),
            expected_bytes,
            "refreshing sibling-directory metadata must preserve exact retained evidence bytes"
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
    let (reopened_again, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("Native compaction startup repair must be idempotent");
    assert!(reopened_again.lane_storage_entries.lock().is_empty());
    let reopened_again_entry = entry.clone();
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
        config.lane_history_retention =
            NonZeroUsize::new(2).expect("small Native history test bound");
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("initialize Kura");
        install_native_amx_startup_carrier_without_participant_evidence(&kura);
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
        let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
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
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("initialize Kura");
        establish_dummy_store_primary_anchor(&kura);
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
        let (_reopened, _) =
            Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
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
        config.lane_history_retention =
            NonZeroUsize::new(2).expect("small Native prune-journal retention");
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("initialize Native prune-journal Kura");
        establish_dummy_store_primary_anchor(&kura);
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
            removed_settlements: vec![receipts[0].participant_settlement.clone()],
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
        let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .unwrap_or_else(|error| panic!("{crash_stage} recovery failed: {error}"));
        assert!(reopened.lane_storage_entries.lock().is_empty());
        let reopened_entry = entry.clone();
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
        let (reopened_again, _) =
            Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
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
    config.lane_history_retention =
        NonZeroUsize::new(2).expect("latest-protected Native prune retention");
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("initialize latest-protected Native prune Kura");
    establish_dummy_store_primary_anchor(&kura);
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
        removed_settlements: receipts
            .iter()
            .map(|receipt| receipt.participant_settlement.clone())
            .collect(),
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
    let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
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
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("initialize hostile temporary Native prune Kura");
    establish_dummy_store_primary_anchor(&kura);
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
    let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
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
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("initialize protected Native prune damage Kura");
        establish_dummy_store_primary_anchor(&kura);
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
        let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
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
        let (fixture, entry, receipt) = native_amx_indexed_latest_index_evidence_fixture();
        let NativeAmxPublicationCapacityFixture {
            _temp_dir: temp_dir,
            kura,
            lane_config,
            ..
        } = fixture;
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let (incarnations, activation_heights) = active_fixture_geometry_maps(&kura, &lane_config);
        let indexed = Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .expect("pending exact carrier before half-pair fault")
            .records;
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
        sync_dir(Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root)).as_path())
            .expect("sync the exact pending half-pair fault");
        assert_eq!(
            Kura::read_native_amx_publication_index_for_store(&kura.store_root)
                .expect("half-pair fault must retain real publication authority")
                .records,
            indexed
        );
        let files = snapshot_regular_files_recursively(&kura.store_root);
        let network_id = kura.bound_lane_storage_network().unwrap();
        drop(kura);
        let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("indexed missing Native evidence must remain repair-pending");
        assert_eq!(
            snapshot_regular_files_recursively(&reopened.store_root),
            files
        );
        assert!(
            !reopened
                .lane_storage_entries
                .lock()
                .contains_key(&entry.lane_id)
        );
        reopened.bind_lane_storage_network(network_id).unwrap();
        reopened
            .recover_lane_geometry_journal(&lane_config, &incarnations, &activation_heights)
            .expect("restore only the original authenticated secondary geometry");
        reopened
            .finish_restored_lane_segments_with_geometry(&lane_config)
            .expect("retain the indexed highest half-pair for State-driven repair");
        assert!(
            !missing_data_path.exists(),
            "startup must not fabricate {missing_kind}"
        );
        assert_eq!(
            Kura::read_native_amx_publication_index_for_store(&reopened.store_root)
                .expect("unfinished route must keep the original pending index")
                .records,
            indexed
        );
        let reopened_entry = reopened
            .lane_storage_entry(entry.lane_id)
            .expect("replayed secondary publication route");
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
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("initialize strict-removal Kura");
    establish_dummy_store_primary_anchor(&kura);
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
fn native_amx_latest_strict_read_distinguishes_absence_and_valid_nonmatching_evidence() {
    let (_temp_dir, _config, _lane_config, kura) = temporary_kura_fixture();
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("active primary route");
    establish_dummy_store_primary_anchor(&kura);
    let guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    kura.bind_consensus_output_guard(Arc::clone(&guard))
        .expect("bind guard");
    assert_eq!(
        kura.consensus_storage_read(
            kura.read_latest_native_amx_participant_application_receipt(entry.lane_id)
        )
        .expect("genuinely absent Native history"),
        NativeAmxLatestReceiptObservation::Absent,
    );
    let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
        .expect("publish exact Native latest pointer");
    let descriptor = &receipt.participant_proposal.descriptor;
    assert_eq!(
        kura.consensus_storage_read(
            kura.read_latest_native_amx_participant_application_receipt(entry.lane_id)
        )
        .expect("authenticate complete Native application"),
        NativeAmxLatestReceiptObservation::Applied(receipt.clone()),
    );
    for (dataspace, incarnation, proposal_hash) in [
        (
            entry.dataspace_id,
            descriptor.lane_incarnation,
            Hash::new(b"valid competing proposal"),
        ),
        (
            DataSpaceId::new(entry.dataspace_id.as_u64().saturating_add(1)),
            descriptor.lane_incarnation,
            receipt.participant_proposal.proposal_hash,
        ),
        (
            entry.dataspace_id,
            Hash::new(b"valid competing incarnation"),
            receipt.participant_proposal.proposal_hash,
        ),
    ] {
        let observed = kura
            .consensus_storage_read(
                kura.read_latest_native_amx_participant_application_receipt(entry.lane_id),
            )
            .expect("candidate mismatch is not local corruption");
        let NativeAmxLatestReceiptObservation::Applied(observed) = observed else {
            panic!("valid stored evidence must remain visible independently of the candidate");
        };
        let stored = &observed.participant_proposal;
        assert!(
            stored.descriptor.dataspace_id != dataspace
                || stored.descriptor.lane_incarnation != incarnation
                || stored.proposal_hash != proposal_hash
        );
        assert!(!guard.restart_required());
        assert!(guard.acquire().is_some());
    }
}

#[test]
fn native_amx_latest_strict_read_preserves_damaged_occupied_evidence() {
    for damaged_kind in [
        "latest",
        "receipt",
        "manifest",
        "canonical wire",
        "checkpoint",
        "commit manifest",
        "missing latest",
        "missing manifest",
    ] {
        let (_temp_dir, _config, _lane_config, kura) = temporary_kura_fixture();
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("active primary route");
        let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
        kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect("publish exact Native latest pointer");
        let descriptor = &receipt.participant_proposal.descriptor;
        let latest_path = Kura::native_amx_participant_receipt_latest_index_path_for_entry(
            &entry,
            &kura.store_root,
        );
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
        let block_path = kura
            .block_store
            .lock()
            .path_to_blockchain
            .join("blocks.data");
        let checkpoint_path = kura.wsv_checkpoint_path(1);
        let commit_manifest_path = kura.commit_manifest_path(1);
        let paths = [
            latest_path.clone(),
            manifest_path.clone(),
            receipt_path.clone(),
            block_path.clone(),
            checkpoint_path.clone(),
            commit_manifest_path.clone(),
        ];
        assert!(
            kura.get_block(nonzero!(1_usize)).is_some(),
            "warm canonical decoded cache"
        );
        let damaged_path = match damaged_kind {
            "latest" | "missing latest" => latest_path,
            "manifest" | "missing manifest" => manifest_path,
            "receipt" => receipt_path,
            "canonical wire" => block_path,
            "checkpoint" => checkpoint_path,
            "commit manifest" => commit_manifest_path,
            _ => unreachable!(),
        };
        if damaged_kind.starts_with("missing") {
            std::fs::remove_file(&damaged_path).expect("remove one exact referenced sidecar");
        } else {
            let mut bytes = std::fs::read(&damaged_path).expect("read real evidence bytes");
            bytes[0] ^= 0x80;
            std::fs::write(&damaged_path, bytes).expect("damage actual occupied evidence");
        }
        let before = paths
            .iter()
            .map(|path| std::fs::read(path).ok())
            .collect::<Vec<_>>();
        let guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        kura.bind_consensus_output_guard(Arc::clone(&guard))
            .expect("bind guard");
        assert!(
            kura.consensus_storage_read(
                kura.read_latest_native_amx_participant_application_receipt(entry.lane_id)
            )
            .is_err(),
            "{damaged_kind} cannot be hidden by warm caches or candidate filtering",
        );
        assert!(guard.restart_required(), "{damaged_kind}");
        assert!(guard.acquire().is_none(), "{damaged_kind}");
        let after = paths
            .iter()
            .map(|path| std::fs::read(path).ok())
            .collect::<Vec<_>>();
        assert_eq!(
            before, after,
            "strict observation never repairs {damaged_kind}"
        );
    }
}

#[test]
fn native_amx_latest_strict_read_rejects_stale_active_incarnation_evidence() {
    let (_temp_dir, _config, _lane_config, kura) = temporary_kura_fixture();
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("active primary route");
    let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
        .expect("publish exact Native latest pointer");
    let latest_path =
        Kura::native_amx_participant_receipt_latest_index_path_for_entry(&entry, &kura.store_root);
    let before = std::fs::read(&latest_path).expect("read exact original latest pointer");
    let replacement = Hash::new(b"same Native route replacement incarnation");
    assert_ne!(
        replacement,
        receipt.participant_proposal.descriptor.lane_incarnation
    );
    kura.substitute_lane_marker_identity_for_test(&entry, replacement, 0)
        .expect("replace same route's active incarnation marker");
    assert!(
        kura.read_latest_native_amx_participant_application_receipt(entry.lane_id)
            .is_err()
    );
    assert_eq!(
        std::fs::read(&latest_path).unwrap(),
        before,
        "stale occupied source is retained"
    );
}

#[test]
fn native_amx_latest_strict_read_defers_authenticated_pending_tip_metadata() {
    for pending_shape in ["metadata absent", "unbound checkpoint"] {
        let directory = TempDir::new().unwrap();
        let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
        let catalog = LaneCatalog::default();
        let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
        let (kura, _) =
            Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog).unwrap();
        let mut state = State::try_new_with_chain_and_network_id_with_default_telemetry(
            World::default(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
            ChainId::from("native-amx-pending-tip"),
            test_network_id(b"kura-v2-finality-test"),
        )
        .expect("construct State before admitting its initial lane identity");
        state
            .prepare_configured_primary_geometry_anchor(&catalog)
            .unwrap();
        state.install_active_lane_markers_for_tests();
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("active primary route");
        let established_incarnation = {
            let _geometry_guard = kura.lane_geometry_lock.lock();
            kura.active_lane_incarnation_marker(&entry)
                .expect("State established the authoritative primary incarnation")
        };
        let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
        {
            let _geometry_guard = kura.lane_geometry_lock.lock();
            assert_eq!(
                kura.active_lane_incarnation_marker(&entry)
                    .expect("fixture preserves the authoritative primary incarnation"),
                established_incarnation,
            );
        }
        assert_eq!(
            receipt.participant_proposal.descriptor.lane_incarnation,
            established_incarnation.0,
        );
        kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect("publish exact Native latest pointer");
        kura.remove_commit_manifest_without_binding_for_tests(1)
            .expect("interrupt post-apply metadata");
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
                .expect("leave exact unbound checkpoint"),
            _ => unreachable!(),
        }
        let guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        kura.bind_consensus_output_guard(Arc::clone(&guard))
            .expect("bind guard");
        assert_eq!(
            kura.consensus_storage_read(
                kura.read_latest_native_amx_participant_application_receipt(entry.lane_id)
            )
            .expect("authenticated exact-tip crash remains recoverable"),
            NativeAmxLatestReceiptObservation::PendingTipMetadata(receipt.clone()),
            "{pending_shape} remains occupied until owned Apply recovery completes",
        );
        assert!(
            !crate::state::State::lane_block_predecessor_is_applied_for_snapshot(
                &state.query_view(),
                &receipt.participant_proposal,
                crate::state::LanePredecessorApplicationMode::CurrentTip,
            )
            .expect("pending Native tip is recoverable"),
            "pending evidence blocks an empty first-slot predecessor"
        );
        assert!(!guard.restart_required());
        assert!(guard.acquire().is_some());
    }
}

#[test]
fn native_amx_latest_strict_read_preserves_one_extra_pair_until_pending_tip_recovery() {
    let temp_dir = TempDir::new().expect("pending Native retained suffix");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.lane_history_retention = nonzero!(1_usize);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("open one-pair Native retention fixture");
    establish_dummy_store_primary_anchor(&kura);
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("active primary route");
    let first = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
    kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
        .expect("publish first complete pointer");
    let mut blocks = DummyBlocks {
        blocks: vec![
            kura.get_block(nonzero!(1_usize))
                .expect("retained first carrier"),
        ],
    };
    let second_block = blocks.next();
    kura.store_block(Arc::clone(&second_block))
        .expect("append second canonical carrier");
    let second = install_native_amx_evidence_fixture_at_block(
        &kura,
        &entry,
        &[2],
        None,
        second_block,
        Some(&first.participant_proposal),
        Some(first.participant_settlement_hash),
    )
    .pop()
    .expect("second Native application receipt");
    let checkpoint_path = kura.wsv_checkpoint_path(2);
    let manifest_path = kura.commit_manifest_path(2);
    let checkpoint = std::fs::read(&checkpoint_path).expect("second exact checkpoint bytes");
    let manifest = std::fs::read(&manifest_path).expect("second exact commit manifest bytes");
    kura.remove_commit_manifest_without_binding_for_tests(2)
        .expect("interrupt tip metadata");
    kura.remove_wsv_checkpoint_without_binding_for_tests(2)
        .expect("tip checkpoint pending");
    kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
        .expect("owned startup retains previous complete pair alongside pending tip");
    let evidence_directory = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root));
    let before = snapshot_regular_files_recursively(&evidence_directory);
    assert_eq!(
        kura.read_latest_native_amx_participant_application_receipt(entry.lane_id)
            .expect("retention plus one is valid only for the authenticated pending frontier"),
        NativeAmxLatestReceiptObservation::PendingTipMetadata(second.clone()),
    );
    assert_eq!(
        snapshot_regular_files_recursively(&evidence_directory),
        before,
        "runtime observation cannot prune the previous complete pair"
    );
    let first_receipt_path =
        Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
    assert!(first_receipt_path.exists());
    write_synced_native_amx_test_file(&checkpoint_path, &checkpoint);
    write_synced_native_amx_test_file(&manifest_path, &manifest);
    let before_cleanup = snapshot_regular_files_recursively(&evidence_directory);
    assert_eq!(
        kura.read_latest_native_amx_participant_application_receipt(entry.lane_id)
            .expect("completed metadata may precede cleanup in live Apply"),
        NativeAmxLatestReceiptObservation::Applied(second.clone()),
    );
    assert_eq!(
        snapshot_regular_files_recursively(&evidence_directory),
        before_cleanup,
        "Applied observation must not perform pending retention cleanup"
    );
    assert!(first_receipt_path.exists());
    kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
        .expect("owned recovery completes retention after exact metadata returns");
    assert!(!first_receipt_path.exists());
    assert_eq!(
        kura.read_latest_native_amx_participant_application_receipt(entry.lane_id)
            .expect("completed retained frontier"),
        NativeAmxLatestReceiptObservation::Applied(second),
    );
}

#[test]
fn native_amx_history_read_exposes_highest_repair_half_without_mutation() {
    for missing_kind in ["manifest", "receipt"] {
        for pointer_state in ["current", "previous", "absent"] {
            let (_temp_dir, _config, _lane_config, kura) = temporary_kura_fixture();
            let entry = kura
                .lane_storage_entry(LaneId::SINGLE)
                .expect("active primary route");
            let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
            kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
                .expect("publish complete highest pointer");
            let latest_path = Kura::native_amx_participant_receipt_latest_index_path_for_entry(
                &entry,
                &kura.store_root,
            );
            let manifest_path =
                Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 2);
            let receipt_path =
                Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 2);
            let original_manifest = fs::read(&manifest_path).expect("exact highest manifest");
            let missing_path = if missing_kind == "manifest" {
                &manifest_path
            } else {
                &receipt_path
            };
            let missing_bytes =
                fs::read(missing_path).expect("exact missing half before interruption");
            fs::remove_file(missing_path).expect("interrupt highest pair publication");
            match pointer_state {
                "previous" => write_synced_native_amx_test_file(
                    &latest_path,
                    &norito::encode_canonical(
                        &NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipts[0]),
                    )
                    .expect("previous exact derived pointer"),
                ),
                "absent" => fs::remove_file(&latest_path).expect("derived pointer awaits startup"),
                "current" => {}
                _ => unreachable!(),
            }
            let evidence_directory = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root));
            let before = snapshot_regular_files_recursively(&evidence_directory);
            let guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
            kura.bind_consensus_output_guard(Arc::clone(&guard))
                .expect("bind guard");
            let history = kura
                .consensus_storage_read(
                    kura.read_native_amx_participant_application_history(entry.lane_id),
                )
                .expect("one authenticated highest half remains repairable");
            assert_eq!(
                history
                    .entries()
                    .map(|(height, _)| height)
                    .collect::<Vec<_>>(),
                vec![1, 2]
            );
            assert_eq!(
                history.get(1),
                Some(&NativeAmxParticipantApplicationObservation::Applied(
                    receipts[0].clone()
                ))
            );
            let expected = if missing_kind == "manifest" {
                NativeAmxParticipantApplicationObservation::PendingManifestRepair(
                    receipts[1].clone(),
                )
            } else {
                NativeAmxParticipantApplicationObservation::PendingReceiptRepair(
                    norito::decode_canonical(&original_manifest)
                        .expect("decode original exact manifest"),
                )
            };
            assert_eq!(history.get(2), Some(&expected));
            assert!(
                history.drain_evidence(2).is_none(),
                "pending highest slot cannot authorize drain"
            );
            assert!(
                history.drain_evidence(1).is_none(),
                "older Applied pair cannot hide pending highest debt"
            );
            assert!(
                history.get(3).is_none(),
                "in-memory absence never hides the occupied highest slot"
            );
            assert!(!guard.restart_required());
            assert_eq!(
                snapshot_regular_files_recursively(&evidence_directory),
                before,
                "{missing_kind}/{pointer_state}: read-only history must not repair the pair or pointer"
            );
            // Finish the exact interrupted half, then let the existing owned
            // startup operation publish the derived pointer and qualify the pair.
            write_synced_native_amx_test_file(missing_path, &missing_bytes);
            kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
                .expect("owned startup completes exact publication");
            let repaired = kura
                .read_native_amx_participant_application_history(entry.lane_id)
                .expect("complete repaired history");
            assert_eq!(
                repaired.get(2),
                Some(&NativeAmxParticipantApplicationObservation::Applied(
                    receipts[1].clone()
                ))
            );
            assert_eq!(
                kura.read_latest_native_amx_participant_application_receipt(entry.lane_id)
                    .expect("complete live frontier"),
                NativeAmxLatestReceiptObservation::Applied(receipts[1].clone())
            );
        }
    }
}

#[test]
fn native_amx_history_read_rejects_occupied_damage_before_exact_lookup() {
    for damaged_kind in [
        "older receipt",
        "older manifest",
        "missing older receipt",
        "missing older manifest",
        "partial canonical wire",
        "partial checkpoint",
        "partial commit manifest",
        "receipt-only checkpoint",
        "receipt-only canonical wire",
    ] {
        let (_temp_dir, _config, _lane_config, kura) = temporary_kura_fixture();
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("active primary route");
        let partial =
            damaged_kind.starts_with("partial") || damaged_kind.starts_with("receipt-only");
        let heights: &[u64] = if partial { &[1] } else { &[1, 2] };
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, heights);
        kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect("publish complete pointer");
        assert!(
            kura.get_block(nonzero!(1_usize)).is_some(),
            "warm actual carrier cache"
        );
        let healthy = kura
            .read_native_amx_participant_application_history(entry.lane_id)
            .expect("healthy history");
        assert_eq!(
            healthy.get(*heights.last().expect("highest slot")),
            Some(&NativeAmxParticipantApplicationObservation::Applied(
                receipts.last().expect("highest receipt").clone()
            ))
        );
        let damaged_path = match damaged_kind {
            "older receipt" | "missing older receipt" => {
                Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1)
            }
            "older manifest" | "missing older manifest" => {
                Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1)
            }
            "partial canonical wire" | "receipt-only canonical wire" => kura
                .block_store
                .lock()
                .path_to_blockchain
                .join("blocks.data"),
            "partial checkpoint" | "receipt-only checkpoint" => kura.wsv_checkpoint_path(1),
            "partial commit manifest" => kura.commit_manifest_path(1),
            _ => unreachable!(),
        };
        if damaged_kind.starts_with("partial") {
            fs::remove_file(Kura::native_amx_participant_receipt_path_for_entry(
                &entry,
                &kura.store_root,
                1,
            ))
            .expect("leave highest authenticated manifest pending receipt repair");
        } else if damaged_kind.starts_with("receipt-only") {
            fs::remove_file(Kura::native_amx_application_manifest_path_for_entry(
                &entry,
                &kura.store_root,
                1,
            ))
            .expect("leave highest structural receipt pending manifest repair");
        }
        let damaged_bytes = if damaged_kind.starts_with("missing") {
            fs::remove_file(&damaged_path).expect("puncture retained complete history");
            None
        } else {
            let mut bytes = fs::read(&damaged_path).expect("real occupied evidence bytes");
            bytes[0] ^= 0x80;
            fs::write(&damaged_path, &bytes).expect("damage real occupied evidence");
            Some(bytes)
        };
        let evidence_directory = Kura::lane_artifact_dir(&entry.blocks_dir(&kura.store_root));
        let before = snapshot_regular_files_recursively(&evidence_directory);
        let guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        kura.bind_consensus_output_guard(Arc::clone(&guard))
            .expect("bind guard");
        assert!(
            kura.consensus_storage_read(
                kura.read_native_amx_participant_application_history(entry.lane_id)
            )
            .is_err(),
            "{damaged_kind} cannot become exact lookup absence or repairable debt"
        );
        assert!(guard.restart_required());
        assert!(guard.acquire().is_none());
        assert_eq!(fs::read(&damaged_path).ok(), damaged_bytes);
        assert_eq!(
            snapshot_regular_files_recursively(&evidence_directory),
            before
        );
    }
}

#[test]
fn native_amx_history_read_distinguishes_empty_and_authenticated_pruned_suffix() {
    let temp_dir = TempDir::new().expect("bounded history fixture");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.lane_history_retention = nonzero!(1_usize);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("open bounded history");
    establish_dummy_store_primary_anchor(&kura);
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("active route");
    establish_dummy_store_primary_anchor(&kura);
    let empty = kura
        .read_native_amx_participant_application_history(entry.lane_id)
        .expect("genuine absence");
    assert!(empty.entries().next().is_none());
    assert!(empty.get(1).is_none());
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2]);
    kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
        .expect("owned retention cleanup");
    let retained = kura
        .read_native_amx_participant_application_history(entry.lane_id)
        .expect("authenticated pruned suffix");
    assert!(
        retained.get(1).is_none(),
        "pruned predecessor remains unavailable"
    );
    assert_eq!(
        retained
            .entries()
            .map(|(height, _)| height)
            .collect::<Vec<_>>(),
        vec![2]
    );
    assert_eq!(
        retained.get(2),
        Some(&NativeAmxParticipantApplicationObservation::Applied(
            receipts[1].clone()
        ))
    );
}

fn native_amx_prune_planning_maps_for_test(
    kura: &Kura,
    entry: &LaneStorageEntry,
    receipts: &[NativeAmxParticipantApplicationReceiptArtifact],
) -> (
    BTreeMap<u64, NativeAmxParticipantApplicationManifestArtifactV1>,
    BTreeMap<u64, NativeAmxParticipantApplicationReceiptArtifact>,
) {
    let manifests = receipts
        .iter()
        .map(|receipt| {
            let height = receipt.participant_proposal.descriptor.lane_block_height;
            let path = Kura::native_amx_application_manifest_path_for_entry(
                entry,
                &kura.store_root,
                height,
            );
            let manifest =
                norito::decode_canonical::<NativeAmxParticipantApplicationManifestArtifactV1>(
                    &fs::read(path).expect("read exact planner fixture manifest"),
                )
                .expect("decode exact planner fixture manifest");
            (height, manifest)
        })
        .collect();
    let receipts = receipts
        .iter()
        .map(|receipt| {
            (
                receipt.participant_proposal.descriptor.lane_block_height,
                receipt.clone(),
            )
        })
        .collect();
    (manifests, receipts)
}

#[test]
fn native_amx_prospective_prune_plan_is_zero_until_exact_retention_is_crossed() {
    let temp = TempDir::new().expect("prospective Native prune directory");
    let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("open prospective Native prune Kura");
    establish_dummy_store_primary_anchor(&kura);
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("planner route");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
    let (mut manifests, mut receipt_map) =
        native_amx_prune_planning_maps_for_test(&kura, &entry, &receipts);
    let incoming_manifest = manifests.remove(&3).expect("incoming manifest");
    let incoming_receipt = receipt_map.remove(&3).expect("incoming receipt");
    assert!(
        Kura::plan_native_amx_evidence_prune_intent_from_artifacts(
            nonzero!(2_usize),
            kura.native_amx_participant_evidence_file_bytes(),
            kura.native_amx_evidence_prune_intent_max_bytes(),
            &manifests,
            &receipt_map,
        )
        .expect("retained pair plan")
        .is_none(),
        "fitting retained evidence needs no journal"
    );
    manifests.insert(3, incoming_manifest);
    receipt_map.insert(3, incoming_receipt);
    let plan = Kura::plan_native_amx_evidence_prune_intent_from_artifacts(
        nonzero!(2_usize),
        kura.native_amx_participant_evidence_file_bytes(),
        kura.native_amx_evidence_prune_intent_max_bytes(),
        &manifests,
        &receipt_map,
    )
    .expect("incoming pair plan")
    .expect("one removed pair requires a journal");
    let expected = native_amx_prune_intent_for_test(&kura, &entry, &receipts[2], &[1]);
    assert_eq!(
        plan, expected,
        "reserve the actual prefix and canonical settlement preimage"
    );
    let encoded = norito::encode_canonical(&plan).expect("exact planned journal");
    assert!(encoded.len() < kura.native_amx_evidence_prune_intent_max_bytes());
    assert_eq!(
        plan.removed_settlements,
        vec![receipts[0].participant_settlement.clone()]
    );
    assert!(
        Kura::plan_native_amx_evidence_prune_intent_from_artifacts(
            nonzero!(3_usize),
            kura.native_amx_participant_evidence_file_bytes(),
            kura.native_amx_evidence_prune_intent_max_bytes(),
            &manifests,
            &receipt_map,
        )
        .expect("all three pairs fit")
        .is_none()
    );
}

#[test]
fn native_amx_prospective_prune_plan_matches_authenticated_publication() {
    let temp = TempDir::new().expect("authenticated Native prune directory");
    let mut config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    config.lane_history_retention = nonzero!(2_usize);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("open authenticated Native prune Kura");
    establish_dummy_store_primary_anchor(&kura);
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("planner route");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
    let (manifests, receipt_map) =
        native_amx_prune_planning_maps_for_test(&kura, &entry, &receipts);
    let prospective = Kura::plan_native_amx_evidence_prune_intent_from_artifacts(
        config.lane_history_retention,
        kura.native_amx_participant_evidence_file_bytes(),
        kura.native_amx_evidence_prune_intent_max_bytes(),
        &manifests,
        &receipt_map,
    )
    .expect("prospective journal")
    .expect("oldest pair removed");
    let before = native_amx_prune_evidence_snapshot(&kura, &entry, &[1, 2, 3]);
    let _prune = kura.prune_lock.lock();
    let _canonical = kura.canonical_chain_lock.lock();
    let _geometry = kura.lane_geometry_lock.lock();
    let _sidecar = kura.sidecar_lock.lock();
    let namespace = kura
        .native_amx_evidence_namespace_for_entry(&entry)
        .expect("bound namespace");
    let inventory = kura
        .inventory_native_amx_evidence_files_locked(&namespace, true)
        .expect("bounded inventory");
    let authenticated = kura
        .plan_native_amx_evidence_pair_prune_locked(&entry, &namespace, &inventory)
        .expect("authenticated journal plan")
        .expect("oldest pair removed");
    assert_eq!(prospective, authenticated);
    assert_native_amx_prune_evidence_snapshot(&before, "planning is read-only");
    let mut resources = kura
        .begin_total_disk_usage_mutation()
        .with_resource_children(1);
    kura.prune_native_amx_evidence_pairs_locked(&mut resources, &entry, &namespace)
        .expect("publish and finish exact authenticated prune journal");
    resources.finish();
    for height in [1, 2, 3] {
        for path in [
            Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, height),
            Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, height),
        ] {
            assert_eq!(
                path.exists(),
                height != 1,
                "only the exact oldest pair may be removed"
            );
        }
    }
    let (stable, temporary) = native_amx_prune_special_paths(&kura, &entry);
    assert!(
        !stable.exists() && !temporary.exists(),
        "journal lifetime ends after completed cleanup"
    );
}

#[test]
fn native_amx_prospective_prune_plan_preserves_byte_and_history_bounds() {
    let temp = TempDir::new().expect("bounded Native prune directory");
    let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("open bounded Native prune Kura");
    establish_dummy_store_primary_anchor(&kura);
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("planner route");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
    let (mut manifests, mut receipt_map) =
        native_amx_prune_planning_maps_for_test(&kura, &entry, &receipts);
    let pair_limit = manifests
        .iter()
        .map(|(height, manifest)| {
            u64::try_from(
                manifest.encode_framed().unwrap().len()
                    + receipt_map[height].encode_framed().unwrap().len(),
            )
            .unwrap()
        })
        .max()
        .expect("three pairs");
    let byte_plan = Kura::plan_native_amx_evidence_prune_intent_from_artifacts(
        nonzero!(3_usize),
        pair_limit,
        kura.native_amx_evidence_prune_intent_max_bytes(),
        &manifests,
        &receipt_map,
    )
    .expect("byte-based prefix plan")
    .expect("only newest pair fits");
    assert_eq!(
        byte_plan
            .entries
            .iter()
            .map(|entry| entry.participant_height)
            .collect::<Vec<_>>(),
        vec![1, 1, 2, 2]
    );
    let actual_journal_len = norito::encode_canonical(&byte_plan).unwrap().len();
    assert!(
        Kura::plan_native_amx_evidence_prune_intent_from_artifacts(
            nonzero!(3_usize),
            pair_limit,
            actual_journal_len - 1,
            &manifests,
            &receipt_map,
        )
        .is_err(),
        "hard journal bound cannot be bypassed by variable settlement preimages"
    );
    assert!(
        Kura::plan_native_amx_evidence_prune_intent_from_artifacts(
            nonzero!(3_usize),
            0,
            kura.native_amx_evidence_prune_intent_max_bytes(),
            &manifests,
            &receipt_map,
        )
        .is_err(),
        "protected pair must fit the stable bound"
    );
    manifests.remove(&2);
    receipt_map.remove(&2);
    assert!(
        Kura::plan_native_amx_evidence_prune_intent_from_artifacts(
            nonzero!(3_usize),
            kura.native_amx_participant_evidence_file_bytes(),
            kura.native_amx_evidence_prune_intent_max_bytes(),
            &manifests,
            &receipt_map,
        )
        .is_err(),
        "prospective accounting cannot hide an interior Native settlement"
    );
}

#[test]
fn native_amx_reconstructed_cleanup_reserves_exact_journal_before_growth() {
    let temp = TempDir::new().expect("Native exact journal recovery directory");
    let mut config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    config.lane_history_retention = nonzero!(2_usize);
    let (mut kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("open Native journal recovery");
    establish_dummy_store_primary_anchor(&kura);
    let entry = kura
        .lane_storage_entry(LaneId::SINGLE)
        .expect("Native recovery route");
    let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
    let newest = &receipts[2];
    let (latest, _) = native_amx_latest_index_test_paths(&kura, &entry);
    write_synced_native_amx_test_file(
        &latest,
        &norito::encode_canonical(&NativeAmxParticipantReceiptLatestIndexV2::from_receipt(
            newest,
        ))
        .expect("encode exact latest"),
    );
    let journal = native_amx_prune_intent_for_test(&kura, &entry, newest, &[1]);
    let exact = u64::try_from(
        norito::encode_canonical(&journal)
            .expect("encode exact journal")
            .len(),
    )
    .expect("journal length");
    let before = snapshot_regular_files_recursively(&kura.store_root);
    kura.rebuild_native_amx_publication_capacity_on_startup()
        .expect("reconstruct all retained Native owners read-only");
    assert_eq!(
        snapshot_regular_files_recursively(&kura.store_root),
        before,
        "reconstruction must not publish, prune or repair"
    );
    assert_eq!(
        kura.native_amx_publication_capacity_reserved_bytes()
            .expect("Native capacity"),
        exact
    );
    assert!(
        exact
            < u64::try_from(kura.native_amx_evidence_prune_intent_max_bytes())
                .expect("decoder ceiling")
    );
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("merge capacity"),
        0
    );
    let exact_limit = canonical_storage_budget_base_for_test(&kura);
    Arc::get_mut(&mut kura)
        .expect("exclusive recovery fixture")
        .max_disk_usage_bytes = exact_limit - 1;
    let rejected = kura
        .rebuild_native_amx_publication_capacity_on_startup()
        .expect_err("one byte below exact operation reservation fails before repair");
    assert!(
        matches!(rejected, Error::StorageBudgetExceeded { required, .. } if required == exact_limit)
    );
    assert_eq!(snapshot_regular_files_recursively(&kura.store_root), before);
    assert!(
        !kura
            .native_amx_resident_recovery_complete
            .load(Ordering::Acquire)
    );
    assert_eq!(
        kura.native_amx_publication_capacity_reserved_bytes()
            .expect("failed reconstruction retains ownership"),
        exact
    );
    Arc::get_mut(&mut kura)
        .expect("exclusive recovery retry")
        .max_disk_usage_bytes = exact_limit;
    kura.rebuild_native_amx_publication_capacity_on_startup()
        .expect("exact retry fits");
    kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
        .expect("actual prune with reserved journal");
    assert_eq!(
        kura.native_amx_publication_capacity_reserved_bytes()
            .expect("capacity after exact cleanup"),
        0
    );
    assert!(
        !Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1).exists()
    );
    assert!(
        !Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1).exists()
    );
    assert_eq!(
        kura.read_native_amx_participant_application_history(LaneId::SINGLE)
            .expect("remaining exact history")
            .entries()
            .count(),
        2
    );
}

#[test]
fn native_amx_cleanup_barrier_failure_retains_ownership_until_exact_retry() {
    for (label, barrier) in strict_progress_sidecar_failure_modes().into_iter().skip(2) {
        let (fixture, _entry, receipt) = native_amx_indexed_latest_index_evidence_fixture();
        let kura = &fixture.kura;
        let carrier = Kura::native_amx_publication_carrier(&fixture.block)
            .expect("exact pending cleanup carrier");
        let (sibling_manifest, sibling_receipt) = native_amx_participant_application_artifacts(
            &fixture.manifest,
            HashOf::new(&fixture.finality),
        )
        .expect("exact two-route cleanup artifacts")
        .into_iter()
        .nth(1)
        .expect("the other real publication route");
        {
            let _prune = kura.prune_lock.lock();
            kura.cleanup_native_amx_participant_application_evidence_under_publication_guard(
                &sibling_receipt,
            )
            .expect("complete the sibling before testing the final route barrier");
        }
        let owned = kura
            .native_amx_publication_capacity_reservations
            .lock()
            .clone();
        assert_eq!(owned.len(), 1);
        let owner = owned
            .get(&carrier)
            .expect("store-owned final cleanup obligation");
        assert_eq!(owner.routes.len(), 2);
        assert_eq!(
            owner
                .routes
                .values()
                .filter(|route| !route.cleanup_complete)
                .count(),
            1,
            "only the selected route remains unfinished"
        );
        assert!(
            owner
                .routes
                .values()
                .all(|route| route.outstanding_components.is_empty())
        );
        let record = owner
            .index_record
            .as_ref()
            .expect("real pending index authority");
        let indexed = Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .expect("exact index before the cleanup barrier")
            .records;
        assert_eq!(indexed, BTreeMap::from([(carrier, record.clone())]));
        // A completed sibling cannot acquire new cleanup work on an exact store
        // retry while the selected route still owns the durable carrier index.
        let sibling_entry = kura
            .lane_storage_entry(sibling_manifest.leaf.lane_id)
            .expect("live completed sibling route");
        let (sibling_latest, sibling_temporary) =
            native_amx_latest_index_test_paths(kura, &sibling_entry);
        let sibling_bytes = norito::encode_canonical(
            &NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&sibling_receipt),
        )
        .expect("exact redundant sibling temporary bytes");
        assert_eq!(
            fs::read(&sibling_latest).expect("completed sibling stable pointer"),
            sibling_bytes
        );
        assert!(!sibling_temporary.exists());
        let completed_sibling = owner
            .routes
            .iter()
            .find(|(route, _)| route.lane_id == sibling_manifest.leaf.lane_id)
            .map(|(_, route)| route)
            .expect("completed sibling ownership");
        assert!(completed_sibling.cleanup_complete);
        assert!(!completed_sibling.physical_cleanup_pending);
        let without_redundant_temporary = snapshot_regular_files_recursively(&kura.store_root);
        write_synced_native_amx_test_file(&sibling_temporary, &sibling_bytes);
        let redundant_files = snapshot_regular_files_recursively(&kura.store_root);
        let redundant_enforced = kura
            .refresh_disk_usage_bytes()
            .expect("account exact redundant sibling temporary");
        let redundant_total = kura.disk_usage_bytes().expect("redundant temporary total");
        let retry_error = kura
            .store_block(Arc::clone(&fixture.block))
            .expect_err("an exact retry cannot reopen a completed sibling cleanup");
        assert!(
            matches!(&retry_error, Error::PruneIntentConflict(reason)
                if reason == "Native AMX exact retry changed an owned publication component"),
            "reject the exact completed-route cleanup transition: {retry_error}"
        );
        assert_eq!(
            *kura.native_amx_publication_capacity_reservations.lock(),
            owned
        );
        assert_eq!(
            Kura::read_native_amx_publication_index_for_store(&kura.store_root)
                .expect("unchanged index after rejected completed-route retry")
                .records,
            indexed
        );
        assert_eq!(
            snapshot_regular_files_recursively(&kura.store_root),
            redundant_files
        );
        assert_eq!(
            kura.kura_disk_usage_bytes()
                .expect("unchanged retry enforced usage"),
            redundant_enforced
        );
        assert_eq!(
            kura.kura_total_disk_usage_bytes()
                .expect("unchanged retry total usage"),
            redundant_total
        );
        assert_eq!(
            kura.disk_usage_bytes()
                .expect("unchanged retry cached usage"),
            redundant_total
        );
        assert_eq!(
            fs::read(&sibling_temporary).expect("owned redundant temporary"),
            sibling_bytes
        );
        fs::remove_file(&sibling_temporary)
            .expect("remove only the test-owned redundant temporary");
        sync_dir(
            sibling_temporary
                .parent()
                .expect("sibling temporary parent"),
        )
        .expect("sync exact redundant temporary removal");
        assert_eq!(
            snapshot_regular_files_recursively(&kura.store_root),
            without_redundant_temporary
        );
        let index_relative = PathBuf::from(NATIVE_AMX_PUBLICATION_INDEX_DIRECTORY)
            .join(record.file_name().expect("exact cleanup index filename"));
        let before = snapshot_regular_files_recursively(&kura.store_root);
        let index_bytes = record.encoded().expect("exact persisted index bytes");
        assert_eq!(before.get(&index_relative), Some(&index_bytes));
        let enforced_before = kura
            .refresh_disk_usage_bytes()
            .expect("refresh pre-fault usage");
        let total_before = kura.disk_usage_bytes().expect("pre-fault total usage");
        {
            let _prune = kura.prune_lock.lock();
            barrier.inject();
            let error = kura
                .cleanup_native_amx_participant_application_evidence_under_publication_guard(
                    &receipt,
                )
                .expect_err("failed directory durability must retain cleanup ownership");
            assert!(
                matches!(&error, Error::IO(source, _)
                if source.kind() == std::io::ErrorKind::InvalidData
                    && source.to_string() == "Native AMX capacity completion directory durability sync failed"),
                "{label} must fail at the exact cleanup directory barrier: {error}"
            );
        }
        assert_eq!(
            *kura.native_amx_publication_capacity_reservations.lock(),
            owned,
            "{label} must not release or change any ownership on failure"
        );
        assert_eq!(
            Kura::read_native_amx_publication_index_for_store(&kura.store_root)
                .expect("retained exact index after failed barrier")
                .records,
            indexed
        );
        assert_eq!(snapshot_regular_files_recursively(&kura.store_root), before);
        assert_eq!(
            kura.kura_disk_usage_bytes()
                .expect("enforced usage after failure"),
            enforced_before
        );
        assert_eq!(
            kura.kura_total_disk_usage_bytes()
                .expect("total usage after failure"),
            total_before
        );
        assert_eq!(
            kura.disk_usage_bytes().expect("cached usage after failure"),
            total_before
        );
        {
            let _prune = kura.prune_lock.lock();
            kura.cleanup_native_amx_participant_application_evidence_under_publication_guard(
                &receipt,
            )
            .expect("exact durable cleanup retry");
        }
        assert!(
            kura.native_amx_publication_capacity_reservations
                .lock()
                .is_empty(),
            "{label} retry closes the exact owner"
        );
        assert!(
            Kura::read_native_amx_publication_index_for_store(&kura.store_root)
                .expect("completed publication index")
                .records
                .is_empty()
        );
        // Successful final cleanup retires only the exact operation index. Every
        // canonical, geometry and Native evidence byte remains unchanged.
        let mut expected_after = before;
        assert_eq!(
            expected_after.remove(&index_relative),
            Some(index_bytes.clone())
        );
        assert_eq!(
            snapshot_regular_files_recursively(&kura.store_root),
            expected_after
        );
        let retired_bytes = u64::try_from(index_bytes.len()).expect("bounded retired index length");
        assert_eq!(
            kura.kura_disk_usage_bytes()
                .expect("enforced usage after retry"),
            enforced_before - retired_bytes
        );
        assert_eq!(
            kura.kura_total_disk_usage_bytes()
                .expect("total usage after retry"),
            total_before - retired_bytes
        );
        assert_eq!(
            kura.disk_usage_bytes().expect("cached usage after retry"),
            total_before - retired_bytes
        );
    }
}

#[test]
fn native_amx_pending_prune_journal_is_physical_not_reserved_twice() {
    for temporary in [false, true] {
        let temp = TempDir::new().expect("Native pending journal directory");
        let mut config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
        config.lane_history_retention = nonzero!(2_usize);
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("open pending journal fixture");
        establish_dummy_store_primary_anchor(&kura);
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("pending journal route");
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, &[1, 2, 3]);
        let newest = &receipts[2];
        let (latest, _) = native_amx_latest_index_test_paths(&kura, &entry);
        write_synced_native_amx_test_file(
            &latest,
            &norito::encode_canonical(&NativeAmxParticipantReceiptLatestIndexV2::from_receipt(
                newest,
            ))
            .expect("encode pending journal latest"),
        );
        let intent = native_amx_prune_intent_for_test(&kura, &entry, newest, &[1]);
        let (stable, staged) = native_amx_prune_special_paths(&kura, &entry);
        write_synced_native_amx_test_file(
            if temporary { &staged } else { &stable },
            &norito::encode_canonical(&intent).expect("encode pending exact journal"),
        );
        let before = snapshot_regular_files_recursively(&kura.store_root);
        kura.rebuild_native_amx_publication_capacity_on_startup()
            .expect("read-only pending-journal reservation reconstruction");
        assert_eq!(snapshot_regular_files_recursively(&kura.store_root), before);
        assert_eq!(
            kura.native_amx_publication_capacity_reserved_bytes()
                .expect("physical journal allocation"),
            0
        );
        assert_eq!(
            kura.native_amx_publication_capacity_reservations
                .lock()
                .len(),
            1,
            "zero growth still owns cleanup completion"
        );
        {
            let owners = kura.native_amx_publication_capacity_reservations.lock();
            let owner = owners
                .values()
                .next()
                .expect("physical prune cleanup owner");
            assert!(
                owner.index_record.is_none(),
                "maintenance has no publication index"
            );
            assert_eq!(owner.routes.len(), 1);
            let route = owner
                .routes
                .values()
                .next()
                .expect("exact prune maintenance route");
            assert!(route.physical_cleanup_pending);
            assert_eq!(route.prune_journal_bytes, 0);
            assert!(route.outstanding_components.is_empty());
            assert!(!route.cleanup_complete);
        }
        kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect("finish exact pending journal");
        assert!(
            kura.native_amx_publication_capacity_reservations
                .lock()
                .is_empty()
        );
        assert!(!stable.exists() && !staged.exists());
    }
}

#[test]
fn native_amx_capacity_restart_after_store_without_native_files_reconstructs_exact_owner() {
    let NativeAmxPublicationCapacityFixture {
        _temp_dir,
        kura,
        block,
        manifest,
        finality,
        lane_config,
    } = native_amx_publication_capacity_fixture();
    kura.store_block(Arc::clone(&block))
        .expect("durably store Native carrier before evidence");
    let commit_receipt = kura
        .store_v2_finality_artifact(&finality)
        .expect("durable exact carrier finality");
    assert_v2_commit_receipt_matches_artifact(&commit_receipt, &finality);
    let expected = kura
        .native_amx_publication_capacity_reserved_bytes()
        .expect("live operation allocation");
    assert!(expected > 0);
    for leaf in manifest.entries() {
        let entry = kura
            .lane_storage_entry(leaf.leaf.lane_id)
            .expect("active publication route");
        assert!(
            !Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1)
                .exists()
        );
        assert!(
            !Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1)
                .exists()
        );
    }
    let (incarnations, activation_heights): (BTreeMap<_, _>, BTreeMap<_, _>) = lane_config
        .entries()
        .iter()
        .map(|entry| {
            let (incarnation, activation) = kura
                .active_lane_incarnation_marker(
                    &kura
                        .lane_storage_entry(entry.lane_id)
                        .expect("exact active identity"),
                )
                .expect("authenticate the original journal-published lane");
            ((entry.lane_id, incarnation), (entry.lane_id, activation))
        })
        .unzip();
    let before_restart = snapshot_regular_files_recursively(&kura.store_root);
    let network_id = kura.bound_lane_storage_network().unwrap();
    drop(kura);
    let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("restart before any Native publication");
    assert_eq!(
        reopened
            .native_amx_publication_capacity_reserved_bytes()
            .expect("reconstructed allocation"),
        expected
    );
    assert_eq!(
        reopened
            .native_amx_publication_capacity_reservations
            .lock()
            .len(),
        1
    );
    assert_eq!(
        reopened
            .post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("ordinary carrier has no merge owner"),
        0
    );
    assert_eq!(
        snapshot_regular_files_recursively(&reopened.store_root),
        before_restart,
        "cold reservation reconstruction must not publish secondary lane evidence"
    );
    assert_eq!(reopened.lane_storage_entries.lock().len(), 0);
    reopened.bind_lane_storage_network(network_id).unwrap();
    reopened
        .recover_lane_geometry_journal(&lane_config, &incarnations, &activation_heights)
        .expect("authenticate and restore the actual secondary lane geometry");
    reopened
        .finish_restored_lane_segments_with_geometry(&lane_config)
        .expect("complete startup recovery after authenticated geometry restoration");
    reopened
        .store_block(Arc::clone(&block))
        .expect("exact durable carrier retry");
    assert_eq!(
        reopened
            .native_amx_publication_capacity_reserved_bytes()
            .expect("deduplicated restart allocation"),
        expected
    );
}

#[test]
fn native_amx_capacity_restart_with_highest_half_pair_keeps_missing_component() {
    for retained_kind in [
        NativeAmxEvidenceKind::Manifest,
        NativeAmxEvidenceKind::Receipt,
    ] {
        let NativeAmxPublicationCapacityFixture {
            _temp_dir,
            kura,
            block,
            manifest,
            finality,
            lane_config,
        } = native_amx_publication_capacity_fixture();
        kura.store_block(Arc::clone(&block))
            .expect("store half-pair carrier");
        let commit_receipt = kura
            .store_v2_finality_artifact(&finality)
            .expect("half-pair finality");
        assert_v2_commit_receipt_matches_artifact(&commit_receipt, &finality);
        let expected = kura
            .native_amx_publication_capacity_reserved_bytes()
            .expect("all component allocation");
        let artifacts =
            native_amx_participant_application_artifacts(&manifest, HashOf::new(&finality))
                .expect("exact finality artifacts");
        let (manifest, receipt) = &artifacts[0];
        let entry = kura
            .lane_storage_entry(manifest.leaf.lane_id)
            .expect("half-pair route");
        let (path, bytes) = match retained_kind {
            NativeAmxEvidenceKind::Manifest => (
                Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1),
                manifest.encode_framed().expect("manifest bytes"),
            ),
            NativeAmxEvidenceKind::Receipt => (
                Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1),
                receipt.encode_framed().expect("receipt bytes"),
            ),
        };
        fs::create_dir_all(path.parent().expect("half-pair directory"))
            .expect("create exact fixture namespace");
        write_synced_native_amx_test_file(&path, &bytes);
        let (incarnations, activation_heights): (BTreeMap<_, _>, BTreeMap<_, _>) = lane_config
            .entries()
            .iter()
            .map(|entry| {
                let (incarnation, activation) = kura
                    .active_lane_incarnation_marker(
                        &kura
                            .lane_storage_entry(entry.lane_id)
                            .expect("exact active identity"),
                    )
                    .expect("authenticate the original journal-published lane");
                ((entry.lane_id, incarnation), (entry.lane_id, activation))
            })
            .unzip();
        let before_restart = snapshot_regular_files_recursively(&kura.store_root);
        let network_id = kura.bound_lane_storage_network().unwrap();
        drop(kura);
        let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
        let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("restart with highest half-pair");
        let remaining = expected - u64::try_from(bytes.len()).expect("stable half allocation");
        assert_eq!(
            reopened
                .native_amx_publication_capacity_reserved_bytes()
                .expect("half-pair remaining capacity"),
            remaining
        );
        assert_eq!(fs::read(path).expect("retained half is unchanged"), bytes);
        assert_eq!(
            snapshot_regular_files_recursively(&reopened.store_root),
            before_restart,
            "cold reservation reconstruction must not publish secondary lane evidence"
        );
        assert_eq!(reopened.lane_storage_entries.lock().len(), 0);
        reopened.bind_lane_storage_network(network_id).unwrap();
        reopened
            .recover_lane_geometry_journal(&lane_config, &incarnations, &activation_heights)
            .expect("authenticate and restore the actual secondary lane geometry");
        reopened
            .finish_restored_lane_segments_with_geometry(&lane_config)
            .expect("complete startup recovery after authenticated geometry restoration");
        reopened
            .store_block(Arc::clone(&block))
            .expect("exact retry preserves actual finality hash and half-pair credit");
        assert_eq!(
            reopened
                .native_amx_publication_capacity_reserved_bytes()
                .expect("half-pair retry"),
            remaining
        );
    }
}

#[test]
fn native_amx_capacity_promoted_temporary_never_regains_consumed_allocation_on_retry() {
    for kind in [
        NativeAmxEvidenceKind::Manifest,
        NativeAmxEvidenceKind::Receipt,
    ] {
        let fixture = native_amx_publication_capacity_fixture();
        let kura = &fixture.kura;
        kura.store_block(Arc::clone(&fixture.block))
            .expect("store temporary carrier");
        let commit_receipt = kura
            .store_v2_finality_artifact(&fixture.finality)
            .expect("temporary exact finality");
        assert_v2_commit_receipt_matches_artifact(&commit_receipt, &fixture.finality);
        let artifacts = native_amx_participant_application_artifacts(
            &fixture.manifest,
            HashOf::new(&fixture.finality),
        )
        .expect("temporary exact pair");
        let (manifest, receipt) = &artifacts[0];
        let entry = kura
            .lane_storage_entry(manifest.leaf.lane_id)
            .expect("temporary route");
        let manifest_path =
            Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
        let receipt_path =
            Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
        fs::create_dir_all(manifest_path.parent().expect("temporary directory"))
            .expect("create fixture namespace");
        let (path, bytes) = match kind {
            NativeAmxEvidenceKind::Manifest => (
                manifest_path,
                manifest.encode_framed().expect("manifest temporary bytes"),
            ),
            NativeAmxEvidenceKind::Receipt => {
                write_synced_native_amx_test_file(
                    &manifest_path,
                    &manifest
                        .encode_framed()
                        .expect("stable prerequisite manifest"),
                );
                (
                    receipt_path,
                    receipt.encode_framed().expect("receipt temporary bytes"),
                )
            }
        };
        let temporary = path.with_extension("norito.tmp");
        write_synced_native_amx_test_file(&temporary, &bytes);
        kura.rebuild_native_amx_publication_capacity_on_startup()
            .expect("read-only physical temporary credit");
        let before = kura
            .native_amx_publication_capacity_reserved_bytes()
            .expect("temporary-aware reserve");
        {
            let _prune = kura.prune_lock.lock();
            let _canonical = kura.canonical_chain_lock.lock();
            let _geometry = kura.lane_geometry_lock.lock();
            let _sidecar = kura.sidecar_lock.lock();
            let namespace = kura
                .native_amx_evidence_namespace_for_entry(&entry)
                .expect("bound temporary namespace");
            with_native_resource_batch_for_test(kura, |resources| {
                kura.recover_native_amx_evidence_publication_temp_locked(
                    resources,
                    &entry,
                    &namespace,
                    NativeAmxEvidenceRecoveryPhase::Startup,
                )
            })
            .expect("promote exact physical temporary");
        }
        assert!(!temporary.exists());
        assert_eq!(fs::read(path).expect("promoted artifact"), bytes);
        kura.store_block(Arc::clone(&fixture.block))
            .expect("temporary-to-stable exact retry must not report allocation growth");
        assert_eq!(
            kura.native_amx_publication_capacity_reserved_bytes()
                .expect("retry remaining allocation"),
            before
        );
    }
}

#[test]
fn native_amx_capacity_exact_latest_temporary_recovers_without_second_allocation() {
    for stable_present in [false, true] {
        let temp = TempDir::new().expect("Native exact latest temporary directory");
        let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
        let (mut kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("open latest temporary fixture");
        establish_dummy_store_primary_anchor(&kura);
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("latest temporary route");
        let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
        let (stable, temporary) = native_amx_latest_index_test_paths(&kura, &entry);
        let bytes = norito::encode_canonical(
            &NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipt),
        )
        .expect("exact latest temporary bytes");
        assert!(
            !stable.exists(),
            "fixture begins without a derived latest pointer"
        );
        if stable_present {
            write_synced_native_amx_test_file(&stable, &bytes);
        }
        write_synced_native_amx_test_file(&temporary, &bytes);
        kura.rebuild_native_amx_publication_capacity_on_startup()
            .expect("authenticate physical latest temporary");
        assert_eq!(
            kura.native_amx_publication_capacity_reserved_bytes()
                .expect("latest physical credit"),
            0
        );
        assert_eq!(
            kura.native_amx_publication_capacity_reservations
                .lock()
                .len(),
            1
        );
        {
            let owners = kura.native_amx_publication_capacity_reservations.lock();
            let owner = owners
                .values()
                .next()
                .expect("physical latest cleanup owner");
            assert!(
                owner.index_record.is_none(),
                "cleanup must not invent a publication index"
            );
            assert_eq!(owner.routes.len(), 1);
            let route = owner
                .routes
                .values()
                .next()
                .expect("exact maintenance route");
            assert!(route.physical_cleanup_pending);
            assert!(!route.cleanup_complete);
            assert_eq!(
                route
                    .outstanding_components
                    .contains(&NativeAmxPublicationComponent::Latest),
                !stable_present,
                "an identical stable pointer needs only temporary cleanup"
            );
        }
        let exact = canonical_storage_budget_base_for_test(&kura);
        Arc::get_mut(&mut kura)
            .expect("exclusive latest temporary fixture")
            .max_disk_usage_bytes = exact;
        kura.rebuild_native_amx_publication_capacity_on_startup()
            .expect("restart exact physical limit admits no duplicate allocation");
        kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect("promote existing exact temporary");
        assert_eq!(fs::read(&stable).expect("promoted latest"), bytes);
        assert!(!temporary.exists());
        assert!(
            kura.native_amx_publication_capacity_reservations
                .lock()
                .is_empty()
        );
        let completed_files = snapshot_regular_files_recursively(&kura.store_root);
        kura.rebuild_native_amx_publication_capacity_on_startup()
            .expect("completed pointer without physical residue needs no maintenance owner");
        assert!(
            kura.native_amx_publication_capacity_reservations
                .lock()
                .is_empty()
        );
        assert_eq!(
            kura.native_amx_publication_capacity_reserved_bytes()
                .expect("completed cleanup allocation"),
            0
        );
        assert_eq!(
            snapshot_regular_files_recursively(&kura.store_root),
            completed_files
        );
    }
}

#[test]
fn native_amx_capacity_restart_finds_unpublished_carrier_below_ordinary_tip() {
    let NativeAmxPublicationCapacityFixture {
        _temp_dir,
        kura,
        block,
        manifest: _,
        finality,
        lane_config,
    } = native_amx_publication_capacity_fixture();
    kura.store_block(Arc::clone(&block))
        .expect("store unpublished Native carrier");
    let commit_receipt = kura
        .store_v2_finality_artifact(&finality)
        .expect("exact first-carrier finality");
    assert_v2_commit_receipt_matches_artifact(&commit_receipt, &finality);
    let expected = kura
        .native_amx_publication_capacity_reserved_bytes()
        .expect("unpublished Native allocation");
    assert!(expected > 0);
    let mut ordinary: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, Some(block.as_ref()))
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    attach_ok_results_to_block(&mut ordinary);
    kura.store_block(ordinary)
        .expect("ordinary successor is independent of Native publication");
    assert_eq!(kura.blocks_count(), 2);
    assert_eq!(
        kura.native_amx_publication_capacity_reserved_bytes()
            .expect("live Native owner survives ordinary successor"),
        expected
    );
    drop(kura);
    let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("restart after ordinary successor");
    assert_eq!(
        reopened
            .native_amx_publication_capacity_reserved_bytes()
            .expect("exact older Native recovery allocation"),
        expected
    );
    assert_eq!(
        reopened
            .native_amx_publication_capacity_reservations
            .lock()
            .len(),
        1
    );
}

#[test]
fn native_amx_owned_carrier_body_pin_overrides_remote_replica_eviction() {
    // Exercise the eviction boundary with its existing signed complete-wire fixture.
    // Real Native carrier registration and completion are exercised separately above.
    let temp = TempDir::new().expect("Native pending body pin directory");
    let (_, kura, blocks) = open_eviction_compaction_fixture(&temp, 4);
    let block = &blocks[1];
    let carrier = Kura::native_amx_publication_carrier(block).expect("exact signed body pin");
    let (_, required) = advertise_required_replicas(&kura, nonzero!(2_usize));
    kura.native_amx_publication_capacity_reservations
        .lock()
        .insert(
            carrier,
            NativeAmxPublicationCapacityReservation {
                index_record: None,
                index_additional_bytes: 0,
                routes: BTreeMap::new(),
            },
        );
    let before = block.encode_wire().expect("exact original body");
    assert_eq!(
        kura.evict_block_bodies(required)
            .expect("owned-body eviction admission"),
        0
    );
    assert!(
        !kura
            .block_store
            .lock()
            .read_block_index(1)
            .expect("pinned inline index")
            .is_evicted()
    );
    assert_eq!(
        kura.get_block_without_merge_sidecar(nonzero!(2_usize))
            .expect("pinned local recovery body")
            .encode_wire()
            .expect("pinned body wire"),
        before
    );
    // Remove only this fixture owner to demonstrate that remote evidence and all
    // other eviction requirements were already sufficient; the pin caused the refusal.
    kura.native_amx_publication_capacity_reservations
        .lock()
        .remove(&carrier);
    assert!(
        kura.evict_block_bodies(required)
            .expect("eviction after pin release")
            > 0
    );
    assert!(
        kura.block_store
            .lock()
            .read_block_index(1)
            .expect("evicted index")
            .is_evicted()
    );
}

#[test]
fn native_amx_precommit_scope_failure_preserves_before_marker_until_restart() {
    let NativeAmxPublicationCapacityFixture {
        _temp_dir,
        kura,
        block,
        manifest: _,
        finality: _,
        lane_config,
    } = native_amx_publication_capacity_fixture();
    let carrier = Kura::native_amx_publication_carrier(&block).expect("precommit exact carrier");
    let failed: Result<()> = (|| {
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        let mut owner = kura
            .begin_native_amx_store_capacity_under_prune_and_canonical_guards(&block, None, None)?
            .expect("Native precommit capacity owner");
        kura.check_storage_budget(&block, None)?;
        owner.publish_pending_index()?;
        // A fallible lane/association stage exits this very same production
        // guard scope before durable_write_started; no wall-clock assumption.
        Err(Error::PruneIntentConflict(
            "injected post-index pre-canonical staging error".to_owned(),
        ))
    })();
    assert!(failed.is_err());
    assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
    assert!(
        Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .expect("retained exact locator")
            .records
            .contains_key(&carrier)
    );
    let ordinary = DummyBlocks::new().next();
    assert!(matches!(
        kura.store_block(Arc::clone(&ordinary)),
        Err(Error::CanonicalStoragePoisoned)
    ));
    assert_eq!(
        kura.block_store
            .lock()
            .read_exact_durable_index_count()
            .expect("unchanged before-marker height"),
        0
    );
    drop(kura);
    let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("cold exact precommit retirement");
    assert!(
        Kura::read_native_amx_publication_index_for_store(&reopened.store_root)
            .expect("closed precommit locator")
            .records
            .is_empty()
    );
    assert!(
        reopened
            .native_amx_publication_capacity_reservations
            .lock()
            .is_empty()
    );
    reopened
        .store_block(ordinary)
        .expect("ordinary append only after proven durable precommit retirement");
}

#[test]
fn native_amx_canonical_prune_retires_only_exact_above_target_pending_carriers() {
    for target in [0_u64, 1] {
        let fixture = native_amx_publication_capacity_fixture();
        let kura = &fixture.kura;
        kura.store_block(Arc::clone(&fixture.block))
            .expect("unfinalized Native canonical carrier");
        let carrier = Kura::native_amx_publication_carrier(&fixture.block)
            .expect("exact pending prune carrier");
        let mut ordinary: SignedBlock =
            BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
                .chain(0, Some(fixture.block.as_ref()))
                .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
                .unpack(|_| {})
                .into();
        attach_ok_results_to_block(&mut ordinary);
        kura.store_block(ordinary)
            .expect("ordinary unfinalized successor");
        let before = Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .expect("pre-prune pending inventory");
        assert!(before.records.contains_key(&carrier));
        kura.prune_to_height(target)
            .expect("exact canonical Native retirement transaction");
        let after = Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .expect("post-prune pending inventory");
        assert_eq!(after.records.contains_key(&carrier), target == 1);
        assert_eq!(
            kura.native_amx_publication_capacity_reservations
                .lock()
                .contains_key(&carrier),
            target == 1
        );
        assert_eq!(
            kura.exact_durable_blocks_count()
                .expect("pruned canonical marker"),
            usize::try_from(target).expect("target fits usize")
        );
        if target == 1 {
            assert_eq!(
                after.records.get(&carrier),
                before.records.get(&carrier),
                "retained exact record is unchanged"
            );
        }
    }
}

#[test]
fn native_amx_pending_retirement_recovers_only_from_exact_canonical_prune_intent() {
    if run_prune_crash_test_in_subprocess() {
        return;
    }
    let NativeAmxPublicationCapacityFixture {
        _temp_dir,
        kura,
        block,
        manifest: _,
        finality: _,
        lane_config,
    } = native_amx_publication_capacity_fixture();
    kura.store_block(Arc::clone(&block))
        .expect("unfinalized indexed Native carrier");
    let carrier =
        Kura::native_amx_publication_carrier(&block).expect("exact interrupted-prune carrier");
    kura.fail_prune_after_stage
        .store(PRUNE_STAGE_MEMORY, Ordering::Relaxed);
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| kura.prune_to_height(0)));
    assert!(
        failed.is_err(),
        "prune fails after canonical suffix removal but before exact index retirement"
    );
    let pending = Kura::read_native_amx_publication_index_for_store(&kura.store_root)
        .expect("pending Native record survives interrupted prune");
    let record = pending
        .records
        .get(&carrier)
        .expect("exact pre-prune owner retained");
    let intent = Kura::canonical_prune_intent_artifact_inventory(&kura.store_root)
        .expect("retained canonical prune authority")
        .stable
        .expect("durable prune intent")
        .intent;
    assert!(
        intent
            .native_amx_retirement_record_hashes
            .binary_search(&Hash::new(record.encoded().expect("bound record")))
            .is_ok()
    );
    assert!(
        kura.native_amx_publication_capacity_reservations
            .lock()
            .contains_key(&carrier),
        "failure never releases the live owner"
    );
    drop(kura);
    let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("exact pending Native retirement forward recovery");
    assert_eq!(
        reopened
            .exact_durable_blocks_count()
            .expect("completed prune height"),
        0
    );
    assert!(
        Kura::read_native_amx_publication_index_for_store(&reopened.store_root)
            .expect("completed indexed retirement")
            .records
            .is_empty()
    );
    assert!(
        reopened
            .native_amx_publication_capacity_reservations
            .lock()
            .is_empty()
    );
    assert!(!Kura::prune_intent_path_for(&reopened.store_root).exists());
}

#[test]
fn native_amx_completed_publication_retry_reauthenticates_without_new_index_or_reservation() {
    let fixture = native_amx_two_route_repair_fixture();
    let first = {
        let _publication = fixture.kura.prune_lock.lock();
        fixture
            .kura
            .persist_native_amx_participant_application_evidence_under_publication_guard(
                &fixture.block,
                &fixture.plan,
                NativeAmxParticipantApplicationPublicationMode::PostWsvRepair,
            )
            .expect("complete real two-route Native publication")
    };
    assert!(
        fixture
            .kura
            .native_amx_publication_capacity_reservations
            .lock()
            .is_empty()
    );
    assert!(
        Kura::read_native_amx_publication_index_for_store(&fixture.kura.store_root)
            .expect("retired completed index")
            .records
            .is_empty()
    );
    let before = snapshot_regular_files_recursively(&fixture.kura.store_root);
    fixture
        .kura
        .store_block(Arc::clone(&fixture.block))
        .expect("completed exact canonical store retry");
    let repeated = {
        let _publication = fixture.kura.prune_lock.lock();
        fixture
            .kura
            .persist_native_amx_participant_application_evidence_under_publication_guard(
                &fixture.block,
                &fixture.plan,
                NativeAmxParticipantApplicationPublicationMode::PostWsvRepair,
            )
            .expect("completed exact publication retry")
    };
    let NativeAmxParticipantApplicationPrepublicationToken {
        original_kura: first_kura,
        application_block_height: first_height,
        application_block_hash: first_hash,
        executed_block_wire_hash: first_wire_hash,
        finality_artifact_hash: first_finality_hash,
        manifest_root: first_manifest_root,
        manifest_leaf_count: first_leaf_count,
        identities: first_identities,
    } = first;
    let NativeAmxParticipantApplicationPrepublicationToken {
        original_kura: repeated_kura,
        application_block_height: repeated_height,
        application_block_hash: repeated_hash,
        executed_block_wire_hash: repeated_wire_hash,
        finality_artifact_hash: repeated_finality_hash,
        manifest_root: repeated_manifest_root,
        manifest_leaf_count: repeated_leaf_count,
        identities: repeated_identities,
    } = repeated;
    assert!(repeated_kura.same_instance(&first_kura));
    assert_eq!(repeated_height, first_height);
    assert_eq!(repeated_hash, first_hash);
    assert_eq!(repeated_wire_hash, first_wire_hash);
    assert_eq!(repeated_finality_hash, first_finality_hash);
    assert_eq!(repeated_manifest_root, first_manifest_root);
    assert_eq!(repeated_leaf_count, first_leaf_count);
    assert_eq!(repeated_identities, first_identities);
    assert!(
        fixture
            .kura
            .native_amx_publication_capacity_reservations
            .lock()
            .is_empty()
    );
    assert_eq!(
        snapshot_regular_files_recursively(&fixture.kura.store_root),
        before
    );
}

#[test]
fn native_amx_uncommitted_association_cleanup_failure_blocks_mutation_until_cold_recovery() {
    for replacing in [false, true] {
        let NativeAmxPublicationCapacityFixture {
            _temp_dir,
            kura,
            block,
            manifest: _,
            finality: _,
            lane_config,
        } = native_amx_publication_capacity_fixture();
        let mut ordinary = DummyBlocks::new();
        let old = replacing.then(|| ordinary.next());
        if let Some(old) = &old {
            kura.store_block(Arc::clone(old))
                .expect("ordinary selected tip before Native replacement");
        }
        let before_count = u64::from(replacing);
        let carrier =
            Kura::native_amx_publication_carrier(&block).expect("incoming Native carrier");
        kura.fail_next_block_write.store(true, Ordering::Relaxed);
        kura.fail_next_canonical_association_cleanup
            .store(true, Ordering::Relaxed);
        let error = if replacing {
            kura.replace_top_block(Arc::clone(&block))
        } else {
            kura.store_block(Arc::clone(&block))
        }
        .expect_err("actual uncommitted write followed by association cleanup failure");
        assert!(matches!(
            error,
            Error::IO(ref source, ref path)
                if source.to_string() == "injected canonical association cleanup failure"
                    && *path == kura.canonical_association_stage_path()
        ));
        assert!(!kura.fail_next_block_write.load(Ordering::Relaxed));
        assert!(
            !kura
                .fail_next_canonical_association_cleanup
                .load(Ordering::Relaxed),
            "the actual association removal boundary must have executed"
        );
        assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
        let inventory = Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .expect("failed cleanup retains its exact discovery record");
        let record = inventory
            .records
            .get(&carrier)
            .expect("pending Native record");
        assert_eq!(
            record.origin,
            NativeAmxPublicationIndexOriginV1::CanonicalWrite
        );
        assert_eq!(record.selection_marker.count, before_count);
        assert_eq!(
            record.selection_marker.tip_hash,
            old.as_ref().map(|old| old.hash())
        );
        assert!(
            kura.native_amx_publication_capacity_reservations
                .lock()
                .contains_key(&carrier),
            "failed cleanup retains the complete capacity owner"
        );
        let next = ordinary.next();
        assert!(matches!(
            kura.store_block(Arc::clone(&next)),
            Err(Error::CanonicalStoragePoisoned)
        ));
        assert_eq!(
            kura.block_store
                .lock()
                .read_exact_durable_index_count()
                .expect("failure preserves the selected marker"),
            before_count
        );
        assert!(kura.canonical_association_stage_path().is_file());
        drop(kura);
        let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
        let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("cold recovery resolves the selected old marker and exact pending index");
        assert!(
            Kura::read_native_amx_publication_index_for_store(&reopened.store_root)
                .expect("closed pending-index inventory")
                .records
                .is_empty()
        );
        assert!(
            reopened
                .native_amx_publication_capacity_reservations
                .lock()
                .is_empty()
        );
        assert!(!reopened.canonical_association_stage_path().exists());
        if let Some(old) = &old {
            assert_eq!(
                reopened
                    .get_block_without_merge_sidecar(NonZeroUsize::new(1).unwrap())
                    .expect("selected old replacement body is preserved")
                    .encode_wire()
                    .unwrap(),
                old.encode_wire().unwrap()
            );
        }
        reopened
            .store_block(next)
            .expect("new mutation only after exact cold rollback");
    }
}

#[test]
fn native_amx_proven_uncommitted_cleanup_releases_owner_without_poison() {
    for replacing in [false, true] {
        let fixture = native_amx_publication_capacity_fixture();
        let kura = &fixture.kura;
        let mut ordinary = DummyBlocks::new();
        if replacing {
            kura.store_block(ordinary.next())
                .expect("ordinary selected tip");
        }
        let carrier =
            Kura::native_amx_publication_carrier(&fixture.block).expect("incoming Native carrier");
        kura.fail_next_block_write.store(true, Ordering::Relaxed);
        let error = if replacing {
            kura.replace_top_block(Arc::clone(&fixture.block))
        } else {
            kura.store_block(Arc::clone(&fixture.block))
        }
        .expect_err("known uncommitted block-write failure");
        assert!(matches!(
            error,
            Error::IO(ref source, _) if source.to_string() == "kura store_block injected failure"
        ));
        assert!(!kura.fail_next_block_write.load(Ordering::Relaxed));
        assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
        assert!(!kura.canonical_association_stage_path().exists());
        assert!(
            Kura::read_native_amx_publication_index_for_store(&kura.store_root)
                .expect("proven rollback inventory")
                .records
                .is_empty()
        );
        assert!(
            !kura
                .native_amx_publication_capacity_reservations
                .lock()
                .contains_key(&carrier)
        );
        kura.store_block(ordinary.next())
            .expect("successful exact rollback permits the next ordinary mutation");
    }
}

#[test]
fn native_amx_startup_completion_allows_owned_sibling_change_but_rejects_protected_file_replacement()
 {
    for replaced_kind in [
        None,
        Some(NativeAmxEvidenceKind::Manifest),
        Some(NativeAmxEvidenceKind::Receipt),
    ] {
        let temp_dir = TempDir::new().expect("Native startup pair identity directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        let entry = kura
            .lane_storage_entry(LaneId::SINGLE)
            .expect("active primary route");
        let receipt = install_native_amx_latest_index_evidence_fixture(&kura, &entry);
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        let _geometry = kura.lane_geometry_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        let namespace = kura
            .native_amx_evidence_namespace_for_entry(&entry)
            .expect("exact Native namespace");
        let admitted = kura
            .inventory_native_amx_evidence_files_locked(&namespace, false)
            .expect("protected pair before owned sibling publication");
        let height = receipt.participant_proposal.descriptor.lane_block_height;
        let manifest_file = admitted.manifests.get(&height).expect("admitted manifest");
        let manifest = kura
            .decode_native_amx_manifest_file_locked(&entry, &namespace, manifest_file)
            .expect("authenticated manifest payload");
        let latest_path = Kura::native_amx_participant_receipt_latest_index_path_for_entry(
            &entry,
            &kura.store_root,
        );
        fs::write(
            &latest_path,
            norito::encode_canonical(&NativeAmxParticipantReceiptLatestIndexV2::from_receipt(
                &receipt,
            ))
            .expect("exact derived sibling bytes"),
        )
        .expect("owned latest-pointer sibling publication");
        if let Some(kind) = replaced_kind {
            let old = match kind {
                NativeAmxEvidenceKind::Manifest => admitted.manifests.get(&height).unwrap(),
                NativeAmxEvidenceKind::Receipt => admitted.receipts.get(&height).unwrap(),
            };
            let bytes = fs::read(&old.path).expect("same protected payload bytes");
            let replacement = tempfile::NamedTempFile::new_in(old.path.parent().unwrap())
                .expect("distinct replacement inode while original still exists");
            fs::write(replacement.path(), &bytes).expect("stage identical protected bytes");
            replacement
                .persist(&old.path)
                .expect("replace exact path with another inode");
            assert_eq!(fs::read(&old.path).unwrap(), bytes);
            let current = Kura::regular_sidecar_metadata_for(
                &kura.store_root,
                &old.path,
                old.path.parent().unwrap(),
            )
            .expect("replacement metadata")
            .expect("replacement exists");
            assert!(!Kura::sidecar_metadata_same_object(
                &old.metadata.file,
                &current.file
            ));
        }
        let result = kura.validate_native_amx_startup_completed_pair_locked(
            &entry, &namespace, &admitted, &manifest, &receipt,
        );
        if replaced_kind.is_some() {
            let error = result
                .expect_err("identical bytes cannot authorize replacement of a protected file");
            assert!(
                error
                    .to_string()
                    .contains("changed an admitted protected file"),
                "{error}"
            );
        } else {
            result.expect("owned sibling changes preserve both exact protected files");
        }
    }
}
