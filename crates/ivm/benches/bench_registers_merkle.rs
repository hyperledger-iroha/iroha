use std::time::Instant;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ivm::Registers;

const REGISTER_COUNT: usize = 256;
const WRITABLE_REGISTER_COUNT: usize = REGISTER_COUNT - 1;

#[inline]
fn bench_register_index(step: u32) -> usize {
    (((step as usize) - 1) % WRITABLE_REGISTER_COUNT) + 1
}

fn bench_registers_merkle(c: &mut Criterion) {
    let mut group = c.benchmark_group("registers_merkle");

    // Banner indicating mode
    if cfg!(feature = "merkle_incremental") {
        println!("Registers Merkle mode: incremental (per-update O(log n))");
    } else {
        println!("Registers Merkle mode: full rebuild (lazy on read)");
    }

    let sizes = [10u32, 100, 1_000, 10_000];
    for &n in &sizes {
        group.bench_function(BenchmarkId::new("set_many_then_root", n), |b| {
            b.iter(|| {
                let mut regs = Registers::new();
                for i in 1..=n {
                    let idx = bench_register_index(i);
                    regs.set(idx, i as u64);
                }
                std::hint::black_box(regs.merkle_root());
            });
        });

        group.bench_function(BenchmarkId::new("set_and_root_each", n), |b| {
            b.iter(|| {
                let mut regs = Registers::new();
                for i in 1..=n {
                    let idx = bench_register_index(i);
                    regs.set(idx, i as u64);
                    std::hint::black_box(regs.merkle_root());
                }
            });
        });

        group.bench_function(BenchmarkId::new("set_many_then_path", n), |b| {
            b.iter(|| {
                let mut regs = Registers::new();
                for i in 1..=n {
                    let idx = bench_register_index(i);
                    regs.set(idx, i as u64);
                }
                let idx = bench_register_index(n);
                std::hint::black_box(regs.merkle_path(idx));
            });
        });

        group.bench_function(BenchmarkId::new("set_and_path_each", n), |b| {
            b.iter(|| {
                let mut regs = Registers::new();
                for i in 1..=n {
                    let idx = bench_register_index(i);
                    regs.set(idx, i as u64);
                    std::hint::black_box(regs.merkle_path(idx));
                }
            });
        });
    }

    group.finish();

    // Lightweight one-shot timing to print ratio table (not part of Criterion stats)
    println!("\nRegisters Merkle lazy-rebuild ratios (one-shot timing)");
    println!(
        "size, root_ratio(set_and_each / set_many_then), path_ratio(set_and_each / set_many_then)"
    );
    for &n in &sizes {
        // Root: many-then-root
        let mut regs = Registers::new();
        let t0 = Instant::now();
        for i in 1..=n {
            let idx = bench_register_index(i);
            regs.set(idx, i as u64);
        }
        std::hint::black_box(regs.merkle_root());
        let many_then_root_ms = t0.elapsed().as_secs_f64() * 1000.0;

        // Root: set-and-root-each
        let mut regs = Registers::new();
        let t1 = Instant::now();
        for i in 1..=n {
            let idx = bench_register_index(i);
            regs.set(idx, i as u64);
            std::hint::black_box(regs.merkle_root());
        }
        let and_root_each_ms = t1.elapsed().as_secs_f64() * 1000.0;

        // Path: many-then-path
        let mut regs = Registers::new();
        let t2 = Instant::now();
        for i in 1..=n {
            let idx = bench_register_index(i);
            regs.set(idx, i as u64);
        }
        let idx = bench_register_index(n);
        std::hint::black_box(regs.merkle_path(idx));
        let many_then_path_ms = t2.elapsed().as_secs_f64() * 1000.0;

        // Path: set-and-path-each
        let mut regs = Registers::new();
        let t3 = Instant::now();
        for i in 1..=n {
            let idx = bench_register_index(i);
            regs.set(idx, i as u64);
            std::hint::black_box(regs.merkle_path(idx));
        }
        let and_path_each_ms = t3.elapsed().as_secs_f64() * 1000.0;

        let root_ratio = and_root_each_ms / many_then_root_ms.max(1e-9);
        let path_ratio = and_path_each_ms / many_then_path_ms.max(1e-9);
        println!("{n}, {root_ratio:.2}x, {path_ratio:.2}x");
    }
}

criterion_group!(benches, bench_registers_merkle);
criterion_main!(benches);
