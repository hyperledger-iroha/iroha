//! Exact requested layouts for World journal wrappers and their field vectors.
//!
//! Planning uses only types from the sole World overlay inventory and allocates
//! no payload or layout collection. It needs neither a World nor its writers.
//! Capture retains one typed Box per field plus a field Vec. Installation adds
//! one prepared Box per field and another Vec while all original boxes and the
//! original Vec remain alive for abort/retry.
//!
//! This is shell demand only, never complete carrier admission. Nested MV/EBR
//! storage, payloads, events, catalogs, runtime owners, archive custody, allocator
//! bookkeeping and budget control storage require their own resource admission.
//! No encoded-size estimate or inline `size_of` inference funds those owners.

use super::*;
use mv::allocation::{AllocationBudget, AllocationRefusal, AllocationReservation};
use std::alloc::Layout;

/// Checked requested shell bytes for capture and one simultaneous installation.
///
/// The two demands coexist. The original field Vec is retained, not reused for
/// prepared entries, and each prepared wrapper owns its original retained Box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::state) struct WorldJournalShellDemand {
    fields: usize,
    capture_bytes: usize,
    installation_bytes: usize,
    total_bytes: usize,
}

trait FieldShells {
    fn retained_layout() -> Layout;
    fn prepared_layout() -> Layout;
}

impl<K: Key, V: Value> FieldShells for Storage<K, V> {
    fn retained_layout() -> Layout {
        Layout::new::<RetainedStorage<K, V>>()
    }

    fn prepared_layout() -> Layout {
        publication::storage_shell_layout::<K, V>()
    }
}

impl<V: Value> FieldShells for Cell<V> {
    fn retained_layout() -> Layout {
        Layout::new::<RetainedCell<V>>()
    }

    fn prepared_layout() -> Layout {
        publication::cell_shell_layout::<V>()
    }
}

impl FieldShells for TriggerSet {
    fn retained_layout() -> Layout {
        Layout::new::<RetainedTriggers>()
    }

    fn prepared_layout() -> Layout {
        publication::triggers_shell_layout()
    }
}

fn add_layout(total: usize, layout: Layout) -> Result<usize, AllocationRefusal> {
    total
        .checked_add(layout.size())
        .ok_or(AllocationRefusal::DemandOverflow)
}

fn field_layouts<T: FieldShells>(_target: fn(&World) -> &T) -> (Layout, Layout) {
    // The accessor exists only for type inference. Calling it would need an
    // actual World; planning deliberately has no such input or construction.
    (T::retained_layout(), T::prepared_layout())
}

impl WorldJournalShellDemand {
    /// Derive every typed wrapper and Vec layout before execution or capture.
    pub(in crate::state) fn plan() -> Result<Self, AllocationRefusal> {
        let mut demand = Self {
            fields: 0,
            capture_bytes: 0,
            installation_bytes: 0,
            total_bytes: 0,
        };
        macro_rules! plan_fields {
            ($demand:ident;
                [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {{
                $( $demand.add_field(field_layouts(|world: &World| &world.$prefix))?; )*
                $( $demand.add_field(field_layouts(|world: &World| &world.$privacy))?; )*
                $( $demand.add_field(field_layouts(|world: &World| &world.$suffix))?; )*
            }};
        }
        with_world_overlay_fields!(plan_fields, demand);
        demand.finish()
    }

    fn add_field(
        &mut self,
        (retained, prepared): (Layout, Layout),
    ) -> Result<(), AllocationRefusal> {
        let fields = self
            .fields
            .checked_add(1)
            .ok_or(AllocationRefusal::DemandOverflow)?;
        let capture_bytes = add_layout(self.capture_bytes, retained)?;
        let installation_bytes = add_layout(self.installation_bytes, prepared)?;
        self.fields = fields;
        self.capture_bytes = capture_bytes;
        self.installation_bytes = installation_bytes;
        Ok(())
    }

    fn finish(mut self) -> Result<Self, AllocationRefusal> {
        let retained_vector = Layout::array::<Box<dyn RetainedWorldField>>(self.fields)
            .map_err(|_| AllocationRefusal::DemandOverflow)?;
        let prepared_vector = publication::field_vector_layout(self.fields)
            .map_err(|_| AllocationRefusal::DemandOverflow)?;
        self.capture_bytes = add_layout(self.capture_bytes, retained_vector)?;
        self.installation_bytes = add_layout(self.installation_bytes, prepared_vector)?;
        self.total_bytes = self
            .capture_bytes
            .checked_add(self.installation_bytes)
            .ok_or(AllocationRefusal::DemandOverflow)?;
        Ok(self)
    }

    /// Number of typed fields enumerated by the sole overlay inventory.
    pub(in crate::state) fn field_count(self) -> usize {
        self.fields
    }

    /// All retained Box pointees and the original field Vec allocation.
    pub(in crate::state) fn capture_bytes(self) -> usize {
        self.capture_bytes
    }

    /// All additional prepared Box pointees and the simultaneous prepared Vec.
    pub(in crate::state) fn installation_bytes(self) -> usize {
        self.installation_bytes
    }

    /// Checked sum of both coexisting demands, not an aggregate allocation Layout.
    pub(in crate::state) fn total_bytes(self) -> usize {
        self.total_bytes
    }

    /// Reserve this finite shell demand before constructing any of its objects.
    ///
    /// Retain this original owner through capture, installation, abort and retry,
    /// releasing it only after the shells are freed. An aggregate carrier planner
    /// should combine `total_bytes()` with its other actual layout demands and
    /// acquire one reservation instead; this helper grants no nested admission.
    pub(in crate::state) fn try_reserve(
        self,
        budget: &AllocationBudget,
    ) -> Result<AllocationReservation, AllocationRefusal> {
        budget.try_reserve_bytes(self.total_bytes)
    }
}

#[cfg(test)]
#[path = "world_journal_resources_tests.rs"]
mod tests;
