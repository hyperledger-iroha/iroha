//! Exact Archive ledger/source shape rejection. These checks grant no admission.
use super::*;

fn q(receive: bool) -> QInput {
    let g = iroha_plonk::transcript::decode_point::<Eq>(
        &iroha_plonk_recursion::VESTA_TRIVIAL_GENERATOR,
    )
    .unwrap();
    let (x, y) = Option::<(Fq, Fq)>::from(g.coordinates()).unwrap();
    let mut bounded = vec![Fq::ZERO; 3];
    let mut u = [Fq::ONE; K];
    if !receive {
        u[..4].fill(Fq::ZERO);
    }
    bounded.extend(u);
    QInput {
        proof: vec![],
        instances: vec![
            bounded,
            vec![x, y],
            if receive {
                vec![Fq::from(12), Fq::from(10)]
            } else {
                vec![Fq::from(12)]
            },
            if receive {
                vec![Fq::ONE, Fq::ZERO, Fq::ZERO, Fq::ONE, Fq::ZERO]
            } else {
                vec![Fq::ONE]
            },
            vec![Fq::from(if receive { 16 } else { 12 })],
        ],
    }
}

#[test]
fn archive_receive_and_status_preserve_distinct_qsigma_source_obligations() {
    for receive in [false, true] {
        let original = q(receive);
        let source = if receive { 16 } else { 12 };
        assert!(q_sigma_part(&original, source).is_ok());
        for mutation in 0..6 {
            let mut altered = original.clone();
            match mutation {
                0 => altered.instances[2][0] = Fq::from(13),
                1 => altered.instances[3][0] = Fq::ZERO,
                2 => altered.instances[4][0] = Fq::from(14),
                3 => altered.instances[1] = vec![Fq::ZERO, Fq::ZERO],
                4 => {
                    altered.instances[2].push(Fq::from(10));
                }
                _ => {
                    let n = altered.instances[0].len();
                    altered.instances[0][n - 1] = Fq::ZERO;
                }
            }
            assert!(
                q_sigma_part(&altered, source).is_err(),
                "receive={receive} mutation={mutation}"
            );
        }
        assert!(q_sigma_part(&original, if receive { 12 } else { 16 }).is_err());
    }
    let mut wrong = q(true);
    wrong.instances[2][1] = Fq::from(2);
    assert!(q_sigma_part(&wrong, 16).is_err());
    wrong.instances[2][1] = Fq::from(11);
    assert!(q_sigma_part(&wrong, 16).is_ok());
}

#[test]
fn exact_archive_context_retains_all_originals_without_cross_form_substitution() {
    let receive = context_specs(Variant::ArchiveReceive, 4096, 3456).unwrap();
    let status = context_specs(Variant::ArchiveStatus, 4096, 3456).unwrap();
    assert_eq!(receive.len(), 13);
    assert_eq!(status.len(), 15);
    assert_eq!(receive[..12], status[..12]);
    assert_eq!(receive[6].capacity, 163);
    assert_eq!(receive[9].capacity, 99);
    assert_eq!(receive[10].capacity, 4096);
    assert_eq!(receive[11].capacity, 3456);
    assert_eq!(receive[12].capacity, 3456);
    assert_eq!(status[12].capacity, 4096);
    assert_eq!(status[13].capacity, 162);
    assert_eq!(status[14].capacity, 1125);
    for specs in [receive, status] {
        assert_eq!(
            specs.iter().map(|s| s.tag).collect::<Vec<_>>(),
            (1..=specs.len() as u32).collect::<Vec<_>>()
        );
    }
    for variant in [
        Variant::Bootstrap,
        Variant::Load,
        Variant::Send,
        Variant::Receive,
        Variant::Unload,
        Variant::Retiring,
        Variant::RefreshBlacklist,
    ] {
        assert_eq!(context_specs(variant, 4096, 3456), Err(Error::Artifact));
    }
}

#[test]
fn archive_task_and_q_order_owns_each_authentication_and_both_map_removals() {
    let (parts, tasks) = operation_schedule();
    assert_eq!(A_STAGE_COUNT, 4);
    assert_eq!(W_STAGE_COUNT, 3);
    assert_eq!(parts, vec![vec![], vec![0], vec![1], vec![2]]);
    assert_eq!(
        tasks,
        vec![
            vec![OperationTask::ArchiveOwnProof],
            vec![],
            vec![OperationTask::ArchiveAuthorization],
            vec![OperationTask::ArchiveEvidence, OperationTask::ArchiveMaps]
        ]
    );
    let actual = tasks.into_iter().flatten().collect::<Vec<_>>();
    assert_eq!(
        OperationTask::required(Variant::ArchiveReceive),
        Some(actual.as_slice())
    );
    assert_eq!(
        OperationTask::required(Variant::ArchiveStatus),
        Some(actual.as_slice())
    );
    assert_ne!(
        OperationTask::required(Variant::Receive),
        Some(actual.as_slice())
    );
}

#[test]
fn incoming_full_claim_codec_rejects_high_limbs_and_dropped_opening() {
    assert_eq!(
        decode_foreign(Fp::from(2).pow_vartime([128]), Fp::ZERO),
        Err(Error::Input)
    );
    assert_eq!(
        decode_foreign(Fp::ZERO, Fp::from(2).pow_vartime([128])),
        Err(Error::Input)
    );
    assert!(pallas_from_fields(&[Fp::ONE; 33]).is_err());
    assert!(incoming_from_fields(&[Fp::ONE; 105]).is_err());
    assert!(incoming_from_fields(&[Fp::ONE; 107]).is_err());
    for n in [0, 17, 85, 86, 106] {
        assert!(incoming_from_fields(&vec![Fp::ZERO; n]).is_err());
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
fn terminal_binding_rejects_substitution_of_the_installed_omega_root_and_claim() {
    // Binding specimen only; no proof, installed package or monetary head is admitted.
    let p = iroha_plonk::transcript::decode_point::<Ep>(
        &iroha_plonk_recursion::PALLAS_TRIVIAL_GENERATOR,
    )
    .unwrap();
    let v = iroha_plonk::transcript::decode_point::<Eq>(
        &iroha_plonk_recursion::VESTA_TRIVIAL_GENERATOR,
    )
    .unwrap();
    let claim = FoldInput::from_normalized(p, 16, [Fq::ONE; K]).unwrap();
    let public = core::array::from_fn(|i| Fp::from(i as u64 + 1));
    let digest = terminal_digest(&public, &claim).unwrap();
    let vesta = AccumulatorT::new(v, [Fp::ONE; K]).unwrap();
    let instances = omega_instances(digest, &vesta).unwrap();
    for column in [5, 8, 17] {
        let mut substituted = public;
        substituted[column] += Fp::ONE;
        let changed = terminal_digest(&substituted, &claim).unwrap();
        assert_ne!(changed, digest, "head/credential/Omega root column{column}");
        assert_ne!(omega_instances(changed, &vesta).unwrap(), instances);
    }
    let mut challenges = [Fq::ONE; K];
    challenges[15] = Fq::from(2).pow_vartime([200]);
    assert_ne!(
        terminal_digest(
            &public,
            &FoldInput::from_normalized(p, 16, challenges).unwrap()
        )
        .unwrap(),
        digest
    );
    let altered_vesta = AccumulatorT::new(v, [Fp::from(2); K]).unwrap();
    let changed = omega_instances(digest, &altered_vesta).unwrap();
    assert_eq!(changed[0], instances[0]);
    assert_eq!(changed[1], instances[1]);
    assert_ne!(changed[2], instances[2]);
}
