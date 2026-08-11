//! Quarantine lane: classification + explicit overflow rejection test.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::items_after_statements)]

// no nonzero macro used in this file
use std::{borrow::Cow, sync::Arc};

use iroha_core::{
    block::BlockBuilder, governance::manifest::LaneManifestRegistry, state::StateReadOnly,
};
use iroha_data_model::prelude::*;

fn quarantine_metadata() -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        "quarantine"
            .parse()
            .expect("canonical quarantine metadata key"),
        true,
    );
    metadata
}

#[test]
fn quarantine_overflow_rejects_one_tx() {
    // Set up a minimal world with one domain and an authority account.
    let chain_id: ChainId = "chain".parse().unwrap();
    let (authority_id, kp) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&authority_id);
    let account = Account::new(authority_id.clone()).build(&authority_id);
    let world = iroha_core::state::World::with([domain], [account], []);
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();

    let mut state =
        iroha_core::state::State::new_with_chain_for_testing(world, kura, query, chain_id.clone());
    let network_id = *state.network_id_ref();
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));

    // Configure quarantine: allow only 1 tx per block (to force overflow).
    let mut cfg = state.view().pipeline().clone();
    cfg.quarantine_max_txs_per_block = 1;
    cfg.quarantine_tx_max_cycles = 0;
    state.set_pipeline(cfg);

    // Build two transactions whose signed metadata opts into the quarantine lane.
    let tx1 = TransactionBuilder::new(
        network_id,
        authority_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "q1".to_string())])
    .with_metadata(quarantine_metadata())
    .sign(kp.private_key());
    let tx2 = TransactionBuilder::new(
        network_id,
        authority_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "q2".to_string())])
    .with_metadata(quarantine_metadata())
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
