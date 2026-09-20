//! Finite insertion into the original MV current/undo pair.
//!
//! This admits original node, cursor, reader, tracking and copied payload owners.
//! It does not admit publication/release control objects or iteration workspace.
//! Transactions, capture/detachment, removal, mutable access and replacement
//! blocks remain unavailable until their complete ownership paths are funded.

use super::*;
use crate::{
    ReleaseWait,
    allocation::{AllocationBudget, AllocationCharge, AllocationRefusal, AllocationReservation},
};
use concread::bptree::{
    AllocationDemand, ClearAdmissionError, ClonePlanning, InsertAdmissionError, NodeFunding,
    PairInsertError, PlanningError, Prepaid,
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

/// Either opening/insertion admission failed or the caller aborted its block.
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

fn admit<P: AdmittedStoragePolicy>(
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
        .try_partition(current.bytes())
        .expect("part of the same checked complete demand");
    Ok((current, original))
}

fn writer_error(
    error: InsertAdmissionError<AdmittedStorageError>,
    role: StorageRole,
    release: ReleaseWait,
) -> AdmittedStorageError {
    match error {
        InsertAdmissionError::Busy => AdmittedStorageError::Busy { role, release },
        InsertAdmissionError::Poisoned => AdmittedStorageError::Poisoned { role },
        InsertAdmissionError::Planning(error) => AdmittedStorageError::Planning(error),
        InsertAdmissionError::Refused(error) => error,
        InsertAdmissionError::Changed => unreachable!("new original writer has no detached input"),
    }
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
    /// block: both original cursors and this aggregate remain unusable. This is
    /// insertion admission, not admitted World execution or detached publication.
    pub fn try_with_admitted_block<R, E>(
        &self,
        operation: impl for<'s> FnOnce(&mut Block<'s, K, V, Prepaid<P>>) -> Result<R, E>,
    ) -> Result<R, AdmittedBlockError<E>> {
        let budget = self
            .allocation
            .as_ref()
            .expect("admitted Storage original pool");
        budget.with_deferred_refund_notifications(|| {
            let mut block = self
                .open_admitted_block()
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
            publication.publish(|| {
                if dirty {
                    blocks.release_with(|writer| writer.commit());
                }
                revert.release_with(|writer| writer.commit());
            });
            Ok(output)
        })
    }

    // Private: a physical writer may only exist inside try_with_admitted_block's
    // complete synchronous refund scope. No public open/detach escape is exposed.
    fn open_admitted_block(&self) -> Result<Block<'_, K, V, Prepaid<P>>, AdmittedStorageError> {
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
        let revert = self
            .revert_released
            .with_acquisition_unwind_notification(|| {
                self.revert
                    .try_write_admitted(|demand| policy::<P>(budget, undo, demand))
            })
            .map_err(|error| writer_error(error, StorageRole::Undo, wait))?;
        let mut revert = self.revert_released.poisoning_guard(revert);
        let wait = self.blocks_released.observe();
        let blocks = self
            .blocks_released
            .with_acquisition_unwind_notification(|| {
                self.blocks
                    .try_write_admitted(|demand| policy::<P>(budget, current, demand))
            })
            .map_err(|error| writer_error(error, StorageRole::Current, wait))?;
        let blocks = self.blocks_released.poisoning_guard(blocks);
        let predecessor = self.publication.capture();
        revert
            .try_clear_admitted(|demand| admit::<P>(budget, demand))
            .map_err(|error| match error {
                ClearAdmissionError::Planning(error) => AdmittedStorageError::Planning(error),
                ClearAdmissionError::Refused(error) => error,
            })?;
        Ok(Block {
            revert,
            blocks,
            dirty: false,
            failed: false,
            allocation: Some(budget),
            publication: &self.publication,
            predecessor,
            mode: BlockMode::Ordinary,
        })
    }
}

impl<K, V, P> Block<'_, K, V, Prepaid<P>>
where
    K: Key,
    V: Value,
    P: AdmittedStoragePolicy + ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    fn assert_admitted_operable(&self) {
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
            .try_insert_with_undo_admitted(&mut self.revert, key, value, |demand| {
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

    /// Borrow a private current value without iteration allocation.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.assert_admitted_operable();
        self.blocks.get(key)
    }

    /// Borrow the original value before this block's first mutation of a key.
    pub fn get_before_block(&self, key: &K) -> Option<&V> {
        self.assert_admitted_operable();
        match self.revert.get(key) {
            Some(previous) => previous.as_ref(),
            None => self.blocks.get(key),
        }
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
    /// Whether any admitted insertion changed this private current generation.
    pub fn is_dirty(&self) -> bool {
        self.assert_admitted_operable();
        self.dirty
    }
}
