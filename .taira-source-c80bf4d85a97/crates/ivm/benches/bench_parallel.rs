//! Benchmarks for IVM parallel scheduler and execution paths.
use criterion::Criterion;
use ivm::{
    IVM, ProgramMetadata, encoding,
    parallel::{Block, Scheduler, State, StateAccessSet, Transaction, TxResult},
};

fn build_block(n: usize) -> Block {
    let mut txs = Vec::new();
    for i in 0..n {
        let mut access = StateAccessSet::new();
        access.write_keys.insert(format!("k{i}"));
        txs.push(Transaction {
            code: vec![],
            gas_limit: 0,
            access,
        });
    }
    Block { transactions: txs }
}

fn bench_scheduler(c: &mut Criterion) {
    let cores = num_cpus::get_physical();
    let scheduler = Scheduler::new(cores);
    let block = build_block(100);
    c.bench_function("schedule_100", |b| {
        b.iter(|| {
            scheduler.schedule_block(block.clone(), |_| TxResult {
                success: true,
                gas_used: 1,
            });
        });
    });
    let big_block = build_block(1000);
    c.bench_function("schedule_1000", |b| {
        b.iter(|| {
            scheduler.schedule_block(big_block.clone(), |_| TxResult {
                success: true,
                gas_used: 1,
            });
        });
    });
}

fn bench_ivm_execute(c: &mut Criterion) {
    let cores = num_cpus::get_physical();
    let mut ivm = IVM::new_with_options(Some(cores), State::new(), u64::MAX);
    let block = build_block(100);
    c.bench_function("ivm_execute_block_100", |b| {
        b.iter(|| {
            ivm.execute_block(block.clone());
        });
    });
    let big_block = build_block(1000);
    c.bench_function("ivm_execute_block_1000", |b| {
        b.iter(|| {
            ivm.execute_block(big_block.clone());
        });
    });
}

fn sample_program() -> Vec<u8> {
    let mut prog = ProgramMetadata::default().encode();
    prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    prog
}

fn bench_ivm_multi_transaction_block(c: &mut Criterion) {
    let cores = num_cpus::get_physical();
    let mut ivm = IVM::new_with_options(Some(cores), State::new(), u64::MAX);
    let program = sample_program();
    let mut txs = Vec::new();
    for i in 0..256usize {
        let mut access = StateAccessSet::new();
        access.write_keys.insert(format!("bench_key_{i}"));
        txs.push(Transaction {
            code: program.clone(),
            gas_limit: 10,
            access,
        });
    }
    let block = Block { transactions: txs };
    c.bench_function("ivm_execute_block_multi_tx_256", |b| {
        b.iter(|| {
            ivm.execute_block(block.clone());
        });
    });
}

fn bench_scheduler_threads(c: &mut Criterion) {
    let block = build_block(1000);
    let max = num_cpus::get_physical();
    let thread_counts = [1usize, 2, 4, max];
    let mut group = c.benchmark_group("scheduler_threads");
    for &t in &thread_counts {
        let scheduler = Scheduler::new(t);
        group.bench_function(format!("threads_{t}"), |b| {
            b.iter(|| {
                scheduler.schedule_block(block.clone(), |_| TxResult {
                    success: true,
                    gas_used: 1,
                });
            });
        });
    }
    group.finish();
}

fn bench_scheduler_dynamic(c: &mut Criterion) {
    let block = build_block(1000);
    let max = num_cpus::get_physical();
    let scheduler = Scheduler::new_dynamic(1, max);
    c.bench_function("scheduler_dynamic", |b| {
        b.iter(|| {
            scheduler.schedule_block(block.clone(), |_| TxResult {
                success: true,
                gas_used: 1,
            });
        });
    });
}

/// Entry point for the benchmark binary.
fn main() {
    // Silence ASCII banner and feature selection in benches.
    ivm::set_banner_enabled(false);
    let mut c = Criterion::default().configure_from_args();
    bench_scheduler(&mut c);
    bench_ivm_execute(&mut c);
    bench_ivm_multi_transaction_block(&mut c);
    bench_scheduler_threads(&mut c);
    bench_scheduler_dynamic(&mut c);
    c.final_summary();
}
