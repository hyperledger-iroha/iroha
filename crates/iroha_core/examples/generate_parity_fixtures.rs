//! Generate canonical JSON fixtures for parity tests.
//!
//! Run with:
//!   cargo run -p `iroha_core` --example `generate_parity_fixtures`
//!
//! It writes fixtures under `crates/iroha_core/tests/fixtures/`.

use std::{fs, io::Write, path::PathBuf};

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    state::StateReadOnly,
};
use iroha_data_model::prelude::*;
use iroha_primitives::numeric::Numeric;
// use mv::storage::StorageReadOnly; // not needed in example

fn fixtures_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p
}

fn write_fixture(name: &str, content: &str) {
    let mut p = fixtures_dir();
    let _ = fs::create_dir_all(&p);
    p.push(format!("{name}.json"));
    let mut f = fs::File::create(&p).expect("create fixture");
    f.write_all(content.as_bytes()).expect("write fixture");
    println!("wrote {}", p.display());
}

fn events_json_filtered(events: &[iroha_data_model::events::prelude::EventBox]) -> String {
    let filtered: Vec<_> = events
        .iter()
        .filter(|e| {
            !matches!(
                e,
                iroha_data_model::events::prelude::EventBox::Time(_)
                    | iroha_data_model::events::prelude::EventBox::Pipeline(_)
                    | iroha_data_model::events::prelude::EventBox::PipelineBatch(_)
            )
        })
        .cloned()
        .collect();
    norito::json::to_json_pretty(&filtered).unwrap()
}

fn run_block_and_events(
    parallel_apply: bool,
    txs: Vec<SignedTransaction>,
) -> (
    Vec<iroha_data_model::events::prelude::EventBox>,
    iroha_core::state::State,
) {
    // Build a fresh world with default sandbox-like setup (rose#wonderland).
    let (alice_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let ad: AssetDefinition =
        AssetDefinition::new("rose#wonderland".parse().unwrap(), NumericSpec::default())
            .build(&alice_id);
    let acc_a = Account::new(alice_id.to_account_id(domain_id.clone())).build(&alice_id);
    let acc_b = Account::new(bob_id.to_account_id(domain_id)).build(&alice_id);
    // Seed initial balances: Alice 60, Bob 10
    let a_coin = AssetId::of(ad.id().clone(), alice_id.clone());
    let b_coin = AssetId::of(ad.id().clone(), bob_id.clone());
    let a0 = Asset::new(a_coin.clone(), Numeric::new(60, 0));
    let b0 = Asset::new(b_coin.clone(), Numeric::new(10, 0));
    let world = iroha_core::state::World::with_assets([domain], [acc_a, acc_b], [ad], [a0, b0], []);
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
    let mut cfg = state.view().pipeline().clone();
    cfg.parallel_apply = parallel_apply;
    state.set_pipeline(cfg);

    // Build a signed block from txs
    let block: SignedBlock = {
        let accepted: Vec<_> = txs
            .into_iter()
            .map(|tx| {
                iroha_core::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(tx))
            })
            .collect();
        BlockBuilder::new(accepted)
            .chain(0, state.view().latest_block().as_deref())
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
            .unpack(|_| {})
            .into()
    };
    // Execute and commit
    let mut sb = state.block(block.header());
    let vb = ValidBlock::validate_unchecked(block, &mut sb).unpack(|_| {});
    let cb = vb.commit_unchecked().unpack(|_| {});
    // Apply block effects without re-executing transactions.
    let events = sb.apply_without_execution(&cb, Vec::<iroha_data_model::peer::PeerId>::new());
    drop(sb);
    (events, state)
}

#[allow(clippy::too_many_lines)]
fn main() {
    // 1) Mint/Burn/Transfer
    let chain_id = ChainId::from("chain");
    let (alice_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
    let rose: AssetDefinitionId = "rose#wonderland".parse().unwrap();
    let a_coin = AssetId::of(rose.clone(), alice_id.clone());
    let b_coin = AssetId::of(rose.clone(), bob_id.clone());
    let tx_mint = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Mint::asset_numeric(7_u32, a_coin.clone())])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_burn = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Burn::asset_numeric(3_u32, b_coin.clone())])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_xfer = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Transfer::asset_numeric(
            a_coin.clone(),
            5_u32,
            bob_id.clone(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let (events_seq, _state_seq) = run_block_and_events(
        false,
        vec![tx_mint.clone(), tx_burn.clone(), tx_xfer.clone()],
    );
    let (events_par, _state_par) = run_block_and_events(true, vec![tx_mint, tx_burn, tx_xfer]);
    write_fixture("mint_burn_transfer", &events_json_filtered(&events_seq));
    write_fixture("mint_burn_transfer", &events_json_filtered(&events_par));

    // 2) KV + NFT lifecycle
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let nft_id: NftId = "n0$wonderland".parse().unwrap();
    let tx_acc_set = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([SetKeyValue::account(
            alice_id.clone(),
            "k1".parse().unwrap(),
            iroha_primitives::json::Json::new(1u32),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_dom_set = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([SetKeyValue::domain(
            domain_id.clone(),
            "dk".parse().unwrap(),
            iroha_primitives::json::Json::new(3u32),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_nft_reg = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Register::nft(Nft::new(nft_id.clone(), Metadata::default()))])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_nft_set = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([SetKeyValue::nft(
            nft_id.clone(),
            "nk".parse().unwrap(),
            iroha_primitives::json::Json::new("v"),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_nft_xfer = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Transfer::nft(
            alice_id.clone(),
            nft_id.clone(),
            bob_id.clone(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_nft_rm = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([RemoveKeyValue::nft(nft_id.clone(), "nk".parse().unwrap())])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_nft_unreg = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Unregister::nft(nft_id.clone())])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_acc_rm = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([RemoveKeyValue::account(
            alice_id.clone(),
            "k1".parse().unwrap(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_dom_rm = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([RemoveKeyValue::domain(
            domain_id.clone(),
            "dk".parse().unwrap(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let txs = vec![
        tx_acc_set,
        tx_dom_set,
        tx_nft_reg,
        tx_nft_set,
        tx_nft_xfer,
        tx_nft_rm,
        tx_nft_unreg,
        tx_acc_rm,
        tx_dom_rm,
    ];
    let (events_seq, _) = run_block_and_events(false, txs.clone());
    let (events_par, _) = run_block_and_events(true, txs);
    write_fixture("kv_and_nft_lifecycle", &events_json_filtered(&events_seq));
    write_fixture("kv_and_nft_lifecycle", &events_json_filtered(&events_par));

    // 3) Asset definition KV set/remove
    let ad: AssetDefinitionId = "rose#wonderland".parse().unwrap();
    let tx_set = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([SetKeyValue::asset_definition(
            ad.clone(),
            "spec".parse().unwrap(),
            iroha_primitives::json::Json::new("golden"),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_rm = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([RemoveKeyValue::asset_definition(
            ad.clone(),
            "spec".parse().unwrap(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let (events_seq, _) = run_block_and_events(false, vec![tx_set.clone(), tx_rm.clone()]);
    let (events_par, _) = run_block_and_events(true, vec![tx_set, tx_rm]);
    write_fixture("asset_definition_kv", &events_json_filtered(&events_seq));
    write_fixture("asset_definition_kv", &events_json_filtered(&events_par));

    // 4) Owner transfers (domain + asset definition)
    let tx_dom_xfer = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Transfer::domain(
            alice_id.clone(),
            domain_id.clone(),
            bob_id.clone(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let tx_ad_xfer = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Transfer::asset_definition(
            alice_id.clone(),
            ad.clone(),
            bob_id.clone(),
        )])
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let (events_seq, _state_seq) =
        run_block_and_events(false, vec![tx_dom_xfer.clone(), tx_ad_xfer.clone()]);
    let (events_par, _state_par) = run_block_and_events(true, vec![tx_dom_xfer, tx_ad_xfer]);
    write_fixture(
        "owner_transfer_domain_asset_def",
        &events_json_filtered(&events_seq),
    );
    write_fixture(
        "owner_transfer_domain_asset_def",
        &events_json_filtered(&events_par),
    );
}
