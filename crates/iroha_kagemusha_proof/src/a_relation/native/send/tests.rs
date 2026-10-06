//! Exact Send original/selector checks; syntax fixtures are not admitted monetary heads.
use super::*;
fn input(mask: u8) -> Inputs {
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
    let sigma = (0..64).collect::<Vec<u8>>();
    let mut raw = (sigma.len() as u32).to_le_bytes().to_vec();
    raw.extend(&sigma);
    let digest = hash_with_domain(
        iroha_plonk_gadgets::statement::STATEMENT_DOMAIN,
        &state.statement,
    );
    let mut bounded = vec![Fq::from_repr(digest.to_repr()).unwrap()];
    bounded.extend(raw.chunks(31).map(|c| le_value::<Fq>(c).unwrap()));
    bounded.extend([Fq::ONE; K]);
    let path = IndexedInsert {
        leaf: crate::tree::IndexedLeaf::default(),
        leaf_slot: 0,
        leaf_siblings: [Fp::ZERO; 32],
        slot: 1,
        slot_siblings: [Fp::ZERO; 32],
    };
    Inputs {
        state,
        sigma,
        omega: vec![],
        objects: core::array::from_fn(|_| vec![]),
        pending: path,
        fee: path,
        q: [
            QInput {
                proof: vec![],
                instances: vec![
                    bounded,
                    vec![],
                    vec![Fq::from(2 + u64::from(mask))],
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
fn all_eight_send_selectors_bind_the_exact_original_sigma_and_statement() {
    for mask in 0..8 {
        let source = input(mask);
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
        let mut q = input(0).q[0].clone();
        q.instances[1] = vec![x, y];
        q.instances[4] = vec![Fq::from(u64::from(k))];
        let n = q.instances[0].len();
        q.instances[0][n - K..n - k as usize].fill(Fq::ZERO);
        assert!(q_sigma_part(&q, k).is_ok());
        let mut bad = q.clone();
        bad.instances[0][n - k as usize] = Fq::ZERO;
        assert!(q_sigma_part(&bad, k).is_err());
        let mut bad = q.clone();
        bad.instances[0][n - K] = Fq::ONE;
        assert!(q_sigma_part(&bad, k).is_err());
        let mut bad = q.clone();
        bad.instances[3][0] = Fq::ZERO;
        assert!(q_sigma_part(&bad, k).is_err());
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
            .map(|i| i as u8)
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
