//! Overlay chunking: ensure overlays apply correctly when chunk size is small.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Builds a transaction with many `SetKeyValue` instructions and sets
#![allow(clippy::cast_possible_truncation)]
//! `overlay_chunk_instructions` to a tiny value to force many chunks.

use std::borrow::Cow;

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_data_model::prelude::*;

#[test]
fn overlay_apply_respects_chunking_and_preserves_effects() {
    // Build world with one domain/account
    let (account_id, kp) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let world = iroha_core::state::World::with([domain], [account], []);
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let mut state = iroha_core::state::State::new_with_chain(
        world,
        kura,
        query,
        ChainId::from("chain"),
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state =
        iroha_core::state::State::new_with_chain(world, kura, query, ChainId::from("chain"));

    // Configure tiny chunk size (e.g., 2 instructions per chunk)
    let mut cfg = state.view().pipeline().clone();
    cfg.overlay_chunk_instructions = 2;
    state.set_pipeline(cfg);
    eprintln!("configured pipeline chunk=2");

    // Build one transaction with many SetKeyValue instructions on the same account
    let n_instr = 50usize;
    let mut instrs: Vec<InstructionBox> = Vec::with_capacity(n_instr);
    for i in 0..n_instr {
        let key: Name = format!("k{i}").parse().unwrap();
        instrs.push(
            SetKeyValue::account(
                account_id.clone(),
                key,
                iroha_primitives::json::Json::new(i as u32),
            )
            .into(),
        );
    }
    let tx = TransactionBuilder::new(ChainId::from("chain"), account_id.clone())
        .with_executable(Executable::from_iter(instrs))
        .sign(kp.private_key());

    // Build and apply a block with this transaction
    let accepted = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let new_block = BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .sign(kp.private_key())
        .unpack(|_| {});
    let mut sb = state.block(new_block.header());
    let vb = ValidBlock::validate_unchecked(new_block.into(), &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let _events = sb.apply_without_execution(&cb, Vec::new());
    let _ = sb.commit();

    // Verify all metadata keys were set on the account
    let view = state.view();
    let acc = view
        .world()
        .account(&account_id)
        .expect("account must exist");
    for i in 0..n_instr {
        let key: Name = format!("k{i}").parse().unwrap();
        let val = acc
            .value()
            .0
            .metadata
            .get(&key)
            .cloned()
            .and_then(|j| j.try_into_any_norito::<u32>().ok())
            .unwrap_or(9999);
        assert_eq!(val, i as u32, "metadata key k{i} must equal {i}");
    }
}
