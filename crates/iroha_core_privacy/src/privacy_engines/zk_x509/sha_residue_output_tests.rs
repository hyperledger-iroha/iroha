// Included in the original call-bus test module, using its genuine challenges.

struct FixedResidues<A: PolynomialAirFieldV1> {
    cells: [A; ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1],
    count: usize,
    reserves: Vec<usize>,
}

impl<A: PolynomialAirFieldV1> FixedResidues<A> {
    fn new() -> Self {
        Self {
            cells: [A::ZERO; ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1],
            count: 0,
            reserves: Vec::new(),
        }
    }
    fn values(&self) -> &[A] {
        &self.cells[..self.count]
    }
}

impl<A: PolynomialAirFieldV1> ShaResidueSinkV1<A> for FixedResidues<A> {
    fn len(&self) -> usize {
        self.count
    }
    fn reserve(&mut self, additional: usize) {
        self.reserves.push(additional);
    }
    fn push(&mut self, value: A) {
        assert!(
            self.count < self.cells.len(),
            "public residue extent exceeded"
        );
        self.cells[self.count] = value;
        self.count += 1;
    }
}

fn compare_residue_outputs<A: PolynomialAirFieldV1 + PartialEq>(embed: impl Fn(usize) -> A) {
    use super::super::sha_word_stark as word;
    for seed in 0..8 {
        let current = ZkX509ShaBatchRowV1 {
            base: core::array::from_fn(|i| embed(seed + i)),
            aux: core::array::from_fn(|i| embed(seed + 7 + i)),
            fixed: core::array::from_fn(|i| embed(seed + 11 + i)),
        };
        let next = ZkX509ShaBatchRowV1 {
            base: core::array::from_fn(|i| embed(seed + 17 + i)),
            aux: core::array::from_fn(|i| embed(seed + 23 + i)),
            fixed: core::array::from_fn(|i| embed(seed + 29 + i)),
        };
        let raw_current = current.base[..word::SHA_WORD_BASE_WIDTH_V1]
            .try_into()
            .unwrap();
        let raw_next = next.base[..word::SHA_WORD_BASE_WIDTH_V1]
            .try_into()
            .unwrap();
        let raw_aux = current.aux[..word::SHA_WORD_AUX_WIDTH_V1]
            .try_into()
            .unwrap();
        let raw_next_aux = next.aux[..word::SHA_WORD_AUX_WIDTH_V1].try_into().unwrap();
        let raw_fixed = current.fixed[..word::SHA_WORD_STARK_FIXED_WIDTH_V1]
            .try_into()
            .unwrap();
        let expected =
            word::residue_oracles::original_evaluate_zk_x509_sha_word_stark_residues_over_field_v1(
                raw_current,
                raw_next,
                raw_aux,
                raw_next_aux,
                raw_fixed,
                word_challenges(),
            )
            .unwrap();
        let mut actual = FixedResidues::new();
        word::emit_zk_x509_sha_word_stark_residues_over_field_v1(
            raw_current,
            raw_next,
            raw_aux,
            raw_next_aux,
            raw_fixed,
            word_challenges(),
            &mut actual,
        )
        .unwrap();
        assert!(
            actual.values() == expected,
            "raw public residue order differs"
        );
        assert_eq!(actual.count, 155);
        assert_eq!(actual.reserves, [155]);
        let collecting = word::evaluate_zk_x509_sha_word_stark_residues_over_field_v1(
            raw_current,
            raw_next,
            raw_aux,
            raw_next_aux,
            raw_fixed,
            word_challenges(),
        )
        .unwrap();
        assert!(collecting == expected);

        let cap_current = current.base[..word::SHA_WORD_CAPACITY_BASE_WIDTH_V1]
            .try_into()
            .unwrap();
        let cap_next = next.base[..word::SHA_WORD_CAPACITY_BASE_WIDTH_V1]
            .try_into()
            .unwrap();
        let cap_aux = current.aux[..word::SHA_WORD_CAPACITY_AUX_WIDTH_V1]
            .try_into()
            .unwrap();
        let cap_next_aux = next.aux[..word::SHA_WORD_CAPACITY_AUX_WIDTH_V1]
            .try_into()
            .unwrap();
        let cap_fixed = current.fixed[..word::SHA_WORD_CAPACITY_FIXED_WIDTH_V1]
            .try_into()
            .unwrap();
        let expected = word::residue_oracles::original_evaluate_zk_x509_sha_word_capacity_residues_over_field_v1(
            cap_current, cap_next, cap_aux, cap_next_aux, cap_fixed, word_challenges()).unwrap();
        let mut actual = FixedResidues::new();
        word::emit_zk_x509_sha_word_capacity_residues_over_field_v1(
            cap_current,
            cap_next,
            cap_aux,
            cap_next_aux,
            cap_fixed,
            word_challenges(),
            &mut actual,
        )
        .unwrap();
        assert!(
            actual.values() == expected,
            "capacity public residue order differs"
        );
        assert_eq!(actual.count, 335);
        assert_eq!(actual.reserves, [155]);

        let boundaries = neutral_ca_call_boundaries();
        for segment in 0..4 {
            for complete in [false, true] {
                let boundaries = complete.then_some(&boundaries);
                let expected = original_evaluate_sha_batch_relation_v1(
                    &current,
                    &next,
                    word_challenges(),
                    challenges(),
                    &rfc_challenges(),
                    segment,
                    boundaries,
                )
                .unwrap();
                let mut actual = FixedResidues::new();
                emit_sha_batch_relation_v1(
                    &current,
                    &next,
                    word_challenges(),
                    challenges(),
                    &rfc_challenges(),
                    segment,
                    boundaries,
                    &mut actual,
                )
                .unwrap();
                assert!(
                    actual.values() == expected,
                    "batch public residue order differs"
                );
                assert_eq!(actual.count, if complete { 801 } else { 593 });
                assert_eq!(actual.reserves, [155]);
                let collecting = evaluate_sha_batch_relation_v1(
                    &current,
                    &next,
                    word_challenges(),
                    challenges(),
                    &rfc_challenges(),
                    segment,
                    boundaries,
                )
                .unwrap();
                assert!(collecting == expected);
            }
        }
    }
}

#[test]
fn ordered_emission_matches_preserved_raw_capacity_and_batch_bodies_over_f_and_fp4() {
    compare_residue_outputs(|i| F((i % 17) as u64));
    compare_residue_outputs(|i| {
        crate::privacy_engines::transparent_stark::GoldilocksFp4V1::canonical([
            (i % 17) as u64,
            (i % 11) as u64,
            (i % 7) as u64,
            (i % 5) as u64,
        ])
        .unwrap()
    });
}

#[test]
fn malformed_rows_challenges_and_segments_preserve_first_error_without_emission() {
    for failure in 0..7 {
        let mut current = ZkX509ShaBatchRowV1 {
            base: [F::ZERO; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
            aux: [F::ZERO; ZK_X509_SHA_BATCH_AUX_WIDTH_V1],
            fixed: [F::ZERO; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
        };
        let next = current;
        let mut word = word_challenges();
        let mut call = challenges();
        let mut rfc = rfc_challenges();
        let mut segment = 0;
        match failure {
            0 => current.base[0] = F(GOLDILOCKS_MODULUS_V1),
            1 => current.aux[0] = F(GOLDILOCKS_MODULUS_V1),
            2 => current.fixed[0] = F(GOLDILOCKS_MODULUS_V1),
            3 => word.memory.lanes[0].beta = F::ZERO,
            4 => call.lanes[0].terms[0] = F::ZERO,
            5 => rfc.tuple[0][0] = F::ZERO,
            _ => {
                call.lanes[0].terms[0] = F::ZERO;
                segment = 4;
            }
        }
        let expected = original_evaluate_sha_batch_relation_v1(
            &current, &next, word, call, &rfc, segment, None,
        )
        .unwrap_err();
        let mut actual = FixedResidues::new();
        let observed = emit_sha_batch_relation_v1(
            &current,
            &next,
            word,
            call,
            &rfc,
            segment,
            None,
            &mut actual,
        )
        .unwrap_err();
        assert_eq!(observed, expected);
        assert_eq!(actual.count, 0);
        assert!(actual.reserves.is_empty());
    }
    let current = physical_padding_row_v1(0);
    let expected = original_evaluate_sha_batch_relation_v1(
        &current,
        &current,
        word_challenges(),
        challenges(),
        &rfc_challenges(),
        4,
        None,
    )
    .unwrap_err();
    let mut actual = FixedResidues::new();
    assert_eq!(
        emit_sha_batch_relation_v1(
            &current,
            &current,
            word_challenges(),
            challenges(),
            &rfc_challenges(),
            4,
            None,
            &mut actual
        )
        .unwrap_err(),
        expected
    );
    assert_eq!(actual.count, 0);
}
