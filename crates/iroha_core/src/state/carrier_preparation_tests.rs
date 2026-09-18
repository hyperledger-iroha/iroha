//! Actual four-validator candidate preparation, ownership and drop controls.

use mv::storage::StorageReadOnly;

use super::*;
use crate::{
    block::valid::SumeragiV2ValidationContext,
    governance::manifest::LaneManifestRegistry,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{
        MusubiResolverIndexRevisionV1, State, World, output_capacity::ExecutionOutputPlanState,
        storage_transactions::TransactionsBlockError,
    },
    sumeragi::{network_topology::Topology, v2_apply::V2ApplyService},
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    NetworkId,
    block::consensus_v2::{ConsensusMode, SumeragiV2GenesisContextParameters, ValidatorPower},
    musubi::MusubiRegistrySnapshotV1,
    prelude::*,
};
use iroha_genesis::{GenesisBlock, GenesisBuilder, GenesisTopologyEntry};
use iroha_model_base::{chain::ChainId, peer::PeerId};
use iroha_primitives::time::TimeSource;
use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};

fn genesis(
    parameters: SumeragiV2GenesisContextParameters,
    instructions: &[InstructionBox],
) -> (SignedBlock, Topology) {
    iroha_genesis::init_instruction_registry();
    let mut keys = (0_u8..4)
        .map(|index| KeyPair::try_from_seed(vec![0xB0 + index; 32], Algorithm::BlsNormal).unwrap())
        .collect::<Vec<_>>();
    keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
    let entries = keys
        .iter()
        .map(|key| {
            GenesisTopologyEntry::new(
                PeerId::new(key.public_key().clone()),
                iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
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
    let mut builder =
        GenesisBuilder::new_without_executor(ChainId::from("carrier-preparation"), ".")
            .set_topology(entries)
            .with_sumeragi_v2_context_parameters(parameters)
            .with_kagemusha_mint_finality_genesis_parameters(
                crate::kagemusha_v1_test_fixtures::mint_finality_genesis_parameters(&roster),
            );
    for instruction in instructions {
        builder = builder.append_instruction(instruction.clone());
    }
    let genesis = builder
        .build_raw()
        .unwrap()
        .with_consensus_meta()
        .build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
            &SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
            None,
            None,
            1_000,
        )
        .unwrap()
        .0;
    (genesis, topology)
}

#[inline(never)]
fn state_for(genesis: &SignedBlock) -> Box<State> {
    let account = SAMPLE_GENESIS_ACCOUNT_ID.clone();
    let world = World::with(
        [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&account)],
        [Account::new(account.clone()).build(&account)],
        [],
    );
    let state = Box::new(State::new_with_chain_and_network_id_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        ChainId::from("carrier-preparation"),
        NetworkId::from_genesis_hash(genesis.hash()),
    ));
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));
    state
}

fn signed_genesis_execution<'state>(
    state: &'state State,
    genesis: SignedBlock,
    topology: &Topology,
) -> (ValidBlock, Box<StateBlock<'state>>) {
    ValidBlock::validate_signed_genesis_keep_voting_block(
        genesis,
        topology,
        &SAMPLE_GENESIS_ACCOUNT_ID,
        &TimeSource::new_system(),
        state,
        &mut None,
        ConsensusMode::Permissioned,
    )
    .unpack(|_| {})
    .unwrap_or_else(|(failed, error)| {
        panic!(
            "authenticated genesis execution: {error}; outputs: {:?}",
            failed.execution_outputs()
        )
    })
}

pub(super) fn fixture() -> (Box<State>, SignedBlock, Topology, HeightContext) {
    fixture_with_instructions(&[])
}

pub(super) fn fixture_with_instructions(
    instructions: &[InstructionBox],
) -> (Box<State>, SignedBlock, Topology, HeightContext) {
    let mut parameters = SumeragiV2GenesisContextParameters::recommended();
    {
        let (proposal, topology) = genesis(parameters, instructions);
        let state = state_for(&proposal);
        let (_, staged) = signed_genesis_execution(&state, proposal, &topology);
        parameters.nexus_amx_context_hash =
            *crate::sumeragi::staged_genesis_nexus_amx_context_hash(&staged).as_ref();
        parameters.execution_policy_hash =
            *crate::sumeragi::staged_genesis_execution_policy_hash(&staged)
                .unwrap()
                .as_ref();
    }
    let (proposal, topology) = genesis(parameters, instructions);
    let state = state_for(&proposal);
    let context = {
        let (_, staged) = signed_genesis_execution(&state, proposal.clone(), &topology);
        crate::sumeragi::freeze_staged_genesis_v2(
            &GenesisBlock(proposal.clone()),
            &staged,
            ConsensusMode::Permissioned,
        )
        .unwrap()
        .context()
        .clone()
    };
    assert_eq!(context.roster.len(), 4);
    (state, proposal, topology, context)
}

pub(super) fn prepare<'state>(
    state: &'state State,
    proposal: SignedBlock,
    topology: &Topology,
    context: &HeightContext,
) -> Result<PreparedCarrier<'state>, (Box<SignedBlock>, Box<crate::block::BlockValidationError>)> {
    ValidBlock::validate_and_prepare_sumeragi_v2_candidate_keep_voting_block(
        proposal,
        topology,
        &SAMPLE_GENESIS_ACCOUNT_ID,
        &TimeSource::new_system(),
        state.sumeragi_block_cadence(),
        SumeragiV2ValidationContext::from_height_context(context),
        state,
        &mut None,
    )
}

#[test]
fn candidate_preparation_retains_actual_prefix_and_context_without_publication() {
    let (state, proposal, topology, context) = fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let expected_prefix = {
        let (valid, staged) = ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block(
            proposal.clone(),
            &topology,
            &SAMPLE_GENESIS_ACCOUNT_ID,
            &TimeSource::new_system(),
            state.sumeragi_block_cadence(),
            SumeragiV2ValidationContext::from_height_context(&context),
            &state,
            &mut None,
        )
        .unpack(|_| {})
        .unwrap_or_else(|(_, error)| panic!("candidate execution: {error}"));
        staged.execution_commitment_for_testing(&valid).unwrap()
    };
    let prepared = prepare(&state, proposal.clone(), &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("candidate preparation: {error}"));
    assert_eq!(prepared.execution_prefix_commitment(), expected_prefix);
    assert_eq!(prepared.context(), &context);
    assert_eq!(prepared.block().hash(), proposal.hash());
    assert_eq!(prepared.state.block_hashes.last(), Some(&proposal.hash()));
    assert_eq!(
        prepared.state.world.musubi_resolver_index_checkpoints.len(),
        1
    );
    assert!(prepared.state.exec_witness.is_some());
    assert!(
        prepared
            .state
            .verified_fastpq_source_inventory_for_capture()
            .is_ok()
    );
    assert!(matches!(
        prepared.state.execution_output_plan.as_ref(),
        Some(ExecutionOutputPlanState::Sealed(_))
    ));
    assert!(
        prepared
            .state
            .canonical_carrier_commit_metadata_authorization
            .is_none()
    );
    drop(prepared);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    let prepared = prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("repeat preparation after drop: {error}"));
    // Test-only field access proves preparation did not grant publication. The
    // production owner deliberately has no mutable or consuming State accessor.
    assert_eq!(
        (*prepared.state).commit().unwrap_err(),
        TransactionsBlockError::ExecutionOutputCapacity
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura.blocks_count(), 0);
}

#[test]
fn candidate_preparation_rejects_changed_header_and_frozen_context() {
    let (state, proposal, topology, context) = fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let mut foreign = proposal.clone();
    let mut header = foreign.header();
    header.creation_time_ms += 1;
    foreign.replace_header_for_testing(header);
    assert!(prepare(&state, foreign, &topology, &context).is_err());
    let mut other_height = context.clone();
    other_height.height += 1;
    let mut other_network = context.clone();
    other_network.network_id = NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"foreign genesis")),
    );
    let mut other_roster = context.clone();
    other_roster.roster.swap(0, 1);
    for changed in [other_height, other_network, other_roster] {
        let (_, error) = prepare(&state, proposal.clone(), &topology, &changed)
            .err()
            .expect("foreign context must reject");
        assert!(error.to_string().contains("frozen validation context"));
    }
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura.blocks_count(), 0);
}

#[test]
fn candidate_preparation_drops_all_journals_after_late_metadata_failure() {
    let (state, proposal, topology, context) = fixture();
    {
        // Seed a structurally inconsistent checkpoint history to exercise the
        // fallible tail after membership and the staged hash log were updated.
        // This is corrupt input, never claimed as a finalized State fixture.
        let mut world = state.world.block();
        world.musubi_resolver_index_checkpoints.insert(
            MusubiResolverIndexRevisionV1::new(2).unwrap(),
            MusubiRegistrySnapshotV1 {
                finalized_height: 1,
                finalized_block_hash: *proposal.hash().as_ref(),
                index_revision: 2,
            },
        );
        world.commit();
    }
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    for _ in 0..2 {
        let (_, error) = prepare(&state, proposal.clone(), &topology, &context)
            .err()
            .expect("future checkpoint must reject");
        assert!(
            error
                .to_string()
                .contains("history contains a future revision")
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            before
        );
        assert_eq!(state.committed_height(), 0);
        assert_eq!(state.kura.blocks_count(), 0);
    }
}

#[test]
fn production_candidate_admits_metadata_before_returning_execution_prefix() {
    let (boxed, proposal, topology, context) = fixture();
    let expected = prepare(&boxed, proposal.clone(), &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("candidate preparation: {error}"))
        .execution_prefix_commitment();
    let state: Arc<State> = boxed.into();
    let kura = Arc::clone(&state.kura);
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let (events, _receiver) = tokio::sync::broadcast::channel(32);
    let queue = Arc::new(Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events.clone(),
    ));
    let pops = crate::sumeragi::signed_genesis_validator_pops(&GenesisBlock(proposal.clone()))
        .unwrap()
        .into_values()
        .collect();
    let service = V2ApplyService::new(
        Arc::clone(&state),
        queue,
        Arc::clone(&kura),
        None,
        None,
        state.sumeragi_block_cadence(),
        SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        events,
        pops,
    );
    assert_eq!(
        service.validate_candidate(&context, &proposal).unwrap(),
        expected
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.committed_height(), 0);
    assert_eq!(kura.blocks_count(), 0);
    {
        let mut world = state.world.block();
        world.musubi_resolver_index_checkpoints.insert(
            MusubiResolverIndexRevisionV1::new(2).unwrap(),
            MusubiRegistrySnapshotV1 {
                finalized_height: 1,
                finalized_block_hash: *proposal.hash().as_ref(),
                index_revision: 2,
            },
        );
        world.commit();
    }
    let corrupt_before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let error = service.validate_candidate(&context, &proposal).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("history contains a future revision")
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        corrupt_before
    );
    assert_eq!(state.committed_height(), 0);
    assert_eq!(kura.blocks_count(), 0);
}

#[test]
fn candidate_prepares_exact_events_once_and_drop_does_not_deliver() {
    let (state, proposal, topology, context) = fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let expected = {
        let (valid, staged) = ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block(
            proposal.clone(),
            &topology,
            &SAMPLE_GENESIS_ACCOUNT_ID,
            &TimeSource::new_system(),
            state.sumeragi_block_cadence(),
            SumeragiV2ValidationContext::from_height_context(&context),
            &state,
            &mut None,
        )
        .unpack(|_| {})
        .unwrap_or_else(|(_, error)| panic!("candidate execution: {error}"));
        let mut events = staged.world.external_event_buf.clone();
        events.push(
            crate::state::BlockEvent {
                header: valid.as_ref().header(),
                status: crate::state::BlockStatus::Applied,
            }
            .into(),
        );
        events
    };
    let prepared = prepare(&state, proposal.clone(), &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("event preparation: {error}"));
    assert_eq!(prepared._publication_events, expected);
    assert!(prepared.state.world.external_event_buf.is_empty());
    drop(prepared);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura.blocks_count(), 0);
    let repeated = prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("event preparation after drop: {error}"));
    assert_eq!(repeated._publication_events, expected);
}

#[test]
fn foreign_carrier_event_preparation_preserves_the_exact_buffer() {
    let (state, proposal, topology, _) = fixture();
    let (valid, mut staged) = signed_genesis_execution(&state, proposal, &topology);
    let events = staged.world.external_event_buf.clone();
    let mut foreign = valid.as_ref().header();
    foreign.creation_time_ms += 1;
    let error = staged
        .prepare_carrier_publication_events(foreign)
        .unwrap_err();
    assert!(error.to_string().contains("different carrier"));
    assert_eq!(staged.world.external_event_buf, events);
    assert_eq!(state.committed_height(), 0);
}
