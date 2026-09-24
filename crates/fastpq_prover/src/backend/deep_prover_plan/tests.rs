//! Fixed-source DEEP preflight refusals and checked resource arithmetic.

use super::*;
use crate::gadgets::compact_smt_air::{DigestLimbs, PublicStatement, PublicUpdate};
use iroha_crypto::Hash;

fn digest(seed: u8) -> DigestLimbs {
    let hash = Hash::new([seed; 33]);
    let bytes: &[u8; 32] = hash.as_ref();
    core::array::from_fn(|limb| {
        u32::from_le_bytes(bytes[4 * limb..4 * limb + 4].try_into().unwrap())
    })
}

fn relation() -> CompactTransferAir {
    CompactTransferAir::new(
        &PublicStatement {
            updates: [
                PublicUpdate {
                    old_leaf: digest(1),
                    new_leaf: digest(2),
                    path: 0xa59c_71e3,
                },
                PublicUpdate {
                    old_leaf: digest(3),
                    new_leaf: digest(4),
                    path: 0x6a35_8e1c,
                },
            ],
            old_root: digest(5),
            new_root: digest(6),
        },
        Some(b"DEEP preflight full relation"),
    )
    .unwrap()
}

fn with_source<T>(f: impl FnOnce(&SourceTraceColumns<'_>, &CompactTransferAir) -> T) -> T {
    let zeros = vec![0; TRACE_ROWS];
    let period: Vec<[u64; COLUMN_COUNT]> = (0..512)
        .map(|phase| {
            let mut complete = [0; COLUMN_COUNT];
            if phase < 408 {
                let domain = b"fastpq:v1:smt:node|";
                for limb in 0..4 {
                    complete[32 + limb] = u64::from(u32::from_le_bytes(
                        domain[4 * limb..4 * limb + 4].try_into().unwrap(),
                    ));
                }
                for byte in 0..24 {
                    complete[276 + byte] = u64::from(phase < 6 && 24 * phase + byte < 83);
                }
                complete[300] = 83;
                complete[301] = if phase < 6 {
                    ((24 * (phase + 1)) as u64).min(83)
                } else {
                    83
                };
            }
            complete
        })
        .collect();
    let known: Vec<Vec<u64>> = PUBLIC_COLUMNS
        .iter()
        .map(|&column| {
            (0..TRACE_ROWS)
                .map(|row| period[row % period.len()][column])
                .collect()
        })
        .collect();
    let mut columns = vec![zeros.as_slice(); COLUMN_COUNT];
    for (&column, values) in PUBLIC_COLUMNS.iter().zip(&known) {
        columns[column] = values;
    }
    let sentinel_sources = [31, 36, 52, 64, 275, 302, 341];
    let sentinels: Vec<Vec<u64>> = sentinel_sources
        .iter()
        .enumerate()
        .map(|(slot, _)| {
            let mut values = zeros.clone();
            values[0] = slot as u64 + 1;
            values[TRACE_ROWS - 1] = slot as u64 + 101;
            values
        })
        .collect();
    for (&column, values) in sentinel_sources.iter().zip(&sentinels) {
        columns[column] = values;
    }
    let source = SourceTraceColumns::new(&columns).unwrap();
    f(&source, &relation())
}

fn shape(coefficients: usize, degree_bound: usize) -> MaskingShape {
    MaskingShape {
        trace_coefficients: TRACE_ROWS,
        trace_degree_bound: TRACE_ROWS,
        mask_coefficients: coefficients,
        mask_degree_bound: degree_bound,
    }
}

fn limits() -> DeepProverLimits {
    DeepProverLimits {
        max_complete_row_lde_bytes: complete_row_lde_floor_bytes().unwrap(),
    }
}

#[test]
fn exact_source_and_full_air_plan_refuses_unmasked_private_openings() {
    with_source(|source, air| {
        for (slot, column) in [31, 36, 52, 64, 275, 302, 341].into_iter().enumerate() {
            let retained = COMMITTED_COLUMNS.binary_search(&column).unwrap();
            assert_eq!(source.committed_row(0).unwrap()[retained], slot as u64 + 1);
            assert_eq!(
                source.committed_row(TRACE_ROWS - 1).unwrap()[retained],
                slot as u64 + 101
            );
        }
        let zero = [F::ZERO];
        let masks = vec![zero.as_slice(); COMMITTED_COLUMN_COUNT];
        let shapes = vec![shape(1, 1); COMMITTED_COLUMN_COUNT];
        let plan = DeepProverPlan::new(source, air, &masks, &shapes, limits()).unwrap();
        assert_eq!(plan.complete_row_lde_floor_bytes, 20_199_768_064);
        assert_eq!(plan.numerator_degree_bound, 196_479);
        assert_eq!(plan.conditional_quotient_degree_bound, 130_943);
        assert_eq!(plan.widened_row_opening_bytes, 616_448);
        assert!(plan.widened_row_opening_bytes > plan.proof_byte_target);
        assert_eq!(plan.proof_byte_target, 512 * 1024);
        assert_eq!(plan.two_max_child_frames_bytes, 1_012_702);
        assert_eq!(plan.axt_inner_payload_ceiling_bytes, 1024 * 1024);
        assert_eq!(
            plan.axt_inner_payload_ceiling_bytes - plan.two_max_child_frames_bytes,
            35_874
        );
        assert_eq!(
            plan.require_private_proof(),
            Err(DeepPrivateProofRefusal::UnmaskedPrivateOpenings)
        );
    });
}

#[test]
fn every_sampled_retained_region_stays_closed_with_partial_or_extension_masks() {
    with_source(|source, air| {
        let zero = [F::ZERO];
        let nonzero = [F::ONE];
        let mut masks = vec![zero.as_slice(); COMMITTED_COLUMN_COUNT];
        let shapes = vec![shape(1, 1); COMMITTED_COLUMN_COUNT];
        for source_column in [0, 31, 36, 52, 64, 275, 302, 341] {
            let retained_column = COMMITTED_COLUMNS.binary_search(&source_column).unwrap();
            masks[retained_column] = &nonzero;
            let plan = DeepProverPlan::new(source, air, &masks, &shapes, limits()).unwrap();
            assert_eq!(
                plan.require_private_proof(),
                Err(DeepPrivateProofRefusal::UnmaskedPrivateOpenings)
            );
            assert!(plan.conditional_quotient_degree_bound < 3 * TRACE_ROWS);
            masks[retained_column] = &zero;
        }
        let extension_mask = [F::new([0, 1, 0, 0]).unwrap()];
        masks[COMMITTED_COLUMN_COUNT - 1] = &extension_mask;
        let plan = DeepProverPlan::new(source, air, &masks, &shapes, limits()).unwrap();
        assert_eq!(
            plan.require_private_proof(),
            Err(DeepPrivateProofRefusal::ExtensionMaskCannotUseBaseRows {
                retained_column: COMMITTED_COLUMN_COUNT - 1,
                source_column: COMMITTED_COLUMNS[COMMITTED_COLUMN_COUNT - 1],
            })
        );
    });
}

#[test]
fn late_mask_term_and_full_air_degree_are_counted_without_interpolation() {
    with_source(|source, air| {
        let mut high = [F::ZERO; 66];
        high[65] = F::ONE;
        let masks = vec![high.as_slice(); COMMITTED_COLUMN_COUNT];
        let shapes = vec![shape(66, 66); COMMITTED_COLUMN_COUNT];
        let plan = DeepProverPlan::new(source, air, &masks, &shapes, limits()).unwrap();
        assert_eq!(
            plan.require_private_proof(),
            Err(DeepPrivateProofRefusal::MissingReviewedHidingAndComposition)
        );
        assert!(plan.conditional_quotient_degree_bound > 2 * TRACE_ROWS);
        assert!(plan.numerator_degree_bound > 3 * TRACE_ROWS);
    });
}

#[test]
fn screened_half_trace_mask_degree_fits_but_cannot_authorize_proving() {
    with_source(|source, air| {
        let mut high = vec![F::ZERO; TRACE_ROWS / 2];
        high[TRACE_ROWS / 2 - 1] = F::ONE;
        let masks = vec![high.as_slice(); COMMITTED_COLUMN_COUNT];
        let shapes = vec![shape(high.len(), high.len()); COMMITTED_COLUMN_COUNT];
        let plan = DeepProverPlan::new(source, air, &masks, &shapes, limits()).unwrap();
        assert_eq!(FRI_DEGREES[0], 2 * TRACE_ROWS);
        assert_eq!(plan.numerator_degree_bound, 262_015);
        assert_eq!(plan.conditional_quotient_degree_bound, 196_479);
        assert_eq!(plan.conditional_quotient_degree_bound - TRACE_ROWS, 130_943);
        assert_eq!(
            plan.require_private_proof(),
            Err(DeepPrivateProofRefusal::MissingReviewedHidingAndComposition)
        );
    });
}

#[test]
fn high_mask_with_out_of_budget_quotient_is_rejected_before_proving() {
    with_source(|source, air| {
        let mut high = vec![F::ZERO; TRACE_ROWS];
        high[TRACE_ROWS - 1] = F::ONE;
        let masks = vec![high.as_slice(); COMMITTED_COLUMN_COUNT];
        let shapes = vec![shape(high.len(), high.len()); COMMITTED_COLUMN_COUNT];
        let plan = DeepProverPlan::new(source, air, &masks, &shapes, limits()).unwrap();
        assert_eq!(plan.numerator_degree_bound, 327_551);
        assert_eq!(plan.conditional_quotient_degree_bound, 262_015);
        assert_eq!(
            plan.require_private_proof(),
            Err(DeepPrivateProofRefusal::QuotientExceedsFriDegree {
                quotient_degree_bound: 262_015,
                high_half_degree_bound: 196_479,
                fri_degree_bound: FRI_DEGREES[0],
            })
        );
    });
}

#[test]
fn insufficient_materialized_lde_budget_refuses_before_mask_inspection() {
    with_source(|source, air| {
        let floor = complete_row_lde_floor_bytes().unwrap();
        let result = DeepProverPlan::new(
            source,
            air,
            &[],
            &[],
            DeepProverLimits {
                max_complete_row_lde_bytes: floor - 1,
            },
        );
        // Shape validation is first, so supply the complete mask dimensions.
        assert!(matches!(result, Err(Error::InvalidTraceShape { .. })));
        let zero = [F::ZERO];
        let masks = vec![zero.as_slice(); COMMITTED_COLUMN_COUNT];
        let shapes = vec![shape(1, 1); COMMITTED_COLUMN_COUNT];
        assert!(matches!(
            DeepProverPlan::new(
                source,
                air,
                &masks,
                &shapes,
                DeepProverLimits {
                    max_complete_row_lde_bytes: floor - 1,
                },
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_deep_complete_row_lde_bytes",
                actual,
                max,
            }) if actual == floor && max == floor - 1
        ));
        assert!(DeepProverPlan::new(source, air, &masks, &shapes, limits()).is_ok());
    });
}

#[test]
fn malformed_or_noncanonical_masks_refuse_before_degree_report() {
    with_source(|source, air| {
        let zero = [F::ZERO];
        let nonzero = [F::ONE];
        let mut masks = vec![zero.as_slice(); COMMITTED_COLUMN_COUNT];
        let mut shapes = vec![shape(1, 1); COMMITTED_COLUMN_COUNT];
        masks[0] = &nonzero;
        let bad = [F::from_coefficients_unchecked_for_test([
            0,
            0,
            0,
            super::super::GOLDILOCKS_MODULUS,
        ])];
        let last = COMMITTED_COLUMN_COUNT - 1;
        masks[last] = &bad;
        assert!(matches!(
            DeepProverPlan::new(source, air, &masks, &shapes, limits()),
            Err(Error::NonCanonicalGoldilocksElement { context: "deep_prover_mask", indices })
                if indices == [last, 0, 3]
        ));
        let padded = [F::ZERO, F::ONE];
        masks[last] = &padded;
        shapes[last] = shape(2, 1);
        assert!(matches!(
            DeepProverPlan::new(source, air, &masks, &shapes, limits()),
            Err(Error::InvalidTraceShape { details })
                if details == "DEEP mask has nonzero declared degree padding"
        ));
        shapes[last] = shape(2, 2);
        assert!(DeepProverPlan::new(source, air, &masks, &shapes, limits()).is_ok());
    });
}

#[test]
fn checked_byte_and_degree_arithmetic_rejects_overflow() {
    assert_eq!(complete_row_lde_floor_bytes().unwrap(), 20_199_768_064);
    assert!(checked_product(&[usize::MAX, 2]).is_err());
    assert!(checked_sum(&[usize::MAX, 1]).is_err());
}
