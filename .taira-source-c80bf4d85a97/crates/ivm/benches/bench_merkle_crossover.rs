//! Merkle leaf-scaling benchmark to observe CPU → GPU crossover.
//! Measures CPU-only vs auto-accelerated root computation across sizes.
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use ivm::{ByteMerkleTree, MerkleTree};

fn make_data(leaves: usize, chunk: usize) -> Vec<u8> {
    // Deterministic non-zero pattern to avoid zero-fast-paths
    let mut v = vec![0u8; leaves * chunk];
    for (i, byte) in v.iter_mut().enumerate() {
        let x = i as u32;
        *byte = (x.rotate_left(7) ^ 0x5Au32) as u8;
    }
    v
}

fn bench_merkle_crossover(c: &mut Criterion) {
    let chunk = 32usize;
    let mut group = c.benchmark_group("merkle_crossover");
    // Scale leaves from 1K up to 128K (32 KiB → 4 MiB payload)
    for leaves in [1024usize, 2048, 4096, 8192, 16384, 32768, 65536, 131072] {
        let bytes = leaves * chunk;
        group.throughput(Throughput::Bytes(bytes as u64));
        let id_cpu = BenchmarkId::new("cpu", leaves);
        let id_auto = BenchmarkId::new("auto", leaves);

        group.bench_function(id_cpu, |b| {
            b.iter_batched(
                || make_data(leaves, chunk),
                |data| {
                    // CPU path via canonical Merkle builder
                    let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(
                        std::hint::black_box(&data),
                        chunk,
                    )
                    .expect("valid chunk");
                    std::hint::black_box(tree.root());
                },
                BatchSize::LargeInput,
            )
        });

        group.bench_function(id_auto, |b| {
            b.iter_batched(
                || make_data(leaves, chunk),
                |data| {
                    let _root =
                        ByteMerkleTree::root_from_bytes_accel(std::hint::black_box(&data), chunk);
                    std::hint::black_box(_root);
                },
                BatchSize::LargeInput,
            )
        });
    }
    group.finish();
}

criterion_group!(benches, bench_merkle_crossover);
criterion_main!(benches);
