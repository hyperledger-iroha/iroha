//! Pure exact-original and homogeneous-frame rejection tests, never an admitted Refresh.

use super::*;

fn original() -> Inputs {
    let state = StateWitness {
        core: [Fp::ZERO; 33],
        rest: [Fp::ZERO; 8],
        lineage: [Fp::ZERO; 18],
    };
    let state = RefreshWitness {
        predecessor: state,
        successor: state,
        statement: [Fp::ZERO; 26],
        update: crate::admin_sigma::RefreshUpdateWitness {
            kind: crate::admin_sigma::RefreshKind::Credential,
            digest: Fp::ZERO,
            scheme: [Fp::ZERO; 2],
            asset: [Fp::ZERO; 2],
            wallet: [Fp::ZERO; 2],
            counter: Fp::ZERO,
            issued_at_ms: Fp::ZERO,
            expires_at_ms: Fp::ZERO,
            root: Fp::ZERO,
            controls: Fp::ZERO,
            fee_schedule: Fp::ZERO,
        },
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
        objects: core::array::from_fn(|_| vec![]),
        insertion: None,
        quota: None,
        q: [
            QInput {
                proof: vec![],
                instances: vec![
                    bounded,
                    vec![],
                    vec![Fq::from(14)],
                    vec![Fq::ONE],
                    vec![Fq::from(12)],
                ],
            },
            QInput {
                proof: vec![],
                instances: vec![],
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
fn exact_refresh_sigma_tape_statement_length_and_selector_are_bound() {
    let source = original();
    assert_eq!(check_sigma_tape(&source), Ok(()));
    for mutation in 0..6 {
        let mut bad = source.clone();
        match mutation {
            0 => bad.sigma[40] ^= 1,
            1 => bad.sigma.push(7),
            2 => bad.q[0].instances[0][1] += Fq::ONE,
            3 => bad.q[0].instances[2][0] = Fq::ZERO,
            4 => bad.q[0].instances[2][0] = Fq::from(2),
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
fn refresh_q_sigma_normalization_rejects_wrong_selector_source_or_scalar_alias() {
    let mut input = original().q[0].clone();
    let point = iroha_plonk::transcript::decode_point::<Eq>(
        &iroha_plonk_recursion::VESTA_TRIVIAL_GENERATOR,
    )
    .unwrap();
    let (x, y) = Option::<(Fq, Fq)>::from(point.coordinates()).unwrap();
    input.instances[1] = vec![x, y];
    let n = input.instances[0].len();
    input.instances[0][n - K..n - K + 4].fill(Fq::ZERO);
    assert!(q_sigma_part(&input, 12).is_ok());
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
        assert!(q_sigma_part(&bad, 12).is_err(), "mutation{mutation}");
    }
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
fn all_fixed_refresh_originals_bind_body_and_every_signature_limb() {
    for variant in [
        Variant::RefreshCredential,
        Variant::RefreshSchemePolicy,
        Variant::RefreshBlacklist,
        Variant::RefreshQuotaShare,
        Variant::RefreshTimeAnchor,
    ] {
        let kinds = object_kinds(variant).unwrap();
        assert_eq!(
            kinds[..3],
            [
                ObjectKind::Credential,
                ObjectKind::Certificate,
                ObjectKind::Receipt
            ]
        );
        assert_eq!(kinds[4], ObjectKind::Certificate);
        for kind in kinds {
            let raw = (0..kind.body_len() + 64)
                .map(|i| i as u8)
                .collect::<Vec<_>>();
            let original = object_digest(kind, &raw).unwrap();
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
                assert_ne!(object_digest(kind, &changed).unwrap(), original);
            }
            assert!(object_digest(kind, &raw[..raw.len() - 1]).is_err());
        }
    }
    for variant in [
        Variant::Bootstrap,
        Variant::Load,
        Variant::Send,
        Variant::Receive,
        Variant::ArchiveStatus,
        Variant::Unload,
        Variant::Retiring,
    ] {
        assert!(object_kinds(variant).is_err());
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
fn fixed_union_projection_rejects_wrong_kind_each_field_and_original_signature() {
    use crate::admin_sigma::{RefreshKind, RefreshUpdateWitness};
    for (variant, kind, typed) in [
        (
            Variant::RefreshCredential,
            ObjectKind::Credential,
            RefreshKind::Credential,
        ),
        (
            Variant::RefreshSchemePolicy,
            ObjectKind::SchemePolicy,
            RefreshKind::SchemePolicy,
        ),
        (
            Variant::RefreshBlacklist,
            ObjectKind::Blacklist,
            RefreshKind::Blacklist,
        ),
        (
            Variant::RefreshQuotaShare,
            ObjectKind::QuotaShare,
            RefreshKind::QuotaShare,
        ),
        (
            Variant::RefreshTimeAnchor,
            ObjectKind::TimeAnchor,
            RefreshKind::TimeAnchor,
        ),
    ] {
        // Projection parsing only: this counting specimen is never authenticated,
        // passed to native Advance, proved, or treated as an admitted signed object.
        let mut raw = vec![0; kind.body_len() + 64];
        raw[0] = 1;
        let expected = RefreshUpdateWitness {
            kind: typed,
            digest: object_digest(kind, &raw).unwrap(),
            scheme: [Fp::ZERO; 2],
            asset: [Fp::ZERO; 2],
            wallet: [Fp::ZERO; 2],
            counter: Fp::ZERO,
            issued_at_ms: Fp::ZERO,
            expires_at_ms: Fp::ZERO,
            root: Fp::ZERO,
            controls: Fp::ZERO,
            fee_schedule: Fp::ZERO,
        };
        assert_eq!(check_update_projection(variant, &expected, &raw), Ok(()));
        for changed in 0..14 {
            let mut altered = expected;
            match changed {
                0 => {
                    altered.kind = if typed == RefreshKind::Credential {
                        RefreshKind::TimeAnchor
                    } else {
                        RefreshKind::Credential
                    }
                }
                1 => altered.digest += Fp::ONE,
                2..=3 => altered.scheme[changed - 2] += Fp::ONE,
                4..=5 => altered.asset[changed - 4] += Fp::ONE,
                6..=7 => altered.wallet[changed - 6] += Fp::ONE,
                8 => altered.counter += Fp::ONE,
                9 => altered.issued_at_ms += Fp::ONE,
                10 => altered.expires_at_ms += Fp::ONE,
                11 => altered.root += Fp::ONE,
                12 => altered.controls += Fp::ONE,
                _ => altered.fee_schedule += Fp::ONE,
            }
            assert_eq!(
                check_update_projection(variant, &altered, &raw),
                Err(Error::Input)
            );
        }
        raw[kind.body_len() + 48] ^= 1;
        assert_eq!(
            check_update_projection(variant, &expected, &raw),
            Err(Error::Input)
        );
        assert_eq!(
            check_update_projection(variant, &expected, &raw[..raw.len() - 1]),
            Err(Error::Input)
        );
    }
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

#[test]
fn typed_refresh_context_schema_rejects_foreign_operations_and_cross_kind_updates() {
    for variant in [
        Variant::RefreshCredential,
        Variant::RefreshSchemePolicy,
        Variant::RefreshBlacklist,
        Variant::RefreshQuotaShare,
        Variant::RefreshTimeAnchor,
    ] {
        let schema = RefreshObjects::context_specs(variant).unwrap();
        assert_eq!(schema.map(|spec| spec.tag), [1, 2, 3, 4, 5]);
        let lengths = object_kinds(variant)
            .unwrap()
            .map(|kind| (kind.body_len() + 64) as u32);
        assert_eq!(schema.map(|spec| spec.capacity), lengths);
    }
    let renewal = RefreshObjects::context_specs(Variant::RefreshCredential).unwrap();
    for variant in [
        Variant::RefreshSchemePolicy,
        Variant::RefreshBlacklist,
        Variant::RefreshQuotaShare,
        Variant::RefreshTimeAnchor,
    ] {
        assert_ne!(
            RefreshObjects::context_specs(variant).unwrap()[3],
            renewal[3]
        );
    }
    for variant in [
        Variant::Bootstrap,
        Variant::Load,
        Variant::Send,
        Variant::Receive,
        Variant::ArchiveReceive,
        Variant::ArchiveStatus,
        Variant::Unload,
        Variant::Retiring,
    ] {
        assert_eq!(
            RefreshObjects::context_specs(variant),
            Err(LayoutError::Synthesis)
        );
    }
}
