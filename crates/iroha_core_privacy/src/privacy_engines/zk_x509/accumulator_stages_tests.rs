//! Native owner parity, staged commitment provenance and explicit storage erasure.

use super::super::super::private_table::inspection;
use super::super::tests::{credential_main_pre_aux_v1, fixture};
use super::*;
use rand::{SeedableRng, rngs::StdRng};

#[test]
fn native_staged_columns_match_every_actual_row_and_clear_retained_storage() {
    let (trace, schedule, _, _) = fixture();
    let public = ca_accumulator_stark_public_v1(&trace, &schedule).unwrap();
    let binding = derive_zk_x509_credential_pre_aux_binding_v1(
        credential_main_pre_aux_v1(),
        ca_profile_digest_v1().unwrap(),
        ca_public_digest_v1(TEST_PROOF_INSTANCE_V1, public, &schedule).unwrap(),
        PrivacyOuterDigestV1::from_bytes([0x81; 48]),
    )
    .unwrap();
    let ((), cleared) = inspection::observe_v1(|| {
        let base = native_base_columns_v1(&trace).unwrap();
        let auxiliary = native_aux_columns_v1(&trace, public, binding).unwrap();
        let reference = build_ca_accumulator_stark_material_v1(
            &trace,
            &schedule,
            binding.sha(),
            binding.rfc5280(),
        )
        .unwrap();
        assert_eq!(base.len(), 695);
        assert_eq!(auxiliary.len(), 128);
        assert_eq!(&*base, &reference.base_columns);
        assert_eq!(&*auxiliary, &reference.aux_columns);
        assert!(
            base.iter()
                .chain(auxiliary.iter())
                .all(|column| column.len() == 4096)
        );
        for row in 0..4096 {
            let expected = trace.base_row(row).unwrap();
            for (column, value) in expected.iter().enumerate() {
                assert_eq!(base[column][row], *value);
            }
        }
        drop(auxiliary);
        drop(base);
    });
    assert!(cleared.iter().all(|entry| entry.nonzero_after == 0));
    assert!(cleared.iter().filter(|entry| entry.cells == 4096).count() >= 823);
    assert!(cleared.iter().any(|entry| entry.cells == 695));
    assert!(cleared.iter().any(|entry| entry.cells == 128));
}

#[test]
fn staged_original_commitments_replay_exactly_and_bind_the_shared_challenge() {
    let (trace, schedule, _, _) = fixture();
    let state = commit_ca_through_auxiliary_v1(
        &trace,
        &schedule,
        credential_main_pre_aux_v1(),
        &mut StdRng::seed_from_u64(191),
    )
    .unwrap();
    state.validate_v1().unwrap();
    assert_eq!(state.trace_roots.len(), 1);
    assert_eq!(
        state.trace_roots[0].base_root,
        commit_original_columns_v1(TEST_PROOF_INSTANCE_V1, &state.base).unwrap()
    );
    assert_eq!(
        state.auxiliary_root_v1(),
        commit_original_columns_v1(TEST_PROOF_INSTANCE_V1, &state.auxiliary).unwrap()
    );
    let expected = derive_zk_x509_credential_pre_aux_binding_v1(
        credential_main_pre_aux_v1(),
        ca_profile_digest_v1().unwrap(),
        ca_public_digest_v1(TEST_PROOF_INSTANCE_V1, state.public, &schedule).unwrap(),
        state.trace_roots[0].base_root,
    )
    .unwrap();
    assert_eq!(state.binding_v1(), expected);
    assert!(format!("{state:?}").contains("<private coefficients redacted>"));
    assert_eq!(
        state.allocated_payload_bytes_v1().unwrap(),
        823 * (core::mem::size_of::<Vec<F>>() + 6196 * 8)
            + core::mem::size_of::<CaAwaitingMainAuxiliaryV1>()
    );
    assert_eq!(
        state.allocated_payload_bytes_v1().unwrap(),
        CaAwaitingMainAuxiliaryV1::payload_bound_v1().unwrap()
    );
    for column in 0..128 {
        let selected = (96..100).contains(&column) || (116..128).contains(&column);
        assert_eq!(state.link_column_v1(column).is_ok(), selected);
        if selected {
            assert_eq!(state.link_column_v1(column).unwrap().len(), 6196);
        }
    }
    assert!(state.link_column_v1(128).is_err());
    let plan = CaMainPrivateLinkPlanV1::new_v1(&schedule, state.binding.sha()).unwrap();
    let point = E::canonical([31, 5, 17, 2]).unwrap();
    let values = state.open_link_values_v1(&plan, point).unwrap();
    let mixes: [E; 108] = core::array::from_fn(|index| E::from_base(F(index as u64 + 19)));
    let coefficients = state
        .link_deep_coefficients_v1(&plan, point, &values, &mixes)
        .unwrap();
    assert_eq!(coefficients.len(), 6196);
    assert_eq!(coefficients.capacity(), 6196);
    assert_eq!(coefficients.last(), Some(&E::ZERO));
    for x in [E::from_base(F(7)), E::canonical([13, 29, 37, 43]).unwrap()] {
        let expected =
            plan.ca_openings_v1()
                .iter()
                .enumerate()
                .fold(E::ZERO, |sum, (index, opening)| {
                    let at_x = state
                        .auxiliary
                        .open_v1(usize::from(opening.column), x)
                        .unwrap();
                    sum.add(
                        at_x.sub(values[index])
                            .mul(x.sub(point.mul_base(opening.multiplier)).inv().unwrap())
                            .mul(mixes[index]),
                    )
                });
        let actual = coefficients
            .iter()
            .rev()
            .fold(E::ZERO, |value, coefficient| value.mul(x).add(*coefficient));
        assert_eq!(actual, expected);
    }
    for index in [0, 53, 107] {
        let mut bad_values = values;
        bad_values[index] = bad_values[index].add(E::ONE);
        assert!(
            state
                .link_deep_coefficients_v1(&plan, point, &bad_values, &mixes)
                .is_err()
        );
    }
    assert!(
        state
            .link_deep_coefficients_v1(&plan, point, &values[..107], &mixes)
            .is_err()
    );
    assert!(
        state
            .link_deep_coefficients_v1(&plan, point, &values, &mixes[..107])
            .is_err()
    );
    assert!(state.open_link_values_v1(&plan, E::ONE).is_err());
    drop(coefficients);
    let mut state = state;
    let root = state.trace_roots[0].aux_root;
    state.trace_roots[0].aux_root = PrivacyOuterDigestV1::default();
    assert_eq!(
        state.validate_v1(),
        Err(ZkX509CaAccumulatorProofErrorV1::TranscriptMismatch)
    );
    state.trace_roots[0].aux_root = root;
    let transcript_state = state.auxiliary_transcript_state;
    state.auxiliary_transcript_state = PrivacyOuterDigestV1::default();
    assert_eq!(
        state.validate_v1(),
        Err(ZkX509CaAccumulatorProofErrorV1::TranscriptMismatch)
    );
    state.auxiliary_transcript_state = transcript_state;
    state.validate_v1().unwrap();
    let ((), cleared) = inspection::observe_v1(|| drop(state));
    assert_eq!(
        cleared.iter().filter(|entry| entry.cells == 6196).count(),
        823
    );
    assert!(cleared.iter().all(|entry| entry.nonzero_after == 0));
}

#[test]
fn staged_native_scratch_clears_each_family_on_return_and_unwind() {
    for unwind in [false, true] {
        let (result, cleared) = inspection::observe_v1(|| {
            std::panic::catch_unwind(|| {
                let mut scratch = CaAuxScratchV1::zero_v1();
                scratch.base.fill(F(3));
                scratch.current.fill(F(5));
                scratch.previous.fill(F(7));
                if unwind {
                    panic!("deliberate staged native scratch unwind");
                }
                drop(scratch);
            })
        });
        assert_eq!(result.is_err(), unwind);
        assert_eq!(
            cleared.iter().map(|entry| entry.cells).collect::<Vec<_>>(),
            [695, 128, 128]
        );
        assert!(
            cleared
                .iter()
                .all(|entry| entry.nonzero_before == entry.cells && entry.nonzero_after == 0)
        );
    }
}

#[test]
fn local_ca_denominator_table_matches_every_actual_coset_row_and_rejects_foreign_root() {
    let root = goldilocks_primitive_root_v1(ZK_X509_CA_FRI_LDE_LOG2_V1).unwrap();
    let inverses = ca_local_vanishing_inverses_v1(root).unwrap();
    let mut x = F(GOLDILOCKS_GENERATOR_V1);
    for row in 0..65_536 {
        assert_eq!(inverses[row % 16].mul(x.pow(4096).sub(F::ONE)), F::ONE);
        x = x.mul(root);
    }
    assert!(ca_local_vanishing_inverses_v1(F::ONE).is_err());
    assert!(ca_local_vanishing_inverses_v1(root.mul(root)).is_err());
}

#[test]
fn original_ca_local_composition_matches_complete_relation_with_only_public_alphas_zeroed() {
    let (trace, schedule, _, _) = fixture();
    let state = commit_ca_through_auxiliary_v1(
        &trace,
        &schedule,
        credential_main_pre_aux_v1(),
        &mut StdRng::seed_from_u64(913),
    )
    .unwrap();
    let layout = ca_aggregate_layout_v1().unwrap();
    let alphas: Vec<E> = (0..1363)
        .map(|index| {
            E::canonical([
                index as u64 + 7,
                index as u64 * 3 + 11,
                index as u64 * 5 + 13,
                index as u64 * 7 + 17,
            ])
            .unwrap()
        })
        .collect();
    let expected = {
        let base = state.base.local_lde_v1().unwrap();
        let aux = state.auxiliary.local_lde_v1().unwrap();
        let fixed =
            ca_fixed_lde_columns_v1(&compile_ca_accumulator_fixed_columns_v1().unwrap()).unwrap();
        let material = build_ca_accumulator_stark_material_v1(
            &trace,
            &schedule,
            state.binding.sha(),
            state.binding.rfc5280(),
        )
        .unwrap();
        let claims = ca_accumulator_stark_terminal_claims_v1(&material);
        let mut complete_alphas = alphas.clone();
        complete_alphas.resize(1379, E::ZERO);
        ca_composition_lanes_v1(
            state.public,
            &base,
            &aux,
            &fixed,
            state.binding.sha(),
            state.binding.rfc5280(),
            claims,
            &complete_alphas,
            &layout,
            &mut StdRng::seed_from_u64(917),
        )
        .unwrap()
    };
    let actual = state
        .local_composition_v1(&alphas, &mut StdRng::seed_from_u64(917))
        .unwrap();
    assert_eq!(
        &*actual, &*expected,
        "every codeword and adjacent mask is identical for the same local relation and entropy"
    );
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].len(), 4);
    assert!(actual[0].iter().all(|column| column.len() == 65_536));
    for bad_len in [0, 1362, 1364, 1379] {
        let mut bad = alphas.clone();
        bad.resize(bad_len, E::ONE);
        assert!(
            state
                .local_composition_v1(&bad, &mut StdRng::seed_from_u64(917))
                .is_err()
        );
    }
    let mut bad = alphas.clone();
    bad[0] = E::from_raw_coefficients_for_testing([
        F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1),
        F::ZERO,
        F::ZERO,
        F::ZERO,
    ]);
    assert!(
        state
            .local_composition_v1(&bad, &mut StdRng::seed_from_u64(917))
            .is_err()
    );
    let ((), cleared) = inspection::observe_v1(|| drop(actual));
    assert_eq!(
        cleared.iter().filter(|entry| entry.cells == 65_536).count(),
        4
    );
    assert!(cleared.iter().all(|entry| entry.nonzero_after == 0));
}

#[test]
fn joint_ca_working_gate_counts_actual_capacity_and_rejects_overflow() {
    assert_eq!(
        check_ca_working_payload_v1(&[CA_JOINT_WORKING_CAP_V1]).unwrap(),
        CA_JOINT_WORKING_CAP_V1
    );
    assert!(check_ca_working_payload_v1(&[CA_JOINT_WORKING_CAP_V1, 1]).is_err());
    assert!(check_ca_working_payload_v1(&[usize::MAX, 1]).is_err());
    let columns = vec![Vec::<F>::with_capacity(17), vec![F::ONE; 3]];
    let exact = columns.capacity() * core::mem::size_of::<Vec<F>>()
        + columns
            .iter()
            .map(|column| column.capacity() * core::mem::size_of::<F>())
            .sum::<usize>();
    assert_eq!(
        ca_matrix_payload_v1(&columns, columns.capacity()).unwrap(),
        exact
    );
    assert!(ca_matrix_payload_v1(&columns, 1).is_err());
    assert!(ca_matrix_payload_v1::<F>(&[], usize::MAX).is_err());
}
