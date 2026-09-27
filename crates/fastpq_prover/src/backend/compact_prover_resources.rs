//! Conservative per-segment charges for the sole bounded DEEP producer.
//!
//! The shared producer charge covers its fixed LDE, commitment trees, quotient,
//! extension arrays and encoded openings. This outer boundary additionally
//! charges physical witness expansion, conversion and caller context before any
//! private witness is expanded. Charges sum lifetimes, are local policy, and do
//! not reserve process RSS. Private SMT construction and retained child frames
//! retain their separate request-level limits.

use super::deep_prover;
use crate::{
    Error, Result,
    gadgets::compact_smt_air::{COLUMN_COUNT, PHYSICAL_ROW_COUNT},
};

/// Fixed conservative payload ceiling of the exact quotient phase.
///
/// The exact plan must fit this ceiling before any full-domain LDE is allocated.
/// Charging the entire ceiling here keeps witness preflight independent of
/// witness coefficients and the later source-derived exact quotient plan.
pub(super) fn quotient_payload_ceiling() -> Result<usize> {
    usize::try_from(8_u64 << 30).map_err(|_| overflow())
}

/// Charge the complete producer plus conversion and independently bound context.
pub(super) fn segment_charge(statement_bytes: usize, child_frame_bytes: usize) -> Result<usize> {
    let mut charge = deep_prover::payload_charge(quotient_payload_ceiling()?)?;
    // Logical/physical witness growth, full base rows and guarded projected
    // coefficients fit within five complete base matrices. The source matrix
    // is released before the producer retains its 301 full-domain LDE columns.
    add(&mut charge, &[5, COLUMN_COUNT, PHYSICAL_ROW_COUNT, 8])?;
    add(&mut charge, &[8, statement_bytes])?;
    add(&mut charge, &[8, child_frame_bytes])?;
    Ok(charge)
}

/// Reject an insufficient complete segment policy before private row allocation.
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
    use crate::backend::{
        compact_public_columns::COMMITTED_COLUMN_COUNT,
        deep_geometry::{LDE_ROWS, TRACE_ROWS},
        deep_proof::MAX_FRAME_BYTES,
    };

    #[test]
    fn charge_covers_the_shared_producer_quotient_and_source_conversion() {
        let source = 5 * COLUMN_COUNT * PHYSICAL_ROW_COUNT * 8;
        let producer = deep_prover::payload_charge(quotient_payload_ceiling().unwrap()).unwrap();
        assert_eq!(segment_charge(0, 0).unwrap(), source + producer);
        assert!(producer > COMMITTED_COLUMN_COUNT * LDE_ROWS * 8);
        assert_eq!(
            quotient_payload_ceiling().unwrap(),
            usize::try_from(8_u64 << 30).unwrap()
        );
        assert_eq!(PHYSICAL_ROW_COUNT, TRACE_ROWS);
        assert_eq!(COMMITTED_COLUMN_COUNT, 301);
    }

    #[test]
    fn variable_context_and_frame_charges_reject_overflow() {
        let base = segment_charge(0, 0).unwrap();
        assert_eq!(segment_charge(3, 7).unwrap(), base + 8 * (3 + 7));
        assert!(segment_charge(usize::MAX, 0).is_err());
        assert!(segment_charge(0, usize::MAX).is_err());
    }

    #[test]
    fn exact_policy_boundary_is_inclusive() {
        let actual = segment_charge(256 * 1024, MAX_FRAME_BYTES).unwrap();
        assert_eq!(
            check_segment_charge(256 * 1024, MAX_FRAME_BYTES, actual).unwrap(),
            actual
        );
        assert!(matches!(
            check_segment_charge(256 * 1024, MAX_FRAME_BYTES, actual - 1),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_prover_segment_charge_bytes", actual: measured, max,
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
