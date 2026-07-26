//! Planner micro-benchmarks for the FASTPQ production pipeline.
//!
//! These benchmarks exercise the canonical planner across CPU and GPU
//! execution paths so operators can capture comparative performance data
//! on reference hardware.

use std::{convert::TryFrom, hint::black_box, sync::OnceLock};

use criterion::{BatchSize, BenchmarkId, Criterion};
use fastpq_isi::{CANONICAL_PARAMETER_SETS, StarkParameterSet};
use fastpq_prover::{ExecutionMode, Planner};

const GOLDILOCKS_MODULUS: u64 = 0xffff_ffff_0000_0001;
const COLUMN_COUNT: usize = 16;

fn trace_logs(max_log: u32) -> Vec<u32> {
    let start = max_log.saturating_sub(1);
    (start..=max_log).collect()
}

fn generate_columns(len: usize, column_count: usize, seed: u64) -> Vec<Vec<u64>> {
    let mut columns = Vec::with_capacity(column_count);
    for column in 0..column_count {
        let mut data = Vec::with_capacity(len);
        let column_seed = seed.wrapping_add((column as u64).wrapping_mul(0x9e37_79b9));
        let rotate = u32::try_from((column & 31) + 1).unwrap_or(1);
        for idx in 0..len {
            let raw = column_seed
                ^ ((idx as u64).rotate_left(rotate))
                ^ ((idx as u64).wrapping_mul(0x2545_f491_4f6c_dd1d));
            data.push(raw % GOLDILOCKS_MODULUS);
        }
        columns.push(data);
    }
    columns
}

fn resolved_gpu_mode() -> ExecutionMode {
    static MODE: OnceLock<ExecutionMode> = OnceLock::new();
    *MODE.get_or_init(|| ExecutionMode::Auto.resolve())
}

fn bench_fft_for_params(c: &mut Criterion, params: StarkParameterSet) {
    let planner = Planner::new(&params);
    let group_name = format!("planner_fft::{}", params.name);
    let mut group = c.benchmark_group(&group_name);

    for trace_log in trace_logs(params.trace_log_size) {
        let len = 1usize << trace_log;
        let id = format!("trace=2^{trace_log};cols={COLUMN_COUNT}");

        group.bench_with_input(BenchmarkId::new("cpu", &id), &len, |b, _| {
            b.iter_batched(
                || generate_columns(len, COLUMN_COUNT, 0x51a2_d3f4),
                |mut columns| {
                    planner.fft_columns(&mut columns);
                    black_box(columns);
                },
                BatchSize::LargeInput,
            );
        });

        let gpu_label = match resolved_gpu_mode() {
            ExecutionMode::Gpu => "gpu",
            _ => "gpu_fallback",
        };
        group.bench_with_input(BenchmarkId::new(gpu_label, &id), &len, |b, _| {
            b.iter_batched(
                || generate_columns(len, COLUMN_COUNT, 0x51a2_d3f4),
                |mut columns| {
                    planner.fft_gpu(&mut columns);
                    black_box(columns);
                },
                BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

fn bench_ifft_for_params(c: &mut Criterion, params: StarkParameterSet) {
    let planner = Planner::new(&params);
    let group_name = format!("planner_ifft::{}", params.name);
    let mut group = c.benchmark_group(&group_name);

    for trace_log in trace_logs(params.trace_log_size) {
        let len = 1usize << trace_log;
        let id = format!("trace=2^{trace_log};cols={COLUMN_COUNT}");

        group.bench_with_input(BenchmarkId::new("cpu", &id), &len, |b, _| {
            b.iter_batched(
                || {
                    let mut freq = generate_columns(len, COLUMN_COUNT, 0x33c0_1bef);
                    planner.fft_columns(&mut freq);
                    freq
                },
                |mut freq| {
                    planner.ifft_columns(&mut freq);
                    black_box(freq);
                },
                BatchSize::LargeInput,
            );
        });

        let gpu_label = match resolved_gpu_mode() {
            ExecutionMode::Gpu => "gpu",
            _ => "gpu_fallback",
        };
        group.bench_with_input(BenchmarkId::new(gpu_label, &id), &len, |b, _| {
            b.iter_batched(
                || {
                    let mut freq = generate_columns(len, COLUMN_COUNT, 0x33c0_1bef);
                    planner.fft_columns(&mut freq);
                    freq
                },
                |mut freq| {
                    planner.ifft_gpu(&mut freq);
                    black_box(freq);
                },
                BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

fn bench_lde_for_params(c: &mut Criterion, params: StarkParameterSet) {
    let planner = Planner::new(&params);
    let group_name = format!("planner_lde::{}", params.name);
    let mut group = c.benchmark_group(&group_name);
    let blowup_log = planner.blowup_log();

    for trace_log in trace_logs(params.trace_log_size) {
        if trace_log + blowup_log > params.lde_log_size {
            continue;
        }
        let len = 1usize << trace_log;
        let eval_log = trace_log + blowup_log;
        let id = format!("trace=2^{trace_log}->lde=2^{eval_log};cols={COLUMN_COUNT}");

        group.bench_with_input(BenchmarkId::new("cpu", &id), &len, |b, _| {
            b.iter_batched(
                || generate_columns(len, COLUMN_COUNT, 0x6b72_940c),
                |columns| {
                    let outputs = planner.lde_columns(&columns);
                    black_box(outputs);
                },
                BatchSize::LargeInput,
            );
        });

        let gpu_label = match resolved_gpu_mode() {
            ExecutionMode::Gpu => "gpu",
            _ => "gpu_fallback",
        };
        group.bench_with_input(BenchmarkId::new(gpu_label, &id), &len, |b, _| {
            b.iter_batched(
                || generate_columns(len, COLUMN_COUNT, 0x6b72_940c),
                |columns| {
                    let outputs = planner.lde_gpu(&columns);
                    black_box(outputs);
                },
                BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

fn register_planner_benchmarks(c: &mut Criterion) {
    for params in CANONICAL_PARAMETER_SETS {
        bench_fft_for_params(c, params);
        bench_ifft_for_params(c, params);
        bench_lde_for_params(c, params);
    }
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    register_planner_benchmarks(&mut criterion);
    criterion.final_summary();
}
