//! Resource custody for the authenticated Core source storage boundary.
//!
//! These owners use the existing proof-workspace and ordered-storage ledgers.
//! Starting a session establishes resource identity only. It authenticates no
//! key, source, commitment, Phase23 context or proof, and cannot attest to work
//! performed before that session started. TODO: the eventual source/qPCS
//! orchestration must carry its existing pair into this owner, not start another
//! session mid-proof. Complete proof-work units and whole-process RSS remain open.

use super::{
    global_lookup_statement_v1::vector_arithmetic_plane_openings_v1::ordered_snapshot_v1::resource_budget_v1::{
        OrderedStorageReservationV1, OrderedStorageSessionBudgetV1, StorageBudgetErrorV1,
    },
    rns_native_resource_budget::{
        RnsNativeProofResourceBudgetV1, RnsNativeResourceErrorV1, RnsNativeResourceReservationV1,
    },
    rns_native_source::{
        ZkAmsMkheRnsNativeSourceArenaV1 as Arena, ZkAmsMkheRnsNativeSourceErrorV1 as Error,
        ZkAmsMkheRnsNativeSourceLayoutV1 as Layout, ZK_AMS_MKHE_RNS_NATIVE_SOURCE_TOTAL_FILE_BYTES_V1,
    },
};

/// One move-only resource session containing the canonical workspace/I/O pair.
///
/// This is neither cryptographic authority nor evidence about preexisting
/// allocations. There is no public from-parts, reset, replacement or issuer
/// accessor. Internal orchestration must retain this original pair throughout
/// all future source and proof stages; creating a new session does not continue
/// a previous one.
pub struct ZkAmsMkheRnsNativeSourceResourcesV1 {
    proof: RnsNativeProofResourceBudgetV1,
    storage: OrderedStorageSessionBudgetV1,
}

impl ZkAmsMkheRnsNativeSourceResourcesV1 {
    /// Start a new resource session under the fixed native40 public caps.
    #[must_use]
    pub fn new_v1() -> Self {
        Self {
            proof: RnsNativeProofResourceBudgetV1::default(),
            storage: OrderedStorageSessionBudgetV1::new_v1(),
        }
    }

    /// Consume this session and preadmit the exact source files and named memory.
    ///
    /// The backend supplies its concrete retained-owner and temporary-workspace
    /// sizes. File lengths and all write/seal I/O derive only from the canonical
    /// layout. Admission precedes any file creation. The returned reservation
    /// must be dropped after the admitted files and buffers are destroyed.
    /// Hash/AEAD work is not assigned invented field-operation units.
    ///
    /// # Errors
    /// Rejects invalid layouts, checked-arithmetic overflow, poisoned ledgers,
    /// or any cumulative workspace, spool or I/O ceiling breach.
    pub fn admit_source_v1(
        mut self,
        layout: Layout,
        retained_bytes: u64,
        construction_scratch_bytes: u64,
    ) -> Result<ZkAmsMkheRnsNativeSourceReservationV1, Error> {
        layout.validate()?;
        let memory = self
            .proof
            .reserve_workspace_v1(retained_bytes, construction_scratch_bytes)
            .map_err(map_memory_error_v1)?;
        let files = self
            .storage
            .reserve_files_v1(ZK_AMS_MKHE_RNS_NATIVE_SOURCE_TOTAL_FILE_BYTES_V1)
            .map_err(map_storage_error_v1)?;
        Ok(ZkAmsMkheRnsNativeSourceReservationV1 {
            resources: self,
            memory,
            files,
            written: [0; 2],
            sealed: [false; 2],
        })
    }
}

/// Source files and named memory retained under their original resource pair.
///
/// The backend owns this after its leaf handles so destruction closes both files
/// before releasing live-file credits. Attempted write/seal/read I/O is never
/// refunded, including when the backend reports an error or unwinds.
pub struct ZkAmsMkheRnsNativeSourceReservationV1 {
    resources: ZkAmsMkheRnsNativeSourceResourcesV1,
    memory: RnsNativeResourceReservationV1,
    files: OrderedStorageReservationV1,
    written: [u64; 2],
    sealed: [bool; 2],
}

impl ZkAmsMkheRnsNativeSourceReservationV1 {
    /// Release construction scratch only after its concrete buffers are gone.
    pub fn release_construction_scratch_v1(&mut self) {
        self.memory.release_scratch();
    }

    /// Reserve a child from the original workspace ledger before allocation.
    ///
    /// Returned chunks retain their own reservation even after this owner drops.
    /// The caller must destroy its buffers before dropping this permit.
    ///
    /// # Errors
    /// Rejects arithmetic overflow, poisoned ledgers or cumulative cap exhaustion.
    pub fn reserve_workspace_v1(
        &self,
        retained_bytes: u64,
        scratch_bytes: u64,
    ) -> Result<ZkAmsMkheRnsNativeSourceWorkspaceV1, Error> {
        self.memory
            .reserve_child_workspace_v1(retained_bytes, scratch_bytes)
            .map(|inner| ZkAmsMkheRnsNativeSourceWorkspaceV1 { inner })
            .map_err(map_memory_error_v1)
    }

    /// Check a child belongs to this original session, without exporting identity.
    #[must_use]
    pub fn owns_workspace_v1(&self, workspace: &ZkAmsMkheRnsNativeSourceWorkspaceV1) -> bool {
        workspace.inner.belongs_to_v1(&self.resources.proof)
    }

    /// Debit one exact next record before its leaf write.
    ///
    /// # Errors
    /// Rejects duplicate/out-of-order writes before debit, or ledger poison.
    /// A backend failure after successful debit must poison the whole source.
    pub fn charge_write_v1(&mut self, arena: Arena, slot: u64) -> Result<(), Error> {
        let index = arena_index_v1(arena);
        if self.sealed[index] || slot >= arena.slot_count() || slot != self.written[index] {
            return Err(Error::UnexpectedWrite);
        }
        self.files
            .charge_reserved_io_v1(record_bytes_v1(arena))
            .map_err(map_storage_error_v1)?;
        self.written[index] += 1;
        Ok(())
    }

    /// Debit the complete arena before authenticating its seal pass.
    ///
    /// # Errors
    /// Rejects incomplete/repeated seals before debit or a poisoned ledger.
    pub fn charge_seal_v1(&mut self, arena: Arena) -> Result<(), Error> {
        let index = arena_index_v1(arena);
        if self.sealed[index] || self.written[index] != arena.slot_count() {
            return Err(Error::Incomplete);
        }
        self.files
            .charge_reserved_io_v1(arena.file_bytes())
            .map_err(map_storage_error_v1)?;
        self.sealed[index] = true;
        Ok(())
    }

    /// Confirm both exact write/seal schedules spent their original reservation.
    ///
    /// # Errors
    /// Rejects incomplete schedules or a poisoned ledger.
    pub fn require_sealed_v1(&self) -> Result<(), Error> {
        if self.sealed != [true; 2] {
            return Err(Error::Incomplete);
        }
        self.files.require_sealed_v1().map_err(map_storage_error_v1)
    }

    /// Debit one authenticated read before entering the immutable leaf.
    ///
    /// The backend first reserves workspace; a workspace refusal therefore
    /// cannot consume this I/O budget or poison either snapshot.
    ///
    /// # Errors
    /// Rejects incomplete sources, out-of-range slots, ledger poison or the
    /// cumulative I/O ceiling. Successful debit is never refunded.
    pub fn charge_read_v1(&mut self, arena: Arena, slot: u64) -> Result<(), Error> {
        self.require_sealed_v1()?;
        if slot >= arena.slot_count() {
            return Err(Error::UnexpectedWrite);
        }
        self.files
            .charge_read_io_v1(record_bytes_v1(arena))
            .map_err(map_storage_error_v1)
    }
}

/// Move-only named workspace carried by one actual backend allocation.
///
/// Its original ledger survives destruction of the parent source. The backend
/// destroys its secret allocation before dropping this value.
pub struct ZkAmsMkheRnsNativeSourceWorkspaceV1 {
    inner: RnsNativeResourceReservationV1,
}

impl ZkAmsMkheRnsNativeSourceWorkspaceV1 {
    /// Release scratch after the leaf returns and all operation scratch is gone.
    pub fn release_scratch_v1(&mut self) {
        self.inner.release_scratch();
    }
}

const fn arena_index_v1(arena: Arena) -> usize {
    match arena {
        Arena::Main => 0,
        Arena::Nonce => 1,
    }
}

const fn record_bytes_v1(arena: Arena) -> u64 {
    arena.plaintext_bytes() + 16
}

fn map_memory_error_v1(error: RnsNativeResourceErrorV1) -> Error {
    match error {
        RnsNativeResourceErrorV1::LedgerPoisoned => Error::Poisoned,
        RnsNativeResourceErrorV1::ArithmeticOverflow
        | RnsNativeResourceErrorV1::WorkLimit
        | RnsNativeResourceErrorV1::WorkspaceLimit => Error::ResourceCeilingExceeded,
    }
}

fn map_storage_error_v1(error: StorageBudgetErrorV1) -> Error {
    match error {
        StorageBudgetErrorV1::Poisoned => Error::Poisoned,
        StorageBudgetErrorV1::Order => Error::Incomplete,
        StorageBudgetErrorV1::Overflow
        | StorageBudgetErrorV1::SpoolLimit
        | StorageBudgetErrorV1::IoLimit => Error::ResourceCeilingExceeded,
    }
}

#[cfg(test)]
#[path = "rns_native_source_resources_tests.rs"]
mod tests;
