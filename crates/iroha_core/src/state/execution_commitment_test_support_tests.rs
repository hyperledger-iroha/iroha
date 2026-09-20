//! Borrowed projection controls using genuinely authenticated genesis execution.

use super::*;
use crate::{
    governance::manifest::LaneManifestRegistry,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World, storage_transactions::TransactionsBlockError},
    sumeragi::network_topology::Topology,
};
use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockSignature, SignedBlock,
        consensus_v2::{ConsensusMode, SumeragiV2GenesisContextParameters, ValidatorPower},
    },
    prelude::*,
};
use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry};
use iroha_model_base::{chain::ChainId, peer::PeerId};
use iroha_primitives::time::TimeSource;
use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
use std::sync::Arc;

fn fixture() -> (Box<State>, SignedBlock, Topology) {
    iroha_genesis::init_instruction_registry();
    let chain_id = ChainId::from("borrowed-execution-commitment");
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
    let genesis = GenesisBuilder::new_without_executor(chain_id.clone(), ".")
        .set_topology(entries)
        .with_sumeragi_v2_context_parameters(SumeragiV2GenesisContextParameters::recommended())
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

fn execute<'state>(
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
    .unwrap_or_else(|(_, error)| panic!("authenticated genesis execution: {error}"))
}

#[test]
fn retained_execution_projection_matches_canonical_consensus_projection_without_publication() {
    let (state, genesis, topology) = fixture();
    let (valid, staged) = execute(&state, genesis, &topology);
    let manifest = exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        valid.as_ref(),
        staged.staged_merge_entry(),
    )
    .unwrap();
    let lanes = exec::LaneFinalityManifestV1::from_result_bearing_block(valid.as_ref()).unwrap();
    let expected = exec::execution_commitment_from_validated_block(
        staged.exec_witness.as_ref().unwrap(),
        &manifest,
        &lanes,
        valid.as_ref(),
    )
    .unwrap();
    assert_eq!(
        staged.execution_commitment_for_testing(&valid).unwrap(),
        expected
    );
    assert_eq!(
        staged.execution_commitment_for_testing(&valid).unwrap(),
        expected
    );
    assert!(
        staged.exec_witness.is_some(),
        "projection only borrows the actual witness"
    );
    staged.verify_execution_output_seal(valid.as_ref()).unwrap();
    assert!(matches!(
        staged.execution_output_plan.as_ref(),
        Some(crate::state::output_capacity::ExecutionOutputPlanState::Sealed(_))
    ));
    assert!(
        matches!(
            (*staged).commit().unwrap_err(),
            TransactionsBlockError::ExecutionOutputCapacity
        ),
        "a projected commitment is not the unfinished State publication authority"
    );
}

#[test]
fn retained_execution_projection_requires_the_actual_cached_witness() {
    let (state, genesis, topology) = fixture();
    let (valid, mut staged) = execute(&state, genesis, &topology);
    let before = staged.execution_commitment_for_testing(&valid).unwrap();
    let witness = staged.exec_witness.take();
    assert_eq!(
        staged.execution_commitment_for_testing(&valid).unwrap_err(),
        "test projection requires a captured execution witness"
    );
    staged.verify_execution_output_seal(valid.as_ref()).unwrap();
    staged.exec_witness = witness;
    assert_eq!(
        staged.execution_commitment_for_testing(&valid).unwrap(),
        before
    );
}

#[test]
fn retained_execution_projection_rejects_foreign_proposals_and_same_proposal_wire_substitution() {
    let (state, genesis, topology) = fixture();
    let (valid, staged) = execute(&state, genesis, &topology);
    let expected = staged.execution_commitment_for_testing(&valid).unwrap();
    let mut foreign = valid.as_ref().clone();
    let mut header = foreign.header();
    header.creation_time_ms += 1;
    foreign.replace_header_for_testing(header);
    assert_ne!(foreign.hash(), valid.as_ref().hash());
    assert!(
        staged
            .execution_commitment_for_testing(&ValidBlock::new_unverified_for_tests(foreign))
            .is_err()
    );
    let mut changed_wire = valid.as_ref().clone();
    changed_wire
        .add_signature(BlockSignature::new(
            1,
            SignatureOf::from_hash(
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
                changed_wire.hash(),
            ),
        ))
        .unwrap();
    assert_eq!(changed_wire.hash(), valid.as_ref().hash());
    assert_ne!(
        Hash::new(changed_wire.encode_wire().unwrap()),
        Hash::new(valid.as_ref().encode_wire().unwrap())
    );
    assert!(
        staged
            .execution_commitment_for_testing(&ValidBlock::new_unverified_for_tests(changed_wire))
            .is_err()
    );
    assert_eq!(
        staged.execution_commitment_for_testing(&valid).unwrap(),
        expected
    );
    assert!(
        staged.exec_witness.is_some(),
        "rejection does not clear retained owners"
    );
}
