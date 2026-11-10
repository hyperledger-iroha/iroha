//! Bounded-prefix commitments retain the original full-width padding and blind.

use super::*;
use ff::Field;
use iroha_pasta::{Ep, Eq};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

fn parity<C: PastaCurve>() {
    let params = ParamsIpa::<C>::new(6).unwrap();
    let mut rng = ChaCha20Rng::from_seed([27; 32]);
    let blind = C::ScalarExt::random(&mut rng);
    for prefix in [0, 1, 8, 9, 57, 64] {
        let values = (0..64)
            .map(|i| {
                if i < prefix {
                    C::ScalarExt::from((i as u64 * 997) & 32767)
                } else {
                    C::ScalarExt::random(&mut rng)
                }
            })
            .collect::<Vec<_>>();
        if prefix < 64 {
            assert!(values[prefix].bit_length_vartime() > 15);
        }
        let reference = commit_lagrange(
            &params,
            &values,
            &blind,
            Secrecy::Secret,
            MemoryBudget::DEFAULT,
        )
        .unwrap();
        for tables in [
            CommitmentTables::none(),
            CommitmentTables::build(&params, MemoryBudget::DEFAULT),
        ] {
            for threads in [1, 4] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                let result = pool
                    .install(|| {
                        tables.commit_lagrange_bounded_prefix_cancellable(
                            &params,
                            &values,
                            (prefix, 15),
                            &blind,
                            MemoryBudget::DEFAULT,
                            None,
                        )
                    })
                    .unwrap();
                assert_eq!(result.to_affine(), reference.to_affine());
            }
        }
    }
    let empty = CommitmentTables::none()
        .commit_lagrange_bounded_prefix_cancellable(
            &params,
            &[],
            (0, 0),
            &blind,
            MemoryBudget::new(0),
            None,
        )
        .unwrap();
    assert_eq!(empty, params.w().to_curve() * blind);
}

#[test]
fn bounded_commitments_preserve_random_padding_and_blinds_on_both_curves() {
    parity::<Ep>();
    parity::<Eq>();
}

#[test]
fn invalid_prefix_is_rejected_before_tables_small_inputs_or_budget_fallback() {
    let params = ParamsIpa::<Ep>::new(4).unwrap();
    let blind = iroha_pasta::Fq::ONE;
    let values = [iroha_pasta::Fq::from(32768); 16];
    for tables in [
        CommitmentTables::none(),
        CommitmentTables::build(&params, MemoryBudget::DEFAULT),
    ] {
        for count in [1, 8, 16] {
            for budget in [MemoryBudget::new(0), MemoryBudget::DEFAULT] {
                assert_eq!(
                    tables.commit_lagrange_bounded_prefix_cancellable(
                        &params,
                        &values[..count],
                        (count, 15),
                        &blind,
                        budget,
                        None
                    ),
                    Err(MsmError::ScalarBound)
                );
            }
        }
        assert!(matches!(
            tables.commit_lagrange_bounded_prefix_cancellable(
                &params,
                &values,
                (17, 15),
                &blind,
                MemoryBudget::DEFAULT,
                None
            ),
            Err(MsmError::LengthMismatch(_))
        ));
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert_eq!(
            tables.commit_lagrange_bounded_prefix_cancellable(
                &params,
                &values,
                (0, 0),
                &blind,
                MemoryBudget::DEFAULT,
                Some(&cancellation)
            ),
            Err(MsmError::Cancelled)
        );
    }
    let too_long = [iroha_pasta::Fq::ZERO; 17];
    assert!(matches!(
        CommitmentTables::none().commit_lagrange_bounded_prefix_cancellable(
            &params,
            &too_long,
            (17, 0),
            &blind,
            MemoryBudget::DEFAULT,
            None
        ),
        Err(MsmError::LengthMismatch(_))
    ));
}

/// One batch of mixed bounded and unbounded columns equals committing each column
/// alone, with and without tables and on one or several workers.
fn batch_parity<C: PastaCurve>() {
    let params = ParamsIpa::<C>::new(6).unwrap();
    let mut rng = ChaCha20Rng::from_seed([31; 32]);
    for prefix in [0, 1, 9, 57, 64] {
        let narrow = |rng: &mut ChaCha20Rng, salt: u64| {
            (0..64)
                .map(|i| {
                    if i < prefix {
                        C::ScalarExt::from((i as u64 * 991 + salt) & 32767)
                    } else {
                        C::ScalarExt::random(&mut *rng)
                    }
                })
                .collect::<Vec<_>>()
        };
        let full = |rng: &mut ChaCha20Rng| {
            (0..64)
                .map(|_| C::ScalarExt::random(&mut *rng))
                .collect::<Vec<_>>()
        };
        let values = [
            narrow(&mut rng, 3),
            full(&mut rng),
            narrow(&mut rng, 5),
            narrow(&mut rng, 7),
            full(&mut rng),
        ];
        let bounds = [Some(15), None, Some(15), Some(16), None];
        let blinds: Vec<_> = (0..values.len())
            .map(|_| C::ScalarExt::random(&mut rng))
            .collect();
        let columns: Vec<(&[C::ScalarExt], Option<usize>)> = values
            .iter()
            .zip(bounds)
            .map(|(column, bound)| (&column[..], bound))
            .collect();
        let reference: Vec<_> = values
            .iter()
            .zip(&blinds)
            .map(|(column, blind)| {
                commit_lagrange(
                    &params,
                    column,
                    blind,
                    Secrecy::Secret,
                    MemoryBudget::DEFAULT,
                )
                .unwrap()
                .to_affine()
            })
            .collect();
        for tables in [
            CommitmentTables::none(),
            CommitmentTables::build(&params, MemoryBudget::DEFAULT),
        ] {
            for threads in [1, 4] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                let batch = pool
                    .install(|| {
                        tables.commit_lagrange_bounded_prefix_columns_cancellable(
                            &params,
                            &columns,
                            prefix,
                            &blinds,
                            MemoryBudget::DEFAULT,
                            None,
                        )
                    })
                    .unwrap();
                let batch: Vec<_> = batch.iter().map(|point| point.to_affine()).collect();
                assert_eq!(batch, reference, "prefix {prefix}, {threads} threads");
            }
        }
    }
    let empty = CommitmentTables::<C>::none()
        .commit_lagrange_bounded_prefix_columns_cancellable(
            &params,
            &[],
            0,
            &[],
            MemoryBudget::DEFAULT,
            None,
        )
        .unwrap();
    assert!(empty.is_empty());
}

#[test]
fn batched_bounded_prefix_columns_equal_single_column_commitments_on_both_curves() {
    batch_parity::<Ep>();
    batch_parity::<Eq>();
}

#[test]
fn batched_bounded_prefix_columns_reject_like_the_single_column_route() {
    let params = ParamsIpa::<Ep>::new(4).unwrap();
    let ok = [iroha_pasta::Fq::from(7); 16];
    let wide = [iroha_pasta::Fq::from(32768); 16];
    let blinds = [iroha_pasta::Fq::ONE; 2];
    for tables in [
        CommitmentTables::none(),
        CommitmentTables::build(&params, MemoryBudget::DEFAULT),
    ] {
        let batch = |columns: &[(&[iroha_pasta::Fq], Option<usize>)],
                     prefix: usize,
                     blinds: &[iroha_pasta::Fq],
                     cancellation: Option<&CancellationToken>| {
            tables.commit_lagrange_bounded_prefix_columns_cancellable(
                &params,
                columns,
                prefix,
                blinds,
                MemoryBudget::DEFAULT,
                cancellation,
            )
        };
        assert_eq!(
            batch(&[(&ok, Some(15)), (&wide, Some(15))], 16, &blinds, None),
            Err(MsmError::ScalarBound)
        );
        // An unbounded column takes any value.
        assert!(batch(&[(&ok, Some(15)), (&wide, None)], 16, &blinds, None).is_ok());
        assert!(matches!(
            batch(&[(&ok, Some(15)), (&wide, None)], 16, &blinds[..1], None),
            Err(MsmError::LengthMismatch(_))
        ));
        assert!(matches!(
            batch(&[(&ok, Some(15)), (&wide, None)], 17, &blinds, None),
            Err(MsmError::LengthMismatch(_))
        ));
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert_eq!(
            batch(
                &[(&ok, Some(15)), (&wide, None)],
                16,
                &blinds,
                Some(&cancellation)
            ),
            Err(MsmError::Cancelled)
        );
    }
    assert!(matches!(
        CommitmentTables::none().commit_lagrange_bounded_prefix_columns_cancellable(
            &params,
            &[(&ok[..], Some(15)), (&ok[..8], Some(15))],
            8,
            &blinds,
            MemoryBudget::DEFAULT,
            None
        ),
        Err(MsmError::LengthMismatch(_))
    ));
}
