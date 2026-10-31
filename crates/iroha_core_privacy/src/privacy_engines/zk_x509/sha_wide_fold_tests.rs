// Full-relation checks and costs for the widened native SHA base-error fold.

#[test]
fn widened_sha_relation_preserves_every_residue_segment_and_terminal() {
    use super::super::sha_word_stark::wide_fold::ShaWordFoldPowersV1;
    let word = word_challenges();
    let powers = ShaWordFoldPowersV1::new_v1(word).unwrap();
    let mut residues = Vec::with_capacity(ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1);
    let allocation = (residues.as_ptr(), residues.capacity());
    for seed in 0_usize..8 {
        let row = |offset: usize| ZkX509ShaBatchRowV1 {
            base: core::array::from_fn(|i| {
                F::reduce((seed + offset + i + 1) as u128 * (GOLDILOCKS_MODULUS_V1 - 1) as u128)
            }),
            aux: core::array::from_fn(|i| {
                F::reduce((seed + offset + i + 17) as u128 * (GOLDILOCKS_MODULUS_V1 - 3) as u128)
            }),
            fixed: core::array::from_fn(|i| {
                F::reduce((seed + offset + i + 31) as u128 * (GOLDILOCKS_MODULUS_V1 - 5) as u128)
            }),
        };
        let current = row(0);
        let next = row(47);
        for segment in 0..4 {
            for full in [false, true] {
                let terminals = neutral_ca_call_boundaries();
                let boundaries = full.then_some(&terminals);
                let expected = evaluate_sha_batch_relation_v1(
                    &current,
                    &next,
                    word,
                    challenges(),
                    &rfc_challenges(),
                    segment,
                    boundaries,
                )
                .unwrap();
                if full {
                    evaluate_sha_batch_relation_with_fold_v1(
                        &current,
                        &next,
                        word,
                        challenges(),
                        &rfc_challenges(),
                        segment,
                        boundaries,
                        &mut residues,
                        powers.row_v1(),
                    )
                    .unwrap();
                } else {
                    evaluate_sha_batch_local_with_powers_into_v1(
                        &current,
                        &next,
                        word,
                        challenges(),
                        &rfc_challenges(),
                        segment,
                        &mut residues,
                        &powers,
                    )
                    .unwrap();
                }
                assert_eq!(residues, expected);
                assert_eq!((residues.as_ptr(), residues.capacity()), allocation);
            }
        }
    }
}

#[test]
fn widened_sha_relation_preserves_original_refusals_before_destination_mutation() {
    use super::super::sha_word_stark::wide_fold::ShaWordFoldPowersV1;
    let powers = ShaWordFoldPowersV1::new_v1(word_challenges()).unwrap();
    for fault in 0..11 {
        let mut current = ZkX509ShaBatchRowV1 {
            base: [F::ZERO; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
            aux: [F::ZERO; ZK_X509_SHA_BATCH_AUX_WIDTH_V1],
            fixed: [F::ZERO; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
        };
        let mut next = current;
        let mut word = word_challenges();
        let mut call = challenges();
        let mut rfc = rfc_challenges();
        let mut segment = 0;
        let mut terminals = neutral_ca_call_boundaries();
        match fault {
            0 => current.base[0] = F(GOLDILOCKS_MODULUS_V1),
            1 => current.aux[0] = F(GOLDILOCKS_MODULUS_V1),
            2 => current.fixed[0] = F(GOLDILOCKS_MODULUS_V1),
            3 => next.base[0] = F(GOLDILOCKS_MODULUS_V1),
            4 => next.aux[0] = F(GOLDILOCKS_MODULUS_V1),
            5 => next.fixed[0] = F(GOLDILOCKS_MODULUS_V1),
            6 => {
                call.lanes[0].terms[0] = F::ZERO;
                segment = 255;
            }
            7 => rfc.tuple[0][0] = F::ZERO,
            8 => word.base_folding[0] = F::ZERO,
            9 => terminals[0].call = 255,
            _ => segment = 255,
        }
        let expected = evaluate_sha_batch_relation_v1(
            &current,
            &next,
            word,
            call,
            &rfc,
            segment,
            Some(&terminals),
        )
        .unwrap_err();
        let mut residues = vec![F(93); ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1];
        let before = residues.clone();
        let actual = evaluate_sha_batch_relation_with_fold_v1(
            &current,
            &next,
            word,
            call,
            &rfc,
            segment,
            Some(&terminals),
            &mut residues,
            powers.row_v1(),
        )
        .unwrap_err();
        assert_eq!(actual, expected);
        assert_eq!(residues, before);
        if fault != 9 {
            let expected_local =
                evaluate_sha_batch_relation_v1(&current, &next, word, call, &rfc, segment, None)
                    .unwrap_err();
            let actual_local = evaluate_sha_batch_local_with_powers_into_v1(
                &current,
                &next,
                word,
                call,
                &rfc,
                segment,
                &mut residues,
                &powers,
            )
            .unwrap_err();
            assert_eq!(actual_local, expected_local);
            assert_eq!(residues, before);
        }
    }
    let mut different = word_challenges();
    different.base_folding[0] = F(997);
    let different = ShaWordFoldPowersV1::new_v1(different).unwrap();
    let row = ZkX509ShaBatchRowV1 {
        base: [F::ZERO; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
        aux: [F::ZERO; ZK_X509_SHA_BATCH_AUX_WIDTH_V1],
        fixed: [F::ZERO; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
    };
    let mut residues = vec![F(19); ZK_X509_SHA_BATCH_LOCAL_CONSTRAINT_COUNT_V1];
    let before = residues.clone();
    assert!(
        evaluate_sha_batch_local_with_powers_into_v1(
            &row,
            &row,
            word_challenges(),
            challenges(),
            &rfc_challenges(),
            0,
            &mut residues,
            &different,
        )
        .is_err()
    );
    assert_eq!(residues, before);
}

#[test]
#[ignore = "isolated full SHA relation Horner-versus-widened fold A/B"]
fn widened_sha_relation_cost_preserves_all_constraints_and_outputs() {
    use super::super::sha_word_stark::wide_fold::{ShaWordFoldPowersV1, ShaWordWideFoldV1};
    use rayon::prelude::*;
    use std::{hint::black_box, time::Instant};

    assert!(!cfg!(debug_assertions), "use an optimized build");
    let word = word_challenges();
    let setup = Instant::now();
    let powers = ShaWordFoldPowersV1::new_v1(word).unwrap();
    let setup_seconds = setup.elapsed().as_secs_f64();
    let current = ZkX509ShaBatchRowV1 {
        base: core::array::from_fn(|i| F(i as u64 + 1)),
        aux: core::array::from_fn(|i| F(i as u64 + 17)),
        fixed: core::array::from_fn(|i| F(i as u64 + 31)),
    };
    let next = current;
    let expected = evaluate_zk_x509_sha_batch_local_residues_over_field_v1(
        &current,
        &next,
        word,
        challenges(),
        &rfc_challenges(),
        0,
    )
    .unwrap();
    const ROWS: usize = 1 << 15;
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.broadcast(|_| ());
        let mut buffers = PrivateTableV1::new(
            (0..workers)
                .map(|_| vec![F::ZERO; ZK_X509_SHA_BATCH_LOCAL_CONSTRAINT_COUNT_V1])
                .collect(),
            zeroize_field_rows_v1::<Vec<F>>,
        );
        let allocations = buffers
            .iter()
            .map(|buffer| (buffer.as_ptr(), buffer.capacity()))
            .collect::<Vec<_>>();
        for round in 0..4 {
            for wide in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let started = Instant::now();
                pool.install(|| {
                    buffers
                        .par_iter_mut()
                        .enumerate()
                        .for_each(|(worker, residues)| {
                            for _ in (worker..ROWS).step_by(workers) {
                                if wide {
                                    evaluate_sha_batch_relation_with_fold_v1(
                                        black_box(&current),
                                        black_box(&next),
                                        word,
                                        challenges(),
                                        &rfc_challenges(),
                                        0,
                                        None,
                                        residues,
                                        powers.row_v1(),
                                    )
                                    .unwrap();
                                } else {
                                    evaluate_zk_x509_sha_batch_local_residues_into_v1(
                                        black_box(&current),
                                        black_box(&next),
                                        word,
                                        challenges(),
                                        &rfc_challenges(),
                                        0,
                                        residues,
                                    )
                                    .unwrap();
                                }
                                black_box(&*residues);
                            }
                        });
                });
                let seconds = started.elapsed().as_secs_f64();
                for (buffer, allocation) in buffers.iter().zip(&allocations) {
                    assert_eq!(buffer, &expected);
                    assert_eq!((buffer.as_ptr(), buffer.capacity()), *allocation);
                }
                eprintln!(
                    "X509 SHA full relation fold A/B: workers={workers} round={round} widened={wide} rows={ROWS} seconds={seconds:.9} public_powers_bytes={} row_scratch_bytes={} setup_seconds={setup_seconds:.9} exact_residues={} original_base_errors=441 parity=all_fields_exact; full original validation and constraints included, setup excluded from repeated row timing",
                    ShaWordFoldPowersV1::HEAP_PAYLOAD_BYTES_V1,
                    core::mem::size_of::<ShaWordWideFoldV1<'_>>(),
                    expected.len(),
                );
            }
        }
    }
}
