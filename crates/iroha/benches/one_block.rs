#![expect(missing_docs)]

use criterion::{criterion_group, criterion_main, Criterion};
use iroha::{
    client::Client,
    data_model::{parameter::BlockParameter, prelude::*},
};
use iroha_test_network::*;
use iroha_test_samples::{load_sample_wasm, ALICE_ID, BOB_ID};
use nonzero_ext::nonzero;

const N_TRANSACTIONS_PER_BLOCK: u64 = 1;

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("one_block");
    group.sample_size(10);
    group.bench_function("trigger_executable_builtin", |b| {
        b.iter_batched(setup_builtin, routine, criterion::BatchSize::SmallInput);
    });
    group.bench_function("trigger_executable_wasm", |b| {
        b.iter_batched(setup_wasm, routine, criterion::BatchSize::SmallInput);
    });
    group.finish();
}

fn setup_builtin() -> Input {
    let rose: AssetDefinitionId = "rose#wonderland".parse().unwrap();
    let rose_alice: AssetId = format!("{rose}#{}", ALICE_ID.clone()).parse().unwrap();
    let transfer_rose_alice_bob = Transfer::asset_numeric(rose_alice.clone(), 1u32, BOB_ID.clone());
    setup(vec![transfer_rose_alice_bob])
}

fn setup_wasm() -> Input {
    setup(load_sample_wasm("trigger_transfer_one"))
}

/// Given a test network equipped with a trigger
fn setup(trigger_executable: impl Into<Executable>) -> Input {
    let rose: AssetDefinitionId = "rose#wonderland".parse().unwrap();
    let rose_alice: AssetId = format!("{rose}#{}", ALICE_ID.clone()).parse().unwrap();
    let rose_bob: AssetId = format!("{rose}#{}", BOB_ID.clone()).parse().unwrap();
    let register_trigger = Register::trigger(Trigger::new(
        "transfer_one_to_bob_on_mint_roses_at_alice"
            .parse()
            .unwrap(),
        Action::new(
            trigger_executable,
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            AssetEventFilter::new()
                .for_asset(rose_alice.clone())
                .for_events(AssetEventSet::Created),
        ),
    ));
    let (network, rt) = NetworkBuilder::new()
        .with_genesis_instruction(register_trigger)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(N_TRANSACTIONS_PER_BLOCK)),
        )))
        .start_blocking()
        .unwrap();
    let mut test_client = network.client();
    test_client.add_transaction_nonce = true;
    let n0_rose_alice = get_asset_value(&test_client, rose_alice.clone());
    let n0_rose_bob = get_asset_value(&test_client, rose_bob.clone());
    let mint_rose_alice = Mint::asset_numeric(1u32, rose_alice.clone());

    Input {
        network,
        rt,
        test_client,
        rose_alice,
        rose_bob,
        n0_rose_alice,
        n0_rose_bob,
        mint_rose_alice,
    }
}

struct Input {
    network: Network,
    rt: tokio::runtime::Runtime,
    test_client: Client,
    rose_alice: AssetId,
    rose_bob: AssetId,
    n0_rose_alice: Numeric,
    n0_rose_bob: Numeric,
    mint_rose_alice: Mint<Numeric, Asset>,
}

/// # Scenario
///
/// 0. Transaction: [mint a rose for Alice, mint another rose for Alice]
/// 0. Trigger execution: asset created (some roses for Alice) -> transfer a rose from Alice to Bob
fn routine(
    Input {
        network: _network,
        rt: _rt,
        test_client,
        rose_alice,
        rose_bob,
        n0_rose_alice,
        n0_rose_bob,
        mint_rose_alice,
    }: Input,
) {
    let mint_twice = [mint_rose_alice.clone(), mint_rose_alice];
    #[allow(clippy::reversed_empty_ranges)]
    for _ in 1..N_TRANSACTIONS_PER_BLOCK {
        // Transaction nonce is enabled in setup, otherwise hashes may collide
        test_client
            .submit_all(mint_twice.clone())
            .expect("transaction should be submitted");
    }
    test_client
        .submit_all_blocking(mint_twice)
        .expect("transaction should be committed");
    // TODO peer.once_block(2)
    assert_eq!(
        test_client.get_status().unwrap().blocks,
        2,
        "Extra blocks created"
    );

    let n1_rose_alice = get_asset_value(&test_client, rose_alice);
    let n1_rose_bob = get_asset_value(&test_client, rose_bob);

    // FIXME
    // assert_eq!(
    //     n1_rose_alice,
    //     n0_rose_alice.checked_add(N_TRANSACTIONS_PER_BLOCK.into()).unwrap()
    // );
    // assert_eq!(n1_rose_bob, n0_rose_bob.checked_add(N_TRANSACTIONS_PER_BLOCK.into()).unwrap());
    assert_eq!(
        n1_rose_alice,
        n0_rose_alice
            .checked_add(Numeric::from(2 * N_TRANSACTIONS_PER_BLOCK))
            .unwrap()
    );
    assert_eq!(n1_rose_bob, n0_rose_bob);
}

fn get_asset_value(client: &Client, asset_id: AssetId) -> Numeric {
    let Ok(asset) = client
        .query(FindAssets::new())
        .filter_with(|asset| asset.id.eq(asset_id))
        .execute_single()
    else {
        return Numeric::ZERO;
    };

    *asset.value()
}

criterion_group!(benches, bench);
criterion_main!(benches);
