//! Benchmarks for core query execution (e.g., FindAccounts) over varying state sizes.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::all)]
#![allow(
    clippy::doc_markdown,
    clippy::uninlined_format_args,
    clippy::field_reassign_with_default,
    clippy::items_after_statements
)]
use std::sync::LazyLock;

use criterion::Criterion;
use iroha_core::{
    prelude::*,
    query::snapshot::{CursorMode as LaneCursorMode, run_on_snapshot, run_on_snapshot_with_mode},
    query::store::LiveQueryStore,
    smartcontracts::{Execute, ValidQuery, isi::query::QueryLimits},
    state::{State, World},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    prelude::*,
    query::{
        account::prelude::FindAccounts,
        asset::prelude::{FindAssets, FindAssetsDefinitions},
        domain::prelude::FindDomains,
        dsl::CompoundPredicate,
        trigger::prelude::{FindActiveTriggerIds, FindTriggers},
    },
};

// Shared Tokio runtime for benches that need background tasks (e.g., LiveQueryStore)
static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the Runtime")
});

fn fixture_account_in_domain(label: &str, _domain_id: &DomainId) -> AccountId {
    let seed: Vec<u8> = label.as_bytes().iter().copied().cycle().take(32).collect();
    let (public_key, _) = KeyPair::from_seed(seed, Algorithm::Ed25519).into_parts();
    AccountId::new(public_key)
}

fn bench_domain_id() -> DomainId {
    "bench".parse().expect("bench domain id")
}

fn bench_account(label: &str) -> AccountId {
    fixture_account_in_domain(label, &bench_domain_id())
}

fn build_state_with_accounts(n: usize) -> State {
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    // Ensure Tokio reactor is available for LiveQueryStore background task
    let _guard = RUNTIME.enter();
    let query_handle = LiveQueryStore::start_test();

    let domain_id = bench_domain_id();
    let authority_id = bench_account("authority");
    let domain = Domain::new(domain_id.clone()).build(&authority_id);

    let mut accounts = Vec::with_capacity(n);
    for i in 0..n {
        let acc_id = bench_account(&format!("user{i}"));
        // Use the account itself as the authority for building
        let account = Account::new(acc_id.to_account_id(domain_id.clone())).build(&acc_id);
        accounts.push(account);
    }

    State::new(
        World::with([domain], accounts, []),
        kura,
        query_handle,
        #[cfg(feature = "telemetry")]
        <_>::default(),
    )
}

fn bench_find_accounts_small(c: &mut Criterion) {
    let state = build_state_with_accounts(1_000);
    c.bench_function("find_accounts_iter_1k", |b| {
        b.iter(|| {
            let state_view = state.view();
            let iter = ValidQuery::execute(FindAccounts, CompoundPredicate::PASS, &state_view)
                .expect("query execute");
            let count = iter.count();
            std::hint::black_box(count);
        })
    });
}

fn bench_find_accounts_large(c: &mut Criterion) {
    let state = build_state_with_accounts(10_000);
    c.bench_function("find_accounts_iter_10k", |b| {
        b.iter(|| {
            let state_view = state.view();
            let iter = ValidQuery::execute(FindAccounts, CompoundPredicate::PASS, &state_view)
                .expect("query execute");
            let count = iter.count();
            std::hint::black_box(count);
        })
    });
}

fn bench_find_accounts_sort_id(c: &mut Criterion) {
    let state = build_state_with_accounts(10_000);
    c.bench_function("find_accounts_sort_by_id_10k", |b| {
        b.iter(|| {
            let v = state.view();
            let iter = ValidQuery::execute(FindAccounts, CompoundPredicate::PASS, &v)
                .expect("query execute");
            let mut vec: Vec<_> = iter.collect();
            vec.sort_by(|a, b| a.id().cmp(b.id()));
            std::hint::black_box(vec.len());
        })
    });
}

fn bench_find_accounts_paginate(c: &mut Criterion) {
    let state = build_state_with_accounts(10_000);
    let page = 100usize;
    c.bench_function("find_accounts_paginate_10k_page_100", |b| {
        b.iter(|| {
            let v = state.view();
            let iter = ValidQuery::execute(FindAccounts, CompoundPredicate::PASS, &v)
                .expect("query execute");
            let items: Vec<_> = iter.collect();
            let mut pages = 0usize;
            let mut idx = 0usize;
            while idx < items.len() {
                let end = (idx + page).min(items.len());
                let _page_slice = &items[idx..end];
                pages += 1;
                idx = end;
            }
            std::hint::black_box(pages);
        })
    });
}

fn bench_snapshot_vs_live_find_domains_first_batch(c: &mut Criterion) {
    // Build world with 10k domains
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let _guard = RUNTIME.enter();
    let query_handle = LiveQueryStore::start_test();
    let authority_id = bench_account("authority");
    let mut domains = Vec::with_capacity(10_000);
    for i in 0..10_000 {
        let id: DomainId = format!("d{}", i).parse().unwrap();
        domains.push(Domain::new(id).build(&authority_id));
    }
    let state = State::new_for_testing(
        World::with(
            domains,
            [
                Account::new(authority_id.to_account_id("d0".parse().unwrap()))
                    .build(&authority_id),
            ],
            [],
        ),
        kura,
        query_handle.clone(),
    );

    // Build erased iterable FindDomains with small fetch_size
    let params = iroha_data_model::query::parameters::QueryParams::default();
    let payload =
        norito::codec::Encode::encode(&iroha_data_model::query::domain::prelude::FindDomains);
    let erased = iroha_data_model::query::ErasedIterQuery::<Domain>::new(
        iroha_data_model::query::dsl::CompoundPredicate::PASS,
        iroha_data_model::query::dsl::SelectorTuple::default(),
        payload,
    );
    let qbox: iroha_data_model::query::QueryBox<_> = Box::new(erased);

    // Live (baseline): just execute ValidQuery and take first batch materialization cost
    c.bench_function("live_find_domains_first_batch", |b| {
        b.iter(|| {
            let v = state.view();
            let iter = ValidQuery::execute(
                iroha_data_model::query::domain::prelude::FindDomains,
                iroha_data_model::query::dsl::CompoundPredicate::PASS,
                &v,
            )
            .expect("query execute");
            // Materialize first 100
            let count = iter.take(100).count();
            std::hint::black_box(count);
        })
    });

    // Snapshot ephemeral
    c.bench_function("snapshot_ephemeral_find_domains_first_batch", |b| {
        b.iter(|| {
            let request = iroha_data_model::query::QueryRequest::Start(
                iroha_data_model::query::QueryWithParams::new(&qbox, params.clone()),
            );
            let resp = run_on_snapshot(
                &state,
                &query_handle,
                &authority_id,
                request,
                QueryLimits::default(),
            )
            .expect("lane ok");
            let iroha_data_model::query::QueryResponse::Iterable(first) = resp else {
                panic!("expected iterable")
            };
            let (batch, _rem, _cur) = first.into_parts();
            let v = match batch.into_iter().next().expect("slice") {
                iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
                _ => unreachable!(),
            };
            std::hint::black_box(v.len());
        })
    });

    // Snapshot stored (same first-batch measurement)
    c.bench_function("snapshot_stored_find_domains_first_batch", |b| {
        b.iter(|| {
            let request = iroha_data_model::query::QueryRequest::Start(
                iroha_data_model::query::QueryWithParams::new(&qbox, params.clone()),
            );
            let resp = run_on_snapshot_with_mode(
                &state,
                &query_handle,
                &authority_id,
                request,
                LaneCursorMode::Stored,
                QueryLimits::default(),
            )
            .expect("lane ok");
            let iroha_data_model::query::QueryResponse::Iterable(first) = resp else {
                panic!("expected iterable")
            };
            let (batch, _rem, _cur) = first.into_parts();
            let v = match batch.into_iter().next().expect("slice") {
                iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
                _ => unreachable!(),
            };
            std::hint::black_box(v.len());
        })
    });
}

fn bench_snapshot_vs_live_find_assets_first_batch(c: &mut Criterion) {
    use iroha_data_model::query::asset::prelude::FindAssets;
    // Build world: 1k accounts, each with one asset
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let _guard = RUNTIME.enter();
    let query_handle = LiveQueryStore::start_test();
    let domain_id: DomainId = "bench".parse().unwrap();
    let asset_def_id: AssetDefinitionId = "coin#bench".parse().unwrap();
    let mut accounts = Vec::with_capacity(1_000);
    let mut assets: Vec<iroha_data_model::asset::Asset> = Vec::with_capacity(1_000);
    for i in 0..1_000 {
        let acc_id = bench_account(&format!("user{i}"));
        let acc = Account::new(acc_id.to_account_id(domain_id.clone())).build(&acc_id);
        accounts.push(acc);
        let asset_id = AssetId::new(asset_def_id.clone(), acc_id.clone());
        assets.push(iroha_data_model::asset::Asset::new(
            asset_id,
            iroha_primitives::numeric::Numeric::from(1_u32),
        ));
    }
    let domain = Domain::new(domain_id).build(&accounts[0].id().clone());
    let world = World::with_assets(
        [domain],
        accounts.clone(),
        [AssetDefinition::numeric(asset_def_id.clone()).build(&accounts[0].id().clone())],
        assets,
        [],
    );
    let state = State::new_for_testing(world, kura, query_handle.clone());

    // Build erased iterable FindAssets
    let params = iroha_data_model::query::parameters::QueryParams::default();
    let payload = norito::codec::Encode::encode(&FindAssets);
    let erased = iroha_data_model::query::ErasedIterQuery::<Asset>::new(
        iroha_data_model::query::dsl::CompoundPredicate::PASS,
        iroha_data_model::query::dsl::SelectorTuple::default(),
        payload,
    );
    let qbox: iroha_data_model::query::QueryBox<_> = Box::new(erased);

    // Live baseline
    c.bench_function("live_find_assets_first_batch", |b| {
        b.iter(|| {
            let v = state.view();
            let iter = ValidQuery::execute(
                FindAssets,
                iroha_data_model::query::dsl::CompoundPredicate::PASS,
                &v,
            )
            .expect("query execute");
            std::hint::black_box(iter.take(100).count());
        })
    });
    // Snapshot ephemeral vs stored
    let authority = bench_account("authority");
    c.bench_function("snapshot_ephemeral_find_assets_first_batch", |b| {
        b.iter(|| {
            let request = iroha_data_model::query::QueryRequest::Start(
                iroha_data_model::query::QueryWithParams::new(&qbox, params.clone()),
            );
            let resp = run_on_snapshot(
                &state,
                &query_handle,
                &authority,
                request,
                QueryLimits::default(),
            )
            .expect("lane ok");
            let iroha_data_model::query::QueryResponse::Iterable(first) = resp else {
                panic!("expected iterable")
            };
            let (batch, _rem, _cur) = first.into_parts();
            let v = match batch.into_iter().next().expect("slice") {
                iroha_data_model::query::QueryOutputBatchBox::Asset(v) => v,
                _ => unreachable!(),
            };
            std::hint::black_box(v.len());
        })
    });
    c.bench_function("snapshot_stored_find_assets_first_batch", |b| {
        b.iter(|| {
            let request = iroha_data_model::query::QueryRequest::Start(
                iroha_data_model::query::QueryWithParams::new(&qbox, params.clone()),
            );
            let resp = run_on_snapshot_with_mode(
                &state,
                &query_handle,
                &authority,
                request,
                LaneCursorMode::Stored,
                QueryLimits::default(),
            )
            .expect("lane ok");
            let iroha_data_model::query::QueryResponse::Iterable(first) = resp else {
                panic!("expected iterable")
            };
            let (batch, _rem, _cur) = first.into_parts();
            let v = match batch.into_iter().next().expect("slice") {
                iroha_data_model::query::QueryOutputBatchBox::Asset(v) => v,
                _ => unreachable!(),
            };
            std::hint::black_box(v.len());
        })
    });
}

fn bench_snapshot_sorted_asset_defs_first_batch(c: &mut Criterion) {
    use iroha_data_model::query::asset::prelude::FindAssetsDefinitions;
    // Build world with 10k asset defs and rank metadata
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let _guard = RUNTIME.enter();
    let query_handle = LiveQueryStore::start_test();
    let auth = bench_account("authority");
    let domain = Domain::new("bench".parse().unwrap()).build(&auth);
    let mut defs = Vec::with_capacity(10_000);
    for i in 0..10_000 {
        let id: AssetDefinitionId = format!("ad{}#bench", i).parse().unwrap();
        let mut ad = AssetDefinition::numeric(id).build(&auth);
        let _ = ad.metadata_mut().insert(
            "rank".parse().unwrap(),
            iroha_primitives::json::Json::from(norito::json!(i % 100)),
        );
        defs.push(ad);
    }
    let world = World::with(
        [domain],
        [Account::new(auth.to_account_id("bench".parse().unwrap())).build(&auth)],
        defs,
    );
    // Provide default telemetry only when enabled; otherwise call 3-arg ctor
    let state = {
        #[cfg(feature = "telemetry")]
        {
            State::new(world, kura, query_handle.clone(), <_>::default())
        }
        #[cfg(not(feature = "telemetry"))]
        {
            State::new(world, kura, query_handle.clone())
        }
    };

    // Params with sorting by metadata key rank
    let mut params = iroha_data_model::query::parameters::QueryParams::default();
    params.sorting =
        iroha_data_model::query::parameters::Sorting::by_metadata_key("rank".parse().unwrap());
    let payload = norito::codec::Encode::encode(&FindAssetsDefinitions);
    let erased = iroha_data_model::query::ErasedIterQuery::<AssetDefinition>::new(
        iroha_data_model::query::dsl::CompoundPredicate::PASS,
        iroha_data_model::query::dsl::SelectorTuple::default(),
        payload,
    );
    let qbox: iroha_data_model::query::QueryBox<_> = Box::new(erased);

    c.bench_function("snapshot_ephemeral_sorted_asset_defs_first_batch", |b| {
        b.iter(|| {
            let request = iroha_data_model::query::QueryRequest::Start(
                iroha_data_model::query::QueryWithParams::new(&qbox, params.clone()),
            );
            let resp = run_on_snapshot(
                &state,
                &query_handle,
                &auth,
                request,
                QueryLimits::default(),
            )
            .expect("lane ok");
            let iroha_data_model::query::QueryResponse::Iterable(first) = resp else {
                panic!("expected iterable")
            };
            let (batch, _rem, _cur) = first.into_parts();
            let v = match batch.into_iter().next().expect("slice") {
                iroha_data_model::query::QueryOutputBatchBox::AssetDefinition(v) => v,
                _ => unreachable!(),
            };
            std::hint::black_box(v.len());
        })
    });
    c.bench_function("snapshot_stored_sorted_asset_defs_first_batch", |b| {
        b.iter(|| {
            let request = iroha_data_model::query::QueryRequest::Start(
                iroha_data_model::query::QueryWithParams::new(&qbox, params.clone()),
            );
            let resp = run_on_snapshot_with_mode(
                &state,
                &query_handle,
                &auth,
                request,
                LaneCursorMode::Stored,
                QueryLimits::default(),
            )
            .expect("lane ok");
            let iroha_data_model::query::QueryResponse::Iterable(first) = resp else {
                panic!("expected iterable")
            };
            let (batch, _rem, _cur) = first.into_parts();
            let v = match batch.into_iter().next().expect("slice") {
                iroha_data_model::query::QueryOutputBatchBox::AssetDefinition(v) => v,
                _ => unreachable!(),
            };
            std::hint::black_box(v.len());
        })
    });
}
fn build_state_with_assets(n_accounts: usize, assets_per_account: usize) -> State {
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let _guard = RUNTIME.enter();
    let query_handle = LiveQueryStore::start_test();

    let domain_id = bench_domain_id();
    let authority_id = bench_account("authority");
    let domain = Domain::new(domain_id.clone()).build(&authority_id);

    let asset_def_id: AssetDefinitionId = "coin#bench".parse().expect("asset def id");
    let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority_id);

    let mut accounts = Vec::with_capacity(n_accounts);
    let mut assets = Vec::with_capacity(n_accounts * assets_per_account);
    for i in 0..n_accounts {
        let acc_id = bench_account(&format!("user{i}"));
        let account = Account::new(acc_id.to_account_id(domain_id.clone())).build(&acc_id);
        for j in 0..assets_per_account {
            let ad = if j == 0 {
                asset_def_id.clone()
            } else {
                // create a handful of distinct definitions to vary lookups
                format!("coin{}_#bench", j)
                    .parse()
                    .unwrap_or_else(|_| asset_def_id.clone())
            };
            let asset_id = AssetId::new(ad, acc_id.clone());
            let value = Numeric::new(u128::from(j as u64 + 1), 0);
            assets.push(Asset::new(asset_id, value));
        }
        accounts.push(account);
    }

    State::new(
        World::with_assets([domain], accounts, [asset_def], assets, []),
        kura,
        query_handle,
        #[cfg(feature = "telemetry")]
        <_>::default(),
    )
}

fn bench_find_assets_iter(c: &mut Criterion) {
    let state = build_state_with_assets(5_000, 2);
    c.bench_function("find_assets_iter_10k", |b| {
        b.iter(|| {
            let v = state.view();
            let iter = ValidQuery::execute(FindAssets, CompoundPredicate::PASS, &v)
                .expect("query execute");
            let count = iter.count();
            std::hint::black_box(count);
        })
    });
}

fn bench_find_assets_filter_account(c: &mut Criterion) {
    let state = build_state_with_assets(10_000, 1);
    let target = bench_account("user9999");
    c.bench_function("find_assets_filter_by_account", |b| {
        b.iter(|| {
            let v = state.view();
            let iter = ValidQuery::execute(FindAssets, CompoundPredicate::PASS, &v)
                .expect("query execute");
            let count = iter.filter(|a| a.id().account() == &target).count();
            std::hint::black_box(count);
        })
    });
}

fn bench_find_assets_filter_quantity(c: &mut Criterion) {
    let state = build_state_with_assets(5_000, 2);
    c.bench_function("find_assets_filter_quantity_ge_2", |b| {
        b.iter(|| {
            let v = state.view();
            let iter = ValidQuery::execute(FindAssets, CompoundPredicate::PASS, &v)
                .expect("query execute");
            let count = iter.filter(|a| *a.value() >= Numeric::new(2, 0)).count();
            std::hint::black_box(count);
        })
    });
}

fn build_state_with_domains(n: usize) -> State {
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let _guard = RUNTIME.enter();
    let query_handle = LiveQueryStore::start_test();

    let authority_id = bench_account("authority");
    let mut domains = Vec::with_capacity(n);
    for i in 0..n {
        let id: DomainId = format!("d{}", i).parse().expect("domain id");
        domains.push(Domain::new(id).build(&authority_id));
    }

    State::new(
        World::with(domains, [], []),
        kura,
        query_handle,
        #[cfg(feature = "telemetry")]
        <_>::default(),
    )
}

fn bench_find_domains_iter(c: &mut Criterion) {
    let state = build_state_with_domains(5_000);
    c.bench_function("find_domains_iter_5k", |b| {
        b.iter(|| {
            let v = state.view();
            let iter = ValidQuery::execute(FindDomains, CompoundPredicate::PASS, &v)
                .expect("query execute");
            let count = iter.count();
            std::hint::black_box(count);
        })
    });
}

fn bench_find_domains_sort(c: &mut Criterion) {
    let state = build_state_with_domains(10_000);
    c.bench_function("find_domains_sort_by_id_10k", |b| {
        b.iter(|| {
            let v = state.view();
            let iter = ValidQuery::execute(FindDomains, CompoundPredicate::PASS, &v)
                .expect("query execute");
            let mut vec: Vec<_> = iter.collect();
            vec.sort_by(|a, b| a.id().cmp(b.id()));
            std::hint::black_box(vec.len());
        })
    });
}

fn build_state_with_asset_definitions(n: usize) -> State {
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let _guard = RUNTIME.enter();
    let query_handle = LiveQueryStore::start_test();

    let domain_id = bench_domain_id();
    let authority_id = bench_account("authority");
    let domain = Domain::new(domain_id.clone()).build(&authority_id);
    let owner = Account::new(authority_id.to_account_id(domain_id)).build(&authority_id);

    let mut defs = Vec::with_capacity(n);
    for i in 0..n {
        let def_id: AssetDefinitionId = format!("coin{}#bench", i).parse().expect("ad id");
        defs.push(AssetDefinition::numeric(def_id).build(&authority_id));
    }

    State::new(
        World::with_assets([domain], [owner], defs, [], []),
        kura,
        query_handle,
        #[cfg(feature = "telemetry")]
        <_>::default(),
    )
}

fn bench_find_asset_defs_iter(c: &mut Criterion) {
    let state = build_state_with_asset_definitions(10_000);
    c.bench_function("find_asset_defs_iter_10k", |b| {
        b.iter(|| {
            let v = state.view();
            let iter =
                ValidQuery::execute(FindAssetsDefinitions::new(), CompoundPredicate::PASS, &v)
                    .expect("query execute");
            let count = iter.count();
            std::hint::black_box(count);
        })
    });
}

fn build_state_with_triggers(n_time: usize, n_by_call: usize) -> State {
    use iroha_core::block::BlockBuilder;
    use iroha_data_model::{
        events::time::{ExecutionTime, TimeEventFilter},
        trigger::prelude::*,
    };

    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let _guard = RUNTIME.enter();
    let query_handle = LiveQueryStore::start_test();

    // Start with an empty world and then register domain/account to act as authority
    // Use `new_for_testing` to provide a telemetry instance when required
    let state = State::new_for_testing(World::default(), kura, query_handle);

    fn dummy_accepted_transaction() -> AcceptedTransaction<'static> {
        use std::{borrow::Cow, time::Duration};

        let chain_id: ChainId = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("valid chain id");
        let keypair = KeyPair::random();
        let authority = AccountId::new(keypair.public_key().clone());
        let mut builder = TransactionBuilder::new(chain_id, authority);
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Log::new(Level::INFO, "dummy".to_owned())])
            .sign(keypair.private_key());
        AcceptedTransaction::new_unchecked(Cow::Owned(tx))
    }

    // Create a header for a block to stage registrations
    let latest = state.view().latest_block();
    let priv_key = iroha_crypto::KeyPair::random().private_key().clone();
    let header = BlockBuilder::new(vec![dummy_accepted_transaction()])
        .chain(0, latest.as_deref())
        .sign(&priv_key)
        .unpack(|_| {})
        .header();

    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();

    // Authority
    let domain_id = bench_domain_id();
    let authority_id = bench_account("authority");
    Register::domain(Domain::new(domain_id.clone()))
        .execute(&authority_id, &mut stx)
        .expect("register domain");
    Register::account(Account::new(authority_id.to_account_id(domain_id)))
        .execute(&authority_id, &mut stx)
        .expect("register account");

    // Time triggers (PreCommit), minimal action body
    for i in 0..n_time {
        let trig_id: TriggerId = format!("time_{}", i).parse().unwrap();
        let t = Trigger::new(
            trig_id,
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Indefinitely,
                authority_id.clone(),
                TimeEventFilter::new(ExecutionTime::PreCommit),
            ),
        );
        Register::trigger(t)
            .execute(&authority_id, &mut stx)
            .expect("register time trigger");
    }

    // By-call triggers (no-op)
    use iroha_data_model::events::execute_trigger::ExecuteTriggerEventFilter;
    for i in 0..n_by_call {
        let trig_id: TriggerId = format!("call_{}", i).parse().unwrap();
        let t = Trigger::new(
            trig_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Indefinitely,
                authority_id.clone(),
                ExecuteTriggerEventFilter::new().for_trigger(trig_id),
            ),
        );
        Register::trigger(t)
            .execute(&authority_id, &mut stx)
            .expect("register by-call trigger");
    }

    stx.apply();
    state_block.commit().unwrap();

    state
}

fn bench_find_triggers_iter(c: &mut Criterion) {
    let state = build_state_with_triggers(5_000, 5_000);
    c.bench_function("find_triggers_iter_10k", |b| {
        b.iter(|| {
            let v = state.view();
            let iter = ValidQuery::execute(FindTriggers, CompoundPredicate::PASS, &v)
                .expect("query execute");
            let count = iter.count();
            std::hint::black_box(count);
        })
    });
}

fn bench_find_active_trigger_ids_iter(c: &mut Criterion) {
    let state = build_state_with_triggers(10_000, 0);
    c.bench_function("find_active_trigger_ids_iter_10k", |b| {
        b.iter(|| {
            let v = state.view();
            let iter = ValidQuery::execute(FindActiveTriggerIds, CompoundPredicate::PASS, &v)
                .expect("query execute");
            let count = iter.count();
            std::hint::black_box(count);
        })
    });
}

/// Entry point for the benchmark binary.
fn main() {
    // Silence IVM banner for benches that may trigger VM init in inner paths.
    #[allow(unused_imports)]
    {
        use ivm::set_banner_enabled;
        set_banner_enabled(false);
    }
    let mut c = Criterion::default().configure_from_args();
    bench_find_accounts_small(&mut c);
    bench_find_accounts_large(&mut c);
    bench_find_assets_iter(&mut c);
    bench_find_assets_filter_account(&mut c);
    bench_find_assets_filter_quantity(&mut c);
    bench_find_domains_iter(&mut c);
    bench_find_domains_sort(&mut c);
    bench_find_asset_defs_iter(&mut c);
    bench_find_triggers_iter(&mut c);
    bench_find_active_trigger_ids_iter(&mut c);
    bench_find_accounts_sort_id(&mut c);
    bench_find_accounts_paginate(&mut c);
    bench_snapshot_vs_live_find_domains_first_batch(&mut c);
    bench_snapshot_vs_live_find_assets_first_batch(&mut c);
    bench_snapshot_sorted_asset_defs_first_batch(&mut c);
    c.final_summary();
}
