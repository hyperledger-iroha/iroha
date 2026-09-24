//! Registered role-16 instructions remain closed without a topology State owner.
use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::isi::{
        InitialNativeInstructionAdmission, execute_borrowed_instruction,
        registered_native_instruction_initial_admission,
    },
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    block::BlockHeader,
    isi::InstructionBox,
    sorafs::topology_authority::{
        TopologyActionV1, TopologyHeadV1, TopologyRevocationV1, TopologyTransitionV1,
    },
};
use mv::storage::StorageReadOnly;

#[test]
fn every_registered_topology_action_stays_closed_without_state_mutation() {
    let state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let key = KeyPair::try_from_seed(vec![17; 32], Algorithm::Ed25519)
        .expect("deterministic test authority");
    let authority = AccountId::new(key.public_key().clone());
    let header = BlockHeader::new(
        1.try_into().expect("positive height"),
        state.view().latest_block_hash(),
        None,
        1_000,
        0,
    );
    let mut block = state.block(header);
    let mut tx = block.transaction();
    let before = tx
        .world()
        .smart_contract_state()
        .iter()
        .map(|(path, value)| (path.clone(), value.clone()))
        .collect::<Vec<_>>();
    for action in [
        TopologyActionV1::Configure(Vec::new()),
        TopologyActionV1::Revoke(TopologyRevocationV1 {
            signer: true,
            attester: false,
        }),
    ] {
        let instruction: InstructionBox = MutateSorafsTopologyAuthority {
            transition: TopologyTransitionV1 {
                deployment_id: "sora-main".into(),
                control: TopologyHeadV1::EMPTY,
                operations: TopologyHeadV1::EMPTY,
                action,
            },
        }
        .into();
        assert_eq!(
            registered_native_instruction_initial_admission(&instruction),
            Some(InitialNativeInstructionAdmission::Closed)
        );
        let failure = execute_borrowed_instruction(&instruction, &authority, &mut tx)
            .expect_err("topology execution must remain closed");
        assert!(matches!(
            failure,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                ref message
            )) if message == INITIAL_NATIVE_INSTRUCTION_CLOSED_REASON
        ));
        let after = tx
            .world()
            .smart_contract_state()
            .iter()
            .map(|(path, value)| (path.clone(), value.clone()))
            .collect::<Vec<_>>();
        assert_eq!(after, before);
    }
}
