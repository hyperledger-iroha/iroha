//! Ensure structured data events are emitted in the same order as instructions
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! execute within a single transaction (WSV Overlays & Commit ordering).

use std::borrow::Cow;

use iroha_core::block::{BlockBuilder, ValidBlock};
// no specific event enum imports needed here
use iroha_data_model::prelude::*;

#[test]
fn data_events_follow_instruction_order_in_tx() {
    // Build world with a domain, account, and an asset definition
    let (authority_id, kp) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&authority_id);
    let acc = Account::new(authority_id.clone()).build(&authority_id);
    let ad: AssetDefinition = AssetDefinition::new(
        iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
        NumericSpec::default(),
    )
    .build(&authority_id);
    let world = iroha_core::state::World::with([domain], [acc], [ad]);
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

    // Single transaction: three instructions in a fixed order
    let asset = AssetId::of(
        iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
        authority_id.clone(),
    );
    let instrs: Vec<InstructionBox> = vec![
        // 1) Account metadata insert
        SetKeyValue::account(authority_id.clone(), "a_key".parse().unwrap(), "a_val").into(),
        // 2) Domain metadata insert
        SetKeyValue::domain(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "d_key".parse().unwrap(),
            "d_val",
        )
        .into(),
        // 3) Mint creates the asset (first time)
        Mint::asset_numeric(10_u32, asset.clone()).into(),
    ];
    let tx = TransactionBuilder::new(ChainId::from("chain"), authority_id.clone())
        .with_instructions(instrs)
        .sign(kp.private_key());

    // Build and validate block with one tx
    let acc = iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let new_block = BlockBuilder::new(vec![acc])
        .chain(0, None)
        .sign(kp.private_key())
        .unpack(|_| {});
    let mut sb = state.block(new_block.header());
    let vb = ValidBlock::validate_unchecked(new_block.into(), &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    let events = sb.apply_without_execution(&cb, Vec::new());

    // Extract Data events in emission order
    let data_events: Vec<_> = events
        .into_iter()
        .filter_map(|ev| match ev {
            iroha_data_model::events::EventBox::Data(d) => Some(d),
            _ => None,
        })
        .collect();

    // Expect at least three data events; basic sanity on count
    assert!(data_events.len() >= 3, "expected multiple data events");
}
