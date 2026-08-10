#[test]
fn native_amx_receipt_survives_into_final_header_bound_lane_statement() {
    let _guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .expect("nexus status test lock");
    crate::sumeragi::status::set_lane_settlement_commitments(Vec::new());
    crate::sumeragi::status::set_lane_relay_envelopes(Vec::new());

    let paynet = DataSpaceId::new(7);
    let cbuae = DataSpaceId::new(8);
    let chain_id = ChainId::from("native-amx-test-chain");
    let (authority, signer) = gen_account_in("wonderland");
    let authority_domain = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(authority_domain.clone()).build(&authority);
    let (mut world, keypairs) = native_amx_test_world_with_keys();
    world.domains.insert(authority_domain, domain);
    world.accounts.insert(
        authority.clone(),
        iroha_data_model::account::AccountValue::new(
            iroha_data_model::account::AccountDetails::default(),
        ),
    );
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_chain(world, kura, query_handle, chain_id.clone());
    {
        let nexus = state.nexus.get_mut();
        nexus.enabled = true;
        nexus.lane_catalog = LaneCatalog::new(
            nonzero!(4_u32),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: LaneId::new(1),
                    dataspace_id: paynet,
                    alias: "paynet".to_owned(),
                    ..LaneConfig::default()
                },
                LaneConfig {
                    id: LaneId::new(2),
                    dataspace_id: cbuae,
                    alias: "cbuae".to_owned(),
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("lane catalog");
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        nexus.dataspace_catalog = native_amx_test_catalog(paynet, cbuae);
    }
    install_test_lane_manifests(&state);

    let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
    let tx = TransactionBuilder::new_with_time_source(
        state.network_id,
        authority.clone(),
        &time_source,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([
        InstructionBox::from(Register::domain(Domain::new(
            DomainId::try_new("merchant", "paynet").expect("domain id"),
        ))),
        InstructionBox::from(Register::domain(Domain::new(
            DomainId::try_new("treasury", "cbuae").expect("domain id"),
        ))),
    ])
    .sign(signer.private_key());
    let accepted_for_plan = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
    let plan = {
        let view = state.view();
        crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
            &view.nexus,
            &accepted_for_plan,
            view.world(),
            u64::try_from(time_source.get_unix_time().as_millis()).unwrap_or(u64::MAX),
            1,
        )
        .expect("mixed dataspace write targets should build a native AMX plan")
    };
    assert!(matches!(plan, crate::queue::RoutingPlan::NativeAmx(_)));
    let block_height = 1;
    let mut source_id = [0_u8; iroha_crypto::Hash::LENGTH];
    source_id.copy_from_slice(tx.hash().as_ref());
    let receipt = signed_native_amx_receipt(
        source_id,
        tx.hash_as_entrypoint(),
        &plan,
        block_height,
        &keypairs,
    );
    let context = crate::queue::execution_context_for_routing_plan(tx.hash_as_entrypoint(), &plan)
        .with_native_amx_receipt(receipt.clone());
    let mut validator_set = keypairs
        .iter()
        .map(|keypair| PeerId::new(keypair.public_key().clone()))
        .collect::<Vec<_>>();
    validator_set.sort();
    let mut ownership = iroha_data_model::block::consensus::SumeragiLanePayloadOwnership {
        proposal_height: block_height,
        proposal_view: 0,
        lane_id: receipt.lane_id,
        dataspace_id: receipt.dataspace_id,
        lane_incarnation: receipt.lane_incarnation,
        lane_block_height: receipt.lane_block_height,
        lane_block_view: receipt.lane_block_view,
        subject_hash: Hash::new(b"native AMX settlement subject placeholder"),
        qc_mode_tag: LaneRelayEnvelope::lane_qc_mode_tag_for(
            receipt.lane_id,
            receipt.dataspace_id,
            "native-amx-settlement-test",
        ),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![Hash::from(tx.hash_as_entrypoint())],
        previous_lane_block_height: receipt.lane_block_height.saturating_sub(1),
        previous_lane_block_descriptor_hash: Some(Hash::new(
            b"native AMX settlement predecessor descriptor",
        )),
        lane_block_descriptor_hash: Some(Hash::new(
            b"native AMX settlement descriptor placeholder",
        )),
        lane_block_descriptor_validator_count: u32::try_from(validator_set.len())
            .expect("test validator count fits u32"),
        lane_block_descriptor_min_quorum: u32::try_from(
            crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len()),
        )
        .expect("test validator quorum fits u32"),
        lane_block_descriptor_validator_set: validator_set,
        payload_ownership_hash: Hash::new(b"native AMX settlement ownership placeholder"),
        rbc_instance_hash: Hash::new(b"native AMX settlement RBC placeholder"),
    };
    let replay_hashes = ownership
        .compute_replay_hashes()
        .expect("native AMX settlement ownership replay hashes");
    ownership.subject_hash = replay_hashes.subject_hash;
    ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
    ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
    ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
    let execution_context = BlockExecutionContextBundle::new(vec![context])
        .with_lane_payload_ownerships(vec![ownership]);
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    time_handle.advance(Duration::from_millis(1));

    let block = BlockBuilder::new_with_time_source(vec![accepted], time_source)
        .chain(0, state.view().latest_block().as_deref())
        .with_execution_context(Some(execution_context))
        .sign(keypairs[0].private_key())
        .unpack(|_| {});
    assert_eq!(block.header().height().get(), block_height);
    let mut state_block = state.block(block.header());
    let valid_block = block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    assert!(
        valid_block
            .as_ref()
            .entrypoint_results()
            .all(|(_, _, result)| result.0.is_ok()),
        "native AMX transaction should execute successfully: {:?}",
        valid_block
            .as_ref()
            .entrypoint_results()
            .collect::<Vec<_>>()
    );

    let statements = valid_block.as_ref().lane_finality_statements();
    assert_eq!(statements.len(), 1);
    let statement = &statements[0];
    assert_eq!(statement.block_header_hash, valid_block.as_ref().hash());
    let commitment = &statement.settlement_commitment;
    assert_eq!(commitment.tx_count, 1);
    assert_eq!(commitment.native_amx_receipts, vec![receipt]);
    assert_eq!(
        commitment.lane_id,
        plan.coordinator_route().lane_id,
        "lane-finality statement must use the native AMX coordinator lane"
    );
    assert_eq!(
        commitment.dataspace_id,
        plan.coordinator_route().dataspace_id,
        "lane-finality statement must use the native AMX coordinator dataspace"
    );

    let snapshot = crate::sumeragi::status::snapshot();
    assert!(
        snapshot.lane_settlement_commitments.is_empty() && snapshot.lane_relay_envelopes.is_empty(),
        "candidate validation must not publish process-global relay evidence before commit"
    );

    crate::sumeragi::status::set_lane_settlement_commitments(Vec::new());
    crate::sumeragi::status::set_lane_relay_envelopes(Vec::new());
}

#[test]
fn autonomous_anchor_admission_rejects_same_label_different_network() {
    let mut fixture = autonomous_anchor_fixture(None, 0);
    let display_name = fixture.state.chain_id.clone();
    let original_network_id = fixture.state.network_id;
    fixture.state.network_id = deterministic_test_network_id(0x7A);
    assert_eq!(fixture.state.chain_id, display_name);
    assert_ne!(fixture.state.network_id, original_network_id);

    let view = fixture.state.query_view();
    let error = ValidBlock::validate_execution_context_autonomous_lane_payloads(
        &fixture.block,
        &fixture.topology,
        &view,
        &fixture.bundle,
        fixture.profile.clone(),
    )
    .expect_err("the same display label must not authorize another genesis lineage");
    assert!(matches!(
        error,
        BlockValidationError::ExecutionContextInvalid(message)
            if message.contains("autonomous lane payload envelope")
    ));
}

fn seed_domain_name_lease(world: &mut World, owner: &AccountId, domain_id: &DomainId) {
    let selector = crate::sns::selector_for_domain(domain_id).expect("selector");
    let address =
        iroha_data_model::account::AccountAddress::from_account_id(owner).expect("address");
    let record = iroha_data_model::sns::NameRecordV1::new(
        selector.clone(),
        owner.clone(),
        vec![iroha_data_model::sns::NameControllerV1::account(&address)],
        0,
        0,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        Metadata::default(),
    );
    world.smart_contract_state_mut_for_testing().insert(
        crate::sns::record_storage_key(&selector),
        norito::codec::Encode::encode(&record),
    );
}

#[allow(dead_code)]
fn commit_block_at_height(
    state: &State,
    kura: &Arc<Kura>,
    topology: &Topology,
    leader_private: &PrivateKey,
    height: u64,
    prev_hash: Option<HashOf<BlockHeader>>,
    creation_time_ms: u64,
) -> HashOf<BlockHeader> {
    let valid = ValidBlock::new_dummy_and_modify_header(leader_private, |header| {
        header.set_height(NonZeroU64::new(height).expect("non-zero height in commit helper"));
        header.set_prev_block_hash(prev_hash);
        header.creation_time_ms = creation_time_ms;
    });
    let committed = valid.commit_unchecked().unpack(|_| {});
    {
        let mut state_block = state.block(committed.as_ref().header());
        let _ = state_block.apply_without_execution(&committed, topology.as_ref().to_owned());
        state_block.commit().unwrap();
    }
    kura.store_block(committed.clone())
        .expect("store committed block");
    committed.as_ref().hash()
}

#[test]
fn map_overlay_error_labels_amx_budget() {
    let err =
        crate::pipeline::overlay::OverlayBuildError::IvmRun(ivm::VMError::AmxBudgetExceeded {
            dataspace: DataSpaceId::new(5),
            stage: AmxStage::Commit,
            elapsed_ms: 42,
            budget_ms: 30,
        });
    match super::map_overlay_error(&err) {
        TransactionRejectionReason::Validation(iroha_data_model::ValidationFail::NotPermitted(
            message,
        )) => {
            assert!(
                message.contains("AMX_TIMEOUT"),
                "message missing AMX_TIMEOUT label: {message}"
            );
            assert!(
                message.contains("dataspace=5"),
                "message missing dataspace label: {message}"
            );
            assert!(
                message.contains(
                    &iroha_data_model::errors::CanonicalErrorKind::AMX_TIMEOUT_CODE.to_string()
                ),
                "message missing canonical code: {message}"
            );
        }
        other => panic!("unexpected rejection: {other:?}"),
    }
}

#[test]
fn map_overlay_error_labels_amx_violation_variant() {
    let err = crate::pipeline::overlay::OverlayBuildError::AmxBudgetViolation(
        crate::smartcontracts::ivm::host::AmxBudgetViolation {
            dataspace: DataSpaceId::new(7),
            stage: AmxStage::Prepare,
            elapsed_ms: 99,
            budget_ms: 10,
        },
    );
    match super::map_overlay_error(&err) {
        TransactionRejectionReason::Validation(iroha_data_model::ValidationFail::NotPermitted(
            message,
        )) => {
            assert!(
                message.contains("AMX_TIMEOUT"),
                "message missing AMX_TIMEOUT label: {message}"
            );
            assert!(
                message.contains("dataspace=7"),
                "message missing dataspace label: {message}"
            );
            assert!(
                message.contains(
                    &iroha_data_model::errors::CanonicalErrorKind::AMX_TIMEOUT_CODE.to_string()
                ),
                "message missing canonical code: {message}"
            );
        }
        other => panic!("unexpected rejection: {other:?}"),
    }
}

#[test]
pub fn committed_and_valid_block_hashes_are_equal() {
    let peer_key_pair =
        crate::block::checked_keypair_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
    let peer_id = PeerId::new(peer_key_pair.public_key().clone());
    let topology = Topology::new(vec![peer_id]);
    let valid_block = ValidBlock::new_dummy(peer_key_pair.private_key());
    let committed_block = valid_block
        .clone()
        .commit(&topology)
        .unpack(|_| {})
        .unwrap();

    assert_eq!(valid_block.as_ref().hash(), committed_block.as_ref().hash())
}

#[test]
fn merkle_root_matches_header() {
    use std::borrow::Cow;
    let network_id = deterministic_test_network_id(0x0A);
    let (alice_id, alice_keypair) = gen_account_in("wonderland");

    let log = Log::new(Level::INFO, "test".to_string());

    let tx1 = Box::new(
        TransactionBuilder::new(
            network_id,
            alice_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([log.clone()])
        .sign(alice_keypair.private_key()),
    );
    let tx1: &'static SignedTransaction = Box::leak(tx1);
    let tx1 = AcceptedTransaction::new_unchecked(Cow::Borrowed(tx1));

    let tx2 = Box::new(
        TransactionBuilder::new(
            network_id,
            alice_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([log])
        .sign(alice_keypair.private_key()),
    );
    let tx2: &'static SignedTransaction = Box::leak(tx2);
    let tx2 = AcceptedTransaction::new_unchecked(Cow::Borrowed(tx2));

    let block = BlockBuilder::new(vec![tx1, tx2])
        .chain(0, None)
        .sign(alice_keypair.private_key())
        .unpack(|_| {});

    let block: Box<SignedBlock> = Box::new(block.into());
    let mut tree: Box<MerkleTree<TransactionEntrypoint>> = Box::default();
    for tx in block.external_transactions() {
        tree.add(tx.hash_as_entrypoint());
    }

    assert_eq!(tree.root(), block.header().merkle_root());
}

#[test]
fn entrypoint_merkle_bottom_up_matches_incremental_root_shapes() {
    fn sample_leaf(idx: u8) -> HashOf<TransactionEntrypoint> {
        let mut bytes = [0_u8; Hash::LENGTH];
        bytes[0] = idx;
        bytes[Hash::LENGTH - 1] = idx.wrapping_mul(17);
        HashOf::from_untyped_unchecked(Hash::prehashed(bytes))
    }

    fn incremental_root(
        leaves: &[HashOf<TransactionEntrypoint>],
    ) -> Option<HashOf<MerkleTree<TransactionEntrypoint>>> {
        let mut tree = MerkleTree::default();
        for leaf in leaves {
            tree.add(*leaf);
        }
        tree.root()
    }

    fn bottom_up_root(
        leaves: Vec<HashOf<TransactionEntrypoint>>,
    ) -> Option<HashOf<MerkleTree<TransactionEntrypoint>>> {
        let tree = MerkleTree::from_typed_leaves_parallel(leaves);
        tree.root()
    }

    for count in [1_usize, 2, 3, 4, 5, 8] {
        let leaves = (0..count)
            .map(|idx| sample_leaf(u8::try_from(idx + 1).expect("small test index")))
            .collect::<Vec<_>>();
        assert_eq!(
            bottom_up_root(leaves.clone()),
            incremental_root(&leaves),
            "bottom-up Merkle root must match incremental insertion for {count} leaves"
        );
    }
}

#[test]
fn lane_relay_helper_emits_pending_relay_and_rbc_bytes() {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        block::consensus::{LaneBlockCommitment, LaneSettlementReceipt},
        da::commitment::DaCommitmentBundle,
        nexus::{DataSpaceId, LaneId},
    };

    let da_hash: Option<HashOf<DaCommitmentBundle>> = Some(HashOf::from_untyped_unchecked(
        Hash::prehashed([0xAB; Hash::LENGTH]),
    ));
    let mut block_header = BlockHeader::new(
        core::num::NonZeroU64::new(5).expect("non-zero height"),
        None,
        None,
        None,
        1_700_000_000_000,
        0,
    );
    block_header.set_da_commitments_hash(da_hash);

    let lane_id = LaneId::new(2);
    let dataspace_id = DataSpaceId::new(1);
    let receipt = LaneSettlementReceipt {
        source_id: [0x11; 32],
        local_amount: "0.00001".parse().expect("valid settlement quantity"),
        xor_due: "0.00002".parse().expect("valid settlement quantity"),
        xor_after_haircut: "0.000018".parse().expect("valid settlement quantity"),
        xor_variance: "0.000002".parse().expect("valid settlement quantity"),
        timestamp_ms: 1_700_000_100,
    };
    let settlement = LaneBlockCommitment {
        block_height: 3,
        lane_id,
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id,
        tx_count: 1,
        total_local_amount: receipt.local_amount.clone(),
        total_xor_due: receipt.xor_due.clone(),
        total_xor_after_haircut: receipt.xor_after_haircut.clone(),
        total_xor_variance: receipt.xor_variance.clone(),
        swap_metadata: None,
        receipts: vec![receipt],
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };

    let mut lane_summaries = BTreeMap::new();
    lane_summaries.insert(
        lane_id,
        LaneSummary {
            rbc_bytes_total: 2048,
            ..LaneSummary::default()
        },
    );

    let descriptor_hash = Hash::new(b"lane-relay-helper-descriptor");
    let lane_payload_coordinates = BTreeMap::from([(
        (lane_id, dataspace_id),
        LanePayloadCoordinate {
            lane_incarnation: settlement.lane_incarnation,
            lane_block_height: settlement.block_height,
            lane_block_descriptor_hash: descriptor_hash,
        },
    )]);

    let missing_coordinate = lane_relay_envelopes_for_block(
        &block_header,
        da_hash,
        std::slice::from_ref(&settlement),
        &lane_summaries,
        &BTreeMap::new(),
    )
    .expect_err("settled lanes must have exact payload ownership coordinates");
    assert!(matches!(
        missing_coordinate,
        BlockValidationError::ExecutionContextInvalid(_)
    ));

    let relays = lane_relay_envelopes_for_block(
        &block_header,
        da_hash,
        std::slice::from_ref(&settlement),
        &lane_summaries,
        &lane_payload_coordinates,
    )
    .expect("exact lane payload coordinates build a relay");
    assert_eq!(relays.len(), 1);
    let envelope = &relays[0];
    assert!(
        envelope.finality_authority.is_none(),
        "execution-stage relay must wait for genuine global finality"
    );
    assert_eq!(envelope.rbc_bytes_total, 2048);
    assert_eq!(envelope.block_height, 3);
    assert_eq!(envelope.block_header.height().get(), 5);
    assert_eq!(envelope.lane_block_descriptor_hash, Some(descriptor_hash));
    envelope.verify().expect("envelope should validate");
}

#[test]
fn lane_relay_envelopes_attach_manifest_roots() {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        block::consensus::{LaneBlockCommitment, LaneSettlementReceipt},
        da::commitment::DaCommitmentBundle,
        nexus::{DataSpaceId, LaneId},
    };

    let da_hash: Option<HashOf<DaCommitmentBundle>> = Some(HashOf::from_untyped_unchecked(
        Hash::prehashed([0xAB; Hash::LENGTH]),
    ));
    let mut block_header = BlockHeader::new(
        core::num::NonZeroU64::new(5).expect("non-zero height"),
        None,
        None,
        None,
        1_700_000_000_000,
        0,
    );
    block_header.set_da_commitments_hash(da_hash);

    let lane_id = LaneId::new(2);
    let dataspace_id = DataSpaceId::new(1);
    let receipt = LaneSettlementReceipt {
        source_id: [0x11; 32],
        local_amount: "0.00001".parse().expect("valid settlement quantity"),
        xor_due: "0.00002".parse().expect("valid settlement quantity"),
        xor_after_haircut: "0.000018".parse().expect("valid settlement quantity"),
        xor_variance: "0.000002".parse().expect("valid settlement quantity"),
        timestamp_ms: 1_700_000_100,
    };
    let settlement = LaneBlockCommitment {
        block_height: 3,
        lane_id,
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id,
        tx_count: 1,
        total_local_amount: receipt.local_amount.clone(),
        total_xor_due: receipt.xor_due.clone(),
        total_xor_after_haircut: receipt.xor_after_haircut.clone(),
        total_xor_variance: receipt.xor_variance.clone(),
        swap_metadata: None,
        receipts: vec![receipt],
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };

    let mut lane_summaries = BTreeMap::new();
    lane_summaries.insert(
        lane_id,
        LaneSummary {
            rbc_bytes_total: 512,
            ..LaneSummary::default()
        },
    );

    let lane_payload_coordinates = BTreeMap::from([(
        (lane_id, dataspace_id),
        LanePayloadCoordinate {
            lane_incarnation: settlement.lane_incarnation,
            lane_block_height: settlement.block_height,
            lane_block_descriptor_hash: Hash::new(b"manifest-relay-descriptor"),
        },
    )]);

    let mut envelopes = lane_relay_envelopes_for_block(
        &block_header,
        da_hash,
        std::slice::from_ref(&settlement),
        &lane_summaries,
        &lane_payload_coordinates,
    )
    .expect("exact lane payload coordinates build a relay");
    let manifest_root = [0x44; 32];
    let manifest_roots: BTreeMap<DataSpaceId, [u8; 32]> =
        core::iter::once((dataspace_id, manifest_root)).collect();
    attach_manifest_roots_to_relays(&mut envelopes, &manifest_roots);

    assert_eq!(envelopes.len(), 1);
    envelopes[0].fastpq_proof = Some(iroha_data_model::nexus::LaneFastpqProofMaterial {
        proof_digest: Hash::new(b"test-fastpq-proof"),
        verified_at_height: envelopes[0].block_header.height().get(),
    });
    assert_eq!(envelopes[0].manifest_root, Some(manifest_root));
    assert!(envelopes[0].fastpq_proof.is_some());
    envelopes[0]
        .validate_fastpq_proof_metadata()
        .expect("FastPQ proof material must validate");
}

#[test]
fn dag_fingerprint_stability_smoke() {
    // Build a small world and a block with two independent txs to exercise access-set derivation
    let (alice_id, alice_keypair) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, bob_keypair) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId =
        DomainId::try_new("wonderland", "universal").expect("wonderland domain");
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let ad: AssetDefinition = {
        let __asset_definition_id =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "coin".parse().unwrap(),
            );
        AssetDefinition::new(
            __asset_definition_id.clone(),
            "coin".to_owned(),
            NumericSpec::default(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
    }
    .build(&alice_id);
    let acc_a = Account::new(alice_id.clone()).build(&alice_id);
    let acc_b = Account::new(bob_id.clone()).build(&alice_id);
    let world = crate::state::World::with([domain], [acc_a, acc_b], [ad]);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new(world, kura, query);

    let rose: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
    let a_coin = AssetId::of(rose.clone(), alice_id.clone());
    let tx1 = TransactionBuilder::new(
        state.network_id,
        alice_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Mint::asset_quantity(5_u32, a_coin.clone())])
    .sign(alice_keypair.private_key());
    let tx2 = TransactionBuilder::new(
        state.network_id,
        bob_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([SetKeyValue::account(
        bob_id.clone(),
        "k".parse().unwrap(),
        iroha_primitives::json::Json::new("v"),
    )])
    .sign(bob_keypair.private_key());
    let acc: Vec<_> = vec![tx1, tx2]
        .into_iter()
        .map(|t| crate::tx::AcceptedTransaction::new_unchecked(Cow::Owned(t)))
        .collect();

    // Run twice and ensure both runs succeed (determinism covered by other tests);
    // pipeline persistence is best-effort in tests without a store dir.
    let new_block = BlockBuilder::new(acc.clone())
        .chain(0, None)
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
        .unpack(|_| {});
    assert!(
        new_block
            .execution_context
            .as_ref()
            .and_then(|context| context.lane_payload_ownerships.first())
            .is_some_and(is_default_test_execution_context_ownership),
        "the state-free block builder must mark its lane ownership as validation-only"
    );
    let mut sb = state.block(new_block.header());
    let vb = ValidBlock::validate_unchecked(new_block.into(), &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let _ = sb.apply_without_execution(&cb, Vec::new());
    drop(sb);

    let new_block2 = BlockBuilder::new(acc)
        .chain(0, None)
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
        .unpack(|_| {});
    let mut sb2 = state.block(new_block2.header());
    let vb2 = ValidBlock::validate_unchecked(new_block2.into(), &mut sb2).unpack(|_| {});
    let cb2 = vb2.commit_unchecked().unpack(|_| {});
    let _ = sb2.apply_without_execution(&cb2, Vec::new());
}
