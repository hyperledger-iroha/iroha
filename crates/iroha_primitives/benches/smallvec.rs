//! Criterion benchmark for `SmallVec` Norito (de)serialization.
use criterion::Criterion;
use iroha_primitives::small::{SmallVec, smallvec};
use norito::codec::{Decode, Encode};

/// Benchmark decoding a pre-encoded small vector using the Norito codec.
fn deserialize_smallvec(c: &mut Criterion) {
    let vec: SmallVec<[u32; 8]> = SmallVec(smallvec![1u32; 32]);
    let bytes = vec.encode();
    c.bench_function("smallvec_deserialize", |b| {
        b.iter(|| {
            let mut data = &bytes[..];
            let decoded = SmallVec::<[u32; 8]>::decode(&mut data).expect("decode");
            std::hint::black_box(decoded);
        })
    });
}

/// Criterion entry point for the smallvec benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    deserialize_smallvec(&mut c);
    c.final_summary();
}
