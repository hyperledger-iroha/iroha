//! Pure source-frame mutation checks. No fixture here is an admitted operation.
use super::*;

fn original() -> Inputs {
    let state = BootstrapWitness {
        core: [Fp::ZERO; 33],
        rest: [Fp::ZERO; 8],
        lineage: [Fp::ZERO; 18],
        statement: [Fp::ZERO; 26],
    };
    let sigma = (0..64).collect::<Vec<u8>>();
    let mut raw = (sigma.len() as u32).to_le_bytes().to_vec();
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
        objects: [vec![], vec![], vec![]],
        q: [
            QInput {
                proof: vec![],
                instances: vec![
                    bounded,
                    vec![],
                    vec![Fq::ZERO],
                    vec![Fq::ONE],
                    vec![Fq::from(12)],
                ],
            },
            QInput {
                proof: vec![],
                instances: vec![],
            },
        ],
    }
}

#[test]
fn original_sigma_tape_cannot_be_replaced_with_consistent_statement_only() {
    let input = original();
    assert_eq!(check_sigma_tape(&input), Ok(()));
    for mutation in 0..5 {
        let mut bad = input.clone();
        match mutation {
            0 => bad.sigma[40] ^= 1,
            1 => bad.sigma.push(7),
            2 => bad.q[0].instances[0][1] += Fq::ONE,
            3 => bad.q[0].instances[2][0] = Fq::ONE,
            _ => bad.state.statement[20] += Fp::ONE,
        }
        assert_eq!(
            check_sigma_tape(&bad),
            Err(Error::Input),
            "mutation{mutation}"
        );
    }
}

#[test]
fn malformed_q_exports_never_select_a_scalar_alias_or_short_padding() {
    let mut input = original().q[0].clone();
    let point = decode_point::<Eq>(&VESTA_TRIVIAL_GENERATOR).unwrap();
    let (x, y) = Option::<(Fq, Fq)>::from(point.coordinates()).unwrap();
    input.instances[1] = vec![x, y];
    let n = input.instances[0].len();
    input.instances[0][n - K..n - K + 4].fill(Fq::ZERO);
    assert!(q_sigma_part(&input, 12).is_ok());
    for mutation in 0..5 {
        let mut bad = input.clone();
        match mutation {
            0 => bad.instances[0][n - K] = Fq::ONE,
            1 => bad.instances[0][n - 1] = Fq::ZERO,
            2 => bad.instances[1] = vec![Fq::ZERO, Fq::ZERO],
            3 => bad.instances[3][0] = Fq::ZERO,
            _ => bad.instances[4][0] = Fq::from(16),
        }
        assert!(q_sigma_part(&bad, 12).is_err(), "mutation{mutation}");
    }
    // p is a valid Fq integer, but it is not a canonical Fp scalar.
    let p = [
        0x992d30ed00000001_u64,
        0x224698fc094cf91b,
        0,
        0x4000000000000000,
    ];
    let mut repr = [0; 32];
    for (to, word) in repr.chunks_exact_mut(8).zip(p) {
        to.copy_from_slice(&word.to_le_bytes());
    }
    let mut alias = input;
    alias.instances[0][n - 1] = Fq::from_repr(repr).unwrap();
    assert!(q_sigma_part(&alias, 12).is_err());
}

#[test]
fn exact_signed_original_digest_binds_body_and_both_raw_signature_halves() {
    for kind in object_kinds() {
        let bytes = (0..kind.body_len() + 64)
            .map(|i| i as u8)
            .collect::<Vec<_>>();
        let digest = object_digest(kind, &bytes).unwrap();
        for index in [
            0,
            kind.body_len() - 1,
            kind.body_len(),
            kind.body_len() + 16,
            kind.body_len() + 32,
            kind.body_len() + 48,
        ] {
            let mut bad = bytes.clone();
            bad[index] ^= 1;
            assert_ne!(object_digest(kind, &bad).unwrap(), digest);
        }
        assert!(object_digest(kind, &bytes[..bytes.len() - 1]).is_err());
    }
}

#[test]
fn homogeneous_frame_binds_high_foreign_challenge_limbs_and_fixed_absent_slots() {
    let ep = decode_point::<Ep>(&iroha_plonk_recursion::PALLAS_TRIVIAL_GENERATOR).unwrap();
    let eq = decode_point::<Eq>(&VESTA_TRIVIAL_GENERATOR).unwrap();
    let pallas = FoldInput::from_normalized(ep, 16, [Fq::ONE; K]).unwrap();
    let part = FoldInput::from_normalized(eq, 16, [Fp::ONE; K]).unwrap();
    let fields = std::array::from_fn(|i| Fp::from(i as u64 + 1));
    let public = frame(&fields, &pallas, &part).unwrap();
    assert_eq!(public.len(), 69);
    assert_eq!(&public[62..65], &[Fp::ZERO, Fp::ONE, Fp::ZERO]);
    let mut challenges = *pallas.challenges();
    challenges[15] = Fq::from(2).pow_vartime([200]);
    let changed = FoldInput::from_normalized(ep, 16, challenges).unwrap();
    let altered = frame(&fields, &changed, &part).unwrap();
    assert_ne!(altered[0], public[0]);
    assert_eq!(&altered[1..], &public[1..]);
}

#[test]
fn original_artifact_envelope_rejects_missing_oversized_and_noncanonical_metadata() {
    let config = ReadConfig {
        maximum_bytes: 16,
        maximum_rows: 1 << 16,
        coset_cache: iroha_plonk::keys::CosetCachePolicy::OnDemand,
        msm_budget: MemoryBudget::DEFAULT,
    };
    let oversized_descriptor = vec![0; DESCRIPTOR_MAX_BYTES + 1];
    let oversized_vk = vec![0; VERIFYING_KEY_MAX_BYTES + 1];
    let oversized_pk = vec![0; config.maximum_bytes + 1];
    for (descriptor, verifying_key, proving_key) in [
        (&[][..], &[1][..], &[1][..]),
        (&[1][..], &[][..], &[1][..]),
        (&[1][..], &[1][..], &[][..]),
        (oversized_descriptor.as_slice(), &[1][..], &[1][..]),
        (&[1][..], oversized_vk.as_slice(), &[1][..]),
        (&[1][..], &[1][..], oversized_pk.as_slice()),
        (&[1][..], &[1][..], &[1][..]),
    ] {
        assert_eq!(
            artifact_binding(
                OriginalArtifact {
                    descriptor,
                    verifying_key,
                    proving_key
                },
                CurveV1::Vesta,
                &[69],
                &[InstanceType::Bounded],
                config,
            ),
            Err(Error::Artifact)
        );
    }
}
