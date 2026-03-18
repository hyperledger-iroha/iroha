//! Verify that `ProofEvent::{Verified,Rejected}` carry the transaction `call_hash`.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]

use std::borrow::Cow;

use iroha_core::block::{BlockBuilder, ValidBlock};
use iroha_data_model::{
    events::{
        EventBox,
        data::{prelude::*, proof::ProofEvent},
    },
    prelude::*,
};

#[test]
fn proof_event_includes_call_hash() {
    // Minimal world
    let (authority_id, kp) = iroha_test_samples::gen_account_in("zkd");
    let domain_id: DomainId = "zkd".parse().unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&authority_id);
    let acc = Account::new(authority_id.clone().to_account_id(domain_id)).build(&authority_id);
    let world = iroha_core::state::World::with([domain], [acc], []);
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();
    let state = {
        #[cfg(feature = "telemetry")]
        {
            iroha_core::state::State::new(
                world,
                kura,
                query,
                iroha_core::telemetry::StateTelemetry::default(),
            )
        }
        #[cfg(not(feature = "telemetry"))]
        {
            iroha_core::state::State::new(world, kura, query)
        }
    };

    // Build a tx with a VerifyProof instruction that deterministically rejects
    let pr = iroha_data_model::proof::ProofBox::new("debug/reject".into(), vec![1, 2, 3]);
    let vk = iroha_data_model::proof::VerifyingKeyBox::new("debug/reject".into(), vec![]);
    let attach =
        iroha_data_model::proof::ProofAttachment::new_inline("debug/reject".into(), pr.clone(), vk);
    let verify = iroha_data_model::isi::zk::VerifyProof::new(attach);
    let tx = TransactionBuilder::new(ChainId::from("chain"), authority_id.clone())
        .with_instructions([InstructionBox::from(verify)])
        .sign(kp.private_key());
    let tx_call_hash = tx.hash_as_entrypoint();

    // Build/commit block and collect events
    let acc_tx = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let new_block = BlockBuilder::new(vec![acc_tx])
        .chain(0, None)
        .sign(kp.private_key())
        .unpack(|_| {});
    let mut sb = state.block(new_block.header());
    let vb = ValidBlock::validate_unchecked(new_block.into(), &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let events = sb.apply_without_execution(&cb, Vec::new());

    // Find a Proof::Rejected event and assert call_hash was set
    let mut found = None;
    for ev in events {
        if let EventBox::Data(data) = ev
            && let DataEvent::Proof(ProofEvent::Rejected(r)) = data.as_ref()
        {
            found = Some(r.clone());
            break;
        }
    }
    let r = found.expect("proof rejected event present");
    let ch = r.call_hash.expect("call_hash present in proof event");
    assert_eq!(
        hex::encode(ch),
        hex::encode(iroha_crypto::Hash::from(tx_call_hash).as_ref())
    );
}
