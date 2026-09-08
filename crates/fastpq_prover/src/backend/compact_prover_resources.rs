//! Early structural working-set charges for one fixed compact proof segment.
//!
//! These charges deliberately sum buffers whose lifetimes need not overlap.
//! They cover the fixed trace, public preparation, commitment/FRI payloads and
//! bounded evaluator jobs before witness rows or transforms are constructed.
//! They are accounting units, not an allocator reservation or a process RSS
//! ceiling: allocator metadata, thread stacks, Rayon runtime, cold compilation
//! of the source-owned hash DAG and unrelated process memory are excluded.
//! Private SMT construction, retained child frames and complete public inputs
//! have separate caller limits; this per-segment charge cannot replace them.
//! Dense fallible iterator slots use the current Rust type layout. This is a
//! local prover policy value, never a consensus charge or a proof field.

use fastpq_isi::GoldilocksDigest384V1;

use super::{
    compact_hash_quotient::{
        LOCAL_SLOTS, MAX_PROVER_LEDGER_NODES, MAX_PROVER_OUTPUT_TERMS, MAX_PROVER_SELECTOR_MASKS,
        MAX_PROVER_SELECTOR_RUNS, TRANSITION_SLOTS,
    },
    compact_protocol::MAX_PROVER_JOBS,
    compact_smt_quotient::{FIXED_COLUMN_COUNT, FIXED_ROW_COUNT, RESIDUE_COUNT},
};
use crate::{
    Error, GoldilocksFp4V1, Result,
    gadgets::compact_smt_air::{COLUMN_COUNT, PHYSICAL_HASH_ROWS, PHYSICAL_ROW_COUNT},
};

const BLOWUP: usize = 8;
const MASK_CYCLE_ROWS: usize = 4096;
const QUERIES: usize = 375;
const FRI_FOLDS: usize = 17;
const TREE_DEPTH: usize = 19;
const CONSTRAINTS: usize = LOCAL_SLOTS + TRANSITION_SLOTS + RESIDUE_COUNT;

/// Charge one segment without materializing a relation, witness, FFT or LDE.
///
/// `statement_bytes` includes its complete bound caller context. The frame
/// argument is the caller's already validated maximum for one encoded child,
/// rather than the length of a proof which has already been generated.
pub(super) fn segment_charge(statement_bytes: usize, child_frame_bytes: usize) -> Result<usize> {
    let mut charge = 0usize;
    let lde_rows = product(&[PHYSICAL_ROW_COUNT, BLOWUP])?;

    // Logical-to-physical growth/boxing, physical rows, column conversion and
    // coefficients are charged as five whole base matrices. The planner's
    // parallel padded FFT vectors become the final LDE columns; FFT itself is
    // in-place and has no additional matrix for each Rayon worker.
    add(&mut charge, &[5, COLUMN_COUNT, PHYSICAL_ROW_COUNT, 8])?;
    add(&mut charge, &[COLUMN_COUNT, lde_rows, 8])?;
    add(&mut charge, &[FIXED_COLUMN_COUNT, PHYSICAL_ROW_COUNT, 8])?;
    add(&mut charge, &[FIXED_COLUMN_COUNT, lde_rows, 8])?;
    add(&mut charge, &[2, PHYSICAL_ROW_COUNT, 8])?; // Both planner coset tables.
    add(&mut charge, &[MASK_CYCLE_ROWS, PHYSICAL_HASH_ROWS, 8])?;
    add(
        &mut charge,
        &[MASK_CYCLE_ROWS, MAX_PROVER_SELECTOR_MASKS, 8],
    )?;
    add(&mut charge, &[2, FIXED_ROW_COUNT, FIXED_COLUMN_COUNT, 8])?;

    // Mixed (one), quotient chunks plus concatenated output (two), joint
    // (one), initial FRI clone (one) and retained geometric FRI layers (less
    // than two) fit in eight complete extension arrays. Twelve digest arrays
    // cover the three complete oracle trees, geometric FRI trees, row chunks
    // and tree-building leaves. Fallible parallel collections additionally
    // retain Result slots before extracting successful values. Full trees use
    // 48-byte field digests even when their commitment hash is SHAKE256.
    add(&mut charge, &[8, lde_rows, size_of::<GoldilocksFp4V1>()])?;
    add(
        &mut charge,
        &[12, lde_rows, size_of::<GoldilocksDigest384V1>()],
    )?;
    add(
        &mut charge,
        &[lde_rows, size_of::<Result<GoldilocksDigest384V1>>()],
    )?;
    add(
        &mut charge,
        &[lde_rows, size_of::<Result<GoldilocksFp4V1>>()],
    )?;

    // Each fixed indexed partition owns one evaluator, two input rows and one
    // residue vector. Hash node/mask/run/term counts have normal-path guards.
    add(&mut charge, &[MAX_PROVER_JOBS, MAX_PROVER_LEDGER_NODES, 8])?;
    add(&mut charge, &[MAX_PROVER_JOBS, 2, COLUMN_COUNT, 8])?;
    add(&mut charge, &[MAX_PROVER_JOBS, CONSTRAINTS, 8])?;
    // SHAKE row commitments serialize one row per indexed job. Query assembly
    // later owns another row pair, after evaluator jobs have been dropped.
    add(&mut charge, &[MAX_PROVER_JOBS, COLUMN_COUNT, 8])?;
    add(&mut charge, &[2, COLUMN_COUNT, 8])?;
    add(&mut charge, &[MAX_PROVER_LEDGER_NODES, 64])?;
    add(&mut charge, &[MAX_PROVER_SELECTOR_MASKS, 48])?;
    add(&mut charge, &[MAX_PROVER_SELECTOR_RUNS, 32])?;
    add(&mut charge, &[MAX_PROVER_OUTPUT_TERMS, 32])?;

    // Expanded openings coexist with shared-frontier conversion. Count every
    // path at the largest depth, without discounting shared ancestors, plus
    // ownership/map bookkeeping. Encoding and statement copies are separate
    // conservative charges, not claimed exact Norito allocation sizes.
    add(&mut charge, &[QUERIES, 2, COLUMN_COUNT, 8])?;
    add(&mut charge, &[QUERIES, 4 + FRI_FOLDS, TREE_DEPTH, 48])?;
    add(&mut charge, &[QUERIES, FRI_FOLDS, 3, 32])?;
    add(&mut charge, &[QUERIES, FRI_FOLDS + 4, 256])?;
    add(&mut charge, &[8, child_frame_bytes])?;
    add(&mut charge, &[8, statement_bytes])?;
    Ok(charge)
}

/// Reject insufficient per-segment caller policy before allocating trace data.
pub(super) fn check_segment_charge(
    statement_bytes: usize,
    child_frame_bytes: usize,
    maximum: usize,
) -> Result<usize> {
    let actual = segment_charge(statement_bytes, child_frame_bytes)?;
    if actual > maximum {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_compact_prover_segment_charge_bytes",
            actual,
            max: maximum,
        });
    }
    Ok(actual)
}

fn product(factors: &[usize]) -> Result<usize> {
    factors.iter().try_fold(1usize, |value, factor| {
        value.checked_mul(*factor).ok_or_else(overflow)
    })
}

fn add(total: &mut usize, factors: &[usize]) -> Result<()> {
    *total = total.checked_add(product(factors)?).ok_or_else(overflow)?;
    Ok(())
}

fn overflow() -> Error {
    Error::InvalidTraceShape {
        details: "compact prover segment charge overflow".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_charge_covers_the_trace_and_fixed_preparation() {
        let charge = segment_charge(0, 0).unwrap();
        let base = COLUMN_COUNT * PHYSICAL_ROW_COUNT * 8;
        let lde = base * BLOWUP;
        let fixed = FIXED_COLUMN_COUNT * PHYSICAL_ROW_COUNT * (BLOWUP + 1) * 8;
        assert!(charge > 5 * base + lde + fixed);
        assert_eq!(size_of::<GoldilocksDigest384V1>(), 48);
        assert_eq!(size_of::<GoldilocksFp4V1>(), 32);
        assert_eq!(MAX_PROVER_JOBS, 32);
    }

    #[test]
    fn fixed_profile_matches_the_charged_geometry_and_native_rows() {
        use crate::gadgets::compact_smt_air::SmtRow;

        assert_eq!(PHYSICAL_ROW_COUNT, 65_536);
        assert_eq!(COLUMN_COUNT, 342);
        assert_eq!(CONSTRAINTS, 923);
        assert_eq!(PHYSICAL_HASH_ROWS * BLOWUP, MASK_CYCLE_ROWS);
        assert_eq!(
            fastpq_isi::FASTPQ_FINAL_V1.fri.blowup_factor as usize,
            BLOWUP
        );
        assert_eq!(fastpq_isi::FASTPQ_FINAL_V1.fri.arity, 2);
        assert_eq!((PHYSICAL_ROW_COUNT * BLOWUP).ilog2() as usize, TREE_DEPTH);
        assert_eq!(PHYSICAL_ROW_COUNT * BLOWUP >> FRI_FOLDS, 4);
        // Logical/physical row vectors must not silently acquire an uncharged
        // field or padding while column width remains unchanged.
        assert_eq!(size_of::<SmtRow<u64>>(), COLUMN_COUNT * size_of::<u64>());
    }

    #[test]
    fn both_variable_inputs_contribute_and_overflow_is_rejected() {
        let base = segment_charge(0, 0).unwrap();
        assert_eq!(segment_charge(3, 7).unwrap(), base + 8 * (3 + 7));
        assert!(segment_charge(usize::MAX, 0).is_err());
        assert!(segment_charge(0, usize::MAX).is_err());
    }

    #[test]
    fn exact_policy_boundary_is_inclusive() {
        let actual = segment_charge(256 * 1024, 4_279_877).unwrap();
        assert_eq!(
            check_segment_charge(256 * 1024, 4_279_877, actual).unwrap(),
            actual
        );
        assert!(matches!(
            check_segment_charge(256 * 1024, 4_279_877, actual - 1),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_prover_segment_charge_bytes",
                actual: measured,
                max,
            }) if measured == actual && max == actual - 1
        ));
        assert!(check_segment_charge(0, 0, 0).is_err());
    }

    #[test]
    fn checked_products_and_sums_do_not_wrap() {
        assert_eq!(product(&[3, 5, 7]).unwrap(), 105);
        assert!(product(&[usize::MAX, 2]).is_err());
        let mut total = usize::MAX - 1;
        assert!(add(&mut total, &[2]).is_err());
        assert_eq!(total, usize::MAX - 1);
    }
}
