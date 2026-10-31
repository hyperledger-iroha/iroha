//! Independent scalar-field parity, carry boundaries and public workload timing.

use super::*;
use rand::SeedableRng as _;
use rand_chacha::ChaCha12Rng;

fn scalar_mix(residues: &[F], alphas: &[E]) -> E {
    residues
        .iter()
        .zip(alphas)
        .fold(E::ZERO, |sum, (residue, alpha)| {
            sum.add(alpha.mul_base(*residue))
        })
}

fn streamed_mix(residues: &[F], alphas: &[E]) -> E {
    assert_eq!(residues.len(), alphas.len());
    let mut mix = MainResidueMixV1::new_v1();
    for (residue, alpha) in residues.iter().zip(alphas) {
        mix.push_v1(*residue, alpha);
    }
    mix.finish_v1()
}

fn fixture(length: usize, seed: u8) -> (Vec<F>, Vec<E>) {
    let mut rng = ChaCha12Rng::from_seed([seed; 32]);
    let mut field = || F::reduce(u128::from(rng.try_next_u64().unwrap()));
    let residues = (0..length).map(|_| field()).collect();
    let alphas = (0..length)
        .map(|_| E::from_coefficients(core::array::from_fn(|_| field())).unwrap())
        .collect();
    (residues, alphas)
}

#[test]
fn widened_composition_matches_scalar_reduction_at_carry_and_block_boundaries() {
    let p = crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
    assert_eq!(F::reduce(u128::MAX).add(F::ONE), F::ZERO.sub(F(1 << 32)));
    for length in [0, 1, 2, 3, 31, 1023, 1024, 1025, 2048, 2145, 4097] {
        for value in [F::ZERO, F::ONE, F(1 << 32), F(p - 1)] {
            let residues = vec![value; length];
            for alpha in [
                E::ZERO,
                E::from_coefficients([F(p - 1); 4]).unwrap(),
                E::from_coefficients([F::ZERO, F::ONE, F(1 << 32), F(p - 1)]).unwrap(),
            ] {
                let alphas = vec![alpha; length];
                assert_eq!(
                    streamed_mix(&residues, &alphas),
                    scalar_mix(&residues, &alphas)
                );
            }
        }
        for seed in [0, 17, 255] {
            let (residues, alphas) = fixture(length, seed);
            assert_eq!(
                streamed_mix(&residues, &alphas),
                scalar_mix(&residues, &alphas)
            );
        }
    }
}

#[test]
fn every_registration_mix_is_identical_across_worker_schedules() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let fixtures = layout
        .registered_segments
        .iter()
        .enumerate()
        .map(|(index, registration)| fixture(registration.segment.constraint_count, index as u8))
        .collect::<Vec<_>>();
    let expected = fixtures
        .iter()
        .map(|(r, a)| scalar_mix(r, a))
        .collect::<Vec<_>>();
    for workers in [1, 2, 8, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        let actual = pool.install(|| {
            fixtures
                .par_iter()
                .map(|(r, a)| streamed_mix(r, a))
                .collect::<Vec<_>>()
        });
        assert_eq!(actual, expected);
    }
}

#[test]
fn streamed_mixture_clears_products_carries_and_completed_blocks_on_every_exit() {
    use crate::privacy_engines::zk_x509::private_table::inspection;

    let value = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1 - 1);
    let alpha = E::from_coefficients([value; 4]).unwrap();
    for count in [2, TERMS_PER_BLOCK_V1, TERMS_PER_BLOCK_V1 + 1] {
        for outcome in 0..3 {
            let (result, erased) = inspection::observe_v1(|| {
                std::panic::catch_unwind(|| {
                    let mut mix = MainResidueMixV1::new_v1();
                    for _ in 0..count {
                        mix.push_v1(value, &alpha);
                    }
                    match outcome {
                        0 => assert_eq!(
                            mix.finish_v1(),
                            E::from_coefficients([F(count as u64); 4]).unwrap()
                        ),
                        1 => drop(mix),
                        _ => panic!("synthetic widened mixture unwind"),
                    }
                })
            });
            assert_eq!(result.is_err(), outcome == 2);
            // These maximal products populate both the four sums and the four
            // carry counters, including a complete block before a partial tail.
            assert!(
                erased
                    .iter()
                    .filter(|entry| entry.cells == 4 && entry.nonzero_before == 4)
                    .count()
                    >= 2
            );
            if outcome == 0 || count >= TERMS_PER_BLOCK_V1 {
                assert!(
                    erased
                        .iter()
                        .any(|entry| entry.cells == 1 && entry.nonzero_before == 1)
                );
            }
            assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
        }
    }
}

#[test]
#[ignore = "release-only isolated scalar versus widened composition-mix timings"]
fn composition_mix_cost_diagnostic() {
    assert!(!cfg!(debug_assertions), "use an optimized build");
    for width in [90, 593, 1024, 2145] {
        let (residues, alphas) = fixture(width, 73);
        let iterations = (1 << 24) / width;
        let mut timings = [std::time::Duration::ZERO; 2];
        for round in 0..2 {
            let mut checksums = [E::ZERO; 2];
            for mode in [round, 1 - round] {
                let start = std::time::Instant::now();
                for _ in 0..iterations {
                    let (r, a) = (
                        std::hint::black_box(residues.as_slice()),
                        std::hint::black_box(alphas.as_slice()),
                    );
                    let mixed = if mode == 0 {
                        scalar_mix(r, a)
                    } else {
                        streamed_mix(r, a)
                    };
                    checksums[mode] = checksums[mode].add(std::hint::black_box(mixed));
                }
                timings[mode] += start.elapsed();
            }
            assert_eq!(checksums[0], checksums[1]);
        }
        println!(
            "X509_COMPOSITION_MIX width={width} iterations={iterations} rounds=2 scalar_seconds={:.6} widened_seconds={:.6} speedup={:.3}",
            timings[0].as_secs_f64(),
            timings[1].as_secs_f64(),
            timings[0].as_secs_f64() / timings[1].as_secs_f64()
        );
    }
}
