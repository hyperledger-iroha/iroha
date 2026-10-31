//! Original native execution custody for UID binding reads across overlay rollback.
//!
//! A first read captures its exact canonical path and authoritative raw row in funded
//! immutable buffers. Later reads rejoin the same transaction/read occurrence and
//! compare the current State row before borrowing the retained canonical account.
//! One inline payload and the original paid nodes form a safe zipper. Each pass
//! rewinds once, then advances monotonically without searching from the head.
//! This is decoder custody only: every binding/key and policy check still runs.

use super::*;
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedShared};
use iroha_data_model::asset::uid::{AllocatedAssetUidBindingV1, PendingAssetUidBindingDecodeV1};
use std::cell::Cell;

/// Shared original decoder bank retained by the native signed-source attempt.
#[derive(Clone)]
pub(crate) struct BindingPreparations(ChargedShared<BankLock>);

/// One disposable execution pass; its transaction ordinals restart on actual retry.
pub(crate) struct BindingRun {
    original: BindingPreparations,
    next_transaction: usize,
    generation: usize,
}

/// Exact occurrence of a read in a fresh transaction of the same execution pass.
pub(crate) struct BindingInvocation {
    original: BindingPreparations,
    transaction: usize,
    next_read: Cell<usize>,
    generation: usize,
}

#[derive(Default)]
struct Bank {
    // The inline current payload owns the future list; visited nodes form a reversed list.
    current: Option<Entry>,
    // Exact unfinished append remains inline until its actual metadata node fits.
    pending_append: Option<Entry>,
    previous: Option<ChargedBuffer<Entry>>,
    current_seen: bool,
    last_read: Option<(usize, usize)>,
    generation: usize,
    #[cfg(test)]
    traversals: usize,
}
/// An inline mutex avoids the platform mutex's uncharged lazy heap allocation.
/// Preserve poisoning: no caller may reuse a bank abandoned while its guard unwound.
#[derive(Default)]
struct BankLock {
    inner: parking_lot::Mutex<Bank>,
    poisoned: std::sync::atomic::AtomicBool,
}
struct BankGuard<'a> {
    inner: parking_lot::MutexGuard<'a, Bank>,
    poisoned: &'a std::sync::atomic::AtomicBool,
}
impl BankLock {
    fn lock(&self) -> Result<BankGuard<'_>, ()> {
        let inner = self.inner.lock();
        if self.poisoned.load(std::sync::atomic::Ordering::Acquire) {
            return Err(());
        }
        Ok(BankGuard {
            inner,
            poisoned: &self.poisoned,
        })
    }
    #[cfg(test)]
    fn try_lock(&self) -> Result<BankGuard<'_>, ()> {
        let inner = self.inner.try_lock().ok_or(())?;
        if self.poisoned.load(std::sync::atomic::Ordering::Acquire) {
            return Err(());
        }
        Ok(BankGuard {
            inner,
            poisoned: &self.poisoned,
        })
    }
}
impl std::ops::Deref for BankGuard<'_> {
    type Target = Bank;
    fn deref(&self) -> &Bank {
        &self.inner
    }
}
impl std::ops::DerefMut for BankGuard<'_> {
    fn deref_mut(&mut self) -> &mut Bank {
        &mut self.inner
    }
}
impl Drop for BankGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.poisoned
                .store(true, std::sync::atomic::Ordering::Release);
        }
    }
}
struct Entry {
    // Values and partial graphs retire before their source and charge metadata.
    complete: Option<AllocatedAssetUidBindingV1>,
    // A captured-byte rejection has no authority until the actual State read rejoins.
    rejected: Option<Error>,
    pending: PendingAssetUidBindingDecodeV1,
    path: Option<ChargedBuffer<u8>>,
    row: Option<ChargedBuffer<u8>>,
    identity: (usize, usize),
    path_hash: iroha_crypto::Hash,
    row_hash: iroha_crypto::Hash,
    path_len: usize,
    row_len: usize,
    next: Option<ChargedBuffer<Entry>>,
}

fn invariant() -> crate::execution_attempt::ExecutionDeferred {
    ivm::error::ExecutionDeferral::LocalInvariantViolation.into()
}
fn backing(error: ChargedBufferError) -> crate::execution_attempt::ExecutionDeferred {
    match error {
        ChargedBufferError::Admission(original) => original.into(),
        original @ ChargedBufferError::Allocator { .. } => AllocatorRefusal::try_from(original)
            .expect("matched physical binding backing")
            .into(),
    }
}

impl BindingPreparations {
    /// Advance only the last captured decoder before rebuilding disposable World state.
    /// This grants no UID or policy authority: the actual invocation must still match
    /// its current path, row and occurrence before it can use the completed graph.
    pub(crate) fn prepare_captured(
        &self,
        pool: &AllocationBudget,
    ) -> Result<(), crate::execution_attempt::ExecutionDeferred> {
        if !self.0.belongs_to(pool) {
            return Err(invariant());
        }
        // No decoder or retirement exists before the first capture. Avoid even
        // initializing thread-local refund notification scratch for this empty pass.
        if self.0.lock().map_err(|_| invariant())?.current.is_none() {
            return Ok(());
        }
        pool.with_deferred_refund_notifications(|_| {
            let mut bank = self.0.lock().map_err(|_| invariant())?;
            bank.finish_append(pool)?;
            let Some(current) = bank.current.as_mut() else {
                return Ok(());
            };
            current.prepare_capture(pool)?;
            if current
                .row
                .as_ref()
                .is_none_or(|row| row.as_slice().len() != current.row_len)
                || current.rejected.is_some()
            {
                return Ok(());
            }
            match current.prepare_decode(pool) {
                Ok(()) => Ok(()),
                Err(ExecutionAttemptError::Deferred(original)) => Err(original),
                Err(ExecutionAttemptError::Rejected(error)) => {
                    current.rejected = Some(error);
                    Ok(())
                }
            }
        })
    }
    #[cfg(test)]
    pub(crate) fn same_owner(&self, other: &Self) -> bool {
        ChargedShared::ptr_eq(&self.0, &other.0)
    }
    /// Admit the original shared bank control before a native execution pass.
    pub(crate) fn try_new(
        pool: &AllocationBudget,
    ) -> Result<Self, crate::execution_attempt::ExecutionDeferred> {
        let mut reservation = pool.try_reserve(ChargedShared::<BankLock>::allocation_layout())?;
        let original = ChargedShared::from_reservation(BankLock::default(), &mut reservation)
            .map_err(|(_, error)| match error {
                original @ PrepaidSharedError::Allocator { .. } => {
                    AllocatorRefusal::try_from(original)
                        .expect("matched physical binding bank control")
                        .into()
                }
                PrepaidSharedError::Reservation(_) => invariant(),
            })?;
        Ok(Self(original))
    }

    /// Begin a new disposable World pass without replacing any captured occurrence.
    pub(crate) fn run(
        &self,
        pool: &AllocationBudget,
    ) -> Result<BindingRun, crate::execution_attempt::ExecutionDeferred> {
        if !self.0.belongs_to(pool) {
            return Err(invariant());
        }
        pool.with_deferred_refund_notifications(|_| {
            let mut bank = self.0.lock().map_err(|_| invariant())?;
            let generation = bank.generation.checked_add(1).ok_or_else(invariant)?;
            bank.rewind();
            bank.generation = generation;
            Ok(BindingRun {
                original: self.clone(),
                next_transaction: 0,
                generation,
            })
        })
    }
}
impl BindingRun {
    /// Lend the next exact transaction occurrence without allocating a new owner.
    pub(crate) fn transaction(&mut self) -> BindingInvocation {
        let transaction = self.next_transaction;
        self.next_transaction = transaction.saturating_add(1);
        BindingInvocation {
            original: self.original.clone(),
            transaction,
            next_read: Cell::new(0),
            generation: self.generation,
        }
    }
}
impl BindingInvocation {
    /// Rejoin the actual State read and complete only its original funded decoder.
    pub(crate) fn read(
        &self,
        path: &StatePath,
        row: Option<&[u8]>,
        definition: &AssetDefinitionId,
        account: &AccountId,
        pool: &AllocationBudget,
    ) -> Result<Option<[u8; 32]>, ExecutionAttemptError<Error>> {
        let read = self.next_read.get();
        self.next_read.set(read.saturating_add(1));
        if self.transaction == usize::MAX || read == usize::MAX || !self.original.0.belongs_to(pool)
        {
            return Err(ExecutionAttemptError::Deferred(invariant()));
        }
        let identity = (self.transaction, read);
        let path = path.as_ref().as_bytes();
        // Completion retires decoder controls while the bank is locked. Keep
        // arbitrary original-pool callbacks outside this physical guard.
        let read_locked = || {
            let mut bank = self
                .original
                .0
                .lock()
                .map_err(|_| ExecutionAttemptError::Deferred(invariant()))?;
            if bank.generation != self.generation
                || bank.last_read.is_some_and(|previous| identity <= previous)
            {
                return Err(ExecutionAttemptError::Deferred(invariant()));
            }
            bank.last_read = Some(identity);
            loop {
                #[cfg(test)]
                {
                    bank.traversals += 1;
                }
                let Some(current) = bank.current.as_ref() else {
                    break;
                };
                match identity.cmp(&current.identity) {
                    std::cmp::Ordering::Equal => {
                        bank.current_seen = true;
                        return bank
                            .current
                            .as_mut()
                            .expect("matched original occurrence")
                            .read(path, row, definition, account, pool);
                    }
                    std::cmp::Ordering::Less => {
                        // Only an absent lookup may precede the next retained positive
                        // occurrence. A new row here changes the original read sequence.
                        return if row.is_none() {
                            Ok(None)
                        } else {
                            Err(ExecutionAttemptError::Deferred(invariant()))
                        };
                    }
                    std::cmp::Ordering::Greater if !bank.current_seen => {
                        return Err(ExecutionAttemptError::Deferred(invariant()));
                    }
                    std::cmp::Ordering::Greater => {
                        if !bank.advance() {
                            break;
                        }
                    }
                }
            }
            let Some(row) = row else {
                return Ok(None);
            };
            // Preserve the ordinary header error before path/source/metadata admission.
            norito::validate_canonical_header(row).map_err(|error| {
                canonical_decode_attempt_error(error, |error| {
                    reject(format!("invalid asset UID state: {error}"))
                })
            })?;
            let entry = Entry {
                complete: None,
                rejected: None,
                pending: PendingAssetUidBindingDecodeV1::new(pool),
                path: None,
                row: None,
                identity,
                path_hash: iroha_crypto::Hash::new(path),
                row_hash: iroha_crypto::Hash::new(row),
                path_len: path.len(),
                row_len: row.len(),
                next: None,
            };
            bank.append(entry, pool)
                .map_err(ExecutionAttemptError::Deferred)?;
            bank.current_seen = true;
            bank.current
                .as_mut()
                .expect("original inserted binding occurrence")
                .read(path, Some(row), definition, account, pool)
        };
        // SPEC: §13.4 HC977 flushes actual refunds before the bank mutex unlocks.
        if cfg!(all(test, sumeragi_core_mutation = "HC977")) {
            read_locked()
        } else {
            pool.with_deferred_refund_notifications(|_| read_locked())
        }
    }
}
impl Entry {
    fn read(
        &mut self,
        path: &[u8],
        row: Option<&[u8]>,
        definition: &AssetDefinitionId,
        account: &AccountId,
        pool: &AllocationBudget,
    ) -> Result<Option<[u8; 32]>, ExecutionAttemptError<Error>> {
        let Some(row) = row else {
            return Err(ExecutionAttemptError::Deferred(invariant()));
        };
        if path.len() != self.path_len
            || row.len() != self.row_len
            || iroha_crypto::Hash::new(path) != self.path_hash
            || iroha_crypto::Hash::new(row) != self.row_hash
            || self.path.as_ref().is_some_and(|original| {
                !original.as_slice().is_empty() && original.as_slice() != path
            })
            || self.row.as_ref().is_some_and(|original| {
                !original.as_slice().is_empty() && original.as_slice() != row
            })
            || !self.pending.belongs_to(pool)
            || self
                .complete
                .as_ref()
                .is_some_and(|original| !original.belongs_to(pool))
        {
            return Err(ExecutionAttemptError::Deferred(invariant()));
        }
        // Empty prepaid capture storage is filled only by the matching actual State read.
        self.prepare_capture(pool)
            .map_err(ExecutionAttemptError::Deferred)?;
        let original = self.path.as_mut().expect("funded binding path");
        if original.as_slice().is_empty() {
            original
                .append(path)
                .expect("exact admitted binding path extent");
        }
        let original = self.row.as_mut().expect("funded binding row");
        if original.as_slice().is_empty() {
            original
                .append(row)
                .expect("exact admitted binding row extent");
        }
        if let Some(error) = self.rejected.take() {
            return Err(ExecutionAttemptError::Rejected(error));
        }
        self.prepare_decode(pool)?;
        validate_binding(
            self.complete
                .as_ref()
                .expect("complete original binding")
                .canonical(),
            definition,
            account,
        )
        .map(Some)
        .map_err(ExecutionAttemptError::Rejected)
    }

    /// Admit original empty capture backing before replaying disposable execution.
    fn prepare_capture(
        &mut self,
        pool: &AllocationBudget,
    ) -> Result<(), crate::execution_attempt::ExecutionDeferred> {
        if !self.pending.belongs_to(pool) {
            return Err(invariant());
        }
        if self.path.is_none() {
            self.path = Some(ChargedBuffer::new(self.path_len, pool).map_err(backing)?);
        }
        if self.row.is_none() {
            self.row = Some(ChargedBuffer::new(self.row_len, pool).map_err(backing)?);
        }
        Ok(())
    }

    fn prepare_decode(
        &mut self,
        pool: &AllocationBudget,
    ) -> Result<(), ExecutionAttemptError<Error>> {
        if !self.pending.belongs_to(pool)
            || self
                .complete
                .as_ref()
                .is_some_and(|value| !value.belongs_to(pool))
        {
            return Err(ExecutionAttemptError::Deferred(invariant()));
        }
        if self.complete.is_none() {
            let original = self.row.as_ref().expect("original captured binding row");
            #[cfg(test)]
            let decoded = observation::decode(&mut self.pending, original.as_slice(), pool);
            #[cfg(not(test))]
            let decoded = self.pending.decode(
                original.as_slice(),
                norito::canonical_decode_limits(original.as_slice().len()),
            );
            decoded.map_err(decode_attempt)?;
            self.complete = Some(self.pending.finish().map_err(|error| {
                ExecutionAttemptError::Rejected(reject(format!("invalid asset UID state: {error}")))
            })?);
        }
        Ok(())
    }
}
impl Bank {
    /// Rewind the same funded nodes once; no source, graph or node backing moves.
    fn rewind(&mut self) {
        while let Some(mut node) = self.previous.take() {
            let entry = node
                .as_mut_slice()
                .first_mut()
                .expect("one funded bank entry");
            self.previous = entry.next.take();
            let current = self
                .current
                .as_mut()
                .expect("visited entries retain a current entry");
            std::mem::swap(current, entry);
            current.next = Some(node);
            #[cfg(test)]
            {
                self.traversals += 1;
            }
        }
        self.current_seen = false;
        self.last_read = None;
    }

    /// Move the next original payload inline and retain the visited payload in its node.
    fn advance(&mut self) -> bool {
        let Some(current) = self.current.as_mut() else {
            return false;
        };
        let Some(mut node) = current.next.take() else {
            return false;
        };
        let entry = node
            .as_mut_slice()
            .first_mut()
            .expect("one funded bank entry");
        std::mem::swap(current, entry);
        entry.next = self.previous.take();
        self.previous = Some(node);
        self.current_seen = false;
        #[cfg(test)]
        {
            self.traversals += 1;
        }
        true
    }

    /// Admit one actual metadata node before moving an existing inline payload into it.
    fn append(
        &mut self,
        entry: Entry,
        pool: &AllocationBudget,
    ) -> Result<(), crate::execution_attempt::ExecutionDeferred> {
        if let Some(pending) = self.pending_append.as_ref() {
            if pending.identity != entry.identity
                || pending.path_hash != entry.path_hash
                || pending.row_hash != entry.row_hash
                || pending.path_len != entry.path_len
                || pending.row_len != entry.row_len
            {
                return Err(invariant());
            }
        } else {
            self.pending_append = Some(entry);
        }
        self.finish_append(pool)
    }

    /// Retain the exact pending append across metadata capacity and allocator refusals.
    fn finish_append(
        &mut self,
        pool: &AllocationBudget,
    ) -> Result<(), crate::execution_attempt::ExecutionDeferred> {
        if self.pending_append.is_none() {
            return Ok(());
        }
        if let Some(current) = self.current.as_mut() {
            if current.next.is_some() || !self.current_seen {
                return Err(invariant());
            }
            let mut node = ChargedBuffer::new(1, pool).map_err(backing)?;
            node.push_reserved(self.pending_append.take().expect("original pending append"));
            let previous = node
                .as_mut_slice()
                .first_mut()
                .expect("one funded bank entry");
            std::mem::swap(current, previous);
            previous.next = self.previous.take();
            self.previous = Some(node);
        } else {
            self.current = self.pending_append.take();
        }
        self.current_seen = false;
        Ok(())
    }
}

impl Drop for Bank {
    fn drop(&mut self) {
        // Both zipper chains retire iteratively, including after a partial rewind.
        let future = self
            .current
            .as_mut()
            .and_then(|current| current.next.take());
        for mut next in [future, self.previous.take()] {
            while let Some(mut node) = next {
                next = node
                    .as_mut_slice()
                    .first_mut()
                    .and_then(|entry| entry.next.take());
            }
        }
    }
}

#[cfg(test)]
#[path = "linear_tests.rs"]
mod linear_tests;
#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;

/// Test-only physical contention at the actual binding leaf, after source capture.
#[cfg(test)]
pub(crate) mod observation {
    use super::*;
    #[derive(Default)]
    struct Observation {
        held: Option<iroha_allocation::AllocationReservation>,
        calls: usize,
        source: Option<usize>,
        refuse_replacement: bool,
        replaced: bool,
    }
    thread_local! { static CURRENT: std::cell::RefCell<Option<Observation>> = const { std::cell::RefCell::new(None) }; }
    pub(crate) struct Observer;
    impl Observer {
        pub(crate) fn occupy_after_capture() -> Self {
            CURRENT.with(|current| {
                assert!(current.borrow().is_none());
                *current.borrow_mut() = Some(Observation::default());
            });
            Self
        }
        pub(crate) fn release_and_refuse_replacement(&self) {
            CURRENT.with(|current| {
                let mut current = current.borrow_mut();
                let original = current.as_mut().unwrap();
                original.refuse_replacement = true;
                drop(original.held.take());
            });
        }
        pub(crate) fn snapshot(&self) -> (usize, bool) {
            CURRENT.with(|current| {
                let current = current.borrow();
                let original = current.as_ref().unwrap();
                (original.calls, original.replaced)
            })
        }
    }
    impl Drop for Observer {
        fn drop(&mut self) {
            CURRENT.with(|current| {
                current.borrow_mut().take();
            });
        }
    }
    pub(super) fn decode(
        pending: &mut PendingAssetUidBindingDecodeV1,
        bytes: &[u8],
        pool: &AllocationBudget,
    ) -> Result<(), DecodeError> {
        let deny = CURRENT.with(|current| {
            let mut current = current.borrow_mut();
            let Some(original) = current.as_mut() else {
                return false;
            };
            let source = bytes.as_ptr() as usize;
            if original.source.is_some_and(|retained| retained != source) {
                return false;
            }
            original.source = Some(source);
            if original.calls == 0 {
                let controls: usize = norito::core::PreparedDecodeWorkspace::allocation_layouts()
                    .iter()
                    .map(std::alloc::Layout::size)
                    .sum();
                original.held = Some(
                    pool.try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes() - controls)
                        .unwrap(),
                );
            }
            original.calls += 1;
            original.refuse_replacement
        });
        let mut decode = || pending.decode(bytes, norito::canonical_decode_limits(bytes.len()));
        if deny {
            let (result, replaced) = crate::test_allocations::refuse_layout_occurrence_during(
                norito::core::PreparedDecodeWorkspace::allocation_layouts()[0],
                0,
                decode,
            );
            CURRENT.with(|current| current.borrow_mut().as_mut().unwrap().replaced |= replaced);
            result
        } else {
            decode()
        }
    }
}
