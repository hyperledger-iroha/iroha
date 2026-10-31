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
                    mix_residues_v1(&residues, &alphas),
                    scalar_mix(&residues, &alphas)
                );
            }
        }
        for seed in [0, 17, 255] {
            let (residues, alphas) = fixture(length, seed);
            assert_eq!(
                mix_residues_v1(&residues, &alphas),
                scalar_mix(&residues, &alphas)
            );
        }
    }
}

#[test]
fn indexed_widened_mix_preserves_order_carries_and_empty_input() {
    let p = crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
    for length in [0, 1, 2, 7, 8, 9, 1023, 1024, 1025, 2048, 4097] {
        let residues = (0..length)
            .map(|index| F(p - 1 - (index % 31) as u64))
            .collect::<Vec<_>>();
        let alphas = (0..length)
            .map(|index| {
                E::from_coefficients(core::array::from_fn(|lane| {
                    F(p - 1 - ((index + lane) % 17) as u64)
                }))
                .unwrap()
            })
            .collect::<Vec<_>>();
        let mut visited = 0;
        let actual = mix_indexed_residues_v1(length, |index| {
            assert_eq!(index, visited);
            visited += 1;
            (residues[length - 1 - index], alphas[length - 1 - index])
        });
        assert_eq!(visited, length);
        assert_eq!(actual, scalar_mix(&residues, &alphas));
        assert_eq!(actual, mix_residues_v1(&residues, &alphas));
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
                .map(|(r, a)| mix_residues_v1(r, a))
                .collect::<Vec<_>>()
        });
        assert_eq!(actual, expected);
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
                        mix_residues_v1(r, a)
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
