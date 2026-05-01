//! Criterion benchmarks for Torii hot paths.

use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use criterion::{BatchSize, BenchmarkId, Criterion};
use iroha_config::parameters::actual::TelemetryProfile;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
    telemetry::{StateTelemetry, Telemetry},
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    SignedQuery,
    prelude::*,
    query::{QueryRequest, SingularQueryBox, executor::prelude::FindParameters},
};
use iroha_logger::Level;
use iroha_telemetry::metrics::Metrics;
use iroha_torii::{
    BenchRateLimiter, MaybeTelemetry, NoritoQuery, QueryOptions, ResponseFormat,
    accept_transaction_for_ingress_for_bench, handle_queries_with_opts,
    handle_transaction_with_metrics_for_bench, profile_stats::print_profile,
    verify_signed_query_request_for_bench,
};

fn direct_metrics_telemetry() -> MaybeTelemetry {
    let metrics = Arc::new(Metrics::default());
    let telemetry = Telemetry::from(StateTelemetry::new(metrics, true));
    MaybeTelemetry::from_profile(Some(telemetry), TelemetryProfile::Operator)
}

fn signed_find_parameters(key_pair: &KeyPair) -> SignedQuery {
    let authority = AccountId::new(key_pair.public_key().clone());
    QueryRequest::Singular(SingularQueryBox::FindParameters(FindParameters))
        .with_authority(authority)
        .sign(key_pair)
}

fn bench_signed_query_verify(c: &mut Criterion) {
    let key_pair = KeyPair::random();
    c.bench_function("torii_signed_query_verify_find_parameters", |b| {
        b.iter_batched(
            || signed_find_parameters(&key_pair),
            |signed_query| {
                let verified =
                    verify_signed_query_request_for_bench(signed_query).expect("query verifies");
                std::hint::black_box(verified);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_query_find_parameters(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let key_pair = KeyPair::random();
    let query_store = LiveQueryStore::start_test();
    let query_state = Arc::new(State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        query_store.clone(),
    ));
    let telemetry = direct_metrics_telemetry();

    c.bench_function("torii_query_find_parameters_norito", |b| {
        b.iter_batched(
            || signed_find_parameters(&key_pair),
            |signed_query| {
                let response = runtime
                    .block_on(handle_queries_with_opts(
                        query_store.clone(),
                        Arc::clone(&query_state),
                        signed_query,
                        telemetry.clone(),
                        NoritoQuery(QueryOptions::default()),
                        ResponseFormat::Norito,
                    ))
                    .expect("query completes");
                std::hint::black_box(response);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_transaction_admission(c: &mut Criterion) {
    let chain_id: Arc<ChainId> = Arc::new(
        "torii_hot_path_bench_chain"
            .parse()
            .expect("valid chain id"),
    );
    let tx_state = Arc::new(State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    let tx_key_pair = KeyPair::random();
    let tx_authority = AccountId::new(tx_key_pair.public_key().clone());
    let telemetry = direct_metrics_telemetry();
    let counter = AtomicUsize::new(0);

    c.bench_function("torii_transaction_admission_direct_metrics", |b| {
        b.iter_batched(
            || {
                let index = counter.fetch_add(1, Ordering::Relaxed);
                let instruction = Log::new(Level::INFO, format!("torii-hot-path-bench-{index}"));
                TransactionBuilder::new(chain_id.as_ref().clone(), tx_authority.clone())
                    .with_instructions([InstructionBox::from(instruction)])
                    .sign(tx_key_pair.private_key())
            },
            |tx| {
                let accepted = accept_transaction_for_ingress_for_bench(
                    Arc::clone(&chain_id),
                    Arc::clone(&tx_state),
                    tx,
                    &telemetry,
                )
                .expect("transaction admission succeeds");
                std::hint::black_box(accepted.hash());
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_transaction_handle_enqueue(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let chain_id: Arc<ChainId> = Arc::new(
        "torii_hot_path_enqueue_bench_chain"
            .parse()
            .expect("valid chain id"),
    );
    let tx_state = Arc::new(State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    let tx_key_pair = KeyPair::random();
    let tx_authority = AccountId::new(tx_key_pair.public_key().clone());
    let telemetry = direct_metrics_telemetry();
    let counter = AtomicUsize::new(0);
    let queue_capacity = NonZeroUsize::new(1_000_000).expect("non-zero queue capacity");
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: queue_capacity,
        capacity_per_user: queue_capacity,
        transaction_time_to_live: Duration::from_secs(60),
        ..Default::default()
    };

    c.bench_function("torii_transaction_handle_enqueue_direct_metrics", |b| {
        b.iter_batched(
            || {
                let index = counter.fetch_add(1, Ordering::Relaxed);
                let instruction =
                    Log::new(Level::INFO, format!("torii-hot-path-enqueue-bench-{index}"));
                let tx = TransactionBuilder::new(chain_id.as_ref().clone(), tx_authority.clone())
                    .with_instructions([InstructionBox::from(instruction)])
                    .sign(tx_key_pair.private_key());
                let (events, _) = tokio::sync::broadcast::channel(queue_capacity.get());
                let queue = Arc::new(Queue::from_config(queue_cfg, events));
                (queue, tx)
            },
            |(queue, tx)| {
                let decision = runtime
                    .block_on(handle_transaction_with_metrics_for_bench(
                        Arc::clone(&chain_id),
                        queue,
                        Arc::clone(&tx_state),
                        tx,
                        telemetry.clone(),
                        iroha_torii_shared::uri::TRANSACTION,
                    ))
                    .expect("transaction handle succeeds");
                std::hint::black_box(decision);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_transaction_enqueue_sustained_pressure(c: &mut Criterion) {
    let chain_id: Arc<ChainId> = Arc::new(
        "torii_hot_path_sustained_enqueue_bench_chain"
            .parse()
            .expect("valid chain id"),
    );
    let tx_state = Arc::new(State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    let tx_key_pair = KeyPair::random();
    let tx_authority = AccountId::new(tx_key_pair.public_key().clone());
    let telemetry = direct_metrics_telemetry();
    let counter = AtomicUsize::new(0);
    let queue_capacity = NonZeroUsize::new(1_000_000).expect("non-zero queue capacity");
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: queue_capacity,
        capacity_per_user: queue_capacity,
        transaction_time_to_live: Duration::from_secs(600),
        ..Default::default()
    };

    let make_tx = |index: usize| {
        let instruction = Log::new(
            Level::INFO,
            format!("torii-hot-path-sustained-enqueue-bench-{index}"),
        );
        TransactionBuilder::new(chain_id.as_ref().clone(), tx_authority.clone())
            .with_instructions([InstructionBox::from(instruction)])
            .sign(tx_key_pair.private_key())
    };

    let mut group = c.benchmark_group("torii_transaction_enqueue_sustained_pressure");
    for backlog in [1_000usize, 10_000, 20_000] {
        let (events, _) = tokio::sync::broadcast::channel(queue_capacity.get());
        let queue = Arc::new(Queue::from_config(queue_cfg, events));

        for _ in 0..backlog {
            let index = counter.fetch_add(1, Ordering::Relaxed);
            let tx = make_tx(index);
            let accepted = accept_transaction_for_ingress_for_bench(
                Arc::clone(&chain_id),
                Arc::clone(&tx_state),
                tx,
                &telemetry,
            )
            .expect("prefill transaction admission succeeds");
            queue
                .push(accepted, tx_state.view())
                .expect("prefill enqueue succeeds");
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(backlog),
            &backlog,
            |b, _backlog| {
                b.iter_batched(
                    || {
                        let index = counter.fetch_add(1, Ordering::Relaxed);
                        make_tx(index)
                    },
                    |tx| {
                        let accepted = accept_transaction_for_ingress_for_bench(
                            Arc::clone(&chain_id),
                            Arc::clone(&tx_state),
                            tx,
                            &telemetry,
                        )
                        .expect("transaction admission succeeds");
                        queue
                            .push(accepted, tx_state.view())
                            .expect("sustained enqueue succeeds");

                        let mut guards = Vec::new();
                        queue.get_transactions_for_block(
                            &tx_state.view(),
                            NonZeroUsize::new(1).expect("non-zero block limit"),
                            &mut guards,
                        );

                        std::hint::black_box(());
                        std::hint::black_box(queue.pressure_snapshot());
                        drop(guards);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn bench_rate_limiter(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let distinct_limiter = BenchRateLimiter::new(Some(1_000_000), Some(1_000_000));
    let distinct_counter = AtomicUsize::new(0);
    c.bench_function("torii_rate_limiter_distinct_key", |b| {
        b.iter_batched(
            || {
                let index = distinct_counter.fetch_add(1, Ordering::Relaxed);
                format!("torii-hot-path-bench-{index}")
            },
            |key| {
                let allowed = runtime.block_on(distinct_limiter.allow(&key));
                std::hint::black_box(allowed);
            },
            BatchSize::SmallInput,
        );
    });

    let same_key_limiter = BenchRateLimiter::new(Some(1_000_000), Some(1_000_000));
    c.bench_function("torii_rate_limiter_same_key", |b| {
        b.iter(|| {
            let allowed = runtime.block_on(same_key_limiter.allow("torii-hot-path-bench-shared"));
            std::hint::black_box(allowed);
        });
    });
}

fn smoke_profile_output() {
    let mut samples = Vec::with_capacity(8);
    for micros in 1..=8 {
        samples.push(Duration::from_micros(micros));
    }
    print_profile(
        "criterion_smoke",
        "profile_stats_output_shape",
        samples,
        0,
        NonZeroUsize::new(1).expect("non-zero").get(),
        Duration::from_millis(1),
    );
}

/// Entry point for the benchmark binary.
fn main() {
    smoke_profile_output();
    let mut c = Criterion::default().configure_from_args();
    bench_signed_query_verify(&mut c);
    bench_query_find_parameters(&mut c);
    bench_transaction_admission(&mut c);
    bench_transaction_handle_enqueue(&mut c);
    bench_transaction_enqueue_sustained_pressure(&mut c);
    bench_rate_limiter(&mut c);
    c.final_summary();
}
