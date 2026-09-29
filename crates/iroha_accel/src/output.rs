//! Escaping host outputs retain the original exact allocation charge.

use mv::allocation::ChargedBuffer;
use mv::allocation::{AllocationReservation, PrepaidBufferError};
use std::ops::{Deref, DerefMut};

/// Local refusal while reserving or allocating a caller-owned native result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostOutputError {
    /// The element count cannot be represented by a valid host allocation layout.
    InvalidLayout,
    /// The original configured process host pool has insufficient remaining capacity.
    Capacity,
    /// The allocator could not supply the already reserved backing storage.
    Allocation,
}

impl std::fmt::Display for HostOutputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLayout => "native output layout is not representable",
            Self::Capacity => "native output exceeds available process host capacity",
            Self::Allocation => "native output allocation failed",
        })
    }
}
impl std::error::Error for HostOutputError {}

/// Initialized host output whose original allocation stays charged until final drop.
///
/// There is no safe conversion to an uncharged Vec. Consumers borrow the values
/// or move this owner into persistent storage. Byte custody is separate from the
/// completed operation's in-flight permit.
pub struct HostOutput<T> {
    values: ChargedBuffer<T>,
}

impl<T: Copy + Default> HostOutput<T> {
    pub(crate) fn from_reservation(
        len: usize,
        reservation: &mut AllocationReservation,
    ) -> Result<Self, PrepaidBufferError> {
        let mut values = ChargedBuffer::from_reservation(len, reservation)?;
        for _ in 0..len {
            values.push_reserved(T::default());
        }
        Ok(Self { values })
    }
}

impl<T> HostOutput<T> {
    /// Number of initialized output values.
    pub fn len(&self) -> usize {
        self.values.as_slice().len()
    }
    /// Whether this output has no initialized values.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Borrow the initialized values without detaching their allocation owner.
    pub fn as_slice(&self) -> &[T] {
        self.values.as_slice()
    }
    pub(crate) fn as_mut_slice(&mut self) -> &mut [T] {
        self.values.as_mut_slice()
    }
}

impl<T> Deref for HostOutput<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

// Mutable access never releases or replaces the original allocation.
impl<T> DerefMut for HostOutput<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for HostOutput<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("HostOutput")
            .field(&self.as_slice())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mv::allocation::AllocationBudget;

    #[test]
    fn escaped_output_keeps_original_charge_after_parent_refund_and_shrink() {
        let budget = AllocationBudget::new(64);
        let mut reservation = budget.try_reserve_bytes(64).unwrap();
        let mut output = HostOutput::<u64>::from_reservation(4, &mut reservation).unwrap();
        assert_eq!(output.len(), 4);
        assert!(!output.is_empty());
        output.copy_from_slice(&[1, 2, 3, 4]);
        drop(reservation);
        assert_eq!(budget.reserved_bytes(), 32);
        budget.set_limit_bytes(0);
        assert_eq!(output.as_slice(), [1, 2, 3, 4]);
        assert_eq!(budget.reserved_bytes(), 32);
        drop(output);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn rejected_output_does_not_consume_original_credit() {
        let budget = AllocationBudget::new(8);
        let mut reservation = budget.try_reserve_bytes(8).unwrap();
        assert!(HostOutput::<u64>::from_reservation(2, &mut reservation).is_err());
        assert_eq!(reservation.remaining_bytes(), 8);
        let empty = HostOutput::<u64>::from_reservation(0, &mut reservation).unwrap();
        assert!(empty.is_empty());
        assert_eq!(reservation.remaining_bytes(), 8);
        drop(empty);
        drop(reservation);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
