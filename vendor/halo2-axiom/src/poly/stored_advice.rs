//! Backend-neutral foundation for immutable confidential advice-polynomial snapshots.
//!
//! The consuming prover will own these handles; the borrowed prover remains unchanged.
//! This interface does not provide authentication itself. Its trusted backend must authenticate
//! the complete layout binding and chunk index, zeroize owned plaintext, and enforce one shared
//! materialization window. Snapshot digests are never polynomial commitments.
//!
//! TODO: Integrate admitted consuming synthesis, stored argument queries, quotient outputs,
//! and multi-opening handles before claiming that this foundation reduces complete-proof memory.

use std::fmt;

use blake2b_simd::Params;
use ff::PrimeField;
use halo2curves::pasta::{Fp, Fq};

/// Bounded assignment for explicitly admitted discard-only circuit producers.
pub mod assignment;

/// Fallible phase commitments for explicitly admitted IPA producers.
pub(crate) mod phase;

/// Single-column immutable basis conversions.
pub mod transform;

/// Canonical Pasta scalar byte width.
pub const STORED_SCALAR_BYTES_V1: usize = 32;
/// Fixed scalar count per authenticated chunk; final padding must be zero.
pub const STORED_SCALARS_PER_CHUNK_V1: usize = 256;
/// Maximum domain degree admitted by this foundation, including individual coset parts.
pub const STORED_MAX_K_V1: u32 = 19;

/// Exact canonical scalar field; this identifies the field, not the commitment curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredPastaFieldV1 {
    /// Pasta Fp, including Eq proof scalar columns.
    Fp,
    /// Pasta Fq, including Ep proof scalar columns.
    Fq,
}

impl StoredPastaFieldV1 {
    /// Reject noncanonical field encodings without reducing them modulo the field.
    pub fn is_canonical(self, bytes: &[u8; STORED_SCALAR_BYTES_V1]) -> bool {
        match self {
            Self::Fp => bool::from(Fp::from_repr(*bytes).is_some()),
            Self::Fq => bool::from(Fq::from_repr(*bytes).is_some()),
        }
    }
}

/// Exact interpretation of the stored scalar sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredPolynomialBasisV1 {
    /// Evaluations on the original size-2^k domain.
    Lagrange,
    /// Coefficients in increasing power order.
    Coefficient,
    /// One size-2^k coset part of a domain extended by 2^extension_log parts.
    CosetPart {
        /// Base-two logarithm of the number of parts.
        extension_log: u32,
        /// Numeric part in the existing evaluation-domain order.
        part: u32,
    },
}

/// Coarse, nonsecret failure class; no paths, scalars, or backend strings escape here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredAdviceErrorV1 {
    /// Invalid or overflowing trusted layout input.
    Layout,
    /// Caller expected another immutable polynomial identity or interpretation.
    Context,
    /// Chunk index lies outside the polynomial.
    ChunkIndex,
    /// A sequential write has the wrong index or exact scalar count.
    WriteOrder,
    /// A scalar or final padding has a noncanonical encoding.
    Encoding,
    /// Sealing omitted one or more chunks.
    Incomplete,
    /// Another operation holds this proof's single plaintext window.
    Busy,
    /// A bounded allocation failed.
    Allocation,
    /// The configured number of polynomial handles was exhausted.
    Capacity,
    /// The secure backend or operating-system entropy is unavailable.
    Backend,
    /// An authenticated storage operation failed.
    Storage,
    /// Ciphertext authentication failed.
    Authentication,
    /// An earlier operational, decoding, or callback failure invalidated this owner.
    Poisoned,
    /// The materialization consumer rejected the complete input.
    Consumer,
}

impl fmt::Display for StoredAdviceErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "confidential advice store: {self:?}")
    }
}
impl std::error::Error for StoredAdviceErrorV1 {}

/// Complete immutable identity and interpretation of one stored advice polynomial.
///
/// Ordinals are allocated monotonically by a per-proof provider. A basis conversion produces
/// a new immutable snapshot/ordinal while retaining its advice column and phase coordinates.
/// Each authenticated context includes the fresh proof context, field, basis/coset part, k,
/// exact logical length, chunk geometry, scalar representation version, and zero-padding rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoredAdviceLayoutV1 {
    proof_context: [u8; 32],
    ordinal: u64,
    field: StoredPastaFieldV1,
    basis: StoredPolynomialBasisV1,
    k: u32,
    column: u32,
    phase: u8,
}

impl StoredAdviceLayoutV1 {
    /// Construct bounded metadata from trusted prover coordinates.
    ///
    /// # Errors
    /// Rejects a zero proof context, unsupported phase/domain, or invalid coset part.
    pub fn new(
        proof_context: [u8; 32],
        ordinal: u64,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        column: u32,
        phase: u8,
    ) -> Result<Self, StoredAdviceErrorV1> {
        if proof_context == [0; 32] || k > STORED_MAX_K_V1 || phase > 2 {
            return Err(StoredAdviceErrorV1::Layout);
        }
        if let StoredPolynomialBasisV1::CosetPart {
            extension_log,
            part,
        } = basis
        {
            if extension_log == 0
                || extension_log > STORED_MAX_K_V1 - k
                || part >= (1_u32 << extension_log)
            {
                return Err(StoredAdviceErrorV1::Layout);
            }
        }
        Ok(Self {
            proof_context,
            ordinal,
            field,
            basis,
            k,
            column,
            phase,
        })
    }

    /// Return the exact scalar field.
    pub const fn field(self) -> StoredPastaFieldV1 {
        self.field
    }
    /// Return the stable snapshot ordinal within the fresh proof context.
    pub const fn ordinal(self) -> u64 {
        self.ordinal
    }
    /// Return the exact scalar basis and, where relevant, coset part.
    pub const fn basis(self) -> StoredPolynomialBasisV1 {
        self.basis
    }
    /// Return the original domain degree.
    pub const fn k(self) -> u32 {
        self.k
    }
    /// Return the canonical advice-column index.
    pub const fn column(self) -> u32 {
        self.column
    }
    /// Return the advice phase index.
    pub const fn phase(self) -> u8 {
        self.phase
    }
    /// Return the logical length, excluding final chunk padding.
    pub const fn scalar_count(self) -> usize {
        1_usize << self.k
    }
    /// Return the fixed nonzero number of chunks.
    pub fn chunk_count(self) -> usize {
        self.scalar_count().div_ceil(STORED_SCALARS_PER_CHUNK_V1)
    }
    /// Return the exact number of logical scalars in a chunk.
    ///
    /// # Errors
    /// Rejects a chunk outside the immutable layout.
    pub fn chunk_scalar_count(self, chunk: u64) -> Result<usize, StoredAdviceErrorV1> {
        let chunk = usize::try_from(chunk).map_err(|_| StoredAdviceErrorV1::ChunkIndex)?;
        if chunk >= self.chunk_count() {
            return Err(StoredAdviceErrorV1::ChunkIndex);
        }
        Ok((self.scalar_count() - chunk * STORED_SCALARS_PER_CHUNK_V1)
            .min(STORED_SCALARS_PER_CHUNK_V1))
    }

    /// Compare proof membership without exposing the raw proof context or any storage secret.
    /// Column, ordinal, field and basis must still be checked independently by the caller.
    pub(crate) fn same_proof_context(self, other: Self) -> bool {
        self.proof_context == other.proof_context
    }

    /// Derive the public authenticated context using the backend's existing BLAKE2b primitive.
    ///
    /// This fixed-layout domain binding is not a wire codec, proof commitment, or authority.
    /// All integers are little-endian; scalar bytes use canonical PrimeField::Repr encoding.
    pub fn context_digest(self) -> [u8; 32] {
        let mut hash = Params::new()
            .hash_length(32)
            .personal(b"Halo2PolyStoreV1")
            .to_state();
        hash.update(b"advice.snapshot.v1\0canonical-primefield-repr\0zero-tail\0");
        hash.update(&self.proof_context);
        hash.update(&self.ordinal.to_le_bytes());
        hash.update(&[match self.field {
            StoredPastaFieldV1::Fp => 0,
            StoredPastaFieldV1::Fq => 1,
        }]);
        let (tag, extension_log, part) = match self.basis {
            StoredPolynomialBasisV1::Lagrange => (0, 0_u32, 0_u32),
            StoredPolynomialBasisV1::Coefficient => (1, 0, 0),
            StoredPolynomialBasisV1::CosetPart {
                extension_log,
                part,
            } => (2, extension_log, part),
        };
        hash.update(&[tag]);
        hash.update(&extension_log.to_le_bytes());
        hash.update(&part.to_le_bytes());
        hash.update(&self.k.to_le_bytes());
        hash.update(&self.column.to_le_bytes());
        hash.update(&[self.phase]);
        hash.update(&(self.scalar_count() as u64).to_le_bytes());
        hash.update(&(STORED_SCALARS_PER_CHUNK_V1 as u64).to_le_bytes());
        hash.update(&(STORED_SCALAR_BYTES_V1 as u64).to_le_bytes());
        let mut digest = [0; 32];
        digest.copy_from_slice(hash.finalize().as_bytes());
        digest
    }
}

/// Trusted backend that creates move-only confidential polynomial writers.
///
/// The provider owns a fresh per-proof context, monotonic snapshot ordinals, and one shared
/// plaintext-window lease across every writer and snapshot it creates. It must not use the
/// proof transcript RNG to generate storage keys/nonces or include storage metadata in proofs.
pub trait StoredAdviceProviderV1 {
    /// Backend writer; no raw path, key, descriptor, or digest constructor is exposed.
    type Writer: StoredAdviceWriterV1;
    /// Create one immutable polynomial destination with exact trusted coordinates.
    ///
    /// # Errors
    /// Rejects invalid geometry, exhausted handles, an active window, or backend failures.
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        column: u32,
        phase: u8,
    ) -> Result<Self::Writer, StoredAdviceErrorV1>;
}

/// Sequential write-once destination for an already-frozen polynomial.
pub trait StoredAdviceWriterV1: Sized {
    /// Authenticated immutable read owner returned only after complete sealing.
    type Snapshot: StoredAdviceSnapshotV1;
    /// Return exact metadata.
    fn layout(&self) -> StoredAdviceLayoutV1;
    /// Consume the next exact logical chunk; the backend supplies canonical zero padding.
    ///
    /// # Errors
    /// Invalid indices/counts/encodings are retryable preflights. Operational failures poison.
    fn write_chunk(&mut self, chunk: u64, scalars: &[[u8; 32]]) -> Result<(), StoredAdviceErrorV1>;
    /// Authenticate every slot and consume the writer into a repeatable read owner.
    ///
    /// # Errors
    /// Rejects incomplete writes, an active window, or any backend authentication/I/O failure.
    fn seal(self) -> Result<Self::Snapshot, StoredAdviceErrorV1>;
}

/// Authenticated immutable snapshot with borrowed, bounded plaintext access.
///
/// Callbacks cannot retain a borrow of backend memory. Caller-created copies are outside this
/// interface's storage bound. Callback error/panic must destroy this snapshot's key/file and
/// zeroize partial materialization; pure metadata/index/Busy preflight errors may be retried.
pub trait StoredAdviceSnapshotV1 {
    /// Return exact metadata.
    fn layout(&self) -> StoredAdviceLayoutV1;
    /// Read one authenticated chunk, omitting checked canonical tail padding.
    ///
    /// # Errors
    /// Rejects wrong expected metadata, slot/encoding/authentication/I/O failures, or a busy window.
    fn with_chunk<R>(
        &mut self,
        expected: StoredAdviceLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1>;
    /// Authenticate and materialize exactly one encoded polynomial for a bounded callback.
    ///
    /// # Errors
    /// Rejects wrong metadata, a busy window, allocation/read/decoding failure, or consumer error.
    fn with_column<R>(
        &mut self,
        expected: StoredAdviceLayoutV1,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1>;
}

#[cfg(test)]
mod tests;
