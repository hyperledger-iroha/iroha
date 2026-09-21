//! Independent low-digit, source-order, canonical chunk and erasure controls.
use super::*;
use crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1;

fn empty_group_values_v1() -> ZeroizingT256ScalarVecV1 {
    ZeroizingT256ScalarVecV1::try_with_exact_capacity(16_384).unwrap()
}

fn source_chunk_v1(block: usize) -> ZkAmsPhase23RnsLinkSecretChunkV1 {
    let mut chunk = ZkAmsPhase23RnsLinkSecretChunkV1::new_main_block_zeroed_v1().unwrap();
    for (coefficient, bytes) in chunk.as_mut_bytes_v1().chunks_exact_mut(32).enumerate() {
        let value = (block * 256 + coefficient) as u64;
        bytes[24..].copy_from_slice(&value.to_be_bytes());
    }
    chunk
}

fn complete_group_v1() -> LowDigitGroupV1 {
    let mut values = empty_group_values_v1();
    for block in 0..64 {
        values = append_canonical_group_block_v1(values, source_chunk_v1(block), block).unwrap();
    }
    LowDigitGroupV1 { group: 0, values }
}

fn decoded_low_v1(scalar: &VegaT256ScalarV1) -> u16 {
    let mut bytes = [0; 32];
    scalar.write_le_bytes_ref(&mut bytes);
    assert!(bytes[2..].iter().all(|byte| *byte == 0));
    u16::from_le_bytes(bytes[..2].try_into().unwrap())
}

#[test]
fn all_11696_coordinates_are_exact_existing_group_major_candidate_order() {
    let mut ordinal = 0;
    let mut seen = std::collections::BTreeSet::new();
    for group in 0..344 {
        for slack in [false, true] {
            for digit in 0..17 {
                let coordinate = low_digit_coordinate_v1(ordinal).unwrap();
                assert_eq!(
                    (coordinate.group, coordinate.slack, coordinate.digit),
                    (group, slack, digit)
                );
                assert!(seen.insert((group, slack, digit)));
                ordinal += 1;
            }
        }
    }
    assert_eq!(ordinal, 11_696);
    for invalid in [11_696, 11_697, u16::MAX] {
        assert!(low_digit_coordinate_v1(invalid).is_err());
    }
}

#[test]
fn independent_integer_boundary_vectors_cover_all_17_difference_and_slack_digits() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    // Generated with independent integer division/modulo, not the Rust bit owner.
    let vectors: [(&str, [u16; 17], [u16; 17]); 7] = [
        (
            "0000000000000000000000000000000000000000000000000000000000000000",
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [
                32766, 32767, 32767, 32767, 32767, 32767, 63, 0, 0, 0, 0, 0, 4096, 0, 16384, 32767,
                32767,
            ],
        ),
        (
            "0000000000000000000000000000000000000000000000000000000000000001",
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [
                32765, 32767, 32767, 32767, 32767, 32767, 63, 0, 0, 0, 0, 0, 4096, 0, 16384, 32767,
                32767,
            ],
        ),
        (
            "0000000000000000000000000000000000000000000000000000000000007fff",
            [32767, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [
                32767, 32766, 32767, 32767, 32767, 32767, 63, 0, 0, 0, 0, 0, 4096, 0, 16384, 32767,
                32767,
            ],
        ),
        (
            "0000000000000000000000000000000000000000000000000000000000008000",
            [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [
                32766, 32766, 32767, 32767, 32767, 32767, 63, 0, 0, 0, 0, 0, 4096, 0, 16384, 32767,
                32767,
            ],
        ),
        (
            "7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            [
                32767, 32767, 32767, 32767, 32767, 32767, 32767, 32767, 32767, 32767, 32767, 32767,
                32767, 32767, 32767, 32767, 32767,
            ],
            [
                32767, 32767, 32767, 32767, 32767, 32767, 63, 0, 0, 0, 0, 0, 4096, 0, 16384, 32767,
                32767,
            ],
        ),
        (
            "8000000000000000000000000000000000000000000000000000000000000000",
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [
                32766, 32767, 32767, 32767, 32767, 32767, 63, 0, 0, 0, 0, 0, 4096, 0, 16384, 32767,
                32767,
            ],
        ),
        (
            "ffffffff00000001000000000000000000000000fffffffffffffffffffffffe",
            [
                32766, 32767, 32767, 32767, 32767, 32767, 63, 0, 0, 0, 0, 0, 4096, 0, 16384, 32767,
                32767,
            ],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
    ];
    for (hex, difference, slack) in vectors {
        let mut encoded = [0_u8; 32];
        for (index, byte) in encoded.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
        }
        let scalar = VegaT256ScalarV1::from_be_bytes_exact_ref(&encoded).unwrap();
        for digit in 0..17 {
            assert_eq!(
                decoded_low_v1(&low_digit_scalar_v1(&scalar, false, digit).unwrap()),
                difference[usize::from(digit)]
            );
            assert_eq!(
                decoded_low_v1(&low_digit_scalar_v1(&scalar, true, digit).unwrap()),
                slack[usize::from(digit)]
            );
        }
        for digit in [17, 18, u8::MAX] {
            assert!(low_digit_scalar_v1(&scalar, false, digit).is_err());
        }
    }
}

#[test]
fn canonical_group_append_rejects_wrong_order_shape_and_late_noncanonical_scalar() {
    for (block, nonce) in [(1, false), (64, false), (0, true)] {
        let values = empty_group_values_v1();
        let chunk = if nonce {
            ZkAmsPhase23RnsLinkSecretChunkV1::new_nonce_zeroed_v1().unwrap()
        } else {
            source_chunk_v1(0)
        };
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        assert!(append_canonical_group_block_v1(values, chunk, block).is_err());
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 1);
    }
    let mut chunk = source_chunk_v1(0);
    chunk.as_mut_bytes_v1()[255 * 32..].copy_from_slice(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    assert!(append_canonical_group_block_v1(empty_group_values_v1(), chunk, 0).is_err());
    assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 1);
    let values =
        append_canonical_group_block_v1(empty_group_values_v1(), source_chunk_v1(0), 0).unwrap();
    assert_eq!(values.len(), 256);
    assert!(append_canonical_group_block_v1(values, source_chunk_v1(0), 0).is_err());
}

#[test]
fn source_order_transposes_once_and_emits_all_canonical_coordinates() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let group = complete_group_v1();
    let mut values = group
        .prepare_values_v1(low_digit_coordinate_v1(0).unwrap())
        .unwrap();
    for chunk in 0..32 {
        let emitted = values.emit_next_v1(chunk).unwrap();
        assert_eq!(emitted.len_v1(), 16_384);
        for (local, encoded) in emitted.as_slice_v1().chunks_exact(32).enumerate() {
            assert!(encoded[..30].iter().all(|byte| *byte == 0));
            let v = usize::from(chunk) * 512 + local;
            let expected = ((v % 64) * 256 + v / 64) as u16;
            assert_eq!(
                u16::from_be_bytes(encoded[30..].try_into().unwrap()),
                expected
            );
        }
    }
    values.finish_v1().unwrap();
}

#[test]
fn zero_source_coefficient_retains_nonzero_slack_digits_in_last_coordinate() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let mut group = complete_group_v1();
    group.values.as_mut_slice()[16_383] = VegaT256ScalarV1::from_u64(0);
    for slack in [false, true] {
        let mut values = group
            .prepare_values_v1(LowDigitCoordinateV1 {
                group: 0,
                slack,
                digit: 0,
            })
            .unwrap();
        for chunk in 0..32 {
            let emitted = values.emit_next_v1(chunk).unwrap();
            if chunk == 31 {
                let expected = if slack { 32766_u16 } else { 0_u16 };
                assert_eq!(&emitted.as_slice_v1()[16_382..], &expected.to_be_bytes());
                assert!(
                    emitted.as_slice_v1()[16_352..16_382]
                        .iter()
                        .all(|byte| *byte == 0)
                );
            }
        }
        values.finish_v1().unwrap();
    }
}

#[test]
fn malformed_group_axes_or_length_fail_before_plane_allocation() {
    let group = complete_group_v1();
    for coordinate in [
        LowDigitCoordinateV1 {
            group: 1,
            slack: false,
            digit: 0,
        },
        LowDigitCoordinateV1 {
            group: 0,
            slack: false,
            digit: 17,
        },
        LowDigitCoordinateV1 {
            group: 344,
            slack: true,
            digit: 16,
        },
    ] {
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        assert!(group.prepare_values_v1(coordinate).is_err());
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before);
    }
    let short = LowDigitGroupV1 {
        group: 0,
        values: empty_group_values_v1(),
    };
    assert!(
        short
            .prepare_values_v1(low_digit_coordinate_v1(0).unwrap())
            .is_err()
    );
}

#[test]
fn shared_emission_rejects_skip_duplicate_trailing_and_early_finish_and_erases_values() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let group = complete_group_v1();
    for wrong in [1, 32, u8::MAX] {
        let mut values = group
            .prepare_values_v1(low_digit_coordinate_v1(0).unwrap())
            .unwrap();
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        assert!(values.emit_next_v1(wrong).is_err());
        assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 1);
        assert!(values.emit_next_v1(0).is_err());
        assert!(values.finish_v1().is_err());
    }
    let mut values = group
        .prepare_values_v1(low_digit_coordinate_v1(0).unwrap())
        .unwrap();
    values.emit_next_v1(0).unwrap();
    assert!(values.emit_next_v1(0).is_err());
    let values = group
        .prepare_values_v1(low_digit_coordinate_v1(0).unwrap())
        .unwrap();
    assert!(values.finish_v1().is_err());
    let mut values = group
        .prepare_values_v1(low_digit_coordinate_v1(0).unwrap())
        .unwrap();
    for chunk in 0..32 {
        values.emit_next_v1(chunk).unwrap();
    }
    assert!(values.emit_next_v1(32).is_err());
    assert!(values.finish_v1().is_err());
}

#[test]
fn group_and_prepared_values_are_erased_on_unwind() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    let outcome = std::panic::catch_unwind(|| {
        let group = complete_group_v1();
        let _values = group
            .prepare_values_v1(low_digit_coordinate_v1(0).unwrap())
            .unwrap();
        panic!("exercise D/S value custody unwind");
    });
    assert!(outcome.is_err());
    assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 2);
}

#[test]
fn poisoned_outer_owners_cannot_emit_prepare_finish_or_recover_a_source() {
    assert!(
        LowDigitPreparationV1::<core::convert::Infallible, (), ()> { live: None }
            .prepare_next_v1()
            .is_err()
    );
    assert!(
        LowDigitPreparationV1::<core::convert::Infallible, (), ()> { live: None }
            .finish_v1()
            .is_err()
    );
    let mut plane = PreparedLowDigitPlaneV1::<core::convert::Infallible, (), ()> { live: None };
    assert!(plane.emit_next_value_chunk_v1(0).is_err());
    assert!(plane.finish_v1().is_err());
}

#[test]
fn actual_source_driver_keeps_custody_order_and_completion_without_fake_provider() {
    let source = include_str!("prepared_low_digit_plane_v1.rs");
    let compact = source.split_whitespace().collect::<String>();
    for required in [
        "validate_radix_materialization_source_v1(",
        "validate_low_digit_preparation_start_v1(",
        "live.cursor.commit_prepared_low_digit_v1(&statement)?",
        "validate_materialized_context_v1(&self).map_err(|_|LowDigitWorkspaceErrorV1::Source)?;",
        "Phase23GlobalLookupRadixSourceCursorV2::begin_v2(evidence)?",
        "cursor.read_next_canonical_block_v2(record,first_block+local_block)?",
        "live.cursor.complete_authenticated_source_replay_v1()?",
        "schedule!=source.record.authenticated_read_schedule_root",
        "source.evidence=Some(evidence)",
        "live.values.finish_v1()?",
    ] {
        assert!(
            compact.contains(required),
            "missing real source boundary: {required}"
        );
    }
    for forbidden in [
        "pub fn",
        "point: &VegaT256Point",
        "sample_blinding",
        "TestOnly",
        "with_capacity(",
        "impl Clone",
        "proof_ready",
        "release_ready: true",
    ] {
        assert!(
            !source.contains(forbidden),
            "unapproved authority or allocation: {forbidden}"
        );
    }
    let entry = compact
        .split("fninto_low_digit_preparation_v1(")
        .nth(1)
        .unwrap()
        .split("impl<R:crate::vega::MaskedRelaxedRandomSourceV1,K,P>LowDigitPreparationV1")
        .next()
        .unwrap();
    assert!(
        entry
            .find("validate_materialized_context_v1(&self)")
            .unwrap()
            < entry.find("self.evidence.take()").unwrap()
    );
    assert!(
        entry
            .find("validate_materialized_context_v1(&self)")
            .unwrap()
            < entry.find("admit_low_digit_workspace_v1()").unwrap()
    );
    assert!(
        entry.find("admit_low_digit_workspace_v1()").unwrap()
            < entry.find("self.evidence.take()").unwrap()
    );
    let emit = compact
        .split("fnemit_next_value_chunk_v1(")
        .nth(1)
        .unwrap()
        .split("fnfinish_v1(")
        .next()
        .unwrap();
    assert!(
        emit.find("self.live.take()").unwrap() < emit.find("live.values.emit_next_v1").unwrap()
    );
    assert!(
        emit.find("live.values.emit_next_v1").unwrap() < emit.find("self.live=Some(live)").unwrap()
    );
    let finish = compact.split("fnfinish_v1(").nth(2).unwrap();
    assert!(
        finish.find("live.values.finish_v1()?").unwrap()
            < finish.find("driver.next_plane=").unwrap()
    );
    assert_eq!(source.matches("next_plane: u16,").count(), 1);
}

#[test]
fn low_digit_workspace_exact_capacity_refusal_preserves_credits_and_retries() {
    use crate::vega::zk_ams::mkhe::rns_native_resource_budget::RnsNativeProofResourceBudgetV1;
    let exact = LowDigitWorkspaceV1::bytes_v1();
    assert_eq!(
        exact,
        1_048_576 + core::mem::size_of::<LowDigitWorkspaceV1>() as u64
    );
    let mut budget = RnsNativeProofResourceBudgetV1::with_test_workspace_limit_v1(exact);
    let blocker = budget.reserve_workspace_v1(1, 0).unwrap();
    for _ in 0..2 {
        assert!(matches!(
            LowDigitWorkspaceV1::admit_v1(&mut budget),
            Err(LowDigitWorkspaceErrorV1::Capacity)
        ));
        assert_eq!(budget.live_bytes().unwrap(), 1);
        assert_eq!(budget.consumed().unwrap(), 0);
        assert_eq!(budget.peak_bytes().unwrap(), 1);
    }
    drop(blocker);
    let workspace = LowDigitWorkspaceV1::admit_v1(&mut budget).unwrap();
    assert!(workspace.belongs_to_v1(&budget));
    assert!(!workspace.belongs_to_v1(&RnsNativeProofResourceBudgetV1::default()));
    assert_eq!(budget.live_bytes().unwrap(), exact);
    drop(workspace);
    assert_eq!(budget.live_bytes().unwrap(), 0);
}

#[test]
fn low_digit_workspace_covers_real_group_and_plane_until_both_are_destroyed() {
    use crate::vega::zk_ams::mkhe::rns_native_resource_budget::RnsNativeProofResourceBudgetV1;
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let mut budget = RnsNativeProofResourceBudgetV1::with_test_workspace_limit_v1(
        LowDigitWorkspaceV1::bytes_v1(),
    );
    let workspace = LowDigitWorkspaceV1::admit_v1(&mut budget).unwrap();
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    let group = complete_group_v1();
    let values = group
        .prepare_values_v1(low_digit_coordinate_v1(0).unwrap())
        .unwrap();
    assert_eq!(
        budget.live_bytes().unwrap(),
        LowDigitWorkspaceV1::bytes_v1()
    );
    assert!(matches!(
        LowDigitWorkspaceV1::admit_v1(&mut budget),
        Err(LowDigitWorkspaceErrorV1::Capacity)
    ));
    drop(values);
    assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 1);
    assert_eq!(
        budget.live_bytes().unwrap(),
        LowDigitWorkspaceV1::bytes_v1()
    );
    drop(group);
    assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 2);
    assert_eq!(
        budget.live_bytes().unwrap(),
        LowDigitWorkspaceV1::bytes_v1()
    );
    drop(workspace);
    assert_eq!(budget.live_bytes().unwrap(), 0);
}

#[test]
fn low_digit_workspace_unwind_erases_real_buffers_before_releasing_admission() {
    use crate::vega::zk_ams::mkhe::rns_native_resource_budget::RnsNativeProofResourceBudgetV1;
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let mut budget = RnsNativeProofResourceBudgetV1::with_test_workspace_limit_v1(
        LowDigitWorkspaceV1::bytes_v1(),
    );
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _workspace = LowDigitWorkspaceV1::admit_v1(&mut budget).unwrap();
        let group = complete_group_v1();
        let _values = group
            .prepare_values_v1(low_digit_coordinate_v1(17).unwrap())
            .unwrap();
        panic!("original low-digit lifetime unwind");
    }));
    assert!(result.is_err());
    assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before + 2);
    assert_eq!(budget.live_bytes().unwrap(), 0);
    assert_eq!(budget.consumed().unwrap(), 0);
}

#[test]
fn low_digit_workspace_actual_driver_owns_the_admission_after_secret_fields() {
    let source = include_str!("prepared_low_digit_plane_v1.rs");
    let driver = source
        .split("struct LowDigitPreparationLiveV1<R, K, P> {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    assert!(driver.find("group:").unwrap() < driver.find("_workspace:").unwrap());
    let plane = source
        .split("struct PreparedLowDigitPlaneLiveV1<R, K, P> {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    assert!(plane.find("values:").unwrap() < plane.find("driver:").unwrap());
    let capacity = source
        .split("Err(reason) =>")
        .nth(1)
        .unwrap()
        .split("let result")
        .next()
        .unwrap();
    assert!(capacity.contains("LowDigitWorkspaceErrorV1::Capacity"));
    assert!(capacity.contains("Some(self)"));
    assert!(capacity.contains("None"));
    let owner = include_str!("prepared_low_digit_plane_v1/low_digit_workspace_v1.rs");
    for forbidden in [
        "impl Clone",
        "impl Copy",
        "Default::default()",
        "pub fn",
        "Vec<",
        "Box<",
    ] {
        assert!(
            !owner.contains(forbidden),
            "unexpected admission surface: {forbidden}"
        );
    }
}

#[test]
fn low_digit_workspace_terminal_refusal_cannot_recover_a_source() {
    let refusal = LowDigitPreparationRefusalV1::<core::convert::Infallible, (), ()> {
        reason: LowDigitWorkspaceErrorV1::Source,
        source: None,
    };
    assert_eq!(refusal.reason_v1(), LowDigitWorkspaceErrorV1::Source);
    let refusal = match refusal.retry_v1() {
        Ok(_) => panic!("terminal refusal unexpectedly returned a source"),
        Err(refusal) => refusal,
    };
    assert!(refusal.source.is_none());
    assert_eq!(refusal.reason_v1(), LowDigitWorkspaceErrorV1::Source);
}
