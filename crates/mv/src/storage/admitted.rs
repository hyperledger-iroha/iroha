//! Finite insertion and removal in the original MV current/undo pair.
//!
//! This admits original node, cursor, reader, tracking and copied payload owners.
//! Borrowed iteration retains its traversal state inline without allocating.
//! Publication/release control objects remain separately funded.
//! Transactions additionally admit their ordered local touch owners.
//! Replacement and snapshot restoration admit each edit and its incoming copies.
//! Capture/detachment and mutable access remain unavailable until funded.

use super::*;
use crate::{
    ReleaseWait,
    allocation::{AllocationBudget, AllocationCharge, AllocationRefusal, AllocationReservation},
};
use concread::bptree::{
    AllocationDemand, ClonePlanning, MapAdmissionError, NodeFunding, PairInsertError,
    PairRemoveError, PlanningError, Prepaid,
};

/// Constructs a payload policy from the original finite MV reservation.
///
/// Implementations must retain that same reservation, return its reference from
/// `admission`, and fund every planned node and payload copy from it. The adapter
/// verifies actual pool identity and the complete remaining demand before the
/// policy may mutate either map. Node charges are the real move-only allocation
/// credits, never synthetic witnesses. Payload cloning additionally implements
/// Concread's exact `ClonePlanning` contract for current and undo values.
pub trait AdmittedStoragePolicy: NodeFunding<Charge = AllocationCharge> {
    /// Retain the original reservation without spending or replacing it.
    fn from_admission(reservation: AllocationReservation) -> Self;
    /// Borrow the original remaining prepaid owner.
    fn admission(&self) -> &AllocationReservation;
}

/// Which original physical writer prevented admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageRole {
    /// Current values.
    Current,
    /// First block preimages.
    Undo,
}

/// A local refusal before publishing an admitted block.
#[derive(Debug)]
pub enum AdmittedStorageError {
    /// The original writer is held. The observation grants no future authority.
    Busy {
        /// Original writer role.
        role: StorageRole,
        /// Release observation captured before the failed acquisition.
        release: ReleaseWait,
    },
    /// An earlier unwind made the original writer unavailable for reuse.
    Poisoned {
        /// Original writer role.
        role: StorageRole,
    },
    /// Exact layout or generation planning refused before allocation.
    Planning(PlanningError),
    /// Replanning after an incoming payload copy changed the admitted demand.
    Changed,
    /// The original finite pool cannot admit the complete demand.
    Allocation(AllocationRefusal),
    /// A policy returned a reservation from another pool.
    PolicyIdentity,
    /// A policy did not retain the full original requested demand.
    PolicyDemand {
        /// Bytes passed to the policy constructor.
        expected_bytes: usize,
        /// Bytes retained by the returned policy.
        remaining_bytes: usize,
    },
}

/// Either opening or edit admission failed or the caller aborted its block.
#[derive(Debug)]
pub enum AdmittedBlockError<E> {
    /// Original writer or allocation admission refused.
    Admission(AdmittedStorageError),
    /// The callback abandoned its private block without publication.
    Callback(E),
}

fn policy<P: AdmittedStoragePolicy>(
    budget: &AllocationBudget,
    reservation: AllocationReservation,
    demand: AllocationDemand,
) -> Result<P, AdmittedStorageError> {
    let expected_bytes = demand.bytes();
    if reservation.remaining_bytes() != expected_bytes {
        return Err(AdmittedStorageError::PolicyDemand {
            expected_bytes,
            remaining_bytes: reservation.remaining_bytes(),
        });
    }
    let provider = P::from_admission(reservation);
    if !provider.admission().belongs_to(budget) {
        return Err(AdmittedStorageError::PolicyIdentity);
    }
    if provider.admission().remaining_bytes() != expected_bytes {
        return Err(AdmittedStorageError::PolicyDemand {
            expected_bytes,
            remaining_bytes: provider.admission().remaining_bytes(),
        });
    }
    Ok(provider)
}

pub(super) fn admit<P: AdmittedStoragePolicy>(
    budget: &AllocationBudget,
    demand: AllocationDemand,
) -> Result<P, AdmittedStorageError> {
    let reservation = budget
        .try_reserve_bytes(demand.bytes())
        .map_err(AdmittedStorageError::Allocation)?;
    policy(budget, reservation, demand)
}

// Canonical Concread layout plans are summed once. Both move-only parts retain
// the original pool; partitioning does not acquire or refund any pool credits.
fn reserve_pair(
    budget: &AllocationBudget,
    current: AllocationDemand,
    undo: AllocationDemand,
) -> Result<(AllocationReservation, AllocationReservation), AdmittedStorageError> {
    let total =
        current
            .bytes()
            .checked_add(undo.bytes())
            .ok_or(AdmittedStorageError::Allocation(
                AllocationRefusal::DemandOverflow,
            ))?;
    let mut original = budget
        .try_reserve_bytes(total)
        .map_err(AdmittedStorageError::Allocation)?;
    let current = original
        .try_partition_bytes(current.bytes())
        .expect("part of the same checked complete demand");
    Ok((current, original))
}

fn writer_error(
    error: MapAdmissionError<AdmittedStorageError>,
    role: StorageRole,
    release: ReleaseWait,
) -> AdmittedStorageError {
    match error {
        MapAdmissionError::Busy if release.is_poisoned() => AdmittedStorageError::Poisoned { role },
        MapAdmissionError::Busy => AdmittedStorageError::Busy { role, release },
        MapAdmissionError::Poisoned => AdmittedStorageError::Poisoned { role },
        MapAdmissionError::Planning(error) => AdmittedStorageError::Planning(error),
        MapAdmissionError::Refused(error) => error,
        MapAdmissionError::Changed => unreachable!("new original writer has no detached input"),
    }
}

// An ordinary refusal can release a raw writer before any guard is returned.
// Signal only those releases; contention and poison never acquired authority.
pub(super) fn acquire_writer<T>(
    notification: &ReleaseNotification,
    acquire: impl FnOnce() -> Result<T, MapAdmissionError<AdmittedStorageError>>,
) -> Result<ReleaseGuard<'_, T>, MapAdmissionError<AdmittedStorageError>> {
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

struct AdmittedWriters<'a, K: Key, V: Value, P>
where
    P: ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    revert: ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, Option<V>, Prepaid<P>>>,
    blocks: ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, V, Prepaid<P>>>,
}

fn edit_error(error: MapAdmissionError<AdmittedStorageError>) -> AdmittedStorageError {
    match error {
        MapAdmissionError::Planning(error) => AdmittedStorageError::Planning(error),
        MapAdmissionError::Refused(error) => error,
        MapAdmissionError::Changed => AdmittedStorageError::Changed,
        MapAdmissionError::Busy | MapAdmissionError::Poisoned => {
            unreachable!("an acquired writer never reacquires its physical lock")
        }
    }
}

// Retained source entries are borrowed, so their copies and the destination edit
// share one admission. Both physical writers outlive all temporary payloads.
fn insert_copy<K: Key, V: Value, P: AdmittedStoragePolicy + ClonePlanning<K, V>>(
    writer: &mut BptreeMapWriteTxn<'_, K, V, Prepaid<P>>,
    key: &K,
    value: &V,
    budget: &AllocationBudget,
) -> Result<(), AdmittedStorageError> {
    let current = writer
        .insertion_demand(key)
        .map_err(AdmittedStorageError::Planning)?;
    let mut copies = AllocationDemand::new();
    P::plan_key(key, &mut copies).map_err(AdmittedStorageError::Planning)?;
    P::plan_value(value, &mut copies).map_err(AdmittedStorageError::Planning)?;
    let (current_reservation, copy_reservation) = reserve_pair(budget, current, copies)?;
    let mut copier = policy::<P>(budget, copy_reservation, copies)?;
    let key = copier.clone_key(key);
    let value = copier.clone_value(value);
    let previous = writer
        .try_insert_admitted(key, value, |actual| {
            if actual != current {
                return Err(AdmittedStorageError::Changed);
            }
            policy::<P>(budget, current_reservation, actual)
        })
        .map_err(|((key, value), error)| {
            drop((key, value));
            edit_error(error)
        })?;
    drop(previous);
    drop(copier);
    Ok(())
}

impl<K, V, P> Storage<K, V, Prepaid<P>>
where
    K: Key,
    V: Value,
    P: AdmittedStoragePolicy + ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    /// Construct the same MV storage with one original finite allocation pool.
    ///
    /// A single checked admission covers both maps' real initial node/root/reader
    /// layouts. No empty-map substitute or untracked node path is used. Native
    /// mutex, release notification and publication identity control allocations
    /// are explicit remaining scope, not charged by this node-custody admission.
    pub fn try_new_admitted(budget: AllocationBudget) -> Result<Self, AdmittedStorageError> {
        budget.with_deferred_refund_notifications(|| {
            let current = BptreeMap::<K, V, Prepaid<P>>::node_custody_allocation_demand()
                .map_err(AdmittedStorageError::Planning)?;
            let undo = BptreeMap::<K, Option<V>, Prepaid<P>>::node_custody_allocation_demand()
                .map_err(AdmittedStorageError::Planning)?;
            let (current, undo) = reserve_pair(&budget, current, undo)?;
            let revert =
                BptreeMap::try_new_with_node_custody(|demand| policy::<P>(&budget, undo, demand))?;
            let blocks = BptreeMap::try_new_with_node_custody(|demand| {
                policy::<P>(&budget, current, demand)
            })?;
            Ok(Self {
                publication: Publication::new(),
                revert_released: ReleaseNotification::default(),
                blocks_released: ReleaseNotification::default(),
                revert,
                blocks,
                allocation: Some(budget.clone()),
            })
        })
    }

    /// Run one ordinary block under the original two writers and finite pool.
    ///
    /// Both writer shells are admitted together, then the retained undo map is
    /// cleared through genuine admitted reset. Any opening refusal abandons the
    /// private cursors without changing either published map. `Ok` publishes the
    /// actual current/undo pair; `Err` abandons it. The higher-ranked callback
    /// cannot return physical writer guards or references into its private block.
    ///
    /// The original pool defers refund wakes across acquisition, callback and
    /// final writer destruction. Do not catch an edit panic and keep using the
    /// block: both original cursors and this aggregate remain unusable. This admits
    /// insertion and removal; World execution and detached publication remain unfunded.
    pub fn try_with_admitted_block<R, E>(
        &self,
        operation: impl for<'s> FnOnce(&mut Block<'s, K, V, Prepaid<P>>) -> Result<R, E>,
    ) -> Result<R, AdmittedBlockError<E>> {
        self.with_admitted_block(BlockMode::Ordinary, operation)
    }

    /// Restore the latest block's preimages, then run a replacement under the
    /// original pool and both physical writers.
    ///
    /// Each restoration admits its map edit and incoming copies before allocation.
    /// Undo is cleared only after all preimages are restored. Any opening refusal
    /// or callback error abandons both private trees, including a restored prefix.
    /// Successful edits record preimages from the restored state. Even an empty
    /// replacement advances publication identity with replacement mode.
    ///
    /// The callback cannot return writer guards. Refund notifications remain
    /// deferred until both writers unlock. This is per-edit admission; aggregate
    /// replacement work and State resource policy still require their own bounds.
    pub fn try_with_admitted_replacement<R, E>(
        &self,
        operation: impl for<'s> FnOnce(&mut Block<'s, K, V, Prepaid<P>>) -> Result<R, E>,
    ) -> Result<R, AdmittedBlockError<E>> {
        self.with_admitted_block(BlockMode::Replace, operation)
    }

    /// Restore exact current entries and undo tombstones from a borrowed snapshot.
    ///
    /// The caller authenticates the source schema and fences its acquisition with
    /// the source publication generation. Both destination trees remain private
    /// until all copies succeed. Refusal preserves the original snapshot for retry.
    /// Initial trees, writer shells and each edit's incoming copies use the one
    /// supplied pool; native control storage, decoding and aggregate restore work
    /// require separate admission.
    pub fn try_from_snapshot_admitted<M: StorageMode<K, V>>(
        snapshot: &super::snapshot::Snapshot<'_, K, V, M>,
        budget: AllocationBudget,
    ) -> Result<Self, AdmittedStorageError> {
        budget.with_deferred_refund_notifications(|| {
            let restored = Self::try_new_admitted(budget.clone())?;
            let AdmittedWriters {
                mut revert,
                mut blocks,
            } = restored.open_admitted_writers()?;
            for (key, value) in snapshot.current().iter() {
                insert_copy(&mut blocks, key, value, &budget)?;
            }
            for (key, value) in snapshot.revert_map().iter() {
                insert_copy(&mut revert, key, value, &budget)?;
            }
            publish_pair(
                blocks,
                revert,
                &restored.publication,
                NextPublication::new(),
                true,
            );
            Ok(restored)
        })
    }

    fn with_admitted_block<R, E>(
        &self,
        mode: BlockMode,
        operation: impl for<'s> FnOnce(&mut Block<'s, K, V, Prepaid<P>>) -> Result<R, E>,
    ) -> Result<R, AdmittedBlockError<E>> {
        let budget = self
            .allocation
            .as_ref()
            .expect("admitted Storage original pool");
        budget.with_deferred_refund_notifications(|| {
            let mut block = self
                .open_admitted_block(mode)
                .map_err(AdmittedBlockError::Admission)?;
            let output = operation(&mut block).map_err(AdmittedBlockError::Callback)?;
            block.assert_admitted_operable();
            let Block {
                revert,
                blocks,
                dirty,
                publication,
                ..
            } = block;
            publish_pair(blocks, revert, publication, NextPublication::new(), dirty);
            Ok(output)
        })
    }

    // Private: every caller retains both physical writers inside the original
    // pool's synchronous refund scope. No public open/detach escape is exposed.
    fn open_admitted_writers(&self) -> Result<AdmittedWriters<'_, K, V, P>, AdmittedStorageError> {
        let budget = self
            .allocation
            .as_ref()
            .expect("admitted Storage original pool");
        let current = BptreeMap::<K, V, Prepaid<P>>::writer_start_allocation_demand()
            .map_err(AdmittedStorageError::Planning)?;
        let undo = BptreeMap::<K, Option<V>, Prepaid<P>>::writer_start_allocation_demand()
            .map_err(AdmittedStorageError::Planning)?;
        let (current, undo) = reserve_pair(budget, current, undo)?;
        let wait = self.revert_released.observe();
        let revert = acquire_writer(&self.revert_released, || {
            self.revert
                .try_write_admitted(|demand| policy::<P>(budget, undo, demand))
        })
        .map_err(|error| writer_error(error, StorageRole::Undo, wait))?;
        let wait = self.blocks_released.observe();
        let blocks = acquire_writer(&self.blocks_released, || {
            self.blocks
                .try_write_admitted(|demand| policy::<P>(budget, current, demand))
        })
        .map_err(|error| writer_error(error, StorageRole::Current, wait))?;
        Ok(AdmittedWriters { revert, blocks })
    }

    fn open_admitted_block(
        &self,
        mode: BlockMode,
    ) -> Result<Block<'_, K, V, Prepaid<P>>, AdmittedStorageError> {
        let budget = self
            .allocation
            .as_ref()
            .expect("admitted Storage original pool");
        let AdmittedWriters {
            mut revert,
            mut blocks,
        } = self.open_admitted_writers()?;
        let predecessor = self.publication.capture();
        if mode == BlockMode::Replace {
            for (key, previous) in revert.iter() {
                if let Some(value) = previous {
                    insert_copy(&mut blocks, key, value, budget)?;
                } else {
                    let removed = blocks
                        .try_remove_admitted(key, |demand| admit::<P>(budget, demand))
                        .map_err(edit_error)?;
                    drop(removed);
                }
            }
        }
        revert
            .try_clear_admitted(|demand| admit::<P>(budget, demand))
            .map_err(edit_error)?;
        Ok(Block {
            revert,
            blocks,
            dirty: mode == BlockMode::Replace,
            failed: false,
            allocation: Some(budget),
            publication: &self.publication,
            predecessor,
            mode,
        })
    }
}

impl<K, V, P> Block<'_, K, V, Prepaid<P>>
where
    K: Key,
    V: Value,
    P: AdmittedStoragePolicy + ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    pub(super) fn assert_admitted_operable(&self) {
        assert!(
            !self.failed,
            "admitted block edit unwound; abandon both writers"
        );
        self.blocks.len();
        self.revert.len();
    }

    /// Insert current plus its missing first preimage under one checked demand.
    /// Ordinary refusal returns the exact inputs and keeps both private maps
    /// reusable. Existing first None/Some preimages are never rewritten.
    pub fn try_insert_admitted(
        &mut self,
        key: K,
        value: V,
    ) -> Result<Option<V>, ((K, V), AdmittedStorageError)> {
        self.assert_admitted_operable();
        let budget = self.allocation.expect("admitted block original pool");
        self.failed = true;
        let result = self
            .blocks
            .try_insert_with_undo_admitted(&mut self.revert, key, value, |demand, _key| {
                admit::<P>(budget, demand)
            })
            .map_err(|(input, error)| {
                let error = match error {
                    PairInsertError::Planning(error) => AdmittedStorageError::Planning(error),
                    PairInsertError::Refused(error) => error,
                    PairInsertError::Current(_) | PairInsertError::Undo(_) => {
                        unreachable!("borrowed joined edit never reacquires either writer")
                    }
                };
                (input, error)
            });
        if result.is_ok() {
            self.dirty = true;
        }
        self.failed = false;
        result
    }

    /// Remove through the original current/undo pair under one checked demand.
    ///
    /// An absent query still retains its explicit first None preimage, while only
    /// a present removal marks the block dirty. Refusal returns the exact owned
    /// query without changing either private map. The canonical pair consumes a
    /// successful query before releasing its failure guard; a caught key-drop or
    /// copy panic therefore forbids publication of this original block.
    pub fn try_remove_admitted(&mut self, key: K) -> Result<Option<V>, (K, AdmittedStorageError)> {
        self.assert_admitted_operable();
        let budget = self.allocation.expect("admitted block original pool");
        self.failed = true;
        let result = self
            .blocks
            .try_remove_with_undo_admitted(&mut self.revert, key, |demand, _key| {
                admit::<P>(budget, demand)
            })
            .map_err(|(key, error)| {
                let error = match error {
                    PairRemoveError::Planning(error) => AdmittedStorageError::Planning(error),
                    PairRemoveError::Refused(error) => error,
                };
                (key, error)
            });
        if let Ok(previous) = &result {
            self.dirty |= previous.is_some();
        }
        self.failed = false;
        result
    }

    /// Borrow a private current value without iteration allocation.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.assert_admitted_operable();
        self.blocks.get(key)
    }

    /// Number of private current entries.
    pub fn len(&self) -> usize {
        self.assert_admitted_operable();
        self.blocks.len()
    }
    /// Whether the private current map has no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
