//! Production affine reconstruction, binding refusals and clearing ownership.

use super::*;
use crate::privacy_engines::zk_x509::{
    p256_aggregate_adapter::{P256MainRegistrationV1, P256MainVerifierFixedSourceV1},
    private_table::inspection,
};

fn owner_from_rows(rows: usize) -> MainArithmeticFixedCoefficientsV1 {
    MainArithmeticFixedCoefficientsV1 {
        basis: ZeroizingBaseColumnsV1(Vec::with_capacity(BASIS_WIDTH)),
        full: ZeroizingBaseColumnsV1(
            (0..WIDTH)
                .map(|column| {
                    (0..rows)
                        .map(|row| F((column * rows + row + 1) as u64))
                        .collect()
                })
                .collect(),
        ),
        basis_in_full: false,
    }
}

#[test]
fn sealed_basis_binding_rejects_mismatch_and_preserves_uncertainty_priority() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let bindings = canonical_p256_main_layout_bindings_v1(&layout).unwrap();
    for binding in bindings
        .iter()
        .filter(|binding| binding.main.segment.adapter == SegmentAdapterIdV1::P256Arithmetic)
    {
        validate_binding_v1(*binding, false).unwrap();
        for malformed in 0..4 {
            let mut bad = *binding;
            match malformed {
                0 => bad.main.segment.fixed_width -= 1,
                1 => bad.main.segment.trace_log2 -= 1,
                2 => bad.main.segment.adapter = SegmentAdapterIdV1::P256Window,
                3 => {
                    bad.p256 = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::WindowBatch, 0)
                        .unwrap()
                }
                _ => unreachable!(),
            }
            assert_eq!(
                validate_binding_v1(bad, false),
                Err(ZkX509StarkErrorV1::ProfileMismatch)
            );
            assert_eq!(
                validate_binding_v1(bad, true),
                Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain)
            );
        }
    }
    assert!(DESCRIPTOR_PAYLOAD < DESCRIPTOR_RESERVATION);
}

#[test]
fn production_basis_matches_independent_interpolation_and_four_cosets_on_mixed_public_rows() {
    const LOG: u8 = 3;
    const ROWS: usize = 1 << LOG;
    let selected = [0, 1, 15, 16, 31, 13 * 32, 14 * 32, NATIVE_ROWS - 1];
    let provider = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    for signature in [0, 4] {
        let registration =
            P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::Arithmetic, 0).unwrap();
        let mut owner = owner_from_rows(ROWS);
        for (row, native_row) in selected.into_iter().enumerate() {
            let expected = provider.fixed_row_v1(registration, native_row).unwrap();
            for (column, value) in expected.into_iter().enumerate() {
                owner.full.0[column][row] = value;
            }
        }
        let mut dense = ZeroizingBaseColumnsV1(owner.full.0.clone());
        let root = goldilocks_primitive_root_v1(LOG).unwrap();
        for column in &mut dense.0 {
            goldilocks_ifft_v1(column, root).unwrap();
        }
        for column in BASIS {
            owner
                .basis
                .0
                .push(core::mem::take(&mut owner.full.0[column]));
        }
        for column in &mut owner.basis.0 {
            goldilocks_ifft_v1(column, root).unwrap();
        }
        swap_basis_v1(&mut owner.full.0, &mut owner.basis.0, true, ROWS).unwrap();
        reconstruct(&mut owner.full.0, true, &recipes().unwrap()).unwrap();
        assert_eq!(owner.full.0, dense.0);
        swap_basis_v1(&mut owner.full.0, &mut owner.basis.0, false, ROWS).unwrap();
        let mut basis_coset =
            MainFixedCosetV1::new_v1(LOG, core::mem::take(&mut owner.basis.0)).unwrap();
        let mut dense_coset = MainFixedCosetV1::new_v1(LOG, core::mem::take(&mut dense.0)).unwrap();
        for ordinal in 0..4 {
            if ordinal != 0 {
                swap_basis_v1(&mut owner.full.0, &mut basis_coset.columns.0, false, ROWS).unwrap();
            }
            let full_root = goldilocks_primitive_root_v1(LOG + 2).unwrap();
            let stripe = main_quotient_stripes::MainQuotientStripeV1 {
                rows: ROWS,
                count: 4,
                ordinal,
                next_stride: 1,
                root,
                shift: F(GOLDILOCKS_GENERATOR_V1).mul(full_root.pow(ordinal as u128)),
            };
            basis_coset.evaluate_v1(stripe).unwrap();
            swap_basis_v1(&mut owner.full.0, &mut basis_coset.columns.0, true, ROWS).unwrap();
            reconstruct(&mut owner.full.0, false, &recipes().unwrap()).unwrap();
            assert_eq!(&owner.full.0, dense_coset.evaluate_v1(stripe).unwrap());
        }
    }
}

#[test]
fn arithmetic_basis_every_partial_transfer_unwind_erases_all_allocations_once() {
    const ROWS: usize = 8;
    for moved in 0..=BASIS_WIDTH {
        let (result, erased) = inspection::observe_v1(|| {
            std::panic::catch_unwind(|| {
                let mut owner = owner_from_rows(ROWS);
                let original = owner
                    .full
                    .0
                    .iter()
                    .map(|column| column.as_ptr() as usize)
                    .collect::<std::collections::BTreeSet<_>>();
                for column in BASIS.into_iter().take(moved) {
                    owner
                        .basis
                        .0
                        .push(core::mem::take(&mut owner.full.0[column]));
                }
                let retained = owner
                    .full
                    .0
                    .iter()
                    .chain(&owner.basis.0)
                    .filter(|column| !column.is_empty())
                    .map(|column| column.as_ptr() as usize)
                    .collect::<std::collections::BTreeSet<_>>();
                assert_eq!(original, retained);
                panic!("injected basis move unwind at {moved}");
            })
        });
        assert!(result.is_err());
        assert_eq!(
            erased.iter().map(|entry| entry.cells).sum::<usize>(),
            WIDTH * ROWS
        );
        assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
    }
}

#[test]
fn basis_swap_and_reconstruction_refuse_bad_shapes_before_first_write() {
    let mut owner = owner_from_rows(8);
    for column in BASIS {
        owner
            .basis
            .0
            .push(core::mem::take(&mut owner.full.0[column]));
    }
    owner.basis.0[7].pop();
    let original_full = owner.full.0.clone();
    let original_basis = owner.basis.0.clone();
    assert!(swap_basis_v1(&mut owner.full.0, &mut owner.basis.0, true, 8).is_err());
    assert_eq!(owner.full.0, original_full);
    assert_eq!(owner.basis.0, original_basis);
    assert!(reconstruct(&mut owner.full.0, false, &recipes().unwrap()).is_err());
    assert_eq!(owner.full.0, original_full);
}

#[test]
#[ignore = "production basis owner native19 four-coset parity and erasure, both verifier roles"]
fn production_arithmetic_basis_owner_preserves_every_native19_coset_field() {
    let provider = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let policy = MainBoundedTransformPolicyV1::cpu_v1();
    for signature in [0, 4] {
        let registration =
            P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::Arithmetic, 0).unwrap();
        let mut owner = owner_from_rows(NATIVE_ROWS);
        provider
            .fill_arithmetic_fixed_matrix_v1(registration, &mut owner.full.0)
            .unwrap();
        let mut dense = ZeroizingBaseColumnsV1(owner.full.0.clone());
        for column in BASIS {
            owner
                .basis
                .0
                .push(core::mem::take(&mut owner.full.0[column]));
        }
        let root = goldilocks_primitive_root_v1(NATIVE_LOG).unwrap();
        for columns in [&mut owner.basis.0, &mut dense.0] {
            columns
                .par_iter_mut()
                .for_each(|column| goldilocks_ifft_coarse_v1(column, root).unwrap());
        }
        let cache = owner.cache_columns_v1().unwrap()[..26]
            .iter()
            .map(|column| PrivateTableV1::new(column.clone(), zeroize_fields_v1))
            .collect::<Vec<_>>();
        let mut basis = owner.into_coset_v1(policy, &cache).unwrap();
        let mut dense =
            MainFixedCosetV1::new_v1(NATIVE_LOG, core::mem::take(&mut dense.0)).unwrap();
        for ordinal in 0..4 {
            let stripe =
                main_quotient_stripes::MainQuotientStripeV1::new_v1(NATIVE_LOG, 21, ordinal)
                    .unwrap();
            let actual = basis.evaluate_v1(stripe).unwrap();
            assert_eq!(actual, dense.evaluate_v1(stripe).unwrap());
        }
        let ((), erased) = inspection::observe_v1(|| drop(basis));
        assert_eq!(
            erased.iter().map(|entry| entry.cells).sum::<usize>(),
            WIDTH * NATIVE_ROWS
        );
        assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
    }
}
