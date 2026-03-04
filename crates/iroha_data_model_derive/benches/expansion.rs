//! Benchmarks for expansion and processing in `iroha_data_model_derive`.
//!
//! These benchmarks measure the macro expansion performance of the derive
//! helpers used by the data model. They are intended to catch regressions in
//! compile-time behavior rather than runtime performance.

use criterion::Criterion;
use manyhow::Emitter;
use syn::{Item, ItemMod, parse_quote};

#[path = "../src/model.rs"]
mod model;

/// Benchmark the cost of expanding `impl_model` on a small test module.
fn bench_impl_model(c: &mut Criterion) {
    let module: ItemMod = parse_quote! {
        mod model {
            pub struct A {
                pub value: u32,
            }
            struct B;
        }
    };

    c.bench_function("impl_model_expansion", |b| {
        b.iter(|| {
            let mut emitter = Emitter::new();
            model::impl_model(&mut emitter, &module);
        })
    });
}

/// Benchmark the cost of processing a single `Item` during macro expansion.
fn bench_process_item(c: &mut Criterion) {
    let item: Item = parse_quote! {
        struct A {
            value: u32,
        }
    };

    c.bench_function("process_item_expansion", |b| {
        b.iter(|| {
            model::process_item(item.clone());
        })
    });
}

/// Entrypoint for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_impl_model(&mut c);
    bench_process_item(&mut c);
    c.final_summary();
}
