#[test]
#[allow(clippy::too_many_lines)]
fn manager_sponsored_contract_registration_survives_block_and_committed_replay() {
    for parallel_apply in [false, true] {
        let chain_id =
            ChainId::try_from(format!("contract-registrar-manager-block-{parallel_apply}"))
                .expect("canonical contract-deployment test chain id");
        let network_id = deterministic_test_network_id(0x10);
        let leader = crate::block::checked_keypair();
        let (manager, manager_keypair) = gen_account_in("registrar-manager");
        let (authority, authority_keypair) = gen_account_in("builder");
        let (adversary, adversary_keypair) = gen_account_in("adversary");
        let (missing, missing_keypair) = gen_account_in("missing-self-grant");
        let (malformed, malformed_keypair) = gen_account_in("malformed-self-grant");
        let manager_permission: Permission = iroha_executor_data_model::permission::smart_contract::CanManageSmartContractCodeRegistrars.into();
        let permission: Permission =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode
                .into();
        let accepted_hash = Hash::new(b"manager-sponsored builder upload");
        let existing_replay_hash = Hash::new(b"existing authority bootstrap replay");
        let decorated_hash = Hash::new(b"decorated authority bootstrap");
        let missing_hash = Hash::new(b"missing self-grant bootstrap");
        let malformed_hash = Hash::new(b"malformed self-grant bootstrap");
        let genesis_world = || {
            let mut world = World::with([], [Account::new(manager.clone()).build(&manager)], []);
            world.account_permissions.insert(
                manager.clone(),
                std::collections::BTreeSet::from([manager_permission.clone()]),
            );
            world
        };
        let make_bootstrap_transaction =
            |authority: &AccountId,
             keypair: &KeyPair,
             code_hash: Hash,
             decorated: bool,
             grant: &Permission,
             creation_time_ms: u64| {
                let mut account = Account::new(authority.clone());
                if decorated {
                    let mut metadata = Metadata::default();
                    metadata.insert(
                        "bootstrap-note".parse().expect("metadata name"),
                        Json::new("decorated"),
                    );
                    account = account.with_metadata(metadata);
                }
                let instructions: Vec<InstructionBox> = vec![
                    Register::account(account).into(),
                    Grant::account_permission(grant.clone(), authority.clone()).into(),
                    iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk {
                        code_hash,
                        total_size: 1,
                        chunk_index: 0,
                        chunk_count: 1,
                        chunk: vec![0xA5],
                    }
                    .into(),
                ];
                let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
                    &network_id,
                    authority,
                    0,
                    DataSpaceId::UNIVERSAL,
                )
                .expect("bootstrap contract address");
                let mut transaction_metadata = Metadata::default();
                for key in ["gov_contract_address", "contract_address"] {
                    transaction_metadata.insert(
                        key.parse().expect("deployment metadata name"),
                        Json::new(contract_address.to_string()),
                    );
                }
                let (_time_handle, time_source) =
                    TimeSource::new_mock(Duration::from_millis(creation_time_ms));
                TransactionBuilder::new_with_time_source(
                    network_id,
                    authority.clone(),
                    &time_source,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_metadata(transaction_metadata)
                .with_instructions(instructions)
                .sign(keypair.private_key())
            };
        let install_lane_manifest = |state: &State| {
            let status = crate::governance::manifest::LaneManifestStatus {
                lane: LaneId::SINGLE,
                alias: "bootstrap".to_owned(),
                dataspace: DataSpaceId::UNIVERSAL,
                visibility: iroha_data_model::nexus::LaneVisibility::Public,
                storage: iroha_data_model::nexus::LaneStorageProfile::FullReplica,
                governance: None,
                manifest_path: None,
                governance_rules: None,
                privacy_commitments: Vec::new(),
            };
            let registry = std::sync::Arc::new(
                crate::governance::manifest::LaneManifestRegistry::from_statuses(BTreeMap::from([
                    (LaneId::SINGLE, status),
                ])),
            );
            state.install_lane_manifests(&registry);
        };
        // This authorization regression uses the configured zero-fee unit-test schedule.
        // Nonzero-fee registration and upload require funded explicit fee intents for each signer.
        let mut state = State::new_with_chain_and_network_id_for_testing(
            genesis_world(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            chain_id.clone(),
            network_id,
        );
        install_lane_manifest(&state);
        let mut pipeline = state.pipeline.clone();
        pipeline.parallel_overlay = true;
        pipeline.parallel_apply = parallel_apply;
        pipeline.workers = 2;
        state.set_pipeline(pipeline.clone());
        let (_genesis_handle, genesis_time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let genesis_account = AccountId::new(leader.public_key().clone());
        let genesis_transaction = TransactionBuilder::new_genesis_with_time_source(
            genesis_account.clone(),
            &genesis_time_source,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "bootstrap fixture genesis".to_owned(),
        )])
        .sign(leader.private_key());
        let genesis = BlockBuilder::new_with_time_source(
            vec![AcceptedTransaction::new_unchecked(Cow::Owned(
                genesis_transaction,
            ))],
            genesis_time_source,
        )
        .chain(0, None)
        .sign(leader.private_key())
        .unpack(|_| {});
        let mut genesis_state_block = state.block(genesis.header());
        let mut genesis: SignedBlock = genesis.into();
        ValidBlock::execute_block_outputs_for_test(
            &mut genesis,
            &mut genesis_state_block,
            Some(&genesis_account),
        )
        .expect("configured genesis key authorizes actual genesis execution");
        let valid_genesis = ValidBlock::new_unverified_for_tests(genesis);
        let genesis_signed = valid_genesis.as_ref().clone();
        genesis_state_block
            .commit()
            .expect("commit actual configured-key genesis block");
        let committed_genesis = valid_genesis.commit_unchecked().unpack(|_| {});
        let (_registration_handle, registration_time) =
            TimeSource::new_mock(Duration::from_millis(10));
        let registration = TransactionBuilder::new_with_time_source(
            network_id,
            manager.clone(),
            &registration_time,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(vec![
            InstructionBox::from(Register::account(Account::new(authority.clone()))),
            InstructionBox::from(Grant::account_permission(
                permission.clone(),
                authority.clone(),
            )),
        ])
        .sign(manager_keypair.private_key());
        let (_registration_block_handle, registration_block_time) =
            TimeSource::new_mock(Duration::from_millis(20));
        let registration = BlockBuilder::new_with_time_source(
            vec![AcceptedTransaction::new_unchecked(Cow::Owned(registration))],
            registration_block_time,
        )
        .chain(1, Some(&genesis_signed))
        .sign(leader.private_key())
        .unpack(|_| {});
        let mut registration_state_block = state.block(registration.header());
        let valid_registration = registration
            .validate_and_record_transactions(&mut registration_state_block)
            .unpack(|_| {});
        let registration_errors = valid_registration
            .as_ref()
            .output_results()
            .enumerate()
            .filter_map(|(index, result)| result.as_ref().err().map(|reason| (index, reason)))
            .map(|(index, error)| format!("{index}: {error:?}"))
            .collect::<Vec<_>>();
        assert!(
            registration_errors.is_empty(),
            "genesis-seeded manager must sponsor registration and exact registrar grant with parallel_apply={parallel_apply}: {registration_errors:?}"
        );
        registration_state_block
            .world
            .account(&authority)
            .expect("manager registered builder");
        assert!(
            registration_state_block
                .world
                .account_permissions_iter(&authority)
                .expect("registered builder permissions")
                .any(|stored| stored == &permission)
        );
        assert!(
            !registration_state_block
                .world
                .account_permissions_iter(&authority)
                .expect("builder permissions")
                .any(|stored| stored == &manager_permission)
        );
        let registration_signed = valid_registration.as_ref().clone();
        registration_state_block
            .commit()
            .expect("commit manager-sponsored registration");
        let committed_registration = valid_registration.commit_unchecked().unpack(|_| {});
        let (_upload_handle, upload_time) = TimeSource::new_mock(Duration::from_millis(30));
        let accepted = TransactionBuilder::new_with_time_source(
            network_id,
            authority.clone(),
            &upload_time,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([
            iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk {
                code_hash: accepted_hash,
                total_size: 1,
                chunk_index: 0,
                chunk_count: 1,
                chunk: vec![0xA5],
            },
        ])
        .sign(authority_keypair.private_key());
        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(40));
        let deployment = BlockBuilder::new_with_time_source(
            vec![AcceptedTransaction::new_unchecked(Cow::Owned(accepted))],
            block_time_source,
        )
        .chain(2, Some(&registration_signed))
        .sign(leader.private_key())
        .unpack(|_| {});
        assert!(
            deployment.header().height().get() > 1,
            "builder native upload must execute after genesis"
        );
        let mut deployment_state_block = state.block(deployment.header());
        let valid_deployment = deployment
            .validate_and_record_transactions(&mut deployment_state_block)
            .unpack(|_| {});
        let deployment_errors = valid_deployment
            .as_ref()
            .output_results()
            .enumerate()
            .filter_map(|(index, result)| result.as_ref().err().map(|reason| (index, reason)))
            .map(|(index, error)| format!("{index}: {error:?}"))
            .collect::<Vec<_>>();
        assert!(
            deployment_errors.is_empty(),
            "registered builder native upload must succeed with parallel_apply={parallel_apply}: {deployment_errors:?}"
        );
        deployment_state_block
            .world
            .account(&authority)
            .expect("registered builder exists in validated block");
        assert!(
            deployment_state_block
                .world
                .account_permissions_iter(&authority)
                .expect("builder registrar permissions")
                .any(|stored| stored == &permission)
        );
        assert!(
            deployment_state_block
                .world
                .contract_code_upload_progress(&authority, &accepted_hash)
                .is_some()
        );
        let deployment_signed: SignedBlock = valid_deployment.as_ref().clone();
        deployment_state_block
            .commit()
            .expect("commit builder upload block");
        let committed_deployment = valid_deployment.commit_unchecked().unpack(|_| {});
        let existing_replay = make_bootstrap_transaction(
            &authority,
            &authority_keypair,
            existing_replay_hash.clone(),
            false,
            &permission,
            50,
        );
        let decorated = make_bootstrap_transaction(
            &adversary,
            &adversary_keypair,
            decorated_hash.clone(),
            true,
            &permission,
            51,
        );
        let missing_self_grant = make_bootstrap_transaction(
            &missing,
            &missing_keypair,
            missing_hash,
            false,
            &permission,
            52,
        );
        let malformed_permission = Permission::new(
            "CanRegisterSmartContractCode".to_owned(),
            Json::new("not-the-unit-payload"),
        );
        let malformed_self_grant = make_bootstrap_transaction(
            &malformed,
            &malformed_keypair,
            malformed_hash,
            false,
            &malformed_permission,
            53,
        );
        let (_rejected_handle, rejected_time_source) =
            TimeSource::new_mock(Duration::from_millis(60));
        let rejected = BlockBuilder::new_with_time_source(
            vec![
                AcceptedTransaction::new_unchecked(Cow::Owned(existing_replay)),
                AcceptedTransaction::new_unchecked(Cow::Owned(decorated)),
                AcceptedTransaction::new_unchecked(Cow::Owned(missing_self_grant)),
                AcceptedTransaction::new_unchecked(Cow::Owned(malformed_self_grant)),
            ],
            rejected_time_source,
        )
        .chain(3, Some(&deployment_signed))
        .sign(leader.private_key())
        .unpack(|_| {});
        assert!(
            rejected.header().height().get() > 1,
            "adversarial bootstrap cases must execute after genesis"
        );
        let mut rejected_state_block = state.block(rejected.header());
        let valid_rejected = rejected
            .validate_and_record_transactions(&mut rejected_state_block)
            .unpack(|_| {});
        assert_eq!(
            valid_rejected
                .as_ref()
                .output_results()
                .enumerate()
                .filter_map(|(index, result)| result.as_ref().err().map(|reason| (index, reason)))
                .count(),
            4,
            "existing-authority replay, decorated, missing, and malformed self-grants must all reject"
        );
        assert!(rejected_state_block.world.account(&adversary).is_err());
        for (account, hash) in [(&missing, &missing_hash), (&malformed, &malformed_hash)] {
            assert!(
                rejected_state_block.world.account(account).is_err(),
                "rejected self-grant must roll back account registration"
            );
            assert!(
                rejected_state_block
                    .world
                    .contract_code_upload_progress(account, hash)
                    .is_none(),
                "rejected self-grant must not stage contract bytes"
            );
        }
        assert!(
            rejected_state_block
                .world
                .contract_code_upload_progress(&authority, &existing_replay_hash)
                .is_none()
        );
        assert!(
            rejected_state_block
                .world
                .contract_code_upload_progress(&adversary, &decorated_hash)
                .is_none()
        );
        rejected_state_block
            .commit()
            .expect("commit block containing rejected bootstraps");
        let committed_rejected = valid_rejected.commit_unchecked().unpack(|_| {});
        let mut replay_state = State::new_with_chain_and_network_id_for_testing(
            genesis_world(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            chain_id.clone(),
            network_id,
        );
        install_lane_manifest(&replay_state);
        replay_state.set_pipeline(pipeline);
        for committed in [
            &committed_genesis,
            &committed_registration,
            &committed_deployment,
            &committed_rejected,
        ] {
            let mut replay_block = replay_state.block(committed.as_ref().header());
            replay_block
                .apply_fixture_block(
                    committed,
                    Vec::new(),
                    committed
                        .as_ref()
                        .header()
                        .is_genesis()
                        .then_some(&genesis_account),
                )
                .expect("replay actual phases and compare complete authenticated output metadata");
            replay_block
                .commit()
                .expect("committed manager-sponsored registration and upload must replay");
        }
        let replay_view = replay_state.view();
        let replay_world = replay_view.world();
        replay_world
            .account(&authority)
            .expect("registered builder survives committed replay");
        assert!(replay_world.account(&adversary).is_err());
        assert!(
            replay_world
                .account_permissions_iter(&manager)
                .expect("replayed manager permissions")
                .any(|stored| stored == &manager_permission)
        );
        assert!(
            !replay_world
                .account_permissions_iter(&authority)
                .expect("replayed builder permissions")
                .any(|stored| stored == &manager_permission)
        );
        for (account, hash) in [(&missing, &missing_hash), (&malformed, &malformed_hash)] {
            assert!(
                replay_world.account(account).is_err(),
                "replay must preserve rejected self-grant atomicity"
            );
            assert!(
                replay_world
                    .contract_code_upload_progress(account, hash)
                    .is_none()
            );
        }
        assert!(
            replay_world
                .account_permissions_iter(&authority)
                .expect("replayed builder registrar permissions")
                .any(|stored| stored == &permission)
        );
        assert!(
            replay_world
                .contract_code_upload_progress(&authority, &accepted_hash)
                .is_some()
        );
        assert!(
            replay_world
                .contract_code_upload_progress(&authority, &existing_replay_hash)
                .is_none()
        );
        assert!(
            replay_world
                .contract_code_upload_progress(&adversary, &decorated_hash)
                .is_none()
        );
    }
}
#[tokio::test]
async fn genesis_public_key_is_checked() {
    // Predefined world state
    let genesis_correct_key = crate::block::checked_keypair();
    let genesis_wrong_key = crate::block::checked_keypair();
    let genesis_correct_account_id = AccountId::new(genesis_correct_key.public_key().clone());
    let genesis_wrong_account_id = AccountId::new(genesis_wrong_key.public_key().clone());
    let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_correct_account_id);
    let genesis_wrong_account =
        Account::new(genesis_wrong_account_id.clone()).build(&genesis_wrong_account_id);
    let world = World::with([genesis_domain], [genesis_wrong_account], []);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);
    install_test_lane_manifests(&state);
    // Creating an instruction
    let isi = Log::new(
        iroha_data_model::Level::DEBUG,
        "instruction itself doesn't matter here".to_string(),
    );
    // Create genesis transaction
    // Sign with `genesis_wrong_key` as peer which has incorrect genesis key pair
    // Bypass `accept_genesis` check to allow signing with wrong key
    let tx = TransactionBuilder::new_genesis(
        genesis_wrong_account_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([isi])
    .sign(genesis_wrong_key.private_key());
    let tx = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    // Create genesis block
    let transactions = vec![tx];
    let topology =
        crate::sumeragi::network_topology::test_topology_with_keys([&genesis_correct_key]);
    let unverified_block = BlockBuilder::new(transactions)
        .chain(0, state.view().latest_block().as_deref())
        .with_confidential_features(test_confidential_features(&state, 1))
        .sign(genesis_correct_key.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    state_block.commit().unwrap();
    // Validate genesis block
    // Use correct genesis key and check if transaction is rejected
    let block: SignedBlock = valid_block.into();
    let (_handle, time_source) = TimeSource::new_mock(block.header().creation_time());
    let mut voting_block = None;
    let (_, error) = ValidBlock::validate_signed_genesis_keep_voting_block(
        block,
        &topology,
        &genesis_correct_account_id,
        &time_source,
        &state,
        &mut voting_block,
        iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
    )
    .unpack(|_| {})
    .err()
    .expect("genesis with an unexpected authority must fail validation");
    // The first transaction should be rejected
    assert_eq!(
        error.as_ref(),
        &BlockValidationError::InvalidGenesis(InvalidGenesisError::UnexpectedAuthority)
    );
}
#[tokio::test]
async fn genesis_asset_definition_registration_is_not_domain_gated() {
    let genesis_key_pair = crate::block::checked_keypair();
    let genesis_account_id = AccountId::new(genesis_key_pair.public_key().clone());
    let alice_key_pair = crate::block::checked_keypair();
    let wonderland_domain_id: DomainId =
        DomainId::try_new("wonderland", "universal").expect("Valid domain id");
    let alice_account_id = AccountId::new(alice_key_pair.public_key().clone());
    let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
    let wonderland_domain = Domain::new(wonderland_domain_id.clone()).build(&alice_account_id);
    let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);
    let alice_account = Account::new(alice_account_id.clone()).build(&alice_account_id);
    let world = World::with(
        [genesis_domain, wonderland_domain],
        [genesis_account, alice_account],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);
    install_test_lane_manifests(&state);
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("valid domain id"),
        "xor".parse().expect("valid asset name"),
    );
    let instruction = Register::asset_definition(AssetDefinition::numeric(
        asset_definition_id,
        "xor",
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    ));
    let tx = TransactionBuilder::new_genesis(
        genesis_account_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .sign(genesis_key_pair.private_key());
    let block = SignedBlock::genesis(
        vec![tx],
        genesis_key_pair.private_key(),
        test_confidential_features(&state, 1),
        None,
    );
    let topology = crate::sumeragi::network_topology::test_topology_with_keys([&genesis_key_pair]);
    let (_handle, time_source) = TimeSource::new_mock(block.header().creation_time());
    let mut voting_block = None;
    let (_valid, state_block) = ValidBlock::validate_signed_genesis_keep_voting_block(
        block,
        &topology,
        &genesis_account_id,
        &time_source,
        &state,
        &mut voting_block,
        iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
    )
    .unpack(|_| {})
    .expect("genesis asset-definition registration should not require domain-owner authorization");
    state_block.commit().unwrap();
}
#[tokio::test]
async fn genesis_domain_registration_bootstraps_domain_name_lease() {
    let genesis_key_pair = crate::block::checked_keypair();
    let genesis_account_id = AccountId::new(genesis_key_pair.public_key().clone());
    let wonderland_domain_id: DomainId =
        DomainId::try_new("wonderland", "universal").expect("valid domain id");
    let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
    let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);
    let world = World::with([genesis_domain], [genesis_account], []);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);
    install_test_lane_manifests(&state);
    let instruction = Register::domain(Domain::new(wonderland_domain_id.clone()));
    let tx = TransactionBuilder::new_genesis(
        genesis_account_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .sign(genesis_key_pair.private_key());
    let block = SignedBlock::genesis(
        vec![tx],
        genesis_key_pair.private_key(),
        test_confidential_features(&state, 1),
        None,
    );
    let topology = crate::sumeragi::network_topology::test_topology_with_keys([&genesis_key_pair]);
    let (_handle, time_source) = TimeSource::new_mock(block.header().creation_time());
    let mut voting_block = None;
    let (_valid, state_block) = ValidBlock::validate_signed_genesis_keep_voting_block(
        block,
        &topology,
        &genesis_account_id,
        &time_source,
        &state,
        &mut voting_block,
        iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
    )
    .unpack(|_| {})
    .expect("genesis domain registration should bootstrap the SNS lease");
    state_block.commit().unwrap();
    let view = state.view();
    assert_eq!(
        crate::sns::active_domain_owner(view.world(), &wonderland_domain_id, 0),
        Ok(Some(genesis_account_id)),
        "genesis registration should leave an active domain-name record behind"
    );
}
