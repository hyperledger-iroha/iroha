//! Checked resource arithmetic shared by the producer's phase plans.
//!
//! A plan charges payload bytes and structural work before any private work
//! starts. These helpers keep that arithmetic overflow-checked and give every
//! plan the same transform charge. They choose no limit and admit nothing.

use crate::{Error, Result};

/// Conservative four-lane FFT/IFFT, twist, inspection and cleanup work.
pub(super) fn transform_work(rows: usize) -> Result<usize> {
    if !rows.is_power_of_two() {
        return Err(invalid("transform work requires a power-of-two extent"));
    }
    checked_add(
        checked_mul(checked_mul(64, rows)?, rows.ilog2() as usize + 1)?,
        4096,
    )
}

/// Add two resource counts, refusing a wrapped total.
pub(super) fn checked_add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| invalid("plan resource count overflow"))
}

/// Multiply two resource counts, refusing a wrapped product.
pub(super) fn checked_mul(left: usize, right: usize) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| invalid("plan resource count overflow"))
}

fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_counts_refuse_overflow_and_transform_work_needs_a_power_of_two() {
        assert_eq!(checked_add(usize::MAX - 1, 1).unwrap(), usize::MAX);
        assert!(checked_add(usize::MAX, 1).is_err());
        assert_eq!(checked_mul(usize::MAX / 2, 2).unwrap(), usize::MAX - 1);
        assert!(checked_mul(usize::MAX, 2).is_err());
        assert!(transform_work(0).is_err());
        assert!(transform_work(3).is_err());
        assert!(transform_work(1_usize << (usize::BITS - 1)).is_err());
        assert_eq!(transform_work(1).unwrap(), 64 + 4096);
        assert_eq!(transform_work(8).unwrap(), 64 * 8 * 4 + 4096);
    }
}
