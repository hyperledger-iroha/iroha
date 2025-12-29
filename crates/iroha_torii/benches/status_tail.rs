//! Benchmarks extracting tail values from telemetry status.
#![cfg(feature = "telemetry")]

use criterion::Criterion;
use iroha_telemetry::metrics::{Metrics, Status};
use norito::json::Value;

fn direct(status: &Status, tail: &str) -> Value {
    let mut segments = tail.split('/').filter(|s| !s.is_empty());
    match segments.next().unwrap() {
        "peers" => Value::from(status.peers),
        _ => Value::Null,
    }
}

fn via_norito(status: &Status, tail: &str) -> Value {
    // Use Norito's JSON wrappers to build a Value, then index by segments.
    let value = norito::json::to_value(&status).expect("to_value");
    tail.split('/')
        .filter(|s| !s.is_empty())
        .fold(&value, |acc, seg| &acc[seg])
        .clone()
}

fn bench_status_tail(c: &mut Criterion) {
    let metrics = Metrics::default();
    let status = Status::from(&metrics);
    let tail = "peers";

    c.bench_function("status_peers_direct", |b| {
        b.iter(|| direct(std::hint::black_box(&status), std::hint::black_box(tail)))
    });

    c.bench_function("status_peers_via_norito", |b| {
        b.iter(|| via_norito(std::hint::black_box(&status), std::hint::black_box(tail)))
    });
}

/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_status_tail(&mut c);
    c.final_summary();
}
