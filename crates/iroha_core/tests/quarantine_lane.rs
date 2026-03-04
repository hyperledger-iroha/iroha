//! Quarantine lane: classification + explicit overflow rejection test.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::items_after_statements)]

// no nonzero macro used in this file
use std::borrow::Cow;

use iroha_core::{
    block::{self, BlockBuilder},
    state::StateReadOnly,
};
use iroha_data_model::prelude::*;

#[test]
fn quarantine_overflow_rejects_one_tx() {
    // Set up a minimal world with one domain and an authority account.
    let chain_id: ChainId = "chain".parse().unwrap();
    let (authority_id, kp) = iroha_test_samples::gen_account_in("wonderland");
    let domain: Domain = Domain::new("wonderland".parse().unwrap()).build(&authority_id);
    let account = Account::new(authority_id.clone()).build(&authority_id);
    let world = iroha_core::state::World::with([domain], [account], []);
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();

    #[cfg(feature = "telemetry")]
    let mut state = iroha_core::state::State::new(
        world,
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state = iroha_core::state::State::new(world, kura, query);

    // Configure quarantine: allow only 1 tx per block (to force overflow).
    let mut cfg = state.view().pipeline().clone();
    cfg.quarantine_max_txs_per_block = 1;
    cfg.quarantine_tx_max_cycles = 0;
    cfg.quarantine_tx_max_millis = 0;
    state.set_pipeline(cfg);

    // Install a simple deterministic classifier that marks all txs as quarantine.
    fn classify_all(_: &iroha_data_model::transaction::SignedTransaction) -> bool {
        true
    }
    block::set_quarantine_classifier(Some(classify_all));

    // Build two simple log transactions signed by the authority.
    let tx1 = TransactionBuilder::new(chain_id.clone(), authority_id.clone())
        .with_instructions([Log::new(Level::INFO, "q1".to_string())])
        .sign(kp.private_key());
    let tx2 = TransactionBuilder::new(chain_id.clone(), authority_id.clone())
        .with_instructions([Log::new(Level::INFO, "q2".to_string())])
        .sign(kp.private_key());

    // Convert into accepted txs and build a block with both.
    let a1 = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx1));
    let a2 = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx2));
    let new_block = BlockBuilder::new(vec![a1, a2])
        .chain(0, None)
        .sign(kp.private_key())
        .unpack(|_| {});

    // Validate and record transactions; commit to state.
    let mut sb = state.block(new_block.header());
    let vb = new_block
        .validate_and_record_transactions(&mut sb)
        .unpack(|_| {});
    let _ = sb.commit();

    // Clear the classifier to avoid cross-test contamination.
    block::set_quarantine_classifier(None);

    // Inspect results: exactly one Approved and one Validation(NotPermitted("quarantine overflow"))
    let block = vb.as_ref();
    let mut approved = 0usize;
    let mut rejected_overflow = 0usize;
    for (idx, _tx) in block.external_transactions().enumerate() {
        match block.error(idx) {
            Some(iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                iroha_data_model::ValidationFail::NotPermitted(msg),
            )) if msg == "quarantine overflow" => {
                rejected_overflow += 1;
            }
            None => {
                approved += 1;
            }
            _ => {}
        }
    }

    assert_eq!(approved, 1, "one tx must be approved");
    assert_eq!(rejected_overflow, 1, "one tx must be rejected as overflow");
}
