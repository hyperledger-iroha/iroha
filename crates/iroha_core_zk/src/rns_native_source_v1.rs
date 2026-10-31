//! Core-owned confidential-spool adapter for the replacement MKHE RNS source.
//!
//! Keeping this adapter in `iroha_core_zk` preserves dependency direction: the
//! proof crate defines a backend-neutral move-only interface, while Core owns
//! both the proof crate and `iroha_crypto`'s already-locked XChaCha20-Poly1305
//! spool implementation.  No pathname, key, raw snapshot, or detached digest
//! constructor crosses the boundary.

use std::path::Path;

use iroha_crypto::confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolErrorV1, ConfidentialSpoolLayoutV1,
    ConfidentialSpoolSnapshotV1, ConfidentialSpoolWriterV1,
};
use iroha_zkp_halo2::vega::{
    ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1, ZkAmsMkheRnsNativeSecretChunkV1,
    ZkAmsMkheRnsNativeSourceArenaV1, ZkAmsMkheRnsNativeSourceErrorV1,
    ZkAmsMkheRnsNativeSourceLayoutV1, ZkAmsMkheRnsNativeSourceReservationV1,
    ZkAmsMkheRnsNativeSourceResourcesV1, ZkAmsMkheRnsNativeSourceSnapshotV1,
    ZkAmsMkheRnsNativeSourceWorkspaceV1, ZkAmsMkheRnsNativeSourceWriterV1,
};

/// Core-owned zeroizing secret record.
///
/// The inner owner is consumed directly by the crypto spool; no second
/// plaintext allocation is introduced by this adapter.
pub struct CoreZkAmsMkheRnsNativeSecretChunkV1 {
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
    inner: Option<ConfidentialSpoolChunkV1>,
    // Field order destroys the secret leaf before releasing its workspace.
    workspace: ZkAmsMkheRnsNativeSourceWorkspaceV1,
}

impl CoreZkAmsMkheRnsNativeSecretChunkV1 {
    fn new(
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        admission: &ZkAmsMkheRnsNativeSourceReservationV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeSourceErrorV1> {
        let workspace = admission.reserve_workspace_v1(chunk_named_bytes_v1(arena)?, 0)?;
        let inner = ConfidentialSpoolChunkV1::new_zeroed_v1(arena.plaintext_bytes())
            .map_err(map_spool_error_v1)?;
        Ok(Self {
            arena,
            inner: Some(inner),
            workspace,
        })
    }
}

impl ZkAmsMkheRnsNativeSecretChunkV1 for CoreZkAmsMkheRnsNativeSecretChunkV1 {
    fn arena(&self) -> ZkAmsMkheRnsNativeSourceArenaV1 {
        self.arena
    }

    fn as_slice(&self) -> &[u8] {
        self.inner
            .as_ref()
            .expect("live Core RNS-native secret chunk")
            .as_slice_v1()
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.inner
            .as_mut()
            .expect("live Core RNS-native secret chunk")
            .as_mut_slice_v1()
    }
}

impl Drop for CoreZkAmsMkheRnsNativeSecretChunkV1 {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.as_mut() {
            inner.as_mut_slice_v1().fill(0);
        }
        #[cfg(test)]
        CHUNK_CLEAR_EVENTS_V1.with(|events| {
            events.borrow_mut().push(
                self.inner
                    .as_ref()
                    .map(|inner| inner.as_slice_v1().iter().all(|byte| *byte == 0)),
            );
        });
    }
}

#[cfg(test)]
std::thread_local! {
    static CHUNK_CLEAR_EVENTS_V1: std::cell::RefCell<Vec<Option<bool>>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Core-owned pair of strict sequential confidential-spool writers.
pub struct CoreZkAmsMkheRnsNativeSourceWriterV1 {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    main: Option<ConfidentialSpoolWriterV1>,
    nonce: Option<ConfidentialSpoolWriterV1>,
    // Both leaves are destroyed before their original resource reservation.
    admission: ZkAmsMkheRnsNativeSourceReservationV1,
    #[cfg(test)]
    injected_write_error: Option<ConfidentialSpoolErrorV1>,
    #[cfg(test)]
    panic_on_next_write: bool,
}

/// Core-owned pair of immutable authenticated confidential-spool snapshots.
pub struct CoreZkAmsMkheRnsNativeSourceSnapshotV1 {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    main: Option<ConfidentialSpoolSnapshotV1>,
    nonce: Option<ConfidentialSpoolSnapshotV1>,
    main_snapshot_digest: [u8; 32],
    nonce_snapshot_digest: [u8; 32],
    // Immutable leaves precede the reservation in field destruction order.
    admission: ZkAmsMkheRnsNativeSourceReservationV1,
    #[cfg(test)]
    injected_read_error: Option<ConfidentialSpoolErrorV1>,
    #[cfg(test)]
    panic_on_next_read: bool,
}

impl CoreZkAmsMkheRnsNativeSourceSnapshotV1 {
    fn poison_pair_v1(&mut self) {
        drop(self.main.take());
        drop(self.nonce.take());
        // The trait's structural receipt constructor rejects zero identities.
        // A destroyed pair cannot issue another live-snapshot receipt; already
        // copied public receipts remain explicitly non-authorizing.
        self.main_snapshot_digest = [0; 32];
        self.nonce_snapshot_digest = [0; 32];
    }
}

/// Unwind guard which makes the two-spool snapshot one fail-stop owner.
/// Successful and pure-preflight returns disarm it explicitly; every panic or
/// poisoning raw read error drops both sibling files and keys.
struct CoreZkAmsMkheRnsNativePairReadGuardV1<'a> {
    snapshot: &'a mut CoreZkAmsMkheRnsNativeSourceSnapshotV1,
    armed: bool,
}

impl<'a> CoreZkAmsMkheRnsNativePairReadGuardV1<'a> {
    fn new(snapshot: &'a mut CoreZkAmsMkheRnsNativeSourceSnapshotV1) -> Self {
        Self {
            snapshot,
            armed: true,
        }
    }

    fn read_selected_v1(
        &mut self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        slot: u64,
        expected_context: [u8; 32],
    ) -> Result<ConfidentialSpoolChunkV1, ConfidentialSpoolErrorV1> {
        match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => self
                .snapshot
                .main
                .as_mut()
                .ok_or(ConfidentialSpoolErrorV1::Poisoned)?
                .read_slot_v1(slot, expected_context),
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => self
                .snapshot
                .nonce
                .as_mut()
                .ok_or(ConfidentialSpoolErrorV1::Poisoned)?
                .read_slot_v1(slot, expected_context),
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for CoreZkAmsMkheRnsNativePairReadGuardV1<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.snapshot.poison_pair_v1();
        }
    }
}

impl CoreZkAmsMkheRnsNativeSourceWriterV1 {
    /// Begin the canonical authenticated source arenas with one consumed session.
    ///
    /// Files and write/seal I/O are preadmitted before either file is created.
    /// Original workspace and storage identity remains with the writer, snapshot
    /// and every returned chunk. This constructs genuine local encrypted storage;
    /// it does not authenticate source/key/Phase23 correspondence or make a proof
    /// available. A freshly started resource session cannot account for earlier
    /// allocations. TODO: carry the same session through future source/qPCS joins.
    ///
    /// # Errors
    /// Rejects invalid layouts, original cumulative resource ceilings, unsafe
    /// directories, unavailable entropy/platforms or crypto-spool failures.
    pub fn begin(
        directory: impl AsRef<Path>,
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        resources: ZkAmsMkheRnsNativeSourceResourcesV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeSourceErrorV1> {
        layout.validate()?;
        let main_layout = spool_layout_v1(&layout, ZkAmsMkheRnsNativeSourceArenaV1::Main)?;
        let nonce_layout = spool_layout_v1(&layout, ZkAmsMkheRnsNativeSourceArenaV1::Nonce)?;
        let retained = source_named_bytes_v1(&main_layout, &nonce_layout)?;
        let scratch = u64::try_from(
            main_layout
                .named_operation_workspace_bytes_v1()
                .max(nonce_layout.named_operation_workspace_bytes_v1()),
        )
        .map_err(|_| ZkAmsMkheRnsNativeSourceErrorV1::ResourceCeilingExceeded)?;
        let mut admission = resources.admit_source_v1(layout, retained, scratch)?;
        let main = ConfidentialSpoolWriterV1::create_in_v1(directory.as_ref(), main_layout)
            .map_err(map_spool_error_v1)?;
        let nonce = ConfidentialSpoolWriterV1::create_in_v1(directory.as_ref(), nonce_layout)
            .map_err(map_spool_error_v1)?;
        admission.release_construction_scratch_v1();
        Ok(Self {
            layout,
            main: Some(main),
            nonce: Some(nonce),
            admission,
            #[cfg(test)]
            injected_write_error: None,
            #[cfg(test)]
            panic_on_next_write: false,
        })
    }

    fn poison_pair_v1(&mut self) {
        drop(self.main.take());
        drop(self.nonce.take());
    }
}

/// Drop both writer siblings on an operational failure or unwind.
struct CoreZkAmsMkheRnsNativePairWriteGuardV1<'a> {
    writer: &'a mut CoreZkAmsMkheRnsNativeSourceWriterV1,
    armed: bool,
}

impl CoreZkAmsMkheRnsNativePairWriteGuardV1<'_> {
    fn write_selected_v1(
        &mut self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        slot: u64,
        inner: ConfidentialSpoolChunkV1,
    ) -> Result<(), ConfidentialSpoolErrorV1> {
        match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => self.writer.main.as_mut(),
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => self.writer.nonce.as_mut(),
        }
        .ok_or(ConfidentialSpoolErrorV1::Poisoned)?
        .write_slot_v1(slot, inner)
    }
}

impl Drop for CoreZkAmsMkheRnsNativePairWriteGuardV1<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.writer.poison_pair_v1();
        }
    }
}

impl ZkAmsMkheRnsNativeSourceWriterV1 for CoreZkAmsMkheRnsNativeSourceWriterV1 {
    type Chunk = CoreZkAmsMkheRnsNativeSecretChunkV1;
    type Snapshot = CoreZkAmsMkheRnsNativeSourceSnapshotV1;

    fn allocate_chunk(
        &self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
        if self.main.is_none() || self.nonce.is_none() {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned);
        }
        CoreZkAmsMkheRnsNativeSecretChunkV1::new(arena, &self.admission)
    }

    fn write_slot(
        &mut self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        slot: u64,
        mut chunk: Self::Chunk,
    ) -> Result<(), ZkAmsMkheRnsNativeSourceErrorV1> {
        if self.main.is_none() || self.nonce.is_none() {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned);
        }
        if chunk.arena() != arena
            || chunk.as_slice().len() as u64 != arena.plaintext_bytes()
            || !self.admission.owns_workspace_v1(&chunk.workspace)
        {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite);
        }
        let _operation = self
            .admission
            .reserve_workspace_v1(0, operation_scratch_v1(&self.layout, arena)?)?;
        #[cfg(test)]
        let injected = self.injected_write_error.take();
        #[cfg(test)]
        let panic_on_next_write = core::mem::take(&mut self.panic_on_next_write);
        self.admission.charge_write_v1(arena, slot)?;
        let mut guard = CoreZkAmsMkheRnsNativePairWriteGuardV1 {
            writer: self,
            armed: true,
        };
        // Keep `chunk` and its reservation alive until the leaf has consumed and
        // destroyed the transferred allocation, including during unwinding. Every
        // fallible step after debit is inside the pair's fail-stop guard.
        let inner = chunk
            .inner
            .take()
            .ok_or(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned)?;
        #[cfg(test)]
        assert!(
            !panic_on_next_write,
            "injected Core RNS-native pair write unwind"
        );
        #[cfg(test)]
        let result = if let Some(error) = injected {
            drop(inner);
            Err(error)
        } else {
            guard.write_selected_v1(arena, slot, inner)
        };
        #[cfg(not(test))]
        let result = guard.write_selected_v1(arena, slot, inner);
        if result.is_ok() {
            guard.armed = false;
        }
        drop(guard);
        // Explicitly drop only after the leaf returned. No owner is detached
        // from the reservation while it can still contain secret plaintext.
        drop(chunk);
        result.map_err(map_spool_error_v1)
    }

    fn seal(mut self) -> Result<Self::Snapshot, ZkAmsMkheRnsNativeSourceErrorV1> {
        if self.main.is_none() || self.nonce.is_none() {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned);
        }
        let main_layout = spool_layout_v1(&self.layout, ZkAmsMkheRnsNativeSourceArenaV1::Main)?;
        let nonce_layout = spool_layout_v1(&self.layout, ZkAmsMkheRnsNativeSourceArenaV1::Nonce)?;
        let scratch = u64::try_from(
            main_layout
                .named_operation_workspace_bytes_v1()
                .max(nonce_layout.named_operation_workspace_bytes_v1()),
        )
        .map_err(|_| ZkAmsMkheRnsNativeSourceErrorV1::ResourceCeilingExceeded)?;
        let operation = self.admission.reserve_workspace_v1(0, scratch)?;
        self.admission
            .charge_seal_v1(ZkAmsMkheRnsNativeSourceArenaV1::Main)?;
        let main = self
            .main
            .take()
            .ok_or(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned)?
            .seal_v1()
            .map_err(map_spool_error_v1)?;
        self.admission
            .charge_seal_v1(ZkAmsMkheRnsNativeSourceArenaV1::Nonce)?;
        let nonce = self
            .nonce
            .take()
            .ok_or(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned)?
            .seal_v1()
            .map_err(map_spool_error_v1)?;
        self.admission.require_sealed_v1()?;
        let main_snapshot_digest = *main.snapshot_digest_v1();
        let nonce_snapshot_digest = *nonce.snapshot_digest_v1();
        drop(operation);
        Ok(CoreZkAmsMkheRnsNativeSourceSnapshotV1 {
            layout: self.layout,
            main: Some(main),
            nonce: Some(nonce),
            main_snapshot_digest,
            nonce_snapshot_digest,
            admission: self.admission,
            #[cfg(test)]
            injected_read_error: None,
            #[cfg(test)]
            panic_on_next_read: false,
        })
    }
}

impl ZkAmsMkheRnsNativeSourceSnapshotV1 for CoreZkAmsMkheRnsNativeSourceSnapshotV1 {
    type Chunk = CoreZkAmsMkheRnsNativeSecretChunkV1;

    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        self.layout
    }

    fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; 32] {
        match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => self.main_snapshot_digest,
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => self.nonce_snapshot_digest,
        }
    }

    fn read_slot(
        &mut self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        slot: u64,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
        if self.main.is_none() || self.nonce.is_none() {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned);
        }
        if slot >= arena.slot_count() {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite);
        }
        let mut workspace = self.admission.reserve_workspace_v1(
            chunk_named_bytes_v1(arena)?,
            operation_scratch_v1(&self.layout, arena)?,
        )?;
        let expected_context = self.layout.arena_context_digest(arena);
        #[cfg(test)]
        let injected_read_error = self.injected_read_error.take();
        #[cfg(test)]
        let panic_on_next_read = core::mem::take(&mut self.panic_on_next_read);
        // All capacity/context preflights precede debit. The pair guard is armed
        // immediately after successful debit, before any leaf processing.
        self.admission.charge_read_v1(arena, slot)?;
        let mut guard = CoreZkAmsMkheRnsNativePairReadGuardV1::new(self);
        #[cfg(test)]
        assert!(
            !panic_on_next_read,
            "injected Core RNS-native pair read unwind"
        );
        #[cfg(test)]
        let result = injected_read_error.map_or_else(
            || guard.read_selected_v1(arena, slot, expected_context),
            Err,
        );
        #[cfg(not(test))]
        let result = guard.read_selected_v1(arena, slot, expected_context);
        let inner = match result {
            Ok(inner) => {
                guard.disarm();
                inner
            }
            Err(error) => {
                if !read_error_poisons_v1(error) {
                    guard.disarm();
                }
                let mapped = map_spool_error_v1(error);
                drop(guard);
                return Err(mapped);
            }
        };
        drop(guard);
        workspace.release_scratch_v1();
        Ok(CoreZkAmsMkheRnsNativeSecretChunkV1 {
            arena,
            inner: Some(inner),
            workspace,
        })
    }
}

impl ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1 for CoreZkAmsMkheRnsNativeSourceSnapshotV1 {}

fn read_error_poisons_v1(error: ConfidentialSpoolErrorV1) -> bool {
    match error {
        ConfidentialSpoolErrorV1::SlotOutOfRange { .. }
        | ConfidentialSpoolErrorV1::ContextDigestMismatch => false,
        ConfidentialSpoolErrorV1::EmptyLayout
        | ConfidentialSpoolErrorV1::EmptyChunk
        | ConfidentialSpoolErrorV1::GeometryOverflow
        | ConfidentialSpoolErrorV1::AddressSpaceExceeded
        | ConfidentialSpoolErrorV1::InertContextDigest
        | ConfidentialSpoolErrorV1::LimitExceeded(_)
        | ConfidentialSpoolErrorV1::CipherMessageLimit
        | ConfidentialSpoolErrorV1::Allocation(_)
        | ConfidentialSpoolErrorV1::UnsupportedPlatform
        | ConfidentialSpoolErrorV1::FileOperation { .. }
        | ConfidentialSpoolErrorV1::UnsafeTemporaryFile
        | ConfidentialSpoolErrorV1::TemporaryFileIdentityMismatch
        | ConfidentialSpoolErrorV1::UnsafeDetachedFile
        | ConfidentialSpoolErrorV1::EntropyUnavailable
        | ConfidentialSpoolErrorV1::WeakEntropy(_)
        | ConfidentialSpoolErrorV1::UnexpectedWriteSlot { .. }
        | ConfidentialSpoolErrorV1::ChunkLength { .. }
        | ConfidentialSpoolErrorV1::Incomplete { .. }
        | ConfidentialSpoolErrorV1::FileLength { .. }
        | ConfidentialSpoolErrorV1::Encryption
        | ConfidentialSpoolErrorV1::Authentication
        | ConfidentialSpoolErrorV1::Poisoned => true,
    }
}

fn spool_layout_v1(
    layout: &ZkAmsMkheRnsNativeSourceLayoutV1,
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
) -> Result<ConfidentialSpoolLayoutV1, ZkAmsMkheRnsNativeSourceErrorV1> {
    ConfidentialSpoolLayoutV1::new_v1(
        arena.slot_count(),
        arena.plaintext_bytes(),
        layout.arena_context_digest(arena),
    )
    .map_err(map_spool_error_v1)
}

// This is named payload accounting, not allocator, dependency scratch or RSS.
// The crypto retained helper conservatively counts handles already embedded in
// the Core owner, so this sum can overcount but cannot omit either key payload.
fn source_named_bytes_v1(
    main: &ConfidentialSpoolLayoutV1,
    nonce: &ConfidentialSpoolLayoutV1,
) -> Result<u64, ZkAmsMkheRnsNativeSourceErrorV1> {
    let owner = size_of::<CoreZkAmsMkheRnsNativeSourceWriterV1>()
        .max(size_of::<CoreZkAmsMkheRnsNativeSourceSnapshotV1>());
    let bytes = owner
        .checked_add(main.named_retained_bytes_v1())
        .and_then(|bytes| bytes.checked_add(nonce.named_retained_bytes_v1()))
        .ok_or(ZkAmsMkheRnsNativeSourceErrorV1::ResourceCeilingExceeded)?;
    u64::try_from(bytes).map_err(|_| ZkAmsMkheRnsNativeSourceErrorV1::ResourceCeilingExceeded)
}

fn chunk_named_bytes_v1(
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
) -> Result<u64, ZkAmsMkheRnsNativeSourceErrorV1> {
    arena
        .plaintext_bytes()
        .checked_add(size_of::<CoreZkAmsMkheRnsNativeSecretChunkV1>() as u64)
        .ok_or(ZkAmsMkheRnsNativeSourceErrorV1::ResourceCeilingExceeded)
}

fn operation_scratch_v1(
    layout: &ZkAmsMkheRnsNativeSourceLayoutV1,
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
) -> Result<u64, ZkAmsMkheRnsNativeSourceErrorV1> {
    let layout = spool_layout_v1(layout, arena)?;
    let whole = u64::try_from(layout.named_operation_workspace_bytes_v1())
        .map_err(|_| ZkAmsMkheRnsNativeSourceErrorV1::ResourceCeilingExceeded)?;
    // The returned/consumed chunk and its complete Core owner are independently
    // retained; all other leaf operation payloads remain temporary scratch.
    whole
        .checked_sub(arena.plaintext_bytes())
        .and_then(|bytes| bytes.checked_sub(size_of::<ConfidentialSpoolChunkV1>() as u64))
        .ok_or(ZkAmsMkheRnsNativeSourceErrorV1::ResourceCeilingExceeded)
}

fn map_spool_error_v1(error: ConfidentialSpoolErrorV1) -> ZkAmsMkheRnsNativeSourceErrorV1 {
    match error {
        ConfidentialSpoolErrorV1::EmptyLayout
        | ConfidentialSpoolErrorV1::EmptyChunk
        | ConfidentialSpoolErrorV1::InertContextDigest => {
            ZkAmsMkheRnsNativeSourceErrorV1::InvalidLayout
        }
        ConfidentialSpoolErrorV1::GeometryOverflow
        | ConfidentialSpoolErrorV1::AddressSpaceExceeded
        | ConfidentialSpoolErrorV1::LimitExceeded(_)
        | ConfidentialSpoolErrorV1::CipherMessageLimit => {
            ZkAmsMkheRnsNativeSourceErrorV1::ResourceCeilingExceeded
        }
        ConfidentialSpoolErrorV1::Allocation(_) => ZkAmsMkheRnsNativeSourceErrorV1::Allocation,
        ConfidentialSpoolErrorV1::UnsupportedPlatform
        | ConfidentialSpoolErrorV1::EntropyUnavailable
        | ConfidentialSpoolErrorV1::WeakEntropy(_) => {
            ZkAmsMkheRnsNativeSourceErrorV1::BackendUnavailable
        }
        ConfidentialSpoolErrorV1::FileOperation { .. }
        | ConfidentialSpoolErrorV1::UnsafeTemporaryFile
        | ConfidentialSpoolErrorV1::TemporaryFileIdentityMismatch
        | ConfidentialSpoolErrorV1::UnsafeDetachedFile
        | ConfidentialSpoolErrorV1::FileLength { .. } => ZkAmsMkheRnsNativeSourceErrorV1::Storage,
        ConfidentialSpoolErrorV1::SlotOutOfRange { .. }
        | ConfidentialSpoolErrorV1::UnexpectedWriteSlot { .. }
        | ConfidentialSpoolErrorV1::ChunkLength { .. } => {
            ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite
        }
        ConfidentialSpoolErrorV1::ContextDigestMismatch => {
            ZkAmsMkheRnsNativeSourceErrorV1::InvalidContext
        }
        ConfidentialSpoolErrorV1::Encryption | ConfidentialSpoolErrorV1::Authentication => {
            ZkAmsMkheRnsNativeSourceErrorV1::Authentication
        }
        ConfidentialSpoolErrorV1::Incomplete { .. } => ZkAmsMkheRnsNativeSourceErrorV1::Incomplete,
        ConfidentialSpoolErrorV1::Poisoned => ZkAmsMkheRnsNativeSourceErrorV1::Poisoned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_zkp_halo2::vega::{
        zk_ams_mkhe_rns_native_profile_v1, zk_ams_mkhe_rns_native_topology_v1,
    };

    fn source_layout() -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        let profile = zk_ams_mkhe_rns_native_profile_v1().expect("profile");
        let topology = zk_ams_mkhe_rns_native_topology_v1().expect("topology");
        ZkAmsMkheRnsNativeSourceLayoutV1::new(
            profile.profile_digest,
            topology.topology_digest,
            [0x51; 32],
            [0x52; 32],
            [0x53; 32],
        )
        .expect("source layout")
    }

    #[cfg(unix)]
    fn reduced_spool_v1(
        directory: &std::path::Path,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        context_digest: [u8; 32],
        tags: [u8; 2],
    ) -> ConfidentialSpoolSnapshotV1 {
        let layout = ConfidentialSpoolLayoutV1::new_v1(2, arena.plaintext_bytes(), context_digest)
            .expect("reduced conformance layout");
        let mut writer = ConfidentialSpoolWriterV1::create_in_v1(directory, layout)
            .expect("reduced conformance writer");
        for (slot, tag) in tags.into_iter().enumerate() {
            let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(arena.plaintext_bytes())
                .expect("reduced conformance chunk");
            chunk.as_mut_slice_v1().fill(tag);
            writer
                .write_slot_v1(slot as u64, chunk)
                .expect("reduced conformance write");
        }
        writer.seal_v1().expect("reduced conformance seal")
    }

    #[cfg(unix)]
    fn reduced_snapshot_v1(wrong_main_context: bool) -> CoreZkAmsMkheRnsNativeSourceSnapshotV1 {
        let directory = tempfile::tempdir().expect("reduced conformance directory");
        let layout = source_layout();
        let main_layout = spool_layout_v1(&layout, ZkAmsMkheRnsNativeSourceArenaV1::Main).unwrap();
        let nonce_layout =
            spool_layout_v1(&layout, ZkAmsMkheRnsNativeSourceArenaV1::Nonce).unwrap();
        let mut admission = ZkAmsMkheRnsNativeSourceResourcesV1::new_v1()
            .admit_source_v1(
                layout,
                source_named_bytes_v1(&main_layout, &nonce_layout).unwrap(),
                main_layout.named_operation_workspace_bytes_v1() as u64,
            )
            .unwrap();
        // This reduced storage fixture conservatively precharges the complete
        // canonical schedule before any leaf creation/write/seal. It is only a
        // small authenticated-read fixture, never a canonical source producer.
        for arena in [
            ZkAmsMkheRnsNativeSourceArenaV1::Main,
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
        ] {
            for slot in 0..arena.slot_count() {
                admission.charge_write_v1(arena, slot).unwrap();
            }
            admission.charge_seal_v1(arena).unwrap();
        }
        let mut main_context = layout.arena_context_digest(ZkAmsMkheRnsNativeSourceArenaV1::Main);
        if wrong_main_context {
            main_context[0] ^= 1;
        }
        let main = reduced_spool_v1(
            directory.path(),
            ZkAmsMkheRnsNativeSourceArenaV1::Main,
            main_context,
            [0xA0, 0xA1],
        );
        let nonce = reduced_spool_v1(
            directory.path(),
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
            layout.arena_context_digest(ZkAmsMkheRnsNativeSourceArenaV1::Nonce),
            [0xB0, 0xB1],
        );
        let main_snapshot_digest = *main.snapshot_digest_v1();
        let nonce_snapshot_digest = *nonce.snapshot_digest_v1();
        admission.release_construction_scratch_v1();
        CoreZkAmsMkheRnsNativeSourceSnapshotV1 {
            layout,
            main: Some(main),
            nonce: Some(nonce),
            main_snapshot_digest,
            nonce_snapshot_digest,
            admission,
            injected_read_error: None,
            panic_on_next_read: false,
        }
    }

    fn assert_repeatable_snapshot_v1<T: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1>() {}

    #[cfg(unix)]
    #[test]
    fn snapshot_is_repeatable_out_of_order_and_receipt_stable() {
        assert_repeatable_snapshot_v1::<CoreZkAmsMkheRnsNativeSourceSnapshotV1>();
        let mut snapshot = reduced_snapshot_v1(false);
        let layout = snapshot.layout();
        let receipt = snapshot.structural_receipt().expect("initial receipt");
        let main_digest = snapshot.snapshot_digest(ZkAmsMkheRnsNativeSourceArenaV1::Main);
        let nonce_digest = snapshot.snapshot_digest(ZkAmsMkheRnsNativeSourceArenaV1::Nonce);

        for (arena, slot, tag) in [
            (ZkAmsMkheRnsNativeSourceArenaV1::Main, 1, 0xA1),
            (ZkAmsMkheRnsNativeSourceArenaV1::Nonce, 0, 0xB0),
            (ZkAmsMkheRnsNativeSourceArenaV1::Main, 0, 0xA0),
            (ZkAmsMkheRnsNativeSourceArenaV1::Main, 1, 0xA1),
            (ZkAmsMkheRnsNativeSourceArenaV1::Nonce, 1, 0xB1),
            (ZkAmsMkheRnsNativeSourceArenaV1::Nonce, 0, 0xB0),
        ] {
            let chunk = snapshot.read_slot(arena, slot).expect("repeatable read");
            assert_eq!(chunk.arena(), arena);
            assert_eq!(chunk.as_slice().len() as u64, arena.plaintext_bytes());
            assert!(chunk.as_slice().iter().all(|byte| *byte == tag));
            assert_eq!(snapshot.layout(), layout);
            assert_eq!(snapshot.structural_receipt(), Ok(receipt));
            assert_eq!(
                snapshot.snapshot_digest(ZkAmsMkheRnsNativeSourceArenaV1::Main),
                main_digest
            );
            assert_eq!(
                snapshot.snapshot_digest(ZkAmsMkheRnsNativeSourceArenaV1::Nonce),
                nonce_digest
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn read_preflights_do_not_poison_the_pair() {
        let mut snapshot = reduced_snapshot_v1(false);
        let receipt = snapshot.structural_receipt().expect("initial receipt");
        assert!(matches!(
            snapshot.read_slot(
                ZkAmsMkheRnsNativeSourceArenaV1::Main,
                ZkAmsMkheRnsNativeSourceArenaV1::Main.slot_count(),
            ),
            Err(ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite)
        ));
        assert!(matches!(
            snapshot.read_slot(
                ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
                ZkAmsMkheRnsNativeSourceArenaV1::Nonce.slot_count(),
            ),
            Err(ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite)
        ));
        assert!(
            snapshot
                .read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 0)
                .is_ok()
        );
        assert_eq!(snapshot.structural_receipt(), Ok(receipt));
        assert!(snapshot.main.is_some() && snapshot.nonce.is_some());
    }

    #[cfg(unix)]
    #[test]
    fn context_preflight_is_distinct_from_record_authentication() {
        let mut snapshot = reduced_snapshot_v1(true);
        assert!(matches!(
            snapshot.read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 0),
            Err(ZkAmsMkheRnsNativeSourceErrorV1::InvalidContext)
        ));
        assert!(snapshot.main.is_some() && snapshot.nonce.is_some());
        assert!(
            snapshot
                .read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Nonce, 1)
                .is_ok()
        );
        assert_eq!(
            map_spool_error_v1(ConfidentialSpoolErrorV1::ContextDigestMismatch),
            ZkAmsMkheRnsNativeSourceErrorV1::InvalidContext
        );
        assert_eq!(
            map_spool_error_v1(ConfidentialSpoolErrorV1::Authentication),
            ZkAmsMkheRnsNativeSourceErrorV1::Authentication
        );
    }

    #[cfg(unix)]
    #[test]
    fn operational_or_authentication_failure_poison_both_arenas() {
        for raw_error in [
            ConfidentialSpoolErrorV1::Authentication,
            ConfidentialSpoolErrorV1::FileOperation {
                operation: "read",
                kind: std::io::ErrorKind::Other,
            },
        ] {
            let mut snapshot = reduced_snapshot_v1(false);
            snapshot.injected_read_error = Some(raw_error);
            assert!(matches!(
                snapshot.read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 0),
                Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication
                    | ZkAmsMkheRnsNativeSourceErrorV1::Storage)
            ));
            assert!(snapshot.main.is_none() && snapshot.nonce.is_none());
            assert!(snapshot.structural_receipt().is_err());
            for arena in [
                ZkAmsMkheRnsNativeSourceArenaV1::Main,
                ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
            ] {
                assert!(matches!(
                    snapshot.read_slot(arena, 0),
                    Err(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned)
                ));
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn read_unwind_poison_drops_both_arenas() {
        let mut snapshot = reduced_snapshot_v1(false);
        snapshot.panic_on_next_read = true;
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = snapshot.read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 0);
        }));
        assert!(unwind.is_err());
        assert!(snapshot.main.is_none() && snapshot.nonce.is_none());
        assert!(snapshot.structural_receipt().is_err());
        for arena in [
            ZkAmsMkheRnsNativeSourceArenaV1::Main,
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
        ] {
            assert!(matches!(
                snapshot.read_slot(arena, 0),
                Err(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned)
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn consuming_begin_uses_exact_arenas_and_rejects_cross_arena_chunks() {
        let directory = tempfile::tempdir().expect("private temporary directory");
        let mut writer = CoreZkAmsMkheRnsNativeSourceWriterV1::begin(
            directory.path(),
            source_layout(),
            ZkAmsMkheRnsNativeSourceResourcesV1::new_v1(),
        )
        .expect("source writer");

        let mut main = writer
            .allocate_chunk(ZkAmsMkheRnsNativeSourceArenaV1::Main)
            .expect("main chunk");
        main.as_mut_slice()[0] = 7;
        writer
            .write_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 0, main)
            .expect("first main write");

        let nonce = writer
            .allocate_chunk(ZkAmsMkheRnsNativeSourceArenaV1::Nonce)
            .expect("nonce chunk");
        assert_eq!(
            writer.write_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 1, nonce),
            Err(ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite)
        );
        let nonce = writer
            .allocate_chunk(ZkAmsMkheRnsNativeSourceArenaV1::Nonce)
            .expect("replacement nonce chunk");
        writer
            .write_slot(ZkAmsMkheRnsNativeSourceArenaV1::Nonce, 0, nonce)
            .expect("first nonce write");
        assert!(matches!(
            writer.seal(),
            Err(ZkAmsMkheRnsNativeSourceErrorV1::Incomplete)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn consuming_begin_preserves_exact_next_slot_and_path_free_errors() {
        let directory = tempfile::tempdir().expect("private temporary directory");
        let mut writer = CoreZkAmsMkheRnsNativeSourceWriterV1::begin(
            directory.path(),
            source_layout(),
            ZkAmsMkheRnsNativeSourceResourcesV1::new_v1(),
        )
        .expect("source writer");
        let chunk = writer
            .allocate_chunk(ZkAmsMkheRnsNativeSourceArenaV1::Main)
            .expect("main chunk");
        let error = writer
            .write_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 1, chunk)
            .expect_err("out-of-order write must fail");
        assert_eq!(error, ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite);
        assert!(
            !error
                .to_string()
                .contains(directory.path().to_string_lossy().as_ref())
        );
        assert!(writer.main.is_some() && writer.nonce.is_some());
        let valid = writer
            .allocate_chunk(ZkAmsMkheRnsNativeSourceArenaV1::Main)
            .unwrap();
        writer
            .write_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 0, valid)
            .unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn foreign_same_arena_chunk_is_rejected_before_debit_and_cleared() {
        let directory = tempfile::tempdir().unwrap();
        let mut writer = CoreZkAmsMkheRnsNativeSourceWriterV1::begin(
            directory.path(),
            source_layout(),
            ZkAmsMkheRnsNativeSourceResourcesV1::new_v1(),
        )
        .unwrap();
        let other = CoreZkAmsMkheRnsNativeSourceWriterV1::begin(
            directory.path(),
            source_layout(),
            ZkAmsMkheRnsNativeSourceResourcesV1::new_v1(),
        )
        .unwrap();
        let arena = ZkAmsMkheRnsNativeSourceArenaV1::Main;
        let mut foreign = other.allocate_chunk(arena).unwrap();
        foreign.as_mut_slice().fill(0xD1);
        CHUNK_CLEAR_EVENTS_V1.with(|events| events.borrow_mut().clear());
        assert_eq!(
            writer.write_slot(arena, 0, foreign),
            Err(ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite)
        );
        CHUNK_CLEAR_EVENTS_V1.with(|events| assert_eq!(*events.borrow(), vec![Some(true)]));
        assert!(writer.main.is_some() && writer.nonce.is_some());
        // If the foreign write advanced/debited the schedule this exact slot fails.
        let own = writer.allocate_chunk(arena).unwrap();
        writer.write_slot(arena, 0, own).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn operational_write_error_and_unwind_destroy_both_writer_arenas() {
        for unwind in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let mut writer = CoreZkAmsMkheRnsNativeSourceWriterV1::begin(
                directory.path(),
                source_layout(),
                ZkAmsMkheRnsNativeSourceResourcesV1::new_v1(),
            )
            .unwrap();
            let arena = ZkAmsMkheRnsNativeSourceArenaV1::Main;
            let mut chunk = writer.allocate_chunk(arena).unwrap();
            chunk.as_mut_slice().fill(0xAB);
            CHUNK_CLEAR_EVENTS_V1.with(|events| events.borrow_mut().clear());
            if unwind {
                writer.panic_on_next_write = true;
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = writer.write_slot(arena, 0, chunk);
                    }))
                    .is_err()
                );
            } else {
                writer.injected_write_error = Some(ConfidentialSpoolErrorV1::FileOperation {
                    operation: "write",
                    kind: std::io::ErrorKind::Other,
                });
                assert_eq!(
                    writer.write_slot(arena, 0, chunk),
                    Err(ZkAmsMkheRnsNativeSourceErrorV1::Storage)
                );
            }
            assert!(writer.main.is_none() && writer.nonce.is_none());
            assert!(matches!(
                writer.allocate_chunk(arena),
                Err(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned)
            ));
            // None denotes transfer into the actual crypto leaf; the wrapper
            // still dropped exactly once after that allocation was destroyed.
            CHUNK_CLEAR_EVENTS_V1.with(|events| assert_eq!(*events.borrow(), vec![None]));
        }
    }

    #[cfg(unix)]
    #[test]
    fn read_workspace_refusal_preserves_both_authenticated_snapshots() {
        let mut snapshot = reduced_snapshot_v1(false);
        let main =
            spool_layout_v1(&snapshot.layout, ZkAmsMkheRnsNativeSourceArenaV1::Main).unwrap();
        let nonce =
            spool_layout_v1(&snapshot.layout, ZkAmsMkheRnsNativeSourceArenaV1::Nonce).unwrap();
        let retained = source_named_bytes_v1(&main, &nonce).unwrap();
        let held = snapshot
            .admission
            .reserve_workspace_v1(512 * 1024 * 1024 - retained, 0)
            .unwrap();
        let receipt = snapshot.structural_receipt().unwrap();
        assert!(matches!(
            snapshot.read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 0),
            Err(ZkAmsMkheRnsNativeSourceErrorV1::ResourceCeilingExceeded)
        ));
        assert!(snapshot.main.is_some() && snapshot.nonce.is_some());
        assert_eq!(snapshot.structural_receipt().unwrap(), receipt);
        drop(held);
        assert!(
            snapshot
                .read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 0)
                .is_ok()
        );
    }

    #[cfg(unix)]
    #[test]
    fn held_read_chunk_outlives_snapshot_and_clears_on_drop() {
        let mut snapshot = reduced_snapshot_v1(false);
        let chunk = snapshot
            .read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 1)
            .unwrap();
        assert!(snapshot.admission.owns_workspace_v1(&chunk.workspace));
        drop(snapshot);
        assert!(chunk.as_slice().iter().all(|byte| *byte == 0xA1));
        CHUNK_CLEAR_EVENTS_V1.with(|events| events.borrow_mut().clear());
        drop(chunk);
        CHUNK_CLEAR_EVENTS_V1.with(|events| assert_eq!(*events.borrow(), vec![Some(true)]));
    }

    #[cfg(unix)]
    #[test]
    fn named_workspace_accounting_includes_secret_payload_and_operation_owners() {
        let layout = source_layout();
        let main = spool_layout_v1(&layout, ZkAmsMkheRnsNativeSourceArenaV1::Main).unwrap();
        let nonce = spool_layout_v1(&layout, ZkAmsMkheRnsNativeSourceArenaV1::Nonce).unwrap();
        assert!(
            source_named_bytes_v1(&main, &nonce).unwrap()
                >= size_of::<CoreZkAmsMkheRnsNativeSourceSnapshotV1>() as u64
        );
        for arena in [
            ZkAmsMkheRnsNativeSourceArenaV1::Main,
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
        ] {
            let leaf = spool_layout_v1(&layout, arena).unwrap();
            let whole = chunk_named_bytes_v1(arena).unwrap()
                + operation_scratch_v1(&layout, arena).unwrap();
            assert!(whole >= leaf.named_operation_workspace_bytes_v1() as u64);
            assert_eq!(
                chunk_named_bytes_v1(arena).unwrap(),
                arena.plaintext_bytes() + size_of::<CoreZkAmsMkheRnsNativeSecretChunkV1>() as u64
            );
        }
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "writes and authenticates the canonical 316,239,888-byte source pair"]
    fn canonical_source_round_trip_uses_real_consuming_begin_and_seal() {
        let directory = tempfile::tempdir().unwrap();
        let mut writer = CoreZkAmsMkheRnsNativeSourceWriterV1::begin(
            directory.path(),
            source_layout(),
            ZkAmsMkheRnsNativeSourceResourcesV1::new_v1(),
        )
        .unwrap();
        for arena in [
            ZkAmsMkheRnsNativeSourceArenaV1::Main,
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
        ] {
            for slot in 0..arena.slot_count() {
                let mut chunk = writer.allocate_chunk(arena).unwrap();
                chunk.as_mut_slice().fill((slot % 251) as u8);
                writer.write_slot(arena, slot, chunk).unwrap();
            }
        }
        let mut snapshot = writer.seal().unwrap();
        for arena in [
            ZkAmsMkheRnsNativeSourceArenaV1::Main,
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
        ] {
            for slot in [arena.slot_count() - 1, 0, arena.slot_count() / 2, 0] {
                let chunk = snapshot.read_slot(arena, slot).unwrap();
                assert!(
                    chunk
                        .as_slice()
                        .iter()
                        .all(|byte| *byte == (slot % 251) as u8)
                );
            }
        }
    }
}
