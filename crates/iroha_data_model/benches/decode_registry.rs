//! Benchmarks for decoding instructions via different registries.
//!
//! This Criterion benchmark compares decoding performance between a simple
//! vector-backed registry and the production `InstructionRegistry`.
use criterion::Criterion;
use iroha_data_model::{
    Level,
    isi::{Instruction, InstructionBox, InstructionConstructor, InstructionRegistry, Log},
    prelude::Decode,
};
use norito::core;

/// Minimal vector-backed registry used for benchmarking decode performance.
struct VecRegistry {
    entries: Vec<(&'static str, InstructionConstructor)>,
}

impl VecRegistry {
    /// Create an empty vector-backed registry.
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a single instruction type in the registry.
    fn register<T>(mut self) -> Self
    where
        T: Instruction + Decode + 'static,
    {
        fn ctor<T>(_flags: u8, input: &[u8]) -> Result<InstructionBox, norito::Error>
        where
            T: Instruction + Decode + 'static,
        {
            let instruction = T::decode(&mut &*input)?;
            Ok(Box::new(instruction).into_instruction_box())
        }

        let name = std::any::type_name::<T>();
        self.entries.push((name, ctor::<T>));
        self
    }

    /// Decode an instruction by its type name using the stored constructor.
    fn decode(&self, name: &str, bytes: &[u8]) -> Option<Result<InstructionBox, norito::Error>> {
        self.entries
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, ctor)| ctor(0, bytes))
    }
}

/// Benchmark the decode path for a vector registry vs. `InstructionRegistry`.
fn bench_decode(c: &mut Criterion) {
    let name = std::any::type_name::<Log>();
    let instruction = Log::new(Level::INFO, "bench".into());
    let framed_bytes =
        norito::to_bytes(&instruction).expect("serialize instruction with Norito header");
    let payload = core::from_bytes_view(&framed_bytes)
        .expect("validate framed bytes")
        .as_bytes()
        .to_vec();

    let vec_registry = VecRegistry::new().register::<Log>();
    let hash_registry = InstructionRegistry::new().register::<Log>();

    c.bench_function("decode_vec", |b| {
        b.iter(|| {
            vec_registry.decode(name, &payload).unwrap().unwrap();
        })
    });

    c.bench_function("decode_hashmap", |b| {
        b.iter(|| {
            hash_registry.decode(name, &framed_bytes).unwrap().unwrap();
        })
    });
}

/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_decode(&mut c);
    c.final_summary();
}
