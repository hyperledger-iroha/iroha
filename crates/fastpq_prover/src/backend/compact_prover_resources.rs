//! Witness-free structural payload floor for one masked DEEP proof segment.
//!
//! This floor covers physical witness expansion, bounded masked replay, and
//! caller framing before private trees are constructed. It is not the complete
//! relation-dependent producer charge or an RSS reservation. Each segment must
//! additionally pass the exact `deep_prover::ProducerPlan` before trace expansion;
//! that plan includes quotient, commitment, FRI and codec workspace.

use super::deep_masked_replay::{MaskedReplayPlan, ReplayLimits};
use crate::{
    Error, Result,
    gadgets::compact_smt_air::{COLUMN_COUNT, PHYSICAL_ROW_COUNT},
};

/// Derive the fixed replay requirements without a witness, entropy or transforms.
pub(super) fn replay_plan() -> Result<MaskedReplayPlan> {
    MaskedReplayPlan::new(ReplayLimits {
        max_payload_bytes: usize::MAX,
        max_work_units: usize::MAX,
        max_full_passes: 3,
    })
}

/// Charge a necessary structural floor plus complete bound context and framing.
pub(super) fn segment_charge(statement_bytes: usize, child_frame_bytes: usize) -> Result<usize> {
    let mut charge = replay_plan()?.payload_bytes;
    // Logical/physical witness growth and guarded source columns. The exact
    // producer plan separately counts its borrowed physical source matrix.
    add(&mut charge, &[5, COLUMN_COUNT, PHYSICAL_ROW_COUNT, 8])?;
    add(&mut charge, &[8, statement_bytes])?;
    add(&mut charge, &[8, child_frame_bytes])?;
    Ok(charge)
}

/// Reject insufficient outer structural policy before witness expansion.
pub(super) fn check_segment_charge(
    statement_bytes: usize,
    child_frame_bytes: usize,
    maximum: usize,
) -> Result<()> {
    let actual = segment_charge(statement_bytes, child_frame_bytes)?;
    if actual > maximum {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_compact_prover_segment_charge_bytes",
            actual,
            max: maximum,
        });
    }
    Ok(())
}

fn add(charge: &mut usize, factors: &[usize]) -> Result<()> {
    let amount = factors
        .iter()
        .try_fold(1usize, |n, &factor| n.checked_mul(factor))
        .ok_or_else(overflow)?;
    *charge = charge.checked_add(amount).ok_or_else(overflow)?;
    Ok(())
}

fn overflow() -> Error {
    Error::TransferInvariant {
        details: "compact prover structural charge overflows".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charge_covers_source_conversion_and_masked_replay() {
        let replay = replay_plan().unwrap();
        let source = 5 * COLUMN_COUNT * PHYSICAL_ROW_COUNT * 8;
        assert_eq!(segment_charge(0, 0).unwrap(), source + replay.payload_bytes);
        assert_eq!(replay.stripes(), 128);
        assert_eq!(replay.maximum_column_transforms, 301 * (1 + 3 * 128));
        assert_eq!(
            PHYSICAL_ROW_COUNT,
            crate::backend::deep_geometry::TRACE_ROWS
        );
        assert_eq!(
            size_of::<crate::gadgets::compact_smt_air::SmtRow<u64>>(),
            COLUMN_COUNT * 8
        );
    }

    #[test]
    fn charge_enforces_inclusive_limits_and_context() {
        let charge = segment_charge(256, 1024).unwrap();
        assert_eq!(charge, segment_charge(0, 0).unwrap() + 8 * (256 + 1024));
        check_segment_charge(256, 1024, charge).unwrap();
        assert!(matches!(
            check_segment_charge(256, 1024, charge - 1),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_prover_segment_charge_bytes", actual, max
            }) if actual == charge && max == charge - 1
        ));
        assert!(segment_charge(usize::MAX, 0).is_err());
        assert!(segment_charge(0, usize::MAX).is_err());
    }
}
