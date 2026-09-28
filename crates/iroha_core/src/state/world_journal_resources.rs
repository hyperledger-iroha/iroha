//! Exact requested layouts for World journal wrappers and their field vectors.
//!
//! Planning uses only types from the sole World overlay inventory and allocates
//! no payload or layout collection. It needs neither a World nor its writers.
//! Capture retains one typed Box per field plus a field Vec. Installation adds
//! one prepared Box per field and another Vec while all original boxes and the
//! original Vec remain alive for abort/retry.
//!
//! The exact shared capacity-control layout is also included. This is shell
//! demand only, never complete carrier admission. Nested MV/EBR
//! storage, payloads, events, catalogs, runtime owners, archive custody, allocator
//! bookkeeping and the original allocation-budget control storage require their
//! own resource admission.
//! No encoded-size estimate or inline `size_of` inference funds those owners.

use super::*;
use concread::shared::Shared;
use mv::allocation::{
    AllocationBudget, AllocationCharge, AllocationRefusal, AllocationReservation,
};
use std::alloc::Layout;
use std::sync::atomic::{AtomicBool, Ordering};

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
    control_bytes: usize,
}

trait FieldShells {
    fn retained_layout() -> Layout;
    fn prepared_layout() -> Layout;
}

impl<K: Key, V: Value, M: WorldStorageMode<K, V>> FieldShells for Storage<K, V, M> {
    fn retained_layout() -> Layout {
        Layout::new::<RetainedStorage<K, V, M>>()
    }

    fn prepared_layout() -> Layout {
        publication::storage_shell_layout::<K, V, M>()
    }
}

impl<V: Value, C: Send + Sync + 'static> FieldShells for Cell<V, C> {
    fn retained_layout() -> Layout {
        Layout::new::<RetainedCell<V, C>>()
    }

    fn prepared_layout() -> Layout {
        publication::cell_shell_layout::<V, C>()
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
            control_bytes: ShellCapacity::layout().size(),
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
            .and_then(|bytes| bytes.checked_add(self.control_bytes))
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

    /// Exact shared capacity-control allocation, including inline layout charges.
    pub(in crate::state) fn control_bytes(self) -> usize {
        self.control_bytes
    }

    /// Checked sum of both coexisting demands and their original capacity owner.
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

macro_rules! count_fields {
    (; [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {
        const SHELL_FIELD_COUNT: usize = [
            $(stringify!($prefix),)* $(stringify!($privacy),)* $(stringify!($suffix),)*
        ].len();
    };
}
with_world_overlay_fields!(count_fields);
const SHELL_LAYOUT_COUNT: usize = 2 * SHELL_FIELD_COUNT + 2;
type ShellCapacity = Shared<ShellCharges, AllocationCharge>;

/// One exact inventory of original capture and simultaneous installation capacity.
/// Charges remain reserved across retries. A new installation cannot reuse them
/// until the previous installation's actual Boxes and Vec have been destroyed.
struct ShellCharges {
    installation_live: AtomicBool,
    _layouts: [Option<AllocationCharge>; SHELL_LAYOUT_COUNT],
}

/// Sealed finite capacity acquired before original candidate execution.
///
/// This funds only the inventory-generated World shell layouts and this owner's
/// exact control allocation. It does not fund execution, nested payloads, native
/// locks, events, runtime, decoding or complete carrier restoration/publication.
/// The live retained validator must acquire this alongside its other admission
/// before execution; constructing it after execution supplies no such guarantee.
#[must_use = "retain the original finite shell capacity through capture and publication"]
pub(crate) struct WorldJournalShellReservation {
    capacity: ShellCapacity,
}

impl WorldJournalShellReservation {
    /// Acquire the complete fixed demand before any World or execution writer.
    pub(crate) fn try_reserve(budget: &AllocationBudget) -> Result<Self, AllocationRefusal> {
        let demand = WorldJournalShellDemand::plan()?;
        let mut reservation = demand.try_reserve(budget)?;
        Ok(Self::take_reserved(demand, &mut reservation)
            .expect("the complete shell demand was reserved above"))
    }

    /// Partition already prepaid aggregate credits without reacquiring the pool.
    pub(in crate::state) fn take_reserved(
        demand: WorldJournalShellDemand,
        aggregate: &mut AllocationReservation,
    ) -> Result<Self, mv::allocation::InsufficientReservation> {
        let mut reservation = aggregate.try_partition_bytes(demand.total_bytes())?;
        let control = reservation
            .try_split(ShellCapacity::layout())
            .expect("the exact control layout is included in shell demand");
        let mut layouts = std::array::from_fn(|_| None);
        let mut index = 0;
        macro_rules! charge_fields {
            ($reservation:ident; [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {{
                $(charge_field(&mut $reservation, &mut layouts, &mut index, field_layouts(|world: &World| &world.$prefix));)*
                $(charge_field(&mut $reservation, &mut layouts, &mut index, field_layouts(|world: &World| &world.$privacy));)*
                $(charge_field(&mut $reservation, &mut layouts, &mut index, field_layouts(|world: &World| &world.$suffix));)*
            }};
        }
        with_world_overlay_fields!(charge_fields, reservation);
        for layout in [
            Layout::array::<Box<dyn RetainedWorldField>>(demand.fields)
                .expect("planned capture vector"),
            publication::field_vector_layout(demand.fields).expect("planned prepared vector"),
        ] {
            layouts[index] = Some(
                reservation
                    .try_split(layout)
                    .expect("planned vector capacity"),
            );
            index += 1;
        }
        assert_eq!(index, SHELL_LAYOUT_COUNT);
        assert_eq!(reservation.remaining_bytes(), 0);
        Ok(Self {
            capacity: Shared::new(
                ShellCharges {
                    installation_live: AtomicBool::new(false),
                    _layouts: layouts,
                },
                control,
            ),
        })
    }

    /// A second installation must await destruction of the caller's original
    /// cleanup. Equal budgets or a newly allocated reservation cannot substitute.
    pub(super) fn try_install(&self) -> Result<WorldJournalShellInstallation, ShellsNotRetired> {
        self.capacity
            .installation_live
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| ShellsNotRetired)?;
        Ok(WorldJournalShellInstallation {
            capacity: self.capacity.clone(),
        })
    }

    #[cfg(test)]
    pub(in crate::state) fn for_test() -> Self {
        let demand = WorldJournalShellDemand::plan().expect("fixture shell layouts");
        Self::try_reserve(&AllocationBudget::new(demand.total_bytes()))
            .expect("fixture shell capacity")
    }
}

fn charge_field(
    reservation: &mut AllocationReservation,
    layouts: &mut [Option<AllocationCharge>; SHELL_LAYOUT_COUNT],
    index: &mut usize,
    pair: (Layout, Layout),
) {
    for layout in [pair.0, pair.1] {
        layouts[*index] = Some(
            reservation
                .try_split(layout)
                .expect("planned field shell capacity"),
        );
        *index += 1;
    }
}

/// The previous original installation still owns its physical shells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShellsNotRetired;

/// Moves only into a complete installation or its delayed cleanup, after its
/// Boxes and Vec in field order. Shared capacity cannot refund while either the
/// detached capture or a delayed installation remains alive.
pub(super) struct WorldJournalShellInstallation {
    capacity: ShellCapacity,
}

impl Drop for WorldJournalShellInstallation {
    fn drop(&mut self) {
        self.capacity
            .installation_live
            .store(false, Ordering::Release);
    }
}
