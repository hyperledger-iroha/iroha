//! Unit tests for executing single instructions via the IVM executor.
//!
//! Raw domain registration is reserved for genesis, so the executor runs these
//! instructions inside the authenticated signed genesis: the only context that admits
//! them. A failing instruction rejects the whole genesis, which reports its cause.

use iroha_core::{
    state::{World, WorldReadOnly},
    sumeragi::test_chain::{CertifiedTestChain, StartFailure, TestChainConfig},
};
use iroha_data_model::prelude::*;
use iroha_model_base::domain::DomainId;
/// A fixture chain whose signed genesis carries `instructions` after its topology.
fn genesis_with(instructions: Vec<InstructionBox>) -> TestChainConfig {
    let mut config = TestChainConfig::new(World::new(), 0);
    config.genesis_instructions = instructions;
    config
}
/// Execute `instructions` through the executor in the signed genesis and return its refusal.
fn rejected_genesis(instructions: Vec<InstructionBox>) -> StartFailure {
    match CertifiedTestChain::start(genesis_with(instructions)) {
        Ok(_) => panic!("signed genesis unexpectedly applied every instruction"),
        Err(failure) => failure,
    }
}
#[test]
fn ivm_instruction_executes() {
    let domain_id: DomainId = DomainId::try_new("test", "universal").expect("domain id");
    let instruction: InstructionBox = Register::domain(Domain::new(domain_id.clone())).into();
    let state = crate::native_genesis::certified_state_with(genesis_with(vec![instruction]));
    assert!(state.view().world().domain(&domain_id).is_ok());
}
#[test]
fn ivm_instruction_reports_error() {
    let domain_id: DomainId = DomainId::try_new("fail", "universal").expect("domain id");
    let instruction: InstructionBox = Register::domain(Domain::new(domain_id.clone())).into();
    // The first execution is admitted on its own.
    crate::native_genesis::certified_state_with(genesis_with(vec![instruction.clone()]));
    // Repeating the same registration reports an error instead of applying.
    let result = CertifiedTestChain::start(genesis_with(vec![instruction.clone(), instruction]));
    assert!(result.is_err());
}
#[test]
fn register_genesis_domain_rejected() {
    let domain_id = (*iroha_genesis::GENESIS_DOMAIN_ID).clone();
    let instruction: InstructionBox = Register::domain(Domain::new(domain_id.clone())).into();
    let err = rejected_genesis(vec![instruction]);
    assert!(format!("{err:?}").contains("genesis domain"));
}
