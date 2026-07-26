//! Benchmarks for byte Merkle tree root computation (commit paths).
use criterion::Criterion;
use ivm::ByteMerkleTree;

const CHUNK: usize = 32;

fn sequential_root(data: &[u8]) -> [u8; 32] {
    ByteMerkleTree::from_bytes(data, CHUNK).root()
}

fn parallel_root(data: &[u8]) -> [u8; 32] {
    ByteMerkleTree::from_bytes_parallel(data, CHUNK).root()
}

fn bench_commit(c: &mut Criterion) {
    let data = vec![0u8; 1 << 20]; // 1 MiB
    c.bench_function("commit_sequential", |b| b.iter(|| sequential_root(&data)));
    c.bench_function("commit_parallel", |b| b.iter(|| parallel_root(&data)));
}

/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_commit(&mut c);
    c.final_summary();
}
