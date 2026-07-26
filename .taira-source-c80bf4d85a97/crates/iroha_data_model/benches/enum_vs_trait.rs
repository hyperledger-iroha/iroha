//! Benchmarks enum dispatch vs. trait-object dispatch.
//!
//! Compares performance of a simple increment operation implemented via an
//! enum method and via a trait object call.
use criterion::Criterion;

/// Simple trait used in the benchmark.
trait Executor {
    fn execute(&self, input: u64) -> u64;
}

struct Increment;

impl Executor for Increment {
    fn execute(&self, input: u64) -> u64 {
        input + 1
    }
}

/// Enum-based implementation performing the same work as [`Executor`].
enum EnumExecutor {
    Increment,
}

impl EnumExecutor {
    fn execute(&self, input: u64) -> u64 {
        match self {
            Self::Increment => input + 1,
        }
    }
}

/// Benchmark the enum-based execution path.
fn bench_enum(c: &mut Criterion) {
    let exec = EnumExecutor::Increment;
    c.bench_function("enum", |b| b.iter(|| exec.execute(std::hint::black_box(1))));
}

/// Benchmark the trait-object execution path.
fn bench_trait_object(c: &mut Criterion) {
    let exec: Box<dyn Executor> = Box::new(Increment);
    c.bench_function("trait_object", |b| {
        b.iter(|| exec.execute(std::hint::black_box(1)))
    });
}

/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_enum(&mut c);
    bench_trait_object(&mut c);
    c.final_summary();
}
