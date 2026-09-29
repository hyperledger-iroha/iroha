// SCCP call sites of block validation (`specs/sccp.md` §4.3.2, §4.19): the per-block
// exemption cap judged against the committed parent World, and the consensus height inputs the
// post-execution hook receives on the Sumeragi-core path and for a v2 signed genesis.

fn sccp_call_site_parameters_committed(state: &State) {
    let mut world = state.world.block();
    *world.sccp_parameters.get_mut() =
        Some(iroha_data_model::sccp::params::SccpParametersV1::taira_default());
    world.commit();
}

fn sccp_call_site_genesis() -> (KeyPair, SignedBlock) {
    let key_pair = KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519)
        .expect("deterministic genesis key");
    let transaction = crate::smartcontracts::isi::sccp::test_support::sample_signed_transaction();
    let genesis = SignedBlock::genesis(vec![transaction], key_pair.private_key(), None, None);
    (key_pair, genesis)
}

#[test]
fn sccp_exempt_cap_is_judged_against_the_committed_parent_world() {
    let state = crate::smartcontracts::isi::sccp::test_support::blank_state();
    let key_pair = KeyPair::try_from_seed(vec![0x52; 32], Algorithm::Ed25519)
        .expect("deterministic leader key");
    let block = ValidBlock::new_dummy(key_pair.private_key());
    let signed: &SignedBlock = block.as_ref();
    ValidBlock::validate_sccp_exempt_cap_against(signed, &state.world.view())
        .expect("no transaction is exempt without SCCP");
    {
        let mut state_block = state.block(signed.header());
        {
            let mut transaction = state_block.transaction();
            crate::smartcontracts::isi::sccp::store::parameters::set(
                &mut transaction,
                Some(iroha_data_model::sccp::params::SccpParametersV1::taira_default()),
            );
            transaction.apply();
        }
        assert!(
            crate::smartcontracts::isi::sccp::params::exists(&state_block.world),
            "the block overlay holds the uncommitted parameters"
        );
        assert!(
            !crate::smartcontracts::isi::sccp::params::exists(
                &state_block.sccp_parent_world_view()
            ),
            "block-start writes are invisible to the parent view the cap is judged against"
        );
        ValidBlock::validate_sccp_exempt_cap(signed, &state_block)
            .expect("no transaction of the block is exempt-shaped");
    }
    sccp_call_site_parameters_committed(&state);
    let state_block = state.block(signed.header());
    assert!(crate::smartcontracts::isi::sccp::params::exists(
        &state_block.sccp_parent_world_view()
    ));
    ValidBlock::validate_sccp_exempt_cap(signed, &state_block)
        .expect("no transaction of the block is exempt-shaped");
}

#[test]
fn sccp_exempt_cap_counts_exempt_shapes_of_the_block_entry_points() {
    use crate::smartcontracts::isi::sccp::test_support::SampleInstructions;
    let key_pair = KeyPair::try_from_seed(vec![0x54; 32], Algorithm::Ed25519)
        .expect("deterministic authority key");
    let attestation = |height| {
        let mut submit = SampleInstructions::attestations();
        submit.entries[0].height = height;
        TransactionBuilder::new(
            iroha_data_model::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new([0x54; 32])),
            ),
            AccountId::new(key_pair.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(submit)])
        .sign(key_pair.private_key())
    };
    let block = SignedBlock::genesis(
        vec![
            attestation(9),
            attestation(10),
            crate::smartcontracts::isi::sccp::test_support::sample_signed_transaction(),
        ],
        key_pair.private_key(),
        None,
        None,
    );
    assert_eq!(block.network_entrypoints().count(), 3);
    let with_cap = |cap| {
        let state = crate::smartcontracts::isi::sccp::test_support::blank_state();
        let mut parameters = iroha_data_model::sccp::params::SccpParametersV1::taira_default();
        parameters.max_exempt_transactions_per_block = cap;
        let mut world = state.world.block();
        *world.sccp_parameters.get_mut() = Some(parameters);
        world.commit();
        ValidBlock::validate_sccp_exempt_cap_against(&block, &state.world.view())
    };
    with_cap(2).expect("two exempt-shaped transactions fit a cap of two");
    let error = with_cap(1).expect_err("the second exempt-shaped transaction exceeds the cap");
    assert!(error.to_string().contains("exempt-shaped"), "{error}");
}

#[test]
fn sccp_genesis_height_context_is_frozen_only_for_a_genesis_that_initialized_sccp() {
    let (_genesis_key, genesis) = sccp_call_site_genesis();
    assert!(genesis.header().is_genesis());
    let state = crate::smartcontracts::isi::sccp::test_support::blank_state();
    {
        let state_block = state.block(genesis.header());
        assert_eq!(
            ValidBlock::sccp_genesis_height_context(&genesis, &state_block),
            None,
            "a genesis without SCCP freezes nothing"
        );
    }
    sccp_call_site_parameters_committed(&state);
    let key_pair = KeyPair::try_from_seed(vec![0x53; 32], Algorithm::Ed25519)
        .expect("deterministic leader key");
    let ordinary = ValidBlock::new_dummy(key_pair.private_key());
    let ordinary: &SignedBlock = ordinary.as_ref();
    assert!(!ordinary.header().is_genesis());
    let state_block = state.block(ordinary.header());
    assert_eq!(
        ValidBlock::sccp_genesis_height_context(ordinary, &state_block),
        None,
        "a non-genesis block uses its own authenticated context"
    );
    drop(state_block);
    let state_block = state.block(genesis.header());
    assert_eq!(
        ValidBlock::sccp_genesis_height_context(&genesis, &state_block),
        None,
        "a genesis without signed consensus metadata fails closed for SCCP"
    );
}

#[test]
fn sccp_hook_receives_scheduled_height_inputs_on_the_sumeragi_core_path() {
    use crate::smartcontracts::isi::sccp::{
        height::{SccpHeightInputsV1, sumeragi_epoch},
        hook::observed,
    };
    use crate::sumeragi::{
        payload::{self, Assembly},
        startup::GENESIS_HEIGHT,
        test_chain::{CertifiedTestChain, TestChainConfig},
    };
    use iroha_data_model::parameter::system::ConsensusMode;
    let signer = KeyPair::try_from_seed(vec![0x55; 32], Algorithm::Ed25519)
        .expect("deterministic transaction signer");
    let authority = AccountId::new(signer.public_key().clone());
    let world =
        crate::state::World::with([], [Account::new(authority.clone()).build(&authority)], []);
    {
        let mut block = world.block();
        *block.sccp_parameters.get_mut() =
            Some(iroha_data_model::sccp::params::SccpParametersV1::taira_default());
        block.commit();
    }
    let _ = observed::take();
    let chain =
        CertifiedTestChain::start(TestChainConfig::new(world, 10_000)).expect("the chain starts");
    let state = chain.state();
    let expected = |height: u64| {
        let view = state.view();
        let schedule = view.world().consensus_schedule();
        let scheduled = schedule.get(height).expect("a scheduled height");
        let (epoch, epoch_end_height) =
            sumeragi_epoch(height, GENESIS_HEIGHT, scheduled.params.epoch_length_blocks)
                .expect("a valid epoch length");
        let next_roster = (height == epoch_end_height).then(|| {
            schedule
                .get(height + 1)
                .expect("the next scheduled height")
                .committee
                .clone()
        });
        SccpHeightInputsV1 {
            mode: ConsensusMode::Permissioned,
            height,
            epoch,
            epoch_end_height,
            roster: scheduled.committee.clone(),
            next_roster,
        }
    };
    let genesis_inputs = expected(GENESIS_HEIGHT);
    assert_eq!(
        observed::take(),
        vec![(GENESIS_HEIGHT, Some(genesis_inputs.clone()))],
        "genesis applied by the Sumeragi core feeds its schedule to the hook"
    );
    let validators = chain
        .validators()
        .iter()
        .map(|(peer, _)| peer.clone())
        .collect::<Vec<_>>();
    assert_eq!(genesis_inputs.roster, validators);
    // Block 2 executes exactly as `StateExecutor::run_execution` does.
    let view = state.view();
    let parent = view.latest_block().expect("the applied genesis");
    let scheduled = view
        .world()
        .consensus_schedule()
        .get(GENESIS_HEIGHT + 1)
        .cloned()
        .expect("the schedule covers the next height");
    drop(view);
    let cadence = Duration::from_millis(scheduled.params.block_time_ms);
    let block_time = parent.header().creation_time() + cadence;
    let transaction = chain.sign(
        &signer,
        [InstructionBox::from(Log::new(
            Level::DEBUG,
            "SCCP scheduled height inputs".to_owned(),
        ))],
        u64::try_from(block_time.as_millis()).expect("fixture time fits") - 1,
    );
    let (_, time_source) = TimeSource::new_mock(block_time);
    let accepted = AcceptedTransaction::accept_with_time_source(
        transaction,
        &chain.network_id(),
        Duration::from_secs(1),
        state.view().world().parameters().transaction(),
        &iroha_config::parameters::actual::Crypto::default(),
        &time_source,
    )
    .expect("the signed fixture transaction is accepted");
    let router = crate::queue::Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        tokio::sync::broadcast::channel(16).0,
    );
    let plan = router
        .route_plan_with_state(&accepted, state)
        .expect("the signed fixture transaction routes");
    let block = payload::assemble(
        state,
        Assembly {
            parent: &parent,
            view: 0,
            cadence,
        },
        &[(accepted, plan)],
    )
    .expect("the canonical nonempty block");
    assert_eq!(block.network_entrypoint_count(), 1);
    let topology = Topology::new(scheduled.committee.clone());
    let executed = ValidBlock::validate_sumeragi_block(
        block,
        &topology,
        chain.genesis_account(),
        cadence,
        ConsensusMode::Permissioned,
        crate::sumeragi::lanes::merge::LaneStepInput::default(),
        state,
    )
    .unpack(|_| {})
    .unwrap_or_else(|(_, error)| panic!("the Sumeragi block executes: {error}"));
    drop(executed);
    assert_eq!(
        observed::take(),
        vec![(GENESIS_HEIGHT + 1, Some(expected(GENESIS_HEIGHT + 1)))],
        "a Sumeragi-core block never reaches the hook without height inputs"
    );
}

#[test]
fn sccp_hook_receives_the_nodes_own_frozen_v2_genesis_context() {
    use crate::smartcontracts::isi::sccp::{height::SccpHeightInputsV1, hook::observed};
    use iroha_data_model::block::consensus_v2::{
        ConsensusMode, SumeragiV2GenesisContextParameters, ValidatorPower,
    };
    use iroha_genesis::{GenesisBlock, GenesisBuilder, GenesisTopologyEntry};
    use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
    iroha_genesis::init_instruction_registry();
    let chain_id = iroha_model_base::chain::ChainId::from("sccp-genesis-context");
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.lane_config =
        iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
    let mut keys = (0_u8..4)
        .map(|index| {
            KeyPair::try_from_seed(vec![0xD0 + index; 32], Algorithm::BlsNormal)
                .expect("deterministic validator key")
        })
        .collect::<Vec<_>>();
    keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
    let entries = keys
        .iter()
        .map(|key| {
            GenesisTopologyEntry::new(
                PeerId::new(key.public_key().clone()),
                iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("fixture PoP"),
            )
        })
        .collect::<Vec<_>>();
    let roster = entries
        .iter()
        .map(|entry| ValidatorPower {
            validator: entry.peer.clone(),
            power: 1,
        })
        .collect::<Vec<_>>();
    let topology = Topology::new(entries.iter().map(|entry| entry.peer.clone()));
    let build = |parameters: SumeragiV2GenesisContextParameters| {
        GenesisBuilder::new_without_executor(chain_id.clone(), ".")
            .set_topology(entries.clone())
            .with_sumeragi_v2_context_parameters(parameters)
            .with_kagemusha_mint_finality_genesis_parameters(
                crate::kagemusha_v1_test_fixtures::mint_finality_genesis_parameters(&roster),
            )
            .build_raw()
            .expect("raw genesis")
            .with_consensus_meta()
            .build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
                &SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
                Some(crate::da::active_proof_policy_bundle_at_height(&nexus, 1)),
                None,
                1_000,
            )
            .expect("signed genesis")
            .0
    };
    // The genesis initializes SCCP: its World carries SCCP parameters before execution.
    let state_for = |genesis: &SignedBlock| {
        let account = SAMPLE_GENESIS_ACCOUNT_ID.clone();
        let world = crate::state::World::with(
            [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&account)],
            [Account::new(account.clone()).build(&account)],
            [],
        );
        {
            let mut block = world.block();
            *block.sccp_parameters.get_mut() =
                Some(iroha_data_model::sccp::params::SccpParametersV1::taira_default());
            block.commit();
        }
        let (state, _) = State::new_with_chain_and_network_id_and_pre_genesis_nexus_for_testing(
            world,
            nexus.clone(),
            crate::query::store::LiveQueryStore::start_test(),
            chain_id.clone(),
            iroha_data_model::NetworkId::from_genesis_hash(genesis.hash()),
        );
        let state = Box::new(state);
        let snapshot = state.nexus_snapshot();
        state.install_lane_manifests(&Arc::new(
            crate::governance::manifest::LaneManifestRegistry::empty()
                .rebind(&snapshot.lane_catalog, &snapshot.governance),
        ));
        state
    };
    fn execute<'state>(
        state: &'state State,
        genesis: SignedBlock,
        topology: &Topology,
    ) -> (ValidBlock, Box<StateBlock<'state>>) {
        ValidBlock::validate_signed_genesis(
            genesis,
            topology,
            &SAMPLE_GENESIS_ACCOUNT_ID,
            &TimeSource::new_system(),
            state,
            ConsensusMode::Permissioned,
        )
        .unpack(|_| {})
        .unwrap_or_else(|(_, error)| panic!("authenticated genesis execution: {error}"))
    }
    let mut parameters = SumeragiV2GenesisContextParameters::recommended();
    {
        let proposal = build(parameters);
        let state = state_for(&proposal);
        let (_, staged) = execute(&state, proposal, &topology);
        parameters.nexus_amx_context_hash =
            *crate::sumeragi::staged_genesis_nexus_amx_context_hash(&staged).as_ref();
        parameters.execution_policy_hash =
            *crate::sumeragi::staged_genesis_execution_policy_hash(&staged)
                .expect("staged execution policy")
                .as_ref();
    }
    let proposal = build(parameters);
    let state = state_for(&proposal);
    let _ = observed::take();
    let (_, staged) = execute(&state, proposal.clone(), &topology);
    let calls = observed::take();
    // The node's own freeze after execution, as block publication performs it.
    let bootstrap = crate::sumeragi::freeze_staged_genesis_v2(
        &GenesisBlock(proposal),
        &staged,
        ConsensusMode::Permissioned,
    )
    .expect("the node freezes its own signed genesis");
    let expected = SccpHeightInputsV1::from_height_context(bootstrap.context());
    assert_eq!(
        calls,
        vec![(1, Some(expected.clone()))],
        "the context frozen before the output seal is the node's own genesis context"
    );
    assert_eq!(expected.height, 1);
    assert_eq!(expected.mode, ConsensusMode::Permissioned);
    assert_eq!(
        expected.roster,
        entries
            .iter()
            .map(|entry| entry.peer.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        expected.next_roster.is_some(),
        expected.is_boundary(),
        "the next roster is present exactly at a boundary"
    );
}
