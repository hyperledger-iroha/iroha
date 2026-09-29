fn source_transfer_replay_key_for_tests(
    issuer_nonce: &AxtAnchoredSpendReplayKeyV1,
) -> AxtSourceTransferReplayKeyV1 {
    AxtSourceTransferReplayKeyV1 {
        network_id: issuer_nonce.issuer_context.network_id,
        dataspace_id: issuer_nonce.issuer_context.asset_dsid,
        lane_id: LaneId::SINGLE,
        block_header_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"AXT source transfer replay test block",
        )),
        source_tx_index: 1,
        transcript_index: 2,
        delta_index: 3,
    }
}

state_test! { sync axt_policy_refresh_clears_stale_entries_when_snapshot_missing
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query_handle);
    let dsid = DataSpaceId::new(31);
    let_row! { policy = AxtPolicyEntry { manifest_root: [0x77; 32], target_lane: LaneId::new(2), active_handle_era: 1, next_handle_counter: 1, current_slot: 1, } };
    state.set_axt_policy(dsid, policy);
    let snapshot = state.refresh_axt_policies_from_directory();
    assert!(
        snapshot.is_none(),
        "no snapshot should be derived without manifests"
    );
    let view = state.world.axt_policies.view();
    assert!(
        view.get(&dsid).is_none(),
        "stale policy entries must be cleared"
    );
}
state_test! { sync state_block_axt_policy_snapshot_reads_block_scope
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query_handle);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let dsid = DataSpaceId::new(13);
    let_row! { entry = AxtPolicyEntry { manifest_root: [0x66; 32], target_lane: LaneId::new(2), active_handle_era: 5, next_handle_counter: 4, current_slot: 99, } };
    {
        let_row! { lane_catalog = LaneCatalog::new( nonzero!(3_u32), vec![LaneConfig { id: entry.target_lane, dataspace_id: dsid, alias: "block-scope-axt".into(), ..LaneConfig::default() }], ) .expect("block-scope AXT lane catalog") };
        configure_axt_fixture_lane_catalog(&mut state, lane_catalog);
    }
    let mut block = state.block(header);
    block.world.axt_policies.insert(dsid, entry);
    let expected_slot = block.block_hashes().len() as u64;
    let snapshot = block.axt_policy_snapshot();
    let_row! { binding = snapshot .entries .iter() .find(|binding| binding.dsid == dsid) .expect("policy from block scope available") };
    assert_eq!(binding.policy.manifest_root, entry.manifest_root);
    assert_eq!(binding.policy.target_lane, entry.target_lane);
    assert_eq!(binding.policy.active_handle_era, entry.active_handle_era);
    assert_eq!(
        binding.policy.next_handle_counter,
        entry.next_handle_counter
    );
    assert_eq!(binding.policy.current_slot, expected_slot);
    let expected_version = AxtPolicySnapshot::compute_version(&snapshot.entries);
    assert_eq!(snapshot.version, expected_version);
}
state_test! { sync axt_replay_ledger_overlay_applies
    let dsid = DataSpaceId::new(41);
    let lane = LaneId::new(0);
    let_row! { lane_catalog = LaneCatalog::new( nonzero!(1_u32), vec![public_lane!(lane, dsid, "primary".to_owned())], ) .expect("lane catalog") };
    let_row! { mut nexus = iroha_config::parameters::actual::Nexus { lane_catalog: lane_catalog.clone(), lane_config: iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog), dataspace_catalog: dataspace_catalog_for_lane_catalog(&lane_catalog), routing_policy: LaneRoutingPolicy { default_lane: lane, default_dataspace: dsid, ..Default::default() }, ..Default::default() } };
    nexus.axt.slot_length_ms = NonZeroU64::new(1).expect("slot length");
    nexus.axt.replay_retention_slots = NonZeroU64::new(2).expect("retention");
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_with_nexus_for_testing(World::new(), nexus, query_handle);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 1, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_lane_id = Some(lane);
    let key = AxtHandleReplayKey::from_parts(
        dsid,
        axt_replay_incarnation_for_test(0xAA),
        [0xAA; 32],
        3,
        7,
        lane,
    );
    let_row! { record = axt_replay_record_for_key(&key, 1, 4) };
    stx.world.axt_replay_ledger.insert(key, record.clone());
    stx.apply();
    assert_eq!(
        block.world.axt_replay_ledger.get(&key).cloned(),
        Some(record)
    );
}
state_test! { sync anchored_axt_spend_nonce_overlay_rolls_back_or_applies_atomically
    let state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 1, 0));
    let key = AxtAnchoredSpendReplayKeyV1 {
        issuer_context: AxtHandleIssuerContextV1::default(),
        nonce: AxtSpendNonceV1::try_new([0xD3; 32]).expect("nonzero nonce"),
    };
    {
        let mut discarded = block.transaction();
        discarded.world.axt_spend_nonce_ledger.insert(key, 1);
    }
    assert!(block.world.axt_spend_nonce_ledger.get(&key).is_none());
    {
        let mut applied = block.transaction();
        applied.world.axt_spend_nonce_ledger.insert(key, 1);
        applied.apply();
    }
    assert_eq!(block.world.axt_spend_nonce_ledger.get(&key), Some(&1));
}
state_test! { sync anchored_axt_source_transfer_replay_is_cross_envelope_and_atomic
    let state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 1, 0));
    let first_nonce = AxtAnchoredSpendReplayKeyV1 {
        issuer_context: AxtHandleIssuerContextV1::default(),
        nonce: AxtSpendNonceV1::try_new([0xD4; 32]).expect("first nonce"),
    };
    let source = source_transfer_replay_key_for_tests(&first_nonce);
    {
        let mut discarded = block.transaction();
        discarded
            .world
            .reserve_verified_axt_spend_replay(first_nonce, source, 1)
            .expect("reserve in discarded transaction");
    }
    assert!(block.world.axt_spend_nonce_ledger.get(&first_nonce).is_none());
    assert!(block.world.axt_source_transfer_replay_ledger.get(&source).is_none());

    {
        let mut applied = block.transaction();
        applied
            .world
            .reserve_verified_axt_spend_replay(first_nonce, source, 1)
            .expect("reserve first envelope");
        applied.apply();
    }
    assert_eq!(block.world.axt_spend_nonce_ledger.get(&first_nonce), Some(&1));
    assert_eq!(
        block.world.axt_source_transfer_replay_ledger.get(&source),
        Some(&AxtSourceTransferReplayRecordV1 { issuer_nonce: first_nonce, consumed_slot: 1 })
    );

    let second_nonce = AxtAnchoredSpendReplayKeyV1 {
        nonce: AxtSpendNonceV1::try_new([0xD5; 32]).expect("second nonce"),
        ..first_nonce
    };
    {
        let mut invalid = block.transaction();
        assert_eq!(
            invalid.world.reserve_verified_axt_spend_replay(
                second_nonce,
                AxtSourceTransferReplayKeyV1 { source_tx_index: 65_536, ..source },
                2,
            ),
            Err(AxtSpendReplayReservationErrorV1::InvalidSourceTransfer),
        );
        assert_eq!(
            invalid.world.reserve_verified_axt_spend_replay(
                second_nonce,
                AxtSourceTransferReplayKeyV1 { dataspace_id: DataSpaceId::new(99), ..source },
                2,
            ),
            Err(AxtSpendReplayReservationErrorV1::IdentityMismatch),
        );
        assert_eq!(
            invalid.world.reserve_verified_axt_spend_replay(second_nonce, source, 0),
            Err(AxtSpendReplayReservationErrorV1::ZeroSlot),
        );
        assert!(invalid.world.axt_spend_nonce_ledger.get(&second_nonce).is_none());
    }
    {
        let mut second = block.transaction();
        assert_eq!(
            second.world.reserve_verified_axt_spend_replay(second_nonce, source, 2),
            Err(AxtSpendReplayReservationErrorV1::SourceTransferConsumed),
        );
        assert!(second.world.axt_spend_nonce_ledger.get(&second_nonce).is_none());
    }
    let different_source = AxtSourceTransferReplayKeyV1 { delta_index: 4, ..source };
    {
        let mut second = block.transaction();
        assert_eq!(
            second.world.reserve_verified_axt_spend_replay(first_nonce, different_source, 2),
            Err(AxtSpendReplayReservationErrorV1::IssuerNonceConsumed),
        );
        assert!(second.world.axt_source_transfer_replay_ledger.get(&different_source).is_none());
    }
    assert!(block.world.axt_spend_nonce_ledger.get(&second_nonce).is_none());
    assert!(block.world.axt_source_transfer_replay_ledger.get(&different_source).is_none());
}
state_test! { sync ordinary_block_seals_axt_replay_pruning_before_atomic_commit
    let dsid = DataSpaceId::new(42);
    let lane = LaneId::new(0);
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.axt.slot_length_ms = NonZeroU64::new(1).expect("slot length");
    nexus.axt.replay_retention_slots = NonZeroU64::new(2).expect("retention");
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query_handle);
    state
        .set_nexus(nexus)
        .expect("apply Nexus config for replay ledger pruning test");
    state.seed_genesis_for_testing().expect("publish actual genesis before replay pruning");
    let key = AxtHandleReplayKey::from_parts(
        dsid,
        axt_replay_incarnation_for_test(0xAB),
        [0xAB; 32],
        3,
        7,
        lane,
    );
    let_row! { stale = axt_replay_record_for_key(&key, 1, 2) };
    {
        let mut block = state.world.axt_replay_ledger.block();
        block.insert(key, stale.clone());
        block.commit();
    }
    let keypair = crate::state::checked_keypair();
    let_row! { signed: SignedBlock = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, state.view().latest_block().as_deref()) .sign(keypair.private_key()) .unpack(|_| {}) .into() };
    assert!(
        signed.axt_envelopes().is_none(),
        "test block must not carry AXT envelopes"
    );
    let mut state_block = state.block(signed.header());
    let valid = ValidBlock::validate_unchecked(signed, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let mut staged_snapshot = None;
    state.commit_executed_block_with_precommit_for_testing(state_block, committed, |staged| {
        assert_eq!(
            state.world.axt_replay_ledger.view().get(&key).cloned(),
            Some(stale),
            "preparing the sealed overlay must not mutate committed replay state"
        );
        assert!(staged.world.axt_replay_ledger.get(&key).is_none(),
            "the final publication seal includes deterministic replay pruning");
        let bytes = crate::snapshot::canonical_staged_state_snapshot_bytes(staged);
        let hash = crate::snapshot::canonical_staged_state_snapshot_hash(staged);
        assert_eq!(hash, iroha_crypto::Hash::new(&bytes),
            "staged checkpoint streaming hash matches its canonical bytes");
        staged_snapshot = Some((bytes, hash));
    }).expect("publish actual outputs and commit deferred replay pruning");
    let (staged_bytes, staged_hash) = staged_snapshot.expect("authorized precommit observation");
    assert!(
        state.world.axt_replay_ledger.view().get(&key).is_none(),
        "ordinary block commit should prune expired AXT replay entries"
    );
    let committed_bytes = crate::snapshot::canonical_state_snapshot_bytes(&state);
    let committed_hash = crate::snapshot::canonical_state_snapshot_hash(&state).expect("stable valid fixture snapshot");
    assert_eq!(
        committed_hash,
        iroha_crypto::Hash::new(&committed_bytes),
        "committed checkpoint streaming hash must match its canonical bytes"
    );
    assert!(
        staged_bytes == committed_bytes && staged_hash == committed_hash,
        "pre-WSV checkpoint must project commit-time replay pruning: \
         staged_len={}, committed_len={}, staged_hash={staged_hash}, \
         committed_hash={committed_hash}",
        staged_bytes.len(),
        committed_bytes.len(),
    );
}

state_test! { sync committed_storage_projections_omit_absent_and_empty_changes
    let state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 1, 0));
    assert!(block.json_serialize_committed_axt_replay_ledger().is_none());
    assert!(block.json_serialize_committed_smart_contract_state().is_none());
    block.stage_da_pin_intent_bundle(1, Vec::new()).unwrap();
    assert!(block.json_serialize_committed_smart_contract_state().is_none());
}
state_test! { sync staged_checkpoint_projects_deferred_da_quota_without_applying_it
    let owner_keypair = crate::state::checked_keypair();
    let owner_id = AccountId::new(owner_keypair.public_key().clone());
    let mut world = World::new();
    world.accounts.insert(
        owner_id,
        iroha_data_model::account::AccountValue::new(
            iroha_data_model::account::AccountDetails::default(),
        ),
    );
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    state.seed_genesis_for_testing().expect("publish genesis before ordinary DA carrier");
    let authorization = crate::da::signed_test_ingest_authorization(
        *state.network_id_ref(), &owner_keypair, LaneId::SINGLE, 1, 0, 1,
    );
    let intent = crate::da::signed_test_pin_intent(
        authorization,
        &owner_keypair,
        StorageTicketId::new([0xA4; 32]),
        ManifestDigest::new([0xB5; 32]),
        None,
    );
    let signer = crate::state::checked_keypair();
    let_row! { signed: SignedBlock = BlockBuilder::new(vec![dummy_accepted_transaction()])
        .chain(0, state.view().latest_block().as_deref())
        .with_da_pin_intents(Some(DaPinIntentBundle::new(vec![intent])))
        .sign(signer.private_key()).unpack(|_| {}).into() };
    let mut block = state.block(signed.header());
    let valid = ValidBlock::validate_unchecked(signed, &mut block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let mut observed = None;
    state.commit_executed_block_with_precommit_for_testing(block, committed, |block| {
    let writes = block.pending_da_pin_intents.as_ref()
        .expect("real block application stages its quota bundle").quota_writes.clone();
    assert!(!writes.is_empty(), "signed nonempty DA bundle must charge quota");
    for key in writes.keys() {
        assert!(block.world.smart_contract_state.get(key).is_none(),
            "quota writes remain deferred until commit");
    }
    let before_storage = norito::json::to_json(&block.world.smart_contract_state)
        .expect("unprojected contract storage");
    let projected_storage = block.json_serialize_committed_smart_contract_state()
        .expect("pending quota charges require an exact projection");
    let staged_bytes = crate::snapshot::canonical_staged_state_snapshot_bytes(block);
    let staged_hash = crate::snapshot::canonical_staged_state_snapshot_hash(block);
    assert_eq!(staged_hash, Hash::new(&staged_bytes));
    assert_eq!(norito::json::to_json(&block.world.smart_contract_state)
        .expect("unchanged contract storage"), before_storage);
        observed = Some((writes, projected_storage, staged_bytes, staged_hash));
    }).expect("ordinary DA block commits its actual outputs and deferred quota");
    let (writes, projected_storage, staged_bytes, staged_hash) = observed
        .expect("authorized precommit quota observation");
    assert_eq!(projected_storage, norito::json::to_json(&state.world.smart_contract_state)
        .expect("committed contract storage including exact undo"));
    for (key, value) in &writes {
        assert_eq!(state.world.smart_contract_state.view().get(key), Some(value));
    }
    let committed_bytes = crate::snapshot::canonical_state_snapshot_bytes(&state);
    let committed_hash = crate::snapshot::canonical_state_snapshot_hash(&state).expect("stable valid fixture snapshot");
    assert_eq!(committed_hash, Hash::new(&committed_bytes));
    assert!(staged_bytes == committed_bytes && staged_hash == committed_hash,
        "DA quota checkpoint projection differs from committed WSV: \
         staged_hash={staged_hash}, committed_hash={committed_hash}");
}

state_test! { sync axt_slot_uses_authenticated_time_for_hash_only_snapshot_parent
    fn hash_only_state() -> State {
        let state = blank_state();
        seed_committed_height_for_state_test(&state, 5);
        state
    }

    let unavailable = hash_only_state();
    assert!(unavailable.latest_block_header_fast().is_none());
    let unavailable_view = unavailable.view();
    assert_eq!(
        crate::smartcontracts::ivm::host::current_axt_slot_for_state(&unavailable_view),
        None,
        "a non-genesis hash-only view without authenticated time must fail closed"
    );
    assert!(matches!(
        crate::smartcontracts::ivm::host::CoreHost::from_state(
            ALICE_ID.clone(),
            &unavailable
        ),
        Err(crate::smartcontracts::ivm::host::CoreHostStateError::AxtPolicySnapshot(
            iroha_data_model::nexus::AxtPolicySnapshotValidationError::AuthenticatedLedgerTimeUnavailable
        ))
    ));
    drop(unavailable_view);

    let mut anchored = hash_only_state();
    anchored.nexus.get_mut().axt.slot_length_ms = nonzero!(10_u64);
    let parameters = crate::kagemusha_v1_test_fixtures::genesis_context_parameters();
    let mut mint_finality_voters = (1_u8..=4)
        .map(|seed| {
            let key_pair = iroha_crypto::KeyPair::try_from_seed(
                vec![seed; 32],
                iroha_crypto::Algorithm::BlsNormal,
            )
            .expect("derive deterministic snapshot mint-finality validator");
            iroha_data_model::block::consensus_v2::ValidatorPower {
                validator: iroha_model_base::peer::PeerId::new(key_pair.public_key().clone()),
                power: 1,
            }
        })
        .collect::<Vec<_>>();
    mint_finality_voters.sort_by(|left, right| left.validator.cmp(&right.validator));
    let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
        crate::kagemusha_v1_test_fixtures::mint_finality_genesis_authorization(anchored.network_id, 6, &mint_finality_voters);
    let snapshot_block_hash = anchored
        .latest_block_hash_fast()
        .expect("hash-only fixture has a committed tip");
    anchored.set_authenticated_snapshot_v2_bootstrap_for_testing(SnapshotV2BootstrapRecord {
        version: SnapshotV2BootstrapRecord::VERSION,
        context: HeightContext {
            network_id: anchored.network_id,
            protocol_version: PROTOCOL_VERSION,
            height: 6,
            epoch: 0,
            epoch_end_height: 6,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: Some(
                iroha_data_model::block::consensus_v2::SnapshotBootstrapAnchor {
                    snapshot_height: 5,
                    snapshot_block_hash,
                    snapshot_block_creation_time_ms: 10_000,
                    snapshot_state_hash: Hash::new(b"hash-only-axt-time"),
                },
            ),
            roster: Vec::new(),
            quorum: DualQuorum {
                min_signers: 0,
                total_power: 0,
            },
            kagemusha_mint_finality_authorization,
            kagemusha_mint_finality_authority,
            nexus_amx_context_hash: Hash::prehashed(parameters.nexus_amx_context_hash),
            execution_policy_hash: Hash::prehashed(parameters.execution_policy_hash),
            da_layout: parameters.da_layout,
            leader_seed: [0; 32],
        },
        validator_set_pops: Vec::new(),
    });
    assert!(
        anchored
            .install_provisional_empty_lane_manifests_for_emergency_fast_pre_auth()
            .is_err(),
        "authenticated snapshot State must refuse the pre-authentication manifest installer"
    );
    let stale_prefix_header = BlockHeader::new(nonzero!(1_u64), None, None, 99, 0);
    anchored.update_latest_block_header_cache_for_tests(stale_prefix_header);
    assert_eq!(anchored.latest_block_creation_time_ms_fast(), Some(10_000));
    let anchored_view = anchored.view();
    assert_eq!(anchored_view.query_ledger_time_ms(), 10_000);
    assert_eq!(anchored_view.authenticated_query_ledger_time_ms(), Some(10_000));
    assert_eq!(
        crate::smartcontracts::ivm::host::current_axt_slot_for_state(&anchored_view),
        Some(1_000),
        "AXT expiry must use the authenticated tip anchor, never height 5 or a stale cached prefix header"
    );
}

state_test! { consensus_stack axt_post_validation_envelope_replacement_cannot_publish_world_state
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;

    let mut state = blank_test_state();
    let mut nexus = state.nexus_snapshot();
    nexus.fees.base_fee = Quantity::zero();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    state.set_nexus(nexus).expect("install the fixture fee policy");
    let parent = state
        .seed_genesis_for_testing()
        .expect("publish the genuine predecessor");
    let retained_hash = state.latest_block_hash_fast();
    let transaction = TransactionBuilder::new(
        *state.network_id_ref(),
        SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([iroha_data_model::isi::Log::new(
        iroha_data_model::level::Level::INFO,
        "validate the AXT envelope source".to_owned(),
    )])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let mut signed: SignedBlock = BlockBuilder::new(vec![AcceptedTransaction::new_unchecked(
        Cow::Owned(transaction),
    )])
    .chain(0, Some(&parent))
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
    .unpack(|_| {})
    .into();
    let mut staged = state.block(signed.header());
    ValidBlock::execute_block_outputs_for_test(&mut signed, &mut staged, None)
        .expect("execute and seal the genuine source");
    assert!(signed.output_error(0).is_none());
    let outputs = signed.execution_outputs().to_vec();
    let snapshot = signed.axt_policy_snapshot().cloned().unwrap_or_default();
    let envelope = AxtEnvelopeRecord {
        binding: AxtBinding::new([0xB7; 32]),
        lane: LaneId::SINGLE,
        descriptor: AxtDescriptor {
            dsids: vec![DataSpaceId::UNIVERSAL],
            touches: Vec::new(),
        },
        touches: Vec::new(),
        proofs: Vec::new(),
        spends: Vec::new(),
        commit_height: 2,
    };
    signed
        .set_execution_outputs(
            outputs,
            signed.committed_fragment_count().unwrap_or(0),
            Default::default(),
            vec![envelope],
            snapshot,
            Default::default(),
            Vec::new(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
        .expect("structurally attach a post-validation envelope");
    let committed = ValidBlock::new_unverified_for_tests(signed)
        .commit_unchecked()
        .unpack(|_| {});
    let error = state
        .commit_executed_block_for_testing(staged, committed)
        .expect_err("replacement envelopes cannot reuse the original execution seal");
    assert_eq!(error, "execution output attachment changed after its seal");
    assert_eq!(state.latest_block_hash_fast(), retained_hash);
    assert_eq!(state.committed_height(), 1);
    assert!(state.world.axt_replay_ledger.view().is_empty());
    assert!(state.kura.v2_finality_artifact(2).unwrap().is_none());
}
