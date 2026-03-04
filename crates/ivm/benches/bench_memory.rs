//! Benchmarks for IVM memory commit performance and memory operations.
use criterion::Criterion;
use ivm::Memory;

fn dirty_memory() -> Memory {
    let mut mem = Memory::new(0);
    for i in 0..1024u64 {
        mem.store_u8(Memory::HEAP_START + i, (i & 0xff) as u8)
            .unwrap();
    }
    mem
}

fn dirty_memory_large() -> Memory {
    let mut mem = Memory::new(0);
    // Expand the heap to its maximum size so large writes do not violate
    // memory permissions.
    mem.grow_heap(Memory::HEAP_MAX_SIZE - Memory::HEAP_SIZE)
        .expect("failed to grow heap");
    for i in 0..Memory::HEAP_MAX_SIZE {
        mem.store_u8(Memory::HEAP_START + i, (i & 0xff) as u8)
            .unwrap();
    }
    mem
}

fn bench_memory_commit(c: &mut Criterion) {
    let base = dirty_memory();
    c.bench_function("memory_commit", |b| {
        b.iter(|| {
            let mut mem = base.clone();
            mem.commit();
            std::hint::black_box(())
        })
    });
}

fn bench_memory_commit_large(c: &mut Criterion) {
    let base = dirty_memory_large();
    c.bench_function("memory_commit_large", |b| {
        b.iter(|| {
            let mut mem = base.clone();
            mem.commit();
            std::hint::black_box(())
        })
    });
}

/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_memory_commit(&mut c);
    bench_memory_commit_large(&mut c);
    c.final_summary();
}
