//! Original shared canonical policy controls retained outside disposable World overlays.
//!
//! The controls fund canonical decode scopes only. Policy graphs and serializer scratch
//! still require their own custody; this owner supplies no policy or State authority.

use super::*;
use iroha_allocation::{AllocationBudget, ChargedShared, PrepaidSharedError};
use norito::core::{PreparedDecodeScopeError, PreparedDecodeWorkspace};
use parking_lot::Mutex;

/// The actual original counter controls lent to every policy read of a native attempt.
#[derive(Clone)]
pub struct PolicyDecodeControls(ChargedShared<Mutex<PreparedDecodeWorkspace>>);

impl PolicyDecodeControls {
    /// Admit the shared owner and both counter controls atomically before World acquisition.
    pub(crate) fn try_new(pool: &AllocationBudget) -> Result<Self, ExecutionDeferred> {
        let counters = PreparedDecodeWorkspace::allocation_layouts();
        let mut reservation = pool.try_reserve_layouts([
            ChargedShared::<Mutex<PreparedDecodeWorkspace>>::allocation_layout(),
            counters[0],
            counters[1],
        ])?;
        let workspace =
            PreparedDecodeWorkspace::from_reservation(pool, &mut reservation).map_err(|error| {
                match error {
                    PreparedDecodeScopeError::Allocation(
                        original @ PrepaidSharedError::Allocator { .. },
                    ) => iroha_allocation::AllocatorRefusal::try_from(original)
                        .expect("matched physical policy control refusal")
                        .into(),
                    _ => ExecutionDeferred::from(ExecutionDeferral::LocalInvariantViolation),
                }
            })?;
        let original = ChargedShared::from_reservation(Mutex::new(workspace), &mut reservation)
            .map_err(|(_, error)| match error {
                original @ PrepaidSharedError::Allocator { .. } => {
                    iroha_allocation::AllocatorRefusal::try_from(original)
                        .expect("matched physical policy owner refusal")
                        .into()
                }
                PrepaidSharedError::Reservation(_) => {
                    ExecutionDeferred::from(ExecutionDeferral::LocalInvariantViolation)
                }
            })?;
        Ok(Self(original))
    }

    /// Decode the current row through the same canonical kernel and original controls.
    pub(crate) fn decode<T>(
        &self,
        pool: &AllocationBudget,
        bytes: &[u8],
        limits: norito::DecodeLimits,
    ) -> Result<T, norito::CanonicalDecodeControlError>
    where
        T: norito::NoritoSerialize,
        for<'de> T: norito::NoritoDeserialize<'de>,
    {
        norito::validate_canonical_header(bytes)
            .map_err(norito::CanonicalDecodeControlError::Decode)?;
        if !self.0.belongs_to(pool) {
            return Err(norito::CanonicalDecodeControlError::Scope(
                PreparedDecodeScopeError::ForeignPool,
            ));
        }
        // A decoder may retire temporary values during canonical verification. Any
        // original-pool notification runs only after the shared physical guard ends.
        pool.with_deferred_refund_notifications(|_| self.0.lock().decode_canonical(bytes, limits))
    }
}

#[cfg(test)]
mod tests;
