//! Three bounded immutable checkpoint imports per live authority; no current-state evidence.

use super::ServiceAuthority;
#[cfg(test)]
use crate::managed::native_operation::decode_checkpoint;
use crate::{
    managed::{
        Result,
        native_operation::{MAX_CHECKPOINT_BYTES, decode_checkpoint_with_validation},
    },
    verify::finality::FinalityVerifier,
};
use iroha_data_model::{
    NetworkId,
    block::execution_output::ExecutionOutputV1,
    sumeragi_finality::{EpochValidationScope, SumeragiFinalityCheckpoint, VerifiedSumeragiBlock},
};
use std::{
    mem::size_of,
    sync::{Mutex, OnceLock, TryLockError},
};

const SLOTS: usize = 3;
const MAX_CHAIN_CAPACITY: usize = 1024;

struct Entry {
    bytes: Vec<u8>,
    network: NetworkId,
    chain: String,
    verifier: FinalityVerifier,
    retained_import_envelope: usize,
}

/// Cache-owned requested retention, not producer scratch, allocator overhead or caller aliases.
/// Four independently bounded decoded graphs coexist: checkpoint, signed tip, result commitment
/// and core header. Availability has an uncharged raw copy and Arc; the signed tip's sole retained
/// Merkle array requests at most four nodes per canonically encoded 32-byte leaf. The two outer
/// Arcs are constructed after decoding. Pending observations are absent from imported originals.
fn retained_import_envelope(encoded_len: usize) -> Option<usize> {
    if encoded_len > MAX_CHECKPOINT_BYTES {
        return None;
    }
    let merkle_nodes = (encoded_len / iroha_crypto::Hash::LENGTH)
        .checked_mul(4)?
        .checked_mul(size_of::<Option<iroha_crypto::HashOf<ExecutionOutputV1>>>())?;
    norito::canonical_decode_limits(encoded_len)
        .max_total_allocated_bytes()
        .checked_mul(4)?
        .checked_add(encoded_len)?
        .checked_add(merkle_nodes)?
        .checked_add(
            norito::core::owned_arc_allocation_bytes::<SumeragiFinalityCheckpoint>().ok()?,
        )?
        .checked_add(
            norito::core::owned_arc_allocation_bytes::<OnceLock<VerifiedSumeragiBlock>>().ok()?,
        )?
        .checked_add(norito::core::owned_arc_allocation_bytes::<Vec<u8>>().ok()?)?
        .checked_add(size_of::<Entry>())?
        .checked_add(MAX_CHAIN_CAPACITY)
}

#[derive(Default)]
struct Entries {
    slots: [Option<Entry>; SLOTS],
    first: usize,
    len: usize,
}

impl Entries {
    fn clear(&mut self) {
        for slot in &mut self.slots {
            drop(slot.take());
        }
        self.first = 0;
        self.len = 0;
    }

    fn oldest(&mut self) {
        if self.len != 0 {
            drop(self.slots[self.first].take());
            self.first = (self.first + 1) % SLOTS;
            self.len -= 1;
        }
    }

    fn has_room(&self, key_capacity: usize, weight: usize) -> bool {
        let key_total = self
            .slots
            .iter()
            .flatten()
            .try_fold(key_capacity, |total, entry| {
                total.checked_add(entry.bytes.capacity())
            });
        let weight_total = self
            .slots
            .iter()
            .flatten()
            .try_fold(weight, |total, entry| {
                total.checked_add(entry.retained_import_envelope)
            });
        self.len < SLOTS
            && key_total.is_some_and(|total| total <= MAX_CHECKPOINT_BYTES)
            && weight_total
                .zip(retained_import_envelope(MAX_CHECKPOINT_BYTES))
                .is_some_and(|(total, ceiling)| total <= ceiling)
    }

    fn make_room(&mut self, key_capacity: usize, weight: usize) -> bool {
        while self.len != 0 && !self.has_room(key_capacity, weight) {
            self.oldest();
        }
        self.has_room(key_capacity, weight)
    }

    fn insert(&mut self, entry: Entry) {
        self.slots[(self.first + self.len) % SLOTS] = Some(entry);
        self.len += 1;
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Fixed FIFO slots retain only exact successful imports. Aggregate encoded key capacities stay
/// within the original one-frame bound, and final memo-owned slots fit one maximum-frame envelope.
/// An incoming candidate is checked under the gate before decoding; this is not a reservation
/// across concurrent misses. Concurrent producer work, scratch and caller-owned results are separate.
/// No entry is retained per body, purpose, height or peer, and no failure is remembered.
#[derive(Default)]
pub(super) struct CheckpointCache {
    entry: Mutex<Entries>,
    #[cfg(test)]
    decode_attempts: std::sync::atomic::AtomicUsize,
}

impl ServiceAuthority {
    #[cfg(test)]
    /// Count attempted canonical imports without changing the optional cache.
    pub(in crate::managed) fn test_checkpoint_import_attempts(&self) -> usize {
        self.checkpoint_cache
            .decode_attempts
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Import an exact independently selected checkpoint, retaining only successful immutable work.
    /// Callers still read and authenticate their original custody before supplying these bytes.
    pub(in crate::managed) fn decode_checkpoint(&self, bytes: &[u8]) -> Result<FinalityVerifier> {
        self.checkpoint_cache
            .decode(bytes, self.config.network_id, self.config.chain.as_str())
    }
}

/// A borrowed pure workspace for one retained prerequisite; never stored in authority or History.
/// Each decode still selects its current original bytes, source and enclosing admission owner.
pub(in crate::managed) struct CheckpointImports<'a, 'v> {
    authority: &'a ServiceAuthority,
    validation: Option<&'v mut EpochValidationScope>,
}

impl<'a, 'v> CheckpointImports<'a, 'v> {
    pub(in crate::managed) fn new(
        authority: &'a ServiceAuthority,
        validation: Option<&'v mut EpochValidationScope>,
    ) -> Self {
        Self {
            authority,
            validation,
        }
    }

    pub(in crate::managed) fn decode(&mut self, bytes: &[u8]) -> Result<FinalityVerifier> {
        self.authority.checkpoint_cache.decode_with_validation(
            bytes,
            self.authority.config.network_id,
            self.authority.config.chain.as_str(),
            self.validation.as_deref_mut(),
        )
    }

    pub(in crate::managed) fn retained_finality(
        &mut self,
        directory: &iroha_fs::PrivateDirectory,
        transaction: &iroha_data_model::transaction::SignedTransaction,
    ) -> Result<Option<crate::managed::native_operation::ManagedTransactionFinality>> {
        crate::managed::native_operation::retained_carrier_using(directory, transaction, |bytes| {
            self.decode(bytes)
        })
    }
}

impl CheckpointCache {
    fn decode(&self, bytes: &[u8], network: NetworkId, chain: &str) -> Result<FinalityVerifier> {
        self.decode_with_validation(bytes, network, chain, None)
    }

    fn decode_with_validation(
        &self,
        bytes: &[u8],
        network: NetworkId,
        chain: &str,
        mut validation: Option<&mut EpochValidationScope>,
    ) -> Result<FinalityVerifier> {
        // A scope entered after warming still receives its original physical admission.
        let active_owner = norito::core::decode_limits_active();
        let weight = retained_import_envelope(bytes.len());
        let (may_store, retained_before_import) = match self.entry.try_lock() {
            Ok(mut selected) => {
                if active_owner {
                    selected.clear();
                    (false, false)
                } else {
                    if let Some(entry) = selected.slots.iter().flatten().find(|entry| {
                        entry.network == network && entry.chain == chain && entry.bytes == bytes
                    }) {
                        // Returning an Arc clone cannot advance this immutable original.
                        return Ok(entry.verifier.clone());
                    }
                    // Account the incoming decoded envelope before its canonical producer runs.
                    let eligible =
                        weight.is_some_and(|weight| selected.make_room(bytes.len(), weight));
                    if !eligible {
                        selected.clear();
                    }
                    (eligible, !selected.is_empty())
                }
            }
            Err(TryLockError::WouldBlock) => (false, false),
            Err(TryLockError::Poisoned(mut poisoned)) => {
                poisoned.get_mut().clear();
                (false, false)
            }
        };
        let offered = self.import(
            bytes,
            network,
            chain,
            if active_owner {
                None
            } else {
                validation.as_deref_mut()
            },
        );
        let verifier = match offered {
            Ok(verifier) => verifier,
            Err(error) => {
                // Mapped errors cannot identify allocator failure. Outside an enclosing owner,
                // release every accessible optional graph and retry the same canonical producer
                // once; the two pure epoch contexts may remain, never an authority verdict.
                let retry = if may_store {
                    match self.entry.try_lock() {
                        Ok(mut selected) => {
                            let retained = retained_before_import || !selected.is_empty();
                            selected.clear();
                            retained
                        }
                        Err(TryLockError::Poisoned(mut poisoned)) => {
                            poisoned.get_mut().clear();
                            false
                        }
                        Err(TryLockError::WouldBlock) => false,
                    }
                } else {
                    false
                };
                if !retry {
                    self.clear_after_refusal();
                    return Err(error);
                }
                drop(error);
                match self.import(bytes, network, chain, validation.as_deref_mut()) {
                    Ok(verifier) => verifier,
                    Err(error) => {
                        // Another caller may have filled the memo during the cold retry. Refusal
                        // clears all reachable slots without waiting for a concurrent holder.
                        self.clear_after_refusal();
                        return Err(error);
                    }
                }
            }
        };
        if may_store {
            // Optional allocation/capacity refusal never replaces a successful producer result.
            let mut image = Vec::new();
            let mut selected_chain = String::new();
            if chain.len() <= MAX_CHAIN_CAPACITY
                && image.try_reserve_exact(bytes.len()).is_ok()
                && selected_chain.try_reserve_exact(chain.len()).is_ok()
                && selected_chain.capacity() <= MAX_CHAIN_CAPACITY
                && image.capacity() <= MAX_CHECKPOINT_BYTES
            {
                image.extend_from_slice(bytes);
                selected_chain.push_str(chain);
                if let Some(weight) = weight
                    && let Ok(mut selected) = self.entry.try_lock()
                {
                    // Recheck capacities and FIFO admission after concurrent cold work. Avoid
                    // duplicate entries when another importer selected the same original.
                    let duplicate = selected.slots.iter().flatten().any(|entry| {
                        entry.network == network && entry.chain == chain && entry.bytes == bytes
                    });
                    if !duplicate && selected.make_room(image.capacity(), weight) {
                        selected.insert(Entry {
                            bytes: image,
                            network,
                            chain: selected_chain,
                            verifier: verifier.clone(),
                            retained_import_envelope: weight,
                        });
                    }
                }
            }
        }
        Ok(verifier)
    }

    fn clear_after_refusal(&self) {
        match self.entry.try_lock() {
            Ok(mut selected) => selected.clear(),
            Err(TryLockError::Poisoned(mut poisoned)) => poisoned.get_mut().clear(),
            Err(TryLockError::WouldBlock) => {}
        }
    }

    // One canonical producer for cold, nonwaiting fallback and optional-memory relief retry.
    // Each actual invocation owns its original admission and is counted independently.
    fn import(
        &self,
        bytes: &[u8],
        network: NetworkId,
        chain: &str,
        validation: Option<&mut EpochValidationScope>,
    ) -> Result<FinalityVerifier> {
        #[cfg(test)]
        self.decode_attempts
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        #[cfg(test)]
        if crate::managed::native_operation::deadline_diagnostics::active() {
            crate::managed::native_operation::deadline_diagnostics::import_attempt(
                bytes,
                network,
                chain,
                self.decode_attempts
                    .load(std::sync::atomic::Ordering::Relaxed),
            );
        }
        decode_checkpoint_with_validation(bytes, network, chain, validation)
    }
}

#[cfg(test)]
#[path = "checkpoint_cache/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "checkpoint_cache/bounded_tests.rs"]
mod bounded_tests;
