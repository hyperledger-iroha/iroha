//! Joint current/undo admission using the original map and reservation owners.

use super::{
    touched_keys::{AdmittedKeys, KeyInsertion},
    *,
};
use crate::allocation::{AllocationBudget, AllocationRefusal, AllocationReservation};
use concread::bptree::{
    AllocationDemand, ClonePlanning, MapAdmissionError, MapMode, NodeCloning, NodeFunding,
    PlanningError, Prepaid,
};
use std::{alloc::Layout, convert::Infallible};

mod sealed {
    pub trait Sealed {}
}

/// Sealed choice of the actual current, undo and touched-key allocation owners.
///
/// Prepaid storage exposes closed operations only. Unrestricted mutable payload
/// access cannot establish admission for the caller's later allocations.
pub trait StorageMode<K: Key, V: Value>: MapMode + NodeCloning<K, V> + sealed::Sealed {
    /// Original undo-tree mode using the same allocation charge type.
    type Undo: MapMode<Charge = Self::Charge> + NodeCloning<K, Option<V>>;
    /// Original transaction-local ordered key owner.
    type Touches: TouchKeys<K>;
}

/// Ordered borrowed access to one mode's private touched-key owner.
#[doc(hidden)]
pub trait TouchKeys<K>: Default {
    /// Borrowed keys in canonical order, without allocating an iterator.
    type Iter<'a>: DoubleEndedIterator<Item = &'a K> + ExactSizeIterator
    where
        Self: 'a,
        K: 'a;
    /// Borrow the original keys.
    fn keys(&self) -> Self::Iter<'_>;
}

impl<K: Ord> TouchKeys<K> for BTreeSet<K> {
    type Iter<'a>
        = std::collections::btree_set::Iter<'a, K>
    where
        K: 'a;
    fn keys(&self) -> Self::Iter<'_> {
        self.iter()
    }
}
impl<K, C> Default for AdmittedKeys<K, C> {
    fn default() -> Self {
        Self::new()
    }
}
impl<K, C> TouchKeys<K> for AdmittedKeys<K, C> {
    type Iter<'a>
        = std::slice::Iter<'a, K>
    where
        K: 'a,
        C: 'a;
    fn keys(&self) -> Self::Iter<'_> {
        self.as_slice().iter()
    }
}
impl sealed::Sealed for Untracked {}
impl<K: Key, V: Value> StorageMode<K, V> for Untracked {
    type Undo = Untracked;
    type Touches = BTreeSet<K>;
}
impl<P: NodeFunding> sealed::Sealed for Prepaid<P> {}
impl<K: Key, V: Value, P: ClonePlanning<K, V>> StorageMode<K, V> for Prepaid<P> {
    type Undo = Prepaid<UndoPolicy<P>>;
    type Touches = AdmittedKeys<K, P::Charge>;
}

/// Uses the same payload policy for original undo values; absence owns no payload.
#[doc(hidden)]
pub struct UndoPolicy<P>(P);
impl<P: NodeFunding> NodeFunding for UndoPolicy<P> {
    type Charge = P::Charge;
    fn take_node_charge(&mut self, layout: Layout) -> Self::Charge {
        self.0.take_node_charge(layout)
    }
}
impl<K, V, P: NodeCloning<K, V>> NodeCloning<K, Option<V>> for UndoPolicy<P> {
    fn clone_key(&mut self, key: &K) -> K {
        self.0.clone_key(key)
    }
    fn clone_value(&mut self, value: &Option<V>) -> Option<V> {
        value.as_ref().map(|value| self.0.clone_value(value))
    }
}
impl<K, V, P: ClonePlanning<K, V>> ClonePlanning<K, Option<V>> for UndoPolicy<P> {
    fn plan_key(key: &K, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        P::plan_key(key, demand)
    }
    fn plan_value(value: &Option<V>, demand: &mut AllocationDemand) -> Result<(), PlanningError> {
        if let Some(value) = value {
            P::plan_value(value, demand)?;
        }
        Ok(())
    }
}

/// Refusal before a jointly admitted operation changes its original owners.
#[derive(Debug)]
pub enum StorageAdmissionError {
    /// Original physical writer is held; release enclosing writers before waiting.
    Busy(crate::ReleaseWait),
    /// An original writer unwound and requires reconstruction.
    Poisoned,
    /// Original map identity changed.
    Changed,
    /// A complete concrete allocation bound could not be established.
    Planning(PlanningError),
    /// The original finite pool refused the whole demand.
    Capacity(AllocationRefusal),
}
impl From<PlanningError> for StorageAdmissionError {
    fn from(error: PlanningError) -> Self {
        Self::Planning(error)
    }
}
impl From<AllocationRefusal> for StorageAdmissionError {
    fn from(error: AllocationRefusal) -> Self {
        Self::Capacity(error)
    }
}
fn acquisition_error(
    error: MapAdmissionError<StorageAdmissionError>,
    wait: crate::ReleaseWait,
) -> StorageAdmissionError {
    match error {
        MapAdmissionError::Busy if wait.is_poisoned() => StorageAdmissionError::Poisoned,
        MapAdmissionError::Busy => StorageAdmissionError::Busy(wait),
        MapAdmissionError::Poisoned => StorageAdmissionError::Poisoned,
        MapAdmissionError::Changed => StorageAdmissionError::Changed,
        MapAdmissionError::Planning(error) => StorageAdmissionError::Planning(error),
        MapAdmissionError::Refused(error) => error,
    }
}

// A closed map admission can release its raw writer on an ordinary planning or
// capacity refusal before it returns any guard. Those releases must wake peers
// that observed the transient writer. Pure contention/poison acquires nothing
// and must not invent a release or turn a held writer into a busy retry loop.
pub(super) fn acquire_writer<T>(
    notification: &ReleaseNotification,
    acquire: impl FnOnce() -> Result<T, MapAdmissionError<StorageAdmissionError>>,
) -> Result<ReleaseGuard<'_, T>, MapAdmissionError<StorageAdmissionError>> {
    match notification.with_acquisition_unwind_notification(acquire) {
        Ok(writer) => Ok(notification.poisoning_guard(writer)),
        Err(error) => {
            if matches!(
                error,
                MapAdmissionError::Planning(_)
                    | MapAdmissionError::Refused(_)
                    | MapAdmissionError::Changed
            ) {
                drop(notification.guard(()));
            }
            Err(error)
        }
    }
}

fn partition(
    reservation: &mut AllocationReservation,
    demand: AllocationDemand,
) -> AllocationReservation {
    reservation
        .try_partition_bytes(demand.bytes())
        .expect("partition of original complete demand")
}

struct AdmittedWriters<'a, K: Key, V: Value, P: ClonePlanning<K, V>> {
    revert: ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, Option<V>, Prepaid<UndoPolicy<P>>>>,
    blocks: ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, V, Prepaid<P>>>,
}

// The original physical guards outlive every input copy, policy and returned
// payload. A normal second-plan refusal remains a local refusal, not a panic.
fn insert_copy<K: Key, V: Value, P: ClonePlanning<K, V>>(
    writer: &mut BptreeMapWriteTxn<'_, K, V, Prepaid<P>>,
    key: &K,
    value: &V,
    budget: &AllocationBudget,
    provider: &mut impl FnMut(AllocationReservation) -> P,
    notification: &ReleaseNotification,
) -> Result<(), StorageAdmissionError> {
    let current = writer.insertion_demand(key)?;
    let mut copies = AllocationDemand::new();
    P::plan_key(key, &mut copies)?;
    P::plan_value(value, &mut copies)?;
    let mut total = current;
    total.add_demand(copies)?;
    let mut reservation = budget.try_reserve_bytes(total.bytes())?;
    let current_reservation = partition(&mut reservation, current);
    let mut copier = provider(reservation);
    let key = copier.clone_key(key);
    let value = copier.clone_value(value);
    let wait = notification.observe();
    let previous = writer
        .try_insert_admitted(key, value, |actual| {
            if actual != current {
                return Err(StorageAdmissionError::Changed);
            }
            Ok(provider(current_reservation))
        })
        .map_err(|((key, value), error)| {
            drop((key, value));
            acquisition_error(error, wait)
        })?;
    drop(previous);
    drop(copier);
    Ok(())
}

impl<K: Key, V: Value, P: ClonePlanning<K, V>> Storage<K, V, Prepaid<P>> {
    /// Construct both original trees from one reservation of their combined layouts.
    ///
    /// `provider` must move its original reservation into the payload policy,
    /// without acquiring another budget or allocating unadmitted storage.
    /// TODO: admit native mutex and MV identity/notification storage before
    /// claiming complete Storage construction or activating funded State.
    pub fn try_new_with_node_custody(
        budget: &AllocationBudget,
        mut provider: impl FnMut(AllocationReservation) -> P,
    ) -> Result<Self, StorageAdmissionError> {
        let mut revert = None;
        let blocks = BptreeMap::try_new_with_node_custody(|current: AllocationDemand| {
            let mut current_provider = None;
            revert = Some(BptreeMap::try_new_with_node_custody(
                |undo: AllocationDemand| {
                    let mut total = current;
                    total.add_demand(undo)?;
                    let mut reservation = budget.try_reserve_bytes(total.bytes())?;
                    current_provider = Some(provider(partition(&mut reservation, current)));
                    Ok::<_, StorageAdmissionError>(UndoPolicy(provider(reservation)))
                },
            )?);
            Ok::<_, StorageAdmissionError>(current_provider.expect("joint original admission"))
        })?;
        Ok(Self {
            publication: Publication::new(),
            revert_released: ReleaseNotification::default(),
            blocks_released: ReleaseNotification::default(),
            revert: revert.expect("joint original undo tree"),
            blocks,
        })
    }

    // Acquisition and publication use the same undo-before-current order.
    // Clear admission includes the new empty undo leaf; retained admission
    // leaves every original preimage available to replacement or restoration.
    fn try_original_writers(
        &self,
        budget: &AllocationBudget,
        provider: &mut impl FnMut(AllocationReservation) -> P,
        clear_undo: bool,
    ) -> Result<AdmittedWriters<'_, K, V, P>, StorageAdmissionError> {
        let mut blocks = None;
        let undo_wait = self.revert_released.observe();
        let revert = acquire_writer(&self.revert_released, || {
            let admit_pair = |undo: AllocationDemand| {
                let mut undo_provider = None;
                let current_wait = self.blocks_released.observe();
                let writer = acquire_writer(&self.blocks_released, || {
                    self.blocks.try_write_admitted(|current| {
                        let mut total = undo;
                        total.add_demand(current)?;
                        let mut reservation = budget.try_reserve_bytes(total.bytes())?;
                        undo_provider =
                            Some(UndoPolicy(provider(partition(&mut reservation, undo))));
                        Ok::<_, StorageAdmissionError>(provider(reservation))
                    })
                })
                .map_err(|error| acquisition_error(error, current_wait))?;
                blocks = Some(writer);
                Ok(undo_provider.expect("joint original undo admission"))
            };
            if clear_undo {
                self.revert.try_clear_admitted(admit_pair)
            } else {
                self.revert.try_write_admitted(admit_pair)
            }
        })
        .map_err(|error| acquisition_error(error, undo_wait))?;
        Ok(AdmittedWriters {
            revert,
            blocks: blocks.expect("joint original current writer"),
        })
    }

    /// Acquire both original writers and clear the previous undo with one admission.
    ///
    /// The callback factory has the same contract as construction. Enclose this
    /// call and the entire returned block lifetime in the budget's synchronous
    /// refund-notification scope. Refusal never waits or publishes either tree.
    pub fn try_block_admitted(
        &self,
        budget: &AllocationBudget,
        mut provider: impl FnMut(AllocationReservation) -> P,
    ) -> Result<Block<'_, K, V, Prepaid<P>>, StorageAdmissionError> {
        let AdmittedWriters { revert, blocks } =
            self.try_original_writers(budget, &mut provider, true)?;
        drop(provider);
        Ok(Block::new(
            revert,
            blocks,
            false,
            &self.publication,
            self.publication.capture(),
            BlockMode::Ordinary,
        ))
    }

    /// Restore both exact images from an already authenticated snapshot.
    ///
    /// The caller must fence source capture with its publication generation and
    /// authenticate the snapshot schema before restoration. Source roots and
    /// payloads remain borrowed, so any refusal permits retry from the same
    /// original snapshot. No current value or undo tombstone is inferred.
    ///
    /// Initial tree and writer storage, then each map edit and incoming payload
    /// copy, are admitted before allocation using the supplied original policy.
    /// The complete new store remains private until both images and factory
    /// cleanup succeed. Keep restoration and all returned owners inside the
    /// budget's refund-notification discipline. Source decoding, native mutex and
    /// identity storage still require separate admission; this constructor does
    /// not establish an aggregate execution or restore-work bound.
    pub fn try_from_snapshot_with_node_custody<M: StorageMode<K, V>>(
        snapshot: &super::snapshot::Snapshot<'_, K, V, M>,
        budget: &AllocationBudget,
        mut provider: impl FnMut(AllocationReservation) -> P,
    ) -> Result<Self, StorageAdmissionError> {
        let restored = Self::try_new_with_node_custody(budget, &mut provider)?;
        let AdmittedWriters {
            mut revert,
            mut blocks,
        } = restored.try_original_writers(budget, &mut provider, false)?;
        for (key, value) in snapshot.current().iter() {
            insert_copy(
                &mut blocks,
                key,
                value,
                budget,
                &mut provider,
                &restored.blocks_released,
            )?;
        }
        for (key, value) in snapshot.revert_map().iter() {
            insert_copy(
                &mut revert,
                key,
                value,
                budget,
                &mut |reservation| UndoPolicy(provider(reservation)),
                &restored.revert_released,
            )?;
        }
        drop(provider);
        // No external reader can observe this newly constructed target before
        // both original writers have committed. Error/unwind drops it entirely.
        blocks.release_with(|writer| writer.commit());
        revert.release_with(|writer| writer.commit());
        Ok(restored)
    }

    /// Acquire a replacement block, restoring the last block's original preimages.
    ///
    /// The original undo writer is acquired before the current writer. Their
    /// shells are admitted together; each restored entry then admits its complete
    /// map edit and any incoming payload copies before allocating. Undo is cleared
    /// only after every preimage has been restored in the private current tree.
    /// A refusal drops both private writers without publishing either tree, even
    /// when an earlier preimage was already restored. This is per-edit admission,
    /// not one aggregate reservation for the complete replacement.
    ///
    /// `provider` must consume only the supplied original reservation. All copies
    /// use its explicit payload policy, including keys and values retained by the
    /// committed undo generation. Keep this call and the complete returned block
    /// lifetime inside the budget's synchronous refund-notification scope. Never
    /// await capacity while holding the returned block. A panic while these
    /// physical writers remain held aborts them and poisons their original locks.
    /// No replacement block is returned if execution or factory cleanup unwinds.
    pub fn try_block_and_revert_admitted(
        &self,
        budget: &AllocationBudget,
        mut provider: impl FnMut(AllocationReservation) -> P,
    ) -> Result<Block<'_, K, V, Prepaid<P>>, StorageAdmissionError> {
        let AdmittedWriters {
            mut revert,
            mut blocks,
        } = self.try_original_writers(budget, &mut provider, false)?;
        let undo_wait = self.revert_released.observe();
        let predecessor = self.publication.capture();

        // Borrow the original held undo root throughout restoration. A private
        // current edit cannot invalidate these keys or nested preimage owners.
        for (key, before) in revert.iter() {
            match before {
                Some(value) => {
                    insert_copy(
                        &mut blocks,
                        key,
                        value,
                        budget,
                        &mut provider,
                        &self.blocks_released,
                    )?;
                }
                None => {
                    let current_wait = self.blocks_released.observe();
                    let previous = blocks
                        .try_remove_admitted(key, |demand| {
                            Ok::<_, StorageAdmissionError>(provider(
                                budget.try_reserve_bytes(demand.bytes())?,
                            ))
                        })
                        .map_err(|error| acquisition_error(error, current_wait))?;
                    drop(previous);
                }
            }
        }
        revert
            .try_clear_admitted(|demand| {
                Ok::<_, StorageAdmissionError>(UndoPolicy(provider(
                    budget.try_reserve_bytes(demand.bytes())?,
                )))
            })
            .map_err(|error| acquisition_error(error, undo_wait))?;
        // User-owned factory cleanup must finish while both writers still abort
        // on unwind; no partially restored block may escape a failed destructor.
        drop(provider);
        Ok(Block::new(
            revert,
            blocks,
            true,
            &self.publication,
            predecessor,
            BlockMode::Replace,
        ))
    }
}

struct EditPlan {
    current: AllocationDemand,
    undo: Option<AllocationDemand>,
    touch: Option<KeyInsertion>,
    copies: AllocationDemand,
    total: AllocationDemand,
}

// These original component owners remain alive until the current edit finishes.
// The transaction's failed flag stays armed throughout their destruction.
struct PreparedEdit<P> {
    demand: AllocationDemand,
    current: AllocationReservation,
    copier: P,
    remainder: AllocationReservation,
}

impl<K: Key, V: Value, P: ClonePlanning<K, V>> Transaction<'_, K, V, Prepaid<P>> {
    fn plan_edit(&self, key: &K, current: AllocationDemand) -> Result<EditPlan, PlanningError> {
        self.assert_operable();
        let undo = self.revert.as_ref().expect("live transaction undo root");
        let undo = if undo.get(key).is_none() {
            Some(undo.insertion_demand(key)?)
        } else {
            None
        };
        let touch = self.touched.insertion(key)?;
        let mut copies = AllocationDemand::new();
        if undo.is_some() {
            P::plan_key(key, &mut copies)?;
            if let Some(before) = self.current().get(key) {
                P::plan_value(before, &mut copies)?;
            }
        }
        if let Some(touch) = &touch {
            P::plan_key(key, &mut copies)?;
            if let Some(layout) = touch.layout() {
                copies.add_layout(layout)?;
            }
        }
        let mut total = current;
        total.add_demand(copies)?;
        if let Some(undo) = undo {
            total.add_demand(undo)?;
        }
        Ok(EditPlan {
            current,
            undo,
            touch,
            copies,
            total,
        })
    }

    /// Observe the complete current, first-undo and touched-key insertion demand.
    /// This allocation-free observation grants no authority; the closed edit
    /// replans under the same exclusive transaction borrow before reserving.
    pub fn insertion_demand(&self, key: &K) -> Result<AllocationDemand, PlanningError> {
        self.assert_operable();
        self.plan_edit(key, self.current().insertion_demand(key)?)
            .map(|plan| plan.total)
    }

    /// Observe the complete current, first-undo and touched-key removal demand.
    /// A missing key still needs its first absence and touch witness. Replanning
    /// under the same exclusive transaction borrow precedes actual admission.
    pub fn removal_demand(&self, key: &K) -> Result<AllocationDemand, PlanningError> {
        self.assert_operable();
        self.plan_edit(key, self.current().removal_demand(key)?)
            .map(|plan| plan.total)
    }

    fn prepare_edit(
        &mut self,
        key: &K,
        plan: EditPlan,
        budget: &AllocationBudget,
        provider: &mut impl FnMut(AllocationReservation) -> P,
    ) -> Result<PreparedEdit<P>, StorageAdmissionError> {
        let EditPlan {
            current,
            undo,
            touch,
            copies,
            total,
        } = plan;
        let mut reservation = budget.try_reserve_bytes(total.bytes())?;
        self.failed = true;
        let current_reservation = partition(&mut reservation, current);
        let undo_reservation = undo.map(|demand| partition(&mut reservation, demand));
        let mut copier = provider(partition(&mut reservation, copies));
        if let Some(touch) = touch {
            let touched_key = copier.clone_key(key);
            let charge = touch.layout().map(|layout| copier.take_node_charge(layout));
            self.touched.insert(touch, touched_key, charge);
        }
        if let (Some(demand), Some(reservation)) = (undo, undo_reservation) {
            let undo_key = copier.clone_key(key);
            let before = self
                .blocks
                .as_ref()
                .expect("live transaction current root")
                .get(key)
                .map(|value| copier.clone_value(value));
            self.revert
                .as_mut()
                .expect("live transaction undo root")
                .try_insert_admitted(undo_key, before, |actual| {
                    assert_eq!(actual, demand, "same exclusively held undo plan");
                    Ok::<_, Infallible>(UndoPolicy(provider(reservation)))
                })
                .unwrap_or_else(|_| panic!("original fully admitted undo insertion changed"));
        }
        Ok(PreparedEdit {
            demand: current,
            current: current_reservation,
            copier,
            remainder: reservation,
        })
    }

    /// Insert after reserving current, first undo, nested copies and touch storage together.
    ///
    /// Refusal returns the original inputs before allocating or mutating. Incoming
    /// key/value storage is already owned; its funding belongs to the caller.
    /// `provider` consumes only its original component reservation. On unwind the
    /// transaction cannot apply; dropping it restores both original parent roots.
    /// Release the enclosing block before suspending on a capacity observation.
    pub fn try_insert_admitted(
        &mut self,
        key: K,
        value: V,
        budget: &AllocationBudget,
        mut provider: impl FnMut(AllocationReservation) -> P,
    ) -> Result<Option<V>, ((K, V), StorageAdmissionError)> {
        self.assert_operable();
        let plan = match self
            .current()
            .insertion_demand(&key)
            .and_then(|current| self.plan_edit(&key, current))
        {
            Ok(plan) => plan,
            Err(error) => return Err(((key, value), error.into())),
        };
        let PreparedEdit {
            demand,
            current,
            copier,
            remainder,
        } = match self.prepare_edit(&key, plan, budget, &mut provider) {
            Ok(prepared) => prepared,
            Err(error) => return Err(((key, value), error)),
        };
        let previous = self
            .blocks
            .as_mut()
            .expect("live transaction current root")
            .try_insert_admitted(key, value, |actual| {
                assert_eq!(actual, demand, "same exclusively held current plan");
                Ok::<_, Infallible>(provider(current))
            })
            .unwrap_or_else(|_| panic!("original fully admitted current insertion changed"));
        // Keep failure armed across arbitrary policy and unused-credit cleanup.
        drop(copier);
        drop(remainder);
        drop(provider);
        self.dirty = true;
        self.failed = false;
        Ok(previous)
    }

    /// Remove after reserving the current edit, first undo and touch witnesses together.
    ///
    /// The borrowed query remains owned by the caller. A missing key still records
    /// an explicit touch and first absence, without making a clean block dirty.
    /// Refusal precedes all copies and mutations. A panic during execution or
    /// cleanup forbids applying this transaction; dropping it restores its parent.
    /// `provider` consumes only its original reservation. Keep the edit and all
    /// cleanup inside the budget's refund-notification scope, and release the
    /// enclosing block before waiting on refused capacity.
    pub fn try_remove_admitted(
        &mut self,
        key: &K,
        budget: &AllocationBudget,
        mut provider: impl FnMut(AllocationReservation) -> P,
    ) -> Result<Option<V>, StorageAdmissionError> {
        self.assert_operable();
        let plan = self.plan_edit(key, self.current().removal_demand(key)?)?;
        let PreparedEdit {
            demand,
            current,
            copier,
            remainder,
        } = self.prepare_edit(key, plan, budget, &mut provider)?;
        let previous = self
            .blocks
            .as_mut()
            .expect("live transaction current root")
            .try_remove_admitted(key, |actual| {
                assert_eq!(actual, demand, "same exclusively held current removal plan");
                Ok::<_, Infallible>(provider(current))
            })
            .unwrap_or_else(|_| panic!("original fully admitted current removal changed"));
        drop(copier);
        drop(remainder);
        drop(provider);
        self.dirty |= previous.is_some();
        self.failed = false;
        Ok(previous)
    }
}
