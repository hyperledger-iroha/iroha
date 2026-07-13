//! Reproduces the ISI gas calibration benchmark setup for debugging.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    executor::{Executor, InstructionExecutionProfile},
    gas,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::prelude::*;
use iroha_test_samples::gen_account_in;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn bench_block_header() -> BlockHeader {
    BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0)
}

fn build_bench_state() -> (State, AccountId, AccountId) {
    let (authority, _) = gen_account_in("wonderland");
    let (recipient, _) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let authority_account = Account::new(authority.clone()).build(&authority);
    let recipient_account = Account::new(recipient.clone()).build(&recipient);
    let world = World::with([domain], [authority_account, recipient_account], []);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query_handle);
    (state, authority, recipient)
}

#[test]
fn execute_register_domain_like_bench() {
    let (state, authority, _recipient) = build_bench_state();
    let executor = Executor::default();
    let mut block = state.block(bench_block_header());
    let mut tx = block.transaction();
    let instr = Register::domain(Domain::new(
        DomainId::try_new("bench", "universal").unwrap(),
    ))
    .into();
    let _ = gas::meter_instruction(&instr);
    executor
        .execute_instruction_with_profile(
            &mut tx,
            &authority,
            instr,
            InstructionExecutionProfile::Bench,
        )
        .expect("bench profile execution");
    let domain_id = DomainId::try_new("bench", "universal").expect("valid domain id");
    assert!(
        tx.world.domains().get(&domain_id).is_some(),
        "domain registered into world state"
    );
    assert!(
        tx.world.accounts().get(&authority).is_some(),
        "authority account present"
    );
}
