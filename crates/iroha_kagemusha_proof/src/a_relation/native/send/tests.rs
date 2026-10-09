//! Pure exact-original and homogeneous-frame rejection tests, never an admitted Send.

use super::*;

fn original(mask: u8) -> Inputs {
    let state = StateWitness {
        core: [Fp::ZERO; 33],
        rest: [Fp::ZERO; 8],
        lineage: [Fp::ZERO; 18],
    };
    let mut state = SendState {
        before: state,
        after: state,
        statement: [Fp::ZERO; 26],
    };
    state.statement[11] = Fp::from(u64::from(mask));
    state.before.core[21] = Fp::from(u64::from(mask));
    state.after.core[21] = Fp::from(u64::from(mask));
    let sigma = (0..64).collect::<Vec<u8>>();
    let mut raw = u32::try_from(sigma.len()).unwrap().to_le_bytes().to_vec();
    raw.extend(&sigma);
    let statement = hash_with_domain(
        iroha_plonk_gadgets::statement::STATEMENT_DOMAIN,
        &state.statement,
    );
    let mut bounded = vec![Fq::from_repr(statement.to_repr()).unwrap()];
    bounded.extend(raw.chunks(31).map(|chunk| le_value::<Fq>(chunk).unwrap()));
    bounded.extend([Fq::ONE; K]);
    Inputs {
        state,
        sigma,
        omega: vec![],
        objects: core::array::from_fn(|_| vec![]),
        pending: IndexedInsert {
            leaf: crate::tree::IndexedLeaf::default(),
            leaf_slot: 0,
            leaf_siblings: [Fp::ZERO; 32],
            slot: 1,
            slot_siblings: [Fp::ZERO; 32],
        },
        fee: IndexedInsert {
            leaf: crate::tree::IndexedLeaf::default(),
            leaf_slot: 0,
            leaf_siblings: [Fp::ZERO; 32],
            slot: 1,
            slot_siblings: [Fp::ZERO; 32],
        },
        q: [
            QInput {
                proof: vec![],
                instances: vec![
                    bounded,
                    vec![],
                    vec![send_selector(mask).unwrap()],
                    vec![Fq::ONE],
                    vec![Fq::from(12)],
                ],
            },
            QInput {
                proof: vec![],
                instances: vec![],
            },
        ],
        predecessor: PredecessorInput {
            proof: vec![],
            pallas: [0; 544],
            vesta: [0; 544],
        },
    }
}

#[test]
fn exact_send_sigma_tape_statement_length_and_selector_are_bound() {
    let source = original(0);
    assert_eq!(check_sigma_tape(&source, 0), Ok(()));
    for mutation in 0..6 {
        let mut bad = source.clone();
        match mutation {
            0 => bad.sigma[40] ^= 1,
            1 => bad.sigma.push(7),
            2 => bad.q[0].instances[0][1] += Fq::ONE,
            3 => bad.q[0].instances[2][0] = Fq::ZERO,
            4 => bad.q[0].instances[2][0] = Fq::from(3),
            _ => bad.state.statement[20] += Fp::ONE,
        }
        assert_eq!(
            check_sigma_tape(&bad, 0),
            Err(Error::Input),
            "mutation{mutation}"
        );
    }
}

#[test]
fn send_q_sigma_normalization_rejects_wrong_selector_source_or_scalar_alias() {
    let mut input = original(0).q[0].clone();
    let point = iroha_plonk::transcript::decode_point::<Eq>(
        &iroha_plonk_recursion::VESTA_TRIVIAL_GENERATOR,
    )
    .unwrap();
    let (x, y) = Option::<(Fq, Fq)>::from(point.coordinates()).unwrap();
    input.instances[1] = vec![x, y];
    let n = input.instances[0].len();
    input.instances[0][n - K..n - K + 4].fill(Fq::ZERO);
    assert!(q_sigma_part(&input, 12, 0).is_ok());
    for mutation in 0..6 {
        let mut bad = input.clone();
        match mutation {
            0 => bad.instances[0][n - K] = Fq::ONE,
            1 => bad.instances[0][n - 1] = Fq::ZERO,
            2 => bad.instances[1] = vec![Fq::ZERO, Fq::ZERO],
            3 => bad.instances[3][0] = Fq::ZERO,
            4 => bad.instances[4][0] = Fq::from(16),
            _ => bad.instances[2][0] = Fq::ZERO,
        }
        assert!(q_sigma_part(&bad, 12, 0).is_err(), "mutation{mutation}");
    }
    let p = [
        0x992d_30ed_0000_0001_u64,
        0x2246_98fc_094c_f91b,
        0,
        0x4000_0000_0000_0000,
    ];
    let mut repr = [0; 32];
    for (to, word) in repr.chunks_exact_mut(8).zip(p) {
        to.copy_from_slice(&word.to_le_bytes());
    }
    let mut alias = input;
    alias.instances[0][n - 1] = Fq::from_repr(repr).unwrap();
    assert!(q_sigma_part(&alias, 12, 0).is_err());
}

#[test]
fn all_five_signed_originals_bind_body_and_each_raw_signature_limb() {
    assert_eq!(
        object_kinds(),
        [
            ObjectKind::Credential,
            ObjectKind::Request,
            ObjectKind::FeeSchedule,
            ObjectKind::Certificate,
            ObjectKind::Receipt
        ]
    );
    for kind in object_kinds() {
        let raw = (0..kind.body_len() + 64)
            .map(|i| u8::try_from(i % 256).unwrap())
            .collect::<Vec<_>>();
        let digest = object_digest(kind, &raw).unwrap();
        for index in [
            0,
            kind.body_len() - 1,
            kind.body_len(),
            kind.body_len() + 16,
            kind.body_len() + 32,
            kind.body_len() + 48,
        ] {
            let mut changed = raw.clone();
            changed[index] ^= 1;
            assert_ne!(object_digest(kind, &changed).unwrap(), digest);
        }
        assert!(object_digest(kind, &raw[..raw.len() - 1]).is_err());
    }
}

#[test]
fn final_digest_has_exact52_fields_and_keeps_high_foreign_challenge_limbs() {
    let ep = iroha_plonk::transcript::decode_point::<Ep>(
        &iroha_plonk_recursion::PALLAS_TRIVIAL_GENERATOR,
    )
    .unwrap();
    let eq = iroha_plonk::transcript::decode_point::<Eq>(
        &iroha_plonk_recursion::VESTA_TRIVIAL_GENERATOR,
    )
    .unwrap();
    let mut challenges = [Fq::ONE; K];
    challenges[15] = Fq::from(2).pow_vartime([200]);
    let pallas = FoldInput::from_normalized(ep, 16, challenges).unwrap();
    let fields = core::array::from_fn(|i| Fp::from(i as u64 + 1));
    let digest = terminal_digest(&fields, &pallas).unwrap();
    let (x, y) = Option::<(Fp, Fp)>::from(ep.coordinates()).unwrap();
    let mut expected = fields.to_vec();
    expected.extend([x, y]);
    for challenge in challenges {
        let repr = challenge.to_repr();
        expected.push(Fp::from_u128(u128::from_le_bytes(
            repr[..16].try_into().unwrap(),
        )));
        expected.push(Fp::from_u128(u128::from_le_bytes(
            repr[16..].try_into().unwrap(),
        )));
    }
    assert_eq!(expected.len(), 52);
    assert_eq!(
        digest,
        hash_with_domain(super::super::super::LINEAGE_DOMAIN, &expected)
    );
    challenges[15] = Fq::ONE;
    assert_ne!(
        digest,
        terminal_digest(
            &fields,
            &FoldInput::from_normalized(ep, 16, challenges).unwrap()
        )
        .unwrap()
    );
    let part = FoldInput::from_normalized(eq, 16, [Fp::ONE; K]).unwrap();
    let public = internal_public(digest, &part).unwrap();
    assert_eq!(public.len(), 69);
    assert_eq!(&public[62..65], &[Fp::ZERO, Fp::ONE, Fp::ZERO]);
    assert_eq!(&public[22..42], &public[42..62]);
    assert_eq!(&public[65..69], &public[22..26]);
}

#[test]
fn fixed_controls_off_mask_rejects_each_foreign_control_source() {
    for index in 0..3 {
        let mut input = original(0);
        match index {
            0 => input.state.before.core[21] = Fp::ONE,
            1 => input.state.after.core[21] = Fp::ONE,
            _ => input.state.statement[11] = Fp::ONE,
        }
        assert_eq!(check_sigma_tape(&input, 0), Err(Error::Input));
    }
}

#[test]
fn consuming_originals_use_exact_public320_proof_and_two544_claims() {
    let p = iroha_plonk::transcript::decode_point::<Ep>(
        &iroha_plonk_recursion::PALLAS_TRIVIAL_GENERATOR,
    )
    .unwrap();
    let v = iroha_plonk::transcript::decode_point::<Eq>(
        &iroha_plonk_recursion::VESTA_TRIVIAL_GENERATOR,
    )
    .unwrap();
    let p = AccumulatorT::new(p, [Fq::ONE; K]).unwrap();
    let v = AccumulatorT::new(v, [Fp::ONE; K]).unwrap();
    let fields = core::array::from_fn(|i| Fp::from(u64::try_from(i + 1).unwrap()));
    let proof = vec![41; 3712];
    let tape = lineage_bytes(&fields, &proof, &p, &v).unwrap();
    assert_eq!(tape.len(), 320 + 3712 + 1088);
    assert_eq!(tape[0..2], 1_u16.to_le_bytes());
    assert_eq!(tape[162], 4);
    assert_eq!(
        tape[163..179],
        fields[10].to_repr()[..16]
            .iter()
            .copied()
            .rev()
            .collect::<Vec<_>>()
    );
    assert_eq!(tape[320..4032], proof);
    assert_eq!(tape[4032..4576], p.to_bytes());
    assert_eq!(tape[4576..], v.to_bytes());
    let framed = frame(&tape).unwrap();
    assert_eq!(
        framed[..4],
        u32::try_from(tape.len()).unwrap().to_le_bytes()
    );
    assert_eq!(framed[4..], tape);
    for index in [1, 2, 3, 4, 6, 7, 9, 10, 11, 12, 14] {
        let mut changed = fields;
        changed[index] += Fp::from(2).pow_vartime([128]);
        assert_eq!(lineage_bytes(&changed, &proof, &p, &v), Err(Error::Input));
    }
    let mut changed = fields;
    changed[13] += Fp::from(2).pow_vartime([104]);
    assert_eq!(lineage_bytes(&changed, &proof, &p, &v), Err(Error::Input));
    changed = fields;
    changed[0] = Fp::from(2);
    assert_eq!(lineage_bytes(&changed, &proof, &p, &v), Err(Error::Input));
    // Omega's key identity is checked separately against the exact predecessor VK.
    changed = fields;
    changed[17] += Fp::ONE;
    assert_eq!(lineage_bytes(&changed, &proof, &p, &v).unwrap(), tape);
    for index in [5, 8, 15, 16] {
        changed = fields;
        changed[index] += Fp::ONE;
        assert_ne!(lineage_bytes(&changed, &proof, &p, &v).unwrap(), tape);
    }
}

#[test]
fn all_eight_send_selectors_bind_the_exact_original_sigma_and_statement() {
    for mask in 0..8 {
        let source = original(mask);
        assert_eq!(check_sigma_tape(&source, mask), Ok(()));
        for changed in 0..8 {
            if changed != mask {
                assert_eq!(check_sigma_tape(&source, changed), Err(Error::Input));
            }
        }
        let mut bad = source.clone();
        bad.sigma[40] ^= 1;
        assert_eq!(check_sigma_tape(&bad, mask), Err(Error::Input));
        let mut bad = source.clone();
        bad.state.statement[20] += Fp::ONE;
        assert_eq!(check_sigma_tape(&bad, mask), Err(Error::Input));
    }
}
#[test]
fn both_source_classes_require_exact_challenge_normalization_and_hard_verdict() {
    let eq = iroha_plonk::transcript::decode_point::<Eq>(
        &iroha_plonk_recursion::VESTA_TRIVIAL_GENERATOR,
    )
    .unwrap();
    let (x, y) = Option::<(Fq, Fq)>::from(eq.coordinates()).unwrap();
    for k in [12_u32, 14] {
        let mut q = original(0).q[0].clone();
        q.instances[1] = vec![x, y];
        q.instances[4] = vec![Fq::from(u64::from(k))];
        let n = q.instances[0].len();
        q.instances[0][n - K..n - k as usize].fill(Fq::ZERO);
        assert!(q_sigma_part(&q, k, 0).is_ok());
        let mut bad = q.clone();
        bad.instances[0][n - k as usize] = Fq::ZERO;
        assert!(q_sigma_part(&bad, k, 0).is_err());
        let mut bad = q.clone();
        bad.instances[0][n - K] = Fq::ONE;
        assert!(q_sigma_part(&bad, k, 0).is_err());
        let mut bad = q.clone();
        bad.instances[3][0] = Fq::ZERO;
        assert!(q_sigma_part(&bad, k, 0).is_err());
    }
}
#[test]
fn exact_five_object_context_preserves_signed_request_and_total_fee_slot() {
    assert_eq!(
        object_kinds(),
        [
            ObjectKind::Credential,
            ObjectKind::Request,
            ObjectKind::FeeSchedule,
            ObjectKind::Certificate,
            ObjectKind::Receipt
        ]
    );
    for kind in object_kinds() {
        let raw = (0..kind.body_len() + 64)
            .map(|i| u8::try_from(i % 256).unwrap())
            .collect::<Vec<_>>();
        let digest = object_digest(kind, &raw).unwrap();
        for i in [
            0,
            kind.body_len() - 1,
            kind.body_len(),
            kind.body_len() + 16,
            kind.body_len() + 32,
            kind.body_len() + 48,
        ] {
            let mut changed = raw.clone();
            changed[i] ^= 1;
            assert_ne!(object_digest(kind, &changed).unwrap(), digest);
        }
        assert!(object_digest(kind, &raw[..raw.len() - 1]).is_err());
    }
}
#[test]
fn consuming_lineage_has_exact320_original_public_bytes_and_both_claims() {
    let ep = iroha_plonk::transcript::decode_point::<Ep>(
        &iroha_plonk_recursion::PALLAS_TRIVIAL_GENERATOR,
    )
    .unwrap();
    let eq = iroha_plonk::transcript::decode_point::<Eq>(
        &iroha_plonk_recursion::VESTA_TRIVIAL_GENERATOR,
    )
    .unwrap();
    let p = AccumulatorT::new(ep, [Fq::ONE; K]).unwrap();
    let v = AccumulatorT::new(eq, [Fp::ONE; K]).unwrap();
    let mut fields = core::array::from_fn(|i| Fp::from(i as u64 + 1));
    fields[0] = Fp::ONE;
    let raw = lineage_bytes(&fields, &[13; 64], &p, &v).unwrap();
    assert_eq!(raw.len(), 320 + 64 + 2 * 544);
    assert_eq!(&raw[320..384], &[13; 64]);
    assert_eq!(&raw[384..928], &p.to_bytes());
    assert_eq!(&raw[928..], &v.to_bytes());
    for i in [1, 6, 9, 13, 14] {
        let mut bad = fields;
        bad[i] = Fp::from(2).pow_vartime([200]);
        assert!(lineage_bytes(&bad, &[13; 64], &p, &v).is_err());
    }
    let framed = frame(&raw).unwrap();
    assert_eq!(
        u32::from_le_bytes(framed[..4].try_into().unwrap()) as usize,
        raw.len()
    );
    assert_eq!(&framed[4..], raw);
}

#[test]
fn original_bounds_reject_missing_oversized_and_over_domain_tables() {
    let config = ReadConfig {
        maximum_bytes: 16,
        maximum_rows: 1 << 16,
        coset_cache: iroha_plonk::keys::CosetCachePolicy::OnDemand,
        msm_budget: MemoryBudget::DEFAULT,
    };
    assert_eq!(original_bounds(&[], 1 << 16, config), Err(Error::Artifact));
    assert_eq!(
        original_bounds(&[0; 17], 1 << 16, config),
        Err(Error::Artifact)
    );
    assert_eq!(
        original_bounds(&[0; 16], (1 << 16) + 1, config),
        Err(Error::Artifact)
    );
    assert_eq!(original_bounds(&[0; 16], 1 << 16, config), Ok(()));
    // Passing these bounds alone never parses or admits an original proving key.
}
