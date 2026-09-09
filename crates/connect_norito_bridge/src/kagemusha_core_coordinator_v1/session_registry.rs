//! Test-only process-local ownership and revocation kernel for native wallet sessions.
//!
//! This module grants no enrollment, hardware or monetary authority. Its keys and payloads
//! must come from the native enrolled-open verifier. One retained owner serializes all uses
//! of a wallet for the process lifetime, independently of revocable UI handles. Completion
//! tickets retain their original owner and cannot install a session after cancellation or an
//! account switch. No registry lock is held while checking proofs or performing device I/O.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
};

use super::native_deadline::NativeDeadlineV1;

const MAX_OWNERS: usize = 64;
const MAX_HANDLES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RegistryError {
    Rejected,
    Capacity,
    Poisoned,
}

type Result<T> = std::result::Result<T, RegistryError>;

struct Owner<K, O> {
    key: K,
    value: Arc<Mutex<O>>,
    usable: bool,
}

struct Pending<P> {
    id: u64,
    owner: usize,
    selection: u64,
    deadline: NativeDeadlineV1,
    payload: Option<P>,
}

struct Handle {
    owner: usize,
    selection: u64,
}

struct State<K, O, P> {
    next_id: u64,
    selection: u64,
    selected_owner: Option<usize>,
    preparing: Option<u64>,
    owners: Vec<Owner<K, O>>,
    pending: Option<Pending<P>>,
    handles: BTreeMap<u64, Handle>,
}

/// Bounded lifecycle state; intentionally private to the verified native open boundary.
pub(super) struct SessionRegistry<K, O, P> {
    identity: Arc<()>,
    state: Mutex<State<K, O, P>>,
}

/// Read-only dispatch permit for one native preparation. It cannot create an owner,
/// extend the original deadline, or survive the borrowed begin invocation. A callback that
/// waits for its owner must check again under that owner immediately before source I/O.
pub(super) struct PreparationPermitV1<'a, K, O, P> {
    registry: &'a SessionRegistry<K, O, P>,
    id: u64,
    deadline: NativeDeadlineV1,
}

impl<K: Eq, O, P> PreparationPermitV1<'_, K, O, P> {
    pub(super) fn require_current(&self) -> Result<()> {
        let state = self.registry.lock()?;
        if state.preparing == Some(self.id) && self.deadline.check().is_ok() {
            Ok(())
        } else {
            Err(RegistryError::Rejected)
        }
    }
}

/// Single-use selected attempt, consumed before signature verification outside the registry.
pub(super) struct OpenCompletion<O, P> {
    registry: Arc<()>,
    id: u64,
    owner: usize,
    selection: u64,
    deadline: NativeDeadlineV1,
    owner_value: Arc<Mutex<O>>,
    pub(super) payload: P,
}

/// A queued invocation owns the exact original native owner, but must pass revocation again
/// after acquiring that owner's serialization lock and immediately before dispatch.
pub(super) struct Invocation<O> {
    registry: Arc<()>,
    handle: u64,
    owner: usize,
    selection: u64,
    owner_value: Arc<Mutex<O>>,
}

/// A completed invocation remains associated with its original owner even after revocation.
/// The caller persists exact dispatched results within the owner before returning from I/O.
pub(super) struct InvocationResult<T> {
    pub(super) value: T,
    pub(super) session_is_current: bool,
}

impl<K: Eq, O, P> SessionRegistry<K, O, P> {
    pub(super) fn new() -> Self {
        Self {
            identity: Arc::new(()),
            state: Mutex::new(State {
                next_id: 1,
                selection: 0,
                selected_owner: None,
                preparing: None,
                owners: Vec::new(),
                pending: None,
                handles: BTreeMap::new(),
            }),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, State<K, O, P>>> {
        self.state.lock().map_err(|_| RegistryError::Poisoned)
    }

    /// Begin a bounded native preparation before any source read or owner-lock wait.
    /// The callback supplies only verified native ownership; it runs without the global lock.
    /// Each preparation supersedes older pending work but preserves existing UI handles until
    /// a different owner is published. Revocation or a newer begin during I/O invalidates this
    /// preparation, even when its owner and account happen to match the new selection.
    pub(super) fn begin(
        &self,
        deadline: NativeDeadlineV1,
        prepare: impl FnOnce(PreparationPermitV1<'_, K, O, P>) -> Result<(K, Arc<Mutex<O>>, P)>,
    ) -> Result<u64> {
        let id = {
            let mut state = self.lock()?;
            if deadline.check().is_err() {
                return Err(RegistryError::Rejected);
            }
            let id = state.next_id;
            let next_id = id.checked_add(1).ok_or(RegistryError::Capacity)?;
            state.next_id = next_id;
            state.preparing = Some(id);
            state.pending = None;
            id
        };
        let permit = PreparationPermitV1 {
            registry: self,
            id,
            deadline: deadline.clone(),
        };
        permit.require_current()?;
        let (key, owner_value, payload) = prepare(permit)?;
        let mut state = self.lock()?;
        if state.preparing != Some(id) || deadline.check().is_err() {
            return Err(RegistryError::Rejected);
        }
        let existing = state.owners.iter().position(|owner| owner.key == key);
        if existing.is_none()
            && state
                .owners
                .iter()
                .any(|owner| Arc::ptr_eq(&owner.value, &owner_value))
        {
            return Err(RegistryError::Rejected);
        }
        if let Some(index) = existing {
            if !state.owners[index].usable || !Arc::ptr_eq(&state.owners[index].value, &owner_value)
            {
                return Err(RegistryError::Rejected);
            }
        } else if state.owners.len() >= MAX_OWNERS {
            return Err(RegistryError::Capacity);
        }
        let owner = existing.unwrap_or(state.owners.len());
        let selection = if state.selected_owner == Some(owner) {
            state.selection
        } else {
            state
                .selection
                .checked_add(1)
                .ok_or(RegistryError::Capacity)?
        };
        // Waiting for the registry or a handset suspension may exhaust the original lease.
        // Recheck at publication, before adding an owner or revoking the previous selection.
        if deadline.check().is_err() {
            return Err(RegistryError::Rejected);
        }
        if existing.is_none() {
            state.owners.push(Owner {
                key,
                value: owner_value,
                usable: true,
            });
        }
        if state.selected_owner != Some(owner) {
            state.handles.clear();
        }
        state.preparing = None;
        state.selection = selection;
        state.selected_owner = Some(owner);
        state.pending = Some(Pending {
            id,
            owner,
            selection,
            deadline,
            payload: Some(payload),
        });
        Ok(id)
    }

    #[cfg(test)]
    pub(super) fn preparing_for_test(&self) -> Result<Option<u64>> {
        Ok(self.lock()?.preparing)
    }

    /// Remove a specific challenge without canceling a replacement challenge.
    pub(super) fn cancel(&self, id: u64) -> Result<()> {
        let mut state = self.lock()?;
        if state.preparing == Some(id) {
            state.preparing = None;
        }
        if state
            .pending
            .as_ref()
            .is_some_and(|pending| pending.id == id)
        {
            state.pending = None;
        }
        Ok(())
    }

    /// Explicit logout/account revocation also cancels an in-flight open completion.
    pub(super) fn revoke_selection(&self) -> Result<()> {
        let mut state = self.lock()?;
        let selection = state
            .selection
            .checked_add(1)
            .ok_or(RegistryError::Capacity)?;
        state.selection = selection;
        state.selected_owner = None;
        state.preparing = None;
        state.pending = None;
        state.handles.clear();
        Ok(())
    }

    pub(super) fn take_completion(&self, id: u64) -> Result<OpenCompletion<O, P>> {
        let mut state = self.lock()?;
        let pending = state.pending.as_mut().ok_or(RegistryError::Rejected)?;
        if pending.id != id || pending.deadline.check().is_err() {
            return Err(RegistryError::Rejected);
        }
        let payload = pending.payload.take().ok_or(RegistryError::Rejected)?;
        let (owner, selection, deadline) =
            (pending.owner, pending.selection, pending.deadline.clone());
        Ok(OpenCompletion {
            registry: Arc::clone(&self.identity),
            id,
            owner,
            selection,
            deadline,
            payload,
            owner_value: Arc::clone(&state.owners[owner].value),
        })
    }

    /// Consume one verified enrollment phase into a fresh possession attempt without opening
    /// a handle. `prepare` checks the still-current owner under its serialization lock and
    /// produces the next native payload. No callback may replace or mutate that owner.
    /// The new attempt retains the original selection and deadline; an old cancel cannot
    /// cancel it. Cryptographic work and source reads never hold the global registry lock.
    pub(super) fn advance<V>(
        &self,
        completion: OpenCompletion<O, P>,
        verify: impl FnOnce(P) -> Result<V>,
        prepare: impl FnOnce(&O, V) -> Result<P>,
    ) -> Result<u64> {
        if !Arc::ptr_eq(&completion.registry, &self.identity) {
            return Err(RegistryError::Rejected);
        }
        // Reject already cancelled/replaced/expired tickets before parsing or crypto.
        // This is a dispatch check only; the registry lock never spans the callback.
        self.require_current_completion(
            completion.id,
            completion.owner,
            completion.selection,
            &completion.deadline,
        )?;
        let verified = verify(completion.payload)?;
        let owner = completion
            .owner_value
            .lock()
            .map_err(|_| RegistryError::Poisoned)?;
        // Revocation may have happened during verification or while queued on the
        // original owner. Recheck under that owner before any source/hardware I/O.
        self.require_current_completion(
            completion.id,
            completion.owner,
            completion.selection,
            &completion.deadline,
        )?;
        let payload = prepare(&owner, verified)?;
        let mut state = self.lock()?;
        if !Self::completion_is_current(
            &state,
            completion.id,
            completion.owner,
            completion.selection,
            &completion.deadline,
        ) {
            return Err(RegistryError::Rejected);
        }
        let id = state.next_id;
        let next_id = id.checked_add(1).ok_or(RegistryError::Capacity)?;
        state.pending = Some(Pending {
            id,
            owner: completion.owner,
            selection: completion.selection,
            deadline: completion.deadline,
            payload: Some(payload),
        });
        state.next_id = next_id;
        Ok(id)
    }

    fn require_current_completion(
        &self,
        id: u64,
        owner: usize,
        selection: u64,
        deadline: &NativeDeadlineV1,
    ) -> Result<()> {
        if Self::completion_is_current(&*self.lock()?, id, owner, selection, deadline) {
            Ok(())
        } else {
            Err(RegistryError::Rejected)
        }
    }

    fn completion_is_current(
        state: &State<K, O, P>,
        id: u64,
        owner: usize,
        selection: u64,
        deadline: &NativeDeadlineV1,
    ) -> bool {
        state.pending.as_ref().is_some_and(|pending| {
            pending.id == id
                && pending.owner == owner
                && pending.selection == selection
                && pending.payload.is_none()
        }) && state.selection == selection
            && state.selected_owner == Some(owner)
            && state.owners[owner].usable
            && deadline.check().is_ok()
    }

    /// Signature verification consumes P before this call. `prepare` revalidates the full
    /// still-current native checkpoint and current initial-admission authority under the owner
    /// lock. It may do I/O. `install` must only install that prepared local observer/evidence;
    /// it runs after the final cancellation check, under both locks, and must not do I/O.
    /// Neither callback may replace this retained owner or create monetary authority.
    pub(super) fn finish<V, C>(
        &self,
        completion: OpenCompletion<O, P>,
        verify: impl FnOnce(P) -> Result<V>,
        prepare: impl FnOnce(&O, V) -> Result<C>,
        install: impl FnOnce(&mut O, C) -> Result<()>,
    ) -> Result<u64> {
        if !Arc::ptr_eq(&completion.registry, &self.identity) {
            return Err(RegistryError::Rejected);
        }
        // Reject already cancelled/replaced/expired tickets before parsing or crypto.
        // This is a dispatch check only; the registry lock never spans the callback.
        self.require_current_completion(
            completion.id,
            completion.owner,
            completion.selection,
            &completion.deadline,
        )?;
        let verified = verify(completion.payload)?;
        let mut owner = completion
            .owner_value
            .lock()
            .map_err(|_| RegistryError::Poisoned)?;
        // Revocation may have happened during verification or while queued on the
        // original owner. Recheck under that owner before any source/hardware I/O.
        self.require_current_completion(
            completion.id,
            completion.owner,
            completion.selection,
            &completion.deadline,
        )?;
        let prepared = prepare(&owner, verified)?;
        let mut state = self.lock()?;
        if !Self::completion_is_current(
            &state,
            completion.id,
            completion.owner,
            completion.selection,
            &completion.deadline,
        ) {
            return Err(RegistryError::Rejected);
        }
        if state.handles.len() >= MAX_HANDLES {
            return Err(RegistryError::Capacity);
        }
        let handle = state.next_id;
        let next_id = handle.checked_add(1).ok_or(RegistryError::Capacity)?;
        // Consume before installing: an error cannot expose or retry a partially installed
        // observer. The owner implementation must prepare mutations before this pure commit.
        state.pending = None;
        if let Err(error) = install(&mut owner, prepared) {
            // A failed local commit has no safe rollback assumption. Retain the original
            // owner for diagnosis/recovery, but no old or future session may use it.
            state.owners[completion.owner].usable = false;
            state
                .handles
                .retain(|_, handle| handle.owner != completion.owner);
            return Err(error);
        }
        state.next_id = next_id;
        state.handles.insert(
            handle,
            Handle {
                owner: completion.owner,
                selection: completion.selection,
            },
        );
        Ok(handle)
    }

    /// Closing a UI handle never drops its process-lifetime owner or an already dispatched
    /// operation. Repeated close is harmless; IDs never recycle.
    pub(super) fn close(&self, handle: u64) -> Result<()> {
        self.lock()?.handles.remove(&handle);
        Ok(())
    }

    pub(super) fn invocation(&self, handle: u64) -> Result<Invocation<O>> {
        let state = self.lock()?;
        let entry = state.handles.get(&handle).ok_or(RegistryError::Rejected)?;
        Ok(Invocation {
            registry: Arc::clone(&self.identity),
            handle,
            owner: entry.owner,
            selection: entry.selection,
            owner_value: Arc::clone(&state.owners[entry.owner].value),
        })
    }

    fn invocation_is_current(state: &State<K, O, P>, invocation: &Invocation<O>) -> bool {
        state.selected_owner == Some(invocation.owner)
            && state.selection == invocation.selection
            && state.owners[invocation.owner].usable
            && state.handles.get(&invocation.handle).is_some_and(|handle| {
                handle.owner == invocation.owner && handle.selection == invocation.selection
            })
    }

    /// Serialize on the original owner, then check revocation at the dispatch linearization
    /// point. `operation` must durably retain any dispatched device completion before returning,
    /// including when close/account switch races with I/O. Global registry contention never
    /// holds up device I/O; a stale queued call never dispatches.
    pub(super) fn dispatch<T>(
        &self,
        invocation: Invocation<O>,
        operation: impl FnOnce(&mut O) -> T,
    ) -> Result<InvocationResult<T>> {
        if !Arc::ptr_eq(&invocation.registry, &self.identity) {
            return Err(RegistryError::Rejected);
        }
        let mut owner = invocation
            .owner_value
            .lock()
            .map_err(|_| RegistryError::Poisoned)?;
        if !Self::invocation_is_current(&*self.lock()?, &invocation) {
            return Err(RegistryError::Rejected);
        }
        let value = operation(&mut owner);
        let session_is_current = Self::invocation_is_current(&*self.lock()?, &invocation);
        Ok(InvocationResult {
            value,
            session_is_current,
        })
    }
}

#[cfg(test)]
#[path = "session_registry/tests.rs"]
mod tests;
