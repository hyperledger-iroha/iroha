//! Core adapter for the consuming prover's immutable confidential polynomial snapshots.
//!
//! Axiom owns metadata and backend-neutral traits; Core reuses `iroha_crypto`'s already-locked
//! authenticated, unlinked confidential spool. Each snapshot gets the spool's fresh key/arena;
//! a separate fresh public proof context and monotonic ordinal bind polynomial interpretation.
//! Neither storage randomness nor ciphertext digests enter the proof transcript.
//!
//! A provider permits one active plaintext operation across all its writers/snapshots. Reads
//! hold one 8 KiB spool chunk; full materialization additionally holds exactly one encoded
//! column, bounded at 2^19 * 32 bytes. Caller inputs/copies, decoded field arrays, allocator
//! overhead, kernel buffers, and the rest of the prover are outside this payload accounting.
//! No whole-process RSS or 128 MiB qualification follows from this foundation.
//!
//! The caller supplies a private directory whose ancestors it controls. The existing spool's
//! pathname setup, fork/swap/core-dump policy and derived-cipher-state lifecycle limitations
//! still apply; this adapter does not claim to repair them.
//!
//! TODO: Wire only into a reviewed consuming-prover path after assignment/replay, query, and
//! argument lifetimes are bounded. The borrowed prover is unchanged and remains the oracle.

use std::{cell::Cell, path::PathBuf, rc::Rc};

use halo2_proofs::poly::stored_advice::{
    STORED_SCALAR_BYTES_V1, STORED_SCALARS_PER_CHUNK_V1, StoredAdviceErrorV1, StoredAdviceLayoutV1,
    StoredAdviceProviderV1, StoredAdviceSnapshotV1, StoredAdviceWriterV1, StoredPastaFieldV1,
    StoredPolynomialBasisV1,
};
use iroha_crypto::confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolErrorV1, ConfidentialSpoolLayoutV1,
    ConfidentialSpoolSnapshotV1, ConfidentialSpoolWriterV1,
};
use rand_core_06::{OsRng, RngCore as _};
use zeroize::Zeroizing;

const CHUNK_BYTES: usize = STORED_SCALAR_BYTES_V1 * STORED_SCALARS_PER_CHUNK_V1;
const MAX_LIVE_SNAPSHOTS_PER_PROOF: usize = 512;

/// Single-threaded per-proof store owner with a shared one-operation plaintext window.
///
/// This type deliberately provides neither `Clone` nor `Debug`. Its handle budget is a
/// foundation bound, not a production configuration knob or a Claim capacity declaration.
pub struct CoreStoredAdviceProviderV1 {
    directory: PathBuf,
    proof_context: [u8; 32],
    next_ordinal: u64,
    window: Rc<Cell<bool>>,
    handles: Rc<LiveSnapshotBudget>,
}

impl CoreStoredAdviceProviderV1 {
    /// Create a fresh process-local proof context without consuming transcript randomness.
    ///
    /// # Errors
    /// Rejects unavailable OS entropy or an all-zero context. File checks occur in `create`.
    pub fn new(directory: impl Into<PathBuf>) -> Result<Self, StoredAdviceErrorV1> {
        let mut proof_context = [0; 32];
        OsRng
            .try_fill_bytes(&mut proof_context)
            .map_err(|_| StoredAdviceErrorV1::Backend)?;
        if proof_context == [0; 32] {
            return Err(StoredAdviceErrorV1::Backend);
        }
        Ok(Self {
            directory: directory.into(),
            proof_context,
            next_ordinal: 0,
            window: Rc::new(Cell::new(false)),
            handles: Rc::new(LiveSnapshotBudget {
                live: Cell::new(0),
                limit: MAX_LIVE_SNAPSHOTS_PER_PROOF,
            }),
        })
    }
}

struct LiveSnapshotBudget {
    live: Cell<usize>,
    limit: usize,
}

// The lease moves from writer to snapshot. Retired snapshots free a slot without
// recycling their ordinal; sequential basis conversions cannot exhaust a lifetime quota.
struct LiveSnapshotLease(Rc<LiveSnapshotBudget>);

impl LiveSnapshotLease {
    fn acquire(budget: &Rc<LiveSnapshotBudget>) -> Result<Self, StoredAdviceErrorV1> {
        let live = budget.live.get();
        if live >= budget.limit {
            return Err(StoredAdviceErrorV1::Capacity);
        }
        budget.live.set(live + 1);
        Ok(Self(Rc::clone(budget)))
    }
}

impl Drop for LiveSnapshotLease {
    fn drop(&mut self) {
        self.0.live.set(self.0.live.get() - 1);
    }
}

struct PlaintextWindow(Rc<Cell<bool>>);

impl PlaintextWindow {
    fn acquire(window: &Rc<Cell<bool>>) -> Result<Self, StoredAdviceErrorV1> {
        if window.replace(true) {
            return Err(StoredAdviceErrorV1::Busy);
        }
        Ok(Self(Rc::clone(window)))
    }
}
impl Drop for PlaintextWindow {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// Move-only sequential polynomial writer; no plaintext chunks escape as owned values.
pub struct CoreStoredAdviceWriterV1 {
    layout: StoredAdviceLayoutV1,
    next_chunk: u64,
    raw: Option<ConfidentialSpoolWriterV1>,
    window: Rc<Cell<bool>>,
    lease: LiveSnapshotLease,
}

/// Move-only authenticated polynomial snapshot with a shared bounded plaintext window.
pub struct CoreStoredAdviceSnapshotV1 {
    layout: StoredAdviceLayoutV1,
    raw: Option<ConfidentialSpoolSnapshotV1>,
    window: Rc<Cell<bool>>,
    _lease: LiveSnapshotLease,
    #[cfg(test)]
    injected_read_error: Option<StoredAdviceErrorV1>,
    #[cfg(test)]
    panic_on_read: bool,
}

impl StoredAdviceProviderV1 for CoreStoredAdviceProviderV1 {
    type Writer = CoreStoredAdviceWriterV1;

    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        column: u32,
        phase: u8,
    ) -> Result<Self::Writer, StoredAdviceErrorV1> {
        let next_ordinal = self
            .next_ordinal
            .checked_add(1)
            .ok_or(StoredAdviceErrorV1::Capacity)?;
        let layout = StoredAdviceLayoutV1::new(
            self.proof_context,
            self.next_ordinal,
            field,
            basis,
            k,
            column,
            phase,
        )?;
        let spool_layout = ConfidentialSpoolLayoutV1::new_v1(
            layout.chunk_count() as u64,
            CHUNK_BYTES as u64,
            layout.context_digest(),
        )
        .map_err(map_spool_error)?;
        let _window = PlaintextWindow::acquire(&self.window)?;
        let lease = LiveSnapshotLease::acquire(&self.handles)?;
        // Burn the ordinal before external effects; failed creation cannot recycle an identity.
        self.next_ordinal = next_ordinal;
        let raw = ConfidentialSpoolWriterV1::create_in_v1(&self.directory, spool_layout)
            .map_err(map_spool_error)?;
        Ok(CoreStoredAdviceWriterV1 {
            layout,
            next_chunk: 0,
            raw: Some(raw),
            window: Rc::clone(&self.window),
            lease,
        })
    }
}

impl StoredAdviceWriterV1 for CoreStoredAdviceWriterV1 {
    type Snapshot = CoreStoredAdviceSnapshotV1;

    fn layout(&self) -> StoredAdviceLayoutV1 {
        self.layout
    }

    fn write_chunk(&mut self, chunk: u64, scalars: &[[u8; 32]]) -> Result<(), StoredAdviceErrorV1> {
        if self.raw.is_none() {
            return Err(StoredAdviceErrorV1::Poisoned);
        }
        if chunk != self.next_chunk || self.layout.chunk_scalar_count(chunk)? != scalars.len() {
            return Err(StoredAdviceErrorV1::WriteOrder);
        }
        if scalars
            .iter()
            .any(|scalar| !self.layout.field().is_canonical(scalar))
        {
            return Err(StoredAdviceErrorV1::Encoding);
        }
        let _window = PlaintextWindow::acquire(&self.window)?;
        // Keep the resources outside self until success: allocation/I/O/encryption errors and
        // unwinding all leave this writer poisoned and drop its retained key/file.
        let mut raw = self.raw.take().ok_or(StoredAdviceErrorV1::Poisoned)?;
        let mut plaintext =
            ConfidentialSpoolChunkV1::new_zeroed_v1(CHUNK_BYTES as u64).map_err(map_spool_error)?;
        for (destination, scalar) in plaintext
            .as_mut_slice_v1()
            .chunks_exact_mut(32)
            .zip(scalars)
        {
            destination.copy_from_slice(scalar);
        }
        raw.write_slot_v1(chunk, plaintext)
            .map_err(map_spool_error)?;
        self.next_chunk += 1;
        self.raw = Some(raw);
        Ok(())
    }

    fn seal(mut self) -> Result<Self::Snapshot, StoredAdviceErrorV1> {
        if self.raw.is_none() {
            return Err(StoredAdviceErrorV1::Poisoned);
        }
        if self.next_chunk != self.layout.chunk_count() as u64 {
            return Err(StoredAdviceErrorV1::Incomplete);
        }
        let _window = PlaintextWindow::acquire(&self.window)?;
        let raw = self
            .raw
            .take()
            .ok_or(StoredAdviceErrorV1::Poisoned)?
            .seal_v1()
            .map_err(map_spool_error)?;
        Ok(CoreStoredAdviceSnapshotV1 {
            layout: self.layout,
            raw: Some(raw),
            window: Rc::clone(&self.window),
            _lease: self.lease,
            #[cfg(test)]
            injected_read_error: None,
            #[cfg(test)]
            panic_on_read: false,
        })
    }
}

impl CoreStoredAdviceSnapshotV1 {
    fn preflight(&self, expected: StoredAdviceLayoutV1) -> Result<(), StoredAdviceErrorV1> {
        if self.raw.is_none() {
            return Err(StoredAdviceErrorV1::Poisoned);
        }
        if expected != self.layout {
            return Err(StoredAdviceErrorV1::Context);
        }
        Ok(())
    }

    fn read_operation<R>(
        &mut self,
        operation: impl FnOnce(
            &mut ConfidentialSpoolSnapshotV1,
            StoredAdviceLayoutV1,
        ) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1> {
        let _window = PlaintextWindow::acquire(&self.window)?;
        // The lower-level spool restores its owner after a successful raw read. Taking it here
        // additionally makes scalar validation and callback errors/panics fail-stop.
        let mut raw = self.raw.take().ok_or(StoredAdviceErrorV1::Poisoned)?;
        #[cfg(test)]
        {
            if std::mem::take(&mut self.panic_on_read) {
                panic!("injected advice read panic");
            }
            if let Some(error) = self.injected_read_error.take() {
                return Err(error);
            }
        }
        let output = operation(&mut raw, self.layout)?;
        self.raw = Some(raw);
        Ok(output)
    }
}

fn validated_chunk<'a>(
    layout: StoredAdviceLayoutV1,
    index: u64,
    plaintext: &'a ConfidentialSpoolChunkV1,
) -> Result<&'a [[u8; 32]], StoredAdviceErrorV1> {
    if plaintext.as_slice_v1().len() != CHUNK_BYTES {
        return Err(StoredAdviceErrorV1::Encoding);
    }
    let (scalars, remainder) = plaintext.as_slice_v1().as_chunks::<32>();
    let count = layout.chunk_scalar_count(index)?;
    if !remainder.is_empty()
        || scalars[..count]
            .iter()
            .any(|scalar| !layout.field().is_canonical(scalar))
        || scalars[count..].iter().any(|scalar| *scalar != [0; 32])
    {
        return Err(StoredAdviceErrorV1::Encoding);
    }
    Ok(&scalars[..count])
}

impl StoredAdviceSnapshotV1 for CoreStoredAdviceSnapshotV1 {
    fn layout(&self) -> StoredAdviceLayoutV1 {
        self.layout
    }

    fn with_chunk<R>(
        &mut self,
        expected: StoredAdviceLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1> {
        self.preflight(expected)?;
        self.layout.chunk_scalar_count(chunk)?;
        self.read_operation(|raw, layout| {
            let plaintext = raw
                .read_slot_v1(chunk, layout.context_digest())
                .map_err(map_spool_error)?;
            consume(validated_chunk(layout, chunk, &plaintext)?)
        })
    }

    fn with_column<R>(
        &mut self,
        expected: StoredAdviceLayoutV1,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1> {
        self.preflight(expected)?;
        self.read_operation(|raw, layout| {
            let mut column = Zeroizing::new(Vec::<[u8; 32]>::new());
            column
                .try_reserve_exact(layout.scalar_count())
                .map_err(|_| StoredAdviceErrorV1::Allocation)?;
            column.resize(layout.scalar_count(), [0; 32]);
            for index in 0..layout.chunk_count() as u64 {
                let plaintext = raw
                    .read_slot_v1(index, layout.context_digest())
                    .map_err(map_spool_error)?;
                let scalars = validated_chunk(layout, index, &plaintext)?;
                let start = index as usize * STORED_SCALARS_PER_CHUNK_V1;
                column[start..start + scalars.len()].copy_from_slice(scalars);
                // The zeroizing chunk drops before the next read; only one encoded column lives.
            }
            consume(&column)
        })
    }
}

fn map_spool_error(error: ConfidentialSpoolErrorV1) -> StoredAdviceErrorV1 {
    match error {
        ConfidentialSpoolErrorV1::EmptyLayout
        | ConfidentialSpoolErrorV1::EmptyChunk
        | ConfidentialSpoolErrorV1::GeometryOverflow
        | ConfidentialSpoolErrorV1::AddressSpaceExceeded
        | ConfidentialSpoolErrorV1::InertContextDigest
        | ConfidentialSpoolErrorV1::LimitExceeded(_)
        | ConfidentialSpoolErrorV1::CipherMessageLimit => StoredAdviceErrorV1::Layout,
        ConfidentialSpoolErrorV1::Allocation(_) => StoredAdviceErrorV1::Allocation,
        ConfidentialSpoolErrorV1::UnsupportedPlatform
        | ConfidentialSpoolErrorV1::EntropyUnavailable
        | ConfidentialSpoolErrorV1::WeakEntropy(_) => StoredAdviceErrorV1::Backend,
        ConfidentialSpoolErrorV1::SlotOutOfRange { .. } => StoredAdviceErrorV1::ChunkIndex,
        ConfidentialSpoolErrorV1::UnexpectedWriteSlot { .. }
        | ConfidentialSpoolErrorV1::ChunkLength { .. } => StoredAdviceErrorV1::WriteOrder,
        ConfidentialSpoolErrorV1::ContextDigestMismatch => StoredAdviceErrorV1::Context,
        ConfidentialSpoolErrorV1::Incomplete { .. } => StoredAdviceErrorV1::Incomplete,
        ConfidentialSpoolErrorV1::Encryption | ConfidentialSpoolErrorV1::Authentication => {
            StoredAdviceErrorV1::Authentication
        }
        ConfidentialSpoolErrorV1::Poisoned => StoredAdviceErrorV1::Poisoned,
        ConfidentialSpoolErrorV1::FileOperation { .. }
        | ConfidentialSpoolErrorV1::UnsafeTemporaryFile
        | ConfidentialSpoolErrorV1::TemporaryFileIdentityMismatch
        | ConfidentialSpoolErrorV1::UnsafeDetachedFile
        | ConfidentialSpoolErrorV1::FileLength { .. } => StoredAdviceErrorV1::Storage,
    }
}

#[cfg(all(test, unix))]
mod tests;
