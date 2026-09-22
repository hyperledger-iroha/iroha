// Two or three separate Native publication routes, plus their coordinator. No merge
// owner or fabricated capacity reservation is installed by this fixture.
struct NativeAmxPublicationCapacityFixture {
    _temp_dir: TempDir,
    kura: Arc<Kura>,
    block: Arc<SignedBlock>,
    manifest: crate::sumeragi::exec::NativeAmxApplicationManifestV1,
    finality: V2FinalityArtifact,
    lane_config: RuntimeLaneConfig,
}

fn native_amx_capacity_fixture_qc(
    body: iroha_data_model::block::consensus::NativeAmxAttestationBodyV2,
    key: &KeyPair,
) -> iroha_data_model::block::consensus::NativeAmxAttestationQcV2 {
    use iroha_data_model::block::consensus::NativeAmxAttestationQcV2;
    let signature = iroha_crypto::Signature::try_new(key.private_key(), &body.signature_preimage())
        .expect("sign exact first-height Native capacity attestation");
    let aggregate = iroha_crypto::bls_normal_aggregate_signatures(&[signature.payload()])
        .expect("aggregate Native capacity fixture attestation");
    let validators = vec![PeerId::new(key.public_key().clone())];
    let qc = NativeAmxAttestationQcV2::try_new(
        body,
        iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        HashOf::new(&validators),
        validators,
        vec![iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("Native capacity PoP")],
        vec![1],
        aggregate,
    )
    .expect("valid exact Native capacity QC");
    let pops = BTreeMap::from([(
        key.public_key().clone(),
        iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("frozen fixture PoP"),
    )]);
    crate::native_amx::validate_native_amx_qc(
        &qc,
        &qc.body,
        &[PeerId::new(key.public_key().clone())],
        1,
        &pops,
    )
    .expect("re-signed Native fixture QC passes actual cryptographic validation");
    qc
}

fn native_amx_publication_capacity_fixture() -> NativeAmxPublicationCapacityFixture {
    native_amx_publication_capacity_fixture_with_route_count(3)
}

#[test]
fn native_amx_startup_reserves_journal_routes_without_publishing_live_geometry() {
    let NativeAmxPublicationCapacityFixture {
        _temp_dir,
        kura,
        block,
        manifest: _,
        finality: _,
        lane_config,
    } = native_amx_publication_capacity_fixture();
    kura.store_block(Arc::clone(&block))
        .expect("store real unpublished Native carrier");
    let expected = kura
        .native_amx_publication_capacity_reserved_bytes()
        .expect("complete three-route reservation");
    assert!(expected > 0);
    let carrier = Kura::native_amx_publication_carrier(&block).expect("exact carrier");
    let before = snapshot_regular_files_recursively(&kura.store_root);
    let store_root = kura.store_root.clone();
    drop(kura);
    let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("read-only journal route reservation before State geometry");
    assert!(
        reopened.lane_storage_entries.lock().is_empty(),
        "reservation reads must not publish any active lane geometry"
    );
    assert_eq!(
        reopened
            .native_amx_publication_capacity_reserved_bytes()
            .expect("reconstructed complete allocation"),
        expected
    );
    {
        let owners = reopened.native_amx_publication_capacity_reservations.lock();
        assert_eq!(owners.len(), 1);
        assert_eq!(
            owners
                .get(&carrier)
                .expect("retained exact carrier owner")
                .routes
                .len(),
            3
        );
    }
    assert_eq!(
        snapshot_regular_files_recursively(&store_root),
        before,
        "reservation reconstruction must not provision or rewrite lane storage"
    );
    assert_eq!(
        reopened.get_durable_block_hash(nonzero!(1_usize)),
        Some(block.hash())
    );
}

#[test]
fn native_amx_startup_rejects_changed_or_moving_physical_binding_without_growth() {
    for moving in [false, true] {
        let NativeAmxPublicationCapacityFixture {
            _temp_dir,
            kura,
            block,
            manifest,
            finality: _,
            lane_config,
        } = native_amx_publication_capacity_fixture();
        kura.store_block(Arc::clone(&block))
            .expect("store exact Native carrier");
        let leaf = &manifest
            .entries()
            .first()
            .expect("three Native routes")
            .leaf;
        let entry = kura
            .lane_storage_entry(leaf.lane_id)
            .expect("fixture Native route");
        if moving {
            kura.seal_native_amx_reservation_pair_move_for_test(&entry)
                .expect("stage exact paired move seal");
        } else {
            kura.substitute_lane_marker_identity_for_test(
                &entry,
                Hash::new(b"wrong Native reservation incarnation"),
                0,
            )
            .expect("substitute a different physical incarnation");
        }
        let store_root = kura.store_root.clone();
        let before = snapshot_regular_files_recursively(&store_root);
        drop(kura);
        let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
        assert!(
            Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).is_err(),
            "startup must reject a changed or moving journal-selected route"
        );
        assert_eq!(
            snapshot_regular_files_recursively(&store_root),
            before,
            "unproven physical geometry must not permit recovery growth or evidence deletion"
        );
    }
}

#[allow(clippy::too_many_lines)]
fn native_amx_publication_capacity_fixture_with_route_count(
    route_count: usize,
) -> NativeAmxPublicationCapacityFixture {
    assert!(
        matches!(route_count, 2 | 3),
        "fixture supports two or three publication routes"
    );
    use crate::queue::{RouteLeg, RouteLegRole, RoutingDecision, RoutingPlan};
    use iroha_data_model::block::consensus::{NativeAmxParticipantSettlement, NativeAmxPhase};

    let original = crate::sumeragi::exec::result_bearing_native_manifest_block_for_tests();
    let signer = KeyPair::try_from_seed(vec![0x41; 32], Algorithm::BlsNormal)
        .expect("the real exec fixture's deterministic Native attestation signer");
    let mut contexts = original
        .execution_context()
        .expect("Native context")
        .external
        .clone();
    let mut configured_routes = BTreeMap::new();
    for context in &mut contexts {
        let receipt = context
            .native_amx_receipt
            .as_mut()
            .expect("Native source receipt");
        // The source helper has two separate routes and a coordinator. Add
        // the optional third route before rebuilding all hashes and signatures.
        if route_count == 3 {
            let mut third = receipt.legs[0].clone();
            third.lane_id = LaneId::new(4);
            third.dataspace_id = DataSpaceId::new(10);
            third.participant_proposal.descriptor.lane_id = third.lane_id;
            third.participant_proposal.descriptor.dataspace_id = third.dataspace_id;
            third.participant_proposal.descriptor.lane_incarnation =
                Hash::new(b"third separate Native capacity participant");
            receipt.legs.push(third);
        }
        receipt
            .legs
            .sort_by_key(|leg| (leg.dataspace_id, leg.lane_id));
        receipt.authority_context_height = 1;
        receipt.lane_block_height = 1;
        for leg in &mut receipt.legs {
            let proposal = &mut leg.participant_proposal;
            let descriptor = &mut proposal.descriptor;
            descriptor.proposal_height = 1;
            descriptor.previous_lane_block_height = 0;
            descriptor.previous_lane_block_descriptor_hash = None;
            descriptor.lane_block_height = 1;
            descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
            proposal.proposal_hash = proposal.computed_proposal_hash();
            crate::lane_consensus::validate_lane_block_proposal(proposal)
                .expect("canonical first-height capacity proposal");
            leg.participant_settlement = NativeAmxParticipantSettlement::try_new(
                leg.lane_id,
                leg.dataspace_id,
                proposal.descriptor.lane_incarnation,
                1,
                1,
                None,
                leg.participant_settlement.source_ids().to_vec(),
            )
            .expect("canonical first-height capacity settlement");
            leg.participant_settlement_hash = leg
                .participant_settlement
                .computed_hash()
                .expect("hash first-height capacity settlement");
            let old = configured_routes.insert(
                leg.lane_id,
                (leg.dataspace_id, proposal.descriptor.lane_incarnation),
            );
            assert!(old.is_none_or(
                |route| route == (leg.dataspace_id, proposal.descriptor.lane_incarnation,)
            ));
        }
        let coordinator = receipt
            .legs
            .iter()
            .find(|leg| leg.lane_id == receipt.lane_id && leg.dataspace_id == receipt.dataspace_id)
            .expect("separate coordinator remains in the receipt")
            .participant_proposal
            .clone();
        receipt.coordinator_proposal_hash = coordinator.proposal_hash;
        let routing_plan = RoutingPlan::native_amx(
            RoutingDecision::new(receipt.lane_id, receipt.dataspace_id),
            receipt
                .legs
                .iter()
                .map(|leg| {
                    RouteLeg::new(
                        RoutingDecision::new(leg.lane_id, leg.dataspace_id),
                        RouteLegRole::Participant,
                    )
                })
                .collect(),
        );
        receipt.plan_digest = routing_plan.digest();
        context.routing_plan_digest = receipt.plan_digest;
        context.routing_plan_legs =
            crate::queue::execution_context_legs_for_routing_plan(&routing_plan);
        for leg in &mut receipt.legs {
            let descriptor = &leg.participant_proposal.descriptor;
            let mut prepare = leg.prepare_qc.body;
            prepare.round.height = 1;
            prepare.plan_digest = receipt.plan_digest;
            prepare.authority_context_height = 1;
            prepare.planned_coordinator_block_height = 1;
            prepare.coordinator_proposal_hash = coordinator.proposal_hash;
            prepare.participant_lane_id = descriptor.lane_id;
            prepare.participant_dataspace_id = descriptor.dataspace_id;
            prepare.participant_lane_incarnation = descriptor.lane_incarnation;
            prepare.participant_previous_block_height = 0;
            prepare.participant_previous_block_descriptor_hash = None;
            prepare.participant_lane_block_height = 1;
            prepare.participant_proposal_hash = leg.participant_proposal.proposal_hash;
            prepare.participant_settlement_commitment = Hash::from(leg.participant_settlement_hash);
            prepare.phase = NativeAmxPhase::Prepare;
            let mut commit = prepare;
            commit.phase = NativeAmxPhase::Commit;
            leg.prepare_qc = native_amx_capacity_fixture_qc(prepare, &signer);
            leg.commit_qc = native_amx_capacity_fixture_qc(commit, &signer);
        }
        assert!(
            crate::native_amx::receipt_shape_matches_coordinator_payload(
                Some(receipt),
                &routing_plan,
                &receipt.source_id,
                Hash::from(context.entrypoint_hash),
                receipt.network_id,
                &coordinator,
            )
        );
    }
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("first height"),
        None,
        iroha_crypto::MerkleTree::root_from_typed_leaves(original.network_input_hashes()),
        40,
        6,
    );
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(signer.private_key(), header.hash())
            .expect("sign first-height Native carrier"),
    );
    let mut block = SignedBlock::presigned(
        signature,
        header,
        original.external_transactions().cloned().collect(),
    );
    block.set_execution_context(Some(BlockExecutionContextBundle::new(contexts)));
    attach_ok_results_to_block(&mut block);
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(signer.private_key(), block.header().hash())
            .expect("sign complete first-height Native carrier"),
    );
    block
        .replace_signatures([signature].into_iter().collect())
        .expect("bind the completed carrier header");
    let manifest =
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(&block)
            .expect("derive exact ordinary Native publication manifest");
    assert_eq!(
        usize::try_from(manifest.count()).expect("fixture route count fits usize"),
        route_count,
        "coordinator is excluded; every distinct publication envelope remains"
    );
    assert!(
        manifest
            .entries()
            .iter()
            .all(|entry| entry.leaf.participant_height == 1
                && entry.leaf.predecessor_height == 0
                && entry.leaf.predecessor_descriptor_hash.is_none())
    );
    let mut execution = v2_finality_fixture_execution_commitment();
    execution.native_amx_application_manifest_root = manifest.root();
    execution.native_amx_application_manifest_count = manifest.count();
    let finality = v2_finality_artifact_for_block_with_execution(&block, execution);
    let lanes = std::iter::once(ModelLaneConfig::default())
        .chain(std::iter::once(ModelLaneConfig {
            id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(1),
            alias: "non-native-capacity-competitor".to_owned(),
            ..ModelLaneConfig::default()
        }))
        .chain(
            configured_routes
                .iter()
                .map(|(lane, (dataspace, _))| ModelLaneConfig {
                    id: *lane,
                    dataspace_id: *dataspace,
                    alias: format!("native-capacity-{}", lane.as_u32()),
                    ..ModelLaneConfig::default()
                }),
        )
        .collect();
    let catalog = LaneCatalog::new(NonZeroU32::new(10).expect("lane bound"), lanes)
        .expect("participants plus coordinator and separate competitor catalog");
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let temp_dir = TempDir::new().expect("Native publication capacity directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog)
        .expect("persistent Kura with authentic configured catalog");
    assert_eq!(
        kura.lane_storage_entries.lock().len(),
        0,
        "canonical-only constructor must leave all State geometry unpublished"
    );
    let mut requested_incarnations = configured_routes
        .iter()
        .map(|(lane, (_, incarnation))| (*lane, *incarnation))
        .collect::<BTreeMap<_, _>>();
    requested_incarnations.insert(
        LaneId::new(1),
        Hash::new(b"kura-autonomous-view-incarnation"),
    );
    // The real configured-primary anchor and journaled transition own every
    // secondary directory and marker. Restart must not depend on a test-only
    // live map or an incarnation marker without its durable geometry binding.
    publish_initial_configured_lane_geometry_for_test(&kura, &lane_config, &requested_incarnations);
    let (baseline, phases, has_temporary) = kura
        .lane_geometry_journal_state_for_test()
        .expect("authenticate the fixture's published geometry journal");
    assert_eq!(
        baseline,
        Some(LaneLifecycleParameterV1::catalog_hash(&catalog))
    );
    assert_eq!(phases, vec!["catalog_published"]);
    assert!(!has_temporary, "geometry publication must be complete");
    kura.assert_native_amx_fixture_geometry_for_test(&lane_config, &requested_incarnations);
    Arc::get_mut(&mut kura)
        .expect("exclusive fresh capacity Kura")
        .max_disk_usage_bytes = u64::MAX / 4;
    NativeAmxPublicationCapacityFixture {
        _temp_dir: temp_dir,
        kura,
        block: Arc::new(block),
        manifest,
        finality,
        lane_config,
    }
}

// Physical usage and every pre-existing reservation family, excluding only
// this test's projected new operation. No reservation map is changed here.
fn native_amx_capacity_existing_peak(kura: &Kura) -> u64 {
    let used = kura
        .refresh_disk_usage_bytes()
        .expect("measure canonical physical usage");
    let (persisted, unindexed) = kura
        .persisted_count_and_unindexed_bytes()
        .expect("durable budget frontier");
    [
        kura.pending_block_bytes(persisted, unindexed)
            .expect("pending canonical bytes"),
        kura.lane_publication_budget_reserved_bytes()
            .expect("all lane publication owners"),
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("certified bundle owners"),
        kura.autonomous_global_terminal_outcome_reserved_bytes()
            .expect("terminal owners"),
        Kura::canonical_prune_intent_maintenance_headroom_bytes(),
    ]
    .into_iter()
    .try_fold(used, u64::checked_add)
    .expect("exact existing peak fits u64")
}

fn native_amx_capacity_block_growth(kura: &Kura, block: &SignedBlock) -> u64 {
    kura.block_required_bytes_for_budget(block, None, u64::MAX)
        .expect("canonical block and immutable lane geometry")
        .checked_add(
            kura.canonical_association_stage_additional_bytes(block, None)
                .expect("exact canonical crash stage"),
        )
        .expect("block and stage growth fits u64")
}

// Initial histories contain nothing to prune. Independently frame all three
// payloads per route so an inflated shared planner estimate cannot pass.
fn native_amx_initial_publication_bytes(
    manifest: &crate::sumeragi::exec::NativeAmxApplicationManifestV1,
) -> u64 {
    let artifacts = native_amx_participant_application_artifacts(
        manifest,
        native_amx_participant_application_finality_placeholder_hash(),
    )
    .expect("derive exact first-height Native artifacts");
    assert_eq!(artifacts.len(), 3);
    artifacts
        .iter()
        .flat_map(|(manifest, receipt)| {
            let latest = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(receipt);
            [
                manifest
                    .encode_framed()
                    .expect("frame first Native manifest")
                    .len(),
                receipt
                    .encode_framed()
                    .expect("frame first Native receipt")
                    .len(),
                norito::encode_canonical(&latest)
                    .expect("frame first Native latest pointer")
                    .len(),
            ]
        })
        .map(|bytes| u64::try_from(bytes).expect("encoded payload fits u64"))
        .try_fold(0_u64, u64::checked_add)
        .expect("exact three-route payload sum")
}

#[test]
fn native_amx_store_retains_three_publication_owners_until_prepublication() {
    let fixture = native_amx_publication_capacity_fixture();
    let kura = &fixture.kura;
    let carrier =
        Kura::native_amx_publication_carrier(&fixture.block).expect("exact carrier identity");
    assert!(
        kura.native_amx_publication_capacity_reservations
            .lock()
            .is_empty()
    );
    kura.store_block(Arc::clone(&fixture.block))
        .expect("ordinary Native store admission");
    let reserved = kura
        .native_amx_publication_capacity_reserved_bytes()
        .expect("Native envelope");
    assert!(reserved > 0);
    assert_eq!(
        reserved,
        native_amx_initial_publication_bytes(&fixture.manifest)
    );
    assert_eq!(
        kura.lane_publication_budget_reserved_bytes()
            .expect("combined lane envelope"),
        reserved
    );
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("merge-only bytes"),
        0
    );
    assert!(
        kura.associated_merge_entry_for_block(&fixture.block)
            .expect("ordinary association")
            .is_none()
    );
    {
        let owners = kura.native_amx_publication_capacity_reservations.lock();
        assert_eq!(owners.len(), 1);
        let owner = owners
            .get(&carrier)
            .expect("store owns this exact wire carrier");
        assert_eq!(owner.routes.len(), 3);
        assert!(
            owner
                .routes
                .values()
                .all(|route| route.outstanding_components.len() == 3
                    && route.prune_journal_bytes == 0
                    && !route.cleanup_complete)
        );
    }
    for entry in fixture.manifest.entries() {
        let lane = kura
            .lane_storage_entry(entry.leaf.lane_id)
            .expect("Native lane");
        assert!(
            !Kura::native_amx_application_manifest_path_for_entry(&lane, &kura.store_root, 1)
                .exists()
        );
        assert!(
            !Kura::native_amx_participant_receipt_path_for_entry(&lane, &kura.store_root, 1)
                .exists()
        );
    }
    let _ = kura
        .store_v2_finality_artifact(&fixture.finality)
        .expect("durable exact Native finality");
    assert_eq!(
        kura.native_amx_publication_capacity_reserved_bytes()
            .expect("store-to-publication gap"),
        reserved
    );
    kura.prepublish_native_amx_participant_application_evidence(&fixture.block, None)
        .expect("all three real Native routes publish before WSV");
    assert!(
        kura.wsv_checkpoint(1)
            .expect("pre-WSV checkpoint lookup")
            .is_none()
    );
    assert!(
        kura.commit_manifest(1)
            .expect("pre-WSV manifest lookup")
            .is_none()
    );
    let owners = kura.native_amx_publication_capacity_reservations.lock();
    let owner = owners
        .get(&carrier)
        .expect("publication retains cleanup identity until WSV");
    assert_eq!(owner.routes.len(), 3);
    assert!(
        owner
            .routes
            .values()
            .all(|route| route.outstanding_components.is_empty() && !route.cleanup_complete)
    );
    // Published stable files now count as physical usage. An empty first-route
    // prune plan must not continue reserving those same payload bytes.
    assert_eq!(owner.reserved_bytes(), Some(0));
}

#[test]
fn native_amx_exact_store_and_prepublication_retry_do_not_duplicate_capacity() {
    let fixture = native_amx_publication_capacity_fixture();
    let kura = &fixture.kura;
    kura.store_block(Arc::clone(&fixture.block))
        .expect("initial ordinary Native store");
    let first = kura
        .native_amx_publication_capacity_reservations
        .lock()
        .clone();
    kura.store_block(Arc::clone(&fixture.block))
        .expect("exact ordinary Native store retry");
    assert_eq!(
        *kura.native_amx_publication_capacity_reservations.lock(),
        first
    );
    assert_eq!(
        kura.exact_durable_blocks_count()
            .expect("retry durable count"),
        1
    );
    let _ = kura
        .store_v2_finality_artifact(&fixture.finality)
        .expect("Native retry finality");
    kura.prepublish_native_amx_participant_application_evidence(&fixture.block, None)
        .expect("initial Native prepublication");
    let published = kura
        .native_amx_publication_capacity_reservations
        .lock()
        .clone();
    kura.store_block(Arc::clone(&fixture.block))
        .expect("exact store retry after publication");
    kura.prepublish_native_amx_participant_application_evidence(&fixture.block, None)
        .expect("exact Native prepublication retry");
    assert_eq!(
        *kura.native_amx_publication_capacity_reservations.lock(),
        published
    );
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("ordinary Native has no merge owner"),
        0
    );
}

#[test]
fn native_amx_initial_store_rejects_one_byte_below_complete_publication_peak() {
    let mut fixture = native_amx_publication_capacity_fixture();
    let (_, projected) = {
        let _prune = fixture.kura.prune_lock.lock();
        let _canonical = fixture.kura.canonical_chain_lock.lock();
        fixture
            .kura
            .native_amx_publication_plan_under_prune_and_canonical_guards(&fixture.block, None)
            .expect("read-only prospective Native plan")
            .expect("three Native routes")
    };
    assert_eq!(projected.routes.len(), 3);
    assert!(
        projected
            .routes
            .values()
            .all(|route| route.prune_journal_bytes == 0)
    );
    let native = native_amx_initial_publication_bytes(&fixture.manifest);
    assert_eq!(projected.reserved_bytes(), Some(native));
    assert!(native > 0);
    let index_publication = {
        let _prune = fixture.kura.prune_lock.lock();
        let _canonical = fixture.kura.canonical_chain_lock.lock();
        fixture
            .kura
            .prepare_native_amx_publication_index(&fixture.block, None, None)
            .expect("prepare the exact durable carrier locator without publishing it")
    };
    assert!(
        index_publication.additional_bytes > 0,
        "the first carrier must fund its own durable locator"
    );
    let exact = native_amx_capacity_existing_peak(&fixture.kura)
        .checked_add(native_amx_capacity_block_growth(
            &fixture.kura,
            &fixture.block,
        ))
        .and_then(|total| total.checked_add(native))
        .and_then(|total| total.checked_add(index_publication.additional_bytes))
        .expect("full Native admission peak including the exact carrier locator");
    let used = fixture
        .kura
        .kura_disk_usage_bytes()
        .expect("baseline physical usage");
    Arc::get_mut(&mut fixture.kura)
        .expect("exclusive Native admission fixture")
        .max_disk_usage_bytes = exact - 1;
    let error = fixture.kura.store_block(Arc::clone(&fixture.block))
        .expect_err("one byte below block plus crash stage plus carrier locator plus all Native routes must fail before commit");
    assert!(
        matches!(error, Error::StorageBudgetExceeded { limit, required, .. }
        if limit == exact - 1 && required == exact)
    );
    assert_eq!(
        fixture
            .kura
            .exact_durable_blocks_count()
            .expect("rejected durable count"),
        0
    );
    assert_eq!(fixture.kura.blocks_count(), 0);
    assert!(
        fixture
            .kura
            .native_amx_publication_capacity_reservations
            .lock()
            .is_empty()
    );
    assert_eq!(
        fixture
            .kura
            .kura_disk_usage_bytes()
            .expect("rejected physical usage"),
        used
    );
    Arc::get_mut(&mut fixture.kura)
        .expect("exclusive exact-bound fixture")
        .max_disk_usage_bytes = exact;
    fixture
        .kura
        .store_block(Arc::clone(&fixture.block))
        .expect("the exact initial peak must admit the same block");
    assert_eq!(
        fixture
            .kura
            .exact_durable_blocks_count()
            .expect("accepted durable count"),
        1
    );
    assert_eq!(
        fixture
            .kura
            .native_amx_publication_capacity_reserved_bytes()
            .expect("durable carrier still owns Native work"),
        native
    );
}

#[test]
fn native_amx_reserved_capacity_cannot_be_spent_by_a_competing_canonical_block() {
    let mut fixture = native_amx_publication_capacity_fixture();
    fixture
        .kura
        .store_block(Arc::clone(&fixture.block))
        .expect("reserve actual ordinary Native work");
    let owned = fixture
        .kura
        .native_amx_publication_capacity_reservations
        .lock()
        .clone();
    let native = fixture
        .kura
        .native_amx_publication_capacity_reserved_bytes()
        .expect("Native reserved bytes");
    assert!(native > 0);
    let mut next: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, Some(fixture.block.as_ref()))
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    next.set_execution_context(None);
    attach_ok_results_to_block(&mut next);
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
            next.header().hash(),
        )
        .expect("sign competing block"),
    );
    next.replace_signatures([signature].into_iter().collect())
        .expect("completed competing block signature");
    let next = Arc::new(next);
    let exact = native_amx_capacity_existing_peak(&fixture.kura)
        .checked_add(native_amx_capacity_block_growth(&fixture.kura, &next))
        .expect("competing canonical peak");
    Arc::get_mut(&mut fixture.kura)
        .expect("exclusive competing budget fixture")
        .max_disk_usage_bytes = exact - native;
    let error = fixture
        .kura
        .store_block(Arc::clone(&next))
        .expect_err("canonical competitor must not consume Native publication headroom");
    assert!(
        matches!(error, Error::StorageBudgetExceeded { limit, required, .. }
        if limit == exact - native && required == exact)
    );
    assert_eq!(
        fixture
            .kura
            .exact_durable_blocks_count()
            .expect("competitor rejected"),
        1
    );
    assert_eq!(
        *fixture
            .kura
            .native_amx_publication_capacity_reservations
            .lock(),
        owned
    );
    Arc::get_mut(&mut fixture.kura)
        .expect("exclusive restored budget fixture")
        .max_disk_usage_bytes = exact;
    fixture
        .kura
        .store_block(next)
        .expect("competitor admits with its own complete extra headroom");
    assert_eq!(
        fixture
            .kura
            .exact_durable_blocks_count()
            .expect("competitor accepted"),
        2
    );
    assert_eq!(
        *fixture
            .kura
            .native_amx_publication_capacity_reservations
            .lock(),
        owned
    );
}

#[test]
fn native_amx_reserved_capacity_cannot_be_spent_by_autonomous_claim_staging() {
    let mut fixture = native_amx_publication_capacity_fixture();
    let lane = fixture
        .lane_config
        .entry(LaneId::new(1))
        .expect("separate non-Native competitor lane");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_, _, payload) = two_reservation_autonomous_lane_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        1,
        &signer,
    );
    let entry = fixture
        .kura
        .lane_storage_entry(lane.lane_id)
        .expect("configured competitor storage");
    assert_eq!(
        entry.incarnation, payload.origin_proposal.descriptor.lane_incarnation,
        "the competitor uses its original journal-admitted identity"
    );
    fixture
        .kura
        .store_block(Arc::clone(&fixture.block))
        .expect("reserve real ordinary Native work");
    let owned = fixture
        .kura
        .native_amx_publication_capacity_reservations
        .lock()
        .clone();
    let native = fixture
        .kura
        .native_amx_publication_capacity_reserved_bytes()
        .expect("Native reserved bytes");
    assert!(native > 0);
    let claims = payload
        .entrypoint_hashes
        .iter()
        .map(|entrypoint| AutonomousLaneEntrypointClaimV1::new(&payload, *entrypoint))
        .collect::<Vec<_>>();
    let staged_bytes = claims
        .iter()
        .map(|claim| {
            u64::try_from(
                norito::encode_canonical(claim)
                    .expect("frame real autonomous claim")
                    .len(),
            )
            .expect("claim length fits u64")
        })
        .try_fold(0_u64, u64::checked_add)
        .expect("complete competing claim set");
    let paths = claims
        .iter()
        .map(|claim| {
            Kura::autonomous_lane_entrypoint_claim_path(
                &fixture.kura.store_root,
                &claim.network_id,
                &claim.entrypoint_hash,
            )
        })
        .collect::<Vec<_>>();
    let exact = native_amx_capacity_existing_peak(&fixture.kura)
        .checked_add(staged_bytes)
        .expect("complete shared claim admission peak");
    Arc::get_mut(&mut fixture.kura)
        .expect("exclusive claim budget fixture")
        .max_disk_usage_bytes = exact - native;
    let error = {
        let _prune = fixture.kura.prune_lock.lock();
        let _canonical = fixture.kura.canonical_chain_lock.lock();
        let pending = fixture
            .kura
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("pending canonical snapshot");
        let _geometry = fixture.kura.lane_geometry_lock.lock();
        let _sidecar = fixture.kura.sidecar_lock.lock();
        fixture
            .kura
            .prepare_autonomous_lane_entrypoint_claims_locked(pending, &payload)
            .expect_err("non-Native claim staging must preserve outstanding Native capacity")
    };
    assert!(
        matches!(&error, Error::IO(_, _))
            && error
                .to_string()
                .contains("globally reserved terminal or carrier capacity"),
        "claim staging must reject specifically at shared disk capacity: {error}"
    );
    for path in &paths {
        assert!(!path.exists());
        assert!(!Kura::autonomous_lane_entrypoint_claim_temp_path(path).exists());
    }
    assert_eq!(
        *fixture
            .kura
            .native_amx_publication_capacity_reservations
            .lock(),
        owned
    );
    Arc::get_mut(&mut fixture.kura)
        .expect("exclusive complete claim budget")
        .max_disk_usage_bytes = exact;
    let staged = {
        let _prune = fixture.kura.prune_lock.lock();
        let _canonical = fixture.kura.canonical_chain_lock.lock();
        let pending = fixture
            .kura
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("pending canonical snapshot");
        let _geometry = fixture.kura.lane_geometry_lock.lock();
        let _sidecar = fixture.kura.sidecar_lock.lock();
        fixture
            .kura
            .prepare_autonomous_lane_entrypoint_claims_locked(pending, &payload)
            .expect("real claim set admits when its own peak is funded")
    };
    assert_eq!(staged.len(), claims.len());
    assert_eq!(
        *fixture
            .kura
            .native_amx_publication_capacity_reservations
            .lock(),
        owned
    );
}

fn native_amx_replay_test_complete_publication(
    kura: &Kura,
    block: &SignedBlock,
    finality: &V2FinalityArtifact,
) {
    let height = block.header().height().get();
    let checkpoint = Hash::new(b"Native no-snapshot replay exact WSV checkpoint");
    kura.store_wsv_checkpoint(height, block.hash(), checkpoint)
        .expect("persist exact replay WSV checkpoint");
    kura.store_commit_manifest(
        CommitManifest::new(height, block.hash(), None, None, checkpoint, None)
            .with_authenticated_v2_commit_authority(finality),
    )
    .expect("persist exact finality-bound replay commit manifest");
    assert_eq!(
        kura.repair_native_amx_participant_application_evidence(block)
            .expect("complete actual Native publication after WSV"),
        3
    );
    assert!(
        Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .expect("read completed publication index")
            .records
            .is_empty()
    );
    assert!(
        kura.native_amx_publication_capacity_reservations
            .lock()
            .is_empty()
    );
}

#[test]
fn native_amx_completed_tip_does_not_resurrect_publication_during_no_snapshot_replay() {
    let NativeAmxPublicationCapacityFixture {
        _temp_dir,
        kura,
        block,
        manifest: _,
        finality,
        lane_config,
    } = native_amx_publication_capacity_fixture();
    let (incarnations, activations) = active_fixture_geometry_maps(&kura, &lane_config);
    kura.store_block(Arc::clone(&block))
        .expect("store real Native carrier");
    let _finality_receipt = kura
        .store_v2_finality_artifact(&finality)
        .expect("persist real finality");
    native_amx_replay_test_complete_publication(&kura, &block, &finality);
    let network_id = kura.bound_lane_storage_network().unwrap();
    drop(kura);
    let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("cold open a fully completed Native tip");
    reopened.bind_lane_storage_network(network_id).unwrap();
    reopened
        .rewind_native_amx_fixture_geometry_before_replay_for_test()
        .expect("real pre-genesis geometry restore must not infer new tip obligations");
    assert_eq!(
        reopened
            .lane_geometry_journal_state_for_test()
            .expect("rewound journal")
            .1,
        vec!["rolled_back"]
    );
    assert!(
        reopened
            .native_amx_publication_capacity_reservations
            .lock()
            .is_empty()
    );
    drop(reopened);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("cold restart while completed secondary geometry is retained for replay");
    assert!(
        reopened
            .native_amx_publication_capacity_reservations
            .lock()
            .is_empty()
    );
    reopened.bind_lane_storage_network(network_id).unwrap();
    reopened
        .recover_lane_geometry_journal(&lane_config, &incarnations, &activations)
        .expect("forward replay the exact retained configured geometry");
    reopened
        .finish_restored_lane_segments_with_geometry(&lane_config)
        .expect("revalidate completed pairs without synthetic publication owners");
    assert!(
        reopened
            .native_amx_publication_capacity_reservations
            .lock()
            .is_empty()
    );
    assert!(
        Kura::read_native_amx_publication_index_for_store(&reopened.store_root)
            .expect("completion index remains retired")
            .records
            .is_empty()
    );
    assert_eq!(
        reopened.get_durable_block_hash(nonzero!(1_usize)),
        Some(block.hash())
    );
}

#[test]
fn native_amx_indexed_publication_survives_no_snapshot_rewind_cold_restart_and_completion() {
    for prepublished in [false, true] {
        let NativeAmxPublicationCapacityFixture {
            _temp_dir,
            kura,
            block,
            manifest: _,
            finality,
            lane_config,
        } = native_amx_publication_capacity_fixture();
        let (incarnations, activations) = active_fixture_geometry_maps(&kura, &lane_config);
        kura.store_block(Arc::clone(&block))
            .expect("store indexed Native carrier");
        let _finality_receipt = kura
            .store_v2_finality_artifact(&finality)
            .expect("persist exact finality");
        if prepublished {
            kura.prepublish_native_amx_participant_application_evidence(&block, None)
                .expect("publish all routes but retain their pre-WSV owner");
        }
        let before = kura
            .native_amx_publication_capacity_reservations
            .lock()
            .clone();
        let index = Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .expect("durable index before rewind")
            .records;
        assert_eq!(index.len(), 1);
        let carrier =
            Kura::native_amx_publication_carrier(&block).expect("exact canonical carrier");
        assert_eq!(before[&carrier].routes.len(), 3);
        assert!(
            before[&carrier]
                .routes
                .values()
                .all(|route| !route.cleanup_complete)
        );
        let network_id = kura.bound_lane_storage_network().unwrap();
        drop(kura);
        let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
        let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("reconstruct all three indexed routes before State geometry");
        reopened.bind_lane_storage_network(network_id).unwrap();
        reopened
            .rewind_native_amx_fixture_geometry_before_replay_for_test()
            .expect("preserve pending publication while rewinding to the genesis cursor");
        assert_eq!(
            *reopened.native_amx_publication_capacity_reservations.lock(),
            before
        );
        assert_eq!(
            Kura::read_native_amx_publication_index_for_store(&reopened.store_root)
                .expect("unchanged pending index after rewind")
                .records,
            index
        );
        assert_eq!(reopened.lane_storage_entries.lock().len(), 1);
        let retained_before = snapshot_regular_files_recursively(&reopened.store_root);
        drop(reopened);
        let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("cold reconstruct exact obligations from journal-retained physical pairs");
        assert_eq!(
            snapshot_regular_files_recursively(&reopened.store_root),
            retained_before,
            "cold ownership reconstruction must not normalize, provision or rewrite retained pairs"
        );
        assert_eq!(
            *reopened.native_amx_publication_capacity_reservations.lock(),
            before
        );
        assert_eq!(
            reopened.get_durable_block_hash(nonzero!(1_usize)),
            Some(block.hash())
        );
        assert_eq!(
            Kura::native_amx_publication_carrier(
                &reopened
                    .get_block_without_merge_sidecar(nonzero!(1_usize))
                    .expect("pinned carrier body")
            )
            .expect("reconstructed exact carrier"),
            carrier
        );
        reopened.bind_lane_storage_network(network_id).unwrap();
        reopened
            .recover_lane_geometry_journal(&lane_config, &incarnations, &activations)
            .expect("move exact retained pairs forward into live geometry");
        reopened
            .finish_restored_lane_segments_with_geometry(&lane_config)
            .expect("refresh namespace bindings after real forward replay");
        assert_eq!(
            *reopened.native_amx_publication_capacity_reservations.lock(),
            before
        );
        assert_eq!(
            Kura::read_native_amx_publication_index_for_store(&reopened.store_root)
                .expect("pending authority survives forward replay")
                .records,
            index
        );
        native_amx_replay_test_complete_publication(&reopened, &block, &finality);
    }
}

#[test]
fn native_amx_unfinished_evidence_requires_its_exact_uncorrupted_pending_index() {
    for (corrupt, rolled_back, prepublished, ordinary_tip) in [
        (false, false, false, false),
        (true, false, false, false),
        (false, false, true, false),
        (true, false, true, false),
        (false, true, true, false),
        (true, true, true, false),
        (false, false, true, true),
        (false, true, true, true),
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
            .expect("store real indexed carrier");
        let _finality_receipt = kura
            .store_v2_finality_artifact(&finality)
            .expect("exact finality");
        if prepublished {
            kura.prepublish_native_amx_participant_application_evidence(&block, None)
                .expect("prepublish authoritative pairs, still before WSV");
        }
        if ordinary_tip {
            // Give the interior carrier its real finality-bound WSV join so a
            // missing application checkpoint cannot mask missing-index refusal.
            // Leave one receipt absent: the remaining manifest then identifies
            // unfinished publication through physical secondary discovery,
            // independently of the bounded Native-tip consistency check.
            let checkpoint = Hash::new(b"Native interior missing-index WSV checkpoint");
            kura.store_wsv_checkpoint(1, block.hash(), checkpoint)
                .expect("persist the exact interior WSV checkpoint");
            kura.store_commit_manifest(
                CommitManifest::new(1, block.hash(), None, None, checkpoint, None)
                    .with_authenticated_v2_commit_authority(&finality),
            )
            .expect("persist the exact finality-bound interior commit manifest");
            let artifacts =
                native_amx_participant_application_artifacts(&manifest, HashOf::new(&finality))
                    .expect("exact interior Native artifacts");
            {
                let _prune = kura.prune_lock.lock();
                let _canonical = kura.canonical_chain_lock.lock();
                let _sidecar = kura.sidecar_lock.lock();
                assert!(artifacts.iter().all(|(manifest, receipt)| {
                    kura.native_amx_participant_application_receipt_matches_manifest_and_available_evidence_under_prune_canonical_and_sidecar_guards(receipt, manifest)
                }), "all interior pairs must pass the complete authority predicate before receipt removal");
            }
            let participant = manifest.entries().first().expect("three real participants");
            let entry = kura
                .lane_storage_entry(participant.leaf.lane_id)
                .expect("first exact interior participant route");
            let receipt_path = Kura::native_amx_participant_receipt_path_for_entry(
                &entry,
                &kura.store_root,
                participant.leaf.participant_height,
            );
            fs::remove_file(&receipt_path)
                .expect("leave an exact indexed manifest with its receipt still missing");
            let mut ordinary: SignedBlock =
                BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
                    .chain(0, Some(block.as_ref()))
                    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
                    .unpack(|_| {})
                    .into();
            attach_ok_results_to_block(&mut ordinary);
            kura.store_block(ordinary)
                .expect("append ordinary tip above pending Native evidence");
        }
        if rolled_back {
            kura.rewind_native_amx_fixture_geometry_before_replay_for_test()
                .expect("retain all pending secondary pairs before index corruption");
        }
        let inventory = Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .expect("read exact pending index");
        let record = inventory
            .records
            .values()
            .next()
            .expect("one pending carrier");
        let path = Kura::native_amx_publication_index_directory_for(&kura.store_root)
            .join(record.file_name().expect("exact record filename"));
        if corrupt {
            fs::write(&path, [0xA5]).expect("corrupt the exact stable index");
        } else {
            fs::remove_file(&path).expect("remove the unfinished carrier's index");
        }
        let store_root = kura.store_root.clone();
        let files = snapshot_regular_files_recursively(&store_root);
        drop(kura);
        let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
        let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
            Ok(_) => panic!("cold secondary evidence cannot manufacture publication authority"),
            Err(error) => error,
        };
        if !corrupt {
            assert!(
                matches!(&error, Error::PruneIntentConflict(message) if message.contains("exact pending index")),
                "missing index case rolled_back={rolled_back}, prepublished={prepublished}, ordinary_tip={ordinary_tip}: {error}"
            );
        }
        assert_eq!(
            snapshot_regular_files_recursively(&store_root),
            files,
            "failed cold admission must retain all exact evidence and geometry bytes"
        );
    }
}

#[test]
fn native_amx_completed_pair_latest_maintenance_is_bounded_without_reopening_publication() {
    let fixture = native_amx_publication_capacity_fixture();
    let kura = &fixture.kura;
    kura.store_block(Arc::clone(&fixture.block))
        .expect("store real Native carrier");
    let _finality_receipt = kura
        .store_v2_finality_artifact(&fixture.finality)
        .expect("persist exact finality");
    native_amx_replay_test_complete_publication(kura, &fixture.block, &fixture.finality);
    // Reconstruct one derived pointer from its exact stable pair, not an absent
    // authoritative manifest or receipt. Sibling routes remain fully complete.
    let leaf = &fixture
        .manifest
        .entries()
        .last()
        .expect("last publication route")
        .leaf;
    let entry = kura
        .lane_storage_entry(leaf.lane_id)
        .expect("completed route");
    let path =
        Kura::native_amx_participant_receipt_latest_index_path_for_entry(&entry, &kura.store_root);
    fs::remove_file(&path).expect("remove only the reconstructible derived pointer");
    kura.rebuild_native_amx_publication_capacity_on_startup()
        .expect("reserve exact derived-pointer maintenance");
    {
        let owners = kura.native_amx_publication_capacity_reservations.lock();
        assert_eq!(owners.len(), 1);
        let owner = owners.values().next().expect("one maintenance owner");
        assert!(owner.index_record.is_none());
        assert_eq!(owner.routes.len(), 1);
        let capacity = owner.routes.values().next().expect("one derived target");
        assert_eq!(
            capacity.outstanding_components,
            BTreeSet::from([NativeAmxPublicationComponent::Latest])
        );
        assert!(capacity.reserved_bytes().expect("exact maintenance size") > 0);
    }
    kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
        .expect("repair the derived pointer and validate all untouched siblings");
    assert!(path.exists());
    assert!(
        kura.native_amx_publication_capacity_reservations
            .lock()
            .is_empty()
    );
    assert!(
        Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .expect("maintenance creates no new publication index")
            .records
            .is_empty()
    );
}

#[test]
fn native_amx_retained_replay_pair_corruption_is_rejected_without_recovery_growth() {
    for fault_index in 0..4 {
        let NativeAmxPublicationCapacityFixture {
            _temp_dir,
            kura,
            block,
            manifest,
            finality: _,
            lane_config,
        } = native_amx_publication_capacity_fixture();
        kura.store_block(Arc::clone(&block))
            .expect("store genuine indexed Native carrier before any publication");
        kura.rewind_native_amx_fixture_geometry_before_replay_for_test()
            .expect("retain genuine journal-owned secondary pairs for replay");
        let lane = manifest
            .entries()
            .first()
            .expect("three pending routes")
            .leaf
            .lane_id;
        let paths = kura
            .native_amx_retained_pair_fault_paths_for_test(lane)
            .expect("resolve exact retained data and both seals from geometry authority");
        if fault_index < 2 {
            let path = &paths[fault_index];
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(path)
                .expect("open exact retained block or merge bytes");
            std::io::Write::write_all(&mut file, &[0xA5]).expect("change one retained byte digest");
        } else {
            kura.corrupt_native_amx_retained_pair_seal_for_test(lane, fault_index == 2)
                .expect("corrupt exactly one original unsealed marker field");
        }
        let store_root = kura.store_root.clone();
        let corrupted = snapshot_regular_files_recursively(&store_root);
        drop(kura);
        let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
        assert!(
            Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).is_err(),
            "cold pending reservation must reject changed retained data or incomplete paired seals"
        );
        assert_eq!(
            snapshot_regular_files_recursively(&store_root),
            corrupted,
            "strict refusal must neither normalize retained authority nor grow recovery storage"
        );
    }
}

#[test]
fn native_amx_completed_repair_retains_exact_owner_across_interruption_and_reopen() {
    let NativeAmxPublicationCapacityFixture {
        _temp_dir,
        kura,
        block,
        manifest,
        finality,
        lane_config,
    } = native_amx_publication_capacity_fixture();
    let (incarnations, activations) = active_fixture_geometry_maps(&kura, &lane_config);
    kura.store_block(Arc::clone(&block))
        .expect("store actual Native carrier");
    let _finality_receipt = kura
        .store_v2_finality_artifact(&finality)
        .expect("authenticate canonical executed wire");
    native_amx_replay_test_complete_publication(&kura, &block, &finality);
    let leaf = &manifest.entries().last().expect("three exact routes").leaf;
    let entry = kura.lane_storage_entry(leaf.lane_id).expect("active route");
    let path = Kura::native_amx_application_manifest_path_for_entry(
        &entry,
        &kura.store_root,
        leaf.participant_height,
    );
    let expected = fs::read(&path).expect("completed authoritative manifest");
    fs::remove_file(&path).expect("lose one sidecar after publication completed");
    let carrier = Kura::native_amx_publication_carrier(&block).unwrap();
    {
        let _publication = kura.prune_lock.lock();
        let plan = kura
            .native_amx_participant_application_evidence_for_block_under_publication_guard(
                &block,
                true,
                NativeAmxMergeAssociation::CommittedOnly,
            )
            .expect("derive exact complete finality and WSV-backed plan");
        let targets = (0..plan.artifacts.len()).collect::<Vec<_>>();
        assert!(
            kura.ensure_native_amx_publication_capacity_under_publication_guard(
                &block, &plan, &targets,
            )
            .expect("durably admit completed repair before any sidecar write")
        );
        let held = Kura::read_native_amx_publication_index_for_store(&kura.store_root).unwrap();
        assert_eq!(
            held.records[&carrier].origin,
            NativeAmxPublicationIndexOriginV1::CompletedRepair
        );
        assert!(
            kura.ensure_native_amx_publication_capacity_under_publication_guard(
                &block, &plan, &targets,
            )
            .expect("same live repair retries retain original index")
        );
        assert_eq!(
            Kura::read_native_amx_publication_index_for_store(&kura.store_root)
                .unwrap()
                .records,
            held.records
        );
        assert!(
            !path.exists(),
            "the interrupted point precedes the artifact write"
        );
    }
    let reserved = kura
        .native_amx_publication_capacity_reserved_bytes()
        .unwrap();
    assert!(reserved > 0);
    let network_id = kura.bound_lane_storage_network().unwrap();
    drop(kura);
    let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("restart reconstructs completed repair from its exact locator");
    assert_eq!(
        reopened
            .native_amx_publication_capacity_reserved_bytes()
            .unwrap(),
        reserved
    );
    assert_eq!(
        Kura::read_native_amx_publication_index_for_store(&reopened.store_root)
            .unwrap()
            .records[&carrier]
            .origin,
        NativeAmxPublicationIndexOriginV1::CompletedRepair
    );
    reopened.bind_lane_storage_network(network_id).unwrap();
    reopened
        .recover_lane_geometry_journal(&lane_config, &incarnations, &activations)
        .expect("restore only authenticated route geometry");
    reopened
        .finish_restored_lane_segments_with_geometry(&lane_config)
        .expect("restore bounded outstanding repair custody");
    assert_eq!(
        reopened
            .repair_native_amx_participant_application_evidence(&block)
            .expect("settle all original participant routes"),
        3
    );
    assert_eq!(fs::read(&path).expect("repaired exact sidecar"), expected);
    assert!(
        Kura::read_native_amx_publication_index_for_store(&reopened.store_root)
            .unwrap()
            .records
            .is_empty()
    );
    assert_eq!(
        reopened
            .native_amx_publication_capacity_reserved_bytes()
            .unwrap(),
        0
    );
    let files = snapshot_regular_files_recursively(&reopened.store_root);
    assert_eq!(
        reopened
            .repair_native_amx_participant_application_evidence(&block)
            .expect("completed retry"),
        3
    );
    assert_eq!(
        snapshot_regular_files_recursively(&reopened.store_root),
        files,
        "completed retry does not manufacture new custody or bytes"
    );
}

#[test]
fn native_amx_completed_repair_rechecks_exact_wire_and_wsv_join_without_growth() {
    let fixture = native_amx_publication_capacity_fixture();
    let kura = &fixture.kura;
    kura.store_block(Arc::clone(&fixture.block))
        .expect("store real Native carrier");
    let _finality_receipt = kura
        .store_v2_finality_artifact(&fixture.finality)
        .expect("exact finality");
    native_amx_replay_test_complete_publication(kura, &fixture.block, &fixture.finality);
    let _publication = kura.prune_lock.lock();
    let plan = kura
        .native_amx_participant_application_evidence_for_block_under_publication_guard(
            &fixture.block,
            true,
            NativeAmxMergeAssociation::CommittedOnly,
        )
        .expect("capture completed source before adverse changes");
    let _canonical = kura.canonical_chain_lock.lock();
    let mut changed_wire = fixture.block.as_ref().clone();
    let wrong_key = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    changed_wire
        .replace_signatures(
            [BlockSignature::new(
                0,
                SignatureOf::try_from_hash(wrong_key.private_key(), changed_wire.header().hash())
                    .unwrap(),
            )]
            .into_iter()
            .collect(),
        )
        .unwrap();
    assert_eq!(
        changed_wire.hash(),
        fixture.block.hash(),
        "same header does not authenticate complete wire"
    );
    let files = snapshot_regular_files_recursively(&kura.store_root);
    assert!(
        kura.prepare_native_amx_repair_publication_index(
            &changed_wire,
            None,
            &plan,
            &(0..plan.artifacts.len()).collect::<Vec<_>>()
        )
        .is_err()
    );
    for path in [
        kura.commit_manifest_path(fixture.block.header().height().get()),
        kura.wsv_checkpoint_path(fixture.block.header().height().get()),
    ] {
        let bytes = fs::read(&path).expect("completed join evidence");
        fs::remove_file(&path).expect("remove one exact join dependency");
        let missing_files = snapshot_regular_files_recursively(&kura.store_root);
        assert!(
            kura.prepare_native_amx_repair_publication_index(
                &fixture.block,
                None,
                &plan,
                &(0..plan.artifacts.len()).collect::<Vec<_>>()
            )
            .is_err(),
            "a previously captured plan cannot bypass a missing finalized join"
        );
        assert_eq!(
            snapshot_regular_files_recursively(&kura.store_root),
            missing_files
        );
        fs::write(&path, &bytes).expect("restore exact join evidence");
    }
    let mut wrong_plan = plan;
    wrong_plan.artifacts[0].1.application_block_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"wrong completed checkpoint"));
    assert!(
        kura.prepare_native_amx_repair_publication_index(
            &fixture.block,
            None,
            &wrong_plan,
            &(0..wrong_plan.artifacts.len()).collect::<Vec<_>>()
        )
        .is_err()
    );
    assert_eq!(snapshot_regular_files_recursively(&kura.store_root), files);
    assert!(
        kura.native_amx_publication_capacity_reservations
            .lock()
            .is_empty()
    );
}

#[test]
fn native_amx_missing_unfinished_index_cannot_be_recreated_as_completed_repair() {
    for has_wsv_join in [false, true] {
        let fixture = native_amx_publication_capacity_fixture();
        let kura = &fixture.kura;
        kura.store_block(Arc::clone(&fixture.block))
            .expect("admit actual Native write");
        let _finality_receipt = kura
            .store_v2_finality_artifact(&fixture.finality)
            .expect("exact finality");
        kura.prepublish_native_amx_participant_application_evidence(&fixture.block, None)
            .expect("durable prepublication retains its original pending index");
        if has_wsv_join {
            let checkpoint = Hash::new(b"unfinished repair adverse WSV checkpoint");
            kura.store_wsv_checkpoint(1, fixture.block.hash(), checkpoint)
                .unwrap();
            kura.store_commit_manifest(
                CommitManifest::new(1, fixture.block.hash(), None, None, checkpoint, None)
                    .with_authenticated_v2_commit_authority(&fixture.finality),
            )
            .unwrap();
        }
        let _publication = kura.prune_lock.lock();
        let plan = kura
            .native_amx_participant_application_evidence_for_block_under_publication_guard(
                &fixture.block,
                has_wsv_join,
                NativeAmxMergeAssociation::CommittedOnly,
            )
            .expect("authenticate the exact source without claiming completed publication");
        let _canonical = kura.canonical_chain_lock.lock();
        let leaf = &fixture.manifest.entries().last().unwrap().leaf;
        let entry = kura.lane_storage_entry(leaf.lane_id).unwrap();
        let receipt = Kura::native_amx_participant_receipt_path_for_entry(
            &entry,
            &kura.store_root,
            leaf.participant_height,
        );
        fs::remove_file(receipt).expect("unfinished route lacks its exact receipt");
        let inventory =
            Kura::read_native_amx_publication_index_for_store(&kura.store_root).unwrap();
        assert_eq!(inventory.records.len(), 1);
        for file in &inventory.files {
            fs::remove_file(&file.path).expect("inject lost unfinished index");
        }
        let owners = kura
            .native_amx_publication_capacity_reservations
            .lock()
            .clone();
        let files = snapshot_regular_files_recursively(&kura.store_root);
        assert!(
            kura.prepare_native_amx_repair_publication_index(
                &fixture.block,
                None,
                &plan,
                &(0..plan.artifacts.len()).collect::<Vec<_>>()
            )
            .is_err(),
            "completed repair cannot replace lost unfinished index; WSV join={has_wsv_join}"
        );
        assert_eq!(
            *kura.native_amx_publication_capacity_reservations.lock(),
            owners
        );
        assert_eq!(snapshot_regular_files_recursively(&kura.store_root), files);
    }
}

#[test]
fn native_amx_completed_repair_restart_revalidates_retained_authority_without_growth() {
    for fault in 0..4 {
        let NativeAmxPublicationCapacityFixture {
            _temp_dir,
            kura,
            block,
            manifest,
            finality,
            lane_config,
        } = native_amx_publication_capacity_fixture();
        kura.store_block(Arc::clone(&block))
            .expect("store exact Native carrier");
        let _finality_receipt = kura
            .store_v2_finality_artifact(&finality)
            .expect("retain exact canonical finality");
        native_amx_replay_test_complete_publication(&kura, &block, &finality);
        let leaf = &manifest.entries().last().expect("three exact routes").leaf;
        let entry = kura
            .lane_storage_entry(leaf.lane_id)
            .expect("active target");
        let missing_manifest = Kura::native_amx_application_manifest_path_for_entry(
            &entry,
            &kura.store_root,
            leaf.participant_height,
        );
        fs::remove_file(&missing_manifest).expect("completed route loses only its manifest");
        {
            let _publication = kura.prune_lock.lock();
            let plan = kura
                .native_amx_participant_application_evidence_for_block_under_publication_guard(
                    &block,
                    true,
                    NativeAmxMergeAssociation::CommittedOnly,
                )
                .expect("exact complete repair plan");
            let targets = (0..plan.artifacts.len()).collect::<Vec<_>>();
            assert!(
                kura.ensure_native_amx_publication_capacity_under_publication_guard(
                    &block, &plan, &targets,
                )
                .expect("persist repair owner before the adverse restart cut")
            );
        }
        let height = block.header().height().get();
        let path = match fault {
            0 => kura.commit_manifest_path(height),
            1 => kura.wsv_checkpoint_path(height),
            2 => Kura::native_amx_participant_receipt_path_for_entry(
                &entry,
                &kura.store_root,
                leaf.participant_height,
            ),
            _ => Kura::native_amx_participant_receipt_latest_index_path_for_entry(
                &entry,
                &kura.store_root,
            ),
        };
        let retained_bytes =
            fs::read(&path).expect("prior completed publication retained this dependency");
        fs::remove_file(&path).expect("remove one authority dependency after locator admission");
        let store_root = kura.store_root.clone();
        drop(kura);
        let files = snapshot_regular_files_recursively(&store_root);
        let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
        assert!(
            Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).is_err(),
            "CompletedRepair origin alone cannot authorize startup; missing dependency={fault}"
        );
        assert_eq!(
            snapshot_regular_files_recursively(&store_root),
            files,
            "failed reauthentication cannot repair, normalize or grow durable storage"
        );
        fs::write(&path, retained_bytes).expect("restore exact source bytes, not a new authority");
        let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("the same interrupted repair is recoverable with all original evidence");
        assert!(
            reopened
                .native_amx_publication_capacity_reserved_bytes()
                .unwrap()
                > 0
        );
        assert!(
            !missing_manifest.exists(),
            "read-only startup does not prematurely settle the missing artifact"
        );
    }
}

/// Build an actual second canonical carrier advancing only one Native route.
/// All source transactions, participant statements and both QCs are re-signed;
/// the other participant from the first carrier is absent from the new plan.
fn native_amx_completed_repair_successor_for_one_route(
    fixture: &NativeAmxPublicationCapacityFixture,
    route: LaneId,
) -> (
    Arc<SignedBlock>,
    V2FinalityArtifact,
    crate::sumeragi::exec::NativeAmxApplicationManifestV1,
) {
    use crate::queue::{RouteLeg, RouteLegRole, RoutingDecision, RoutingPlan};
    use iroha_data_model::block::consensus::{NativeAmxParticipantSettlement, NativeAmxPhase};
    let height = fixture.block.header().height().get() + 1;
    let signer = KeyPair::try_from_seed(vec![0x41; 32], Algorithm::BlsNormal)
        .expect("existing Native fixture attestation signer");
    let mut contexts = fixture.block.execution_context().unwrap().external.clone();
    let network_id = contexts[0].native_amx_receipt.as_ref().unwrap().network_id;
    let transactions = (0..contexts.len())
        .map(|index| {
            let key = KeyPair::try_from_seed(
                vec![0x31 + u8::try_from(index).unwrap(); 32],
                Algorithm::Ed25519,
            )
            .expect("existing transaction authority");
            TransactionBuilder::new(
                network_id,
                AccountId::new(key.public_key().clone()),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(
                Level::INFO,
                format!("Native repair successor {height}:{index}"),
            )])
            .sign(key.private_key())
        })
        .collect::<Vec<_>>();
    let hashes = transactions
        .iter()
        .map(|tx| tx.hash_as_entrypoint())
        .collect::<Vec<_>>();
    let sources = transactions
        .iter()
        .map(|tx| *tx.hash().as_ref())
        .collect::<Vec<_>>();
    for (index, context) in contexts.iter_mut().enumerate() {
        context.entrypoint_hash = hashes[index];
        let receipt = context.native_amx_receipt.as_mut().unwrap();
        receipt.source_id = sources[index];
        receipt.authority_context_height = height;
        receipt.lane_block_height += 1;
        let coordinator_route = RoutingDecision::new(receipt.lane_id, receipt.dataspace_id);
        receipt.legs.retain(|leg| {
            leg.lane_id == route
                || (leg.lane_id == receipt.lane_id && leg.dataspace_id == receipt.dataspace_id)
        });
        assert_eq!(
            receipt.legs.len(),
            2,
            "one advancing participant plus the coordinator"
        );
        for leg in &mut receipt.legs {
            let previous_settlement = leg.participant_settlement_hash;
            let descriptor = &mut leg.participant_proposal.descriptor;
            descriptor.proposal_height = height;
            descriptor.previous_lane_block_height = descriptor.lane_block_height;
            descriptor.previous_lane_block_descriptor_hash = Some(descriptor.descriptor_hash);
            descriptor.lane_block_height += 1;
            descriptor.accepted_transaction_hashes =
                hashes.iter().copied().map(Hash::from).collect();
            descriptor.subject_hash = Hash::new_from_chunks(&[
                b"Native repair successor subject",
                &height.to_le_bytes(),
                descriptor.subject_hash.as_ref(),
            ]);
            descriptor.payload_ownership_hash = Hash::new_from_chunks(&[
                b"Native repair successor ownership",
                &height.to_le_bytes(),
                descriptor.payload_ownership_hash.as_ref(),
            ]);
            descriptor.rbc_instance_hash = Hash::new_from_chunks(&[
                b"Native repair successor availability",
                &height.to_le_bytes(),
                descriptor.rbc_instance_hash.as_ref(),
            ]);
            descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
            leg.participant_proposal.proposal_hash =
                leg.participant_proposal.computed_proposal_hash();
            crate::lane_consensus::validate_lane_block_proposal(&leg.participant_proposal).unwrap();
            let descriptor = &leg.participant_proposal.descriptor;
            leg.participant_settlement = NativeAmxParticipantSettlement::try_new(
                leg.lane_id,
                leg.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.lane_block_height,
                height,
                Some(previous_settlement),
                sources.clone(),
            )
            .expect("exact contiguous grouped successor settlement");
            leg.participant_settlement_hash = leg.participant_settlement.computed_hash().unwrap();
        }
        let coordinator = receipt
            .legs
            .iter()
            .find(|leg| leg.lane_id == receipt.lane_id && leg.dataspace_id == receipt.dataspace_id)
            .unwrap()
            .participant_proposal
            .clone();
        receipt.coordinator_proposal_hash = coordinator.proposal_hash;
        let routing_plan = RoutingPlan::native_amx(
            coordinator_route,
            receipt
                .legs
                .iter()
                .map(|leg| {
                    RouteLeg::new(
                        RoutingDecision::new(leg.lane_id, leg.dataspace_id),
                        RouteLegRole::Participant,
                    )
                })
                .collect(),
        );
        receipt.plan_digest = routing_plan.digest();
        context.routing_plan_digest = receipt.plan_digest;
        context.routing_plan_legs =
            crate::queue::execution_context_legs_for_routing_plan(&routing_plan);
        for leg in &mut receipt.legs {
            let descriptor = &leg.participant_proposal.descriptor;
            let mut prepare = leg.prepare_qc.body;
            prepare.round.height = height;
            prepare.source_id = sources[index];
            prepare.tx_entrypoint_hash = hashes[index];
            prepare.plan_digest = receipt.plan_digest;
            prepare.authority_context_height = height;
            prepare.planned_coordinator_block_height = coordinator.descriptor.lane_block_height;
            prepare.coordinator_proposal_hash = coordinator.proposal_hash;
            prepare.participant_previous_block_height = descriptor.previous_lane_block_height;
            prepare.participant_previous_block_descriptor_hash =
                descriptor.previous_lane_block_descriptor_hash;
            prepare.participant_lane_block_height = descriptor.lane_block_height;
            prepare.participant_proposal_hash = leg.participant_proposal.proposal_hash;
            prepare.participant_settlement_commitment = Hash::from(leg.participant_settlement_hash);
            prepare.phase = NativeAmxPhase::Prepare;
            let mut commit = prepare;
            commit.phase = NativeAmxPhase::Commit;
            leg.prepare_qc = native_amx_capacity_fixture_qc(prepare, &signer);
            leg.commit_qc = native_amx_capacity_fixture_qc(commit, &signer);
        }
        assert!(
            crate::native_amx::receipt_shape_matches_coordinator_payload(
                Some(receipt),
                &routing_plan,
                &receipt.source_id,
                Hash::from(context.entrypoint_hash),
                receipt.network_id,
                &coordinator,
            )
        );
    }
    let header = BlockHeader::new(
        NonZeroU64::new(height).unwrap(),
        Some(fixture.block.hash()),
        iroha_crypto::MerkleTree::root_from_typed_leaves(hashes.iter().copied()),
        fixture
            .block
            .header()
            .creation_time()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
            + 1,
        6,
    );
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(signer.private_key(), header.hash()).unwrap(),
    );
    let mut block = SignedBlock::presigned(signature, header, transactions);
    block.set_execution_context(Some(BlockExecutionContextBundle::new(contexts)));
    attach_ok_results_to_block(&mut block);
    block
        .replace_signatures(
            [BlockSignature::new(
                0,
                SignatureOf::try_from_hash(signer.private_key(), block.header().hash()).unwrap(),
            )]
            .into_iter()
            .collect(),
        )
        .unwrap();
    let manifest =
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(&block)
            .unwrap();
    assert_eq!(manifest.count(), 1);
    assert_eq!(manifest.entries()[0].leaf.lane_id, route);
    let mut execution = v2_finality_fixture_execution_commitment();
    execution.native_amx_application_manifest_root = manifest.root();
    execution.native_amx_application_manifest_count = manifest.count();
    let finality = v2_finality_artifact_for_block_with_keys(
        &block,
        Some(&fixture.finality),
        &v2_finality_fixture_keys(),
        execution,
    );
    (Arc::new(block), finality, manifest)
}

fn native_amx_complete_exact_fixture_carrier(
    kura: &Kura,
    block: &SignedBlock,
    finality: &V2FinalityArtifact,
    routes: usize,
) {
    kura.store_block(Arc::new(block.clone()))
        .expect("actual canonical append owns publication first");
    let _receipt = kura
        .store_v2_finality_artifact(finality)
        .expect("authenticate exact canonical finality");
    kura.prepublish_native_amx_participant_application_evidence(block, None)
        .expect("complete all real Native publication components before WSV join");
    let height = block.header().height().get();
    let checkpoint =
        Hash::new_from_chunks(&[b"completed repair fixture WSV", &height.to_le_bytes()]);
    kura.store_wsv_checkpoint(height, block.hash(), checkpoint)
        .unwrap();
    kura.store_commit_manifest(
        CommitManifest::new(height, block.hash(), None, None, checkpoint, None)
            .with_authenticated_v2_commit_authority(finality),
    )
    .unwrap();
    assert_eq!(
        kura.repair_native_amx_participant_application_evidence(block)
            .unwrap(),
        routes
    );
    assert!(
        Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .unwrap()
            .records
            .is_empty()
    );
    assert!(
        kura.native_amx_publication_capacity_reservations
            .lock()
            .is_empty()
    );
}

#[test]
fn native_amx_partial_completed_repair_preserves_later_route_across_cold_reopen() {
    let fixture = native_amx_publication_capacity_fixture_with_route_count(2);
    native_amx_complete_exact_fixture_carrier(&fixture.kura, &fixture.block, &fixture.finality, 2);
    let a = fixture.manifest.entries()[0].leaf.lane_id;
    let b = fixture.manifest.entries()[1].leaf.lane_id;
    let (second, second_finality, second_manifest) =
        native_amx_completed_repair_successor_for_one_route(&fixture, b);
    native_amx_complete_exact_fixture_carrier(&fixture.kura, &second, &second_finality, 1);
    let (incarnations, activations) =
        active_fixture_geometry_maps(&fixture.kura, &fixture.lane_config);
    let NativeAmxPublicationCapacityFixture {
        _temp_dir,
        kura,
        block,
        lane_config,
        ..
    } = fixture;
    let markers = crate::state::State::native_amx_participant_frontier_markers(&block).unwrap();
    let marker = markers
        .iter()
        .find(|marker| marker.lane_id == a)
        .unwrap()
        .clone();
    let a_entry = kura.lane_storage_entry(a).unwrap();
    let b_entry = kura.lane_storage_entry(b).unwrap();
    let a_path =
        Kura::native_amx_application_manifest_path_for_entry(&a_entry, &kura.store_root, 1);
    let a_bytes = fs::read(&a_path).unwrap();
    let b_directory = Kura::lane_artifact_dir(&b_entry.blocks_dir(&kura.store_root));
    let b_files = snapshot_regular_files_recursively(&b_directory);
    let b_latest_path = Kura::native_amx_participant_receipt_latest_index_path_for_entry(
        &b_entry,
        &kura.store_root,
    );
    let b_latest = fs::read(&b_latest_path).unwrap();
    fs::remove_file(&a_path).expect("lose only the old A1 manifest after B2 completed");
    let carrier = Kura::native_amx_publication_carrier(&block).unwrap();
    {
        let _publication = kura.prune_lock.lock();
        let plan = kura
            .native_amx_participant_application_evidence_for_block_under_publication_guard(
                &block,
                true,
                NativeAmxMergeAssociation::CommittedOnly,
            )
            .unwrap();
        let targets = kura
            .native_amx_participant_application_repair_target_indices(
                &plan,
                std::slice::from_ref(&marker),
            )
            .unwrap();
        assert_eq!(targets.len(), 1);
        let before = snapshot_regular_files_recursively(&kura.store_root);
        for invalid in [
            Vec::new(),
            vec![targets[0], targets[0]],
            vec![plan.artifacts.len()],
        ] {
            assert!(
                kura.ensure_native_amx_publication_capacity_under_publication_guard(
                    &block, &plan, &invalid
                )
                .is_err()
            );
            assert_eq!(
                snapshot_regular_files_recursively(&kura.store_root),
                before,
                "invalid exact target selection must fail before durable side effects"
            );
            assert!(
                kura.native_amx_publication_capacity_reservations
                    .lock()
                    .is_empty()
            );
        }
        assert!(
            kura.ensure_native_amx_publication_capacity_under_publication_guard(
                &block, &plan, &targets
            )
            .expect("original complete carrier plus later B2 completion covers every route")
        );
        let owners = kura.native_amx_publication_capacity_reservations.lock();
        let owner = owners
            .get(&carrier)
            .expect("one durable complete-carrier repair owner");
        assert_eq!(
            owner.index_record.as_ref().unwrap().origin,
            NativeAmxPublicationIndexOriginV1::CompletedRepair
        );
        assert_eq!(owner.routes.len(), 1, "B2 independently proves B1 terminal");
        assert_eq!(owner.routes.keys().next().unwrap().lane_id, a);
        assert!(
            !a_path.exists(),
            "interrupt after durable repair admission, before A1 write"
        );
    }
    assert_eq!(snapshot_regular_files_recursively(&b_directory), b_files);
    let reserved = kura
        .native_amx_publication_capacity_reserved_bytes()
        .unwrap();
    assert!(reserved > 0);
    let b_manifest = Kura::native_amx_application_manifest_path_for_entry(
        &b_entry,
        &kura.store_root,
        second_manifest.entries()[0].leaf.participant_height,
    );
    let b_manifest_bytes = fs::read(&b_manifest).unwrap();
    fs::remove_file(&b_manifest).expect("lose the non-target terminal proof after admission");
    let store_root = kura.store_root.clone();
    let network_id = kura.bound_lane_storage_network().unwrap();
    drop(kura);
    let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
    let damaged = snapshot_regular_files_recursively(&store_root);
    assert!(
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).is_err(),
        "restart must reauthenticate B2, not trust the old CompletedRepair tag"
    );
    assert_eq!(snapshot_regular_files_recursively(&store_root), damaged);
    fs::write(&b_manifest, b_manifest_bytes).expect("restore only the original B2 proof bytes");
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("restart derives the exact partial repair through journal physical targets");
    assert_eq!(
        reopened.lane_storage_entries.lock().len(),
        0,
        "repair reconstruction must not publish any live State geometry"
    );
    assert_eq!(
        reopened
            .native_amx_publication_capacity_reserved_bytes()
            .unwrap(),
        reserved
    );
    reopened.bind_lane_storage_network(network_id).unwrap();
    reopened
        .recover_lane_geometry_journal(&lane_config, &incarnations, &activations)
        .unwrap();
    reopened
        .finish_restored_lane_segments_with_geometry(&lane_config)
        .unwrap();
    assert_eq!(
        reopened
            .repair_native_amx_participant_application_evidence_for_markers(
                &block,
                std::slice::from_ref(&marker)
            )
            .unwrap(),
        1
    );
    assert_eq!(fs::read(&a_path).unwrap(), a_bytes);
    assert_eq!(
        fs::read(&b_latest_path).unwrap(),
        b_latest,
        "A1 repair cannot roll B2 back to B1"
    );
    assert_eq!(snapshot_regular_files_recursively(&b_directory), b_files);
    assert!(
        Kura::read_native_amx_publication_index_for_store(&reopened.store_root)
            .unwrap()
            .records
            .is_empty()
    );
    assert_eq!(
        reopened
            .native_amx_publication_capacity_reserved_bytes()
            .unwrap(),
        0
    );
    let completed = snapshot_regular_files_recursively(&store_root);
    assert_eq!(
        reopened
            .repair_native_amx_participant_application_evidence_for_markers(
                &block,
                std::slice::from_ref(&marker)
            )
            .unwrap(),
        1
    );
    assert!(
        reopened
            .repair_native_amx_participant_application_evidence(&block)
            .is_err(),
        "requesting stale B1 publication remains prohibited even after A1 repair"
    );
    assert_eq!(snapshot_regular_files_recursively(&store_root), completed);
}

#[test]
fn native_amx_partial_completed_repair_rejects_unproved_non_target_completion() {
    for fault in 0..5 {
        let fixture = native_amx_publication_capacity_fixture_with_route_count(2);
        let kura = &fixture.kura;
        native_amx_complete_exact_fixture_carrier(kura, &fixture.block, &fixture.finality, 2);
        let a = fixture.manifest.entries()[0].leaf.lane_id;
        let b = fixture.manifest.entries()[1].leaf.lane_id;
        if fault != 4 {
            let (second, finality, _) =
                native_amx_completed_repair_successor_for_one_route(&fixture, b);
            native_amx_complete_exact_fixture_carrier(kura, &second, &finality, 1);
        }
        let a_entry = kura.lane_storage_entry(a).unwrap();
        let b_entry = kura.lane_storage_entry(b).unwrap();
        let a_path =
            Kura::native_amx_application_manifest_path_for_entry(&a_entry, &kura.store_root, 1);
        fs::remove_file(a_path).unwrap();
        match fault {
            0 => fs::remove_file(Kura::native_amx_application_manifest_path_for_entry(
                &b_entry,
                &kura.store_root,
                2,
            ))
            .unwrap(),
            1 => fs::write(
                Kura::native_amx_participant_receipt_path_for_entry(&b_entry, &kura.store_root, 2),
                [0xA5],
            )
            .unwrap(),
            2 => fs::remove_file(kura.wsv_checkpoint_path(2)).unwrap(),
            3 => {
                let (stable, temporary) = native_amx_latest_index_test_paths(kura, &b_entry);
                write_synced_native_amx_test_file(&temporary, &fs::read(stable).unwrap());
            }
            _ => fs::remove_file(Kura::native_amx_application_manifest_path_for_entry(
                &b_entry,
                &kura.store_root,
                1,
            ))
            .unwrap(),
        }
        let marker = crate::state::State::native_amx_participant_frontier_markers(&fixture.block)
            .unwrap()
            .into_iter()
            .find(|marker| marker.lane_id == a)
            .unwrap();
        let before = snapshot_regular_files_recursively(&kura.store_root);
        assert!(
            kura.repair_native_amx_participant_application_evidence_for_markers(
                &fixture.block,
                &[marker]
            )
            .is_err(),
            "A1 target cannot omit unproved non-target B publication; fault={fault}"
        );
        assert_eq!(
            snapshot_regular_files_recursively(&kura.store_root),
            before,
            "failed full-carrier completion proof must not publish A1 or replace B authority"
        );
        assert!(
            Kura::read_native_amx_publication_index_for_store(&kura.store_root)
                .unwrap()
                .records
                .is_empty()
        );
        assert!(
            kura.native_amx_publication_capacity_reservations
                .lock()
                .is_empty()
        );
    }
}

struct NativeAmxCompletedRepairTempFixture {
    source: NativeAmxPublicationCapacityFixture,
    manifest_path: PathBuf,
    manifest_bytes: Vec<u8>,
    receipt_path: PathBuf,
    receipt_bytes: Vec<u8>,
    latest_path: PathBuf,
    latest_bytes: Vec<u8>,
    reserved_before_write: u64,
    index: NativeAmxPublicationIndexRecord,
}

#[derive(Clone, Copy, Debug)]
enum NativeAmxRepairTempCut {
    Synced,
    Empty,
    Partial,
}

fn native_amx_completed_repair_after_actual_temp_sync() -> NativeAmxCompletedRepairTempFixture {
    native_amx_completed_repair_after_actual_temp_cut(NativeAmxRepairTempCut::Synced)
}

// These cuts use the real repair writer after its exact index is durable:
// exclusive creation, a proper prefix write, or complete fsync before rename.
fn native_amx_completed_repair_after_actual_temp_cut(
    cut: NativeAmxRepairTempCut,
) -> NativeAmxCompletedRepairTempFixture {
    let source = native_amx_publication_capacity_fixture();
    let kura = &source.kura;
    native_amx_complete_exact_fixture_carrier(kura, &source.block, &source.finality, 3);
    let leaf = &match cut {
        NativeAmxRepairTempCut::Synced => source.manifest.entries().last().unwrap(),
        NativeAmxRepairTempCut::Empty | NativeAmxRepairTempCut::Partial => {
            source.manifest.entries().first().unwrap()
        }
    }
    .leaf;
    let entry = kura.lane_storage_entry(leaf.lane_id).unwrap();
    let manifest_path = Kura::native_amx_application_manifest_path_for_entry(
        &entry,
        &kura.store_root,
        leaf.participant_height,
    );
    let receipt_path = Kura::native_amx_participant_receipt_path_for_entry(
        &entry,
        &kura.store_root,
        leaf.participant_height,
    );
    let latest_path =
        Kura::native_amx_participant_receipt_latest_index_path_for_entry(&entry, &kura.store_root);
    let manifest_bytes = fs::read(&manifest_path).unwrap();
    let receipt_bytes = fs::read(&receipt_path).unwrap();
    let latest_bytes = fs::read(&latest_path).unwrap();
    fs::remove_file(&manifest_path).unwrap();
    let carrier = Kura::native_amx_publication_carrier(&source.block).unwrap();
    {
        let _publication = kura.prune_lock.lock();
        let plan = kura
            .native_amx_participant_application_evidence_for_block_under_publication_guard(
                &source.block,
                true,
                NativeAmxMergeAssociation::CommittedOnly,
            )
            .unwrap();
        let targets = (0..plan.artifacts.len()).collect::<Vec<_>>();
        assert!(
            kura.ensure_native_amx_publication_capacity_under_publication_guard(
                &source.block,
                &plan,
                &targets,
            )
            .unwrap()
        );
    }
    let index = Kura::read_native_amx_publication_index_for_store(&kura.store_root)
        .unwrap()
        .records[&carrier]
        .clone();
    assert_eq!(
        index.origin,
        NativeAmxPublicationIndexOriginV1::CompletedRepair
    );
    let reserved_before_write = kura
        .native_amx_publication_capacity_reserved_bytes()
        .unwrap();
    assert!(reserved_before_write >= u64::try_from(manifest_bytes.len()).unwrap());
    let (temporary_len, expected_error) = match cut {
        NativeAmxRepairTempCut::Synced => {
            fail_after_next_native_amx_evidence_temp_sync_for_tests();
            (manifest_bytes.len(), "interruption after temporary fsync")
        }
        NativeAmxRepairTempCut::Empty | NativeAmxRepairTempCut::Partial => {
            let bytes = match cut {
                NativeAmxRepairTempCut::Empty => 0,
                _ => manifest_bytes.len() / 2,
            };
            assert!(bytes < manifest_bytes.len());
            fail_after_next_native_amx_evidence_temp_prefix_for_tests(bytes);
            (bytes, "interruption during temporary write")
        }
    };
    let error = kura
        .repair_native_amx_participant_application_evidence(&source.block)
        .expect_err("actual repair stops at the selected physical writer crash cut");
    assert!(error.to_string().contains(expected_error), "{error}");
    assert!(
        !FAIL_AFTER_NEXT_NATIVE_AMX_EVIDENCE_TEMP_SYNC.with(|flag| flag.get())
            && FAIL_AFTER_NEXT_NATIVE_AMX_EVIDENCE_TEMP_PREFIX.with(|flag| flag.get().is_none()),
        "the real physical writer consumed the crash cut"
    );
    assert!(!manifest_path.exists());
    assert_eq!(
        fs::read(manifest_path.with_extension("norito.tmp")).unwrap(),
        manifest_bytes[..temporary_len]
    );
    assert_eq!(fs::read(&receipt_path).unwrap(), receipt_bytes);
    assert_eq!(fs::read(&latest_path).unwrap(), latest_bytes);
    assert_eq!(
        Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .unwrap()
            .records[&carrier],
        index
    );
    NativeAmxCompletedRepairTempFixture {
        source,
        manifest_path,
        manifest_bytes,
        receipt_path,
        receipt_bytes,
        latest_path,
        latest_bytes,
        reserved_before_write,
        index,
    }
}

#[test]
fn native_amx_completed_repair_recovers_actual_synced_temporary_on_restart() {
    let NativeAmxCompletedRepairTempFixture {
        source:
            NativeAmxPublicationCapacityFixture {
                _temp_dir,
                kura,
                block,
                manifest: _,
                finality: _,
                lane_config,
            },
        manifest_path,
        manifest_bytes,
        receipt_path,
        receipt_bytes,
        latest_path,
        latest_bytes,
        reserved_before_write,
        index,
    } = native_amx_completed_repair_after_actual_temp_sync();
    let network_id = kura.bound_lane_storage_network().unwrap();
    let (incarnations, activations) = active_fixture_geometry_maps(&kura, &lane_config);
    let temp_path = manifest_path.with_extension("norito.tmp");
    let before_reopen = snapshot_regular_files_recursively(&kura.store_root);
    // The capacity phase authenticates the owned temporary without publishing
    // it. Complete Strict startup subsequently has retained-instance authority
    // to promote the bytes and retire the exact finished locator before State.
    kura.rebuild_native_amx_publication_capacity_on_startup()
        .expect("authenticate the original repair index and its exact temporary");
    assert_eq!(
        Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .unwrap()
            .records[&index.carrier],
        index,
        "capacity authentication retains the original locator"
    );
    assert_eq!(
        kura.native_amx_publication_capacity_reserved_bytes()
            .unwrap(),
        reserved_before_write - u64::try_from(manifest_bytes.len()).unwrap(),
        "physical temp bytes consume exactly their allocation, never a second reservation"
    );
    assert_eq!(
        snapshot_regular_files_recursively(&kura.store_root),
        before_reopen,
        "capacity authentication itself must not mutate repair evidence"
    );
    let mut expected_completed = before_reopen;
    assert_eq!(
        expected_completed.remove(temp_path.strip_prefix(&kura.store_root).unwrap()),
        Some(manifest_bytes.clone())
    );
    assert!(
        expected_completed
            .remove(
                &PathBuf::from(NATIVE_AMX_PUBLICATION_INDEX_DIRECTORY)
                    .join(index.file_name().unwrap())
            )
            .is_some()
    );
    assert!(
        expected_completed
            .insert(
                manifest_path
                    .strip_prefix(&kura.store_root)
                    .unwrap()
                    .to_path_buf(),
                manifest_bytes.clone()
            )
            .is_none()
    );
    drop(kura);
    let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("an exact retained repair index owns the canonical temporary on Strict restart");
    assert!(reopened.lane_storage_entries.lock().is_empty());
    assert_eq!(
        snapshot_regular_files_recursively(&reopened.store_root),
        expected_completed,
        "Strict startup only promotes the authenticated manifest and retires its completed index"
    );
    reopened.bind_lane_storage_network(network_id).unwrap();
    reopened
        .recover_lane_geometry_journal(&lane_config, &incarnations, &activations)
        .unwrap();
    reopened
        .finish_restored_lane_segments_with_geometry(&lane_config)
        .expect("existing authenticated temporary recovery completes the exact publication");
    assert_eq!(
        reopened
            .repair_native_amx_participant_application_evidence(&block)
            .unwrap(),
        3
    );
    assert_eq!(fs::read(&manifest_path).unwrap(), manifest_bytes);
    assert!(!temp_path.exists());
    assert_eq!(
        fs::read(&receipt_path).unwrap(),
        receipt_bytes,
        "original completed receipt is untouched"
    );
    assert_eq!(
        fs::read(&latest_path).unwrap(),
        latest_bytes,
        "original completed latest pointer is untouched"
    );
    assert!(
        Kura::read_native_amx_publication_index_for_store(&reopened.store_root)
            .unwrap()
            .records
            .is_empty()
    );
    assert_eq!(
        reopened
            .native_amx_publication_capacity_reserved_bytes()
            .unwrap(),
        0
    );
    let completed = snapshot_regular_files_recursively(&reopened.store_root);
    assert_eq!(
        reopened
            .repair_native_amx_participant_application_evidence(&block)
            .unwrap(),
        3
    );
    assert_eq!(
        snapshot_regular_files_recursively(&reopened.store_root),
        completed,
        "completed retry neither grows storage nor recreates the retired locator"
    );
}

#[test]
fn native_amx_completed_repair_retries_actual_synced_temporary_with_original_index() {
    let cut = native_amx_completed_repair_after_actual_temp_sync();
    let kura = &cut.source.kura;
    assert_eq!(
        kura.repair_native_amx_participant_application_evidence(&cut.source.block)
            .unwrap(),
        3,
        "same-process retry uses the same admitted repair, not pristine admission"
    );
    assert_eq!(fs::read(&cut.manifest_path).unwrap(), cut.manifest_bytes);
    assert!(!cut.manifest_path.with_extension("norito.tmp").exists());
    assert_eq!(fs::read(&cut.receipt_path).unwrap(), cut.receipt_bytes);
    assert_eq!(fs::read(&cut.latest_path).unwrap(), cut.latest_bytes);
    assert!(
        Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .unwrap()
            .records
            .is_empty()
    );
    assert_eq!(
        kura.native_amx_publication_capacity_reserved_bytes()
            .unwrap(),
        0
    );
}

#[test]
fn native_amx_completed_repair_recovers_actual_empty_and_partial_temporaries_on_restart() {
    for crash_cut in [
        NativeAmxRepairTempCut::Empty,
        NativeAmxRepairTempCut::Partial,
    ] {
        let cut = native_amx_completed_repair_after_actual_temp_cut(crash_cut);
        let NativeAmxPublicationCapacityFixture {
            _temp_dir,
            kura,
            block,
            lane_config,
            ..
        } = cut.source;
        let network_id = kura.bound_lane_storage_network().unwrap();
        let (incarnations, activations) = active_fixture_geometry_maps(&kura, &lane_config);
        let temp_path = cut.manifest_path.with_extension("norito.tmp");
        let mut expected_recovered = snapshot_regular_files_recursively(&kura.store_root);
        assert!(
            expected_recovered
                .remove(temp_path.strip_prefix(&kura.store_root).unwrap())
                .is_some()
        );
        let mut expected_completed = expected_recovered.clone();
        assert!(
            expected_completed
                .remove(
                    &PathBuf::from(NATIVE_AMX_PUBLICATION_INDEX_DIRECTORY)
                        .join(cut.index.file_name().unwrap())
                )
                .is_some()
        );
        assert!(
            expected_completed
                .insert(
                    cut.manifest_path
                        .strip_prefix(&kura.store_root)
                        .unwrap()
                        .to_path_buf(),
                    cut.manifest_bytes.clone(),
                )
                .is_none()
        );
        drop(kura);
        let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
        let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("Strict restart authenticates and removes only the owned incomplete manifest");
        assert!(reopened.lane_storage_entries.lock().is_empty());
        assert_eq!(
            snapshot_regular_files_recursively(&reopened.store_root),
            expected_recovered
        );
        assert!(!cut.manifest_path.exists());
        assert!(!temp_path.exists());
        assert_eq!(
            Kura::read_native_amx_publication_index_for_store(&reopened.store_root)
                .unwrap()
                .records[&cut.index.carrier],
            cut.index,
            "prefix cleanup retains the exact original durable repair locator; cut={crash_cut:?}"
        );
        assert_eq!(
            reopened
                .native_amx_publication_capacity_reserved_bytes()
                .unwrap(),
            cut.reserved_before_write
        );
        assert_eq!(fs::read(&cut.receipt_path).unwrap(), cut.receipt_bytes);
        assert_eq!(fs::read(&cut.latest_path).unwrap(), cut.latest_bytes);
        reopened.bind_lane_storage_network(network_id).unwrap();
        reopened
            .recover_lane_geometry_journal(&lane_config, &incarnations, &activations)
            .unwrap();
        reopened
            .finish_restored_lane_segments_with_geometry(&lane_config)
            .unwrap();
        assert_eq!(
            reopened
                .repair_native_amx_participant_application_evidence(&block)
                .unwrap(),
            3
        );
        assert_eq!(
            snapshot_regular_files_recursively(&reopened.store_root),
            expected_completed
        );
        assert!(
            Kura::read_native_amx_publication_index_for_store(&reopened.store_root)
                .unwrap()
                .records
                .is_empty()
        );
        assert_eq!(
            reopened
                .native_amx_publication_capacity_reserved_bytes()
                .unwrap(),
            0
        );
        assert_eq!(
            reopened
                .repair_native_amx_participant_application_evidence(&block)
                .unwrap(),
            3
        );
        assert_eq!(
            snapshot_regular_files_recursively(&reopened.store_root),
            expected_completed
        );
    }
}

#[test]
fn native_amx_completed_repair_retries_actual_empty_and_partial_temporaries() {
    for crash_cut in [
        NativeAmxRepairTempCut::Empty,
        NativeAmxRepairTempCut::Partial,
    ] {
        let cut = native_amx_completed_repair_after_actual_temp_cut(crash_cut);
        let kura = &cut.source.kura;
        assert_eq!(
            kura.repair_native_amx_participant_application_evidence(&cut.source.block)
                .unwrap(),
            3
        );
        assert_eq!(fs::read(&cut.manifest_path).unwrap(), cut.manifest_bytes);
        assert!(!cut.manifest_path.with_extension("norito.tmp").exists());
        assert_eq!(fs::read(&cut.receipt_path).unwrap(), cut.receipt_bytes);
        assert_eq!(fs::read(&cut.latest_path).unwrap(), cut.latest_bytes);
        assert!(
            Kura::read_native_amx_publication_index_for_store(&kura.store_root)
                .unwrap()
                .records
                .is_empty()
        );
        assert_eq!(
            kura.native_amx_publication_capacity_reserved_bytes()
                .unwrap(),
            0
        );
        let completed = snapshot_regular_files_recursively(&kura.store_root);
        assert_eq!(
            kura.repair_native_amx_participant_application_evidence(&cut.source.block)
                .unwrap(),
            3
        );
        assert_eq!(
            snapshot_regular_files_recursively(&kura.store_root),
            completed
        );
    }
}

#[test]
fn native_amx_completed_repair_rejects_unowned_and_mixed_invalid_prefixes_without_cleanup() {
    for fault in 0..3 {
        for crash_cut in [
            NativeAmxRepairTempCut::Empty,
            NativeAmxRepairTempCut::Partial,
        ] {
            let cut = native_amx_completed_repair_after_actual_temp_cut(crash_cut);
            let kura = &cut.source.kura;
            let temp_path = cut.manifest_path.with_extension("norito.tmp");
            match fault {
                0 => {
                    // The valid prefix belongs to the first route. An invalid
                    // later route must prevent cleanup of either physical object.
                    let other = &cut.source.manifest.entries().last().unwrap().leaf;
                    let entry = kura.lane_storage_entry(other.lane_id).unwrap();
                    let foreign = Kura::native_amx_application_manifest_path_for_entry(
                        &entry,
                        &kura.store_root,
                        other.participant_height,
                    )
                    .with_extension("norito.tmp");
                    assert_ne!(foreign, temp_path);
                    write_synced_native_amx_test_file(&foreign, &[0]);
                }
                1 => {
                    let inventory =
                        Kura::read_native_amx_publication_index_for_store(&kura.store_root)
                            .unwrap();
                    assert_eq!(inventory.files.len(), 1);
                    fs::remove_file(&inventory.files[0].path).unwrap();
                }
                _ => {
                    // Still short, but not a prefix of the authenticated manifest.
                    write_synced_native_amx_test_file(&temp_path, &[cut.manifest_bytes[0] ^ 0xff]);
                }
            }
            let before = snapshot_regular_files_recursively(&kura.store_root);
            let reserved = kura
                .native_amx_publication_capacity_reserved_bytes()
                .unwrap();
            assert!(
                kura.repair_native_amx_participant_application_evidence(&cut.source.block)
                    .is_err(),
                "fault={fault}, cut={crash_cut:?}"
            );
            assert_eq!(snapshot_regular_files_recursively(&kura.store_root), before);
            assert_eq!(
                kura.native_amx_publication_capacity_reserved_bytes()
                    .unwrap(),
                reserved
            );
            let store_root = kura.store_root.clone();
            let NativeAmxPublicationCapacityFixture {
                _temp_dir,
                kura,
                lane_config,
                ..
            } = cut.source;
            drop(kura);
            let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
            assert!(
                Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).is_err(),
                "fault={fault}, cut={crash_cut:?}"
            );
            assert_eq!(
                snapshot_regular_files_recursively(&store_root),
                before,
                "invalid sibling or absent authority prevents every prefix unlink"
            );
        }
    }
}

#[test]
fn native_amx_completed_repair_rejects_foreign_tampered_and_unowned_temporaries() {
    for fault in 0..4 {
        let cut = native_amx_completed_repair_after_actual_temp_sync();
        let kura = &cut.source.kura;
        let temp_path = cut.manifest_path.with_extension("norito.tmp");
        match fault {
            0 => {
                let mut tampered = cut.manifest_bytes.clone();
                tampered.push(0); // Complete canonical content plus unowned trailing bytes.
                write_synced_native_amx_test_file(&temp_path, &tampered);
            }
            1 => {
                let other = &cut.source.manifest.entries()[0].leaf;
                let entry = kura.lane_storage_entry(other.lane_id).unwrap();
                let foreign = Kura::native_amx_application_manifest_path_for_entry(
                    &entry,
                    &kura.store_root,
                    other.participant_height,
                );
                write_synced_native_amx_test_file(&temp_path, &fs::read(foreign).unwrap());
            }
            2 => {
                let inventory =
                    Kura::read_native_amx_publication_index_for_store(&kura.store_root).unwrap();
                assert_eq!(inventory.files.len(), 1);
                fs::remove_file(&inventory.files[0].path).unwrap();
                // Losing the locator cannot turn an interrupted repair into a
                // fresh repair admission, even with exact finality/receipt bytes.
                let before = snapshot_regular_files_recursively(&kura.store_root);
                assert!(
                    kura.repair_native_amx_participant_application_evidence(&cut.source.block)
                        .is_err()
                );
                assert_eq!(snapshot_regular_files_recursively(&kura.store_root), before);
            }
            _ => {
                let wrong_height = temp_path
                    .parent()
                    .unwrap()
                    .join(Kura::native_amx_evidence_file_name(
                        NativeAmxEvidenceKind::Manifest,
                        2,
                    ))
                    .with_extension("norito.tmp");
                fs::rename(&temp_path, wrong_height).unwrap();
            }
        }
        let store_root = kura.store_root.clone();
        let files = snapshot_regular_files_recursively(&store_root);
        let NativeAmxPublicationCapacityFixture {
            _temp_dir,
            kura,
            lane_config,
            ..
        } = cut.source;
        drop(kura);
        let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
        assert!(
            Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).is_err(),
            "foreign, noncanonical, wrong-height or unindexed temp cannot acquire recovery authority; fault={fault}"
        );
        assert_eq!(
            snapshot_regular_files_recursively(&store_root),
            files,
            "failed restart preserves all authoritative and forensic bytes; fault={fault}"
        );
    }
}

#[test]
fn native_amx_retained_prepublication_reauthenticates_exact_original_kura_and_frontiers() {
    let fixture = native_amx_publication_capacity_fixture();
    fixture
        .kura
        .store_block(Arc::clone(&fixture.block))
        .unwrap();
    let receipt = fixture
        .kura
        .store_v2_finality_artifact(&fixture.finality)
        .unwrap();
    assert_eq!(receipt.artifact_hash(), HashOf::new(&fixture.finality));
    let token = fixture
        .kura
        .prepublish_native_amx_participant_application_evidence(&fixture.block, None)
        .unwrap();
    let frontiers =
        crate::state::State::native_amx_participant_frontier_markers(&fixture.block).unwrap();
    assert_eq!(frontiers.len(), 3);
    let before = snapshot_regular_files_recursively(&fixture.kura.store_root);
    fixture
        .kura
        .reauthenticate_native_amx_prepublication(
            &token,
            &fixture.block,
            &fixture.manifest,
            &fixture.finality,
            &frontiers,
        )
        .expect("live Apply uses the same original-Kura durable join");
    drop(
        fixture
            .kura
            .try_publication_lease()
            .expect("live wrapper releases all Kura fences before State staging"),
    );
    assert!(
        fixture
            .kura
            .reauthenticate_native_amx_prepublication(
                &token,
                &fixture.block,
                &fixture.manifest,
                &fixture.finality,
                &frontiers[..2],
            )
            .is_err()
    );
    drop(
        fixture
            .kura
            .try_publication_lease()
            .expect("live wrapper also releases all fences on refusal"),
    );
    {
        let lease = fixture.kura.try_publication_lease().unwrap();
        lease
            .reauthenticate_native_amx_prepublication(
                &token,
                &fixture.block,
                &fixture.manifest,
                &fixture.finality,
                &frontiers,
            )
            .expect("original nonempty participant evidence under all original fences");
        let mut wrong_finality = fixture.finality.clone();
        wrong_finality
            .commit_qc
            .execution_commitment
            .post_state_root = Hash::new(b"another retained participant execution");
        assert!(
            lease
                .reauthenticate_native_amx_prepublication(
                    &token,
                    &fixture.block,
                    &fixture.manifest,
                    &wrong_finality,
                    &frontiers,
                )
                .is_err(),
            "retained token cannot authorize a different decided execution"
        );
        let mut reordered = frontiers.clone();
        reordered.swap(0, 1);
        assert!(
            lease
                .reauthenticate_native_amx_prepublication(
                    &token,
                    &fixture.block,
                    &fixture.manifest,
                    &fixture.finality,
                    &reordered,
                )
                .is_err(),
            "equal cardinality cannot replace the exact ordered State projection"
        );
        assert!(
            lease
                .reauthenticate_native_amx_prepublication(
                    &token,
                    &fixture.block,
                    &fixture.manifest,
                    &fixture.finality,
                    &frontiers[..2],
                )
                .is_err(),
            "one missing frontier must refuse the complete publication"
        );
        let mut drifted = frontiers.clone();
        drifted[2].source_count += 1;
        assert!(
            lease
                .reauthenticate_native_amx_prepublication(
                    &token,
                    &fixture.block,
                    &fixture.manifest,
                    &fixture.finality,
                    &drifted,
                )
                .is_err(),
            "later frontier drift must not be hidden by earlier matches"
        );
    }
    assert_eq!(
        snapshot_regular_files_recursively(&fixture.kura.store_root),
        before
    );

    // A second real Kura has the exact same canonical block and participant
    // artifacts. Equality of durable bytes cannot substitute the original owner.
    let foreign = native_amx_publication_capacity_fixture();
    foreign
        .kura
        .store_block(Arc::clone(&fixture.block))
        .unwrap();
    let receipt = foreign
        .kura
        .store_v2_finality_artifact(&fixture.finality)
        .unwrap();
    assert_eq!(receipt.artifact_hash(), HashOf::new(&fixture.finality));
    let foreign_token = foreign
        .kura
        .prepublish_native_amx_participant_application_evidence(&fixture.block, None)
        .unwrap();
    let lease = foreign.kura.try_publication_lease().unwrap();
    assert!(
        lease
            .reauthenticate_native_amx_prepublication(
                &token,
                &fixture.block,
                &fixture.manifest,
                &fixture.finality,
                &frontiers,
            )
            .is_err(),
        "a token from another live Kura must not authorize matching storage"
    );
    lease
        .reauthenticate_native_amx_prepublication(
            &foreign_token,
            &fixture.block,
            &fixture.manifest,
            &fixture.finality,
            &frontiers,
        )
        .expect("foreign Kura may authenticate only its own original token");
}

#[test]
fn native_amx_retained_prepublication_rereads_every_durable_component_before_visibility() {
    for component in ["manifest", "receipt", "latest"] {
        for missing in [true, false] {
            let fixture = native_amx_publication_capacity_fixture();
            fixture
                .kura
                .store_block(Arc::clone(&fixture.block))
                .unwrap();
            let receipt = fixture
                .kura
                .store_v2_finality_artifact(&fixture.finality)
                .unwrap();
            assert_eq!(receipt.artifact_hash(), HashOf::new(&fixture.finality));
            let token = fixture
                .kura
                .prepublish_native_amx_participant_application_evidence(&fixture.block, None)
                .unwrap();
            let frontiers =
                crate::state::State::native_amx_participant_frontier_markers(&fixture.block)
                    .unwrap();
            // Damage the final route: successful earlier readbacks must not
            // permit a partial participant publication to escape.
            let leaf = &fixture.manifest.entries().last().unwrap().leaf;
            let entry = fixture.kura.lane_storage_entry(leaf.lane_id).unwrap();
            let path = match component {
                "manifest" => Kura::native_amx_application_manifest_path_for_entry(
                    &entry,
                    &fixture.kura.store_root,
                    leaf.participant_height,
                ),
                "receipt" => Kura::native_amx_participant_receipt_path_for_entry(
                    &entry,
                    &fixture.kura.store_root,
                    leaf.participant_height,
                ),
                "latest" => Kura::native_amx_participant_receipt_latest_index_path_for_entry(
                    &entry,
                    &fixture.kura.store_root,
                ),
                _ => unreachable!(),
            };
            if missing {
                std::fs::remove_file(&path).unwrap();
            } else {
                std::fs::write(&path, b"corrupt retained participant evidence").unwrap();
            }
            let before = snapshot_regular_files_recursively(&fixture.kura.store_root);
            let lease = fixture.kura.try_publication_lease().unwrap();
            assert!(
                lease
                    .reauthenticate_native_amx_prepublication(
                        &token,
                        &fixture.block,
                        &fixture.manifest,
                        &fixture.finality,
                        &frontiers,
                    )
                    .is_err(),
                "stale token accepted {component}, missing={missing}"
            );
            drop(lease);
            assert_eq!(
                snapshot_regular_files_recursively(&fixture.kura.store_root),
                before,
                "reauthentication cannot silently repair or rewrite {component}"
            );
            assert!(fixture.kura.wsv_checkpoint(1).unwrap().is_none());
            assert!(fixture.kura.commit_manifest(1).unwrap().is_none());
            drop(
                fixture
                    .kura
                    .try_publication_lease()
                    .expect("refusal releases every Kura fence"),
            );
        }
    }
}

#[test]
fn native_amx_retained_prepublication_survives_original_fence_contention() {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, Wake, Waker},
    };
    struct ParticipantWake(std::sync::atomic::AtomicUsize);
    impl Wake for ParticipantWake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let fixture = native_amx_publication_capacity_fixture();
    fixture
        .kura
        .store_block(Arc::clone(&fixture.block))
        .unwrap();
    let receipt = fixture
        .kura
        .store_v2_finality_artifact(&fixture.finality)
        .unwrap();
    assert_eq!(receipt.artifact_hash(), HashOf::new(&fixture.finality));
    let token = fixture
        .kura
        .prepublish_native_amx_participant_application_evidence(&fixture.block, None)
        .unwrap();
    let frontiers =
        crate::state::State::native_amx_participant_frontier_markers(&fixture.block).unwrap();
    let before = snapshot_regular_files_recursively(&fixture.kura.store_root);
    let held = fixture.kura.sidecar_lock.lock();
    let mut wait = match fixture.kura.try_publication_lease() {
        Err(KuraPublicationPreparationError::Busy { field, wait }) => {
            assert_eq!(field, "sidecar_lock");
            wait.wait_for_release()
        }
        _ => panic!("the original sidecar owner must defer publication"),
    };
    let wakes = Arc::new(ParticipantWake(std::sync::atomic::AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&wakes));
    assert_eq!(
        Pin::new(&mut wait).poll(&mut Context::from_waker(&waker)),
        Poll::Pending
    );
    let other = Kura::blank_kura_for_testing();
    drop(other.try_publication_lease().unwrap());
    assert_eq!(wakes.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    drop(held);
    assert_eq!(wakes.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(
        Pin::new(&mut wait).poll(&mut Context::from_waker(&waker)),
        Poll::Ready(())
    );
    fixture
        .kura
        .try_publication_lease()
        .unwrap()
        .reauthenticate_native_amx_prepublication(
            &token,
            &fixture.block,
            &fixture.manifest,
            &fixture.finality,
            &frontiers,
        )
        .expect("same retained token succeeds after its original owner releases");
    assert_eq!(
        snapshot_regular_files_recursively(&fixture.kura.store_root),
        before
    );
}

#[test]
fn native_amx_retained_prepublication_is_reminted_from_durable_evidence_after_restart() {
    let NativeAmxPublicationCapacityFixture {
        _temp_dir,
        kura,
        block,
        manifest,
        finality,
        lane_config,
    } = native_amx_publication_capacity_fixture();
    let (incarnations, activations) = active_fixture_geometry_maps(&kura, &lane_config);
    let network_id = kura.bound_lane_storage_network().unwrap();
    kura.store_block(Arc::clone(&block)).unwrap();
    let receipt = kura.store_v2_finality_artifact(&finality).unwrap();
    assert_eq!(receipt.artifact_hash(), HashOf::new(&finality));
    let original_token = kura
        .prepublish_native_amx_participant_application_evidence(&block, None)
        .unwrap();
    let frontiers = crate::state::State::native_amx_participant_frontier_markers(&block).unwrap();
    assert!(kura.wsv_checkpoint(1).unwrap().is_none());
    drop(kura);

    let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("strict restart recovers completed participant evidence before WSV");
    reopened.bind_lane_storage_network(network_id).unwrap();
    reopened
        .recover_lane_geometry_journal(&lane_config, &incarnations, &activations)
        .unwrap();
    reopened
        .finish_restored_lane_segments_with_geometry(&lane_config)
        .unwrap();
    let before = snapshot_regular_files_recursively(&reopened.store_root);
    assert!(
        reopened
            .try_publication_lease()
            .unwrap()
            .reauthenticate_native_amx_prepublication(
                &original_token,
                &block,
                &manifest,
                &finality,
                &frontiers,
            )
            .is_err(),
        "reopening the same directory cannot revive an old in-memory owner"
    );
    assert_eq!(
        snapshot_regular_files_recursively(&reopened.store_root),
        before
    );
    let recovered_token = reopened
        .prepublish_native_amx_participant_application_evidence(&block, None)
        .expect("reconstruct original-instance custody from the exact durable artifacts");
    reopened
        .try_publication_lease()
        .unwrap()
        .reauthenticate_native_amx_prepublication(
            &recovered_token,
            &block,
            &manifest,
            &finality,
            &frontiers,
        )
        .expect("restart's new owner authenticates the recovered participant evidence");
    assert_eq!(reopened.exact_durable_blocks_count().unwrap(), 1);
    assert!(reopened.wsv_checkpoint(1).unwrap().is_none());
}

#[test]
fn native_amx_empty_prepublication_requires_current_durable_finality() {
    for missing in [true, false] {
        let kura = Kura::blank_kura_for_testing();
        let block = DummyBlocks::new().next_with_results();
        let manifest =
            crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(
                &block,
            )
            .expect("derive actual result-bearing ordinary carrier manifest");
        assert!(manifest.entries().is_empty());
        let finality = v2_finality_artifact_for_block(&block);
        kura.store_block(Arc::clone(&block))
            .expect("persist ordinary carrier");
        let receipt = kura
            .store_v2_finality_artifact(&finality)
            .expect("persist exact signed carrier finality");
        assert_eq!(receipt.artifact_hash(), HashOf::new(&finality));
        let token = kura
            .prepublish_native_amx_participant_application_evidence(&block, None)
            .expect("mint original empty-participant token from real durable evidence");
        let frontiers = crate::state::State::native_amx_participant_frontier_markers(&block)
            .expect("derive ordinary State frontier projection");
        assert!(frontiers.is_empty());
        let reauthenticate = || {
            kura.reauthenticate_native_amx_prepublication(
                &token, &block, &manifest, &finality, &frontiers,
            )
        };
        let original_files = snapshot_regular_files_recursively(&kura.store_root);
        reauthenticate().expect("empty participant obligation still joins its exact carrier");
        kura.try_publication_lease()
            .expect("live wrapper releases all four fences")
            .reauthenticate_native_amx_prepublication(
                &token, &block, &manifest, &finality, &frontiers,
            )
            .expect("held original lease authenticates the empty participant obligation");
        assert_eq!(
            snapshot_regular_files_recursively(&kura.store_root),
            original_files
        );

        let path = kura.v2_finality_artifact_path(finality.height);
        let original_finality = std::fs::read(&path).expect("read exact durable finality bytes");
        if missing {
            std::fs::remove_file(&path).expect("remove durable finality after token capture");
        } else {
            std::fs::write(&path, b"corrupt empty-participant finality")
                .expect("corrupt durable finality after token capture");
        }
        let damaged_files = snapshot_regular_files_recursively(&kura.store_root);
        assert!(
            reauthenticate().is_err(),
            "an empty participant list cannot bypass current durable finality; missing={missing}"
        );
        assert!(
            kura.try_publication_lease()
                .expect("live refusal releases all four fences")
                .reauthenticate_native_amx_prepublication(
                    &token, &block, &manifest, &finality, &frontiers,
                )
                .is_err(),
            "captured token and cached finality cannot replace durable readback; missing={missing}"
        );
        assert_eq!(
            snapshot_regular_files_recursively(&kura.store_root),
            damaged_files,
            "reauthentication must not repair missing or corrupt finality"
        );
        assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
        assert!(kura.wsv_checkpoint(1).unwrap().is_none());
        assert!(kura.commit_manifest(1).unwrap().is_none());

        std::fs::write(&path, original_finality).expect("restore exact original fixture bytes");
        reauthenticate().expect("the same original token works after exact durable restoration");
        assert_eq!(
            snapshot_regular_files_recursively(&kura.store_root),
            original_files
        );
    }
}

#[test]
fn native_amx_live_custody_wrappers_unlock_together_before_callbacks() {
    use std::{future::Future, pin::Pin, sync::atomic::AtomicUsize, task::{Context, Wake, Waker}};
    struct Reenter {
        kura: Arc<Kura>,
        wakes: AtomicUsize,
    }
    impl Wake for Reenter {
        fn wake(self: Arc<Self>) {
            for lock in [&self.kura.prune_lock, &self.kura.canonical_chain_lock,
                         &self.kura.lane_geometry_lock, &self.kura.sidecar_lock] {
                assert!(lock.try_lock_or_wait().is_ok(), "live custody still holds a sibling fence");
            }
            self.wakes.fetch_add(1, Ordering::SeqCst);
        }
    }
    let fixture = native_amx_publication_capacity_fixture();
    fixture.kura.store_block(Arc::clone(&fixture.block)).unwrap();
    let receipt = fixture.kura.store_v2_finality_artifact(&fixture.finality).unwrap();
    let token = fixture.kura.prepublish_native_amx_participant_application_evidence(&fixture.block, None).unwrap();
    let frontiers = crate::state::State::native_amx_participant_frontier_markers(&fixture.block).unwrap();
    let files = snapshot_regular_files_recursively(&fixture.kura.store_root);
    for (archive, invalid) in [(false, false), (false, true), (true, true)] {
        let sidecar = fixture.kura.sidecar_lock.lock();
        let mut wait = fixture.kura.sidecar_lock.try_lock_or_wait().err().unwrap().wait_for_release();
        let initial = sidecar.release_deferred();
        let callback = Arc::new(Reenter { kura: Arc::clone(&fixture.kura), wakes: AtomicUsize::new(0) });
        let waker = Waker::from(Arc::clone(&callback));
        assert!(Pin::new(&mut wait).poll(&mut Context::from_waker(&waker)).is_pending());
        let passed = if archive {
            fixture.kura.authenticate_archive_capture(
                fixture.finality.height_context.network_id.clone(),
                fixture.block.header().height().get(),
                [0; 32], 0, &receipt,
            ).is_ok()
        } else {
            fixture.kura.reauthenticate_native_amx_prepublication(
                &token, &fixture.block, &fixture.manifest, &fixture.finality,
                if invalid { &frontiers[..2] } else { &frontiers },
            ).is_ok()
        };
        assert_eq!(passed, !invalid);
        assert_eq!(callback.wakes.load(Ordering::SeqCst), 1);
        assert!(Pin::new(&mut wait).poll(&mut Context::from_waker(Waker::noop())).is_ready());
        drop(wait);
        drop(initial);
    }
    assert_eq!(snapshot_regular_files_recursively(&fixture.kura.store_root), files);
}
