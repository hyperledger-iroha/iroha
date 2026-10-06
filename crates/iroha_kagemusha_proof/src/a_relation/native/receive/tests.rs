//! Pure production framing/canonical-boundary tests; no fake proof acceptance.

use super::*;
use iroha_plonk::transcript::decode_point;
use iroha_plonk_recursion::{PALLAS_TRIVIAL_GENERATOR, VESTA_TRIVIAL_GENERATOR};

#[test]
fn recorded_selector_uses_original_version_and_preserves_soft_pair_failures() {
    let segments = ObjectKind::Request.secondary_segments();
    assert!(segments.contains(&SegmentSpec::little(354, 8)));
    assert!(segments.contains(&SegmentSpec::little(362, 16)));
    assert!(segments.contains(&SegmentSpec::little(378, 16)));
    let mut request = vec![0; ObjectKind::Request.body_len() + 64];
    request[..2].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(recorded_selector(&request), Ok(0));
    request[354..362].copy_from_slice(&19_u64.to_le_bytes());
    // A malformed or missing recorded root is a soft false predicate. It cannot
    // select a cheaper sigma class or bypass its authenticated safe search route.
    assert_eq!(recorded_selector(&request), Ok(1));
    request[362..394].copy_from_slice(&Fp::ONE.to_repr());
    assert_eq!(recorded_selector(&request), Ok(1));
    for _current_mask in 0..8 {
        assert_eq!(recorded_selector(&request), Ok(1));
    }
    request[354..362].fill(0);
    assert_eq!(recorded_selector(&request), Ok(0));
    request[362..394].fill(0xff);
    assert_eq!(recorded_selector(&request), Ok(0));
    request[354..362].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(recorded_selector(&request), Ok(1));
    request[..2].copy_from_slice(&9_u16.to_le_bytes());
    assert_eq!(recorded_selector(&request), Ok(1));
    request.pop();
    assert_eq!(recorded_selector(&request), Err(Error::Input));
}

#[test]
fn payment_cap_rejects_oversized_originals_and_overflow() {
    assert_eq!(MAX_PAYMENT_ORIGINAL_BYTES, 8597);
    assert_eq!(check_payment_original_sizes(6176, 2421), Ok(()));
    assert_eq!(check_payment_original_sizes(6176, 2422), Err(Error::Input));
    assert_eq!(
        check_payment_original_sizes(11_360 + 320 + 1088, 1),
        Err(Error::Input)
    );
    assert_eq!(
        check_payment_original_sizes(usize::MAX, 1),
        Err(Error::Input)
    );
}

#[test]
fn receive_schedule_owns_all_tasks_and_q_slots_exactly_once() {
    let (partitions, tasks) = operation_schedule();
    assert_eq!(partitions.len(), A_STAGE_COUNT);
    assert_eq!(tasks.len(), A_STAGE_COUNT);
    assert_eq!(W_STAGE_COUNT, 10);
    assert_eq!(
        partitions.iter().flatten().copied().collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    for variant in [Variant::Receive, Variant::ReceiveRenewed] {
        OperationTask::validate(variant, &tasks).expect("complete fixed operation tasks");
        let plan =
            crate::a_relation::results::ReceiveResultPlan::from_tasks(variant, &tasks).unwrap();
        assert_eq!(plan.schema(), [(1, 9), (2, 1), (3, 6), (4, 2), (5, 3)]);
        let mut missing = tasks.clone();
        missing[6].clear();
        assert!(OperationTask::validate(variant, &missing).is_err());
        let mut duplicate = tasks.clone();
        duplicate[9].push(OperationTask::ReceiveProofs);
        duplicate[9].sort_unstable();
        assert!(OperationTask::validate(variant, &duplicate).is_err());
    }
}

#[test]
fn current_signature_owner_q_partition_and_terminal_effects_are_fixed() {
    let (partitions, tasks) = operation_schedule();
    assert_eq!(partitions[4], [0]);
    assert_eq!(partitions[5], [1]);
    assert_eq!(partitions[6], [2]);
    assert!(tasks[5].contains(&OperationTask::ReceiveAuthorization));
    assert!(tasks[6].contains(&OperationTask::ReceiveSignatures));
    assert_eq!(tasks[9], [OperationTask::ReceiveProofs]);
    assert_eq!(tasks[10], [OperationTask::ReceiveEffects]);
    let mut wrong = tasks.clone();
    wrong.swap(9, 10);
    assert!(
        crate::a_relation::results::ReceiveResultPlan::from_tasks(Variant::Receive, &wrong)
            .is_err()
    );
}

#[test]
fn foreign_challenge_codec_is_exact_and_rejects_integer_aliases() {
    for scalar in [Fq::ONE, -Fq::ONE, Fq::from(2).pow_vartime([128])] {
        let [lo, hi] = foreign_limbs(&scalar);
        assert_eq!(
            decode_foreign(Fp::from_u128(lo), Fp::from_u128(hi)),
            Ok(scalar)
        );
    }
    assert_eq!(
        decode_foreign(Fp::from(2).pow_vartime([128]), Fp::ZERO),
        Err(Error::Input)
    );
    assert_eq!(
        decode_foreign(Fp::ZERO, Fp::from(2).pow_vartime([128])),
        Err(Error::Input)
    );
    assert_eq!(
        decode_foreign(Fp::from_u128(u128::MAX), Fp::from_u128(u128::MAX)),
        Err(Error::Input)
    );
}

#[test]
fn context_links_include_source_k_and_da_has_exactly_fifty_two_words() {
    let p = AccumulatorT::<Ep>::new(
        decode_point::<Ep>(&PALLAS_TRIVIAL_GENERATOR).unwrap(),
        [Fq::ONE; K],
    )
    .unwrap();
    let v = AccumulatorT::<Eq>::new(
        decode_point::<Eq>(&VESTA_TRIVIAL_GENERATOR).unwrap(),
        [Fp::ONE; K],
    )
    .unwrap();
    let public = core::array::from_fn(|i| Fp::from(i as u64 + 1));
    let mut da = public.to_vec();
    push_pallas(&mut da, &p.as_input()).unwrap();
    assert_eq!(da.len(), 52);
    assert_eq!(
        terminal_digest(&public, &p.as_input()).unwrap(),
        hash_with_domain(crate::a_relation::LINEAGE_DOMAIN, &da)
    );
    let mut context = vec![
        Fp::ONE,
        Fp::from(4),
        Fp::from(3),
        Fp::from(97),
        Fp::from(16),
    ];
    push_pallas(&mut context, &p.as_input()).unwrap();
    context.extend(vesta_words(&v.as_input()).unwrap());
    context.push(Fp::from(16));
    push_pallas(&mut context, &p.as_input()).unwrap();
    assert_eq!(context.len(), 94);
    assert_eq!(
        continued_digest(Fp::from(97), Fp::from(4), 2, &p, &v, &p).unwrap(),
        hash_with_domain(u64::from_le_bytes(*b"kgwctx_1"), &context)
    );
    let mut bad = da[18..].to_vec();
    bad[2] = Fp::ZERO;
    bad[3] = Fp::ZERO;
    assert!(pallas_from_fields(&bad).is_err());
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
