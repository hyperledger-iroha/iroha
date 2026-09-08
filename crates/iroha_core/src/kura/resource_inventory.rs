//! Fixed-family resource accounting shared by Kura's independent index owners.
//!
//! Snapshots perform no I/O and never initialize an owner. Unregistered,
//! interrupted, overflowing, or concurrently mutating inventories are explicitly
//! unavailable. Counts describe represented index entries, never estimated RSS.

use parking_lot::Mutex;

/// Closed first-release inventory; adding an owner changes the inventory schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum Family {
    /// Materialized canonical hash and reverse-height associations.
    ResidentCanonical,
    /// All transaction and Kaigi index associations and inventory memberships.
    ResidentTransaction,
    /// Merge frame, epoch, and latest-route associations.
    ResidentMerge,
    /// Sparse merge-carrier forward and reverse associations.
    ResidentCarrier,
    /// Exact block-identity records and each nested authenticated peer observation.
    ResidentReplica,
    /// Ordinary finality-LRU records and associations in every live startup allocation.
    /// Arc clones count their shared allocation once, until its last reader exits.
    ResidentVerification,
    /// Lane/config entries, both frontier attestations and capacity reservations.
    /// Includes nested stable, transient, execution, outstanding and terminal memberships.
    ResidentFrontier,
    /// Actual pipeline/FASTPQ queue records, including requeued retries.
    /// Drained local work and payload internals are RSS, not queue associations.
    ResidentQueue,
    /// Canonical fixed-width block index slots.
    CanonicalIndex,
    /// Canonical fixed-width hash journal slots.
    CanonicalHashes,
    /// Pipeline recovery sidecar slots.
    PipelineIndex,
    /// Lane payload ownership sidecar slots.
    OwnershipIndex,
    /// Certified lane block sidecar slots.
    CertifiedIndex,
    /// Lane execution input sidecar slots.
    ExecutionInputIndex,
    /// Lane execution preflight sidecar slots.
    ExecutionPreflightIndex,
    /// Lane application receipt sidecar slots.
    ApplicationReceiptIndex,
    /// Autonomous merge source bundle sidecar slots.
    MergeBundleIndex,
    /// Canonical autonomous replica sidecar slots.
    CanonicalReplicaIndex,
    /// Standalone sparse merge-carrier index records.
    MergeCarrierRecord,
    /// Standalone Native AMX latest-receipt index records.
    NativeLatestRecord,
    /// Query status and projection checkpoint marker records.
    QueryMarkerRecords,
    /// Other declared evidence-key, replay-claim and recovery index records.
    EvidenceKeyRecords,
    /// Actual logical file bytes over the complete declared Kura storage scope.
    StorageBytes,
}

/// Number of closed first-release component families.
pub(crate) const FAMILY_COUNT: usize = Family::StorageBytes as usize + 1;
const ALL_MASK: u32 = (1_u32 << FAMILY_COUNT) - 1;
/// Every required family in the fixed serialization and observation order.
pub(crate) const ALL_FAMILIES: [Family; FAMILY_COUNT] = [
    Family::ResidentCanonical,
    Family::ResidentTransaction,
    Family::ResidentMerge,
    Family::ResidentCarrier,
    Family::ResidentReplica,
    Family::ResidentVerification,
    Family::ResidentFrontier,
    Family::ResidentQueue,
    Family::CanonicalIndex,
    Family::CanonicalHashes,
    Family::PipelineIndex,
    Family::OwnershipIndex,
    Family::CertifiedIndex,
    Family::ExecutionInputIndex,
    Family::ExecutionPreflightIndex,
    Family::ApplicationReceiptIndex,
    Family::MergeBundleIndex,
    Family::CanonicalReplicaIndex,
    Family::MergeCarrierRecord,
    Family::NativeLatestRecord,
    Family::QueryMarkerRecords,
    Family::EvidenceKeyRecords,
    Family::StorageBytes,
];

impl Family {
    /// One bounded bit representing this owner.
    pub(crate) const fn mask(self) -> u32 {
        1 << self as usize
    }
}

/// Exact counts in distinct physical representations and actual byte units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Usage {
    /// Stored resident lookup associations, including separate representations.
    pub(crate) resident_associations: u64,
    /// Physical persisted index slots or standalone index records.
    pub(crate) persisted_entries: u64,
    /// Actual stable index-file logical lengths, including format headers.
    pub(crate) index_bytes: u64,
    /// Actual temporary index-file logical lengths while they remain present.
    pub(crate) temporary_index_bytes: u64,
    /// Actual logical bytes in the declared complete storage scope.
    pub(crate) storage_bytes: u64,
}

impl Usage {
    /// Exact represented-entry reduction used by G-SCALE.
    pub(crate) fn represented_entries(self) -> Result<u64, Unavailable> {
        self.resident_associations
            .checked_add(self.persisted_entries)
            .ok_or(Unavailable::Arithmetic)
    }

    /// Checked addition; invalid counts never saturate into plausible evidence.
    pub(crate) fn checked_add(self, other: Self) -> Result<Self, Unavailable> {
        self.zip(other, u64::checked_add)
    }

    fn checked_sub(self, other: Self) -> Result<Self, Unavailable> {
        self.zip(other, u64::checked_sub)
    }

    fn zip(self, other: Self, operation: fn(u64, u64) -> Option<u64>) -> Result<Self, Unavailable> {
        let combine = |left, right| operation(left, right).ok_or(Unavailable::Arithmetic);
        Ok(Self {
            resident_associations: combine(
                self.resident_associations,
                other.resident_associations,
            )?,
            persisted_entries: combine(self.persisted_entries, other.persisted_entries)?,
            index_bytes: combine(self.index_bytes, other.index_bytes)?,
            temporary_index_bytes: combine(
                self.temporary_index_bytes,
                other.temporary_index_bytes,
            )?,
            storage_bytes: combine(self.storage_bytes, other.storage_bytes)?,
        })
    }
}

/// Fixed reason vocabulary; no paths, keys, or remote error text is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unavailable {
    /// An owner has not completed exact initialization.
    Unregistered,
    /// A required mutation or registry lock is active.
    Busy,
    /// A mutation failed, was abandoned, or could not be accounted exactly.
    Interrupted,
    /// Count, byte, mutation-count or generation arithmetic failed.
    Arithmetic,
    /// Initialization was superseded by a concurrent generation.
    GenerationChanged,
    /// An owner supplied an invalid family set or publication set.
    OwnerMismatch,
    /// An index format, filesystem identity, or inventory bound was invalid.
    InvalidInventory,
}

#[derive(Clone, Copy, Debug, Default)]
struct Component {
    registered: bool,
    usage: Usage,
    mutations: u32,
    fault: Option<Unavailable>,
}

#[derive(Debug)]
struct State {
    generation: u64,
    components: [Component; FAMILY_COUNT],
    high_water: Usage,
    fault_count: u64,
    fatal: Option<Unavailable>,
}

/// A coherent complete fixed-size resource observation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Snapshot {
    /// Generation of this complete observation.
    pub(crate) generation: u64,
    /// Exact component vector in fixed enum order.
    pub(crate) components: [Usage; FAMILY_COUNT],
    /// Checked aggregate with separate entry and byte subtotals.
    pub(crate) total: Usage,
    /// Componentwise high-water values of complete published aggregate states.
    /// Mutating intervals are explicitly unavailable and are not covered by this value.
    pub(crate) observed_high_water: Usage,
    /// Sticky number of failed accounting publications in this process instance.
    pub(crate) fault_count: u64,
}

/// Fixed-memory inventory. Its read path does no filesystem work or waiting.
#[derive(Debug)]
pub(crate) struct Inventory {
    state: Mutex<State>,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                generation: 0,
                components: [Component::default(); FAMILY_COUNT],
                high_water: Usage::default(),
                fault_count: 0,
                fatal: None,
            }),
        }
    }
}

impl Inventory {
    fn advance(state: &mut State) -> Result<(), Unavailable> {
        match state.generation.checked_add(1) {
            Some(next) => {
                state.generation = next;
                Ok(())
            }
            None => {
                state.fatal = Some(Unavailable::Arithmetic);
                Err(Unavailable::Arithmetic)
            }
        }
    }

    fn fault(state: &mut State, mask: u32, reason: Unavailable) {
        for (index, component) in state.components.iter_mut().enumerate() {
            if mask & (1 << index) != 0 {
                component.fault.get_or_insert(reason);
            }
        }
        match state.fault_count.checked_add(1) {
            Some(count) => state.fault_count = count,
            None => state.fatal = Some(Unavailable::Arithmetic),
        }
    }

    /// Mark an exact owner set unavailable after a detected accounting failure.
    pub(crate) fn invalidate(&self, mask: u32, reason: Unavailable) {
        let mut state = self.state.lock();
        Self::fault(&mut state, mask & ALL_MASK, reason);
        let _ = Self::advance(&mut state);
    }

    /// Capture the epoch before bounded startup/reconciliation inventory work.
    pub(crate) fn reconciliation_generation(&self) -> Result<u64, Unavailable> {
        let state = self.state.try_lock().ok_or(Unavailable::Busy)?;
        if let Some(error) = state.fatal {
            return Err(error);
        }
        if state
            .components
            .iter()
            .any(|component| component.mutations != 0)
        {
            return Err(Unavailable::Busy);
        }
        Ok(state.generation)
    }

    /// Register exactly audited families only if no writer crossed the inventory.
    /// Other families remain unregistered; this cannot publish partial completeness.
    pub(crate) fn initialize(
        &self,
        generation: u64,
        values: &[(Family, Usage)],
    ) -> Result<(), Unavailable> {
        let mask = publication_mask(values.iter().map(|(family, _)| *family))?;
        let mut state = self.state.lock();
        if let Some(error) = state.fatal {
            return Err(error);
        }
        if state.generation != generation {
            return Err(Unavailable::GenerationChanged);
        }
        if state
            .components
            .iter()
            .any(|component| component.mutations != 0)
        {
            return Err(Unavailable::Busy);
        }
        let mut candidate = state.components;
        let checked = (|| {
            for &(family, usage) in values {
                usage.represented_entries()?;
                candidate[family as usize] = Component {
                    registered: true,
                    usage,
                    mutations: 0,
                    fault: None,
                };
            }
            aggregate(&candidate)
        })();
        if let Err(reason) = checked {
            Self::fault(&mut state, mask, reason);
            let _ = Self::advance(&mut state);
            return Err(reason);
        }
        Self::advance(&mut state)?;
        state.components = candidate;
        update_high_water(&mut state);
        debug_assert_ne!(mask, 0);
        Ok(())
    }

    /// Begin a scoped physical mutation before reading its exact before-state.
    /// The owning storage lock must serialize overlapping physical paths.
    pub(crate) fn begin(&self, mask: u32) -> Result<Mutation<'_>, Unavailable> {
        if mask == 0 || mask & !ALL_MASK != 0 {
            return Err(Unavailable::OwnerMismatch);
        }
        let mut state = self.state.lock();
        if let Some(error) = state.fatal {
            return Err(error);
        }
        let mut candidate = state.components;
        for (index, component) in candidate.iter_mut().enumerate() {
            if mask & (1 << index) != 0 {
                let Some(next) = component.mutations.checked_add(1) else {
                    Self::fault(&mut state, mask, Unavailable::Arithmetic);
                    let _ = Self::advance(&mut state);
                    return Err(Unavailable::Arithmetic);
                };
                component.mutations = next;
            }
        }
        Self::advance(&mut state)?;
        state.components = candidate;
        Ok(Mutation {
            inventory: self,
            mask,
            finished: false,
        })
    }

    /// Observe one registered component in tests without qualifying any other owner.
    #[cfg(test)]
    pub(crate) fn component_usage_for_tests(&self, family: Family) -> Result<Usage, Unavailable> {
        let state = self.state.try_lock().ok_or(Unavailable::Busy)?;
        if let Some(error) = state.fatal {
            return Err(error);
        }
        let component = &state.components[family as usize];
        if let Some(error) = component.fault {
            return Err(error);
        }
        if component.mutations != 0 {
            return Err(Unavailable::Busy);
        }
        if !component.registered {
            return Err(Unavailable::Unregistered);
        }
        Ok(component.usage)
    }

    /// Return one complete coherent snapshot immediately, or explicit unavailability.
    pub(crate) fn try_snapshot(&self) -> Result<Snapshot, Unavailable> {
        let state = self.state.try_lock().ok_or(Unavailable::Busy)?;
        if let Some(error) = state.fatal {
            return Err(error);
        }
        for family in ALL_FAMILIES {
            let component = &state.components[family as usize];
            if let Some(error) = component.fault {
                return Err(error);
            }
            if component.mutations != 0 {
                return Err(Unavailable::Busy);
            }
            if !component.registered {
                return Err(Unavailable::Unregistered);
            }
        }
        Ok(Snapshot {
            generation: state.generation,
            components: state.components.map(|component| component.usage),
            total: aggregate(&state.components)?,
            observed_high_water: state.high_water,
            fault_count: state.fault_count,
        })
    }
}

fn publication_mask(families: impl IntoIterator<Item = Family>) -> Result<u32, Unavailable> {
    let mut mask = 0;
    for family in families {
        if mask & family.mask() != 0 {
            return Err(Unavailable::OwnerMismatch);
        }
        mask |= family.mask();
    }
    if mask == 0 {
        Err(Unavailable::OwnerMismatch)
    } else {
        Ok(mask)
    }
}

fn aggregate(components: &[Component; FAMILY_COUNT]) -> Result<Usage, Unavailable> {
    let total = components
        .iter()
        .try_fold(Usage::default(), |total, component| {
            total.checked_add(component.usage)
        })?;
    total.represented_entries()?;
    Ok(total)
}

fn update_high_water(state: &mut State) {
    if state.components.iter().any(|component| {
        !component.registered || component.mutations != 0 || component.fault.is_some()
    }) {
        return;
    }
    if let Ok(total) = aggregate(&state.components) {
        let high = &mut state.high_water;
        high.resident_associations = high.resident_associations.max(total.resident_associations);
        high.persisted_entries = high.persisted_entries.max(total.persisted_entries);
        high.index_bytes = high.index_bytes.max(total.index_bytes);
        high.temporary_index_bytes = high.temporary_index_bytes.max(total.temporary_index_bytes);
        high.storage_bytes = high.storage_bytes.max(total.storage_bytes);
    }
}

/// Scoped move-only publication ownership; abandoned writes invalidate their families.
#[must_use]
pub(crate) struct Mutation<'a> {
    inventory: &'a Inventory,
    mask: u32,
    finished: bool,
}

impl Mutation<'_> {
    /// Publish exact before/after deltas for every family claimed by this mutation.
    /// Both snapshots must cover the same physical paths under their owner lock.
    pub(crate) fn publish(mut self, values: &[(Family, Usage, Usage)]) -> Result<(), Unavailable> {
        let mask = publication_mask(values.iter().map(|(family, _, _)| *family))?;
        if mask != self.mask {
            return Err(Unavailable::OwnerMismatch);
        }
        let mut state = self.inventory.state.lock();
        let mut candidate = state.components;
        let result = (|| {
            for &(family, before, after) in values {
                let component = &mut candidate[family as usize];
                // Before initialization, writes advance generation but cannot create
                // an apparent baseline. The eventual full inventory owns that sum.
                if component.registered {
                    component.usage = component.usage.checked_sub(before)?.checked_add(after)?;
                }
                component.mutations = component
                    .mutations
                    .checked_sub(1)
                    .ok_or(Unavailable::Arithmetic)?;
            }
            aggregate(&candidate)?;
            Inventory::advance(&mut state)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                state.components = candidate;
                self.finished = true;
                update_high_water(&mut state);
                Ok(())
            }
            Err(error) => {
                Inventory::fault(&mut state, self.mask, error);
                Err(error)
            }
        }
    }
}

impl Drop for Mutation<'_> {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let mut state = self.inventory.state.lock();
        for (index, component) in state.components.iter_mut().enumerate() {
            if self.mask & (1 << index) != 0 {
                match component.mutations.checked_sub(1) {
                    Some(count) => component.mutations = count,
                    None => component.fault = Some(Unavailable::Arithmetic),
                }
            }
        }
        Inventory::fault(&mut state, self.mask, Unavailable::Interrupted);
        let _ = Inventory::advance(&mut state);
    }
}

#[cfg(test)]
#[path = "resource_inventory/tests.rs"]
mod tests;
