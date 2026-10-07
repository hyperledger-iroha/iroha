//! Exact arithmetic, ownership and rejection tests for prepared coset transforms.

use super::*;
use crate::field::{Fp, Fq};
use ff::WithSmallOrderMulGroup;
use rand_chacha::ChaCha20Rng;
use rand_core_06::SeedableRng;

fn coefficients<F: PastaField>(n: usize, seed: u64) -> Vec<F> {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    (0..n).map(|_| F::random(&mut rng)).collect()
}

#[test]
fn prepared_cosets_reuse_storage_and_match_independent_outputs() {
    fn check<F: PastaField + WithSmallOrderMulGroup<3>>() {
        for workers in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            pool.install(|| {
                for k in [0, 1, 2, 3, 4, 12, 13] {
                    let domain = FftDomain::<F>::new(k).unwrap();
                    let mut powers = vec![F::ZERO; domain.n()];
                    let address = powers.as_ptr();
                    let capacity = powers.capacity();
                    for shift in [F::ONE, F::from(7), <F as WithSmallOrderMulGroup<3>>::ZETA] {
                        {
                            let plan = domain.coset_plan(&mut powers, shift).unwrap();
                            // Internal assertions in the FFT module: the plan borrows
                            // the caller's exact buffer; it does not own another Vec.
                            assert_eq!(plan.powers.as_ptr(), address);
                            let expected_powers: Vec<F> = (0..domain.n())
                                .scan(F::ONE, |power, _| {
                                    let value = *power;
                                    *power *= shift;
                                    Some(value)
                                })
                                .collect();
                            assert_eq!(plan.powers, expected_powers);
                            // Reuse the same plan across differing batch widths and seeds.
                            for (round, count) in [0u64, 1, 3, 17, 1].into_iter().enumerate() {
                                let seed_base = u64::try_from(round).unwrap() * 100;
                                let original: Vec<Vec<F>> = (0..count)
                                    .map(|index| coefficients(domain.n(), seed_base + index))
                                    .collect();
                                let mut batch = original.clone();
                                let pointers: Vec<_> = batch.iter().map(Vec::as_ptr).collect();
                                let mut columns: Vec<_> =
                                    batch.iter_mut().map(Vec::as_mut_slice).collect();
                                plan.fft_many(&mut columns).unwrap();
                                for (index, (actual, source)) in
                                    batch.iter_mut().zip(&original).enumerate()
                                {
                                    let mut expected = source.clone();
                                    domain.coset_fft(&mut expected, shift).unwrap();
                                    assert_eq!(
                                        *actual, expected,
                                        "workers={workers} k={k} count={count}"
                                    );
                                    if k <= 4 {
                                        assert_eq!(
                                            *actual,
                                            naive_coset_dft(source, domain.omega(), shift)
                                        );
                                    }
                                    assert_eq!(actual.as_ptr(), pointers[index]);
                                    domain.coset_ifft(actual, shift).unwrap();
                                    assert_eq!(actual, source);
                                }
                                assert_eq!(plan.powers, expected_powers);
                            }
                        }
                        assert_eq!(powers.as_ptr(), address);
                        assert_eq!(powers.capacity(), capacity);
                    }
                }
            });
        }
    }
    check::<Fp>();
    check::<Fq>();
}

#[test]
fn prepared_coset_construction_and_batches_validate_before_mutation() {
    fn check<F: PastaField>() {
        let domain = FftDomain::<F>::new(3).unwrap();
        for length in [0, 7, 9] {
            let mut powers = vec![F::from(19); length];
            let before = powers.clone();
            assert!(matches!(domain.coset_plan(&mut powers, F::ZERO),
                Err(FftError::WrongLength { expected: 8, actual }) if actual == length));
            assert_eq!(powers, before);
        }
        let mut powers = vec![F::from(19); domain.n()];
        let before = powers.clone();
        assert!(matches!(
            domain.coset_plan(&mut powers, F::ZERO),
            Err(FftError::ZeroShift)
        ));
        assert_eq!(powers, before);
        let plan = domain.coset_plan(&mut powers, F::from(7)).unwrap();
        for bad_index in 0..3 {
            let mut batch = vec![vec![F::ONE; 8]; 3];
            // A different domain's length must fail even for a valid final column.
            batch[bad_index].resize(16, F::ONE);
            let before = batch.clone();
            let mut columns: Vec<_> = batch.iter_mut().map(Vec::as_mut_slice).collect();
            assert_eq!(
                plan.fft_many(&mut columns),
                Err(FftError::WrongLength {
                    expected: 8,
                    actual: 16
                })
            );
            assert_eq!(batch, before);
        }
        assert_eq!(plan.fft_many(&mut []), Ok(()));
    }
    check::<Fp>();
    check::<Fq>();
}
