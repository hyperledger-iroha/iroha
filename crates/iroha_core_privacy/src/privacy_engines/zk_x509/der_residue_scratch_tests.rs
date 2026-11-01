// Complete DER residue parity and cost with one reused destination allocation.

#[test]
fn reusable_der_residues_preserve_fields_terminals_and_capacity() {
    fn compare<A: PolynomialAirFieldV1 + core::fmt::Debug + PartialEq>(embed: impl Fn(usize) -> A) {
        let challenge = challenges();
        let public =
            derive_zk_x509_der_stark_public_terminals_v1(ZkX509DerStarkShapeV1, &challenge)
                .unwrap();
        let mut reused = Vec::with_capacity(ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1);
        let allocation = (reused.as_ptr(), reused.capacity());
        for seed in 0..8 {
            let current = core::array::from_fn(|i| embed(seed + i));
            let next = core::array::from_fn(|i| embed(seed + 13 + i));
            let aux = core::array::from_fn(|i| embed(seed + 17 + i));
            let next_aux = core::array::from_fn(|i| embed(seed + 19 + i));
            let fixed = core::array::from_fn(|i| embed(seed + 23 + i));
            let next_fixed = core::array::from_fn(|i| embed(seed + 29 + i));
            let terminals = ZkX509DerStarkTerminalClaimsV1 {
                input_byte: [F(37); 4],
                node: [F(41); 4],
            };
            let full = evaluate_zk_x509_der_stark_residues_v1(
                &current,
                &next,
                &aux,
                &next_aux,
                &fixed,
                &next_fixed,
                &challenge,
                public,
                terminals,
            )
            .unwrap();
            evaluate_zk_x509_der_stark_residues_into_v1(
                &current,
                &next,
                &aux,
                &next_aux,
                &fixed,
                &next_fixed,
                &challenge,
                public,
                terminals,
                &mut reused,
            )
            .unwrap();
            assert_eq!(reused, full);
            assert_eq!((reused.as_ptr(), reused.capacity()), allocation);
            let local = evaluate_zk_x509_der_stark_local_residues_v1(
                &current,
                &next,
                &aux,
                &next_aux,
                &fixed,
                &next_fixed,
                &challenge,
                public,
            )
            .unwrap();
            evaluate_zk_x509_der_stark_local_residues_into_v1(
                &current,
                &next,
                &aux,
                &next_aux,
                &fixed,
                &next_fixed,
                &challenge,
                public,
                &mut reused,
            )
            .unwrap();
            assert_eq!(reused, local);
            assert_eq!((reused.as_ptr(), reused.capacity()), allocation);
        }
    }
    compare(|index| {
        F(if index % 3 == 0 {
            crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1 - 1 - index as u64
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
fn reusable_der_residues_keep_refusals_and_destination() {
    let challenge = challenges();
    let public =
        derive_zk_x509_der_stark_public_terminals_v1(ZkX509DerStarkShapeV1, &challenge).unwrap();
    for fault in 0..7 {
        let mut current = [F::ZERO; ZK_X509_DER_STARK_BASE_WIDTH_V1];
        let mut next = current;
        let mut aux = [F::ZERO; ZK_X509_DER_STARK_AUX_WIDTH_V1];
        let mut next_aux = aux;
        let mut fixed = [F::ZERO; ZK_X509_DER_STARK_FIXED_WIDTH_V1];
        let mut next_fixed = fixed;
        let mut challenge = challenge;
        let noncanonical = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        match fault {
            0 => current[0] = noncanonical,
            1 => next[0] = noncanonical,
            2 => aux[0] = noncanonical,
            3 => next_aux[0] = noncanonical,
            4 => fixed[0] = noncanonical,
            5 => next_fixed[0] = noncanonical,
            _ => challenge.tuple[0][0] = F::ZERO,
        }
        let mut reused = vec![F(91); ZK_X509_DER_STARK_LOCAL_CONSTRAINT_COUNT_V1];
        let before = reused.clone();
        let expected = evaluate_zk_x509_der_stark_local_residues_v1(
            &current,
            &next,
            &aux,
            &next_aux,
            &fixed,
            &next_fixed,
            &challenge,
            public,
        )
        .unwrap_err();
        let actual = evaluate_zk_x509_der_stark_local_residues_into_v1(
            &current,
            &next,
            &aux,
            &next_aux,
            &fixed,
            &next_fixed,
            &challenge,
            public,
            &mut reused,
        )
        .unwrap_err();
        assert_eq!(actual, expected);
        assert_eq!(reused, before);
    }
}

#[test]
#[ignore = "controlled DER allocation-versus-reuse cost diagnostic"]
fn reusable_der_residues_cost_preserves_full_outputs() {
    use std::{hint::black_box, time::Instant};
    let challenge = challenges();
    let public =
        derive_zk_x509_der_stark_public_terminals_v1(ZkX509DerStarkShapeV1, &challenge).unwrap();
    let current = core::array::from_fn(|i| F(i as u64 + 1));
    let next = core::array::from_fn(|i| F(i as u64 + 13));
    let aux = core::array::from_fn(|i| F(i as u64 + 17));
    let next_aux = core::array::from_fn(|i| F(i as u64 + 19));
    let fixed = core::array::from_fn(|i| F(i as u64 + 23));
    let next_fixed = core::array::from_fn(|i| F(i as u64 + 29));
    let expected = evaluate_zk_x509_der_stark_local_residues_v1(
        &current,
        &next,
        &aux,
        &next_aux,
        &fixed,
        &next_fixed,
        &challenge,
        public,
    )
    .unwrap();
    let mut reused = Vec::with_capacity(ZK_X509_DER_STARK_LOCAL_CONSTRAINT_COUNT_V1);
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
                    evaluate_zk_x509_der_stark_local_residues_into_v1(
                        black_box(&current),
                        black_box(&next),
                        &aux,
                        &next_aux,
                        &fixed,
                        &next_fixed,
                        &challenge,
                        public,
                        &mut reused,
                    )
                    .unwrap();
                    black_box(&reused);
                } else {
                    black_box(
                        evaluate_zk_x509_der_stark_local_residues_v1(
                            black_box(&current),
                            black_box(&next),
                            &aux,
                            &next_aux,
                            &fixed,
                            &next_fixed,
                            &challenge,
                            public,
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
                "DER residue storage: round={round}, reuse={reuse}, rows={ROWS}, seconds={:.9}",
                elapsed.as_secs_f64()
            );
        }
    }
}
