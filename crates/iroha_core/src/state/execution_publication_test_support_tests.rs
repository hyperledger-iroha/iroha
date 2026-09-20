//! Publication controls retain actual authenticated execution and complete wire.

use super::*;
use crate::{
    block::BlockBuilder,
    governance::manifest::LaneManifestRegistry,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{StateReadOnly, World, WorldReadOnly},
    sumeragi::network_topology::Topology,
    tx::AcceptedTransaction,
};
use iroha_crypto::{Hash, SignatureOf};
use iroha_data_model::{
    NetworkId,
    block::{BlockSignature, SignedBlock},
    prelude::*,
    transaction::FeePaymentIntent,
};
use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry};
use iroha_model_base::chain::ChainId;
use iroha_primitives::time::TimeSource;
use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
use std::{borrow::Cow, sync::Arc};

fn fixture_with_keys(keys: Vec<KeyPair>) -> (Box<State>, SignedBlock, Topology) {
    iroha_genesis::init_instruction_registry();
    let chain_id = ChainId::from("execution-publication-owner");
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
        .map(|entry| wire::ValidatorPower {
            validator: entry.peer.clone(),
            power: 1,
        })
        .collect::<Vec<_>>();
    let topology = Topology::new(entries.iter().map(|entry| entry.peer.clone()));
    let genesis =
        GenesisBuilder::new_without_executor(chain_id.clone(), ".")
            .set_topology(entries)
            .with_sumeragi_v2_context_parameters(
                wire::SumeragiV2GenesisContextParameters::recommended(),
            )
            .with_kagemusha_mint_finality_genesis_parameters(
                crate::kagemusha_v1_test_fixtures::mint_finality_genesis_parameters(&roster),
            )
            .build_raw()
            .unwrap()
            .with_consensus_meta()
            .build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
                &SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
                None,
                None,
                100,
            )
            .unwrap()
            .0;
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
        chain_id,
        NetworkId::from_genesis_hash(genesis.hash()),
    ));
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));
    (state, genesis, topology)
}

fn fixture() -> (Box<State>, SignedBlock, Topology) {
    fixture_with_keys(keys().unwrap())
}

#[test]
fn component_genesis_retains_execution_and_finality_before_ordinary_work() {
    let (state, _, _) = fixture();
    let genesis = state
        .seed_genesis_for_testing()
        .expect("publish fixture genesis");
    assert_eq!(state.committed_height(), 1);
    assert_eq!(state.latest_block_hash_fast(), Some(genesis.hash()));
    assert!(
        genesis
            .output_results()
            .all(|result| result.as_ref().is_ok())
    );
    let artifact = state.kura.v2_finality_artifact(1).unwrap().unwrap();
    assert_eq!(artifact.block_hash, genesis.hash());
    VerifiedV2FinalityArtifact::verify(artifact).unwrap();
    assert!(state.seed_genesis_for_testing().is_err());
}

#[test]
fn component_genesis_registers_its_missing_authority_through_execution() {
    let signer = KeyPair::try_from_seed(vec![0xA9; 32], Algorithm::Ed25519)
        .expect("explicit fixture signer");
    let authority = AccountId::new(signer.public_key().clone());
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    assert!(state.query_view().world().account(&authority).is_err());
    let genesis = state
        .seed_signed_genesis_for_testing(&signer)
        .expect("execute self-registering genesis");
    assert!(state.query_view().world().account(&authority).is_ok());
    assert_eq!(state.committed_height(), 1);
    assert_eq!(state.latest_block_hash_fast(), Some(genesis.hash()));
    assert!(
        genesis
            .output_results()
            .all(|result| result.as_ref().is_ok())
    );
    let artifact = state.kura.v2_finality_artifact(1).unwrap().unwrap();
    assert_eq!(artifact.block_hash, genesis.hash());
    VerifiedV2FinalityArtifact::verify(artifact).unwrap();
    assert!(state.seed_signed_genesis_for_testing(&signer).is_err());
}

fn execute_genesis<'state>(
    state: &'state State,
    genesis: SignedBlock,
    topology: &Topology,
) -> (CommittedBlock, Box<StateBlock<'state>>) {
    let (valid, staged) = ValidBlock::validate_signed_genesis_keep_voting_block(
        genesis,
        topology,
        &SAMPLE_GENESIS_ACCOUNT_ID,
        &TimeSource::new_system(),
        state,
        &mut None,
        wire::ConsensusMode::Permissioned,
    )
    .unpack(|_| {})
    .unwrap_or_else(|(_, error)| panic!("actual genesis execution: {error}"));
    (valid.commit_unchecked().unpack(|_| {}), staged)
}

#[test]
fn executed_genesis_and_successor_publish_real_finality_and_witnesses() {
    let (state, genesis, topology) = fixture();
    let (committed, staged) = execute_genesis(&state, genesis, &topology);
    let first_hash = committed.as_ref().hash();
    state
        .commit_executed_block_for_testing(*staged, committed)
        .unwrap();
    assert_eq!(state.committed_height(), 1);
    let parent = state.kura.v2_finality_artifact(1).unwrap().unwrap();
    assert_eq!(parent.block_hash, first_hash);
    VerifiedV2FinalityArtifact::verify(parent.clone()).unwrap();
    assert!(
        state
            .kura
            .kagemusha_top_up_operation_ids_v1(1)
            .unwrap()
            .is_empty()
    );
    let tx = TransactionBuilder::new(
        state.network_id,
        SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "actual successor execution".to_owned(),
    )])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let new_block = BlockBuilder::new(vec![accepted])
        .chain(0, state.view().latest_block().as_deref())
        .sign(keys().unwrap()[0].private_key())
        .unpack(|_| {});
    let mut staged = state.block(new_block.header());
    let committed = new_block
        .validate_and_record_transactions(&mut staged)
        .unpack(|_| {})
        .commit_unchecked()
        .unpack(|_| {});
    let second_hash = committed.as_ref().hash();
    let expected_context = v2_context::build_successor_height_context_from_state(
        &parent,
        &state.view(),
        v2_recovery::committed_nexus_amx_context_hash(&state).unwrap(),
    )
    .unwrap();
    state
        .commit_executed_block_for_testing(staged, committed)
        .unwrap();
    assert_eq!(state.committed_height(), 2);
    let finality = state.kura.v2_finality_artifact(2).unwrap().unwrap();
    assert_eq!(finality.height_context, expected_context);
    assert_eq!(finality.block_hash, second_hash);
    VerifiedV2FinalityArtifact::verify(finality).unwrap();
    assert!(
        state
            .kura
            .kagemusha_top_up_operation_ids_v1(2)
            .unwrap()
            .is_empty()
    );
    assert_eq!(state.latest_block_hash_fast(), Some(second_hash));
}

#[test]
fn publication_rejects_an_overlay_from_another_state_before_durable_writes() {
    let (state, genesis, topology) = fixture();
    let (other, _, _) = fixture();
    let (committed, staged) = execute_genesis(&state, genesis, &topology);
    let error = other
        .commit_executed_block_for_testing(*staged, committed)
        .unwrap_err();
    assert!(error.contains("different State"));
    assert_eq!(state.committed_height(), 0);
    assert_eq!(other.committed_height(), 0);
    assert!(state.kura.v2_finality_artifact(1).unwrap().is_none());
    assert!(other.kura.v2_finality_artifact(1).unwrap().is_none());
}

#[test]
fn publication_rejects_changed_sealed_wire_with_the_same_header() {
    let (state, genesis, topology) = fixture();
    let (mut committed, staged) = execute_genesis(&state, genesis, &topology);
    let original = committed.as_ref().hash();
    let before = Hash::new(committed.as_ref().encode_wire().unwrap());
    committed
        .as_mut()
        .add_signature(BlockSignature::new(
            1,
            SignatureOf::from_hash(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(), original),
        ))
        .unwrap();
    assert_eq!(committed.as_ref().hash(), original);
    assert_ne!(Hash::new(committed.as_ref().encode_wire().unwrap()), before);
    let error = state
        .commit_executed_block_for_testing(*staged, committed)
        .unwrap_err();
    assert!(error.contains("attachment changed"));
    assert_eq!(state.committed_height(), 0);
    assert!(state.kura.v2_finality_artifact(1).unwrap().is_none());
}

#[test]
fn publication_requires_the_original_captured_witness() {
    let (state, genesis, topology) = fixture();
    let (committed, mut staged) = execute_genesis(&state, genesis, &topology);
    assert!(staged.take_exec_witness().is_some());
    let error = state
        .commit_executed_block_for_testing(*staged, committed)
        .unwrap_err();
    assert!(error.contains("actual captured witness"));
    assert!(state.kura.v2_finality_artifact(1).unwrap().is_none());
}

#[test]
fn publication_refuses_other_signed_genesis_validator_keys() {
    let mut other_keys = (0xB0_u8..=0xB3)
        .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
        .collect::<Vec<_>>();
    other_keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
    let (state, genesis, topology) = fixture_with_keys(other_keys);
    let (committed, staged) = execute_genesis(&state, genesis, &topology);
    let error = state
        .commit_executed_block_for_testing(*staged, committed)
        .unwrap_err();
    assert!(error.contains("signed genesis voting keys differ"));
    assert!(state.kura.v2_finality_artifact(1).unwrap().is_none());
}
