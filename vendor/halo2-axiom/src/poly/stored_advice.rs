//! Backend-neutral foundation for immutable confidential polynomial snapshots.
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
/// Chunk-only internal sources for bounded stored polynomial consumers.
pub(crate) mod reader;

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

/// Ordered expression side of a lookup in the retained proving key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredLookupSideV1 {
    /// The lookup's compressed input expressions.
    Input,
    /// The lookup's compressed table expressions.
    Table,
}

/// Semantic polynomial identity authenticated independently of its scalar basis.
///
/// Lookup indices use the retained proving key's lookup order. Advice coordinates are valid
/// only for advice polynomials; callers must not encode lookup identities as advice columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredPolynomialRoleV1 {
    /// Undivided quotient numerator evaluations on one exact original coset part.
    /// This scratch role is not an ordinary coefficient polynomial.
    QuotientNumerator,
    /// Inverse-only aliased coefficient intermediate for one original extended-domain part.
    QuotientAliasedPart {
        /// Original part index.
        part: u32,
        /// Original extended logarithm minus base k.
        extension_log: u32,
    },
    /// One ordinary quotient coefficient piece in increasing piece order.
    QuotientPiece {
        /// Piece index, additionally bounded by the retained key at the consuming boundary.
        piece: u32,
    },
    /// Original public instance polynomial.
    Instance {
        /// Original key instance-column index.
        column: u32,
    },
    /// Vanishing argument random coefficient polynomial.
    VanishingRandom,
    /// One canonical advice column in its synthesis phase.
    Advice {
        /// Canonical advice-column index.
        column: u32,
        /// Advice synthesis phase, restricted to 0, 1, or 2.
        phase: u8,
    },
    /// One lookup's compressed input or table polynomial.
    LookupCompressed {
        /// Lookup index in the retained proving key.
        lookup: u32,
        /// Exact expression side compressed with the transcript's theta challenge.
        side: StoredLookupSideV1,
    },
    /// One committed lookup permutation side, retaining its role across legitimate bases.
    LookupPermuted {
        /// Lookup index in the retained proving key.
        lookup: u32,
        /// Permuted input or table side.
        side: StoredLookupSideV1,
    },
    /// One copy-permutation set product in original global column order.
    CopyPermutationProduct {
        /// Original proving-key set index.
        set: u32,
    },
    /// One lookup grand product in original proving-key lookup order.
    LookupProduct {
        /// Original proving-key lookup index.
        lookup: u32,
    },
    /// Ascending unmatched table occurrences after one match per distinct lookup input.
    ///
    /// Only the owner's private leftover count participates; remaining rows are ZERO padding.
    LookupLeftoverTable {
        /// Lookup index in the retained proving key.
        lookup: u32,
    },
    /// One authenticated external-sort pass over a lookup's usable-row prefix.
    ///
    /// This is scratch, not a permuted argument polynomial. Rows outside the key's
    /// usable prefix are canonical ZERO padding and never participate in merging.
    LookupSorted {
        /// Lookup index in the retained proving key.
        lookup: u32,
        /// Original compressed expression side.
        side: StoredLookupSideV1,
        /// Base-two logarithm of each sorted run width, between min(k, 8) and k.
        run_log: u32,
    },
}

/// Coarse, nonsecret failure class; no paths, scalars, or backend strings escape here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredPolynomialErrorV1 {
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

impl fmt::Display for StoredPolynomialErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "confidential polynomial store: {self:?}")
    }
}
impl std::error::Error for StoredPolynomialErrorV1 {}

/// Complete immutable identity and interpretation of one stored polynomial.
///
/// Ordinals are allocated monotonically by a per-proof provider. A basis conversion produces
/// a new immutable snapshot/ordinal while retaining its complete semantic role.
/// Each authenticated context includes the fresh proof context, role, field, basis/coset part, k,
/// exact logical length, chunk geometry, scalar representation version, and zero-padding rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoredPolynomialLayoutV1 {
    proof_context: [u8; 32],
    ordinal: u64,
    field: StoredPastaFieldV1,
    basis: StoredPolynomialBasisV1,
    k: u32,
    role: StoredPolynomialRoleV1,
}

impl StoredPolynomialLayoutV1 {
    /// Construct bounded metadata from trusted prover coordinates.
    ///
    /// # Errors
    /// Rejects a zero proof context, unsupported phase/domain, invalid coset part, or
    /// a sorted scratch pass outside min(k, 8)..=k, or a role outside its exact supported
    /// basis/part/piece bounds. Aliased quotient intermediates are coefficient-only scratch.
    pub fn new(
        proof_context: [u8; 32],
        ordinal: u64,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self, StoredPolynomialErrorV1> {
        if proof_context == [0; 32]
            || matches!(role, StoredPolynomialRoleV1::QuotientNumerator
                if !matches!(basis, StoredPolynomialBasisV1::CosetPart { .. }))
            || k > STORED_MAX_K_V1
            || matches!(role, StoredPolynomialRoleV1::Advice { phase, .. } if phase > 2)
            || matches!(role, StoredPolynomialRoleV1::LookupLeftoverTable { .. }
                if basis != StoredPolynomialBasisV1::Lagrange)
            || matches!(role, StoredPolynomialRoleV1::LookupSorted { run_log, .. }
                if run_log < k.min(8) || run_log > k || basis != StoredPolynomialBasisV1::Lagrange)
        {
            return Err(StoredPolynomialErrorV1::Layout);
        }
        match role {
            StoredPolynomialRoleV1::QuotientAliasedPart {
                part,
                extension_log,
            } => {
                if basis != StoredPolynomialBasisV1::Coefficient
                    || extension_log == 0
                    || extension_log > STORED_MAX_K_V1 - k
                    || u8::try_from(extension_log).is_err()
                    || part >= (1_u32 << extension_log)
                {
                    return Err(StoredPolynomialErrorV1::Layout);
                }
            }
            StoredPolynomialRoleV1::QuotientPiece { piece } => {
                if basis != StoredPolynomialBasisV1::Coefficient
                    || piece >= (1_u32 << (STORED_MAX_K_V1 - k))
                {
                    return Err(StoredPolynomialErrorV1::Layout);
                }
            }
            _ => (),
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
                return Err(StoredPolynomialErrorV1::Layout);
            }
        }
        Ok(Self {
            proof_context,
            ordinal,
            field,
            basis,
            k,
            role,
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
    /// Return the complete authenticated semantic polynomial role.
    pub const fn role(self) -> StoredPolynomialRoleV1 {
        self.role
    }
    /// Admit advice-specific coordinates without treating another role as an advice column.
    ///
    /// # Errors
    /// Rejects every non-advice role, even when its numeric index matches an advice column.
    pub(crate) const fn advice_coordinates(self) -> Result<(u32, u8), StoredPolynomialErrorV1> {
        match self.role {
            StoredPolynomialRoleV1::Advice { column, phase } => Ok((column, phase)),
            StoredPolynomialRoleV1::LookupCompressed { .. }
            | StoredPolynomialRoleV1::LookupSorted { .. }
            | StoredPolynomialRoleV1::LookupLeftoverTable { .. }
            | StoredPolynomialRoleV1::LookupPermuted { .. }
            | StoredPolynomialRoleV1::CopyPermutationProduct { .. }
            | StoredPolynomialRoleV1::LookupProduct { .. }
            | StoredPolynomialRoleV1::Instance { .. }
            | StoredPolynomialRoleV1::VanishingRandom
            | StoredPolynomialRoleV1::QuotientNumerator
            | StoredPolynomialRoleV1::QuotientAliasedPart { .. }
            | StoredPolynomialRoleV1::QuotientPiece { .. } => Err(StoredPolynomialErrorV1::Context),
        }
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
    pub fn chunk_scalar_count(self, chunk: u64) -> Result<usize, StoredPolynomialErrorV1> {
        let chunk = usize::try_from(chunk).map_err(|_| StoredPolynomialErrorV1::ChunkIndex)?;
        if chunk >= self.chunk_count() {
            return Err(StoredPolynomialErrorV1::ChunkIndex);
        }
        Ok((self.scalar_count() - chunk * STORED_SCALARS_PER_CHUNK_V1)
            .min(STORED_SCALARS_PER_CHUNK_V1))
    }

    /// Compare proof membership without exposing the raw proof context or any storage secret.
    /// Role, ordinal, field and basis must still be checked independently by the caller.
    pub(crate) fn same_proof_context(self, other: Self) -> bool {
        self.proof_context == other.proof_context
    }

    /// Derive the public authenticated context using the backend's existing BLAKE2b primitive.
    ///
    /// This fixed-layout domain binding is not a wire codec, proof commitment, or authority.
    /// All integers are little-endian; scalar bytes use canonical PrimeField::Repr encoding.
    /// The role is a fixed-width tag, u32 index, and u8 detail tuple. Sorted scratch
    /// uses tag 2 and detail = 2 * run_log + side (Input = 0, Table = 1); existing
    /// tags retain their exact original encoding. Leftover-table scratch uses tag 3, detail 0.
    /// Permuted arguments use tag 4, detail Input = 0 or Table = 1 across their legitimate bases. Its domain tag
    /// separates this format from former advice-only bindings. Backends must authenticate this
    /// complete digest together with the chunk index; the digest alone authorizes no read.
    pub fn context_digest(self) -> [u8; 32] {
        let mut hash = Params::new()
            .hash_length(32)
            .personal(b"Halo2PolyStoreV1")
            .to_state();
        hash.update(b"polynomial.snapshot.v1\0canonical-primefield-repr\0zero-tail\0");
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
        let (role_tag, role_index, role_detail) = match self.role {
            StoredPolynomialRoleV1::Advice { column, phase } => (0, column, phase),
            StoredPolynomialRoleV1::LookupCompressed { lookup, side } => (
                1,
                lookup,
                match side {
                    StoredLookupSideV1::Input => 0,
                    StoredLookupSideV1::Table => 1,
                },
            ),
            StoredPolynomialRoleV1::LookupLeftoverTable { lookup } => (3, lookup, 0),
            StoredPolynomialRoleV1::CopyPermutationProduct { set } => (5, set, 0),
            StoredPolynomialRoleV1::LookupProduct { lookup } => (6, lookup, 0),
            StoredPolynomialRoleV1::Instance { column } => (7, column, 0),
            StoredPolynomialRoleV1::VanishingRandom => (8, 0, 0),
            StoredPolynomialRoleV1::QuotientNumerator => (9, 0, 0),
            StoredPolynomialRoleV1::QuotientAliasedPart {
                part,
                extension_log,
            } => (10, part, extension_log as u8),
            StoredPolynomialRoleV1::QuotientPiece { piece } => (11, piece, 0),
            StoredPolynomialRoleV1::LookupPermuted { lookup, side } => (
                4,
                lookup,
                match side {
                    StoredLookupSideV1::Input => 0,
                    StoredLookupSideV1::Table => 1,
                },
            ),
            StoredPolynomialRoleV1::LookupSorted {
                lookup,
                side,
                run_log,
            } => (
                2,
                lookup,
                (run_log as u8) * 2
                    + match side {
                        StoredLookupSideV1::Input => 0,
                        StoredLookupSideV1::Table => 1,
                    },
            ),
        };
        hash.update(&[role_tag]);
        hash.update(&role_index.to_le_bytes());
        hash.update(&[role_detail]);
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
/// plaintext-window lease across every role, writer and snapshot it creates. It must not use the
/// proof transcript RNG to generate storage keys/nonces or include storage metadata in proofs.
pub trait StoredPolynomialProviderV1 {
    /// Backend writer; no raw path, key, descriptor, or digest constructor is exposed.
    type Writer: StoredPolynomialWriterV1;
    /// Create one immutable polynomial destination with exact trusted coordinates.
    ///
    /// # Errors
    /// Rejects invalid geometry, exhausted handles, an active window, or backend failures.
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1>;
}

/// Sequential write-once destination for an already-frozen polynomial.
pub trait StoredPolynomialWriterV1: Sized {
    /// Authenticated immutable read owner returned only after complete sealing.
    type Snapshot: StoredPolynomialSnapshotV1;
    /// Return exact metadata.
    fn layout(&self) -> StoredPolynomialLayoutV1;
    /// Consume the next exact logical chunk; the backend supplies canonical zero padding.
    ///
    /// # Errors
    /// Invalid indices/counts/encodings are retryable preflights. Operational failures poison.
    fn write_chunk(
        &mut self,
        chunk: u64,
        scalars: &[[u8; 32]],
    ) -> Result<(), StoredPolynomialErrorV1>;
    /// Authenticate every slot and consume the writer into a repeatable read owner.
    ///
    /// # Errors
    /// Rejects incomplete writes, an active window, or any backend authentication/I/O failure.
    fn seal(self) -> Result<Self::Snapshot, StoredPolynomialErrorV1>;
}

/// Authenticated immutable snapshot with borrowed, bounded plaintext access.
///
/// Callbacks cannot retain a borrow of backend memory. Caller-created copies are outside this
/// interface's storage bound. Callback error/panic must destroy this snapshot's key/file and
/// zeroize partial materialization; pure metadata/index/Busy preflight errors may be retried.
pub trait StoredPolynomialSnapshotV1 {
    /// Return exact metadata.
    fn layout(&self) -> StoredPolynomialLayoutV1;
    /// Read one authenticated chunk, omitting checked canonical tail padding.
    ///
    /// # Errors
    /// Rejects wrong expected metadata, slot/encoding/authentication/I/O failures, or a busy window.
    fn with_chunk<R>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredPolynomialErrorV1>,
    ) -> Result<R, StoredPolynomialErrorV1>;
    /// Authenticate and materialize exactly one encoded polynomial for a bounded callback.
    ///
    /// # Errors
    /// Rejects wrong metadata, a busy window, allocation/read/decoding failure, or consumer error.
    fn with_column<R>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredPolynomialErrorV1>,
    ) -> Result<R, StoredPolynomialErrorV1>;
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "stored_advice/lookup_membership_role_tests.rs"]
mod lookup_membership_role_tests;
