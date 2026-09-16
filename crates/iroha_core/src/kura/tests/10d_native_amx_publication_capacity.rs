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
    assert_eq!(
        reopened
            .lane_storage_entries
            .lock()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![LaneId::SINGLE],
        "reservation reads must not publish secondary lane geometry"
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
            kura.install_lane_incarnation_marker_for_test(
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
        None,
        None,
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
    let entrypoints = contexts
        .iter()
        .map(|context| context.entrypoint_hash)
        .collect::<Vec<_>>();
    block.set_execution_context(Some(BlockExecutionContextBundle::new(contexts)));
    block
        .set_transaction_results(
            Vec::new(),
            &entrypoints,
            vec![TransactionResultInner::Ok(DataTriggerSequence::default()); entrypoints.len()],
        )
        .expect("retain all real Native transaction results");
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
        1,
        "the constructor must leave secondary geometry unpublished"
    );
    let requested_incarnations = configured_routes
        .iter()
        .map(|(lane, (_, incarnation))| (*lane, *incarnation))
        .collect::<BTreeMap<_, _>>();
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
            .prepare_native_amx_publication_index(&fixture.block, None, None, false)
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
    fixture
        .kura
        .install_lane_incarnation_marker_for_test(
            &entry,
            payload.origin_proposal.descriptor.lane_incarnation,
            0,
        )
        .expect("activate only the competing route; preserve all Native incarnations");
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

fn native_amx_replay_test_geometry_maps(
    kura: &Kura,
    lane_config: &RuntimeLaneConfig,
) -> (BTreeMap<LaneId, Hash>, BTreeMap<LaneId, u64>) {
    lane_config
        .entries()
        .iter()
        .map(|entry| {
            let (incarnation, activation) = kura
                .active_lane_incarnation_marker(entry)
                .expect("authenticate each original journal-published route");
            ((entry.lane_id, incarnation), (entry.lane_id, activation))
        })
        .unzip()
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
    let (incarnations, activations) = native_amx_replay_test_geometry_maps(&kura, &lane_config);
    kura.store_block(Arc::clone(&block))
        .expect("store real Native carrier");
    let _finality_receipt = kura
        .store_v2_finality_artifact(&finality)
        .expect("persist real finality");
    native_amx_replay_test_complete_publication(&kura, &block, &finality);
    drop(kura);
    let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("cold open a fully completed Native tip");
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
        let (incarnations, activations) = native_amx_replay_test_geometry_maps(&kura, &lane_config);
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
        drop(kura);
        let config = kura_config_for_dir(&_temp_dir, BLOCKS_IN_MEMORY);
        let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("reconstruct all three indexed routes before State geometry");
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
            kura.clear_native_amx_retained_pair_seal_for_test(lane, fault_index == 2)
                .expect("clear exactly one authenticated paired-seal field");
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
