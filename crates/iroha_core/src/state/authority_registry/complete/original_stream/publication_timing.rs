//! Paired native execution, certification and publication timings under the two commitment profiles.
use std::time::{Duration, Instant};

use crate::{
    state::{World, WorldReadOnly},
    sumeragi::{
        finality::{build_checkpoint, build_proof},
        test_chain::{CertifiedTestChain, TestChainConfig},
    },
};
use iroha_core_privacy::ironwood_state_rows::{
    IronwoodPoolContextV1, IronwoodStateSchemaV1, ironwood_pool_rows,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    Registrable,
    account::{Account, AccountId},
    asset::{AssetBalancePolicy, AssetBalanceScope, AssetDefinition, AssetDefinitionId},
    domain::Domain,
    isi::privacy::BootstrapPrivacyIronwoodPoolV1,
    privacy::{PrivacyIronwoodPoolBootstrapV1, PrivacyPoolIdV1},
    sumeragi_finality::{keyed_state::StateKeyV1, verify_checkpoint_page},
};
use iroha_model_base::domain::DomainId;

struct Measurement {
    commit: Duration,
    read: Option<(Duration, usize)>,
}

fn measure(keyed: bool, existing_rows: usize) -> Vec<Measurement> {
    let payer = AccountId::new(
        KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let reserve = AccountId::new(
        KeyPair::from_seed(vec![0x42; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let domain = DomainId::try_new("bench", "universal").unwrap();
    let asset = AssetDefinitionId::derive_from_components(domain.clone(), "clock".parse().unwrap());
    let pool = PrivacyPoolIdV1::new([0x81; 32]);
    let mut world = World::with(
        [Domain::new(domain).build(&payer)],
        [
            Account::new(payer.clone()).build(&payer),
            Account::new(reserve.clone()).build(&payer),
        ],
        [
            AssetDefinition::numeric(asset.clone(), "measure", AssetBalancePolicy::Global, None)
                .build(&payer),
        ],
    );
    {
        let mut rows = world.smart_contract_state.block();
        for i in 0..existing_rows {
            let key = format!("timing/existing/{i:08}").parse().unwrap();
            rows.insert(key, vec![0x4a; 128]);
        }
        rows.commit();
    }
    crate::runtime::ironwood::install_for_test(&mut world);
    let schema = keyed.then(|| {
        crate::runtime::keyed_state::install_for_test(&mut world);
        let view = world.view();
        let selection = view.runtime_selection();
        let digest = crate::runtime::keyed_state::schema_digest(selection).unwrap();
        let schema = iroha_data_model::sumeragi_finality::SelectedStateSchemaV1::from_selection(
            selection,
            &crate::runtime::release_catalog().unwrap().definitions,
        )
        .unwrap();
        IronwoodStateSchemaV1::from_selected(schema, digest).unwrap()
    });
    let mut config = TestChainConfig::new(world, 5_000);
    config.genesis_instructions.push(
        BootstrapPrivacyIronwoodPoolV1::new(
            PrivacyIronwoodPoolBootstrapV1::new(
                pool,
                asset.clone(),
                AssetBalanceScope::Global,
                reserve,
            )
            .unwrap(),
        )
        .into(),
    );
    let mut chain = CertifiedTestChain::start(config).unwrap();
    {
        use crate::state::StorageReadOnly;
        let view = chain.state().view();
        let seeded = view
            .world()
            .smart_contract_state()
            .iter()
            .filter(|(path, _)| path.as_ref().starts_with("timing/existing/"))
            .count();
        assert_eq!(
            seeded, existing_rows,
            "benchmark ledger must retain all seeded rows"
        );
    }
    let budget = iroha_allocation::AllocationBudget::new(64 << 20);
    let mut measurements = Vec::new();
    for height in 2..=12 {
        let start = Instant::now();
        chain.commit(Vec::new()); // The fixture signs nonempty clock work.
        let commit = start.elapsed();
        let tip = chain.committed(height);
        let checkpoint = build_checkpoint(&chain.state().view(), height).unwrap();
        let frame = build_proof(&chain.state().view(), height).unwrap();
        let page = verify_checkpoint_page(chain.network_id(), &checkpoint, &[frame], 64, 16 << 20)
            .unwrap();
        let read = schema.as_ref().map(|schema| {
            let start = Instant::now();
            let fetch = |rows: Vec<StateKeyV1>| {
                let rows: Vec<_> = rows.into_iter().map(|row| (row.table, row.key)).collect();
                let funded = chain
                    .state()
                    .keyed_state_rows(&tip, &rows, &budget)
                    .unwrap();
                let wire = norito::encode_canonical(&*funded).unwrap();
                drop(funded);
                let proof: iroha_data_model::sumeragi_finality::KeyedStateRowsV1 =
                    norito::decode_canonical_for_admission(
                        &wire,
                        norito::canonical_decode_limits(wire.len()),
                    )
                    .unwrap();
                (proof, wire.len())
            };
            let (first, first_bytes) = fetch(ironwood_pool_rows(pool).unwrap());
            let context =
                IronwoodPoolContextV1::verify(page.tip(), schema, pool, &asset, &payer, &first)
                    .unwrap();
            let (second, second_bytes) = fetch(context.remaining_rows().unwrap());
            context.verify_balances(&second).unwrap();
            (start.elapsed(), first_bytes + second_bytes)
        });
        if height > 2 {
            measurements.push(Measurement { commit, read });
        }
    }
    measurements
}

#[test]
#[ignore = "paired native State execution and read-path benchmark"]
fn certified_ironwood_state_publication_and_read_timing() {
    let median = |mut samples: Vec<u128>| {
        samples.sort_unstable();
        (samples[(samples.len() - 1) / 2] + samples[samples.len() / 2]) / 2
    };
    // Repeat in reverse order to balance warm code and allocator effects.
    let existing_rows = std::env::var("IROHA_STATE_TIMING_ROWS").map_or(0, |rows| {
        rows.parse::<usize>().expect("numeric benchmark row count")
    });
    let mut baseline = measure(false, existing_rows);
    let mut keyed = measure(true, existing_rows);
    keyed.extend(measure(true, existing_rows));
    baseline.extend(measure(false, existing_rows));
    for (name, samples) in [("LtHash", baseline), ("keyed", keyed)] {
        eprintln!(
            "{name}, existing contract rows={existing_rows}, n={}, median full block {}us",
            samples.len(),
            median(samples.iter().map(|s| s.commit.as_micros()).collect()),
        );
        let reads: Vec<_> = samples.iter().filter_map(|sample| sample.read).collect();
        if !reads.is_empty() {
            eprintln!(
                "median certified pool read {}us, median response {} bytes",
                median(reads.iter().map(|(time, _)| time.as_micros()).collect()),
                median(reads.iter().map(|(_, size)| *size as u128).collect())
            );
        }
    }
}
