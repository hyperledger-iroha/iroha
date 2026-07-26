//! Benchmarks for a simple XYK DEX simulation.
use criterion::Criterion;

#[derive(Default)]
struct Token {
    supply: f64,
}

struct Dex {
    reserve_a: f64,
    reserve_b: f64,
}

impl Dex {
    fn new() -> Self {
        Self {
            reserve_a: 0.0,
            reserve_b: 0.0,
        }
    }

    fn add_liquidity(
        &mut self,
        token_a: &mut Token,
        token_b: &mut Token,
        amount_a: f64,
        amount_b: f64,
    ) {
        assert!(token_a.supply >= amount_a);
        assert!(token_b.supply >= amount_b);
        token_a.supply -= amount_a;
        token_b.supply -= amount_b;
        self.reserve_a += amount_a;
        self.reserve_b += amount_b;
    }

    fn swap_a_for_b(&mut self, amount_in: f64) -> f64 {
        let k = self.reserve_a * self.reserve_b;
        self.reserve_a += amount_in;
        let new_reserve_b = k / self.reserve_a;
        let amount_out = self.reserve_b - new_reserve_b;
        self.reserve_b = new_reserve_b;
        amount_out
    }
}

fn bench_xyk(c: &mut Criterion) {
    c.bench_function("xyk_1m_swaps", |b| {
        b.iter(|| {
            // create tokens
            let mut token_a = Token {
                supply: std::hint::black_box(2_000_000.0),
            };
            let mut token_b = Token {
                supply: std::hint::black_box(2_000_000.0),
            };
            // create dex and deposit liquidity
            let mut dex = Dex::new();
            dex.add_liquidity(
                &mut token_a,
                &mut token_b,
                std::hint::black_box(1_000_000.0),
                std::hint::black_box(1_000_000.0),
            );
            // perform 1 million swaps of 1 token each
            for _ in 0..1_000_000 {
                let _ = dex.swap_a_for_b(std::hint::black_box(1.0));
            }
            std::hint::black_box(dex);
        })
    });
}

/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_xyk(&mut c);
    c.final_summary();
}
