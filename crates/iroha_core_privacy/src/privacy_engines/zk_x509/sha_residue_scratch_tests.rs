// Reused SHA residue storage preserves complete local/full relations and refusals.

#[test]
fn reusable_sha_residues_preserve_all_segments_fields_terminals_and_capacities() {
    fn compare<A: PolynomialAirFieldV1 + core::fmt::Debug + PartialEq>(embed: impl Fn(usize) -> A) {
        let mut reused = Vec::with_capacity(ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1);
        let allocation = (reused.as_ptr(), reused.capacity());
        for seed in 0_usize..8 {
            let row = |offset: usize| ZkX509ShaBatchRowV1 {
                base: core::array::from_fn(|i| embed(seed + offset + i)),
                aux: core::array::from_fn(|i| embed(seed + offset + 17 + i)),
                fixed: core::array::from_fn(|i| embed(seed + offset + 31 + i)),
            };
            let current = row(0);
            let next = row(47);
            for segment in 0..4 {
                for full in [true, false] {
                    let terminals = neutral_ca_call_boundaries();
                    let boundaries = full.then_some(&terminals);
                    let expected = evaluate_sha_batch_relation_v1(
                        &current,
                        &next,
                        word_challenges(),
                        challenges(),
                        &rfc_challenges(),
                        segment,
                        boundaries,
                    )
                    .unwrap();
                    reused.clear();
                    emit_sha_batch_relation_v1(
                        &current,
                        &next,
                        word_challenges(),
                        challenges(),
                        &rfc_challenges(),
                        segment,
                        boundaries,
                        &mut reused,
                    )
                    .unwrap();
                    assert_eq!(reused, expected);
                    assert_eq!((reused.as_ptr(), reused.capacity()), allocation);
                }
                let expected = evaluate_zk_x509_sha_batch_local_residues_over_field_v1(
                    &current,
                    &next,
                    word_challenges(),
                    challenges(),
                    &rfc_challenges(),
                    segment,
                )
                .unwrap();
                reused.clear();
                emit_sha_batch_relation_v1(
                    &current,
                    &next,
                    word_challenges(),
                    challenges(),
                    &rfc_challenges(),
                    segment,
                    None,
                    &mut reused,
                )
                .unwrap();
                assert_eq!(reused, expected);
                assert_eq!((reused.as_ptr(), reused.capacity()), allocation);
            }
        }
    }
    compare(|index| {
        F(if index % 3 == 0 {
            GOLDILOCKS_MODULUS_V1 - 1 - index as u64
        } else {
            index as u64
        })
    });
    compare(|index| {
        crate::privacy_engines::transparent_stark::GoldilocksFp4V1::from_coefficients(
            core::array::from_fn(|lane| F((index * 7 + lane) as u64)),
        )
        .unwrap()
    });
}

#[test]
fn reusable_sha_residues_keep_original_refusal_order_and_destination() {
    for fault in 0..10 {
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
            _ => terminals[0].call = 255,
        }
        let mut reused = vec![F(93); ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1];
        let before = reused.clone();
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
        let actual = emit_sha_batch_relation_v1(
            &current,
            &next,
            word,
            call,
            &rfc,
            segment,
            Some(&terminals),
            &mut reused,
        )
        .unwrap_err();
        assert_eq!(actual, expected);
        assert_eq!(reused, before);
    }
}

#[test]
#[ignore = "controlled SHA allocation-versus-reuse cost diagnostic"]
fn reusable_sha_residues_cost_preserves_full_outputs() {
    use std::{hint::black_box, time::Instant};
    let current = ZkX509ShaBatchRowV1 {
        base: core::array::from_fn(|i| F(i as u64 + 1)),
        aux: core::array::from_fn(|i| F(i as u64 + 17)),
        fixed: core::array::from_fn(|i| F(i as u64 + 31)),
    };
    let next = current;
    let expected = evaluate_zk_x509_sha_batch_local_residues_over_field_v1(
        &current,
        &next,
        word_challenges(),
        challenges(),
        &rfc_challenges(),
        0,
    )
    .unwrap();
    let mut reused = Vec::with_capacity(ZK_X509_SHA_BATCH_LOCAL_CONSTRAINT_COUNT_V1);
    let allocation = (reused.as_ptr(), reused.capacity());
    const ROWS: usize = 1 << 15;
    for round in 0..4 {
        for reuse in if round % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let start = Instant::now();
            for _ in 0..ROWS {
                if reuse {
                    reused.clear();
                    emit_sha_batch_relation_v1(
                        black_box(&current),
                        black_box(&next),
                        word_challenges(),
                        challenges(),
                        &rfc_challenges(),
                        0,
                        None,
                        &mut reused,
                    )
                    .unwrap();
                    black_box(&reused);
                } else {
                    black_box(
                        evaluate_zk_x509_sha_batch_local_residues_over_field_v1(
                            black_box(&current),
                            black_box(&next),
                            word_challenges(),
                            challenges(),
                            &rfc_challenges(),
                            0,
                        )
                        .unwrap(),
                    );
                }
            }
            let elapsed = start.elapsed();
            if reuse {
                assert_eq!(reused, expected);
                assert_eq!((reused.as_ptr(), reused.capacity()), allocation);
            }
            eprintln!(
                "SHA residue storage: round={round}, reuse={reuse}, rows={ROWS}, seconds={:.9}",
                elapsed.as_secs_f64()
            );
        }
    }
}
