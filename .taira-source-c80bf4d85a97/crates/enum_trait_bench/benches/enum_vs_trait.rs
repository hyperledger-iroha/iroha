//! Criterion benchmarks comparing enum vs. trait-object dispatch.
use criterion::Criterion;

trait Executor {
    fn execute(&self, input: u64) -> u64;
}

struct Increment;

impl Executor for Increment {
    fn execute(&self, input: u64) -> u64 {
        input + 1
    }
}

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

fn bench_enum(c: &mut Criterion) {
    let exec = EnumExecutor::Increment;
    c.bench_function("enum", |b| b.iter(|| exec.execute(std::hint::black_box(1))));
}

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
